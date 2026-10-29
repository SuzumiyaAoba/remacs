//! Symbol identity, variable cells, aliases, properties, and watcher notifications.

use super::{Interp, plist_get, plist_put};
use crate::lisp::{
    error::Flow,
    obarray::sym,
    value::{SymId, Value},
};

impl Interp {
    pub fn intern<T: crate::lisp::text::TextSource + ?Sized>(&mut self, name: &T) -> SymId {
        self.obarray
            .intern_with_key(name, crate::lisp::obarray::InternKey::from_text(name))
    }

    pub(crate) fn q(&mut self, name: &str) -> SymId {
        if let Some(&symbol) = self.fixed_symbols.get(name) {
            return symbol;
        }
        let symbol = self.obarray.intern(name);
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
            .map(|text| text.borrow().clone())
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
                    // was assembled through the `variable-alias' property,
                    // indirect-variable returns its original argument.
                    if !seen.insert(cur) {
                        return id;
                    }
                    cur = next;
                }
                None => return cur,
            }
        }
    }

    pub fn symbol_value(&self, id: SymId) -> Value {
        let id = self.var_alias_target(id);
        // Buffer-local binding in current buffer wins. `try_borrow`:
        // primitives that hold the buffer mutably borrowed may still
        // consult variables (they see the global binding).
        if id == u32::MAX {
            return Value::Nil; // intern_soft miss — unbound is nil
        }
        if self.quit_flag && self.symbol_name(id) == "quit-flag" {
            return Value::t();
        }
        if let Some(b) = self.buffers.get(self.current_buffer) {
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
        // cache starts nil): test plist membership, not truthiness.
        let mut cur = self.obarray.symbol(id).plist.clone();
        while let Value::Cons(c) = cur {
            let (car, next) = {
                let b = c.borrow();
                (b.car.clone(), b.cdr.clone())
            };
            if matches!(car, Value::Sym(s) if s == prop) {
                return match &next {
                    Value::Cons(c2) => Some(c2.borrow().car.clone()),
                    _ => Some(Value::Nil),
                };
            }
            cur = match next {
                Value::Cons(c2) => c2.borrow().cdr.clone(),
                _ => break,
            };
        }
        None
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
        let result = self.set_symbol_position_inner(id, val);
        self.sync_symbol_position_view();
        result
    }

    #[track_caller]
    fn set_symbol_position_inner(&mut self, id: SymId, val: Value) -> Result<(), Flow> {
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
            return Err(self.signal_data(sym::SETTING_CONSTANT, vec![self.sym(id)]));
        }
        if constant {
            // nil/t can't be set at all.
            return Err(self.signal_data(sym::SETTING_CONSTANT, vec![self.sym(id)]));
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
        self.fire_var_watchers(
            id,
            &val,
            "set",
            (local || default_shadow).then_some(self.current_buffer),
        )?;
        if default_shadow && forwarded && !always_local {
            // GNU's forwarded per-buffer path also writes the default
            // cell, notifying its watchers separately with WHERE=nil.
            return self.set_symbol_default(id, val);
        }
        if local {
            if let Some(b) = self.buffers.get(self.current_buffer) {
                if let Ok(mut bb) = b.try_borrow_mut() {
                    bb.locals.insert(id, val.clone());
                    if id == self.bfn_sym {
                        // Mirror the BVAR write into the buffer slot.
                        bb.file_name = match &val {
                            Value::Str(s) => Some(s.borrow().clone()),
                            _ => None,
                        };
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
        let result = self.set_symbol_default_position_inner(id, val);
        self.sync_symbol_position_view();
        result
    }

    #[track_caller]
    fn set_symbol_default_position_inner(&mut self, id: SymId, val: Value) -> Result<(), Flow> {
        // GNU `Fset_default' walks the variable-alias chain first:
        // `(set-default 'alias v)' writes BASE's default cell, leaving
        // the alias's own cell a legacy remnant readers ignore.
        let id = self.var_alias_target(id);
        self.synchronize_quit_symbol(id);
        if self.obarray.symbol(id).constant {
            return Err(self.signal_data(sym::SETTING_CONSTANT, vec![self.sym(id)]));
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
        self.fire_var_watchers(id, &val, "set", None)?;
        self.obarray.symbol_mut(id).value = val.clone();
        self.sync_undo_inhibit(id, &val);
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
        if prop == self.var_alias_prop {
            self.obarray.symbol_mut(id).var_alias = match val {
                Value::Sym(s) => Some(s),
                _ => None,
            };
        }
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
        let watchers: Vec<Value> = self
            .var_watchers
            .iter()
            .filter(|(s, _)| *s == id)
            .map(|(_, f)| f.clone())
            .collect();
        let where_ = buf.and_then(|b| self.buffer_value(b)).unwrap_or(Value::Nil);
        let op_id = self.intern(op);
        let op_sym = self.sym(op_id);
        for f in watchers {
            self.apply(
                &f,
                vec![self.sym(id), newval.clone(), op_sym.clone(), where_.clone()],
            )?;
        }
        Ok(())
    }
}
