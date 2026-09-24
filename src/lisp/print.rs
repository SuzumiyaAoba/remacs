//! Printing Lisp objects (`prin1`-style readable syntax).

use std::cell::RefCell;
use std::fmt::Write;
use std::rc::Rc;

use super::Interp;
use super::value::Value;

/// Escape a character inside a printed string. Emacs escapes only
/// `"` and `\` — newlines and other control chars print literally
/// unless `print-escape-newlines`/`print-escape-multibyte` are set.
fn escape_char_for_string(c: char, out: &mut String, nl: bool, mb: bool) {
    match c {
        '"' => out.push_str("\\\""),
        '\\' => out.push_str("\\\\"),
        '\n' if nl => out.push_str("\\n"),
        c if mb && (c as u32) > 0x7f => {
            let _ = write!(out, "\\x{:x}", c as u32);
        }
        c => out.push(c),
    }
}

/// Does `c` need a backslash escape inside a printed symbol name?
/// Emacs escapes the chars that would otherwise terminate or
/// re-interpret the symbol token.
fn sym_char_needs_escape(c: char) -> bool {
    matches!(
        c,
        '(' | ')' | '[' | ']' | '\'' | '"' | '`' | ',' | ';' | '#' | '\\'
    ) || c.is_whitespace()
        || (c as u32) < 0x20
        || c == '\u{7f}'
}

/// Print a symbol name, backslash-escaping special chars (`a\ b`, `\,`).
fn push_sym_name(name: &str, out: &mut String) {
    if name.is_empty() {
        out.push_str("##");
        return;
    }
    for c in name.chars() {
        if sym_char_needs_escape(c) {
            out.push('\\');
        }
        out.push(c);
    }
}

thread_local! {
    /// GNU print.c's `being_printed' stack (used when print-circle
    /// is nil): every object printing at depth D sits in slot D, and
    /// an object that reoccurs while still on the stack prints as
    /// `#N' where N is its stack index.  Only containers can be
    /// ancestors, so only they are tracked.
    static BEING_PRINTED: RefCell<Vec<usize>> = const { RefCell::new(Vec::new()) };
    /// `print-circle' state for the current top-level print call:
    /// objects seen by the pre-scan, labels numbered at each shared
    /// object's second encounter (GNU's `print_number_index' order),
    /// and which labeled objects have already printed.
    static CIRCLE: RefCell<Option<CircleCtx>> = const { RefCell::new(None) };
}

/// Per-print state for `print-circle'.
struct CircleCtx {
    /// Objects already seen by the pre-scan.
    seen: std::collections::HashSet<usize>,
    /// ptr -> label number, assigned at second encounter.
    labels: std::collections::HashMap<usize, usize>,
    /// Objects whose `#N=' form has already printed.
    printed: std::collections::HashSet<usize>,
    /// Next label index (GNU's print_number_index starts at 1).
    next: usize,
}

/// Identity key for objects that can appear in the print stack —
/// containers by `eq' identity (Rc pointer), like GNU's BASE_EQ.
fn print_stack_key(v: &Value) -> Option<usize> {
    match v {
        Value::Cons(r) => Some(std::rc::Rc::as_ptr(r) as usize),
        Value::Str(r) => Some(std::rc::Rc::as_ptr(r) as usize),
        Value::Vec(r) => Some(std::rc::Rc::as_ptr(r) as usize),
        Value::Record(r) => Some(std::rc::Rc::as_ptr(r) as usize),
        Value::Hash(r) => Some(std::rc::Rc::as_ptr(r) as usize),
        _ => None,
    }
}

/// GNU's `print_preprocess' pass: depth-first walk of the object
/// graph (car before cdr, like GNU's recursion), numbering each
/// shared/cyclic object when it is encountered a second time.
/// Already-seen objects are not descended into, so cycles terminate.
fn count_print_refs(v: &Value, ctx: &mut CircleCtx) {
    let mut stack = vec![v.clone()];
    while let Some(x) = stack.pop() {
        let Some(k) = print_stack_key(&x) else { continue };
        if !ctx.seen.insert(k) {
            // Second encounter: GNU assigns `print_number_index'
            // here, not at print time.
            if !ctx.labels.contains_key(&k) {
                ctx.labels.insert(k, ctx.next);
                ctx.next += 1;
            }
            continue;
        }
        match &x {
            Value::Cons(c) => {
                let (car, cdr) = {
                    let b = c.borrow();
                    (b.car.clone(), b.cdr.clone())
                };
                stack.push(cdr);
                stack.push(car);
            }
            Value::Vec(r) | Value::Record(r) => {
                stack.extend(r.borrow().iter().rev().cloned());
            }
            Value::Hash(h) => {
                let hh = h.borrow();
                for (_, orig) in hh.keys.iter() {
                    stack.push(orig.clone());
                }
                stack.extend(hh.map.values().cloned());
            }
            _ => {}
        }
    }
}

/// Is `print-circle' handling active for the current print?
fn circle_active() -> bool {
    CIRCLE.with(|c| c.borrow().is_some())
}

