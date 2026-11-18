//! Dynamic binding stack installation, toplevel access, and restoration.
//!
//! Watchers can mutate state or signal while bindings are installed or
//! removed. Preserve notification order and continue unwinding on replacement exits.

use super::Interp;
use crate::lisp::{
    error::Flow,
    obarray::sym,
    value::{SymId, Value},
};

/// A saved dynamic binding (specbind entry).
pub(super) struct SpecBind {
    pub(super) sym: SymId,
    /// `Some(buf)` = binding is buffer-local in `buf`; `None` = global cell.
    pub(super) buf: Option<usize>,
    /// Origin of a buffer-local-capable variable's default binding.
    /// Only assignments made in this buffer suppress automatic locality.
    default_buf: Option<usize>,
    /// Previous value (`None` = no local binding existed).
    pub(super) old: Option<Value>,
}

impl Interp {
    // ---------- specbind (dynamic let) ----------

    /// Push a dynamic binding for `sym` to `val`.
    #[track_caller]
    pub fn specbind(&mut self, id: SymId, val: Value) -> Result<(), Flow> {
        let mark = self.specbind_depth();
        let result = match self.specbind_position_inner(id, val) {
            Ok(()) => Ok(()),
            Err(error) => self.unbind_to(mark).and(Err(error)),
        };
        self.sync_symbol_position_view();
        result
    }

