//! Symbol identity, variable cells, aliases, properties, and watcher notifications.

use super::{Interp, plist_get, plist_lookup, plist_put};
use crate::lisp::{
    error::Flow,
    obarray::sym,
    value::{SymId, Value},
};
use num_traits::ToPrimitive;

impl Interp {
    pub fn intern<T: crate::lisp::text::TextSource + ?Sized>(&mut self, name: &T) -> SymId {
        self.obarray
            .intern_with_key(name, crate::lisp::obarray::InternKey::from_text(name))
    }

    pub(crate) fn q(&mut self, name: &str) -> SymId {
        if let Some(&symbol) = self.fixed_symbols.get(name) {
            return symbol;
        }
        let symbol = self
            .obarray
            .first_interned_symbol(name)
            .unwrap_or_else(|| self.obarray.intern(name));
        self.fixed_symbols.insert(name.to_owned(), symbol);
        symbol
    }

    pub fn intern_soft(&self, name: &str) -> Option<SymId> {
        self.obarray.intern_soft(name)
    }

    pub(crate) fn q_soft(&self, name: &str) -> Option<SymId> {
        self.fixed_symbols
            .get(name)
            .copied()
            .or_else(|| self.obarray.first_interned_symbol(name))
            .or_else(|| self.obarray.intern_soft(name))
    }

    pub fn make_symbol<T: crate::lisp::text::TextSource + ?Sized>(&mut self, name: &T) -> SymId {
        self.obarray.make_symbol(name)
    }

    /// `Value::Sym`, normalized so id 0 becomes `Value::Nil`.
    pub fn sym(&self, id: SymId) -> Value {
        if id == sym::NIL {
            Value::Nil
        } else {
            Value::Sym(id)
        }
    }

    pub(crate) fn symbol_name_text(&self, id: SymId) -> crate::lisp::text::EmacsText {
        if id == u32::MAX {
            return crate::lisp::text::EmacsText::new();
        }
        self.obarray
            .symbol(id)
            .name_string
            .as_ref()
            .map(|text| {
                text.try_borrow()
                    .map(|text| (*text).clone())
                    // Formatting is also used while reporting errors, where
                    // there is no Lisp flow to propagate. If a host callback
                    // reenters the interpreter while mutably borrowing a
                    // symbol's shared name string, use the symbol's stable
                    // original name instead of panicking in RefCell::borrow.
                    .unwrap_or_else(|_| crate::lisp::text::EmacsText::from(self.symbol_name(id)))
            })
            .unwrap_or_else(|| crate::lisp::text::EmacsText::from(self.symbol_name(id)))
    }

    pub fn symbol_name(&self, id: SymId) -> String {
        // SymId::MAX = an `intern_soft' miss; an absent symbol's name
        // resolves to "" (never interned, never equal to a real name).
        if id == u32::MAX {
            return String::new();
        }
        self.obarray.name(id).to_string()
    }

    pub(crate) fn positioned_symbols_enabled(&self) -> bool {
        let id = match self.symbol_position_flag.get() {
            Some(id) => id,
            None => {
                let Some(id) = self.q_soft("symbols-with-pos-enabled") else {
                    return false;
                };
                self.symbol_position_flag.set(Some(id));
                id
            }
        };
        let value = self.symbol_value(id);
        !value.is_nil() && !matches!(value, Value::Sym(s) if s == sym::UNBOUND)
    }

    pub(crate) fn restore_symbol_position_flag(&self, id: SymId) {
        self.symbol_position_flag.set(Some(id));
    }

    pub(crate) fn symbol_position_scope(&self) -> crate::lisp::value::SymbolPositionScope {
        crate::lisp::value::SymbolPositionScope::enter(
            self as *const Self as usize,
            self.positioned_symbols_enabled(),
        )
    }

    pub(crate) fn sync_symbol_position_view(&self) {
        crate::lisp::value::sync_symbol_positions(
            self as *const Self as usize,
            self.positioned_symbols_enabled(),
        );
    }

