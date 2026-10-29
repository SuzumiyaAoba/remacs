//! Evaluation depth, signal delivery, debugger headroom, and quit unwinding.

use super::Interp;
use crate::lisp::{
    error::{EvalResult, Flow},
    obarray::sym,
    value::{SymId, Value},
};

impl Interp {
    /// Connect an asynchronous frontend quit source to this interpreter.
    pub fn set_quit_request(
        &mut self,
        request: Option<std::sync::Arc<crate::frontend::QuitRequest>>,
    ) {
        self.quit_request = request;
    }

    /// eval_sub/Ffuncall and Bcall share the counter, with different GNU
    /// error descriptors. The failed entry remains live during signaling.
    pub(crate) fn with_lisp_eval_depth(
        &mut self,
        bytecode_call: bool,
        run: impl FnOnce(&mut Self) -> EvalResult,
    ) -> EvalResult {
        let mut scope = self.symbol_position_scope();
        let result = self.with_lisp_eval_depth_position_inner(bytecode_call, run);
        scope.finish(self.positioned_symbols_enabled());
        result
    }

    fn with_lisp_eval_depth_position_inner(
        &mut self,
        bytecode_call: bool,
        run: impl FnOnce(&mut Self) -> EvalResult,
    ) -> EvalResult {
        if let Some(status) = self.exit_status {
            return Err(Flow::Exit(status));
        }
        self.maybe_quit()?;
        let saved_depth = self.eval_depth;
        self.eval_depth += 1;
        // GNU consults the live limit on every entry, so a lower `let'
        // or `setq' must take effect before comparing against the cache.
        let vid = self.q("max-lisp-eval-depth");
        if let Value::Int(d) = self.symbol_value(vid) {
            self.max_lisp_eval_depth = d.clamp(0, usize::MAX as i128) as usize;
        }
        if self.eval_depth > self.max_lisp_eval_depth {
            // GNU raises undersized limits to 100 only when exceeded.
            // This internal adjustment does not invoke variable watchers.
            if self.max_lisp_eval_depth < 100 {
                self.max_lisp_eval_depth = 100;
                self.store_error_limit(vid, 100);
            }
            if self.eval_depth > self.max_lisp_eval_depth {
                let error = if bytecode_call {
                    self.error_q::<str>("Lisp nesting exceeds `max-lisp-eval-depth'", &[])
                } else {
                    let depth = Value::Int(self.eval_depth as i128);
                    let nsym = self.q("excessive-lisp-nesting");
                    self.signal_data(nsym, vec![depth])
                };
                let result = self.offer_flow(Err(error));
                self.eval_depth = saved_depth;
                return result;
            }
        }
        let result = run(self);
        let result = self.offer_flow(result);
        self.eval_depth = saved_depth;
        result
    }

    pub(crate) fn offer_flow(&mut self, result: EvalResult) -> EvalResult {
        if let Some(status) = self.exit_status {
            return Err(Flow::Exit(status));
        }
        match result {
            Err(Flow::Quit) => {
                let quit = self.signal_data(sym::QUIT, vec![]);
                self.offer_flow(Err(quit))
            }
            Err(Flow::Exit(status)) => {
                self.exit_status = Some(status);
                self.quit_editor = true;
                Err(Flow::Exit(status))
            }
            Err(Flow::Signal(mut signal)) if !signal.offered => {
                let symbol = signal.symbol();
                let data = signal.data();
                signal.offered = true;
                // Edebug's hook runs before condition lookup and before
                // either condition-case or handler-bind selects a handler.
                if !signal.out_of_memory {
                    if let Some(id) = self.q_soft("signal-hook-function") {
                        if self.bound_p(id) {
                            let hook = self.symbol_value(id);
                            if hook.truthy() {
                                self.with_error_room(20, |i| {
                                    i.apply(&hook, vec![symbol.clone(), data])
                                })?;
                                signal.conditions = Some(self.signal_conditions(&symbol));
                            }
                        }
                    }
                }
                if signal.conditions.is_none() {
                    signal.conditions = Some(self.signal_conditions(&symbol));
                }
                if self.sym_id(&symbol).is_none() {
                    let error = self.wrong_type_mut("symbolp", &symbol);
                    return self.offer_flow(Err(error));
                }
                if signal.conditions.as_ref().is_some_and(Value::is_nil) {
                    let error = self.signal_data(
                        sym::ERROR,
                        vec![Value::string("Invalid error symbol"), symbol],
                    );
                    return self.offer_flow(Err(error));
                }
                // Fsignal skips its immediate frame and then an `error'
                // wrapper. Retain pending special-form frames as GNU does.
                let frames = self.backtrace_snapshot();
                let mut callers = frames.iter().rev().skip(1);
                let mut caller = callers.next();
                if caller.is_some_and(|f| self.sym_id(&f.function) == Some(sym::ERROR)) {
                    caller = callers.next();
                }
                self.signaling_function = if signal.out_of_memory {
                    Value::Nil
                } else {
                    caller.map(|f| f.function.clone()).unwrap_or(Value::Nil)
                };
                match self.offer_signal(&mut signal) {
                    Ok(()) => {
                        if signal.handler.is_some() {
                            self.begin_quit_unwind();
                        }
                        Err(Flow::Signal(signal))
                    }
                    Err(f) => Err(f),
                }
            }
            other => other,
        }
    }