/// In `print-circle' mode: if K's object was numbered by the
/// pre-scan, return Some((label, true-if-first-occurrence)).
/// None for singly referenced objects.
fn circle_label(k: usize) -> Option<(usize, bool)> {
    CIRCLE.with(|c| {
        let mut binding = c.borrow_mut();
        let ctx = binding.as_mut()?;
        let n = ctx.labels.get(&k).copied()?;
        Some((n, ctx.printed.insert(k)))
    })
}

/// Does K's object need a `print-circle' label (shared/cyclic)?
fn circle_counted(k: usize) -> bool {
    CIRCLE.with(|c| {
        c.borrow()
            .as_ref()
            .map(|ctx| ctx.labels.contains_key(&k))
            .unwrap_or(false)
    })
}

impl Interp {
    /// `prin1` representation: readable, escaped.
    pub fn print_to_string(&self, v: &Value) -> String {
        let mut s = String::new();
        let mark = BEING_PRINTED.with(|bp| bp.borrow().len());
        let prev_circle = self.install_circle_ctx(v);
        self.prin1_inner(v, &mut s, 0, false);
        // Unwinding safety: restore the stack mark so a mid-print
        // panic can't poison later prints.
        CIRCLE.with(|c| *c.borrow_mut() = prev_circle);
        BEING_PRINTED.with(|bp| bp.borrow_mut().truncate(mark));
        s
    }

    /// Alias for `print_to_string` matching the Lisp name.
    pub fn prin1_to_string(&self, v: &Value) -> String {
        self.print_to_string(v)
    }

    /// `princ` representation: human-readable (strings unquoted).
    pub fn princ_to_string(&self, v: &Value) -> String {
        let mut s = String::new();
        let mark = BEING_PRINTED.with(|bp| bp.borrow().len());
        let prev_circle = self.install_circle_ctx(v);
        self.princ_inner(v, &mut s, 0, false);
        CIRCLE.with(|c| *c.borrow_mut() = prev_circle);
        BEING_PRINTED.with(|bp| bp.borrow_mut().truncate(mark));
        s
    }

    /// When `print-circle' is non-nil, pre-scan V for shared/cyclic
    /// objects and install the per-print state.  Returns the previous
    /// state so callers can restore it (nested print calls).
    fn install_circle_ctx(&self, v: &Value) -> Option<CircleCtx> {
        if !self.print_var("print-circle").truthy() {
            return None;
        }
        let mut ctx = CircleCtx {
            seen: std::collections::HashSet::new(),
            labels: std::collections::HashMap::new(),
            printed: std::collections::HashSet::new(),
            next: 1,
        };
        count_print_refs(v, &mut ctx);
        CIRCLE.with(|c| c.borrow_mut().replace(ctx))
    }

    /// When ITEMS is an `[overlay BID IDX]' handle, produce GNU's
    /// `#<overlay from B to E in NAME>' (or `#<overlay in no buffer>'
    /// once detached/killed).
    fn overlay_repr(&self, items: &Rc<RefCell<Vec<Value>>>) -> Option<String> {
        let vv = items.borrow();
        if vv.len() != 3 {
            return None;
        }
        let (bid, idx) = match (&vv[0], &vv[1], &vv[2]) {
            (Value::Sym(tag), Value::Int(b), Value::Int(x))
                if Some(*tag) == self.intern_soft("overlay") =>
            {
                (*b as usize, *x as usize)
            }
            _ => return None,
        };
        let ov = match self
            .buffers
            .get(bid)
            .and_then(|b| b.borrow().overlays.get(idx).cloned())
        {
            Some(o) => o,
            // Detached/killed handles still print as overlays.
            None => return Some("#<overlay in no buffer>".into()),
        };
        match ov
            .buffer
            .and_then(|id| self.buffers.get(id).map(|b| b.borrow().name.clone()))
        {
            Some(name) => Some(format!(
                "#<overlay from {} to {} in {}>",
                ov.start + 1,
                ov.end + 1,
                name
            )),
            None => Some("#<overlay in no buffer>".into()),
        }
    }

    fn prin1_inner(&self, v: &Value, out: &mut String, depth: usize, bq: bool) {
        if let Some(k) = print_stack_key(v) {
            if circle_active() {
                // `print-circle': shared/cyclic objects print `#N='
                // at first occurrence and `#N#' afterwards.
                match circle_label(k) {
                    Some((n, false)) => {
                        let _ = write!(out, "#{n}#");
                        return;
                    }
                    Some((n, true)) => {
                        let _ = write!(out, "#{n}=");
                    }
                    None => {}
                }
                self.prin1_inner_obj(v, out, depth, bq);
                return;
            }
            // GNU's being-printed check: an object still on the print
            // stack prints as `#N' (its stack index).
            let hit = BEING_PRINTED.with(|bp| bp.borrow().iter().position(|&x| x == k));
            if let Some(i) = hit {
                let _ = write!(out, "#{i}");
                return;
            }
            BEING_PRINTED.with(|bp| bp.borrow_mut().push(k));
            self.prin1_inner_obj(v, out, depth, bq);
            BEING_PRINTED.with(|bp| {
                bp.borrow_mut().pop();
            });
            return;
        }
        self.prin1_inner_obj(v, out, depth, bq);
    }