    pub fn sym_is(&self, v: &Value, id: SymId) -> bool {
        match v {
            Value::Sym(s) => *s == id,
            Value::Nil => id == sym::NIL,
            _ => crate::lisp::builtins::misc::symbol_position_for(self, v)
                .map(|(s, _)| s == id)
                .unwrap_or(false),
        }
    }

    /// The symbol id of a `Value::Sym`/`Value::Nil`/enabled
    /// `symbols-with-pos-enabled' object, if it is one.  GNU's enabled
    /// sympos are symbols for every primitive (`symbol-name', `get',
    /// `fboundp', ...), so they unwrap to their bare id here.
    pub fn sym_id(&self, v: &Value) -> Option<SymId> {
        match v {
            Value::Sym(s) => Some(*s),
            Value::Nil => Some(sym::NIL),
            _ => crate::lisp::builtins::misc::symbol_position_for(self, v).map(|(s, _)| s),
        }
    }

    /// Global (default) value of a symbol.
    /// Follow `defvaralias' chains: return the ultimate base symbol.
    pub fn var_alias_target(&self, id: SymId) -> SymId {
        if id == u32::MAX {
            return id; // intern_soft miss sentinel: no alias chain
        }
        let mut cur = id;
        let mut seen = std::collections::HashSet::new();
        loop {
            match self.obarray.symbol(cur).var_alias {
                Some(next) => {
                    // GNU follows aliases without a depth limit. If a cycle
                    // reaches this low-level lookup, indirect-variable
                    // returns its original argument.
                    if !seen.insert(cur) {
                        return id;
                    }
                    cur = next;
                }
                None => return cur,
            }
        }
    }

    #[inline]
    pub fn symbol_value(&self, id: SymId) -> Value {
        self.symbol_value_in_buffer(id, self.current_buffer)
    }

    /// Read the active binding in BUFFER without changing the current buffer.
    pub(crate) fn symbol_value_in_buffer(&self, id: SymId, buffer: usize) -> Value {
        let id = self.var_alias_target(id);
        // Buffer-local binding in the requested buffer wins. `try_borrow`:
        // primitives that hold the buffer mutably borrowed may still
        // consult variables (they see the global binding).
        if id == u32::MAX {
            return Value::Nil; // intern_soft miss — unbound is nil
        }
        if self.quit_flag && self.q_soft("quit-flag") == Some(id) {
            return Value::t();
        }
        if let Some(b) = self.buffers.get(buffer) {
            if let Ok(bb) = b.try_borrow() {
                if let Some(v) = bb.locals.get(&id) {
                    return v.clone();
                }
                // `buffer-file-name' is a BVAR: the real buffer slot is
                // the binding; no Lisp-level entry means the slot.
                if self.bfn_sym != 0 && id == self.bfn_sym {
                    return match &bb.file_name {
                        Some(f) => Value::string(f.clone()),
                        None => Value::Nil,
                    };
                }
            }
        }
        let v = &self.obarray.symbol(id).value;
        if let Value::Sym(s) = v {
            if *s == sym::UNBOUND {
                // A voided dump-time variable (GNU_VOID_VARS) still
                // answers reads with its stashed dump value —
                // expansion-time machinery (cl-labels' convert cache,
                // pcase's memoize table, cl-macs's float limits, ...)
                // keeps working while `boundp' stays nil like GNU -Q.
                if let Some(stashed) = self.dumped_variable(id) {
                    return stashed;
                }
                return Value::Sym(sym::UNBOUND);
            }
        }
        v.clone()
    }

    /// Hidden dump-time value for SYM, if it was stashed while the
    /// public value cell was voided for GNU -Q compatibility.
    fn dumped_variable(&self, id: SymId) -> Option<Value> {
        let prop = self.q_soft("remacs--dump-var")?;
        // `nil' is a real stashed value (cl--labels-convert-cache's
        // cache starts nil): test plist membership, not truthiness. The
        // symbol plist is mutable Lisp data and may be circular.
        plist_lookup(&self.obarray.symbol(id).plist, prop)
    }

