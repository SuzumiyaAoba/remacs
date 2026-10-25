//! Live evaluator frames, including calls still evaluating their arguments.
use super::{Argv, EvalResult, Interp, Value};

#[derive(Clone)]
pub struct Frame {
    pub evaluated: bool,
    pub function: Value,
    pub arguments: Value,
}

pub(crate) struct PendingFrame {
    function: Value,
    arguments: Value,
    depth: usize,
    active: bool,
}

impl Interp {
    /// Record a source expression until it enters a resolved function.  The
    /// scope survives argument evaluation and is unwound for every Lisp exit.
    pub(crate) fn with_pending_frame<T>(
        &mut self,
        function: Value,
        arguments: Value,
        run: impl FnOnce(&mut Self, usize) -> T,
    ) -> T {
        let index = self.pending_frames.len();
        self.pending_frames.push(PendingFrame {
            function,
            arguments,
            depth: self.lisp_stack.len(),
            active: !self.current_body_compiled,
        });
        let result = run(self, index);
        self.pending_frames.truncate(index);
        result
    }

    pub(crate) fn finish_pending_frame(&mut self, index: usize) {
        self.pending_frames[index].active = false;
    }

    /// Outermost first, as for lisp_stack. Pending frames at depth N sit
    /// outside call N and inside call N-1. Inactive entries have already
    /// become evaluated calls, or belong to compiled-equivalent bodies.
    pub fn backtrace_snapshot(&self) -> Vec<Frame> {
        let mut result = Vec::new();
        let mut pending = self
            .pending_frames
            .iter()
            .filter(|frame| frame.active)
            .peekable();
        for depth in 0..=self.lisp_stack.len() {
            while pending.peek().is_some_and(|frame| frame.depth == depth) {
                let frame = pending.next().unwrap();
                result.push(Frame {
                    evaluated: false,
                    function: frame.function.clone(),
                    arguments: frame.arguments.clone(),
                });
            }
            if let Some((function, arguments)) = self.lisp_stack.get(depth) {
                result.push(Frame {
                    evaluated: true,
                    function: function.clone(),
                    arguments: Value::list(arguments.as_slice().to_vec()),
                });
            }
        }
        result
    }

    /// A Lisp-visible entry implemented by a host handler (for example the
    /// command-line dispatcher). Record its actual dynamic execution scope.
    pub fn with_call_frame(
        &mut self,
        name: &str,
        arguments: Vec<Value>,
        run: impl FnOnce(&mut Self) -> EvalResult,
    ) -> EvalResult {
        self.with_lisp_eval_depth(false, |i| {
            let function = Value::Sym(i.intern(name));
            let depth = i.lisp_stack.len();
            i.profiler_checkpoint();
            i.lisp_stack.push((function, Argv::new(arguments)));
            let result = run(i);
            let result = i.offer_flow(result);
            i.profiler_checkpoint();
            i.lisp_stack.truncate(depth);
            result
        })
    }
}
