//! remacs — Emacs-compatible editor.
//!
//! Usage:
//!   remacs [options] [FILE...]
//!   --batch: batch mode (no TUI)
//!   -Q / --no-init: skip initialization files
//!   --eval EXPR / --execute EXPR: evaluate Lisp
//!   --load FILE / -l FILE: load a Lisp file
//!   --script FILE: batch + load
//!   --version

use remacs::cli;

use remacs::lisp::{Flow, Interp, OutputSink, Value};

const HELP: &str = r#"Usage: remacs [OPTION...] [FILE...]
An Emacs-compatible editor.

Startup options:
  --help, -help                 Display this help and exit
  --version                    Display the version and exit
  --batch                      Run without an interactive editor
  -nw, --no-window-system       Use the terminal UI instead of the GUI
  -Q, --quick                   Skip init files and X resources
  -q, --no-init-file            Skip user init files
  --no-x-resources              Ignore X resources
  --dump-file FILE             Restore a portable Remacs image

Actions are processed in command-line order:
  --eval EXPR, --execute EXPR   Evaluate the first Lisp object in EXPR
  -f, --funcall FUNCTION        Call FUNCTION
  -l, --load FILE               Load a Lisp file or library
  -L, --directory DIR           Add DIR to load-path; ':DIR' appends it
  --script FILE                Load FILE with lexical binding, then exit
  FILE                         Visit FILE before processing later actions
  --                           Treat all following arguments as filenames

GNU single-hyphen long aliases and --OPTION=VALUE are accepted where supported.
For diagnostics, --ieval EXPR evaluates forms and locates an evaluation error.
"#;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    // GNU sets its `noninteractive' flag during main()'s argv scan —
    // before the interpreter is initialized — so init-time code like
    // `global-font-lock-mode''s `:init-value (not noninteractive)'
    // already sees the batch state.  Carry it to `Interp::new'.
    if cli::options(&args).any(|a| matches!(a, "--batch" | "-batch" | "--script")) {
        // SAFETY: set before Interp::new spawns work; the value is
        // only read during startup on this same thread anyway.
        unsafe { std::env::set_var("REMACS_NONINTERACTIVE", "1") };
    }
    // Deep Lisp recursion (eager macroexpansion of nested backquotes
    // in e.g. transient.el) overflows the default thread stack; GNU's
    // C eval frames are far smaller.  Batch and terminal sessions run
    // the interpreter on a worker with a generous stack.  The AppKit
    // GUI must stay on the main thread, so only the headless path is
    // moved; gui.rs enlarges its logic thread separately.
    let headless = cli::options(&args).any(|a| {
        matches!(
            a,
            "--batch"
                | "-batch"
                | "--eval"
                | "--execute"
                | "--load"
                | "-l"
                | "--script"
                | "--ieval"
                | "-f"
                | "--funcall"
                | "-nw"
                | "--no-window-system"
                | "--version"
                | "--help"
        )
    });
    if headless {
        run_headless(move || real_main(args));
        return;
    }
    real_main(args);
}

fn run_headless(run: impl FnOnce() + Send + 'static) {
    let child = std::thread::Builder::new()
        .name("remacs-main".into())
        .stack_size(512 * 1024 * 1024)
        .spawn(run)
        .expect("failed to spawn interpreter thread");
    if let Err(panic) = child.join() {
        // Preserve the worker panic and Rust's failure exit status;
        // returning normally here used to report success to the caller.
        std::panic::resume_unwind(panic);
    }
}

#[cfg(test)]
mod headless_tests {
    use super::run_headless;
    use std::sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    };

    #[test]
    fn waits_for_the_interpreter_worker_to_finish() {
        let completed = Arc::new(AtomicBool::new(false));
        let worker_completed = completed.clone();
        run_headless(move || worker_completed.store(true, Ordering::SeqCst));
        assert!(completed.load(Ordering::SeqCst));
    }

    #[test]
    fn worker_panic_propagates_after_unwinding_its_resources() {
        struct Cleanup(Arc<AtomicBool>);
        impl Drop for Cleanup {
            fn drop(&mut self) {
                self.0.store(true, Ordering::SeqCst);
            }
        }
        let cleaned = Arc::new(AtomicBool::new(false));
        let worker_cleaned = cleaned.clone();
        let panic = std::panic::catch_unwind(move || {
            run_headless(move || {
                let _cleanup = Cleanup(worker_cleaned);
                panic!("interpreter worker failure");
            })
        })
        .expect_err("worker panic was swallowed");
        assert_eq!(
            panic.downcast_ref::<&str>(),
            Some(&"interpreter worker failure")
        );
        assert!(cleaned.load(Ordering::SeqCst));
    }
}

