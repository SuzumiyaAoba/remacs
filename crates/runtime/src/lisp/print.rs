//! Printing Lisp objects (`prin1`-style readable syntax).

use std::fmt::Write;
use std::rc::Rc;

use super::Interp;
use super::print_text::PrintText;
use super::text::{EmacsChar, EmacsText, TextSource};
use super::value::{StrRef, Value};

mod circle;
mod lists;
mod numbers;
mod objects;
mod render;
pub use circle::f_print_preprocess;
pub use numbers::format_float;
use render::PrintMode;

/// Escape a character inside a printed string. Emacs escapes only
/// `"` and `\` — newlines print literally unless `print-escape-newlines`
/// is set; `print-escape-control-characters` prints C0/DEL as `\N'
/// (unpadded octal) and `print-escape-multibyte` hex-escapes non-ASCII
/// as `\xNNNN' (minimum 4 digits).
fn escape_char_for_string(c: EmacsChar, out: &mut PrintText, nl: bool, mb: bool, cc: bool) {
    match c.unicode() {
        Some('"') => out.push_str("\\\""),
        Some('\\') => out.push_str("\\\\"),
        Some('\n') if nl => out.push_str("\\n"),
        _ if cc && (c.code() < 0x20 || c == '\u{7f}') => {
            let _ = write!(out, "\\{:o}", c.code());
        }
        _ if mb && c.code() > 0x7f => {
            let _ = write!(out, "\\x{:04x}", c.code());
        }
        _ => out.push(c),
    }
}

fn sym_char_needs_escape(c: EmacsChar) -> bool {
    c.unicode().is_some_and(|c| {
        matches!(
            c,
            '(' | ')' | '[' | ']' | '\'' | '"' | '`' | ',' | ';' | '#' | '\\'
        )
    }) || c.is_whitespace()
        || c.code() < 0x20
}

fn push_sym_name<T: TextSource + ?Sized>(name: &T, out: &mut PrintText) {
    name.with_text(|name| {
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
    });
}

/// Text-property intervals that `prin1' will include for STRING.
/// `print-circle' preprocessing and rendering must walk the same object
/// graph, or references hidden from the printed text can steal labels.
pub(super) fn printable_string_props(
    i: &Interp,
    string: &StrRef,
) -> Vec<(usize, usize, Vec<Value>)> {
    let pct = i.print_var("print-charset-text-property");
    let charset_id = i.q_soft("charset");
    // nil → hide all charset props; `default' → only map-based charsets;
    // anything else → show all.
    let show_all = charset_id.is_none()
        || (!pct.is_nil() && !matches!(pct, Value::Sym(id) if i.symbol_name(id) == "default"));
    let visible = |plist: &Vec<Value>| -> Vec<Value> {
        if show_all {
            return plist.clone();
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
            "windows-1252",
            "mac-roman",
            "big5",
            "chinese-gb2312",
        ];
        let cid = charset_id.unwrap();
        let has_non_ascii = string.borrow().chars().any(|c| c.code() >= 0x80);
        let mut result = Vec::new();
        let mut k = 0;
        while k + 1 < plist.len() {
            let hide = if i.sym_id(&plist[k]) == Some(cid) {
                if hide_all {
                    true
                } else {
                    let name = match &plist[k + 1] {
                        Value::Sym(id) => i.symbol_name(*id),
                        _ => String::new(),
                    };
                    !has_non_ascii || !MAP_CHARSETS.contains(&name.as_str())
                }
            } else {
                false
            };
            if !hide {
                result.push(plist[k].clone());
                result.push(plist[k + 1].clone());
            }
            k += 2;
        }
        result
    };

    i.str_props(string)
        .iter()
        .map(|(start, end, plist)| (*start, *end, visible(plist)))
        .filter(|(_, _, plist)| !plist.is_empty())
        .collect()
}