    pub fn bound_p(&self, id: SymId) -> bool {
        let id = self.var_alias_target(id);
        // Constants (nil, t, keywords) are always bound.
        if self.obarray.symbol(id).constant {
            return true;
        }
        if let Some(b) = self.buffers.get(self.current_buffer) {
            if let Ok(bb) = b.try_borrow() {
                if let Some(v) = bb.locals.get(&id) {
                    // A void local binding (make-local-variable on an
                    // unbound variable) counts as unbound.
                    return !matches!(v, Value::Sym(s) if *s == sym::UNBOUND);
                }
            }
        }
        !matches!(
            self.obarray.symbol(id).value,
            Value::Sym(s) if s == sym::UNBOUND
        )
    }

    /// Set a variable the way `set`/`setq` does: local binding if the
    /// current buffer has one or the symbol is "automatically local".
    pub fn set_symbol(&mut self, id: SymId, val: Value) -> Result<(), Flow> {
        let reported_symbol = self.sym(id);
        let result = self.set_symbol_position_inner(id, val, true, &reported_symbol);
        self.sync_symbol_position_view();
        result
    }

    /// `set`/`setq` variant that retains the Lisp symbol object supplied by
    /// the caller for errors such as `setting-constant'.
    pub(crate) fn set_symbol_as(
        &mut self,
        id: SymId,
        val: Value,
        reported_symbol: &Value,
    ) -> Result<(), Flow> {
        let result = self.set_symbol_position_inner(id, val, true, reported_symbol);
        self.sync_symbol_position_view();
        result
    }

    /// Store a variable in its active binding without firing variable
    /// watchers. Use only for C paths whose GNU implementation directly
    /// updates the variable cell, such as `reset_var_on_error'.
    pub(crate) fn set_symbol_no_watchers(&mut self, id: SymId, val: Value) -> Result<(), Flow> {
        let reported_symbol = self.sym(id);
        let result = self.set_symbol_position_inner(id, val, false, &reported_symbol);
        self.sync_symbol_position_view();
        result
    }

