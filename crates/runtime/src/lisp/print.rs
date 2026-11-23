//! Printing Lisp objects (`prin1`-style readable syntax).

use std::rc::Rc;

use super::error::Flow;
use super::Interp;
use super::print_text::PrintText;
use super::text::{EmacsChar, EmacsText};
use super::value::{StrRef, Value};

mod circle;
mod lists;
mod numbers;
mod objects;
mod render;
pub use circle::f_print_preprocess;
pub use numbers::format_float;
use render::{PrintMode, Printer};

/// Escape a character inside a printed string. Emacs escapes only
/// `"` and `\` — newlines print literally unless `print-escape-newlines`
/// is set; `print-escape-control-characters` prints C0/DEL as `\N'
/// (unpadded octal) and `print-escape-multibyte` hex-escapes non-ASCII
/// as `\xNNNN' (minimum 4 digits).
fn escape_char_for_string(
    printer: &mut Printer<'_>,
    c: EmacsChar,
    out: &mut PrintText,
    nl: bool,
    mb: bool,
    cc: bool,
) -> Result<(), crate::lisp::error::Flow> {
    match c.unicode() {
        Some('"') => printer.push_text(out, "\\\"")?,
        Some('\\') => printer.push_text(out, "\\\\")?,
        Some('\n') if nl => printer.push_text(out, "\\n")?,
        _ if cc && (c.code() < 0x20 || c == '\u{7f}') => {
            printer.push_format(out, format_args!("\\{:o}", c.code()))?;
        }
        _ if mb && c.code() > 0x7f => {
            printer.push_format(out, format_args!("\\x{:04x}", c.code()))?;
        }
        _ => printer.push_char(out, c)?,
    }
    Ok(())
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

fn push_sym_name(
    printer: &mut Printer<'_>,
    name: &EmacsText,
    out: &mut PrintText,
) -> Result<(), crate::lisp::error::Flow> {
    if name.is_empty() {
        return printer.push_text(out, "##");
    }
    for c in name.chars() {
        if sym_char_needs_escape(c) {
            printer.push_char(out, '\\')?;
        }
        printer.push_char(out, c)?;
    }
    Ok(())
}

/// Text-property intervals that `prin1' will include for STRING.
/// `print-circle' preprocessing and rendering must walk the same object
/// graph, or references hidden from the printed text can steal labels.
pub(super) fn printable_string_props(
    i: &Interp,
    string: &StrRef,
) -> Result<Vec<(usize, usize, Vec<Value>)>, Flow> {
    let pct = i.print_var("print-charset-text-property");
    let charset_id = i.q_soft("charset");
    // nil → hide all charset props; `default' → only map-based charsets;
    // anything else → show all.
    let show_all = charset_id.is_none()
        || (!pct.is_nil() && !matches!(pct, Value::Sym(id) if i.symbol_name(id) == "default"));
    let has_non_ascii = string
        .try_borrow()
        .map(|string| string.chars().any(|c| c.code() >= 0x80))
        .map_err(|_| i.error("Cannot print a mutably borrowed string"))?;
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

    Ok(i.str_props(string)
        .iter()
        .map(|(start, end, plist)| (*start, *end, visible(plist)))
        .filter(|(_, _, plist)| !plist.is_empty())
        .collect())
}

impl Printer<'_> {
    pub(super) fn push_string_chars(
        &mut self,
        out: &mut PrintText,
        string: &StrRef,
    ) -> Result<(), crate::lisp::error::Flow> {
        let len = string
            .try_borrow()
            .map(|string| string.len())
            .map_err(|_| self.interp().error("Cannot print a mutably borrowed string"))?;
        for index in 0..len {
            let character = {
                let string = string
                    .try_borrow()
                    .map_err(|_| self.interp().error("Cannot print a mutably borrowed string"))?;
                string.get(index)
            };
            let Some(character) = character else {
                break;
            };
            self.push_char(out, character)?;
        }
        Ok(())
    }

    fn prin1_inner_obj_raw(
        &mut self,
        v: &Value,
        out: &mut PrintText,
        depth: usize,
        bq: bool,
    ) -> Result<(), crate::lisp::error::Flow> {
        // print-level: only CONS cells are truncated (GNU print.c
        // checks `print_depth > Vprint_level' solely in the
        // Lisp_Cons branch); vectors, records, hash tables and
        // char-tables always print fully.
        if let Some(level) = self.print_level_limit() {
            if depth >= level {
                if let Value::Cons(_) = v {
                    self.push_text(out, "...")?;
                    return Ok(());
                }
            }
        }
        match v {
            Value::Nil => self.push_text(out, "nil")?,
            Value::Int(i) => self.print_integer(*i, out, PrintMode::Readable)?,
            Value::BigInt(i) => {
                self.push_format(out, format_args!("{i}"))?;
            }
            Value::Float(f) => {
                self.push_format(out, format_args!("{}", self.format_float_output(**f)))?;
            }
            Value::Sym(id) => {
                let gensym_prefix = self.obarray.symbol(*id).uninterned && self.print_gensym();
                if gensym_prefix {
                    self.push_text(out, "#:")?;
                }
                let name = self.print_symbol_name(*id);
                if name.is_empty() && self.obarray.symbol(*id).uninterned && self.print_gensym() {
                    return Ok(());
                }
                // GNU escapes number-like names, leading question marks,
                // and a leading dot unless followed by an ASCII letter.
                if (name.is_ascii()
                    && super::reader::parse_number(&name.to_unicode_lossy()).is_some())
                    || name.starts_with('?')
                    || (name.starts_with('.')
                        && !name.chars().nth(1).is_some_and(|c| c.is_ascii_alphabetic()))
                {
                    self.push_char(out, '\\')?;
                }
                push_sym_name(self, &name, out)?;
            }
            Value::Str(s) => {
                let nl = self.print_escape_newlines();
                let mb = self.print_escape_multibyte();
                let cc = self.print_var("print-escape-control-characters").truthy();
                // GNU prints #("..." s e (plist) ...) when the string
                // carries non-empty text-property intervals.
                let printed = printable_string_props(self.interp(), s)?;
                if !printed.is_empty() {
                    self.push_text(out, "#(")?;
                }
                self.push_char(out, '"')?;
                let na = self.print_var("print-escape-nonascii").truthy();
                let string_len = s
                    .try_borrow()
                    .map(|string| string.len())
                    .map_err(|_| self.interp().error("Cannot print a mutably borrowed string"))?;
                if self.is_unibyte_str(s) {
                    // GNU print_object: unibyte byte-chars ≥0x80 print
                    // as `\NNN' only under print-escape-nonascii;
                    // otherwise the eight-bit char itself is emitted
                    // (stdout writes the 0xC0/0x80 overlong pair, a
                    // string sink stores the eight-bit char).
                    for index in 0..string_len {
                        let character = {
                            let string = s.try_borrow().map_err(|_| {
                                self.interp()
                                    .error("Cannot print a mutably borrowed string")
                            })?;
                            string.get(index)
                        };
                        let Some(c) = character else {
                            break;
                        };
                        let byte = crate::lisp::value::eight_bit_byte(c)
                            .or_else(|| (c.code() >= 0x80).then(|| c.code() as u8));
                        match (byte, na) {
                            (Some(b), true) => {
                                self.push_format(out, format_args!("\\{:03o}", b))?;
                            }
                            (Some(b), false) => {
                                // Sink decides: stdout → overlong pair,
                                // string/buffer → `\NNN', function →
                                // eight-bit code 0x3FFF80+b.
                                self.push_printed_byte(out, b)?;
                            }
                            (None, _) => escape_char_for_string(self, c, out, nl, mb, cc)?,
                        }
                    }
                } else {
                    for index in 0..string_len {
                        let character = {
                            let string = s.try_borrow().map_err(|_| {
                                self.interp()
                                    .error("Cannot print a mutably borrowed string")
                            })?;
                            string.get(index)
                        };
                        let Some(c) = character else {
                            break;
                        };
                        // Eight-bit chars print as their byte's octal
                        // escape even in multibyte strings. Other proxy
                        // chars retain their logical code for sinks.
                        if let Some(b) = crate::lisp::value::eight_bit_byte(c) {
                            self.push_format(out, format_args!("\\{:03o}", b))?;
                        } else {
                            escape_char_for_string(self, c, out, nl, mb, cc)?;
                        }
                    }
                }
                self.push_char(out, '"')?;
                let wrapped = !printed.is_empty();
                for (a, b, pl) in printed {
                    self.push_format(out, format_args!(" {a} {b} "))?;
                    self.prin1_inner(&Value::list(pl), out, depth + 1, bq)?;
                }
                if wrapped {
                    self.push_char(out, ')')?;
                }
            }
            Value::Cons(_) => self.print_list(v, out, depth, bq)?,
            Value::Vec(items) => {
                // `[overlay BID IDX]' handles print like GNU overlays.
                if let Some(r) = self.overlay_repr(items) {
                    self.push_text(out, &r)?;
                    return Ok(());
                }
                self.push_char(out, '[')?;
                let limit = self.print_length_limit();
                let mut n = 0usize;
                let mut truncated = false;
                let len = items
                    .try_borrow()
                    .map(|items| items.len())
                    .map_err(|_| self.interp().error("Cannot print a mutably borrowed vector"))?;
                while n < len {
                    if let Some(l) = limit {
                        if n >= l {
                            truncated = true;
                            break;
                        }
                    }
                    // Do not hold a vector borrow while printing a slot:
                    // an unreadable callback can mutate later slots.
                    if n > 0 {
                        self.push_char(out, ' ')?;
                    }
                    let item = items
                        .try_borrow()
                        .map(|items| items.get(n).cloned())
                        .map_err(|_| {
                            self.interp().error("Cannot print a mutably borrowed vector")
                        })?;
                    let Some(item) = item else {
                        break;
                    };
                    self.prin1_inner(&item, out, depth + 1, bq)?;
                    n += 1;
                }
                if truncated {
                    self.push_text(out, if n == 0 { "..." } else { " ..." })?;
                }
                self.push_char(out, ']')?;
            }
            Value::Record(items) => {
                self.print_record(items, out, depth, bq, PrintMode::Readable)?;
            }
            Value::Hash(h) => {
                // GNU: #s(hash-table) for defaults; `test' appears only
                // when non-eql, `data' only when non-empty.  A named
                // test (define-hash-table-test) prints its symbol.
                let (test_name, test_kind, weakness) = {
                    let hh = h.try_borrow().map_err(|_| {
                        self.interp()
                            .error("Cannot print a mutably borrowed hash table")
                    })?;
                    (hh.test_name.clone(), hh.test, hh.weakness.clone())
                };
                let test: Option<EmacsText> = if test_name.truthy() {
                    Some(match &test_name {
                        Value::Sym(id) => self.print_symbol_name(*id),
                        other => self.prin1_to_string(other),
                    })
                } else {
                    match test_kind {
                        crate::lisp::value::HashTest::Eq => Some(EmacsText::from("eq")),
                        crate::lisp::value::HashTest::Eql => None,
                        crate::lisp::value::HashTest::Equal => Some(EmacsText::from("equal")),
                    }
                };
                self.push_text(out, "#s(hash-table")?;
                if let Some(t) = test {
                    self.push_text(out, " test ")?;
                    self.push_text(out, &t)?;
                }
                if let Some(w) = &weakness {
                    self.push_text(out, " weakness ")?;
                    self.prin1_inner(w, out, depth + 1, bq)?;
                }
                // GNU reads the entry count only after printing the table
                // header, since a function stream can mutate the table there.
                let entry_count = h
                    .try_borrow()
                    .map(|table| table.keys.len())
                    .map_err(|_| {
                        self.interp()
                            .error("Cannot print a mutably borrowed hash table")
                    })?;
                if entry_count > 0 {
                    self.push_text(out, " data (")?;
                    let limit = self.print_length_limit();
                    let printed = limit.map_or(entry_count, |limit| limit.min(entry_count));
                    let slot_count = h
                        .try_borrow()
                        .map(|table| table.keys.0.len())
                        .map_err(|_| {
                            self.interp()
                                .error("Cannot print a mutably borrowed hash table")
                        })?;
                    let mut printed_pairs = 0usize;
                    let mut slot = 0usize;
                    while printed_pairs < printed && slot < slot_count {
                        let entry = h
                            .try_borrow()
                            .map(|table| {
                                table
                                    .keys
                                    .0
                                    .get(slot)
                                    .and_then(Option::as_ref)
                                    .cloned()
                            })
                            .map_err(|_| {
                                self.interp()
                                    .error("Cannot print a mutably borrowed hash table")
                            })?;
                        slot += 1;
                        let Some((hk, k)) = entry else {
                            continue;
                        };
                        if printed_pairs > 0 {
                            self.push_char(out, ' ')?;
                        }
                        self.prin1_inner(&k, out, depth + 1, bq)?;
                        self.push_char(out, ' ')?;
                        // A recursive key printer may update or remove this
                        // entry before its value is reached.
                        let v = h
                            .try_borrow()
                            .map(|table| table.map.get(&hk).cloned().unwrap_or(Value::Nil))
                            .map_err(|_| {
                                self.interp()
                                    .error("Cannot print a mutably borrowed hash table")
                            })?;
                        self.prin1_inner(&v, out, depth + 1, bq)?;
                        printed_pairs += 1;
                    }
                    if printed < entry_count {
                        if printed_pairs > 0 {
                            self.push_char(out, ' ')?;
                        }
                        self.push_text(out, "...")?;
                    }
                    self.push_char(out, ')')?;
                }
                self.push_char(out, ')')?;
            }
            Value::Subr(s) => {
                self.push_format(out, format_args!("#<subr {}>", s.name))?;
            }
            Value::Lambda(l) => {
                // GNU nativecomp prints named native-compiled functions
                // as `#<subr NAME>'. Byte-code objects retain their
                // readable `#[...]' representation even when loaded in a
                // compiled-equivalent context.
                if l
                    .try_is_native_compiled()
                    .map_err(|_| self.interp().error("Cannot inspect a mutably borrowed byte-code object"))?
                {
                    let nm = self.lambda_name(l);
                    if let Some(n) = nm {
                        self.push_format(out, format_args!("#<subr {}>", n))?;
                        return Ok(());
                    }
                }
                self.push_text(out, "#[")?;
                if let Some(bc) = &l.bc_items {
                    // Byte-code objects print as their raw slots like
                    // GNU: #[ARGDESC "BYTESTR" [CONSTS] DEPTH ...].
                    let len = bc
                        .try_borrow()
                        .map(|items| items.len())
                        .map_err(|_| {
                            self.interp()
                                .error("Cannot print a mutably borrowed byte-code object")
                        })?;
                    let limit = self.print_length_limit();
                    let printed = limit.map_or(len, |limit| limit.min(len));
                    for i in 0..printed {
                        if i > 0 {
                            self.push_char(out, ' ')?;
                        }
                        // A slot can be changed by an unreadable callback
                        // while an earlier slot is being printed.
                        let item = bc
                            .try_borrow()
                            .map(|items| items.get(i).cloned())
                            .map_err(|_| {
                                self.interp()
                                    .error("Cannot print a mutably borrowed byte-code object")
                            })?;
                        if let Some(item) = item {
                            self.prin1_inner(&item, out, depth + 1, bq)?;
                        }
                    }
                    if printed < len {
                        if printed > 0 {
                            self.push_char(out, ' ')?;
                        }
                        self.push_text(out, "...")?;
                    }
                    self.push_char(out, ']')?;
                    return Ok(());
                }
                // Emacs 31 prints interpreted functions like
                // #[(x) (x) nil] — arglist, body forms, environment.
                let limit = self.print_length_limit();
                let printed = limit.map_or(3, |limit| limit.min(3));
                if printed > 0 {
                    self.print_lambda_list(l, out)?;
                }
                if printed > 1 {
                    self.push_char(out, ' ')?;
                    // An empty body is a single implicit nil form.
                    let body = l.body_value.clone().unwrap_or_else(|| {
                        if l.body.is_empty() {
                            Value::list(vec![Value::Nil])
                        } else {
                            Value::list(l.body.clone())
                        }
                    });
                    self.prin1_inner(&body, out, depth + 1, bq)?;
                }
                if printed > 2 {
                    self.push_char(out, ' ')?;
                    match &l.env {
                        // A dynamic (non-lexical) lambda has no captured
                        // environment: GNU prints `nil'.
                        None => self.push_text(out, "nil")?,
                        Some(frame) => {
                            let env = if let Some(interp) = self.interp_mut() {
                                lex_frame_to_value(interp, frame)?
                            } else {
                                crate::lisp::eval::lexenv_as_value(&Some(frame.clone()))
                            };
                            self.prin1_inner(&env, out, depth + 1, bq)?;
                        }
                    }
                }
                if printed < 3 {
                    if printed > 0 {
                        self.push_char(out, ' ')?;
                    }
                    self.push_text(out, "...")?;
                }
                self.push_char(out, ']')?;
            }
            Value::Buffer(b) => {
                let live = b
                    .try_borrow()
                    .map(|buffer| buffer.live)
                    .map_err(|_| self.interp().error("Cannot print a mutably borrowed buffer"))?;
                if live {
                    self.push_text(out, "#<buffer ")?;
                    let name = b
                        .try_borrow()
                        .map(|buffer| buffer.name.clone())
                        .map_err(|_| {
                            self.interp().error("Cannot print a mutably borrowed buffer")
                        })?;
                    self.push_text(out, &name)?;
                    self.push_char(out, '>')?;
                } else {
                    self.push_text(out, "#<killed buffer>")?;
                }
            }
            Value::Marker(m) => {
                self.push_text(out, "#<marker ")?;
                let insertion_type = m
                    .try_borrow()
                    .map(|marker| marker.insertion_type)
                    .map_err(|_| self.interp().error("Cannot print a mutably borrowed marker"))?;
                if insertion_type {
                    self.push_text(out, "(moves after insertion) ")?;
                }
                let buffer = m
                    .try_borrow()
                    .map(|marker| marker.buffer)
                    .map_err(|_| self.interp().error("Cannot print a mutably borrowed marker"))?;
                match buffer {
                    Some(buf) => {
                        let position = m
                            .try_borrow()
                            .map(|marker| marker.position + 1)
                            .map_err(|_| {
                                self.interp().error("Cannot print a mutably borrowed marker")
                            })?;
                        self.push_format(out, format_args!("at {} in ", position))?;
                        self.push_text(out, &self.buffer_name_by_id(buf))?;
                        self.push_char(out, '>')?;
                    }
                    None => {
                        // GNU appends the insertion-type annotation
                        // even for detached markers.
                        self.push_text(out, "in no buffer")?;
                        self.push_char(out, '>')?;
                    }
                }
            }
            Value::Window(w) => {
                let id = w
                    .try_borrow()
                    .map(|window| window.id)
                    .map_err(|_| self.interp().error("Cannot print a mutably borrowed window"))?;
                self.push_format(out, format_args!("#<window {} on ", id))?;
                let buffer = w
                    .try_borrow()
                    .map(|window| window.buffer)
                    .map_err(|_| self.interp().error("Cannot print a mutably borrowed window"))?;
                self.push_text(out, &self.buffer_name_by_id(buffer))?;
                self.push_char(out, '>')?;
            }
            Value::Frame(f) => {
                let name = f
                    .try_borrow()
                    .map(|frame| frame.name.clone())
                    .map_err(|_| self.interp().error("Cannot print a mutably borrowed frame"))?
                    .try_borrow()
                    .map(|name| name.clone())
                    .map_err(|_| self.interp().error("Cannot print a mutably borrowed frame name"))?;
                self.push_text(out, "#<frame ")?;
                self.push_text(out, &name)?;
                self.push_format(out, format_args!(" {:#x}>", Rc::as_ptr(f) as usize))?;
            }
            Value::Process(p) => {
                self.push_text(out, "#<process ")?;
                let name = p
                    .try_borrow()
                    .map(|process| process.name.clone())
                    .map_err(|_| self.interp().error("Cannot print a mutably borrowed process"))?;
                self.push_text(out, &name)?;
                self.push_char(out, '>')?;
            }
            // GNU prints the machine address; any unique pointer works.
            Value::Thread(t) => {
                self.push_format(
                    out,
                    format_args!("#<thread 0x{:x}>", Rc::as_ptr(t) as usize),
                )?;
            }
            Value::Mutex(m) => {
                let name = m
                    .try_borrow()
                    .map(|mutex| mutex.name.clone())
                    .map_err(|_| self.interp().error("Cannot print a mutably borrowed mutex"))?;
                match name {
                    Some(n) => {
                        self.push_text(out, "#<mutex ")?;
                        self.push_text(out, &n)?;
                        self.push_char(out, '>')?;
                    }
                    None => self.push_text(out, "#<mutex>")?,
                }
            }
            Value::CondVar(c) => {
                let name = c
                    .try_borrow()
                    .map(|condition| condition.name.clone())
                    .map_err(|_| self.interp().error("Cannot print a mutably borrowed condition variable"))?;
                match name {
                    Some(n) => {
                        self.push_text(out, "#<condvar ")?;
                        self.push_text(out, &n)?;
                        self.push_char(out, '>')?;
                    }
                    None => self.push_text(out, "#<condvar>")?,
                }
            }
            Value::Finalizer(_) => self.push_text(out, "#<finalizer>")?,
        }
        Ok(())
    }

    fn princ_inner_obj_raw(
        &mut self,
        v: &Value,
        out: &mut PrintText,
        depth: usize,
        bq: bool,
    ) -> Result<(), crate::lisp::error::Flow> {
        if let Some(level) = self.print_level_limit() {
            if depth >= level {
                if let Value::Cons(_) = v {
                    self.push_text(out, "...")?;
                    return Ok(());
                }
            }
        }
        match v {
            Value::Int(i) => self.print_integer(*i, out, PrintMode::Display)?,
            Value::BigInt(i) => {
                self.push_format(out, format_args!("{i}"))?;
            }
            Value::Str(s) => self.push_string_chars(out, s)?,
            // princ prints symbol names raw — no backslash escapes.
            Value::Sym(id) => {
                let prefixed = self.obarray.symbol(*id).uninterned && self.print_gensym();
                if prefixed {
                    self.push_text(out, "#:")?;
                }
                let name = self.print_symbol_name(*id);
                if name.is_empty() && !prefixed {
                    self.push_text(out, "##")?;
                } else {
                    self.push_text(out, &name)?;
                }
            }
            // princ prints live buffers as bare names; a killed buffer
            // prints `#<killed buffer>' like the escaped form.
            Value::Buffer(b) => {
                let (live, name) = b
                    .try_borrow()
                    .map(|buffer| (buffer.live, buffer.name.clone()))
                    .map_err(|_| self.interp().error("Cannot print a mutably borrowed buffer"))?;
                if live {
                    self.push_text(out, &name)?;
                } else {
                    self.push_text(out, "#<killed buffer>")?;
                }
            }
            Value::Process(p) => {
                let name = p
                    .try_borrow()
                    .map(|process| process.name.clone())
                    .map_err(|_| self.interp().error("Cannot print a mutably borrowed process"))?;
                self.push_text(out, &name)?;
            }
            Value::Cons(_) => self.print_list_princ(v, out, depth, bq)?,
            Value::Vec(items) => {
                if let Some(r) = self.overlay_repr(items) {
                    self.push_text(out, &r)?;
                    return Ok(());
                }
                self.push_char(out, '[')?;
                let limit = self.print_length_limit();
                let mut n = 0usize;
                let mut truncated = false;
                let len = items
                    .try_borrow()
                    .map(|items| items.len())
                    .map_err(|_| self.interp().error("Cannot print a mutably borrowed vector"))?;
                while n < len {
                    if let Some(l) = limit {
                        if n >= l {
                            truncated = true;
                            break;
                        }
                    }
                    // Match the readable path's live slot reads and keep
                    // Lisp callbacks free to mutate the remaining vector.
                    if n > 0 {
                        self.push_char(out, ' ')?;
                    }
                    let item = items
                        .try_borrow()
                        .map(|items| items.get(n).cloned())
                        .map_err(|_| self.interp().error("Cannot print a mutably borrowed vector"))?;
                    let Some(item) = item else {
                        break;
                    };
                    self.princ_inner(&item, out, depth + 1, bq)?;
                    n += 1;
                }
                if truncated {
                    self.push_text(out, if n == 0 { "..." } else { " ..." })?;
                }
                self.push_char(out, ']')?;
            }
            // Records print `#s(elts...)' under `princ' too — delegating
            // to `prin1_inner' would re-hit the being-printed check and
            // emit `#N' for the pushed object itself.
            Value::Record(items) => {
                self.print_record(items, out, depth, bq, PrintMode::Display)?;
            }
            // Lambdas/subrs would re-hit the being-printed check in
            // `prin1_inner' (the outer `princ_inner' already pushed
            // this object) and emit `#N' — call the raw printer.
            _ => self.prin1_inner_obj_raw(v, out, depth, bq)?,
        }
        Ok(())
    }
}

