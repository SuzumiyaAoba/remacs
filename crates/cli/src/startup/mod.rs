//! Shared command-line startup actions for the terminal and GUI.
use crate::arguments as cli;
use remacs_runtime::host;
mod actions;
mod diagnostics;
mod shutdown;
use actions::{load_file, load_file_script, run_action};
use diagnostics::{print_batch_backtrace, report_flow};
use remacs_runtime::lisp::{Flow, Interp, OutputSink, Value};
use shutdown::batch_exit;

/// Apply the live argument stream before starting an editor front-end.
/// Explicit exits, batch completion and command-line failures end the process.
pub fn run(i: &mut Interp, args: &[String]) {
    let mut batch =
        cli::options(args).any(|option| matches!(option, "--batch" | "-batch" | "--script"));
    let exit = 0i32;
    // GNU captures this directory once before processing file arguments.
    // Visiting a file changes its buffer's default-directory, but must not
    // change the base used for the next command-line file name.
    let directory_symbol = host::symbol(i, "command-line-default-directory");
    let file_directory = i.symbol_value(directory_symbol);

    let left_symbol = host::symbol(i, "command-line-args-left");
    let _ = i.set_symbol_default(
        left_symbol,
        Value::list(args.iter().map(Value::string).collect()),
    );
    let mut parse_options = true;
    let mut load_path_options = remacs_runtime::lisp::load::CommandLineLoadPath::default();
    loop {
        let cli_rest = i.symbol_value(left_symbol);
        if cli_rest.is_nil() {
            break;
        }
        let token_value =
            match with_command_line_frames(i, cli_rest.clone(), |i| pop_cli_string(i, left_symbol))
            {
                Ok(value @ Value::Str(_)) => value,
                Ok(_) => unreachable!(),
                Err(flow) => {
                    report_flow(i, flow);
                    print_batch_backtrace(i);
                    batch_exit(i, 255);
                }
            };
        let Value::Str(token_text) = &token_value else {
            unreachable!()
        };
        let token = token_text.borrow().to_unicode_lossy();
        if parse_options && token == "--" {
            parse_options = false;
            continue;
        }
        let option = if parse_options {
            cli::normalize_option(&token)
        } else {
            None
        };
        if option.is_none() {
            // File arguments are actions; later actions and exit hooks see
            // their buffers, but every relative path uses the startup base.
            inhibit_startup(i);
            let result = (|| {
                let expand = Value::Sym(host::symbol(i, "expand-file-name"));
                let file = i.apply(&expand, vec![Value::string(&token), file_directory.clone()])?;
                let visit = Value::Sym(host::symbol(i, "find-file"));
                i.apply(&visit, vec![file])
            })();
            match result {
                Ok(_) => {}
                Err(Flow::Exit(code)) => remacs_runtime::lifecycle::exit(code),
                Err(flow) => {
                    report_flow(i, flow);
                    batch_exit(i, 255);
                }
            }
            continue;
        }
        let option = option.expect("recognized option");
        let mut args = vec![token.clone()];
        let mut operand = None;
        if let Some(value) = option.value {
            args.push(value.to_owned());
            let start = token.chars().count() - value.chars().count();
            let substring = host::argument_substring(i, token_value.clone(), start);
            operand = Some(substring.expect("inline operand is a valid token substring"));
        } else if cli::takes_operand(&token) {
            match with_command_line_frames(i, cli_rest.clone(), |i| pop_cli_string(i, left_symbol))
            {
                Ok(value @ Value::Str(_)) => {
                    let Value::Str(text) = &value else {
                        unreachable!()
                    };
                    args.push(text.borrow().to_unicode_lossy());
                    operand = Some(value);
                }
                Ok(_) => unreachable!(),
                Err(flow) => {
                    report_flow(i, flow);
                    print_batch_backtrace(i);
                    batch_exit(i, 255);
                }
            }
        }
        let (mut idx, arg) = (0, option.name);
        match arg {
            "-Q" | "--quick" => {
                // GNU: -Q inhibits site/user init, the splash screen,
                // and X resources — it does NOT imply batch.  remacs
                // never loads an init file or splash screen, so the
                // observable remainder is the resource inhibition;
                // interactive startup proceeds normally.
                let sid = host::symbol(i, "inhibit-x-resources");
                let _ = i.set_symbol_default(sid, Value::t());
            }
            "-no-x-resources" | "--no-x-resources" => {
                let sid = host::symbol(i, "inhibit-x-resources");
                let _ = i.set_symbol_default(sid, Value::t());
            }
            "--batch" | "-batch" => {
                batch = true;
            }
            "--no-init" | "-q" => {
                // GNU: -q/--no-init skips user/site init files but
                // still starts the interactive editor.
            }
            "--ieval" => {
                idx += 1;
                if idx < args.len() {
                    let src = args[idx].clone();
                    match i.eval_str(&src) {
                        Ok(_) => {}
                        Err(f) => {
                            let d = match &f {
                                remacs_runtime::lisp::Flow::Signal(signal) => format!(
                                    "signal {} {}",
                                    i.prin1_to_string(&signal.symbol()),
                                    i.prin1_to_string(&signal.data())
                                ),
                                other => format!("{other:?}"),
                            };
                            eprintln!("eval_str err: {d}; bisecting");
                            let chars: std::rc::Rc<Vec<remacs_runtime::lisp::value::EmacsChar>> =
                                std::rc::Rc::new(src.chars().map(Into::into).collect());
                            let mut pos = 0usize;
                            loop {
                                let next = {
                                    let mut r = remacs_runtime::lisp::reader::Reader::with_chars(
                                        i,
                                        chars.clone(),
                                    );
                                    r.set_position(pos);
                                    match r.read() {
                                        Ok(Some(_)) => r.position(),
                                        _ => break,
                                    }
                                };
                                let form_src = &src[pos..next];
                                let form = {
                                    let mut r =
                                        remacs_runtime::lisp::reader::Reader::new(i, form_src);
                                    match r.read() {
                                        Ok(Some(v)) => v,
                                        _ => break,
                                    }
                                };
                                match i.eval(&form) {
                                    Err(e) => {
                                        let d = match &e {
                                            remacs_runtime::lisp::Flow::Signal(signal) => format!(
                                                "signal {} {}",
                                                i.prin1_to_string(&signal.symbol()),
                                                i.prin1_to_string(&signal.data())
                                            ),
                                            other => format!("{other:?}"),
                                        };
                                        eprintln!("FAIL at byte {pos}: {form_src} => {d}");
                                        break;
                                    }
                                    Ok(_) => pos = next,
                                }
                            }
                            remacs_runtime::lifecycle::exit(255);
                        }
                    }
                    remacs_runtime::lifecycle::exit(exit as i128);
                }
            }
            "-L" | "--directory" => {
                let result = with_command_line_frames(i, cli_rest, |i| {
                    load_path_options.add(i, Value::string(&args[1]))
                });
                if let Err(flow) = result {
                    report_flow(i, flow);
                    print_batch_backtrace(i);
                    batch_exit(i, 255);
                }
            }
            "--eval" | "--execute" => {
                idx += 1;
                if idx < args.len() {
                    // GNU `command-line-1': -eval/-execute set
                    // `inhibit-startup-screen' before evaluating.
                    inhibit_startup(i);
                    if !run_action(i, operand.as_ref().unwrap(), cli_rest.clone(), false) {
                        // GNU's `command-line-1': a signaled error
                        // aborts the remaining args and exits 255
                        // (after running `kill-emacs-hook').
                        batch_exit(i, 255);
                    }
                    if i.quit_editor {
                        remacs_runtime::lifecycle::exit(exit as i128);
                    }
                }
            }
            "--load" | "-l" => {
                idx += 1;
                if idx < args.len() {
                    if !load_file(i, &args[idx], cli_rest.clone()) {
                        batch_exit(i, 255);
                    }
                    if i.quit_editor {
                        remacs_runtime::lifecycle::exit(exit as i128);
                    }
                }
            }
            "-f" | "--funcall" => {
                // GNU's `command-line-1': call FUNCTION immediately,
                // leaving the not-yet-processed args in
                // `command-line-args-left' for batch functions such as
                // `batch-native-compile' to consume.
                idx += 1;
                if idx < args.len() {
                    // GNU `command-line-1': -f/-funcall inhibit too.
                    inhibit_startup(i);
                    if !run_action(i, operand.as_ref().unwrap(), cli_rest.clone(), true) {
                        batch_exit(i, 255);
                    }
                    if i.quit_editor {
                        remacs_runtime::lifecycle::exit(exit as i128);
                    }
                    // The next iteration reads the live Lisp value. Functions
                    // may consume, replace, or mutate any remaining arguments.
                }
            }
            "--script" => {
                idx += 1;
                if idx < args.len() {
                    if !load_file_script(i, &args[idx], cli_rest.clone()) {
                        batch_exit(i, 255);
                    }
                    batch_exit(i, exit);
                }
            }
            "-nw" | "--no-window-system" => {
                // Already accounted for by the pre-`Interp::new'
                // dispatch (this process only runs headless paths).
            }
            _ => {
                let flow = with_command_line_frames(i, cli_rest, |i| {
                    Err(i.error(&format!("Unknown option `{arg}'")))
                })
                .unwrap_err();
                report_flow(i, flow);
                print_batch_backtrace(i);
                batch_exit(i, 255);
            }
        }
    }

    if batch {
        // GNU's `command-line-1' ends batch processing with
        // (kill-emacs), which runs `kill-emacs-hook' before exiting.
        batch_exit(i, exit);
    }
}