    #[track_caller]
    fn set_symbol_position_inner(
        &mut self,
        id: SymId,
        val: Value,
        notify_watchers: bool,
        reported_symbol: &Value,
    ) -> Result<(), Flow> {
        let id = self.var_alias_target(id);
        self.synchronize_quit_symbol(id);
        let constant = self.obarray.symbol(id).constant;
        if self.dbg_flags & 0x100 != 0 && constant {
            eprintln!(
                "[setconst {} = {}] caller={:?}",
                self.symbol_name(id),
                self.princ_to_string(&val)
                    .chars()
                    .take(80)
                    .collect::<crate::lisp::text::EmacsText>(),
                std::panic::Location::caller()
            );
            for (f, a) in self.lisp_stack.iter().rev().take(20).rev() {
                eprintln!(
                    "  {} {}",
                    self.princ_to_string(f)
                        .chars()
                        .take(100)
                        .collect::<crate::lisp::text::EmacsText>(),
                    self.princ_to_string(&Value::list(a.as_slice().to_vec()))
                        .chars()
                        .take(160)
                        .collect::<crate::lisp::text::EmacsText>()
                );
            }
        }
        if constant && id != sym::NIL && id != sym::T {
            return Err(self.signal_data(
                sym::SETTING_CONSTANT,
                vec![reported_symbol.clone()],
            ));
        }
        if constant {
            // nil/t can't be set at all.
            return Err(self.signal_data(
                sym::SETTING_CONSTANT,
                vec![reported_symbol.clone()],
            ));
        }
        let sym = self.obarray.symbol(id);
        let automatic = sym.make_local_if_set;
        let always_local = sym.always_local;
        let forwarded = sym.forwarded_variable;
        let existing_local = self
            .buffers
            .get(self.current_buffer)
            .and_then(|b| b.try_borrow().ok().map(|bb| bb.locals.contains_key(&id)))
            .unwrap_or(false);
        let default_shadow = automatic && !existing_local && self.specbind_shadows_current(id);
        let local = always_local || existing_local || (automatic && !default_shadow);
        // GNU's set_internal fires variable watchers BEFORE the store:
        // a signalling watcher aborts the write entirely and observers
        // inside the watcher still see the old value.
        if notify_watchers {
            self.fire_var_watchers(
                id,
                &val,
                "set",
                (local || default_shadow).then_some(self.current_buffer),
            )?;
        }
        self.validate_c_integer_forwarding(id, &val)?;
        if default_shadow && forwarded && !always_local {
            // GNU's forwarded per-buffer path also writes the default
            // cell, notifying its watchers separately with WHERE=nil.
            if notify_watchers {
                return self.set_symbol_default_as(id, val, reported_symbol);
            }
            self.obarray.symbol_mut(id).value = val.clone();
            self.sync_undo_inhibit(id, &val);
            return Ok(());
        }
        if local {
            let file_name = if id == self.bfn_sym {
                match &val {
                    Value::Str(name) => Some(
                        name.try_borrow()
                            .map(|name| (*name).clone())
                            .map_err(|_| {
                                self.error(
                                    "Cannot inspect a mutably borrowed buffer-file-name string",
                                )
                            })?,
                    ),
                    _ => None,
                }
            } else {
                None
            };
            if let Some(b) = self.buffers.get(self.current_buffer) {
                if let Ok(mut bb) = b.try_borrow_mut() {
                    bb.locals.insert(id, val.clone());
                    if id == self.bfn_sym {
                        // Mirror the BVAR write into the buffer slot.
                        bb.file_name = file_name;
                    }
                }
            }
            return Ok(());
        }
        self.obarray.symbol_mut(id).value = val.clone();
        self.sync_undo_inhibit(id, &val);
        Ok(())
    }

    /// Mirror `undo-inhibit-record-point' writes into the shared cell
    /// consulted by `Buffer::record_point'.
    pub(super) fn sync_undo_inhibit(&mut self, id: SymId, val: &Value) {
        if self.undo_inhibit_sym != 0 && id == self.undo_inhibit_sym {
            self.buffers.undo_inhibit_cell().set(!val.is_nil());
        }
    }

    /// Set the global (default) value regardless of buffer-local bindings.
    pub fn set_symbol_default(&mut self, id: SymId, val: Value) -> Result<(), Flow> {
        let reported_symbol = self.sym(id);
        let result = self.set_symbol_default_position_inner(id, val, true, &reported_symbol);
        self.sync_symbol_position_view();
        result
    }

    /// `set-default` variant retaining the caller's symbol object in errors.
    pub(crate) fn set_symbol_default_as(
        &mut self,
        id: SymId,
        val: Value,
        reported_symbol: &Value,
    ) -> Result<(), Flow> {
        let result =
            self.set_symbol_default_position_inner(id, val, true, reported_symbol);
        self.sync_symbol_position_view();
        result
    }

    /// Store a global/default value without variable watchers. Use only for
    /// C paths that directly update their forwarded Lisp cell.
    pub(crate) fn set_symbol_default_no_watchers(
        &mut self,
        id: SymId,
        val: Value,
    ) -> Result<(), Flow> {
        let reported_symbol = self.sym(id);
        let result = self.set_symbol_default_position_inner(id, val, false, &reported_symbol);
        self.sync_symbol_position_view();
        result
    }

