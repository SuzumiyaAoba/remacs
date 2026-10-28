//! Interpreter storage, execution reset, and scoped state restoration.

mod interpreter;
use super::LexEnv;
use crate::lisp::{
    error::EvalResult,
    value::{Lambda, Marker, Value},
};
pub use interpreter::Interp;
use rustc_hash::FxHashMap as HashMap;
use std::{cell::RefCell, rc::Rc};

impl Interp {
    /// Install or remove the host clipboard backend used by GUI selection
    /// primitives. The backend remains attached to this interpreter only.
    pub fn set_clipboard_backend(
        &mut self,
        backend: Option<Rc<dyn crate::frontend::ClipboardBackend>>,
    ) {
        self.clipboard_backend = backend;
    }

    /// Install or remove the host bell backend used by interactive GUI ding.
    pub fn set_bell_backend(&mut self, backend: Option<Rc<dyn crate::frontend::BellBackend>>) {
        self.bell_backend = backend;
    }

    /// Install or remove the host dialog backend used by graphical frames.
    pub fn set_dialog_backend(&mut self, backend: Option<Rc<dyn crate::frontend::DialogBackend>>) {
        self.dialog_backend = backend;
    }

    /// Install or remove the host file panel used by graphical file-name primitives.
    pub fn set_file_dialog_backend(
        &mut self,
        backend: Option<Rc<dyn crate::frontend::FileDialogBackend>>,
    ) {
        self.file_dialog_backend = backend;
    }

    /// Install or remove the host native-window backend.
    pub fn set_frame_window_backend(
        &mut self,
        backend: Option<Rc<dyn crate::frontend::FrameWindowBackend>>,
    ) {
        self.frame_window_backend = backend;
    }

    /// Install or remove the host color panel backend.
    pub fn set_color_panel_backend(&mut self, backend: Option<Rc<dyn crate::frontend::ColorPanelBackend>>) {
        self.color_panel_backend = backend;
    }

    /// Install or remove the host application visibility backend.
    pub fn set_application_visibility_backend(
        &mut self,
        backend: Option<Rc<dyn crate::frontend::ApplicationVisibilityBackend>>,
    ) {
        self.application_visibility_backend = backend;
    }

    pub(crate) fn reset_dump_execution(&mut self) {
        self.specbind.clear();
        self.lexenv = None;
        self.eval_depth = 0;
        self.explicit_eval_depth = 0;
        self.clear_quit_flag();
        self.bc_quit_counter = None;
        self.unwind_quit = None;
        self.catch_tags.clear();
        self.command_args.clear();
        self.interactive_call_depth = 0;
        self.defining_symbol = None;
        self.recursion_depth = 0;
        self.executing_kbd_macro = false;
        self.last_error = None;
        self.output_buffer.clear();
        self.capture_output = false;
        self.pending_insert_behind.clear();
        self.pending_insert_front.clear();
        self.pending_overlay_hooks.clear();
        self.combine_after_change_list.clear();
        self.combine_after_change_buffer = None;
        self.loading_dumped = false;
        self.loads_in_progress.clear();
        self.load_stream = None;
        self.load_stream_symbol = None;
        self.dumped_call_depth = 0;
        self.macroexp_call_depth = 0;
        self.current_lambda_dumped = false;
        self.current_body_compiled = false;
        self.compiled_macro_expansion = false;
        self.reading_elc = false;
        self.boot_loading = false;
        self.definition_history_enabled = true;
        self.loading_compiled_library = false;
        self.bc_depth = 0;
        self.expand_unadvised_depth = 0;
        self.macro_cache.clear();
        self.macro_gen = self.macro_gen.wrapping_add(1);
        *self.macroexpand_all_cache.borrow_mut() = (self.macro_gen, HashMap::default());
        self.overlay_handles.clear();
        self.overlay_cleanup_at = 0;
        self.frame_state_seen = None;
        self.stderr_need_newline = false;
        self.out_last_char = None;
        self.quit_editor = false;
        self.exit_status = None;
        self.unread_switch_frame = Value::Nil;
        self.minibuf_level = 0;
        self.minibuf_cll.clear();
        self.minibuf_prompts.clear();
        self.minibuf_catching_exit = false;
        self.minibuf_exited = false;
        self.minibuf_pending_flow = None;
        self.minibuf_exit_fn = None;
        self.macro_replay.clear();
        self.macro_replaying = false;
        self.macro_pending_flow = None;
        self.macro_strict = false;
        self.handler_bindings.clear();
        self.case_handlers.clear();
        self.current_thread = 0;
        self.threads.truncate(1);
        self.lisp_stack.clear();
        self.pending_frames.clear();
        self.last_error_stack.borrow_mut().clear();
        self.cur_head_form = Value::Nil;
        self.memory_profiler = false;
        self.cpu_profiler = false;
        self.profiling = Default::default();
        self.subr_prof.clear();
        self.x_display_attempted = false;
    }