    fn prin1_inner_obj(&self, v: &Value, out: &mut String, depth: usize, bq: bool) {
        if depth > 200 {
            out.push_str("##");
            return;
        }
        // print-level: nested structure deeper than the limit prints "...".
        if let Some(level) = self.print_level_limit() {
            if depth >= level {
                match v {
                    Value::Cons(_) | Value::Vec(_) | Value::Record(_) | Value::Hash(_) => {
                        out.push_str("...");
                        return;
                    }
                    _ => {}
                }
            }
        }
        match v {
            Value::Nil => out.push_str("nil"),
            Value::Int(i) => {
                let _ = write!(out, "{i}");
            }
            Value::Float(f) => {
                let _ = write!(out, "{}", format_float(**f));
            }
            Value::Sym(id) => {
                if self.obarray.symbol(*id).uninterned && self.print_gensym() {
                    out.push_str("#:");
                }
                let name = self.symbol_name(*id);
                // Emacs escapes a symbol whose name would read back
                // as a number (`\52') or one made solely of dots
                // (`\.', `\..').
                if super::reader::parse_number(&name).is_some()
                    || (!name.is_empty() && name.chars().all(|c| c == '.'))
                {
                    out.push('\\');
                }
                push_sym_name(&name, out);
            }
            Value::Str(s) => {
                let nl = self.print_escape_newlines();
                let mb = self.print_escape_multibyte();
                // GNU prints #("..." s e (plist) ...) when the string
                // carries non-empty text-property intervals.  The
                // `charset' prop is hidden unless
                // `print-charset-text-property' is t (or `default',
                // which shows only map-based charsets).
                let pct = self.print_var("print-charset-text-property");
                let charset_id = self.intern_soft("charset");
                // nil → hide all charset props; `default' → only
                // map-based charsets; anything else → show all.
                let show_all = charset_id.is_none()
                    || (!pct.is_nil()
                        && !matches!(pct, Value::Sym(s) if self.symbol_name(s) == "default"));
                let visible = |pl: &Vec<Value>| -> Vec<Value> {
                    if show_all {
                        return pl.clone();
                    }
                    let hide_all = pct.is_nil();
                    const MAP_CHARSETS: &[&str] = &[
                        "iso-8859-1", "iso-8859-2", "iso-8859-3", "iso-8859-4",
                        "iso-8859-5", "iso-8859-6", "iso-8859-7", "iso-8859-8",
                        "iso-8859-9", "iso-8859-10", "iso-8859-11", "iso-8859-13",
                        "iso-8859-14", "iso-8859-15", "iso-8859-16",
                        "koi8", "koi8-r", "windows-1251", "mac-roman",
                        "big5", "chinese-gb2312",
                    ];
                    let cid = charset_id.unwrap();
                    let mut out: Vec<Value> = Vec::new();
                    let mut k = 0;
                    while k + 1 < pl.len() {
                        let hide = if self.sym_id(&pl[k]) == Some(cid) {
                            if hide_all {
                                true
                            } else {
                                let name = match &pl[k + 1] {
                                    Value::Sym(id) => self.symbol_name(*id),
                                    _ => String::new(),
                                };
                                !MAP_CHARSETS.contains(&name.as_str())
                            }
                        } else {
                            false
                        };
                        if !hide {
                            out.push(pl[k].clone());
                            out.push(pl[k + 1].clone());
                        }
                        k += 2;
                    }
                    out
                };
                let ivs = self.str_props(s);
                let printed: Vec<(usize, usize, Vec<Value>)> = ivs
                    .iter()
                    .map(|(a, b, pl)| (*a, *b, visible(pl)))
                    .filter(|(_, _, pl)| !pl.is_empty())
                    .collect();
                if !printed.is_empty() {
                    out.push_str("#(");
                }
                out.push('"');
                if self.is_unibyte_str(s) {
                    // Unibyte strings (encoder output): byte-chars
                    // ≥0x80 print as `\NNN' octal escapes like GNU.
                    for c in s.borrow().chars() {
                        if (c as u32) >= 0x80 {
                            let _ = write!(out, "\\{:03o}", c as u32);
                        } else {
                            escape_char_for_string(c, out, nl, mb);
                        }
                    }
                } else {
                    for c in s.borrow().chars() {
                        // Eight-bit chars print as their byte's octal
                        // escape even in multibyte strings.
                        if let Some(b) = crate::lisp::value::eight_bit_byte(c) {
                            let _ = write!(out, "\\{:03o}", b);
                        } else {
                            escape_char_for_string(c, out, nl, mb);
                        }
                    }
                }
                out.push('"');
                let wrapped = !printed.is_empty();
                for (a, b, pl) in printed {
                    let _ = write!(out, " {a} {b} ");
                    self.prin1_inner(&Value::list(pl), out, depth + 1, bq);
                }
                if wrapped {
                    out.push(')');
                }
            }
            Value::Cons(_) => self.print_list(v, out, depth, bq),
            Value::Vec(items) => {
                // `[overlay BID IDX]' handles print like GNU overlays.
                if let Some(r) = self.overlay_repr(items) {
                    out.push_str(&r);
                    return;
                }
                let limit = self.print_length_limit();
                out.push('[');
                let mut n = 0usize;
                let mut truncated = false;
                for item in items.borrow().iter() {
                    if let Some(l) = limit {
                        if n >= l {
                            truncated = true;
                            break;
                        }
                    }
                    if n > 0 {
                        out.push(' ');
                    }
                    self.prin1_inner(item, out, depth + 1, bq);
                    n += 1;
                }
                if truncated {
                    out.push_str(if n == 0 { "..." } else { " ..." });
                }
                out.push(']');
            }
            Value::Record(items) => {
                let rr = items.borrow();
                // Positioned symbols print `#<symbol NAME at POS>'.
                if let Some(Value::Sym(t)) = rr.first() {
                    if self.symbol_name(*t) == "symbol-with-pos" {
                        if let [_, Value::Sym(s), Value::Int(p)] = rr.as_slice() {
                            let name = self.symbol_name(*s);
                            let _ = write!(out, "#<symbol {} at {}>", name, p);
                            return;
                        }
                    }
                    if self.symbol_name(*t) == "window-configuration" {
                        let _ = write!(out, "#<window-configuration>");
                        return;
                    }
                    // Obarrays print `#<obarray n=COUNT>' — COUNT is
                    // the number of interned symbols.
                    if self.symbol_name(*t) == "obarray" {
                        let n = match rr.get(1) {
                            Some(Value::Vec(syms)) => syms
                                .borrow()
                                .iter()
                                .filter(|x| matches!(x, Value::Sym(_)))
                                .count(),
                            _ => 0,
                        };
                        let _ = write!(out, "#<obarray n={}>", n);
                        return;
                    }
                    // Terminals print `#<terminal N on NAME>'.
                    if self.symbol_name(*t) == "terminal" {
                        if let [_, Value::Int(n), Value::Str(name)] =
                            rr.as_slice()
                        {
                            let _ = write!(
                                out,
                                "#<terminal {} on {}>",
                                n,
                                name.borrow()
                            );
                            return;
                        }
                    }
                    // Sub char tables print `#^^[DEPTH MIN-CHAR
                    // SLOTS...]' — GNU's trie nodes.
                    if self.symbol_name(*t) == "sub-char-table" {
                        out.push_str("#^^[");
                        if let Some(d) = rr.get(1) {
                            self.prin1_inner(d, out, depth + 1, bq);
                        }
                        out.push(' ');
                        if let Some(m) = rr.get(2) {
                            self.prin1_inner(m, out, depth + 1, bq);
                        }
                        for s in &rr[3..] {
                            out.push(' ');
                            self.prin1_inner(s, out, depth + 1, bq);
                        }
                        out.push(']');
                        return;
                    }
                    // Char tables print `#^[defalt parent purpose
                    // ascii contents[64] extras...]' — GNU's
                    // pseudovector layout.
                    if self.symbol_name(*t) == "char-table" {
                        if let Some(Value::Vec(slots)) = rr.get(2) {
                            let ptr = std::rc::Rc::as_ptr(items) as usize;
                            let slots = slots.borrow();
                            self.print_char_table(
                                ptr,
                                rr.as_slice(),
                                slots.as_slice(),
                                out,
                                depth,
                                bq,
                            );
                            return;
                        }
                    }
                }
                // Bool vectors print `#&N"bytes"' with bits packed
                // LSB-first per byte.
                let is_bv = matches!(rr.first(), Some(Value::Sym(t))
                    if self.symbol_name(*t) == "bool-vector");
                if is_bv {
                    if let Some(Value::Vec(bits)) = rr.get(1) {
                        let bits = bits.borrow();
                        let n = bits.len();
                        let _ = write!(out, "#&{}\"", n);
                        for k in 0..n.div_ceil(8) {
                            let mut byte: u32 = 0;
                            for j in 0..8 {
                                if let Some(Value::Int(b)) = bits.get(k * 8 + j) {
                                    if *b != 0 {
                                        byte |= 1 << j;
                                    }
                                }
                            }
                            match byte {
                                34 => out.push_str("\\\""),
                                92 => out.push_str("\\\\"),
                                0..=127 => out.push(byte as u8 as char),
                                _ => {
                                    let _ = write!(out, "\\{:03o}", byte);
                                }
                            }
                        }
                        out.push('"');
                        return;
                    }
                }
                drop(rr);
                let rr = items.borrow();
                out.push_str("#s(");
                for (i, item) in rr.iter().enumerate() {
                    if i > 0 {
                        out.push(' ');
                    }
                    self.prin1_inner(item, out, depth + 1, bq);
                }
                out.push(')');
            }
            Value::Hash(h) => {
                // GNU: #s(hash-table) for defaults; `test' appears only
                // when non-eql, `data' only when non-empty.
                let hh = h.borrow();
                let test = match hh.test {
                    crate::lisp::value::HashTest::Eq => Some("eq"),
                    crate::lisp::value::HashTest::Eql => None,
                    crate::lisp::value::HashTest::Equal => Some("equal"),
                };
                let empty = hh.keys.is_empty();
                out.push_str("#s(hash-table");
                if let Some(t) = test {
                    let _ = write!(out, " test {}", t);
                }
                if let Some(w) = &hh.weakness {
                    out.push_str(" weakness ");
                    self.prin1_inner(w, out, depth + 1, bq);
                }
                if !empty {
                    out.push_str(" data (");
                    let mut first = true;
                    for (hk, k) in hh.keys.iter() {
                        let Some(v) = hh.map.get(hk) else {
                            continue;
                        };
                        if !first {
                            out.push(' ');
                        }
                        first = false;
                        self.prin1_inner(k, out, depth + 1, bq);
                        out.push(' ');
                        self.prin1_inner(v, out, depth + 1, bq);
                    }
                    out.push(')');
                }
                out.push(')');
            }
            Value::Subr(s) => {
                let _ = write!(out, "#<subr {}>", s.name);
            }
            Value::Lambda(l) => {
                // Emacs 31 prints interpreted functions like
                // #[(x) (x) nil] — arglist, body forms, environment.
                out.push_str("#[");
                self.print_lambda_list(l, out);
                out.push(' ');
                // An empty body is a single implicit nil form.
                let body = if l.body.is_empty() {
                    Value::list(vec![Value::Nil])
                } else {
                    Value::list(l.body.clone())
                };
                self.prin1_inner(&body, out, depth + 1, bq);
                out.push(' ');
                match &l.env {
                    // A dynamic (non-lexical) lambda has no captured
                    // environment: GNU prints `nil'.
                    None => out.push_str("nil"),
                    Some(frame) => {
                        let env = lex_frame_to_value(self, frame);
                        self.prin1_inner(&env, out, depth + 1, bq);
                    }
                }
                out.push(']');
            }
            Value::Buffer(b) => {
                let bb = b.borrow();
                if bb.live {
                    let _ = write!(out, "#<buffer {}>", bb.name);
                } else {
                    out.push_str("#<killed buffer>");
                }
            }
            Value::Marker(m) => {
                let b = m.borrow();
                match b.buffer {
                    Some(buf) => {
                        let _ = write!(
                            out,
                            "#<marker {}at {} in {}>",
                            if b.insertion_type {
                                "(moves after insertion) "
                            } else {
                                ""
                            },
                            b.position + 1,
                            self.buffer_name_by_id(buf)
                        );
                    }
                    None => out.push_str("#<marker in no buffer>"),
                }
            }
            Value::Window(w) => {
                let b = w.borrow();
                let _ = write!(
                    out,
                    "#<window {} on {}>",
                    b.id,
                    self.buffer_name_by_id(b.buffer)
                );
            }
            Value::Frame(f) => {
                let _ = write!(out, "#<frame {} {:#x}>", f.borrow().name, Rc::as_ptr(f) as usize);
            }
            Value::Process(p) => {
                let _ = write!(out, "#<process {}>", p.borrow().name);
            }
            // GNU prints the machine address; any unique pointer works.
            Value::Thread(t) => {
                let _ = write!(out, "#<thread 0x{:x}>", Rc::as_ptr(t) as usize);
            }
            Value::Mutex(m) => {
                let mm = m.borrow();
                match &mm.name {
                    Some(n) => {
                        let _ = write!(out, "#<mutex {n}>");
                    }
                    None => out.push_str("#<mutex>"),
                }
            }
            Value::CondVar(c) => {
                let cc = c.borrow();
                match &cc.name {
                    Some(n) => {
                        let _ = write!(out, "#<condvar {n}>");
                    }
                    None => out.push_str("#<condvar>"),
                }
            }
            Value::Finalizer(_) => out.push_str("#<finalizer>"),
        }
    }