    /// Select a condition-case destination, offering the packet to inner
    /// handler-bind observers in dynamic order. The early signal hook has
    /// already run and the matching-list identity is cached. Observer
    /// nonlocal exits supersede the original signal.
    fn offer_signal(&mut self, signal: &mut crate::lisp::error::Signal) -> Result<(), Flow> {
        // GNU searches handlers in dynamic nesting order: an inner
        // matching condition-case stops propagation to outer handler-bind
        // observers, while observers inside that case run at raise time.
        let mut case_cursor = self.case_handlers.len();
        if !self.handler_bindings.is_empty() {
            let cond = signal.condition.clone();
            for idx in (0..self.handler_bindings.len()).rev() {
                let (conds, handler, case_depth) = self.handler_bindings[idx].clone();
                // Walk condition-case and handler-bind entries in their
                // actual dynamic order. Do not revisit an inner case after
                // an observer changes the condition list.
                while case_cursor > case_depth {
                    case_cursor -= 1;
                    if let Some(clause) =
                        self.case_handlers[case_cursor].matching_clause(self, signal)
                    {
                        signal.handler = Some(crate::lisp::error::SignalHandler {
                            frame: case_cursor,
                            clause,
                        });
                        return Ok(());
                    }
                }
                if signal.matches(self, &conds) {
                    // While a handler runs only strictly-outer bindings
                    // are visible — a handler's own signal can't
                    // re-enter it.
                    let tail = self.handler_bindings.split_off(idx);
                    let cases = self.case_handlers.split_off(case_depth);
                    let r = self.with_error_room(200, |i| i.apply(&handler, vec![cond.clone()]));
                    let r = self.offer_flow(r);
                    if r.is_err() {
                        self.rebase_quit_unwind();
                    }
                    self.case_handlers.extend(cases);
                    self.handler_bindings.extend(tail);
                    r?;
                }
            }
        }
        while case_cursor > 0 {
            case_cursor -= 1;
            if let Some(clause) = self.case_handlers[case_cursor].matching_clause(self, signal) {
                signal.handler = Some(crate::lisp::error::SignalHandler {
                    frame: case_cursor,
                    clause,
                });
                return Ok(());
            }
        }
        Ok(())
    }

    /// GNU max_ensure_room/restore_stack_limits. These C-global updates
    /// neither bind variables nor invoke their watchers, and restoration
    /// returns any depth changed by the callback to the remaining reserve.
    fn with_error_room(
        &mut self,
        room: i128,
        invoke: impl FnOnce(&mut Self) -> EvalResult,
    ) -> EvalResult {
        let max = self.q("max-lisp-eval-depth");
        let reserve = self.q("lisp-eval-depth-reserve");
        let (Value::Int(old_max), Value::Int(available)) =
            (self.symbol_value(max), self.symbol_value(reserve))
        else {
            return invoke(self);
        };
        let additional = (self.eval_depth as i128 + room - old_max).min(available);
        if additional <= 0 {
            return invoke(self);
        }
        self.store_error_limit(max, old_max + additional);
        self.store_error_limit(reserve, available - additional);
        self.max_lisp_eval_depth = (old_max + additional).clamp(0, usize::MAX as i128) as usize;
        let result = invoke(self);
        let result = self.offer_flow(result);
        if let (Value::Int(current_max), Value::Int(remaining)) =
            (self.symbol_value(max), self.symbol_value(reserve))
        {
            self.store_error_limit(reserve, remaining + current_max - old_max);
        }
        self.store_error_limit(max, old_max);
        self.max_lisp_eval_depth = old_max.clamp(0, usize::MAX as i128) as usize;
        result
    }