impl Interp {
    fn prin1_inner_obj_raw(&self, v: &Value, out: &mut PrintText, depth: usize, bq: bool) {
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
            Value::Int(i) => self.print_integer(*i, out, PrintMode::Readable),
            Value::BigInt(i) => {
                let _ = write!(out, "{i}");
            }
            Value::Float(f) => {
                let _ = write!(out, "{}", self.format_float_output(**f));
            }
            Value::Sym(id) => {
                let gensym_prefix = self.obarray.symbol(*id).uninterned && self.print_gensym();
                if gensym_prefix {
                    out.push_str("#:");
                }
                let name = self.print_symbol_name(*id);
                if name.is_empty() && self.obarray.symbol(*id).uninterned && self.print_gensym() {
                    return;
                }
                // GNU escapes number-like names, leading question marks,
                // and a leading dot unless followed by an ASCII letter.
                if (name.is_ascii()
                    && super::reader::parse_number(&name.to_unicode_lossy()).is_some())
                    || name.starts_with('?')
                    || (name.starts_with('.')
                        && !name.chars().nth(1).is_some_and(|c| c.is_ascii_alphabetic()))
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
                // carries non-empty text-property intervals.
                let printed = printable_string_props(self, s);
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
                            .or_else(|| (c.code() >= 0x80).then(|| c.code() as u8));
                        match (byte, na) {
                            (Some(b), true) => {
                                let _ = write!(out, "\\{:03o}", b);
                            }
                            (Some(b), false) => {
                                // Sink decides: stdout → overlong pair,
                                // string/buffer → `\NNN', function →
                                // eight-bit code 0x3FFF80+b.
                                out.push_printed_byte(b);
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
                self.print_record(items, out, depth, bq, PrintMode::Readable);
            }
            Value::Hash(h) => {
                // GNU: #s(hash-table) for defaults; `test' appears only
                // when non-eql, `data' only when non-empty.  A named
                // test (define-hash-table-test) prints its symbol.
                let hh = h.borrow();
                let test: Option<EmacsText> = if hh.test_name.truthy() {
                    Some(match &hh.test_name {
                        Value::Sym(id) => self.print_symbol_name(*id),
                        other => self.prin1_to_string(other),
                    })
                } else {
                    match hh.test {
                        crate::lisp::value::HashTest::Eq => Some(EmacsText::from("eq")),
                        crate::lisp::value::HashTest::Eql => None,
                        crate::lisp::value::HashTest::Equal => Some(EmacsText::from("equal")),
                    }
                };
                let empty = hh.keys.is_empty();
                out.push_str("#s(hash-table");
                if let Some(t) = test {
                    out.push_str(" test ");
                    out.push_str(&t);
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
                    out.push_str("#<buffer ");
                    out.push_str(&bb.name);
                    out.push('>');
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
                            "#<marker {}at {} in ",
                            if b.insertion_type {
                                "(moves after insertion) "
                            } else {
                                ""
                            },
                            b.position + 1
                        );
                        out.push_str(&self.buffer_name_by_id(buf));
                        out.push('>');
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
                let _ = write!(out, "#<window {} on ", b.id);
                out.push_str(&self.buffer_name_by_id(b.buffer));
                out.push('>');
            }
            Value::Frame(f) => {
                out.push_str("#<frame ");
                out.push_str(&f.borrow().name.borrow());
                let _ = write!(out, " {:#x}>", Rc::as_ptr(f) as usize);
            }
            Value::Process(p) => {
                out.push_str("#<process ");
                out.push_str(&p.borrow().name);
                out.push('>');
            }
            // GNU prints the machine address; any unique pointer works.
            Value::Thread(t) => {
                let _ = write!(out, "#<thread 0x{:x}>", Rc::as_ptr(t) as usize);
            }
            Value::Mutex(m) => {
                let mm = m.borrow();
                match &mm.name {
                    Some(n) => {
                        out.push_str("#<mutex ");
                        out.push_str(n);
                        out.push('>');
                    }
                    None => out.push_str("#<mutex>"),
                }
            }
            Value::CondVar(c) => {
                let cc = c.borrow();
                match &cc.name {
                    Some(n) => {
                        out.push_str("#<condvar ");
                        out.push_str(n);
                        out.push('>');
                    }
                    None => out.push_str("#<condvar>"),
                }
            }
            Value::Finalizer(_) => out.push_str("#<finalizer>"),
        }
    }

    fn princ_inner_obj_raw(&self, v: &Value, out: &mut PrintText, depth: usize, bq: bool) {
        if let Some(level) = self.print_level_limit() {
            if depth >= level {
                if let Value::Cons(_) = v {
                    out.push_str("...");
                    return;
                }
            }
        }
        match v {
            Value::Int(i) => self.print_integer(*i, out, PrintMode::Display),
            Value::BigInt(i) => {
                let _ = write!(out, "{i}");
            }
            Value::Str(s) => out.push_str(&s.borrow()),
            // princ prints symbol names raw — no backslash escapes.
            Value::Sym(id) => {
                let prefixed = self.obarray.symbol(*id).uninterned && self.print_gensym();
                if prefixed {
                    out.push_str("#:");
                }
                let name = self.print_symbol_name(*id);
                if name.is_empty() && !prefixed {
                    out.push_str("##");
                } else {
                    out.push_str(&name);
                }
            }
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
                self.print_record(items, out, depth, bq, PrintMode::Display);
            }
            // Lambdas/subrs would re-hit the being-printed check in
            // `prin1_inner' (the outer `princ_inner' already pushed
            // this object) and emit `#N' — call the raw printer.
            _ => self.prin1_inner_obj_raw(v, out, depth, bq),
        }
    }

    /// `print-gensym` variable (default nil): print uninterned
    /// symbols with `#:' only when non-nil.
    fn print_gensym(&self) -> bool {
        self.q_soft("print-gensym")
            .filter(|id| self.bound_p(*id))
            .map(|id| self.symbol_value(id).truthy())
            .unwrap_or(false)
    }

    /// `print-quoted` variable (default t): abbreviate
    /// (quote x) -> 'x, (function x) -> #'x, (\` x) -> `x, etc.
    fn print_quoted(&self) -> bool {
        self.q_soft("print-quoted")
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

    fn print_symbol_name(&self, id: super::value::SymId) -> EmacsText {
        self.symbol_name_text(id)
    }

    fn buffer_name_by_id(&self, id: usize) -> EmacsText {
        self.buffers
            .get(id)
            .map(|b| b.borrow().name.clone())
            .unwrap_or_else(|| "*killed buffer*".into())
    }
}

/// Return the canonical, shared interpreter-environment alist.
fn lex_frame_to_value(_i: &Interp, frame: &Rc<crate::lisp::eval::LexFrame>) -> Value {
    crate::lisp::eval::lexenv_as_value(&Some(frame.clone()))
}