    fn princ_inner(&self, v: &Value, out: &mut String, depth: usize, bq: bool) {
        if let Some(k) = print_stack_key(v) {
            if circle_active() {
                match circle_label(k) {
                    Some((n, false)) => {
                        let _ = write!(out, "#{n}#");
                        return;
                    }
                    Some((n, true)) => {
                        let _ = write!(out, "#{n}=");
                    }
                    None => {}
                }
                self.princ_inner_obj(v, out, depth, bq);
                return;
            }
            // Same being-printed circularity check as prin1.
            let hit = BEING_PRINTED.with(|bp| bp.borrow().iter().position(|&x| x == k));
            if let Some(i) = hit {
                let _ = write!(out, "#{i}");
                return;
            }
            BEING_PRINTED.with(|bp| bp.borrow_mut().push(k));
            self.princ_inner_obj(v, out, depth, bq);
            BEING_PRINTED.with(|bp| {
                bp.borrow_mut().pop();
            });
            return;
        }
        self.princ_inner_obj(v, out, depth, bq);
    }

    fn princ_inner_obj(&self, v: &Value, out: &mut String, depth: usize, bq: bool) {
        if let Some(level) = self.print_level_limit() {
            if depth >= level {
                match v {
                    Value::Cons(_) | Value::Vec(_) | Value::Record(_) | Value::Hash(_) => {
                        out.push_str("...");
                        return;
                    }
                    _ => {}
                }
            }
        }
        match v {
            Value::Str(s) => out.push_str(&s.borrow()),
            // princ prints symbol names raw — no backslash escapes.
            Value::Sym(id) => out.push_str(&self.symbol_name(*id)),
            // princ prints live buffers as bare names; a killed buffer
            // prints `#<killed buffer>' like the escaped form.
            Value::Buffer(b) => {
                let bb = b.borrow();
                if bb.live {
                    out.push_str(&bb.name);
                } else {
                    out.push_str("#<killed buffer>");
                }
            }
            Value::Process(p) => out.push_str(&p.borrow().name),
            Value::Cons(_) => self.print_list_princ(v, out, depth, bq),
            Value::Vec(items) => {
                if let Some(r) = self.overlay_repr(items) {
                    out.push_str(&r);
                    return;
                }
                out.push('[');
                for (i, item) in items.borrow().iter().enumerate() {
                    if i > 0 {
                        out.push(' ');
                    }
                    self.princ_inner(item, out, depth + 1, bq);
                }
                out.push(']');
            }
            _ => self.prin1_inner(v, out, depth, bq),
        }
    }

