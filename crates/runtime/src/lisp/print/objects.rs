//! Structured object renderers: overlay, record, closure and character-table syntax.

use super::render::{PrintMode, Printer};
use crate::lisp::{print_text::PrintText, text::EmacsText, value::Value};
use std::{cell::RefCell, rc::Rc};

impl Printer<'_> {
    fn record_len(
        &self,
        items: &Rc<RefCell<Vec<Value>>>,
    ) -> Result<usize, crate::lisp::error::Flow> {
        items
            .try_borrow()
            .map(|items| items.len())
            .map_err(|_| self.interp().error("Cannot print a mutably borrowed record"))
    }

    fn record_slot(
        &self,
        items: &Rc<RefCell<Vec<Value>>>,
        index: usize,
    ) -> Result<Option<Value>, crate::lisp::error::Flow> {
        items
            .try_borrow()
            .map(|items| items.get(index).cloned())
            .map_err(|_| self.interp().error("Cannot print a mutably borrowed record"))
    }

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
            .and_then(|id| {
                self.buffers
                    .get(id)
                    .and_then(|buffer| buffer.try_borrow().ok().map(|buffer| buffer.name.clone()))
            })
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
        &mut self,
        items: &std::rc::Rc<std::cell::RefCell<Vec<Value>>>,
        out: &mut PrintText,
        depth: usize,
        bq: bool,
        mode: PrintMode,
    ) -> Result<(), crate::lisp::error::Flow> {
        if crate::editor::window_configuration_object(&Value::Record(items.clone())) {
            self.push_text(out, "#<window-configuration>")?;
            return Ok(());
        }
        macro_rules! inner {
            ($v:expr) => {
                mode.render(self, $v, out, depth + 1, bq)?
            };
        }
        {
            // Keep a snapshot for tag/shape dispatch, but do not retain a
            // RefCell borrow across recursive printing. Unreadable callbacks
            // may mutate this record while it is being printed.
            let rr = items
                .try_borrow()
                .map(|items| items.clone())
                .map_err(|_| {
                    self.interp()
                        .error("Cannot print a mutably borrowed record")
                })?;
            if let Some((symbol, position)) =
                crate::lisp::builtins::misc::raw_symbol_position(&Value::Record(items.clone()))
            {
                let bare = self.sym(symbol);
                if !self.print_var("print-symbols-bare").truthy() {
                    self.push_text(out, "#<symbol ")?;
                    inner!(&bare);
                    self.push_format(out, format_args!(" at {}>", position))?;
                } else {
                    inner!(&bare);
                }
                return Ok(());
            }
            if let Some(Value::Sym(_)) = rr.first() {
                // Obarrays print `#<obarray n=COUNT>' — COUNT is
                // the number of interned symbols.
                if Value::Record(items.clone()).record_kind()
                    == Some(crate::lisp::value::RecordKind::Obarray)
                {
                    let n = match rr.get(1) {
                        Some(Value::Vec(syms)) => syms
                            .try_borrow()
                            .map(|syms| {
                                syms.iter()
                                    .filter(|x| matches!(x, Value::Sym(_)))
                                    .count()
                            })
                            .map_err(|_| {
                                self.interp().error("Cannot print a mutably borrowed obarray")
                            })?,
                        _ => 0,
                    };
                    self.push_format(out, format_args!("#<obarray n={}>", n))?;
                    return Ok(());
                }
                // Terminals print `#<terminal N on NAME>'.
                if crate::editor::is_terminal(self.interp(), &Value::Record(items.clone())) {
                    if let [_, Value::Int(n), Value::Str(_)] = rr.as_slice() {
                        self.push_format(out, format_args!("#<terminal {} on ", n))?;
                        let name = match self.record_slot(items, 2)? {
                            Some(Value::Str(name)) => Some(name),
                            _ => None,
                        };
                        if let Some(name) = name {
                            self.push_string_chars(out, &name)?;
                        }
                        self.push_char(out, '>')?;
                        return Ok(());
                    }
                }
                // Tree-sitter objects print `#<treesit-parser in
                // BUF for LANG>' / `#<treesit-node TYPE in B-E>'.
                if let Some(s) = crate::lisp::builtins::treesit::treesit_repr(self, rr.as_slice()) {
                    self.push_text(out, &s)?;
                    return Ok(());
                }
                // Sub char tables print `#^^[DEPTH MIN-CHAR
                // SLOTS...]' — GNU's trie nodes.
                if Value::Record(items.clone()).record_kind()
                    == Some(crate::lisp::value::RecordKind::SubCharTable)
                    && rr.len() >= 3
                {
                    self.push_text(out, "#^^[")?;
                    let depth_value = self.record_slot(items, 1)?.unwrap_or(Value::Nil);
                    inner!(&depth_value);
                    self.push_char(out, ' ')?;
                    let min_char = self.record_slot(items, 2)?.unwrap_or(Value::Nil);
                    inner!(&min_char);
                    // GNU's `print_stack_push_vector' measures
                    // `print-length' against the pseudovector SIZE
                    // (depth+min_char header + slots) but starts
                    // printing at SUB_CHAR_TABLE_OFFSET: a truncated
                    // sub-table emits `print-length - 1' slots.
                    let limit = self.print_length_limit().map(|l| l.saturating_sub(1));
                    let slot_count = self.record_len(items)?.saturating_sub(3);
                    let printed = limit.map_or(slot_count, |limit| limit.min(slot_count));
                    for index in 0..printed {
                        self.push_char(out, ' ')?;
                        let slot = self
                            .record_slot(items, index + 3)?
                            .unwrap_or(Value::Nil);
                        inner!(&slot);
                    }
                    if printed < slot_count {
                        self.push_text(out, " ...")?;
                    }
                    self.push_char(out, ']')?;
                    return Ok(());
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
                    self.push_text(out, "#<font-spec")?;
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
                            let b = c.try_borrow().map_err(|_| {
                                self.interp().error("Cannot print a mutably borrowed cons cell")
                            })?;
                            (b.car.clone(), b.cdr.clone())
                        };
                        if let Value::Cons(c2) = cdr {
                            let (val, next) = {
                                let b = c2.try_borrow().map_err(|_| {
                                    self.interp()
                                        .error("Cannot print a mutably borrowed cons cell")
                                })?;
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
                        self.push_char(out, ' ')?;
                        inner!(slot.as_ref().unwrap_or(&Value::Nil));
                    }
                    self.push_char(out, ' ')?;
                    inner!(&Value::list(extras));
                    self.push_char(out, '>')?;
                    return Ok(());
                }
                if Value::Record(items.clone()).record_kind()
                    == Some(crate::lisp::value::RecordKind::CharTable)
                {
                    if let Some(Value::Vec(slots)) = rr.get(2) {
                        let ptr = std::rc::Rc::as_ptr(items) as usize;
                        self.print_char_table(ptr, items, &slots, out, depth, bq)?;
                        return Ok(());
                    }
                }
            }
            // Bool vectors print `#&N"bytes"' with bits packed
            // LSB-first per byte.
            let is_bv = Value::Record(items.clone()).record_kind()
                == Some(crate::lisp::value::RecordKind::BoolVector);
            if is_bv {
                if let Some(Value::Vec(bits)) = rr.get(1) {
                    let n = bits
                        .try_borrow()
                        .map(|bits| bits.len())
                        .map_err(|_| self.interp().error("Cannot print a mutably borrowed vector"))?;
                    self.push_format(out, format_args!("#&{}\"", n))?;
                    for k in 0..n.div_ceil(8) {
                        let byte = {
                            let bits = bits.try_borrow().map_err(|_| {
                                self.interp().error("Cannot print a mutably borrowed vector")
                            })?;
                            let mut byte = 0u32;
                            for j in 0..8 {
                                if let Some(Value::Int(b)) = bits.get(k * 8 + j) {
                                    if *b != 0 {
                                        byte |= 1 << j;
                                    }
                                }
                            }
                            byte
                        };
                        match byte {
                            34 => self.push_text(out, "\\\"")?,
                            92 => self.push_text(out, "\\\\")?,
                            0..=127 => self.push_char(out, byte as u8 as char)?,
                            _ => {
                                self.push_format(out, format_args!("\\{:03o}", byte))?;
                            }
                        }
                    }
                    self.push_char(out, '"')?;
                    return Ok(());
                }
            }
        }
        self.push_text(out, "#s(")?;
        let len = self.record_len(items)?;
        let limit = self.print_length_limit();
        let printed = limit.map_or(len, |limit| limit.min(len));
        for i in 0..printed {
            if i > 0 {
                self.push_char(out, ' ')?;
            }
            // Read slots only when their turn arrives so callback changes
            // to later fields are visible and no immutable borrow spans Lisp.
            let item = self.record_slot(items, i)?;
            if let Some(item) = item {
                inner!(&item);
            }
        }
        if printed < len {
            if printed > 0 {
                self.push_char(out, ' ')?;
            }
            self.push_text(out, "...")?;
        }
        self.push_char(out, ')')?;
        Ok(())
    }

    pub(super) fn print_lambda_list(
        &mut self,
        l: &crate::lisp::value::Lambda,
        out: &mut PrintText,
    ) -> Result<(), crate::lisp::error::Flow> {
        if let Some(raw) = &l.arglist {
            self.prin1_inner(raw, out, 1, false)?;
            return Ok(());
        }
        if l.required.is_empty() && l.optional.is_empty() && l.rest.is_none() {
            self.push_text(out, "nil")?;
            return Ok(());
        }
        self.push_char(out, '(')?;
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
                self.push_char(out, ' ')?;
            }
            first = false;
            self.push_text(out, &n)?;
        }
        self.push_char(out, ')')?;
        Ok(())
    }

    /// GNU char-table printing: `#^[DEFALT PARENT PURPOSE ASCII
    /// CONTENTS[64] EXTRAS...]'.  Our Record layout is
    /// `[char-table SUBTYPE VEC65 EXTRAS...]' where VEC65 is the trie
    /// root: slot 0 = ASCII cache (scalar or the `#^^[3 0' leaf),
    /// slots 1-64 = top-level blocks.  Sub-char-tables print through
    /// `prin1_inner's `sub-char-table' Record branch as `#^^[...]'.
    pub(super) fn print_char_table(
        &mut self,
        ptr: usize,
        record: &Rc<RefCell<Vec<Value>>>,
        slots: &Rc<RefCell<Vec<Value>>>,
        out: &mut PrintText,
        depth: usize,
        bq: bool,
    ) -> Result<(), crate::lisp::error::Flow> {
        self.push_text(out, "#^[")?;
        // defalt/parent (side-tables keyed on the Record pointer;
        // Weak guards against address reuse by a different live
        // char-table)
        let slots_len = slots
            .try_borrow()
            .map(|slots| slots.len())
            .map_err(|_| self.interp().error("Cannot print a mutably borrowed vector"))?;
        let extras_len = self.record_len(record)?.saturating_sub(3);
        let total_len = 3 + slots_len + extras_len;
        // GNU `print_stack_push_vector' applies `print-length' to the
        // whole pseudovector: defalt, parent, purpose, ascii cache,
        // contents[64] and extras all count toward the limit.
        let limit = self.print_length_limit();
        let mut n = 0usize;
        let print_len = limit.map_or(total_len, |limit| limit.min(total_len));
        while n < print_len {
            if n > 0 {
                self.push_char(out, ' ')?;
            }
            let value = match n {
                0 => self
                    .char_table_defalts
                    .iter()
                    .find(|(key, weak, _)| *key == ptr && weak.upgrade().is_some())
                    .map(|(_, _, value)| value.clone())
                    .unwrap_or(Value::Nil),
                1 => self
                    .char_table_parents
                    .iter()
                    .find(|(key, weak, _)| *key == ptr && weak.upgrade().is_some())
                    .map(|(_, _, value)| value.clone())
                    .unwrap_or(Value::Nil),
                2 => self.record_slot(record, 1)?.unwrap_or(Value::Nil),
                index if index < 3 + slots_len => {
                    slots
                        .try_borrow()
                        .map(|slots| slots.get(index - 3).cloned())
                        .map_err(|_| {
                            self.interp().error("Cannot print a mutably borrowed vector")
                        })?
                        .unwrap_or(Value::Nil)
                }
                index => self
                    .record_slot(record, index - slots_len)?
                    .unwrap_or(Value::Nil),
            };
            self.prin1_inner(&value, out, depth + 1, bq)?;
            n += 1;
        }
        if print_len < total_len {
            self.push_text(out, " ...")?;
        }
        self.push_char(out, ']')?;
        Ok(())
    }
}