    #[track_caller]
    fn set_symbol_default_position_inner(
        &mut self,
        id: SymId,
        val: Value,
        notify_watchers: bool,
        reported_symbol: &Value,
    ) -> Result<(), Flow> {
        // GNU `Fset_default' walks the variable-alias chain first:
        // `(set-default 'alias v)' writes BASE's default cell, leaving
        // the alias's own cell a legacy remnant readers ignore.
        let id = self.var_alias_target(id);
        self.synchronize_quit_symbol(id);
        if self.obarray.symbol(id).constant {
            return Err(self.signal_data(
                sym::SETTING_CONSTANT,
                vec![reported_symbol.clone()],
            ));
        }
        if self.dbg_flags & 0x40000 != 0 && self.symbol_name(id) == "default-terminal-coding-system"
        {
            eprintln!(
                "[WATCH set-default] -> {} \n{}",
                self.prin1_to_string(&val),
                std::backtrace::Backtrace::capture()
            );
        }
        // GNU's set_internal fires variable watchers BEFORE the store:
        // a signalling watcher leaves the default cell untouched.
        if notify_watchers {
            self.fire_var_watchers(id, &val, "set", None)?;
        }
        self.validate_c_integer_forwarding(id, &val)?;
        self.obarray.symbol_mut(id).value = val.clone();
        self.sync_undo_inhibit(id, &val);
        Ok(())
    }

    /// These forwarded globals are stored as signed `EMACS_INT` values in
    /// GNU. Validate their Lisp cells before storing them, so bignums outside
    /// that range signal overflow after the usual variable-watcher callback.
    pub(crate) fn validate_c_integer_forwarding(
        &mut self,
        id: SymId,
        value: &Value,
    ) -> Result<(), Flow> {
        let name = self.symbol_name(id);
        // C forwarding belongs to the canonical interned variable, not to
        // an uninterned symbol that merely shares its printed name.
        if self.q_soft(&name) != Some(id) {
            return Ok(());
        }
        if ![
            "auto-save-interval",
            "baud-rate",
            "command-line-max-length",
            "cons-cells-consed",
            "display-line-numbers-major-tick",
            "display-line-numbers-minor-tick",
            "double-click-fuzz",
            "executing-kbd-macro-index",
            "extra-keyboard-modifiers",
            "face-near-same-color-threshold",
            "floats-consed",
            "gc-cons-threshold",
            "gcs-done",
            "gnutls-log-level",
            "hscroll-margin",
            "integer-width",
            "internal-when-entered-debugger",
            "intervals-consed",
            "large-hscroll-threshold",
            "line-number-display-limit-width",
            "lisp-eval-depth-reserve",
            "long-line-optimizations-bol-search-limit",
            "long-line-optimizations-region-size",
            "max-lisp-eval-depth",
            "max-redisplay-ticks",
            "next-screen-context-lines",
            "num-input-keys",
            "num-nonmacro-input-events",
            "overline-margin",
            "process-error-pause-time",
            "profiler-log-size",
            "profiler-max-stack-depth",
            "pure-bytes-used",
            "read-process-output-max",
            "scroll-margin",
            "scroll-conservatively",
            "scroll-step",
            "string-chars-consed",
            "strings-consed",
            "symbols-consed",
            "tab-bar-button-relief",
            "tool-bar-max-label-size",
            "tool-bar-button-relief",
            "undo-limit",
            "undo-strong-limit",
            "underline-minimum-offset",
            "vector-cells-consed",
        ]
        .iter()
        .any(|candidate| name == *candidate)
        {
            return Ok(());
        }
        let Some(integer) = value.as_bigint() else {
            return Err(self.wrong_type_mut("integerp", value));
        };
        if integer.to_isize().is_none() {
            let overflow = self.q("overflow-error");
            return Err(self.signal_data(overflow, vec![value.clone()]));
        }
        Ok(())
    }

    /// `get` — symbol property.
    pub fn get_prop(&self, id: SymId, prop: SymId) -> Value {
        // `intern_soft` misses surface as SymId::MAX at ~40 call sites —
        // an absent symbol has no properties.
        if id == u32::MAX || prop == u32::MAX {
            return Value::Nil;
        }
        let plist = &self.obarray.symbol(id).plist;
        plist_get(plist, prop)
    }