    pub(crate) fn reset_dump_arguments(&mut self, arguments: &[String]) {
        let executable = std::env::args().next().unwrap_or_else(|| "remacs".into());
        let mut values = vec![Value::string(executable)];
        values.extend(arguments.iter().cloned().map(Value::string));
        let full = Value::list(values);
        let left = Value::list(arguments.iter().cloned().map(Value::string).collect());
        let id = self.q("command-line-args");
        self.obarray.symbol_mut(id).value = full;
        let id = self.q("command-line-args-left");
        self.obarray.symbol_mut(id).value = left;
    }

    pub(crate) fn save_async_match_data(&mut self) {
        if self.running_asynch_code && self.saved_async_match_data.is_none() {
            self.saved_async_match_data = Some(self.match_data.take());
        }
    }

    pub(crate) fn restore_async_match_data(&mut self) {
        if let Some(saved) = self.saved_async_match_data.take() {
            self.match_data = saved;
        }
    }

    /// Enter source evaluation without inheriting a compiled caller's
    /// access to hidden helpers. LOAD preserves its own file provenance;
    /// eval, eval-buffer, and string evaluation start a user context.
    pub(crate) fn with_eval_context<T>(
        &mut self,
        compiled_library: bool,
        evaluate: impl FnOnce(&mut Self) -> T,
    ) -> T {
        let saved = EvaluationContext::capture(self);
        self.current_body_compiled = false;
        self.current_lambda_dumped = false;
        self.compiled_macro_expansion = false;
        self.loading_compiled_library = compiled_library;
        let result = evaluate(self);
        saved.restore(self);
        result
    }

    /// Scope a lambda's provenance independently of its caller. Source
    /// lambdas additionally hide compiler-only helpers during evaluation.
    pub(super) fn with_lambda_context<T>(
        &mut self,
        lambda: &Lambda,
        evaluate: impl FnOnce(&mut Self) -> T,
    ) -> T {
        let compiled = lambda.dumped_doc || lambda.compiled;
        if compiled {
            self.dumped_call_depth += 1;
        }
        let saved_lambda = std::mem::replace(&mut self.current_lambda_dumped, lambda.dumped_doc);
        let saved_compiled = self.current_body_compiled;
        let result = if lambda.dumped_doc {
            self.current_body_compiled = compiled;
            evaluate(self)
        } else {
            self.with_eval_context(false, |i| {
                i.current_body_compiled = compiled;
                evaluate(i)
            })
        };
        self.current_lambda_dumped = saved_lambda;
        self.current_body_compiled = saved_compiled;
        if compiled {
            self.dumped_call_depth -= 1;
        }
        result
    }

    /// Restore the caller's lexical environment before unwinding dynamic
    /// parameters. Keep this order even when argument binding or the body
    /// signals: unbinding can invoke Lisp variable watchers.
    pub(super) fn with_lambda_bindings(
        &mut self,
        env: LexEnv,
        evaluate: impl FnOnce(&mut Self) -> EvalResult,
    ) -> EvalResult {
        let dynamic = env.is_none();
        let saved = std::mem::replace(&mut self.lexenv, env);
        let mark = self.specbind_depth();
        let result = evaluate(self);
        self.lexenv = saved;
        if dynamic {
            self.unbind_to(mark)?;
        }
        result
    }

    // ---------- excursions ----------

    /// Snapshot for `save-excursion`: buffer + point saved as a *marker*
    /// (like GNU), so edits before point move the restored position.
    /// `save_mark` additionally records the mark (`save-mark-and-excursion`).
    pub fn save_excursion_state(&mut self, save_mark: bool) -> ExcursionState {
        let buf = self.current_buffer;
        let ma = self.q_soft("mark-active").unwrap_or(0);
        let (point, mark, mark_active) = self
            .buffers
            .get(buf)
            .map(|r| {
                let bb = r.borrow();
                (
                    bb.point,
                    bb.mark_position(),
                    bb.locals.get(&ma).map(|v| v.truthy()).unwrap_or(false),
                )
            })
            .unwrap_or((0, None, false));
        let mut mk = |pos| {
            let m = crate::lisp::memory::allocate(
                Marker {
                    buffer: Some(buf),
                    position: pos,
                    insertion_type: false,
                },
                crate::lisp::Value::Marker,
            );
            if let Some(b) = self.buffers.get(buf) {
                b.borrow_mut().register_marker(&m);
            }
            m
        };
        ExcursionState {
            buffer: buf,
            point: mk(point),
            mark: if save_mark { mark.map(&mut mk) } else { None },
            mark_active,
        }
    }

    pub fn restore_excursion_state(&mut self, s: ExcursionState) {
        if let Some(b) = self.buffers.get(s.buffer) {
            {
                let ma = self.q_soft("mark-active").unwrap_or(0);
                let mut bb = b.borrow_mut();
                bb.set_point(s.point.borrow().position);
                if let Some(m) = &s.mark {
                    let p = m.borrow().position;
                    let len = bb.text.len();
                    bb.set_mark(p.min(len));
                    bb.locals.insert(
                        ma,
                        if s.mark_active {
                            Value::t()
                        } else {
                            Value::Nil
                        },
                    );
                }
            }
            self.set_current_buffer(s.buffer);
        }
    }

