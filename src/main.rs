//! remacs — Emacs-compatible editor.
//!
//! Usage:
//!   remacs [options] [FILE...]
//!   --batch, -Q --no-init: batch mode (no TUI)
//!   --eval EXPR / --execute EXPR: evaluate Lisp
//!   --load FILE / -l FILE: load a Lisp file
//!   --script FILE: batch + load
//!   --version

use remacs::lisp::{Flow, Interp, OutputSink, Value};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    // Deep Lisp recursion (eager macroexpansion of nested backquotes
    // in e.g. transient.el) overflows the default thread stack; GNU's
    // C eval frames are far smaller.  Batch and terminal sessions run
    // the interpreter on a worker with a generous stack.  The AppKit
    // GUI must stay on the main thread, so only the headless path is
    // moved; gui.rs enlarges its logic thread separately.
    let headless = args.iter().any(|a| {
        matches!(
            a.as_str(),
            "-Q" | "--quick" | "-q" | "--no-init" | "--batch" | "-batch"
                | "--eval" | "--execute" | "--load" | "-l" | "--script"
                | "--ieval" | "-nw" | "--no-window-system" | "--version"
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
    let mut i = Interp::new();

    let mut batch = false;
    let mut no_window = false;
    let mut files: Vec<String> = Vec::new();
    let mut idx = 0;
    let exit = 0i32;

    while idx < args.len() {
        let arg = &args[idx];
        match arg.as_str() {
            "--version" => {
                println!("remacs 0.1 (Emacs-compatible editor in Rust)");
                return;
            }
            "-Q" | "--quick" => {
                batch = true;
                let sid = i.intern("inhibit-x-resources");
                let _ = i.set_symbol_default(sid, Value::t());
            }
            "-no-x-resources" | "--no-x-resources" => {
                let sid = i.intern("inhibit-x-resources");
                let _ = i.set_symbol_default(sid, Value::t());
            }
            "--batch" | "-batch" | "--no-init" | "-q" => {
                batch = true;
            }
            "--ieval" => {
                idx += 1;
                if idx < args.len() {
                    let src = args[idx].clone();
                    match i.eval_str(&src) {
                        Ok(_) => {}
                        Err(f) => {
                            eprintln!("eval_str err: {:?}; bisecting", f);
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
                                        eprintln!("FAIL at byte {pos}: {form_src} => {e:?}");
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
                no_window = true;
            }
            other => files.push(other.to_string()),
        }
        idx += 1;
    }

    if batch {
        // GNU's `command-line-1' ends batch processing with
        // (kill-emacs), which runs `kill-emacs-hook' before exiting.
        batch_exit(&mut i, exit);
    }

    if !no_window {
        // Graphical front-end: the evaluator boots on a logic thread
        // inside run_gui (Interp is !Send, AppKit needs main).
        return match remacs::gui::run_gui(&files) {
            Ok(()) => {}
            Err(e) => {
                eprintln!("remacs: gui error: {}", e);
                std::process::exit(1);
            }
        };
    }

    // Interactive terminal: visit files, run the editor loop.
    for f in &files {
        let form = format!("(find-file {:?})", f);
        let _ = i.eval_str(&form);
    }
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
    match i.eval_first_form(src) {
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

fn load_file(i: &mut Interp, path: &str, cli_rest: &[String]) -> bool {
    i.noninteractive = true;
    if i.output.is_none() {
        i.output = Some(OutputSink::Stdout);
    }
    let nid = i.intern("noninteractive");
    let _ = i.set_symbol_default(nid, Value::t());
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