    fn print_lambda_list(&self, l: &super::value::Lambda, out: &mut String) {
        if let Some(raw) = &l.arglist {
            self.prin1_inner(raw, out, 1, false);
            return;
        }
        if l.required.is_empty() && l.optional.is_empty() && l.rest.is_none() {
            out.push_str("nil");
            return;
        }
        out.push('(');
        let mut first = true;
        let mut names: Vec<String> = l
            .required
            .iter()
            .map(|s| self.symbol_name(*s).to_string())
            .collect();
        if !l.optional.is_empty() {
            names.push("&optional".into());
            for o in &l.optional {
                names.push(self.symbol_name(o.sym).to_string());
            }
        }
        if let Some(r) = l.rest {
            names.push("&rest".into());
            names.push(self.symbol_name(r).to_string());
        }
        for n in names {
            if !first {
                out.push(' ');
            }
            first = false;
            out.push_str(&n);
        }
        out.push(')');
    }

    /// GNU char-table printing: `#^[DEFALT PARENT PURPOSE ASCII
    /// CONTENTS[64] EXTRAS...]'.  Our Record layout is
    /// `[char-table SUBTYPE VEC65 EXTRAS...]' where VEC65 is the trie
    /// root: slot 0 = ASCII cache (scalar or the `#^^[3 0' leaf),
    /// slots 1-64 = top-level blocks.  Sub-char-tables print through
    /// `prin1_inner's `sub-char-table' Record branch as `#^^[...]'.
    fn print_char_table(
        &self,
        ptr: usize,
        rr: &[Value],
        slots: &[Value],
        out: &mut String,
        depth: usize,
        bq: bool,
    ) {
        out.push_str("#^[");
        // defalt (side-table keyed on the Record pointer)
        let defalt = self
            .char_table_defalts
            .iter()
            .find(|(k, _)| *k == ptr)
            .map(|(_, v)| v.clone())
            .unwrap_or(Value::Nil);
        self.prin1_inner(&defalt, out, depth + 1, bq);
        out.push(' ');
        // parent (side-table keyed on the Record pointer)
        let parent = self
            .char_table_parents
            .iter()
            .find(|(k, _)| *k == ptr)
            .map(|(_, v)| v.clone())
            .unwrap_or(Value::Nil);
        self.prin1_inner(&parent, out, depth + 1, bq);
        out.push(' ');
        // purpose
        match rr.get(1) {
            Some(p) => self.prin1_inner(p, out, depth + 1, bq),
            None => out.push_str("nil"),
        }
        // ASCII cache + contents[0..64]
        for s in slots {
            out.push(' ');
            self.prin1_inner(s, out, depth + 1, bq);
        }
        // extra slots
        for e in &rr[3..] {
            out.push(' ');
            self.prin1_inner(e, out, depth + 1, bq);
        }
        out.push(']');
    }