    /// `put` — set symbol property, returns value.
    pub fn put_prop(&mut self, id: SymId, prop: SymId, val: Value) {
        if id == u32::MAX || prop == u32::MAX {
            return; // see get_prop: intern_soft miss sentinel
        }
        self.macro_gen += 1;
        let plist = self.obarray.symbol(id).plist.clone();
        let new_plist = plist_put(&plist, prop, val);
        self.obarray.symbol_mut(id).plist = new_plist;
    }

    /// Run `add-variable-watcher' functions before changing `id'.
    /// GNU calls each watcher as (SYM NEWVAL OPERATION WHERE).
    pub(crate) fn fire_var_watchers(
        &mut self,
        id: SymId,
        newval: &Value,
        op: &str,
        buf: Option<usize>,
    ) -> Result<(), Flow> {
        if self.exit_status.is_some() {
            return Ok(());
        }
        if self.var_watchers.is_empty() || !self.var_watchers.iter().any(|(s, _)| *s == id) {
            return Ok(());
        }
        if let Some(inh) = self.q_soft("inhibit-variable-watchers") {
            if self.bound_p(inh) && self.symbol_value(inh).truthy() {
                return Ok(());
            }
        }
        let where_ = buf.and_then(|b| self.buffer_value(b)).unwrap_or(Value::Nil);
        // OP is a C-style literal (`Qset', `Qlet', ...), not a request to
        // intern in the current public namespace. Preserve the canonical
        // symbol cell if Lisp has removed its name from the obarray.
        let op_id = self.q(op);
        let op_sym = self.sym(op_id);
        let mut cursor = 0;
        loop {
            // Keep the next live list cell, not a snapshot of callbacks.  A
            // callback may remove later entries; insertions at the list head
            // do not restart the walk, matching GNU's list traversal.
            let selected = self
                .var_watchers
                .iter()
                .enumerate()
                .skip(cursor)
                .find(|(_, (symbol, _))| *symbol == id)
                .map(|(index, (_, watcher))| (index, watcher.clone()));
            let Some((index, f)) = selected else { break };
            let following: Vec<Value> = self.var_watchers[index + 1..]
                .iter()
                .filter(|(symbol, _)| *symbol == id)
                .map(|(_, watcher)| watcher.clone())
                .collect();
            self.apply(
                &f,
                vec![self.sym(id), newval.clone(), op_sym.clone(), where_.clone()],
            )?;
            let mut current_index = None;
            for (index, (symbol, watcher)) in self.var_watchers.iter().enumerate() {
                if *symbol == id && crate::lisp::builtins::equal_values(self, watcher, &f)? {
                    current_index = Some(index);
                    break;
                }
            }
            let start = current_index.map_or(0, |index| index + 1);
            let mut next = None;
            'following: for expected in following {
                for (index, (symbol, watcher)) in self.var_watchers.iter().enumerate().skip(start) {
                    if *symbol == id
                        && crate::lisp::builtins::equal_values(self, watcher, &expected)?
                    {
                        next = Some(index);
                        break 'following;
                    }
                }
            }
            let Some(next) = next else { break };
            cursor = next;
        }
        Ok(())
    }
}

#[cfg(test)]
mod dumped_variable_tests {
    use super::*;
    use crate::lisp::obarray::Obarray;

    fn circular_plist(items: Vec<Value>) -> Value {
        let plist = Value::list(items);
        let mut tail = plist.clone();
        while let Value::Cons(cell) = tail {
            let next = cell.borrow().cdr.clone();
            if next.is_nil() {
                cell.borrow_mut().cdr = plist.clone();
                break;
            }
            tail = next;
        }
        plist
    }