impl Interp {
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
        self.q_soft(name)
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
    fn quote_abbrev(
        &self,
        v: &Value,
        bq: bool,
    ) -> Result<Option<(&'static str, Value, bool)>, Flow> {
        use super::obarray::sym;
        if !self.print_quoted() {
            return Ok(None);
        }
        if let Value::Cons(c) = v {
            let (car, cdr) = {
                let b = c
                    .try_borrow()
                    .map_err(|_| self.error("Cannot print a mutably borrowed cons cell"))?;
                (b.car.clone(), b.cdr.clone())
            };
            // `(\, x)` sugar only inside a `` ` `` context; outside the
            // comma symbol prints escaped as `\,`.
            let (prefix, inner_bq) = match car {
                Value::Sym(sym::QUOTE) => ("'", bq),
                Value::Sym(sym::FUNCTION) => ("#'", bq),
                Value::Sym(sym::BACKQUOTE) => ("`", true),
                // The argument of an unquote is outside the enclosing
                // backquote; nested comma forms there must print as data.
                Value::Sym(sym::COMMA) if bq => (",", false),
                // A nested splice argument, like an ordinary unquote, is
                // printed outside the enclosing backquote context.
                Value::Sym(sym::COMMA_AT) if bq => (",@", false),
                Value::Sym(sym::COMMA_DOT) if bq => (",.", bq),
                _ => return Ok(None),
            };
            if let Value::Cons(c2) = &cdr {
                let (cadr, cddr) = {
                    let b = c2.try_borrow().map_err(|_| {
                        self.error("Cannot print a mutably borrowed cons cell")
                    })?;
                    (b.car.clone(), b.cdr.clone())
                };
                if cddr.is_nil() {
                    return Ok(Some((prefix, cadr, inner_bq)));
                }
            }
        }
        Ok(None)
    }

    fn print_symbol_name(&self, id: super::value::SymId) -> EmacsText {
        self.symbol_name_text(id)
    }

    fn buffer_name_by_id(&self, id: usize) -> EmacsText {
        self.buffers
            .get(id)
            .and_then(|buffer| buffer.try_borrow().ok().map(|buffer| buffer.name.clone()))
            .unwrap_or_else(|| "*killed buffer*".into())
    }
}

/// Return the canonical interpreter-environment alist, reporting a borrow
/// conflict through Lisp-visible printer entry points.
fn lex_frame_to_value(
    i: &mut Interp,
    frame: &Rc<crate::lisp::eval::LexFrame>,
) -> Result<Value, Flow> {
    crate::lisp::eval::lexenv_as_value_checked(&Some(frame.clone()))
        .map_err(|error| error.into_flow(i))
}