    /// `print-gensym` variable (default nil): print uninterned
    /// symbols with `#:' only when non-nil.
    fn print_gensym(&self) -> bool {
        self.intern_soft("print-gensym")
            .filter(|id| self.bound_p(*id))
            .map(|id| self.symbol_value(id).truthy())
            .unwrap_or(false)
    }

    /// `print-quoted` variable (default t): abbreviate
    /// (quote x) -> 'x, (function x) -> #'x, (\` x) -> `x, etc.
    fn print_quoted(&self) -> bool {
        self.intern_soft("print-quoted")
            .filter(|id| self.bound_p(*id))
            .map(|id| self.symbol_value(id).truthy())
            .unwrap_or(true)
    }

    fn print_var(&self, name: &str) -> Value {
        self.intern_soft(name)
            .filter(|id| self.bound_p(*id))
            .map(|id| self.symbol_value(id))
            .unwrap_or(Value::Nil)
    }

    /// `print-length` (default nil): max elements printed per sequence.
    fn print_length_limit(&self) -> Option<usize> {
        self.print_var("print-length")
            .int()
            .map(|n| n.max(0) as usize)
    }

    /// `print-level` (default nil): max nesting depth before "...".
    fn print_level_limit(&self) -> Option<usize> {
        self.print_var("print-level")
            .int()
            .map(|n| n.max(0) as usize)
    }

    /// `print-escape-newlines` (default nil): escape `\n` in strings.
    fn print_escape_newlines(&self) -> bool {
        self.print_var("print-escape-newlines").truthy()
    }

    /// `print-escape-multibyte` (default nil): hex-escape non-ASCII.
    fn print_escape_multibyte(&self) -> bool {
        self.print_var("print-escape-multibyte").truthy()
    }