    /// Snapshot for `save-restriction` (narrowing bounds). GNU records the
    /// bounds as markers so edits inside the restriction move them.
    pub fn save_restriction_state(&mut self) -> RestrictionState {
        let buf = self.current_buffer;
        let (begv, zv) = self
            .buffers
            .get(buf)
            .map(|r| {
                let bb = r.borrow();
                (bb.begv, bb.zv)
            })
            .unwrap_or((0, 0));
        let mk = |pos, itype| {
            let m = crate::lisp::memory::allocate(
                Marker {
                    buffer: Some(buf),
                    position: pos,
                    insertion_type: itype,
                },
                crate::lisp::Value::Marker,
            );
            if let Some(b) = self.buffers.get(buf) {
                b.borrow_mut().register_marker(&m);
            }
            m
        };
        // GNU records ZV with insertion_type = t: text inserted exactly at
        // the end of the restriction stays inside it.
        RestrictionState {
            buffer: buf,
            begv: mk(begv, false),
            zv: mk(zv, true),
        }
    }

    pub fn restore_restriction_state(&mut self, s: RestrictionState) {
        if let Some(b) = self.buffers.get(s.buffer) {
            let mut bb = b.borrow_mut();
            bb.begv = s.begv.borrow().position.min(bb.text.len());
            bb.zv = s.zv.borrow().position.max(bb.begv).min(bb.text.len());
        }
    }
}

/// Saved point/buffer for `save-excursion` (marker-based, like GNU).
pub struct ExcursionState {
    pub buffer: usize,
    pub point: Rc<RefCell<Marker>>,
    /// Only `Some` for `save-mark-and-excursion`.
    pub mark: Option<Rc<RefCell<Marker>>>,
    pub mark_active: bool,
}

/// Saved narrowing bounds for `save-restriction` (markers like GNU).
pub struct RestrictionState {
    pub buffer: usize,
    pub begv: Rc<RefCell<Marker>>,
    pub zv: Rc<RefCell<Marker>>,
}

/// These flags describe the immediate evaluation context, rather than
/// ancestor stack frames. Save them together when entering source code.
struct EvaluationContext {
    body_compiled: bool,
    lambda_dumped: bool,
    macro_expansion_compiled: bool,
    library_compiled: bool,
}

impl EvaluationContext {
    fn capture(i: &Interp) -> Self {
        Self {
            body_compiled: i.current_body_compiled,
            lambda_dumped: i.current_lambda_dumped,
            macro_expansion_compiled: i.compiled_macro_expansion,
            library_compiled: i.loading_compiled_library,
        }
    }

    fn restore(self, i: &mut Interp) {
        i.current_lambda_dumped = self.lambda_dumped;
        i.current_body_compiled = self.body_compiled;
        i.compiled_macro_expansion = self.macro_expansion_compiled;
        i.loading_compiled_library = self.library_compiled;
    }
}

/// Caller selection saved after minibuffer mode hooks, with the buffer
/// identity recorded before entering that mode. A hook may select a window.
pub(super) struct MinibufferCaller {
    buffer: usize,
    window: Option<crate::editor::WindowRef>,
}

impl MinibufferCaller {
    pub fn capture(i: &Interp, buffer: usize) -> Self {
        Self {
            buffer,
            window: i
                .selected_frame
                .as_ref()
                .map(|f| f.borrow().selected.clone()),
        }
    }

    pub fn restore(self, i: &mut Interp) {
        if let (Some(frame), Some(window)) = (&i.selected_frame, self.window) {
            frame.borrow_mut().selected = window;
        }
        if i.buffer_live(self.buffer) {
            i.set_current_buffer(self.buffer);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lisp::error::Flow;

    fn provenance(i: &Interp) -> (bool, bool, bool, bool) {
        (
            i.current_body_compiled,
            i.current_lambda_dumped,
            i.compiled_macro_expansion,
            i.loading_compiled_library,
        )
    }

    #[test]
    fn nested_source_contexts_restore_provenance_after_errors() {
        let mut i = Interp::vacant();
        i.current_body_compiled = true;
        i.current_lambda_dumped = true;
        i.compiled_macro_expansion = false;
        i.loading_compiled_library = false;
        let original = provenance(&i);
        let result: Result<(), Flow> = i.with_eval_context(true, |i| {
            assert_eq!(provenance(i), (false, false, false, true));
            let inner: Result<(), Flow> = i.with_eval_context(false, |i| {
                assert_eq!(provenance(i), (false, false, false, false));
                i.current_lambda_dumped = true;
                i.quit_flag = true;
                Err(Flow::Quit)
            });
            assert!(matches!(inner, Err(Flow::Quit)));
            assert_eq!(provenance(i), (false, false, false, true));
            inner
        });
        assert!(matches!(result, Err(Flow::Quit)));
        assert_eq!(provenance(&i), original);
        assert!(
            i.quit_flag,
            "source-context restoration must not consume an unrelated quit request"
        );
    }
}
