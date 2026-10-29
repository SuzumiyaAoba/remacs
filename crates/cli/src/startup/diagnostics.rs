//! Command-line error messages and Lisp backtraces.
use super::shutdown::batch_exit;
use remacs_runtime::host;
use remacs_runtime::lisp::{Flow, Interp, Value, reader::Reader};
use std::rc::Rc;

/// Diagnostic evaluation replays forms after a failure to identify its source.
pub(super) fn eval_bisect(i: &mut Interp, source: &str) -> ! {
    if let Err(flow) = i.eval_str(source) {
        eprintln!("eval_str err: {}; bisecting", describe_flow(i, &flow));
        let chars = Rc::new(source.chars().map(Into::into).collect());
        // Reader positions count characters; source slices and diagnostics use
        // UTF-8 byte offsets. Keep the two coordinate systems explicit.
        let bytes: Vec<_> = source
            .char_indices()
            .map(|(byte, _)| byte)
            .chain(std::iter::once(source.len()))
            .collect();
        let mut position = 0;
        loop {
            let next = {
                let mut reader = Reader::with_chars(i, Rc::clone(&chars));
                reader.set_position(position);
                match reader.read() {
                    Ok(Some(_)) => reader.position(),
                    _ => break,
                }
            };
            let byte = bytes[position];
            let form_source = &source[byte..bytes[next]];
            let form = {
                let mut reader = Reader::new(i, form_source);
                match reader.read() {
                    Ok(Some(form)) => form,
                    _ => break,
                }
            };
            match i.eval(&form) {
                Err(flow) => {
                    eprintln!(
                        "FAIL at byte {byte}: {form_source} => {}",
                        describe_flow(i, &flow)
                    );
                    break;
                }
                Ok(_) => position = next,
            }
        }
        remacs_runtime::lifecycle::exit(255);
    }
    remacs_runtime::lifecycle::exit(0);
}

fn describe_flow(i: &Interp, flow: &Flow) -> String {
    match flow {
        Flow::Signal(signal) => format!(
            "signal {} {}",
            i.prin1_to_string(&signal.symbol()),
            i.prin1_to_string(&signal.data())
        ),
        other => format!("{other:?}"),
    }
}

/// The signal-time snapshot includes evaluated calls and pending forms,
/// plus the host command-line scopes; no display-only padding is needed.
pub(super) fn print_batch_backtrace(i: &mut Interp) {
    for frame in i.last_error_stack.borrow().iter().rev() {
        let name = match &frame.function {
            Value::Sym(id) => i.symbol_name(*id),
            other => i.prin1_to_string(other).to_unicode_lossy(),
        };
        let args = frame
            .arguments
            .list_to_vec()
            .unwrap_or_default()
            .iter()
            .map(|value| i.prin1_to_string(value).to_unicode_lossy())
            .collect::<Vec<_>>()
            .join(" ");
        eprintln!("  {}({})", name, args);
    }
}

pub(super) fn report_flow(i: &mut Interp, flow: Flow) {
    match flow {
        Flow::Signal(signal) => {
            let sym = signal.symbol();
            let data = signal.data();
            // GNU's early debugger announces itself once before the
            // error message it is about to print.
            eprintln!("\ndebug-early-backtrace...done");
            let err_obj = signal.condition;
            let msg = remacs_runtime::lisp::builtins::error_message(i, &err_obj);
            let name = match &sym {
                Value::Sym(id) => i.symbol_name(*id),
                _ => "error".into(),
            };
            let args = data
                .list_to_vec()
                .unwrap_or_default()
                .iter()
                .map(|v| i.prin1_to_string(v).to_unicode_lossy())
                .collect::<Vec<_>>()
                .join(" ");
            eprintln!("{}\n\nError: {} ({})", msg, name, args);
        }
        Flow::Throw(tag, v) => {
            eprintln!(
                "uncaught throw: {} {}",
                i.princ_to_string(&tag),
                i.princ_to_string(&v)
            );
        }
        Flow::Quit => {
            eprintln!("Quit");
        }
        Flow::Exit(code) => {
            remacs_runtime::lifecycle::exit(code);
        }
    }
}

/// Match GNU's startup error handling: batch mode reports and exits,
/// while interactive startup prints the error through the editor command
/// error path and returns control to the front-end loop.
pub(super) fn report_startup_flow(i: &mut Interp, flow: Flow, batch: bool) -> bool {
    if let Flow::Exit(code) = &flow {
        remacs_runtime::lifecycle::exit(*code);
    }
    if batch {
        report_flow(i, flow);
        print_batch_backtrace(i);
        batch_exit(i, 255);
    }
    let condition = match flow {
        Flow::Signal(signal) => signal.condition,
        Flow::Quit => {
            host::finish_quit_unwind(i);
            Value::list(vec![Value::Sym(host::symbol(i, "quit"))])
        }
        Flow::Throw(tag, value) => Value::list(vec![
            Value::Sym(host::symbol(i, "no-catch")),
            tag,
            value,
        ]),
        Flow::Exit(_) => unreachable!(),
    };
    match remacs_runtime::frontend::batch_command_error(i, condition) {
        Ok(_) => false,
        Err(Flow::Exit(code)) => remacs_runtime::lifecycle::exit(code),
        Err(flow) => {
            report_flow(i, flow);
            false
        }
    }
}
