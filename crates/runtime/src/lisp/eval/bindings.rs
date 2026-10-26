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
                        .collect::<String>(),
                    std::panic::Location::caller()
                );
                for (f, a) in self.lisp_stack.iter().rev().take(20).rev() {
                    eprintln!(
                        "  {} {}",
                        self.princ_to_string(f)
                            .chars()
                            .take(100)
                            .collect::<String>(),
                        self.princ_to_string(&Value::list(a.as_slice().to_vec()))
                            .chars()
                            .take(160)
                            .collect::<String>()
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
        // The current buffer's RefCell may already be borrowed when
        // specbind runs inside eval-driven macroexpansion (e.g. a
        // `with-temp-buffer' wrapper holding the buffer).  GNU binds
        // the local slot; when we can't borrow we must not panic —
        // fall back to the global binding for the duration.
        let local_here = always_local
            || self
                .buffers
                .get(self.current_buffer)
                .and_then(|b| b.try_borrow().ok().map(|bb| bb.locals.contains_key(&id)))
                .unwrap_or(false);
        if can_local && !local_here {
            // GNU SPECPDL_LET_DEFAULT: bind the default cell, retaining
            // the origin buffer so setq there updates this binding.
            // Push before notifying watchers, which may change values.
            let old = self.obarray.symbol(id).value.clone();
            self.specbind.push(SpecBind {
                sym: id,
                buf: None,
                default_buf: Some(self.current_buffer),
                old: Some(old),
            });
            if forwarded {
                return self.set_symbol_default(id, val);
            }
            self.fire_var_watchers(id, &val, "let", Some(self.current_buffer))?;
            self.obarray.symbol_mut(id).value = val.clone();
            self.sync_undo_inhibit(id, &val);
            return Ok(());
        }
        // GNU saves the plain value before notifying watchers: they may
        // write the same variable or signal while the binding is installed.
        let saved_global = !local_here;
        if saved_global {
            self.specbind.push(SpecBind {
                sym: id,
                buf: None,
                default_buf: None,
                old: Some(self.obarray.symbol(id).value.clone()),
            });
        }
        self.fire_var_watchers(id, &val, "let", local_here.then_some(self.current_buffer))?;
        if local_here {
            if let Some(b) = self.buffers.get(self.current_buffer) {
                if let Ok(mut bb) = b.try_borrow_mut() {
                    let old = bb.locals.insert(id, val.clone());
                    if id == self.bfn_sym {
                        // BVAR: the buffer slot is the binding.
                        bb.file_name = match &val {
                            Value::Str(s) => Some(s.borrow().clone()),
                            _ => None,
                        };
                    }
                    drop(bb);
                    self.specbind.push(SpecBind {
                        sym: id,
                        buf: Some(self.current_buffer),
                        default_buf: None,
                        old,
                    });
                    bound_buf = Some(self.current_buffer);
                }
            }
        }
        if bound_buf.is_none() {
            if !saved_global {
                self.specbind.push(SpecBind {
                    sym: id,
                    buf: None,
                    default_buf: None,
                    old: Some(self.obarray.symbol(id).value.clone()),
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
        for _ in 0..n {
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
            if let Some(buffer_id) = sb.buf {
                let still_local = self.buffers.get(buffer_id).is_some_and(|buffer| {
                    self.obarray.symbol(sb.sym).always_local
                        || buffer.borrow().locals.contains_key(&sb.sym)
                });
                if !still_local {
                    continue;
                }
            }
            let restored = sb.old.clone().unwrap_or(Value::Nil);
            // GNU notifies before storing the restored value. A global
            // forwarded variable goes through set_default_internal and
            // set_internal, producing set followed by unlet notifications.
            if sb.buf.is_none() && self.obarray.symbol(sb.sym).forwarded_variable {
                self.fire_var_watchers(sb.sym, &restored, "set", None)?;
            }
            self.fire_var_watchers(sb.sym, &restored, "unlet", sb.buf)?;
            match sb.buf {
                Some(buf_id) => {
                    if let Some(b) = self.buffers.get(buf_id) {
                        let mut bb = b.borrow_mut();
                        match sb.old {
                            Some(v) => bb.locals.insert(sb.sym, v),
                            None => bb.locals.shift_remove(&sb.sym),
                        };
                        if sb.sym == self.bfn_sym {
                            bb.file_name = match bb.locals.get(&sb.sym) {
                                Some(Value::Str(s)) => Some(s.borrow().clone()),
                                _ => None,
                            };
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
            if let Err(flow) = self.unbind_bindings(1) {
                // GNU's replacement exit continues unwinding the remaining
                // bindings. Offer it before another unlet callback can run.
                result = self.offer_flow(Err(flow)).map(|_| ());
            }
        }
        result
    }
}