    #[track_caller]
    fn specbind_position_inner(&mut self, id: SymId, val: Value) -> Result<(), Flow> {
        // GNU specbind follows variable indirection before saving or
        // replacing the value cell, including a base's buffer-local slot.
        // Binding the alias's unused cell is invisible to symbol_value.
        let id = self.var_alias_target(id);
        self.synchronize_quit_symbol(id);
        if self.obarray.symbol(id).constant {
            if self.dbg_flags & 0x100 != 0 {
                eprintln!(
                    "[bindconst {}] head_form={} caller={:?}",
                    self.symbol_name(id),
                    self.princ_to_string(&self.cur_head_form)
                        .chars()
                        .take(400)
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
            return Err(self.signal_data(sym::SETTING_CONSTANT, vec![self.sym(id)]));
        }
        let sym = self.obarray.symbol(id);
        let can_local = sym.make_local_if_set || sym.always_local;
        let always_local = sym.always_local;
        let forwarded = sym.forwarded_variable;
        let mut bound_buf = None;
        let binding_buffer = self.current_buffer;
        // The current buffer's RefCell may already be borrowed when
        // specbind runs inside eval-driven macroexpansion (e.g. a
        // `with-temp-buffer' wrapper holding the buffer).  GNU binds
        // the local slot; when we can't borrow we must not panic —
        // fall back to the global binding for the duration.
        let local_binding = self.buffers.get(binding_buffer).and_then(|b| {
            b.try_borrow().ok().and_then(|bb| {
                if always_local || bb.locals.contains_key(&id) {
                    Some(bb.locals.get(&id).cloned())
                } else {
                    None
                }
            })
        });
        let local_here = local_binding.is_some()
            && self
                .buffers
                .get(binding_buffer)
                .is_some_and(|b| b.try_borrow_mut().is_ok());
        let local_old = local_binding.flatten();
        if can_local && !local_here {
            // GNU SPECPDL_LET_DEFAULT: bind the default cell, retaining
            // the origin buffer so setq there updates this binding.
            // Push before notifying watchers, which may change values.
            let old = self.obarray.symbol(id).value.clone();
            self.specbind.push(SpecBind {
                sym: id,
                buf: None,
                default_buf: Some(binding_buffer),
                old: Some(old),
            });
            if forwarded {
                return self.set_symbol_default(id, val);
            }
            self.fire_var_watchers(id, &val, "let", Some(binding_buffer))?;
            self.validate_c_integer_forwarding(id, &val)?;
            self.obarray.symbol_mut(id).value = val.clone();
            self.sync_undo_inhibit(id, &val);
            return Ok(());
        }
        // GNU saves the plain value before notifying watchers: they may
        // write the same variable or signal while the binding is installed.
        let saved_global = !local_here;
        let saved_global_value = self.obarray.symbol(id).value.clone();
        if saved_global {
            self.specbind.push(SpecBind {
                sym: id,
                buf: None,
                default_buf: None,
                old: Some(saved_global_value.clone()),
            });
        } else {
            // Save the local value before invoking watchers.  A `let'
            // watcher may assign to the same variable; unwinding must
            // still restore the value from before the binding began.
            self.specbind.push(SpecBind {
                sym: id,
                buf: Some(binding_buffer),
                default_buf: None,
                old: local_old,
            });
        }
        self.fire_var_watchers(id, &val, "let", local_here.then_some(binding_buffer))?;
        self.validate_c_integer_forwarding(id, &val)?;
        // A variable watcher may hold a Rust mutable borrow of the string it
        // received. Snapshot BVAR's cached filename before writing either
        // slot so the conflict becomes a Lisp error and `specbind' can unwind
        // its reserved record instead of panicking in `RefCell::borrow'.
        let file_name = if local_here && id == self.bfn_sym {
            match &val {
                Value::Str(s) => Some(
                    s.try_borrow()
                        .map(|text| (*text).clone())
                        .map_err(|_| {
                            self.error("Cannot inspect a mutably borrowed buffer-file-name string")
                        })?,
                ),
                _ => None,
            }
        } else {
            None
        };
        if local_here {
            if let Some(b) = self.buffers.get(self.current_buffer) {
                if let Ok(mut bb) = b.try_borrow_mut() {
                    bb.locals.insert(id, val.clone());
                    if id == self.bfn_sym {
                        // BVAR: the buffer slot is the binding.
                        bb.file_name = file_name;
                    }
                    drop(bb);
                    bound_buf = Some(self.current_buffer);
                }
            }
        }
        if bound_buf.is_none() {
            if !saved_global {
                // The buffer slot became unavailable after its watcher
                // ran.  Replace the reserved local unwind record with the
                // global-cell fallback used by this path.
                let local = self.specbind.pop();
                debug_assert!(matches!(local, Some(ref binding)
                    if binding.sym == id && binding.buf == Some(binding_buffer)));
                self.specbind.push(SpecBind {
                    sym: id,
                    buf: None,
                    default_buf: None,
                    old: Some(saved_global_value),
                });
            }
            self.obarray.symbol_mut(id).value = val.clone();
            self.sync_undo_inhibit(id, &val);
        }
        Ok(())
    }

    /// GNU `let_shadows_buffer_binding_p': true when a `let' bound the
    /// DEFAULT cell of a buffer-local-capable variable in this buffer.
    /// A binding made in another buffer does not prevent a new local
    /// assignment here. Plain global and local bindings do not count.
    pub fn specbind_shadows_current(&self, id: SymId) -> bool {
        self.specbind
            .iter()
            .any(|sb| sb.sym == id && sb.default_buf == Some(self.current_buffer))
    }

    /// GNU `specpdl' walk for `defvaralias': any live `let' entry for
    /// `id` (global or buffer-local) makes it let-bound.
    pub fn specbind_for(&self, id: SymId) -> bool {
        self.specbind.iter().any(|sb| sb.sym == id)
    }

    /// Pop `n` specbind entries, restoring values.
    pub fn unbind(&mut self, n: usize) -> Result<(), Flow> {
        self.unbind_to(self.specbind.len().saturating_sub(n))
    }

    pub(crate) fn unbind_bindings(&mut self, n: usize) -> Result<(), Flow> {
        let result = self.unbind_bindings_position_inner(n);
        self.sync_symbol_position_view();
        result
    }

    #[track_caller]
    fn unbind_bindings_position_inner(&mut self, n: usize) -> Result<(), Flow> {
        for _ in 0..n {
            // Check a buffer-local destination before popping its unwind
            // record or notifying watchers. A Rust caller may still hold a
            // buffer borrow while asking the interpreter to unwind; `borrow'
            // here used to panic, and dropping the record first made a later
            // retry impossible.
            let local_binding_live = match self.specbind.last() {
                Some(binding) => match binding.buf {
                    Some(buffer_id) => match self.buffers.get(buffer_id) {
                        Some(buffer) => {
                            let always_local = self.obarray.symbol(binding.sym).always_local;
                            let state = buffer.try_borrow_mut().map_err(|_| {
                                self.error(
                                    "Cannot unwind a buffer-local binding while its buffer is borrowed",
                                )
                            })?;
                            Some(always_local || state.locals.contains_key(&binding.sym))
                        }
                        None => Some(false),
                    },
                    None => None,
                },
                None => return Ok(()),
            };
            let Some(sb) = self.specbind.pop() else {
                return Ok(());
            };
            self.synchronize_quit_symbol(sb.sym);
            if sb.default_buf.is_some() {
                // GNU restores a default through set_default_internal,
                // notifying before the old value is written back.
                if let Some(value) = sb.old {
                    self.set_symbol_default(sb.sym, value)?;
                }
                continue;
            }
            // GNU restores LET_LOCAL only while that original buffer's
            // binding still exists. A killed binding must not be recreated,
            // nor receive an unlet watcher notification.
            if local_binding_live == Some(false) {
                continue;
            }
            let restored = sb.old.clone().unwrap_or(Value::Nil);
            // GNU notifies before storing the restored value. A global
            // forwarded variable goes through set_default_internal and
            // set_internal, producing set followed by unlet notifications.
            if sb.buf.is_none() && self.obarray.symbol(sb.sym).forwarded_variable {
                self.fire_var_watchers(sb.sym, &restored, "set", None)?;
            }
            self.fire_var_watchers(sb.sym, &restored, "unlet", sb.buf)?;
            let file_name_snapshot = match sb.buf {
                Some(buf_id) if sb.sym == self.bfn_sym && self.buffers.get(buf_id).is_some() => {
                    match sb.old.as_ref() {
                        Some(Value::Str(s)) => Some(s.try_borrow().map(|text| (*text).clone())),
                        _ => None,
                    }
                }
                _ => None,
            };
            let restored_file_name = match file_name_snapshot {
                Some(Ok(file_name)) => Some(file_name),
                Some(Err(_)) => {
                    self.specbind.push(sb);
                    return Err(self.error(
                        "Cannot inspect a mutably borrowed buffer-file-name string",
                    ));
                }
                None => None,
            };
            match sb.buf {
                Some(buf_id) => {
                    if let Some(b) = self.buffers.get(buf_id) {
                        let mut bb = match b.try_borrow_mut() {
                            Ok(buffer) => buffer,
                            Err(_) => {
                                // An unlet watcher can acquire a Rust-side
                                // borrow after the pre-pop check above. Keep
                                // the unwind record so a caller can retry;
                                // never panic or silently lose the binding.
                                self.specbind.push(sb);
                                return Err(self.error(
                                    "Cannot unwind a buffer-local binding while its buffer is borrowed",
                                ));
                            }
                        };
                        match sb.old {
                            Some(v) => bb.locals.insert(sb.sym, v),
                            None => bb.locals.shift_remove(&sb.sym),
                        };
                        if sb.sym == self.bfn_sym {
                            bb.file_name = restored_file_name;
                        }
                    }
                }
                None => {
                    if let Some(v) = sb.old {
                        self.obarray.symbol_mut(sb.sym).value = v.clone();
                        self.sync_undo_inhibit(sb.sym, &v);
                    }
                }
            }
        }
        Ok(())
    }

    pub fn specbind_depth(&self) -> usize {
        self.specbind.len()
    }

    /// `default-toplevel-value': the global binding ignoring `let' frames.
    /// The outermost specbind frame for the symbol saved the toplevel value.
    /// Returns None when the symbol's default is void.
    pub fn default_toplevel_value(&self, id: SymId) -> Option<Value> {
        for sb in &self.specbind {
            if sb.sym == id && sb.buf.is_none() {
                return sb.old.clone();
            }
        }
        match &self.obarray.symbol(id).value {
            Value::Sym(s) if *s == sym::UNBOUND => None,
            v => Some(v.clone()),
        }
    }

    /// GNU `default_toplevel_binding': the first (outermost) global
    /// specbind entry for `id`.  Used by `defvar' to detect a `let'
    /// shadowing a still-void default binding.
    pub fn specbind_outermost_unbound(&self, id: SymId) -> bool {
        self.specbind.iter().any(|sb| {
            sb.sym == id
                && sb.buf.is_none()
                && matches!(&sb.old, Some(Value::Sym(s)) if *s == sym::UNBOUND)
        })
    }

    /// GNU `set_specpdl_old_value' on the outermost global specbind
    /// for `id` — installing a value that surfaces as the default when
    /// the shadowing `let' unwinds.
    pub fn set_specbind_outermost_old(&mut self, id: SymId, val: Value) {
        if let Some(sb) = self
            .specbind
            .iter_mut()
            .find(|sb| sb.sym == id && sb.buf.is_none())
        {
            sb.old = Some(val);
        }
    }

    /// `buffer-local-toplevel-value': the toplevel buffer-local binding in
    /// `buf`, ignoring `let' frames. None when no toplevel local exists.
    pub fn buffer_local_toplevel_value(&self, id: SymId, buf: usize) -> Option<Value> {
        for sb in &self.specbind {
            if sb.sym == id && sb.buf == Some(buf) {
                return sb
                    .old
                    .clone()
                    .filter(|v| !matches!(v, Value::Sym(s) if *s == sym::UNBOUND));
            }
        }
        let v = self
            .buffers
            .get(buf)
            .and_then(|b| b.try_borrow().ok().map(|bb| bb.locals.get(&id).cloned()))
            .flatten();
        v.filter(|v| !matches!(v, Value::Sym(s) if *s == sym::UNBOUND))
    }

    /// `set-buffer-local-toplevel-value': set the toplevel buffer-local
    /// binding in `buf` without disturbing in-flight `let' bindings.
    pub fn set_buffer_local_toplevel_value(&mut self, id: SymId, buf: usize, val: Value) {
        for sb in &mut self.specbind {
            if sb.sym == id && sb.buf == Some(buf) {
                sb.old = Some(val);
                return;
            }
        }
        if let Some(b) = self.buffers.get(buf) {
            if let Ok(mut bb) = b.try_borrow_mut() {
                bb.locals.insert(id, val);
            }
        }
    }

    pub fn unbind_to(&mut self, mark: usize) -> Result<(), Flow> {
        self.with_quit_preserved(|i| i.unbind_raw_to(mark))
    }

    pub(crate) fn unbind_raw_to(&mut self, mark: usize) -> Result<(), Flow> {
        let mut result = Ok(());
        while self.specbind.len() > mark {
            let depth = self.specbind.len();
            if let Err(flow) = self.unbind_bindings(1) {
                // A buffer borrow can prevent an unwind record from being
                // popped. Stop here so the caller can release the borrow and
                // retry instead of spinning forever at the same depth.
                if self.specbind.len() >= depth {
                    return Err(flow);
                }
                // GNU's replacement exit continues unwinding the remaining
                // bindings. Offer it before another unlet callback can run.
                result = self.offer_flow(Err(flow)).map(|_| ());
            }
        }
        result
    }
}

#[cfg(test)]
mod failed_specbind_tests {
    use super::*;

    #[test]
    fn failed_watcher_binding_restores_its_specpdl_entry() {
        std::thread::Builder::new()
            .stack_size(64 * 1024 * 1024)
            .spawn(|| {
                let mut interp = Interp::new();
                interp
                    .eval_str(
                        "(defvar qa-failed-specbind-events nil)\
                         (defun qa-failed-specbind-watcher (_symbol _value operation _where)\
                           (push operation qa-failed-specbind-events)\
                           (if (eq operation 'let) (signal 'file-error '(\"bind\")) nil))\
                         (add-variable-watcher 'qa-failed-specbind-variable\
                                               #'qa-failed-specbind-watcher)",
                    )
                    .unwrap();

                let symbol = interp.q("qa-failed-specbind-variable");
                let old_value = interp.symbol_value(symbol);
                let was_bound = interp.bound_p(symbol);
                let mark = interp.specbind_depth();
                let result = interp.specbind(symbol, Value::Int(17));
                assert!(matches!(
                    result,
                    Err(Flow::Signal(ref signal)) if signal.is_symbol(sym::FILE_ERROR)
                ));
                assert_eq!(interp.specbind_depth(), mark);
                assert_eq!(interp.bound_p(symbol), was_bound);
                assert_eq!(
                    interp.prin1_to_string(&interp.symbol_value(symbol)),
                    interp.prin1_to_string(&old_value)
                );
                let events_symbol = interp.q("qa-failed-specbind-events");
                let events = interp.symbol_value(events_symbol).list_to_vec().unwrap();
                let unlet = interp.q("unlet");
                let let_symbol = interp.q("let");
                assert_eq!(events.len(), 2);
                assert_eq!(interp.sym_id(&events[0]), Some(unlet));
                assert_eq!(interp.sym_id(&events[1]), Some(let_symbol));
            })
            .unwrap()
            .join()
            .unwrap();
    }

    #[test]
    fn borrowed_always_local_slot_falls_back_without_panicking_on_cleanup() {
        std::thread::Builder::new()
            .stack_size(64 * 1024 * 1024)
            .spawn(|| {
                let mut interp = Interp::new();
                interp
                    .eval_str(
                        "(defun qa-borrowed-specbind-watcher\
                               (_symbol _value operation _where)\
                           (when (eq operation 'let)\
                             (signal 'file-error '(\"bind\"))))\
                         (add-variable-watcher 'qa-borrowed-specbind-variable\
                                               #'qa-borrowed-specbind-watcher)",
                    )
                    .unwrap();
                let symbol = interp.q("qa-borrowed-specbind-variable");
                interp.obarray.symbol_mut(symbol).always_local = true;
                let buffer = interp
                    .buffers
                    .get(interp.current_buffer)
                    .expect("current buffer")
                    .clone();
                let borrow = buffer.borrow_mut();
                let mark = interp.specbind_depth();
                let result = interp.specbind(symbol, Value::Int(17));
                drop(borrow);

                assert!(matches!(
                    result,
                    Err(Flow::Signal(ref signal)) if signal.is_symbol(sym::FILE_ERROR)
                ));
                assert_eq!(interp.specbind_depth(), mark);
            })
            .unwrap()
            .join()
            .unwrap();
    }

    #[test]
    fn buffer_switch_during_let_keeps_the_reserved_local_record_identity() {
        std::thread::Builder::new()
            .stack_size(64 * 1024 * 1024)
            .spawn(|| {
                let mut interp = Interp::new();
                interp
                    .eval_str(
                        "(defvar qa-buffer-switch-target nil)\
                         (defvar qa-buffer-switch-variable nil)\
                         (defun qa-buffer-switch-watcher\
                                (_symbol _value operation _where)\
                           (when (eq operation 'let)\
                             (set-buffer qa-buffer-switch-target)))\
                         (add-variable-watcher 'qa-buffer-switch-variable\
                                               #'qa-buffer-switch-watcher)",
                    )
                    .unwrap();
                let variable = interp.q("qa-buffer-switch-variable");
                let target = interp.q("qa-buffer-switch-target");
                interp.set_symbol_default(variable, Value::Int(0)).unwrap();
                let first = interp.current_buffer;
                let second = interp.create_buffer(" *qa-buffer-switch-second*");
                let second_value =
                    Value::Buffer(interp.buffers.get(second).expect("second buffer").clone());
                interp.set_symbol(target, second_value).unwrap();
                interp
                    .buffers
                    .get(first)
                    .unwrap()
                    .borrow_mut()
                    .locals
                    .insert(variable, Value::Int(1));

                let second_buffer = interp.buffers.get(second).unwrap().clone();
                let held_second = second_buffer.borrow();
                let mark = interp.specbind_depth();
                let result = interp.specbind(variable, Value::Int(17));
                assert!(result.is_ok(), "unexpected specbind result: {result:?}");
                assert_eq!(interp.current_buffer, second);
                assert!(matches!(interp.symbol_value(variable), Value::Int(17)));
                drop(held_second);

                interp.unbind_to(mark).unwrap();
                assert_eq!(interp.specbind_depth(), mark);
                assert!(matches!(interp.symbol_value(variable), Value::Int(0)));
                assert!(matches!(
                    interp
                        .buffers
                        .get(first)
                        .unwrap()
                        .borrow()
                        .locals
                        .get(&variable),
                    Some(Value::Int(1))
                ));
            })
            .unwrap()
            .join()
            .unwrap();
    }
}