    /// If `v` is a 2-element list (QUOTE x), (FUNCTION x), (\` x),
    /// (\, x) etc., return the abbreviated prefix for `print_quoted`.
    fn quote_abbrev(&self, v: &Value, bq: bool) -> Option<(&'static str, Value, bool)> {
        use super::obarray::sym;
        if !self.print_quoted() {
            return None;
        }
        if let Value::Cons(c) = v {
            let (car, cdr) = {
                let b = c.borrow();
                (b.car.clone(), b.cdr.clone())
            };
            // `(\, x)` sugar only inside a `` ` `` context; outside the
            // comma symbol prints escaped as `\,`.
            let (prefix, inner_bq) = match car {
                Value::Sym(sym::QUOTE) => ("'", bq),
                Value::Sym(sym::FUNCTION) => ("#'", bq),
                Value::Sym(sym::BACKQUOTE) => ("`", true),
                Value::Sym(sym::COMMA) if bq => (",", bq),
                Value::Sym(sym::COMMA_AT) if bq => (",@", bq),
                Value::Sym(sym::COMMA_DOT) if bq => (",.", bq),
                _ => return None,
            };
            if let Value::Cons(c2) = &cdr {
                let (cadr, cddr) = {
                    let b = c2.borrow();
                    (b.car.clone(), b.cdr.clone())
                };
                if cddr.is_nil() {
                    return Some((prefix, cadr, inner_bq));
                }
            }
        }
        None
    }

    /// Print a (possibly dotted) list.
    fn print_list(&self, v: &Value, out: &mut String, depth: usize, bq: bool) {
        if let Some((prefix, inner, inner_bq)) = self.quote_abbrev(v, bq) {
            out.push_str(prefix);
            self.prin1_inner(&inner, out, depth + 1, inner_bq);
            return;
        }
        let limit = self.print_length_limit();
        out.push('(');
        // GNU print.c: `(' ELEM, then ` ' ELEM per continuation cell;
        // a Brent-style tortoise walking the cdr chain detects a cdr
        // cycle and prints `. #N)' with the tortoise's list index.
        let Value::Cons(head) = v else { return };
        let mut cur = {
            let b = head.borrow();
            let car = b.car.clone();
            let next = b.cdr.clone();
            drop(b);
            if limit == Some(0) {
                // GNU prints `(...)' when print-length is 0.
                out.push_str("...)");
                return;
            }
            self.prin1_inner(&car, out, depth + 1, bq);
            next
        };
        let mut tortoise = v.clone();
        let (mut n, mut m, mut idx) = (2usize, 2usize, 0usize);
        let mut printed = 1usize;
        loop {
            match cur {
                Value::Cons(_) => {
                    // `print-circle': a shared/cyclic tail prints as
                    // `. #N=(...)' (first occurrence) or `. #N#';
                    // GNU checks this before `print-length'.
                    if circle_active()
                        && print_stack_key(&cur).map(circle_counted).unwrap_or(false)
                    {
                        out.push_str(" . ");
                        self.prin1_inner(&cur, out, depth + 1, bq);
                        out.push(')');
                        return;
                    }
                    if let Some(l) = limit {
                        if printed >= l {
                            out.push_str(" ...");
                            out.push(')');
                            return;
                        }
                    }
                    out.push(' ');
                    n -= 1;
                    if n == 0 {
                        idx += m;
                        m <<= 1;
                        n = m;
                        tortoise = cur.clone();
                    } else if super::builtins::eq_values(&cur, &tortoise) {
                        let _ = write!(out, ". #{idx})");
                        return;
                    }
                    let Value::Cons(c) = &cur else { unreachable!() };
                    let (car, next) = {
                        let b = c.borrow();
                        (b.car.clone(), b.cdr.clone())
                    };
                    printed += 1;
                    self.prin1_inner(&car, out, depth + 1, bq);
                    cur = next;
                }
                Value::Nil => {
                    out.push(')');
                    return;
                }
                other => {
                    out.push_str(" . ");
                    self.prin1_inner(&other, out, depth + 1, bq);
                    out.push(')');
                    return;
                }
            }
        }
    }

    fn print_list_princ(&self, v: &Value, out: &mut String, depth: usize, bq: bool) {
        if let Some((prefix, inner, inner_bq)) = self.quote_abbrev(v, bq) {
            out.push_str(prefix);
            self.princ_inner(&inner, out, depth + 1, inner_bq);
            return;
        }
        let limit = self.print_length_limit();
        out.push('(');
        let Value::Cons(head) = v else { return };
        let mut cur = {
            let b = head.borrow();
            let car = b.car.clone();
            let next = b.cdr.clone();
            drop(b);
            if limit == Some(0) {
                out.push_str("...)");
                return;
            }
            self.princ_inner(&car, out, depth + 1, bq);
            next
        };
        let mut tortoise = v.clone();
        let (mut n, mut m, mut idx) = (2usize, 2usize, 0usize);
        let mut printed = 1usize;
        loop {
            match cur {
                Value::Cons(_) => {
                    if circle_active()
                        && print_stack_key(&cur).map(circle_counted).unwrap_or(false)
                    {
                        out.push_str(" . ");
                        self.princ_inner(&cur, out, depth + 1, bq);
                        out.push(')');
                        return;
                    }
                    if let Some(l) = limit {
                        if printed >= l {
                            out.push_str(" ...");
                            out.push(')');
                            return;
                        }
                    }
                    out.push(' ');
                    n -= 1;
                    if n == 0 {
                        idx += m;
                        m <<= 1;
                        n = m;
                        tortoise = cur.clone();
                    } else if super::builtins::eq_values(&cur, &tortoise) {
                        let _ = write!(out, ". #{idx})");
                        return;
                    }
                    let Value::Cons(c) = &cur else { unreachable!() };
                    let (car, next) = {
                        let b = c.borrow();
                        (b.car.clone(), b.cdr.clone())
                    };
                    printed += 1;
                    self.princ_inner(&car, out, depth + 1, bq);
                    cur = next;
                }
                Value::Nil => {
                    out.push(')');
                    return;
                }
                other => {
                    out.push_str(" . ");
                    self.princ_inner(&other, out, depth + 1, bq);
                    out.push(')');
                    return;
                }
            }
        }
    }

