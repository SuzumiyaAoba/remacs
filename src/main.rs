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
    let mut i = Interp::new();

    let mut batch = false;
    let mut no_window = false;
    let mut files: Vec<String> = Vec::new();
    let mut idx = 0;
    let mut exit = 0i32;

    while idx < args.len() {
        let arg = &args[idx];
        match arg.as_str() {
            "--version" => {
                println!("remacs 0.1 (Emacs-compatible editor in Rust)");
                return;
            }
            "--batch" | "-batch" | "-Q" | "--no-init" | "--quick" | "-q" => {
                batch = true;
            }
            "--eval" | "--execute" => {
                idx += 1;
                if idx < args.len() {
                    batch = true;
                    if !run_batch(&mut i, &args[idx]) {
                        exit = 1;
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
                    if !load_file(&mut i, &args[idx]) {
                        exit = 1;
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
                        exit = 1;
                    }
                    if let Err(flow) = i.eval_str("(run-hooks 'kill-emacs-hook)") {
                        report_flow(&mut i, flow);
                        exit = 1;
                    }
                    std::process::exit(exit);
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
        if let Err(flow) = i.eval_str("(run-hooks 'kill-emacs-hook)") {
            report_flow(&mut i, flow);
            exit = 1;
        }
        std::process::exit(exit);
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

fn run_batch(i: &mut Interp, src: &str) -> bool {
    i.noninteractive = true;
    if i.output.is_none() {
        i.output = Some(OutputSink::Stdout);
    }
    let nid = i.intern("noninteractive");
    let _ = i.set_symbol_default(nid, Value::t());
    match i.eval_str(src) {
        Ok(_) => true,
        Err(Flow::Exit(code)) => std::process::exit(code as i32),
        Err(flow) => {
            report_flow(i, flow);
            false
        }
    }
}

fn load_file(i: &mut Interp, path: &str) -> bool {
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
            // Reuse run_batch's error formatting on the stored flow.
            report_flow(i, flow);
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
