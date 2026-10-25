//! Batch shutdown, cleanup, and command-loop handoff.
use super::*;

/// Run `kill-emacs-hook' (reporting but ignoring its errors) and exit
/// with CODE — GNU's `kill-emacs' on every batch exit path.
pub(super) fn batch_exit(i: &mut Interp, code: i32) -> ! {
    if code == 0 {
        normal_batch_exit(i);
    }
    let argument = Value::Int(if code == 255 { -1 } else { code as i128 });
    let result = i.with_call_frame("normal-top-level", vec![], |i| {
        i.with_call_frame("command-line", vec![], |i| {
            i.with_call_frame("kill-emacs", vec![argument.clone()], |i| {
                host::terminate(i, argument)
            })
        })
    });
    match host::offer_flow(i, result) {
        Err(Flow::Exit(status)) => remacs_runtime::lifecycle::exit(status),
        Err(flow) => report_flow(i, flow),
        Ok(_) => {}
    }
    remacs_runtime::lifecycle::exit(code as i128);
}

/// Unlike keyboard.c's fatal exit paths, startup.el calls the public
/// function cell.  Advice or a replacement may return; in that case the
/// normal-top-level cleanup and command loop still have work to do.
pub(super) fn normal_batch_exit(i: &mut Interp) -> ! {
    let top_level = Value::Sym(host::symbol(i, "top-level"));
    i.catch_tags.push(top_level.clone());
    let result = i.with_call_frame("normal-top-level", vec![], |i| {
        let result = i.with_call_frame("command-line", vec![], |i| {
            let kill = Value::Sym(host::symbol(i, "kill-emacs"));
            i.apply(&kill, vec![Value::t()])
        });
        let result = host::offer_flow(i, result);
        if matches!(result, Err(Flow::Exit(_))) {
            return result;
        }
        // These hooks are the unwind-protect tail of normal-top-level.
        // An accepted process exit bypasses it, like other Lisp cleanup.
        let cleanup = host::with_quit_preserved(i, |i| {
            let inhibit = host::symbol(i, "inhibit-startup-hooks");
            let runner = Value::Sym(host::symbol(i, "run-hooks"));
            if !i.symbol_value(inhibit).truthy() {
                let startup = Value::Sym(host::symbol(i, "emacs-startup-hook"));
                let term = Value::Sym(host::symbol(i, "term-setup-hook"));
                i.apply(&runner, vec![startup, term])?;
            }
            if !i.symbol_value(inhibit).truthy() {
                let window = Value::Sym(host::symbol(i, "window-setup-hook"));
                i.apply(&runner, vec![window])?;
            }
            Ok(Value::Nil)
        });
        cleanup.and(result)
    });
    i.catch_tags.pop();
    match host::offer_flow(i, result) {
        Ok(_) => {}
        Err(Flow::Throw(tag, _)) if remacs_runtime::lisp::builtins::eq_values(&tag, &top_level) => {
            host::finish_quit_unwind(i);
        }
        Err(Flow::Exit(status)) => remacs_runtime::lifecycle::exit(status),
        Err(flow) => {
            let condition = match flow {
                Flow::Signal(signal) => signal.condition,
                Flow::Quit => Value::list(vec![Value::Sym(host::symbol(i, "quit"))]),
                Flow::Throw(tag, value) => {
                    Value::list(vec![Value::Sym(host::symbol(i, "no-catch")), tag, value])
                }
                Flow::Exit(_) => unreachable!(),
            };
            host::finish_quit_unwind(i);
            if let Err(flow) = remacs_runtime::frontend::batch_command_error(i, condition) {
                report_flow(i, flow);
                batch_exit(i, 255);
            }
        }
    }
    match remacs_runtime::frontend::run_batch_editor(i) {
        Ok(()) => remacs_runtime::lifecycle::exit(host::exit_status(i).unwrap_or(0)),
        Err(flow) => {
            report_flow(i, flow);
            batch_exit(i, 255);
        }
    }
}
