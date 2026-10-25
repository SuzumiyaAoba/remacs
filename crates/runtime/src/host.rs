//! Narrow host API for startup, event dispatch, and evaluator unwinding.
//! Frontends use this module instead of reaching into builtin implementations.
use crate::lisp::{EvalResult, Flow, Interp, SymId, Value};

/// Deliver pending evaluator conditions while the host's frames are live.
pub fn offer_flow(i: &mut Interp, result: EvalResult) -> EvalResult {
    i.offer_flow(result)
}

/// Finish unwinding a quit handled by the host command loop.
pub fn finish_quit_unwind(i: &mut Interp) {
    i.finish_quit_unwind();
}

/// Run cleanup without losing a pending quit.
pub fn with_quit_preserved<T>(
    i: &mut Interp,
    run: impl FnOnce(&mut Interp) -> Result<T, Flow>,
) -> Result<T, Flow> {
    i.with_quit_preserved(run)
}

/// Evaluate the first command-line object with its string metadata intact.
pub fn eval_first_object(i: &mut Interp, source: &Value) -> EvalResult {
    i.eval_first_object(source)
}

/// Preserve the native startup protocol even when Lisp redefines these cells.
pub fn argument_substring(i: &mut Interp, token: Value, start: usize) -> EvalResult {
    crate::lisp::builtins::strfn::f_substring(i, vec![token, Value::Int(start as i128)].into())
}
pub fn argument_function(i: &mut Interp, name: Value) -> EvalResult {
    crate::lisp::builtins::call_native_builtin(i, "intern", vec![name])
}
pub fn terminate(i: &mut Interp, argument: Value) -> EvalResult {
    crate::lisp::builtins::evalfn::f_kill_emacs(i, vec![argument].into())
}
pub fn exit_status(i: &Interp) -> Option<i128> {
    i.exit_status
}

/// Resolve a native startup symbol using the evaluator's canonical symbol cache.
/// Like C-owned Emacs symbols, this identity survives Lisp `unintern` calls.
pub fn symbol(i: &mut Interp, name: &str) -> SymId {
    i.q(name)
}