    fn buffer_name_by_id(&self, id: usize) -> String {
        self.buffer_name(id)
            .unwrap_or_else(|| "*killed buffer*".into())
    }
}

/// Convert a lexical environment chain to a printable alist-of-frames.
fn lex_frame_to_value(i: &Interp, frame: &Rc<crate::lisp::eval::LexFrame>) -> Value {
    // GNU prints the environment as a single alist — the visible
    // bindings innermost-first — or nil when empty.
    let mut pairs: Vec<Value> = Vec::new();
    let mut cur = Some(frame.clone());
    while let Some(f) = cur {
        for (sym_id, val) in f.vars.borrow().iter() {
            pairs.push(Value::cons(Value::Sym(*sym_id), val.clone()));
        }
        cur = f.parent.clone();
    }
    // GNU's environment list is a flat alist innermost-first; the
    // toplevel (empty) environment prints as `(t)'.
    if pairs.is_empty() {
        return Value::list(vec![Value::Sym(crate::lisp::obarray::sym::T)]);
    }
    pairs.sort_by_key(|v| {
        if let Value::Cons(c) = v {
            if let Value::Sym(s) = &c.borrow().car {
                return i.symbol_name(*s).to_string();
            }
        }
        String::new()
    });
    Value::list(pairs)
}

/// C `%.{prec}g` formatting: `prec` significant digits, scientific
/// notation when the decimal exponent is < -4 or >= prec, trailing
/// zeros stripped.
fn format_g(x: f64, prec: usize) -> String {
    // Get `prec` significant digits plus the decimal exponent.
    let sci = format!("{:.*e}", prec - 1, x); // "-d.ddde-12"
    let epos = sci.find('e').unwrap();
    let exp: i32 = sci[epos + 1..].parse().unwrap();
    let mant = &sci[..epos];
    let neg = mant.starts_with('-');
    let digits: String = mant.chars().filter(|c| c.is_ascii_digit()).collect();

    let mut out = String::new();
    if neg {
        out.push('-');
    }
    if exp < -4 || exp >= prec as i32 {
        // Scientific: d.ddde±XX (exponent at least two digits).
        out.push(digits.as_bytes()[0] as char);
        let frac: String = digits[1..].trim_end_matches('0').into();
        if !frac.is_empty() {
            out.push('.');
            out.push_str(&frac);
        }
        out.push('e');
        out.push(if exp < 0 { '-' } else { '+' });
        let _ = write!(out, "{:02}", exp.abs());
    } else if exp >= 0 {
        // Fixed: digits with decimal point inside (exp < prec).
        let ip = exp as usize + 1;
        let (int_part, frac_part) = if digits.len() > ip {
            (&digits[..ip], &digits[ip..])
        } else {
            (digits.as_str(), "")
        };
        out.push_str(int_part);
        for _ in digits.len()..ip {
            out.push('0');
        }
        let frac: String = frac_part.trim_end_matches('0').into();
        if !frac.is_empty() {
            out.push('.');
            out.push_str(&frac);
        }
    } else {
        // 0.000ddd
        out.push_str("0.");
        for _ in 0..(-exp - 1) {
            out.push('0');
        }
        let frac: String = digits.trim_end_matches('0').into();
        out.push_str(&frac);
    }
    out
}

/// Emacs-style float printing (`float_to_string`): shortest of
/// `%.15g`–`%.17g` that reads back to the same double.
/// `1.5`, `1.0`, `1e+20`, `1.0e+INF`, `-1.0e+INF`, `0.0e+NaN`.
pub fn format_float(f: f64) -> String {
    if f.is_nan() {
        return if f.is_sign_negative() { "-0.0e+NaN" } else { "0.0e+NaN" }.into();
    }
    if f.is_infinite() {
        return if f > 0.0 { "1.0e+INF" } else { "-1.0e+INF" }.into();
    }
    if f == 0.0 {
        return if f.is_sign_negative() { "-0.0" } else { "0.0" }.into();
    }
    let mut s = String::new();
    for prec in [15usize, 16, 17] {
        s = format_g(f, prec);
        if s.parse::<f64>().ok() == Some(f) {
            break;
        }
    }
    // A float must always print with a decimal point or exponent.
    if !s.contains('.') && !s.contains('e') {
        s.push_str(".0");
    }
    s
}