    fn store_error_limit(&mut self, symbol: SymId, value: i128) {
        self.store_internal_value(symbol, Value::Int(value));
    }

    pub(crate) fn store_internal_value(&mut self, symbol: SymId, value: Value) {
        let symbol = self.var_alias_target(symbol);
        if let Some(buffer) = self.buffers.get(self.current_buffer) {
            if let Ok(mut buffer) = buffer.try_borrow_mut() {
                if let Some(slot) = buffer.locals.get_mut(&symbol) {
                    *slot = value;
                    return;
                }
            }
        }
        self.obarray.symbol_mut(symbol).value = value;
    }

    pub(super) fn synchronize_quit_symbol(&mut self, symbol: SymId) {
        if self.quit_flag && self.symbol_name(symbol) == "quit-flag" {
            self.synchronize_quit_request();
        }
    }

    /// Move a host's pending request into GNU's shared Lisp value cell.
    pub(crate) fn synchronize_quit_request(&mut self) {
        if self.quit_flag {
            self.quit_flag = false;
            let symbol = self.q("quit-flag");
            self.store_internal_value(symbol, Value::t());
        }
    }

    pub(crate) fn clear_quit_flag(&mut self) {
        self.quit_flag = false;
        let symbol = self.q("quit-flag");
        self.store_internal_value(symbol, Value::Nil);
    }

    pub(crate) fn begin_quit_unwind(&mut self) {
        self.synchronize_quit_request();
        let quit = self.q("quit-flag");
        self.unwind_quit = Some(self.symbol_value(quit));
        self.clear_quit_flag();
    }

    pub(crate) fn finish_quit_unwind(&mut self) {
        if let Some(saved) = self.unwind_quit.take() {
            let quit = self.q("quit-flag");
            if !saved.is_nil() && self.symbol_value(quit).is_nil() {
                self.store_internal_value(quit, saved);
            }
        }
    }

    /// Each discarded catch/condition scope ends one C unbind_to region.
    pub(crate) fn rebase_quit_unwind(&mut self) {
        if self.unwind_quit.is_some() {
            self.finish_quit_unwind();
            self.begin_quit_unwind();
        }
    }

    pub(crate) fn with_quit_preserved<T>(
        &mut self,
        run: impl FnOnce(&mut Self) -> Result<T, Flow>,
    ) -> Result<T, Flow> {
        if self.unwind_quit.is_some() {
            return run(self);
        }
        self.begin_quit_unwind();
        let saved = self.unwind_quit.take().unwrap();
        match run(self) {
            Ok(value) => {
                let quit = self.q("quit-flag");
                if !saved.is_nil() && self.symbol_value(quit).is_nil() {
                    self.store_internal_value(quit, saved);
                }
                Ok(value)
            }
            Err(flow) => Err(self.offer_flow(Err(flow)).unwrap_err()),
        }
    }

    /// GNU maybe_quit/process_quit_flag. Clear the flag without watchers
    /// before invoking a quit observer, throwing to input's tag, or exiting.
    pub(crate) fn maybe_quit(&mut self) -> Result<(), Flow> {
        if self
            .quit_request
            .as_ref()
            .is_some_and(|request| request.take_for_eval())
        {
            self.quit_flag = true;
        }
        self.synchronize_quit_request();
        let Some(symbol) = self.q_soft("quit-flag") else {
            return Ok(());
        };
        let flag = self.symbol_value(symbol);
        if flag.is_nil() {
            return Ok(());
        }
        let inhibit = self.q("inhibit-quit");
        if self.symbol_value(inhibit).truthy() {
            return Ok(());
        }
        self.clear_quit_flag();
        let kill = Value::Sym(self.q("kill-emacs"));
        let result = if crate::lisp::builtins::eq_values(&flag, &kill) {
            crate::lisp::builtins::evalfn::f_kill_emacs(self, vec![Value::Nil, Value::Nil].into())
        } else {
            let input = self.q("throw-on-input");
            let tag = self.symbol_value(input);
            if crate::lisp::builtins::eq_values(&tag, &flag) {
                crate::lisp::builtins::evalfn::f_throw(self, vec![tag, Value::t()].into())
            } else {
                Err(self.signal_data(sym::QUIT, vec![]))
            }
        };
        self.offer_flow(result).map(|_| ())
    }
}