fn real_main(args: Vec<String>) {
    // GNU prints the version without booting the interpreter.
    if cli::options(&args).any(|a| a == "--version") {
        println!("remacs 0.1 (Emacs-compatible editor in Rust)");
        return;
    }
    // Version has GNU's priority; help still respects explicit/sidecar image
    // validation, but can skip Lisp bootstrap when there is no image.
    let help_requested = cli::options(&args).any(|a| a == "--help");
    let (args, dump_file) = match cli::dump_file(&args) {
        Ok(values) => values,
        Err(error) => {
            eprintln!("remacs: {error}");
            std::process::exit(255);
        }
    };
    let dump_file = dump_file.or_else(|| {
        std::env::current_exe()
            .ok()
            .map(|p| p.with_extension("pdmp"))
            .filter(|p| p.is_file())
            .map(|p| p.to_string_lossy().into_owned())
    });
    if help_requested && dump_file.is_none() {
        print!("{HELP}");
        return;
    }

    // GUI dispatch BEFORE `Interp::new': run_gui spawns its own
    // interpreter on the logic thread, so building one here would be
    // a thrown-away ~30s prelude — and any arg that forces batch or
    // the terminal front-end keeps this process headless.
    let batch_arg = help_requested
        || cli::options(&args).any(|a| {
            matches!(
                a,
                "--batch"
                    | "-batch"
                    | "--eval"
                    | "--execute"
                    | "--load"
                    | "-l"
                    | "--script"
                    | "--ieval"
                    | "-f"
                    | "--funcall"
            )
        });
    let no_window_arg = cli::options(&args).any(|a| matches!(a, "-nw" | "--no-window-system"));
    if !batch_arg && !no_window_arg {
        let files: Vec<String> = cli::arguments(&args)
            .filter_map(|arg| match arg {
                cli::Argument::File(name) => Some(name.to_string()),
                cli::Argument::Option { .. } => None,
            })
            .collect();
        return match remacs::gui::run_gui_with_dump(&files, dump_file.as_deref(), &args) {
            Ok(()) => {}
            Err(e) => {
                eprintln!("remacs: gui error: {}", e);
                std::process::exit(1);
            }
        };
    }

    let mut i = match dump_file {
        None => Interp::new(),
        Some(path) => {
            match remacs::lisp::dump::load_with_arguments(std::path::Path::new(&path), Some(&args))
            {
                Ok(interpreter) => interpreter,
                Err(remacs::lisp::dump::LoadError::Exit(code)) => std::process::exit(code as i32),
                Err(error) => {
                    eprintln!("remacs: {error}");
                    std::process::exit(255);
                }
            }
        }
    };
    if help_requested {
        print!("{HELP}");
        return;
    }

    // `command-line-args'/`invocation-name'/`invocation-directory' are
    // seeded in Interp::new (GNU init_cmdargs placement).

    let mut batch = false;
    let exit = 0i32;
    // GNU captures this directory once before processing file arguments.
    // Visiting a file changes its buffer's default-directory, but must not
    // change the base used for the next command-line file name.
    let directory_symbol = i.intern("command-line-default-directory");
    let file_directory = i.symbol_value(directory_symbol);

    let left_symbol = i.intern("command-line-args-left");
    let _ = i.set_symbol_default(
        left_symbol,
        Value::list(args.iter().map(Value::string).collect()),
    );
    let mut parse_options = true;
    let mut load_path_options = remacs::lisp::load::CommandLineLoadPath::default();
    loop {
        let cli_rest = i.symbol_value(left_symbol);
        if cli_rest.is_nil() {
            break;
        }
        let token = match with_command_line_frames(&mut i, cli_rest.clone(), |i| {
            pop_cli_string(i, left_symbol)
        }) {
            Ok(Value::Str(text)) => text.borrow().clone(),
            Ok(_) => unreachable!(),
            Err(flow) => {
                report_flow(&mut i, flow);
                print_batch_backtrace(&mut i);
                batch_exit(&mut i, 255);
            }
        };
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
            inhibit_startup(&mut i);
            let result = (|| {
                let expand = Value::Sym(i.intern("expand-file-name"));
                let file = i.apply(&expand, vec![Value::string(&token), file_directory.clone()])?;
                let visit = Value::Sym(i.intern("find-file"));
                i.apply(&visit, vec![file])
            })();
            match result {
                Ok(_) => {}
                Err(Flow::Exit(code)) => std::process::exit(code as i32),
                Err(flow) => {
                    report_flow(&mut i, flow);
                    batch_exit(&mut i, 255);
                }
            }
            continue;
        }
        let option = option.expect("recognized option");
        let mut args = vec![token.clone()];
        if let Some(value) = option.value {
            args.push(value.to_owned());
        } else if cli::takes_operand(&token) {
            match with_command_line_frames(&mut i, cli_rest.clone(), |i| {
                pop_cli_string(i, left_symbol)
            }) {
                Ok(Value::Str(text)) => args.push(text.borrow().clone()),
                Ok(_) => unreachable!(),
                Err(flow) => {
                    report_flow(&mut i, flow);
                    print_batch_backtrace(&mut i);
                    batch_exit(&mut i, 255);
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
                let sid = i.intern("inhibit-x-resources");
                let _ = i.set_symbol_default(sid, Value::t());
            }
            "-no-x-resources" | "--no-x-resources" => {
                let sid = i.intern("inhibit-x-resources");
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
                                remacs::lisp::Flow::Signal(s, dd, _) => format!(
                                    "signal {} {}",
                                    i.prin1_to_string(s),
                                    i.prin1_to_string(dd)
                                ),
                                other => format!("{other:?}"),
                            };
                            eprintln!("eval_str err: {d}; bisecting");
                            let chars: std::rc::Rc<Vec<char>> =
                                std::rc::Rc::new(src.chars().collect());
                            let mut pos = 0usize;
                            loop {
                                let next = {
                                    let mut r = remacs::lisp::reader::Reader::with_chars(
                                        &mut i,
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
                                    let mut r = remacs::lisp::reader::Reader::new(&mut i, form_src);
                                    match r.read() {
                                        Ok(Some(v)) => v,
                                        _ => break,
                                    }
                                };
                                match i.eval(&form) {
                                    Err(e) => {
                                        let d = match &e {
                                            remacs::lisp::Flow::Signal(s, dd, _) => format!(
                                                "signal {} {}",
                                                i.prin1_to_string(s),
                                                i.prin1_to_string(dd)
                                            ),
                                            other => format!("{other:?}"),
                                        };
                                        eprintln!("FAIL at byte {pos}: {form_src} => {d}");
                                        break;
                                    }
                                    Ok(_) => pos = next,
                                }
                            }
                            std::process::exit(255);
                        }
                    }
                    std::process::exit(exit);
                }
            }
            "-L" | "--directory" => {
                let result = with_command_line_frames(&mut i, cli_rest, |i| {
                    load_path_options.add(i, Value::string(&args[1]))
                });
                if let Err(flow) = result {
                    report_flow(&mut i, flow);
                    print_batch_backtrace(&mut i);
                    batch_exit(&mut i, 255);
                }
            }
            "--eval" | "--execute" => {
                idx += 1;
                if idx < args.len() {
                    batch = true;
                    // GNU `command-line-1': -eval/-execute set
                    // `inhibit-startup-screen' before evaluating.
                    inhibit_startup(&mut i);
                    if !run_batch(&mut i, &args[idx], cli_rest.clone(), false) {
                        // GNU's `command-line-1': a signaled error
                        // aborts the remaining args and exits 255
                        // (after running `kill-emacs-hook').
                        batch_exit(&mut i, 255);
                    }
                    if i.quit_editor {
                        std::process::exit(exit);
                    }
                }
            }
            "--load" | "-l" => {
                idx += 1;
                if idx < args.len() {
                    batch = true;
                    if !load_file(&mut i, &args[idx], cli_rest.clone()) {
                        batch_exit(&mut i, 255);
                    }
                    if i.quit_editor {
                        std::process::exit(exit);
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
                    batch = true;
                    // GNU `command-line-1': -f/-funcall inhibit too.
                    inhibit_startup(&mut i);
                    if !run_batch(&mut i, &args[idx], cli_rest.clone(), true) {
                        batch_exit(&mut i, 255);
                    }
                    if i.quit_editor {
                        std::process::exit(exit);
                    }
                    // The next iteration reads the live Lisp value. Functions
                    // may consume, replace, or mutate any remaining arguments.
                }
            }
            "--script" => {
                idx += 1;
                if idx < args.len() {
                    if !load_file_script(&mut i, &args[idx], cli_rest.clone()) {
                        batch_exit(&mut i, 255);
                    }
                    batch_exit(&mut i, exit);
                }
            }
            "-nw" | "--no-window-system" => {
                // Already accounted for by the pre-`Interp::new'
                // dispatch (this process only runs headless paths).
            }
            _ => {
                let flow = with_command_line_frames(&mut i, cli_rest, |i| {
                    Err(i.error(&format!("Unknown option `{arg}'")))
                })
                .unwrap_err();
                report_flow(&mut i, flow);
                print_batch_backtrace(&mut i);
                batch_exit(&mut i, 255);
            }
        }
    }

    if batch {
        // GNU's `command-line-1' ends batch processing with
        // (kill-emacs), which runs `kill-emacs-hook' before exiting.
        batch_exit(&mut i, exit);
    }

    // Reaching here means a batch flag or -nw was given; with a batch
    // flag we already exited above, and the genuine GUI case was
    // dispatched before Interp::new on the main thread.  What remains
    // is the terminal editor.

    // Interactive terminal: files have been visited; run the editor loop.
    match remacs::term::run_editor(&mut i) {
        Ok(()) => {}
        Err(e) => {
            eprintln!("remacs: terminal error: {}", e);
            std::process::exit(1);
        }
    }
}

