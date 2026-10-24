//! Shared command-line startup actions for the terminal and GUI.
use crate::cli;
use crate::lisp::{Flow, Interp, OutputSink, Value};

/// Apply the live argument stream before starting an editor front-end.
/// Explicit exits, batch completion and command-line failures end the process.
pub fn run(i: &mut Interp, args: &[String]) {
    let mut batch =
        cli::options(args).any(|option| matches!(option, "--batch" | "-batch" | "--script"));
    let exit = 0i32;
    // GNU captures this directory once before processing file arguments.
    // Visiting a file changes its buffer's default-directory, but must not
    // change the base used for the next command-line file name.
    let directory_symbol = i.q("command-line-default-directory");
    let file_directory = i.symbol_value(directory_symbol);

    let left_symbol = i.q("command-line-args-left");
    let _ = i.set_symbol_default(
        left_symbol,
        Value::list(args.iter().map(Value::string).collect()),
    );
    let mut parse_options = true;
    let mut load_path_options = crate::lisp::load::CommandLineLoadPath::default();
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
        let Value::Str(token_text) = &token_value else { unreachable!() };
        let token = token_text.borrow().clone();
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
                let expand = Value::Sym(i.q("expand-file-name"));
                let file = i.apply(&expand, vec![Value::string(&token), file_directory.clone()])?;
                let visit = Value::Sym(i.q("find-file"));
                i.apply(&visit, vec![file])
            })();
            match result {
                Ok(_) => {}
                Err(Flow::Exit(code)) => crate::lifecycle::exit(code),
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
            let substring = crate::lisp::builtins::strfn::f_substring(i,
                vec![token_value.clone(), Value::Int(start as i128)].into());
            operand = Some(substring.expect("inline operand is a valid token substring"));
        } else if cli::takes_operand(&token) {
            match with_command_line_frames(i, cli_rest.clone(), |i| pop_cli_string(i, left_symbol))
            {
                Ok(value @ Value::Str(_)) => {
                    let Value::Str(text) = &value else { unreachable!() };
                    args.push(text.borrow().clone());
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
                let sid = i.q("inhibit-x-resources");
                let _ = i.set_symbol_default(sid, Value::t());
            }
            "-no-x-resources" | "--no-x-resources" => {
                let sid = i.q("inhibit-x-resources");
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
                                crate::lisp::Flow::Signal(signal) => format!(
                                    "signal {} {}",
                                    i.prin1_to_string(&signal.symbol()),
                                    i.prin1_to_string(&signal.data())
                                ),
                                other => format!("{other:?}"),
                            };
                            eprintln!("eval_str err: {d}; bisecting");
                            let chars: std::rc::Rc<Vec<char>> =
                                std::rc::Rc::new(src.chars().collect());
                            let mut pos = 0usize;
                            loop {
                                let next = {
                                    let mut r =
                                        crate::lisp::reader::Reader::with_chars(i, chars.clone());
                                    r.set_position(pos);
                                    match r.read() {
                                        Ok(Some(_)) => r.position(),
                                        _ => break,
                                    }
                                };
                                let form_src = &src[pos..next];
                                let form = {
                                    let mut r = crate::lisp::reader::Reader::new(i, form_src);
                                    match r.read() {
                                        Ok(Some(v)) => v,
                                        _ => break,
                                    }
                                };
                                match i.eval(&form) {
                                    Err(e) => {
                                        let d = match &e {
                                            crate::lisp::Flow::Signal(signal) => format!(
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
                            crate::lifecycle::exit(255);
                        }
                    }
                    crate::lifecycle::exit(exit as i128);
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
                        crate::lifecycle::exit(exit as i128);
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
                        crate::lifecycle::exit(exit as i128);
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
                        crate::lifecycle::exit(exit as i128);
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

/// Run `kill-emacs-hook' (reporting but ignoring its errors) and exit
/// with CODE — GNU's `kill-emacs' on every batch exit path.
fn batch_exit(i: &mut Interp, code: i32) -> ! {
    if code == 0 {
        normal_batch_exit(i);
    }
    let argument = Value::Int(if code == 255 { -1 } else { code as i128 });
    let result = i.with_call_frame("normal-top-level", vec![], |i| {
        i.with_call_frame("command-line", vec![], |i| {
            i.with_call_frame("kill-emacs", vec![argument.clone()], |i| {
                crate::lisp::builtins::evalfn::f_kill_emacs(i, vec![argument].into())
            })
        })
    });
    match i.offer_flow(result) {
        Err(Flow::Exit(status)) => crate::lifecycle::exit(status),
        Err(flow) => report_flow(i, flow),
        Ok(_) => {}
    }
    crate::lifecycle::exit(code as i128);
}

/// Unlike keyboard.c's fatal exit paths, startup.el calls the public
/// function cell.  Advice or a replacement may return; in that case the
/// normal-top-level cleanup and command loop still have work to do.
fn normal_batch_exit(i: &mut Interp) -> ! {
    let top_level = Value::Sym(i.q("top-level"));
    i.catch_tags.push(top_level.clone());
    let result = i.with_call_frame("normal-top-level", vec![], |i| {
        let result = i.with_call_frame("command-line", vec![], |i| {
            let kill = Value::Sym(i.q("kill-emacs"));
            i.apply(&kill, vec![Value::t()])
        });
        let result = i.offer_flow(result);
        if matches!(result, Err(Flow::Exit(_))) {
            return result;
        }
        // These hooks are the unwind-protect tail of normal-top-level.
        // An accepted process exit bypasses it, like other Lisp cleanup.
        let cleanup = i.with_quit_preserved(|i| {
            let inhibit = i.q("inhibit-startup-hooks");
            let runner = Value::Sym(i.q("run-hooks"));
            if !i.symbol_value(inhibit).truthy() {
                let startup = Value::Sym(i.q("emacs-startup-hook"));
                let term = Value::Sym(i.q("term-setup-hook"));
                i.apply(&runner, vec![startup, term])?;
            }
            if !i.symbol_value(inhibit).truthy() {
                let window = Value::Sym(i.q("window-setup-hook"));
                i.apply(&runner, vec![window])?;
            }
            Ok(Value::Nil)
        });
        cleanup.and(result)
    });
    i.catch_tags.pop();
    match i.offer_flow(result) {
        Ok(_) => {}
        Err(Flow::Throw(tag, _)) if crate::lisp::builtins::eq_values(&tag, &top_level) => {
            i.finish_quit_unwind();
        }
        Err(Flow::Exit(status)) => crate::lifecycle::exit(status),
        Err(flow) => {
            let condition = match flow {
                Flow::Signal(signal) => signal.condition,
                Flow::Quit => Value::list(vec![Value::Sym(i.q("quit"))]),
                Flow::Throw(tag, value) => Value::list(vec![
                    Value::Sym(i.q("no-catch")), tag, value,
                ]),
                Flow::Exit(_) => unreachable!(),
            };
            i.finish_quit_unwind();
            if let Err(flow) = crate::term::batch_command_error(i, condition) {
                report_flow(i, flow);
                batch_exit(i, 255);
            }
        }
    }
    match crate::term::run_batch_editor(i) {
        Ok(()) => crate::lifecycle::exit(i.exit_status.unwrap_or(0)),
        Err(flow) => {
            report_flow(i, flow);
            batch_exit(i, 255);
        }
    }
}

/// GNU `command-line-1' sets `inhibit-startup-screen' for -eval/-f/
/// -insert/-visit args and for any file arg when no window system —
/// always true on this headless build.  The varalias
/// `inhibit-splash-screen' shares the cell, so one set covers both.
fn inhibit_startup(i: &mut Interp) {
    let id = i.q("inhibit-startup-screen");
    let _ = i.set_symbol_default(id, Value::t());
}

/// Pop one argument from the live Lisp queue. Validate lazily so a bad
/// later element does not prevent earlier actions from running.
fn pop_cli_string(i: &mut Interp, left: crate::lisp::SymId) -> crate::lisp::EvalResult {
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
    run: impl FnOnce(&mut Interp) -> crate::lisp::EvalResult,
) -> crate::lisp::EvalResult {
    i.with_call_frame("normal-top-level", vec![], |i| {
        i.with_call_frame("command-line", vec![], |i| {
            i.with_call_frame("command-line-1", vec![cli_rest], run)
        })
    })
}

/// The signal-time snapshot includes evaluated calls and pending forms,
/// plus the host command-line scopes; no display-only padding is needed.
fn print_batch_backtrace(i: &mut Interp) {
    for frame in i.last_error_stack.borrow().iter().rev() {
        let name = match &frame.function {
            Value::Sym(id) => i.symbol_name(*id),
            other => i.prin1_to_string(other),
        };
        let args = frame
            .arguments
            .list_to_vec()
            .unwrap_or_default()
            .iter()
            .map(|value| i.prin1_to_string(value))
            .collect::<Vec<_>>()
            .join(" ");
        eprintln!("  {}({})", name, args);
    }
}

fn run_action(i: &mut Interp, source: &Value, cli_rest: Value, funcall: bool) -> bool {
    if i.output.is_none() {
        i.output = Some(OutputSink::Stdout);
    }
    let result = with_command_line_frames(i, cli_rest, |i| {
        if funcall {
            let function = crate::lisp::builtins::call_native_builtin(i, "intern", vec![source.clone()])?;
            i.apply(&function, vec![])
        } else {
            i.eval_first_object(source)
        }
    });
    match result {
        Ok(_) => true,
        Err(Flow::Exit(code)) => crate::lifecycle::exit(code),
        Err(flow) => {
            report_flow(i, flow);
            print_batch_backtrace(i);
            false
        }
    }
}

/// GNU's `-l'/`--script' CLI handlers resolve
/// `(file-truename (expand-file-name FILE))' before `load':
///   -l:       use file-ex  when (file-regular-p file-ex)
///   --script: use truename when (file-exists-p truename)
/// `load' itself keeps the tried path; only the CLI layer truenames.
fn cli_truename(i: &mut Interp, path: &str, regular_only: bool) -> String {
    let ex = crate::editor::expand_file_name_str(i, path);
    let real = crate::buffer::file_truename(&ex);
    let ok = std::fs::metadata(&real)
        .map(|m| !regular_only || m.is_file())
        .unwrap_or(false);
    if ok { real } else { ex }
}

fn load_file(i: &mut Interp, path: &str, cli_rest: Value) -> bool {
    if i.output.is_none() {
        i.output = Some(OutputSink::Stdout);
    }
    let path = &cli_truename(i, path, true);
    match with_command_line_frames(i, cli_rest, |i| crate::lisp::load::eval_file(i, path)) {
        Ok(_) => true,
        Err(Flow::Exit(code)) => crate::lifecycle::exit(code),
        Err(flow) => {
            report_flow(i, flow);
            print_batch_backtrace(i);
            false
        }
    }
}

/// `--script FILE`: batch load with `lexical-binding' forced on,
/// matching GNU's `command-line--load-script'.
fn load_file_script(i: &mut Interp, path: &str, cli_rest: Value) -> bool {
    i.noninteractive = true;
    if i.output.is_none() {
        i.output = Some(OutputSink::Stdout);
    }
    let nid = i.q("noninteractive");
    let _ = i.set_symbol_default(nid, Value::t());
    let path = &cli_truename(i, path, false);
    let tail = match cli_rest {
        Value::Cons(cell) => cell.borrow().cdr.clone(),
        _ => Value::Nil,
    };
    let script_args = Value::cons(Value::string("-scriptload"), tail);
    let result = with_command_line_frames(i, script_args, |i| {
        let function = Value::Sym(i.q("command-line--load-script"));
        i.apply(&function, vec![Value::string(path.clone())])
    });
    match result {
        Ok(_) => true,
        Err(Flow::Exit(code)) => crate::lifecycle::exit(code),
        Err(flow) => {
            report_flow(i, flow);
            print_batch_backtrace(i);
            false
        }
    }
}

fn report_flow(i: &mut Interp, flow: Flow) {
    match flow {
        Flow::Signal(signal) => {
            let sym = signal.symbol();
            let data = signal.data();
            // GNU's early debugger announces itself once before the
            // error message it is about to print.
            eprintln!("\ndebug-early-backtrace...done");
            let err_obj = signal.condition;
            let msg = crate::lisp::builtins::error_message(i, &err_obj);
            let name = match &sym {
                Value::Sym(id) => i.symbol_name(*id),
                _ => "error".into(),
            };
            let args = data
                .list_to_vec()
                .unwrap_or_default()
                .iter()
                .map(|v| i.prin1_to_string(v))
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
            crate::lifecycle::exit(code);
        }
    }
}
