//! Structured object renderers: overlay, record, closure and character-table syntax.

use super::render::PrintMode;
use crate::lisp::{Interp, print_text::PrintText, text::EmacsText, value::Value};
use std::{cell::RefCell, fmt::Write, rc::Rc};

impl Interp {
    /// When ITEMS is an `[overlay BID IDX]' handle, produce GNU's
    /// `#<overlay from B to E in NAME>' (or `#<overlay in no buffer>'
    /// once detached/killed).
    pub(super) fn overlay_repr(&self, items: &Rc<RefCell<Vec<Value>>>) -> Option<EmacsText> {
        let value = Value::Vec(items.clone());
        if !self.is_overlay(&value) {
            return None;
        }
        let ov = match self.overlay_state(&value) {
            Some(o) => o,
            // Detached/killed handles still print as overlays.
            None => return Some("#<overlay in no buffer>".into()),
        };
        match ov
            .buffer
            .and_then(|id| self.buffers.get(id).map(|b| b.borrow().name.clone()))
        {
            Some(name) => {
                let mut text = EmacsText::from(format!(
                    "#<overlay from {} to {} in ",
                    ov.start + 1,
                    ov.end + 1
                ));
                text.append(&name);
                text.push('>');
                Some(text)
            }
            None => Some("#<overlay in no buffer>".into()),
        }
    }

    /// Print a record value. `princ' selects the unescaped inner
    /// printer for record slots (`#s(foo x)' vs `#s(foo "x")').
    pub(super) fn print_record(
        &self,
        items: &std::rc::Rc<std::cell::RefCell<Vec<Value>>>,
        out: &mut PrintText,
        depth: usize,
        bq: bool,
        mode: PrintMode,
    ) {
        if crate::editor::window_configuration_object(&Value::Record(items.clone())) {
            out.push_str("#<window-configuration>");
            return;
        }
        macro_rules! inner {
            ($v:expr) => {
                mode.render(self, $v, out, depth + 1, bq)
            };
        }
        {
            let rr = items.borrow();
            if let Some((symbol, position)) =
                crate::lisp::builtins::misc::raw_symbol_position(&Value::Record(items.clone()))
            {
                let bare = self.sym(symbol);
                if !self.print_var("print-symbols-bare").truthy() {
                    out.push_str("#<symbol ");
                    inner!(&bare);
                    let _ = write!(out, " at {}>", position);
                } else {
                    inner!(&bare);
                }
                return;
            }
            if let Some(Value::Sym(t)) = rr.first() {
                // Obarrays print `#<obarray n=COUNT>' — COUNT is
                // the number of interned symbols.
                if Value::Record(items.clone()).record_kind()
                    == Some(crate::lisp::value::RecordKind::Obarray)
                {
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
                        let _ = write!(out, "#<terminal {} on ", n);
                        out.push_str(&name.borrow());
                        out.push('>');
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
                if Value::Record(items.clone()).record_kind()
                    == Some(crate::lisp::value::RecordKind::FontSpec)
                {
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
                if Value::Record(items.clone()).record_kind()
                    == Some(crate::lisp::value::RecordKind::CharTable)
                {
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
            let is_bv = Value::Record(items.clone()).record_kind()
                == Some(crate::lisp::value::RecordKind::BoolVector);
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

    pub(super) fn print_lambda_list(&self, l: &crate::lisp::value::Lambda, out: &mut PrintText) {
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
        let mut names: Vec<EmacsText> = l
            .required
            .iter()
            .map(|s| self.print_symbol_name(*s))
            .collect();
        if !l.optional.is_empty() {
            names.push("&optional".into());
            for o in &l.optional {
                names.push(self.print_symbol_name(o.sym));
            }
        }
        if let Some(r) = l.rest {
            names.push("&rest".into());
            names.push(self.print_symbol_name(r));
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
    pub(super) fn print_char_table(
        &self,
        ptr: usize,
        rr: &[Value],
        slots: &[Value],
        out: &mut PrintText,
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
}