/// Run `kill-emacs-hook' (reporting but ignoring its errors) and exit
/// with CODE — GNU's `kill-emacs' on every batch exit path.
fn batch_exit(i: &mut Interp, code: i32) -> ! {
    if let Err(flow) = i.eval_str("(run-hooks 'kill-emacs-hook)") {
        report_flow(i, flow);
    }
    std::process::exit(code);
}

/// GNU `command-line-1' sets `inhibit-startup-screen' for -eval/-f/
/// -insert/-visit args and for any file arg when no window system —
/// always true on this headless build.  The varalias
/// `inhibit-splash-screen' shares the cell, so one set covers both.
fn inhibit_startup(i: &mut Interp) {
    let id = i.intern("inhibit-startup-screen");
    let _ = i.set_symbol_default(id, Value::t());
}

/// Pop one argument from the live Lisp queue. Validate lazily so a bad
/// later element does not prevent earlier actions from running.
fn pop_cli_string(i: &mut Interp, left: remacs::lisp::SymId) -> remacs::lisp::EvalResult {
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
    run: impl FnOnce(&mut Interp) -> remacs::lisp::EvalResult,
) -> remacs::lisp::EvalResult {
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

fn run_batch(i: &mut Interp, src: &str, cli_rest: Value, funcall: bool) -> bool {
    i.noninteractive = true;
    if i.output.is_none() {
        i.output = Some(OutputSink::Stdout);
    }
    let nid = i.intern("noninteractive");
    let _ = i.set_symbol_default(nid, Value::t());
    let result = with_command_line_frames(i, cli_rest, |i| {
        if funcall {
            let function = Value::Sym(i.intern(src));
            i.apply(&function, vec![])
        } else {
            i.eval_first_form(src)
        }
    });
    match result {
        Ok(_) => true,
        Err(Flow::Exit(code)) => std::process::exit(code as i32),
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
    let ex = remacs::editor::expand_file_name_str(i, path);
    let real = remacs::buffer::file_truename(&ex);
    let ok = std::fs::metadata(&real)
        .map(|m| !regular_only || m.is_file())
        .unwrap_or(false);
    if ok { real } else { ex }
}

fn load_file(i: &mut Interp, path: &str, cli_rest: Value) -> bool {
    i.noninteractive = true;
    if i.output.is_none() {
        i.output = Some(OutputSink::Stdout);
    }
    let nid = i.intern("noninteractive");
    let _ = i.set_symbol_default(nid, Value::t());
    let path = &cli_truename(i, path, true);
    match with_command_line_frames(i, cli_rest, |i| remacs::lisp::load::eval_file(i, path)) {
        Ok(_) => true,
        Err(Flow::Exit(code)) => std::process::exit(code as i32),
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
    let nid = i.intern("noninteractive");
    let _ = i.set_symbol_default(nid, Value::t());
    let path = &cli_truename(i, path, false);
    let tail = match cli_rest {
        Value::Cons(cell) => cell.borrow().cdr.clone(),
        _ => Value::Nil,
    };
    let script_args = Value::cons(Value::string("-scriptload"), tail);
    let result = with_command_line_frames(i, script_args, |i| {
        let function = Value::Sym(i.intern("command-line--load-script"));
        i.apply(&function, vec![Value::string(path.clone())])
    });
    match result {
        Ok(_) => true,
        Err(Flow::Exit(code)) => std::process::exit(code as i32),
        Err(flow) => {
            report_flow(i, flow);
            print_batch_backtrace(i);
            false
        }
    }
}

fn report_flow(i: &mut Interp, flow: Flow) {
    match flow {
        Flow::Signal(sym, data, _) => {
            // GNU's early debugger announces itself once before the
            // error message it is about to print.
            eprintln!("\ndebug-early-backtrace...done");
            let err_obj = Value::cons(sym.clone(), data.clone());
            let msg = remacs::lisp::builtins::error_message(i, &err_obj);
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
            std::process::exit(code as i32);
        }
    }
}
