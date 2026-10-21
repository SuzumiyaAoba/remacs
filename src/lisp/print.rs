//! Printing Lisp objects (`prin1`-style readable syntax).

use std::cell::RefCell;
use std::fmt::Write;
use std::rc::Rc;

use super::Interp;
use super::value::Value;

/// Escape a character inside a printed string. Emacs escapes only
/// `"` and `\` — newlines print literally unless `print-escape-newlines`
/// is set; `print-escape-control-characters` prints C0/DEL as `\N'
/// (unpadded octal) and `print-escape-multibyte` hex-escapes non-ASCII
/// as `\xNNNN' (minimum 4 digits).
fn escape_char_for_string(c: char, out: &mut String, nl: bool, mb: bool, cc: bool) {
    match c {
        '"' => out.push_str("\\\""),
        '\\' => out.push_str("\\\\"),
        '\n' if nl => out.push_str("\\n"),
        c if cc && ((c as u32) < 0x20 || c == '\u{7f}') => {
            let _ = write!(out, "\\{:o}", c as u32);
        }
        c if mb && (c as u32) > 0x7f => {
            let _ = write!(out, "\\x{:04x}", super::value::lisp_char_code(c));
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
    /// `print-unreadable-function' support (GNU print.c
    /// `print_unreadable_flag'): every `#<...>'-style emit records its
    /// byte span and object here so the `&mut' caller can post-process
    /// with a Lisp callback.
    static UNREADABLE_HITS: RefCell<Vec<(usize, usize, Value)>> =
        const { RefCell::new(Vec::new()) };
}

/// Per-print state for `print-circle'.
struct CircleCtx {
    /// Objects already seen by the pre-scan.
    seen: rustc_hash::FxHashSet<usize>,
    /// ptr -> label number, assigned at second encounter.
    labels: rustc_hash::FxHashMap<usize, usize>,
    /// Objects whose `#N=' form has already printed.
    printed: rustc_hash::FxHashSet<usize>,
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
        // Lambdas print their `[ARGS BODY ENV ...]' slots — a closure
        // whose env captures a `let' binding referencing the closure
        // itself (gv-ref's setter: `(ref . (getter . SETTER))')
        // recurses forever without the being-printed check.
        Value::Lambda(r) => Some(std::rc::Rc::as_ptr(r) as usize),
        _ => None,
    }
}

/// `print-circle' key for an uninterned symbol.  GNU print.c numbers
/// every non-atom reachable object in `print_preprocess' — including
/// symbols when `print-gensym' is on — so a shared `#:test' prints
/// `#1=#:test' … `#1#'.  SymIds live in a different namespace than
/// heap pointers: fold them into key space by setting the high bit.
fn gensym_circle_key(i: &Interp, v: &Value) -> Option<usize> {
    // Cheap shape check first — runs for every printed symbol.
    let Value::Sym(id) = v else {
        return None;
    };
    if !i.obarray.symbol(*id).uninterned || !i.print_gensym() {
        return None;
    }
    Some(*id as usize | (1usize << (usize::BITS - 1)))
}

/// GNU `print_circle_candidate_p' (print.c): only these object kinds
/// participate in `print-circle' numbering — notably EMPTY strings and
/// EMPTY vectors are excluded, which keeps literal `""'s from
/// sprouting `#N=' labels.
fn print_circle_candidate(i: &Interp, v: &Value) -> bool {
    match v {
        Value::Cons(_) => true,
        Value::Str(s) => !s.borrow().is_empty(),
        Value::Sym(_) => gensym_circle_key(i, v).is_some(),
        Value::Vec(r) => !r.borrow().is_empty(),
        Value::Record(_) | Value::Hash(_) | Value::Lambda(_) => true,
        _ => false,
    }
}

/// GNU's `print_preprocess' pass: depth-first walk of the object
/// graph (car before cdr, like GNU's recursion), numbering each
/// shared/cyclic object when it is encountered a second time.
/// Already-seen objects are not descended into, so cycles terminate.
fn count_print_refs(i: &Interp, v: &Value, ctx: &mut CircleCtx) {
    let mut stack = vec![v.clone()];
    while let Some(x) = stack.pop() {
        // GNU's print_circle_candidate_p gate: empty strings/vectors
        // never participate in print-circle numbering.
        if !print_circle_candidate(i, &x) {
            continue;
        }
        let Some(k) = print_stack_key(&x).or_else(|| gensym_circle_key(i, &x)) else {
            continue;
        };
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
thread_local! {
    /// GNU print.c's `print_number_index': persists across print
    /// calls while `print-continuous-numbering' is non-nil.
    static PRINT_NUMBER_INDEX: std::cell::Cell<i64> =
        const { std::cell::Cell::new(0) };
}

/// `print-number-table' outcome for a heap object under print-circle.
enum NtSubst {
    /// Emit this text and print the object's contents after it
    /// (GNU's negative-index `#N=' prefix).
    Prefix(String),
    /// Emit this text INSTEAD of the object (a `#N#' back-reference
    /// or a verbatim substitution string).
    Replace(String),
}

/// `print-number-table' substitution for a heap object (GNU's
/// `Fgethash' in `print_object' under `print-circle'): a string
/// prints verbatim, a negative index prints `#N=' then flips to its
/// positive, a positive index prints `#N#' and consumes the object.
/// The caller uses Some(_) to suppress the ctx labels too — the
/// table is authoritative for objects it contains.
fn number_table_subst(i: &Interp, v: &Value) -> Option<NtSubst> {
    if !i.print_var("print-circle").truthy() {
        return None;
    }
    // Without `print-continuous-numbering' GNU resets the table at
    // every top-level print — a user-bound table is ignored.
    if !i.print_var("print-continuous-numbering").truthy() {
        return None;
    }
    let nt = i.print_var("print-number-table");
    let Value::Hash(h) = nt else {
        return None;
    };
    let key = crate::lisp::builtins::hashfn::hash_key_for(i, v, crate::lisp::value::HashTest::Eq);
    let found = h.borrow().map.get(&key).cloned()?;
    match &found {
        Value::Str(s) => Some(NtSubst::Replace(s.borrow().clone())),
        Value::Int(n) => {
            let n = *n;
            if n < 0 {
                // First print of a labeled object: emit the prefix
                // and mark it printed (GNU stores the positive).
                h.borrow_mut().map.insert(key, Value::Int(-n));
                Some(NtSubst::Prefix(format!("#{}=", -n)))
            } else {
                Some(NtSubst::Replace(format!("#{n}#")))
            }
        }
        _ => None,
    }
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
        Self::sink_text(&s).into_owned()
    }

    /// Alias for `print_to_string` matching the Lisp name.
    pub fn prin1_to_string(&self, v: &Value) -> String {
        self.print_to_string(v)
    }

    /// `prin1` representation honoring `print-unreadable-function'
    /// (GNU print.c): each `#<...>' span recorded during the print is
    /// replaced by the callback's return value (must be a string), and
    /// a thrown tag/signal propagates as usual Lisp flow.
    pub fn print_to_string_cb(&mut self, v: &Value) -> Result<String, crate::lisp::error::Flow> {
        self.reset_print_numbering();
        UNREADABLE_HITS.with(|u| u.borrow_mut().clear());
        let mut s = String::new();
        let mark = BEING_PRINTED.with(|bp| bp.borrow().len());
        let prev_circle = self.install_circle_ctx(v);
        self.prin1_inner(v, &mut s, 0, false);
        CIRCLE.with(|c| *c.borrow_mut() = prev_circle);
        BEING_PRINTED.with(|bp| bp.borrow_mut().truncate(mark));
        let hits = UNREADABLE_HITS.with(|u| std::mem::take(&mut *u.borrow_mut()));
        if hits.is_empty() {
            return Ok(s);
        }
        let puf = self.intern("print-unreadable-function");
        let f = self.symbol_value(puf);
        if !super::builtins::data::functionp_val(self, &f) {
            return Ok(s);
        }
        // Splice right-to-left so recorded offsets stay valid.
        for (lo, hi, obj) in hits.into_iter().rev() {
            let r = self.call_unreadable_printer(&f, obj, true)?;
            if let Value::Str(rep) = r {
                s.replace_range(lo..hi, &rep.borrow());
            }
        }
        Ok(s)
    }

    /// `princ` variant of `print_to_string_cb'.
    pub fn princ_to_string_cb(&mut self, v: &Value) -> Result<String, crate::lisp::error::Flow> {
        self.reset_print_numbering();
        UNREADABLE_HITS.with(|u| u.borrow_mut().clear());
        let mut s = String::new();
        let mark = BEING_PRINTED.with(|bp| bp.borrow().len());
        let prev_circle = self.install_circle_ctx(v);
        self.princ_inner(v, &mut s, 0, false);
        CIRCLE.with(|c| *c.borrow_mut() = prev_circle);
        BEING_PRINTED.with(|bp| bp.borrow_mut().truncate(mark));
        let hits = UNREADABLE_HITS.with(|u| std::mem::take(&mut *u.borrow_mut()));
        if hits.is_empty() {
            return Ok(s);
        }
        let puf = self.intern("print-unreadable-function");
        let f = self.symbol_value(puf);
        if !super::builtins::data::functionp_val(self, &f) {
            return Ok(s);
        }
        for (lo, hi, obj) in hits.into_iter().rev() {
            let r = self.call_unreadable_printer(&f, obj, false)?;
            if let Value::Str(rep) = r {
                s.replace_range(lo..hi, &rep.borrow());
            }
        }
        Ok(s)
    }

    /// `princ` representation: human-readable (strings unquoted).
    pub fn princ_to_string(&self, v: &Value) -> String {
        let mut s = String::new();
        let mark = BEING_PRINTED.with(|bp| bp.borrow().len());
        let prev_circle = self.install_circle_ctx(v);
        self.princ_inner(v, &mut s, 0, false);
        CIRCLE.with(|c| *c.borrow_mut() = prev_circle);
        BEING_PRINTED.with(|bp| bp.borrow_mut().truncate(mark));
        Self::sink_text(&s).into_owned()
    }

    fn call_unreadable_printer(
        &mut self,
        callback: &Value,
        object: Value,
        escape: bool,
    ) -> Result<Value, crate::lisp::error::Flow> {
        let mark = self.specbind_depth();
        let result = (|| {
            // GNU suppresses recursive use of the same unreadable printer.
            let variable = self.intern("print-unreadable-function");
            self.specbind(variable, Value::Nil)?;
            self.apply(callback, vec![object, Value::from_bool(escape)])
        })();
        self.unbind_to(mark)?;
        result
    }

    /// GNU print's direct C assignment resets numbering without notifying
    /// Lisp variable watchers, including when print-circle is nil.
    fn reset_print_numbering(&mut self) {
        if self.print_var("print-continuous-numbering").truthy()
            && !self.print_var("print-number-table").is_nil()
        {
            return;
        }
        PRINT_NUMBER_INDEX.with(|index| index.set(0));
        let symbol = self.intern("print-number-table");
        if let Some(buffer) = self.current_buffer_ref() {
            if let Some(value) = buffer.borrow_mut().locals.get_mut(&symbol) {
                *value = Value::Nil;
                return;
            }
        }
        self.obarray.symbol_mut(symbol).value = Value::Nil;
    }

    /// When `print-circle' is non-nil, pre-scan V for shared/cyclic
    /// objects and install the per-print state.  Returns the previous
    /// state so callers can restore it (nested print calls).
    fn install_circle_ctx(&self, v: &Value) -> Option<CircleCtx> {
        if !self.print_var("print-circle").truthy() {
            return None;
        }
        let mut ctx = CircleCtx {
            seen: rustc_hash::FxHashSet::default(),
            labels: rustc_hash::FxHashMap::default(),
            printed: rustc_hash::FxHashSet::default(),
            next: 1,
        };
        count_print_refs(self, v, &mut ctx);
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
        let key = print_stack_key(v).or_else(|| gensym_circle_key(self, v));
        if let Some(k) = key {
            if let Some(sub) = number_table_subst(self, v) {
                match sub {
                    NtSubst::Replace(t) => {
                        out.push_str(&t);
                        return;
                    }
                    // `#N=' prefix, then print the object body.
                    NtSubst::Prefix(t) => out.push_str(&t),
                }
            }
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
        // `print-unreadable-function' hook: any `#<...>' fragment this
        // object emits is recorded for the `&mut' caller to post-process.
        let start = out.len();
        self.prin1_inner_obj_raw(v, out, depth, bq);
        let piece = &out[start..];
        if piece.len() > 2 && piece.starts_with("#<") && piece.ends_with('>') {
            UNREADABLE_HITS.with(|u| u.borrow_mut().push((start, out.len(), v.clone())));
        }
    }

    fn prin1_inner_obj_raw(&self, v: &Value, out: &mut String, depth: usize, bq: bool) {
        if depth > 200 {
            out.push_str("##");
            return;
        }
        // print-level: only CONS cells are truncated (GNU print.c
        // checks `print_depth > Vprint_level' solely in the
        // Lisp_Cons branch); vectors, records, hash tables and
        // char-tables always print fully.
        if let Some(level) = self.print_level_limit() {
            if depth >= level {
                if let Value::Cons(_) = v {
                    out.push_str("...");
                    return;
                }
            }
        }
        match v {
            Value::Nil => out.push_str("nil"),
            Value::Int(i) => self.print_integer(*i, out, true),
            Value::BigInt(i) => { let _ = write!(out, "{i}"); },
            Value::Float(f) => {
                let _ = write!(out, "{}", self.format_float_output(**f));
            }
            Value::Sym(id) => {
                if self.obarray.symbol(*id).uninterned && self.print_gensym() {
                    out.push_str("#:");
                }
                let name = self.symbol_name(*id);
                // GNU escapes number-like names, leading question marks,
                // and a leading dot unless followed by an ASCII letter.
                if super::reader::parse_number(&name).is_some()
                    || name.starts_with('?')
                    || (name.starts_with('.')
                        && !name.as_bytes().get(1).is_some_and(u8::is_ascii_alphabetic))
                {
                    out.push('\\');
                }
                push_sym_name(&name, out);
            }
            Value::Str(s) => {
                let nl = self.print_escape_newlines();
                let mb = self.print_escape_multibyte();
                let cc = self.print_var("print-escape-control-characters").truthy();
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
                        "iso-8859-1",
                        "iso-8859-2",
                        "iso-8859-3",
                        "iso-8859-4",
                        "iso-8859-5",
                        "iso-8859-6",
                        "iso-8859-7",
                        "iso-8859-8",
                        "iso-8859-9",
                        "iso-8859-10",
                        "iso-8859-11",
                        "iso-8859-13",
                        "iso-8859-14",
                        "iso-8859-15",
                        "iso-8859-16",
                        "koi8",
                        "koi8-r",
                        "windows-1251",
                        "mac-roman",
                        "big5",
                        "chinese-gb2312",
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
                let na = self.print_var("print-escape-nonascii").truthy();
                if self.is_unibyte_str(s) {
                    // GNU print_object: unibyte byte-chars ≥0x80 print
                    // as `\NNN' only under print-escape-nonascii;
                    // otherwise the eight-bit char itself is emitted
                    // (stdout writes the 0xC0/0x80 overlong pair, a
                    // string sink stores the eight-bit char).
                    for c in s.borrow().chars() {
                        let byte = crate::lisp::value::eight_bit_byte(c)
                            .or_else(|| crate::lisp::value::raw_byte_marker(c))
                            .or_else(|| ((c as u32) >= 0x80).then(|| (c as u32) as u8 & 0xFF));
                        match (byte, na) {
                            (Some(b), true) => {
                                let _ = write!(out, "\\{:03o}", b);
                            }
                            (Some(b), false) => {
                                // Sink decides: stdout → overlong pair,
                                // string/buffer → `\NNN', function →
                                // eight-bit code 0x3FFF80+b.
                                out.push(crate::lisp::value::print_eight_bit_char(b));
                            }
                            (None, _) => escape_char_for_string(c, out, nl, mb, cc),
                        }
                    }
                } else {
                    for c in s.borrow().chars() {
                        // Eight-bit chars print as their byte's octal
                        // escape even in multibyte strings. Other proxy
                        // chars retain their logical code for sinks.
                        if let Some(b) = crate::lisp::value::eight_bit_byte(c) {
                            let _ = write!(out, "\\{:03o}", b);
                        } else {
                            escape_char_for_string(c, out, nl, mb, cc);
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
                self.print_record(items, out, depth, bq, false);
            }
            Value::Hash(h) => {
                // GNU: #s(hash-table) for defaults; `test' appears only
                // when non-eql, `data' only when non-empty.  A named
                // test (define-hash-table-test) prints its symbol.
                let hh = h.borrow();
                let test: Option<String> = if hh.test_name.truthy() {
                    Some(match &hh.test_name {
                        Value::Sym(id) => self.symbol_name(*id).to_string(),
                        other => self.prin1_to_string(other),
                    })
                } else {
                    match hh.test {
                        crate::lisp::value::HashTest::Eq => Some("eq".to_string()),
                        crate::lisp::value::HashTest::Eql => None,
                        crate::lisp::value::HashTest::Equal => {
                            Some("equal".to_string())
                        }
                    }
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
                // GNU nativecomp prints named compiled functions as
                // `#<subr NAME>' — our compiled twins (dumped defs,
                // `#[...]' read from a bundled .elc) carry `name' or,
                // when `.elc' loading left it nameless, a name stamped
                // by `defalias' in `compiled_fn_names'.
                if l.compiled {
                    let nm = self.lambda_name(l);
                    if let Some(n) = nm {
                        let _ = write!(out, "#<subr {}>", n);
                        return;
                    }
                }
                out.push_str("#[");
                if let Some(bc) = &l.bc_items {
                    // Byte-code objects print as their raw slots like
                    // GNU: #[ARGDESC "BYTESTR" [CONSTS] DEPTH ...].
                    let items = bc.borrow();
                    for (i, item) in items.iter().enumerate() {
                        if i > 0 {
                            out.push(' ');
                        }
                        self.prin1_inner(item, out, depth + 1, bq);
                    }
                    out.push(']');
                    return;
                }
                // Emacs 31 prints interpreted functions like
                // #[(x) (x) nil] — arglist, body forms, environment.
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
                    None => {
                        // GNU appends the insertion-type annotation
                        // even for detached markers.
                        if b.insertion_type {
                            out.push_str("#<marker (moves after insertion) in no buffer>");
                        } else {
                            out.push_str("#<marker in no buffer>");
                        }
                    }
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
                let _ = write!(
                    out,
                    "#<frame {} {:#x}>",
                    f.borrow().name.borrow().as_str(),
                    Rc::as_ptr(f) as usize
                );
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
            if let Some(sub) = number_table_subst(self, v) {
                match sub {
                    NtSubst::Replace(t) => {
                        out.push_str(&t);
                        return;
                    }
                    NtSubst::Prefix(t) => out.push_str(&t),
                }
            }
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
        let start = out.len();
        self.princ_inner_obj_raw(v, out, depth, bq);
        let piece = &out[start..];
        if piece.len() > 2 && piece.starts_with("#<") && piece.ends_with('>') {
            UNREADABLE_HITS.with(|u| u.borrow_mut().push((start, out.len(), v.clone())));
        }
    }

    fn princ_inner_obj_raw(&self, v: &Value, out: &mut String, depth: usize, bq: bool) {
        if let Some(level) = self.print_level_limit() {
            if depth >= level {
                if let Value::Cons(_) = v {
                    out.push_str("...");
                    return;
                }
            }
        }
        match v {
            Value::Int(i) => self.print_integer(*i, out, false),
            Value::BigInt(i) => { let _ = write!(out, "{i}"); },
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
                    self.princ_inner(item, out, depth + 1, bq);
                    n += 1;
                }
                if truncated {
                    out.push_str(if n == 0 { "..." } else { " ..." });
                }
                out.push(']');
            }
            // Records print `#s(elts...)' under `princ' too — delegating
            // to `prin1_inner' would re-hit the being-printed check and
            // emit `#N' for the pushed object itself.
            Value::Record(items) => {
                self.print_record(items, out, depth, bq, true);
            }
            // Lambdas/subrs would re-hit the being-printed check in
            // `prin1_inner' (the outer `princ_inner' already pushed
            // this object) and emit `#N' — call the raw printer.
            _ => self.prin1_inner_obj_raw(v, out, depth, bq),
        }
    }

    /// Print a record value. `princ' selects the unescaped inner
    /// printer for record slots (`#s(foo x)' vs `#s(foo "x")').
    fn print_record(
        &self,
        items: &std::rc::Rc<std::cell::RefCell<Vec<Value>>>,
        out: &mut String,
        depth: usize,
        bq: bool,
        princ: bool,
    ) {
        if crate::editor::window_configuration_object(&Value::Record(items.clone())) {
            out.push_str("#<window-configuration>");
            return;
        }
        macro_rules! inner {
            ($v:expr) => {
                if princ {
                    self.princ_inner($v, out, depth + 1, bq)
                } else {
                    self.prin1_inner($v, out, depth + 1, bq)
                }
            };
        }
        {
            let rr = items.borrow();
            // `symbols-with-pos-enabled' sympos: a real symbol in GNU —
            // `princ' prints the bare name; `prin1' prints `#<symbol
            // NAME at POS>' unless `print-symbols-bare' is bound.
            if let Some(Value::Int(t)) = rr.first() {
                if *t == crate::lisp::builtins::misc::SYMPOS_MAGIC {
                    if let [_, Value::Sym(s), Value::Int(p)] = rr.as_slice() {
                        if princ || self.print_var("print-symbols-bare").truthy() {
                            push_sym_name(&self.symbol_name(*s), out);
                        } else {
                            let name = self.symbol_name(*s);
                            let _ = write!(out, "#<symbol {} at {}>", name, p);
                        }
                        return;
                    }
                }
            }
            // Positioned symbols print `#<symbol NAME at POS>'.
            if let Some(Value::Sym(t)) = rr.first() {
                if self.symbol_name(*t) == "symbol-with-pos" {
                    if let [_, Value::Sym(s), Value::Int(p)] = rr.as_slice() {
                        let name = self.symbol_name(*s);
                        let _ = write!(out, "#<symbol {} at {}>", name, p);
                        return;
                    }
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
                    if let [_, Value::Int(n), Value::Str(name)] = rr.as_slice() {
                        let _ = write!(out, "#<terminal {} on {}>", n, name.borrow());
                        return;
                    }
                }
                // Tree-sitter objects print `#<treesit-parser in
                // BUF for LANG>' / `#<treesit-node TYPE in B-E>'.
                if let Some(s) = crate::lisp::builtins::treesit::treesit_repr(self, rr.as_slice()) {
                    out.push_str(&s);
                    return;
                }
                // Sub char tables print `#^^[DEPTH MIN-CHAR
                // SLOTS...]' — GNU's trie nodes.
                if self.symbol_name(*t) == "sub-char-table" && rr.len() >= 3 {
                    out.push_str("#^^[");
                    if let Some(d) = rr.get(1) {
                        inner!(d);
                    }
                    out.push(' ');
                    if let Some(m) = rr.get(2) {
                        inner!(m);
                    }
                    // GNU's `print_stack_push_vector' measures
                    // `print-length' against the pseudovector SIZE
                    // (depth+min_char header + slots) but starts
                    // printing at SUB_CHAR_TABLE_OFFSET: a truncated
                    // sub-table emits `print-length - 1' slots.
                    let limit = self.print_length_limit().map(|l| l.saturating_sub(1));
                    let mut n = 0usize;
                    let mut truncated = false;
                    for s in &rr[3..] {
                        if let Some(l) = limit {
                            if n >= l {
                                truncated = true;
                                break;
                            }
                        }
                        out.push(' ');
                        inner!(s);
                        n += 1;
                    }
                    if truncated {
                        out.push_str(" ...");
                    }
                    out.push(']');
                    return;
                }
                // Char tables print `#^[defalt parent purpose
                // ascii contents[64] extras...]' — GNU's
                // pseudovector layout.
                // Font objects are GNU `font' pseudovectors and print
                // `#<font-spec SLOTS...>' (13 property slots); our
                // record keeps a plist in slot 1 — map the known keys
                // to their font.c slot positions and put the rest in
                // the extras list.
                if self.symbol_name(*t) == "font-spec" {
                    out.push_str("#<font-spec");
                    const SLOT_KEYS: [&str; 12] = [
                        ":type",
                        ":foundry",
                        ":family",
                        ":adstyle",
                        ":registry",
                        ":weight",
                        ":slant",
                        ":width",
                        ":size",
                        ":dpi",
                        ":spacing",
                        ":avgwidth",
                    ];
                    let mut slots: Vec<Option<Value>> = vec![None; 13];
                    let mut extras: Vec<Value> = Vec::new();
                    let mut cur = rr.get(1).cloned().unwrap_or(Value::Nil);
                    while let Value::Cons(c) = cur {
                        let (car, cdr) = {
                            let b = c.borrow();
                            (b.car.clone(), b.cdr.clone())
                        };
                        if let Value::Cons(c2) = cdr {
                            let (val, next) = {
                                let b = c2.borrow();
                                (b.car.clone(), b.cdr.clone())
                            };
                            let key: String = match &car {
                                Value::Sym(s) => self.symbol_name(*s).to_string(),
                                _ => String::new(),
                            };
                            match SLOT_KEYS.iter().position(|k| *k == key) {
                                Some(idx) => slots[idx] = Some(val),
                                None if matches!(car, Value::Sym(_)) => {
                                    extras.push(Value::cons(car, val))
                                }
                                None => {}
                            }
                            cur = next;
                        } else {
                            break;
                        }
                    }
                    for slot in slots.iter_mut().take(12) {
                        out.push(' ');
                        inner!(slot.as_ref().unwrap_or(&Value::Nil));
                    }
                    out.push(' ');
                    inner!(&Value::list(extras));
                    out.push('>');
                    return;
                }
                if self.symbol_name(*t) == "char-table" {
                    if let Some(Value::Vec(slots)) = rr.get(2) {
                        let ptr = std::rc::Rc::as_ptr(items) as usize;
                        let slots = slots.borrow();
                        self.print_char_table(ptr, rr.as_slice(), slots.as_slice(), out, depth, bq);
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
        }
        let rr = items.borrow();
        out.push_str("#s(");
        for (i, item) in rr.iter().enumerate() {
            if i > 0 {
                out.push(' ');
            }
            inner!(item);
        }
        out.push(')');
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
        // defalt/parent (side-tables keyed on the Record pointer;
        // Weak guards against address reuse by a different live
        // char-table)
        let defalt = self
            .char_table_defalts
            .iter()
            .find(|(k, w, _)| *k == ptr && w.upgrade().is_some())
            .map(|(_, _, v)| v.clone())
            .unwrap_or(Value::Nil);
        let parent = self
            .char_table_parents
            .iter()
            .find(|(k, w, _)| *k == ptr && w.upgrade().is_some())
            .map(|(_, _, v)| v.clone())
            .unwrap_or(Value::Nil);
        let purpose = rr.get(1).cloned().unwrap_or(Value::Nil);
        // GNU `print_stack_push_vector' applies `print-length' to the
        // whole pseudovector: defalt, parent, purpose, ascii cache,
        // contents[64] and extras all count toward the limit.
        let limit = self.print_length_limit();
        let mut n = 0usize;
        let mut truncated = false;
        for s in [defalt, parent, purpose]
            .iter()
            .chain(slots.iter())
            .chain(rr[3..].iter())
        {
            if let Some(l) = limit {
                if n >= l {
                    truncated = true;
                    break;
                }
            }
            if n > 0 {
                out.push(' ');
            }
            self.prin1_inner(s, out, depth + 1, bq);
            n += 1;
        }
        if truncated {
            out.push_str(" ...");
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

    /// Print a number using the dynamically bound GNU output descriptor.
    pub(crate) fn format_float_output(&self, value: f64) -> String {
        let format = self.print_var("float-output-format");
        if let Value::Str(s) = format {
            if let Some(output) = format_float_descriptor(value, &s.borrow()) {
                return output;
            }
        }
        format_float(value)
    }

    /// GNU print.c: only independent graphic characters and the six
    /// common named escapes use character notation.  `princ' omits the
    /// extra quoting for delimiter characters, but keeps named escapes.
    fn print_integer(&self, n: i128, out: &mut String, escape: bool) {
        if self.print_var("print-integers-as-characters").truthy() && (0..=0x10ffff).contains(&n) {
            let named = match n {
                8 => Some('b'),
                9 => Some('t'),
                10 => Some('n'),
                12 => Some('f'),
                13 => Some('r'),
                32 => Some('s'),
                _ => None,
            };
            let table = self.print_var("unicode-category-table");
            let category = super::builtins::misc::char_table_ref(self, &table, n as usize);
            let graphic = matches!(category, Value::Int(cat)
                if (super::value::FIXNUM_MIN..=super::value::FIXNUM_MAX).contains(&cat)
                    && !matches!(cat, 6..=8 | 23..=28 | 30));
            if let Some(named) = named {
                out.push('?');
                out.push('\\');
                out.push(named);
                return;
            }
            if graphic {
                if let Some(c) = char::from_u32(n as u32) {
                    out.push('?');
                    if escape
                        && matches!(
                            c,
                            ';' | '"' | '\'' | '\\' | '(' | ')' | '{' | '}' | '[' | ']'
                        )
                    {
                        out.push('\\');
                    }
                    out.push(c);
                    return;
                }
            }
        }
        let _ = write!(out, "{n}");
    }

    /// `print-length` (default nil): max elements printed per sequence.
    fn print_length_limit(&self) -> Option<usize> {
        self.print_var("print-length")
            .int()
            .filter(|n| (0..=super::value::FIXNUM_MAX).contains(n))
            .map(|n| n as usize)
    }

    /// `print-level` (default nil): max nesting depth before "...".
    fn print_level_limit(&self) -> Option<usize> {
        self.print_var("print-level")
            .int()
            .filter(|n| (super::value::FIXNUM_MIN..=super::value::FIXNUM_MAX).contains(n))
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
                    if circle_active() && print_stack_key(&cur).map(circle_counted).unwrap_or(false)
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
                    if circle_active() && print_stack_key(&cur).map(circle_counted).unwrap_or(false)
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

/// Return the canonical, shared interpreter-environment alist.
fn lex_frame_to_value(_i: &Interp, frame: &Rc<crate::lisp::eval::LexFrame>) -> Value {
    crate::lisp::eval::lexenv_as_value(&Some(frame.clone()))
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

/// GNU accepts only %.PRECISION followed by e, f or g, at most DBL_DIG
/// digits of precision.  An absent precision is accepted too, as in C printf.
fn format_float_descriptor(value: f64, descriptor: &str) -> Option<String> {
    if !value.is_finite() {
        return None;
    }
    // GNU validates the C string, so text after a NUL is not part of the spec.
    let spec = descriptor.split('\0').next()?.as_bytes();
    if !spec.starts_with(b"%.") {
        return None;
    }
    let mut pos = 2;
    let mut precision = None;
    while let Some(digit @ b'0'..=b'9') = spec.get(pos) {
        let n = precision.unwrap_or(0) * 10 + usize::from(*digit - b'0');
        if n > 15 {
            return None;
        }
        precision = Some(n);
        pos += 1;
    }
    let kind = *spec.get(pos)?;
    if pos + 1 != spec.len() || !matches!(kind, b'e' | b'f' | b'g') {
        return None;
    }
    if precision == Some(0) && kind != b'f' {
        return None;
    }
    let digits = precision.unwrap_or(0);
    let mut output = match kind {
        b'f' => format!("{:.*}", digits, value),
        b'e' => {
            let scientific = format!("{:.*e}", digits, value);
            let (mantissa, exponent) = scientific.split_once('e')?;
            let exponent: i32 = exponent.parse().ok()?;
            format!(
                "{}e{}{:02}",
                mantissa,
                if exponent < 0 { '-' } else { '+' },
                exponent.abs()
            )
        }
        b'g' => format_g(value, digits.max(1)),
        _ => unreachable!(),
    };
    // Only an explicitly zero precision with f may print an integer token.
    if precision != Some(0) && !output.contains('.') && !output.contains('e') {
        output.push_str(".0");
    }
    Some(output)
}

/// Emacs-style float printing (`float_to_string`): shortest of
/// `%.15g`–`%.17g` that reads back to the same double.  Subnormals
/// start at one significant digit, as in gnulib's dtoastr.
/// `1.5`, `1.0`, `1e+20`, `1.0e+INF`, `-1.0e+INF`, `0.0e+NaN`.
/// NaNs print the numeric mantissa GNU derives from the NaN payload
/// (`float_error' notation): `get_nan_payload' strips the quiet bit,
/// and the payload value truncates to the printed integer — literals
/// like `3.5e+NaN' read back as payload 3 and print `3.0e+NaN'.
pub fn format_float(f: f64) -> String {
    if f.is_nan() {
        let payload = f.to_bits() & ((1u64 << 51) - 1);
        let sign = if f.is_sign_negative() { "-" } else { "" };
        // f64 prints payloads ≥2^53 exactly enough for the integral
        // mantissa GNU emits.
        return format!("{}{}.0e+NaN", sign, payload);
    }
    if f.is_infinite() {
        return if f > 0.0 { "1.0e+INF" } else { "-1.0e+INF" }.into();
    }
    if f == 0.0 {
        return if f.is_sign_negative() { "-0.0" } else { "0.0" }.into();
    }
    let mut s = String::new();
    let first_precision = if f.is_subnormal() { 1 } else { 15 };
    for prec in first_precision..=17 {
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

/// A user-supplied print-number-table can use a custom test. Route its
/// writes through puthash so callbacks and their mutation guard apply.
fn preprocess_put(
    i: &mut Interp,
    h: &crate::lisp::value::HashRef,
    key: crate::lisp::value::HashKey,
    original: &Value,
    value: Value,
) -> Result<(), crate::lisp::error::Flow> {
    if h.borrow().user_test.is_some() {
        crate::lisp::builtins::hashfn::f_puthash(
            i,
            vec![original.clone(), value, Value::Hash(h.clone())].into(),
        )?;
    } else {
        h.borrow_mut().map.insert(key, value);
    }
    Ok(())
}

/// GNU print.c `Fprint_preprocess': when `print-circle' is non-nil,
/// fill `print-number-table' — each `print-circle-candidate' gets t
/// at its first encounter and `-N' (bumping `print_number_index') at
/// the second; the `t' singles are left in (the top-level `print'
/// removes them for its own pass).  No-op when print-circle is nil.
pub fn f_print_preprocess(i: &mut Interp, a: crate::lisp::Argv) -> crate::lisp::error::EvalResult {
    if !i.print_var("print-circle").truthy() {
        return Ok(Value::Nil);
    }
    PRINT_NUMBER_INDEX.with(|c| c.set(0));
    let nt_id = i.intern("print-number-table");
    let mut nt = i.symbol_value(nt_id);
    if !matches!(nt, Value::Hash(_)) {
        nt = Value::Hash(crate::lisp::memory::allocate(
            crate::lisp::value::LispHash::new(crate::lisp::value::HashTest::Eq),
            crate::lisp::Value::Hash,
        ))
        .tracked();
        let _ = i.set_symbol(nt_id, nt.clone());
    }
    let Value::Hash(h) = nt else {
        return Ok(Value::Nil);
    };
    let mut stack = vec![a[0].clone()];
    while let Some(x) = stack.pop() {
        if !print_circle_candidate(i, &x) {
            continue;
        }
        let custom = h.borrow().user_test.is_some();
        let key = if custom {
            crate::lisp::builtins::hashfn::table_key(i, &h, &x)?
        } else {
            crate::lisp::builtins::hashfn::hash_key_for(i, &x, crate::lisp::value::HashTest::Eq)
        };
        let cur = h.borrow().map.get(&key).cloned();
        match cur {
            Some(Value::Sym(s)) if s == crate::lisp::sym::T => {
                let n = PRINT_NUMBER_INDEX.with(|c| {
                    c.set(c.get() + 1);
                    c.get()
                });
                preprocess_put(i, &h, key, &x, Value::Int(-(n as i128)))?;
            }
            // Under `print-continuous-numbering' an uninterned symbol
            // always gets a number (GNU's byte-compile-output-docform
            // special).
            None => {
                let gensym = gensym_circle_key(i, &x).is_some();
                if gensym && i.print_var("print-continuous-numbering").truthy() {
                    let n = PRINT_NUMBER_INDEX.with(|c| {
                        c.set(c.get() + 1);
                        c.get()
                    });
                    preprocess_put(i, &h, key, &x, Value::Int(-(n as i128)))?;
                } else {
                    preprocess_put(i, &h, key, &x, Value::Sym(crate::lisp::sym::T))?;
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
                    Value::Hash(hh) => {
                        let h2 = hh.borrow();
                        for (_, orig) in h2.keys.iter() {
                            stack.push(orig.clone());
                        }
                        stack.extend(h2.map.values().cloned());
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }
    Ok(Value::Nil)
}