    #[test]
    fn dump_variable_lookup_handles_missing_and_present_circular_properties() {
        let mut interp = Interp::vacant();
        interp.obarray = Obarray::new();
        let dump_var = interp.intern("remacs--dump-var");
        let unrelated = interp.intern("qa-unrelated-dump-property");

        let missing = interp.intern("qa-missing-dump-property");
        interp.obarray.symbol_mut(missing).value = Value::Sym(sym::UNBOUND);
        interp.obarray.symbol_mut(missing).plist =
            circular_plist(vec![Value::Sym(unrelated), Value::Int(1)]);
        assert!(matches!(
            interp.symbol_value(missing),
            Value::Sym(symbol) if symbol == sym::UNBOUND
        ));

        let present = interp.intern("qa-present-dump-property");
        interp.obarray.symbol_mut(present).value = Value::Sym(sym::UNBOUND);
        interp.obarray.symbol_mut(present).plist = circular_plist(vec![
            Value::Sym(dump_var),
            Value::Int(42),
            Value::Sym(unrelated),
            Value::Int(1),
        ]);
        assert!(matches!(interp.symbol_value(present), Value::Int(42)));
    }
}

#[cfg(test)]
mod quit_flag_identity_tests {
    use super::*;
    use crate::lisp::obarray::Obarray;

    #[test]
    fn pending_quit_only_affects_the_interned_quit_flag() {
        let mut interp = Interp::vacant();
        interp.obarray = Obarray::new();
        let quit_flag = interp.intern("quit-flag");
        let uninterned = interp.make_symbol("quit-flag");
        interp.obarray.symbol_mut(uninterned).value = Value::Int(17);
        interp.quit_flag = true;

        assert!(matches!(interp.symbol_value(uninterned), Value::Int(17)));
        interp.synchronize_quit_symbol(uninterned);
        assert!(interp.quit_flag);
        assert!(matches!(interp.symbol_value(quit_flag), Value::Sym(id) if id == sym::T));

        interp.synchronize_quit_symbol(quit_flag);
        assert!(!interp.quit_flag);
        assert!(matches!(interp.symbol_value(quit_flag), Value::Sym(id) if id == sym::T));
    }

    #[test]
    fn pending_quit_tracks_the_canonical_symbol_after_uninterning() {
        let mut interp = Interp::vacant();
        interp.obarray = Obarray::new();
        let quit_flag = interp.q("quit-flag");
        assert!(interp.obarray.unintern_symbol(quit_flag));

        interp.quit_flag = true;
        assert!(matches!(interp.symbol_value(quit_flag), Value::Sym(id) if id == sym::T));

        interp.synchronize_quit_symbol(quit_flag);
        assert!(!interp.quit_flag);
        assert!(matches!(interp.symbol_value(quit_flag), Value::Sym(id) if id == sym::T));
    }
}

#[cfg(test)]
mod lazy_fixed_symbol_identity_tests {
    use super::*;
    use crate::lisp::obarray::Obarray;

    #[test]
    fn first_fixed_lookup_recovers_the_original_interned_symbol() {
        let mut interp = Interp::vacant();
        interp.obarray = Obarray::new();
        let name = "qa-lazy-native-q-symbol";
        let same_name_uninterned = interp.make_symbol(name);
        let original = interp.intern(name);
        assert!(interp.obarray.unintern_symbol(original));
        let replacement = interp.intern(name);

        let fixed = interp.q(name);

        assert_ne!(same_name_uninterned, original);
        assert_ne!(original, replacement);
        assert_eq!(fixed, original);
        assert_eq!(interp.q(name), original);
    }

    #[test]
    fn first_fixed_lookup_recovers_a_symbol_after_namespace_clear() {
        let mut interp = Interp::vacant();
        interp.obarray = Obarray::new();
        let name = "qa-cleared-native-q-symbol";
        let original = interp.intern(name);
        interp.obarray.clear_namespace();

        assert_eq!(interp.q(name), original);
    }

    #[test]
    fn first_fixed_soft_lookup_recovers_the_original_after_unintern() {
        let mut interp = Interp::vacant();
        interp.obarray = Obarray::new();
        let name = "qa-lazy-native-q-soft-symbol";
        let original = interp.intern(name);
        assert!(interp.obarray.unintern_symbol(original));
        let replacement = interp.intern(name);

        assert_ne!(original, replacement);
        assert_eq!(interp.q_soft(name), Some(original));
    }
}
