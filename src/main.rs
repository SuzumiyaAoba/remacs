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

mod cli;

use remacs::lisp::{Flow, Interp, OutputSink, Value};

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
        )
    });
    if headless {
        let child = std::thread::Builder::new()
            .name("remacs-main".into())
            .stack_size(512 * 1024 * 1024)
            .spawn(move || real_main(args))
            .expect("failed to spawn interpreter thread");
        let _ = child.join();
        return;
    }
    real_main(args);
}

fn real_main(args: Vec<String>) {
    // GNU prints the version without booting the interpreter.
    if cli::options(&args).any(|a| a == "--version") {
        println!("remacs 0.1 (Emacs-compatible editor in Rust)");
        return;
    }

    // GUI dispatch BEFORE `Interp::new': run_gui spawns its own
    // interpreter on the logic thread, so building one here would be
    // a thrown-away ~30s prelude — and any arg that forces batch or
    // the terminal front-end keeps this process headless.
    let batch_arg = cli::options(&args).any(|a| {
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
        return match remacs::gui::run_gui(&files) {
            Ok(()) => {}
            Err(e) => {
                eprintln!("remacs: gui error: {}", e);
                std::process::exit(1);
            }
        };
    }

    let mut i = Interp::new();

    // `command-line-args'/`invocation-name'/`invocation-directory' are
    // seeded in Interp::new (GNU init_cmdargs placement).

    let mut batch = false;
    let exit = 0i32;

    let mut arguments = cli::arguments(&args);
    while let Some(argument) = arguments.next() {
        let (mut idx, arg) = match argument {
            cli::Argument::Option { index, name } => (index, name),
            cli::Argument::File(name) => {
                // Visit files in command-line order, including in batch
                // mode, so later actions and exit hooks see them.
                inhibit_startup(&mut i);
                let form = format!("(find-file {:?})", name);
                match i.eval_str(&form) {
                    Ok(_) => {}
                    Err(Flow::Exit(code)) => std::process::exit(code as i32),
                    Err(flow) => {
                        report_flow(&mut i, flow);
                        batch_exit(&mut i, 255);
                    }
                }
                continue;
            }
        };
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
            "--eval" | "--execute" => {
                idx += 1;
                if idx < args.len() {
                    batch = true;
                    // GNU `command-line-1': -eval/-execute set
                    // `inhibit-startup-screen' before evaluating.
                    inhibit_startup(&mut i);
                    if !run_batch(&mut i, &args[idx], &args[idx - 1..]) {
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
                    if !load_file(&mut i, &args[idx], &args[idx - 1..]) {
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
                    let left: Vec<Value> = args[idx + 1..]
                        .iter()
                        .map(|a| Value::string(a.clone()))
                        .collect();
                    let lid = i.intern("command-line-args-left");
                    let _ = i.set_symbol_default(lid, Value::list(left));
                    let form = format!("(funcall (intern {:?}))", args[idx]);
                    if !run_batch(&mut i, &form, &args[idx - 1..]) {
                        batch_exit(&mut i, 255);
                    }
                    if i.quit_editor {
                        std::process::exit(exit);
                    }
                    // Functions such as batch-native-compile consume
                    // filenames themselves.  Do not visit those again.
                    if let Ok(left) = i.symbol_value(lid).list_to_vec() {
                        let remaining: Option<Vec<String>> = left
                            .iter()
                            .map(|value| {
                                if let Value::Str(text) = value {
                                    Some(text.borrow().clone())
                                } else {
                                    None
                                }
                            })
                            .collect();
                        if let Some(remaining) = remaining {
                            arguments.retain_remaining(&remaining);
                        }
                    }
                }
            }
            "--script" => {
                idx += 1;
                if idx < args.len() {
                    if !load_file_script(&mut i, &args[idx]) {
                        batch_exit(&mut i, 255);
                    }
                    batch_exit(&mut i, exit);
                }
            }
            "-nw" | "--no-window-system" => {
                // Already accounted for by the pre-`Interp::new'
                // dispatch (this process only runs headless paths).
            }
            _ => unreachable!("CLI parser returned an unhandled option"),
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
    // is the terminal editor — including a trailing flag that lost
    // its operand (e.g. `remacs --eval' with no form), where GNU
    // would still open an interactive session.

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

/// GNU batch error backtrace tail: the middle frame (`eval(FORM t)'
/// for --eval, `load-with-code-conversion(...)' for -l), then the
/// `command-line-1'/`command-line'/`normal-top-level' frames.
fn print_batch_backtrace(i: &mut Interp, mid_frame: Option<String>, cli_rest: &[String]) {
    // Lisp frames innermost-first, like GNU's `backtrace'.
    for (fun, argv) in i.last_error_stack.borrow().iter().rev() {
        let name = match fun {
            Value::Sym(id) => i.symbol_name(*id),
            other => i.prin1_to_string(other),
        };
        let args = argv
            .iter()
            .map(|v| i.prin1_to_string(v))
            .collect::<Vec<_>>()
            .join(" ");
        eprintln!("  {}({})", name, args);
    }
    if let Some(frame) = mid_frame {
        eprintln!("  {}", frame);
    }
    let quoted = cli_rest
        .iter()
        .map(|a| format!("{:?}", a))
        .collect::<Vec<_>>()
        .join(" ");
    eprintln!("  command-line-1(({}))", quoted);
    eprintln!("  command-line()");
    eprintln!("  normal-top-level()");
}

fn run_batch(i: &mut Interp, src: &str, cli_rest: &[String]) -> bool {
    i.noninteractive = true;
    if i.output.is_none() {
        i.output = Some(OutputSink::Stdout);
    }
    let nid = i.intern("noninteractive");
    let _ = i.set_symbol_default(nid, Value::t());
    match i.eval_forms(src) {
        Ok(_) => true,
        Err(Flow::Exit(code)) => std::process::exit(code as i32),
        Err(flow) => {
            report_flow(i, flow);
            // GNU's eval frame shows the read form and the lexical env.
            let mid = i
                .read_from_string(src, 0)
                .ok()
                .map(|(form, _)| format!("eval({} t)", i.prin1_to_string(&form)));
            print_batch_backtrace(i, mid, cli_rest);
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

fn load_file(i: &mut Interp, path: &str, cli_rest: &[String]) -> bool {
    i.noninteractive = true;
    if i.output.is_none() {
        i.output = Some(OutputSink::Stdout);
    }
    let nid = i.intern("noninteractive");
    let _ = i.set_symbol_default(nid, Value::t());
    let path = &cli_truename(i, path, true);
    match remacs::lisp::load::eval_file(i, path) {
        Ok(_) => true,
        Err(Flow::Exit(code)) => std::process::exit(code as i32),
        Err(flow) => {
            report_flow(i, flow);
            // GNU resolves the truename; display both like
            // load-with-code-conversion does.
            let real = std::fs::canonicalize(path)
                .map(|p| p.display().to_string())
                .unwrap_or_else(|_| path.to_string());
            let mid = Some(format!(
                "load-with-code-conversion({:?} {:?} nil t)",
                real, real
            ));
            print_batch_backtrace(i, mid, cli_rest);
            false
        }
    }
}

/// `--script FILE`: batch load with `lexical-binding' forced on,
/// matching GNU's `command-line--load-script'.
fn load_file_script(i: &mut Interp, path: &str) -> bool {
    i.noninteractive = true;
    if i.output.is_none() {
        i.output = Some(OutputSink::Stdout);
    }
    let nid = i.intern("noninteractive");
    let _ = i.set_symbol_default(nid, Value::t());
    let path = &cli_truename(i, path, false);
    match remacs::lisp::load::eval_file_script(i, path) {
        Ok(_) => true,
        Err(Flow::Exit(code)) => std::process::exit(code as i32),
        Err(flow) => {
            report_flow(i, flow);
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