/// GNU `command-line-1' sets `inhibit-startup-screen' for -eval/-f/
/// -insert/-visit args and for any file arg when no window system —
/// always true on this headless build.  The varalias
/// `inhibit-splash-screen' shares the cell, so one set covers both.
fn inhibit_startup(i: &mut Interp) {
    let id = host::symbol(i, "inhibit-startup-screen");
    let _ = i.set_symbol_default(id, Value::t());
}

/// Pop one argument from the live Lisp queue. Validate lazily so a bad
/// later element does not prevent earlier actions from running.
fn pop_cli_string(
    i: &mut Interp,
    left: remacs_runtime::lisp::SymId,
) -> remacs_runtime::lisp::EvalResult {
    let (value, tail) = match i.symbol_value(left) {
        Value::Nil => (Value::Nil, Value::Nil),
        Value::Cons(cell) => {
            let pair = cell.borrow();
            (pair.car.clone(), pair.cdr.clone())
        }
        other => return Err(i.wrong_type_mut("listp", &other)),
    };
    i.set_symbol(left, tail)?;
    if !matches!(&value, Value::Str(_)) {
        return Err(i.wrong_type_mut("stringp", &value));
    }
    Ok(value)
}

/// Run an actual command-line action with the Lisp-visible entry frames.
fn with_command_line_frames(
    i: &mut Interp,
    cli_rest: Value,
    run: impl FnOnce(&mut Interp) -> remacs_runtime::lisp::EvalResult,
) -> remacs_runtime::lisp::EvalResult {
    i.with_call_frame("normal-top-level", vec![], |i| {
        i.with_call_frame("command-line", vec![], |i| {
            i.with_call_frame("command-line-1", vec![cli_rest], run)
        })
    })
}
