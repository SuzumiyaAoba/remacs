//! Shared command-line startup actions for the terminal and GUI.
mod actions;
mod diagnostics;
mod queue;
mod shutdown;

use crate::arguments::StartupOptions;
use actions::{Action, run_action, visit_file};
use diagnostics::{eval_bisect, print_batch_backtrace, report_flow};
use queue::{Argument, CommandLine, argument_text};
use remacs_runtime::host;
use remacs_runtime::lisp::{EvalResult, Flow, Interp, Value};
use shutdown::batch_exit;

/// Apply the live argument stream before starting an editor front-end.
/// Explicit exits, batch completion and command-line failures end the process.
pub fn run(i: &mut Interp, args: &[String]) {
    let mut batch = StartupOptions::scan(args).batch;
    // GNU captures this directory once before processing file arguments.
    // Visiting a file must not change the base for subsequent file names.
    let directory_symbol = host::symbol(i, "command-line-default-directory");
    let file_directory = i.symbol_value(directory_symbol);
    let mut queue = CommandLine::new(i, args);
    let mut load_path_options = remacs_runtime::lisp::load::CommandLineLoadPath::default();
    loop {
        let (argument, rest) = match queue.next(i) {
            Ok(Some(argument)) => argument,
            Ok(None) => break,
            Err(flow) => argument_error(i, flow),
        };
        let (name, operand) = match argument {
            Argument::File(file) => {
                inhibit_startup(i);
                visit_file(i, &file, &file_directory);
                continue;
            }
            Argument::Option { name, operand } => (name, operand),
        };
        match name {
            // -Q inhibits site/user init, the splash screen and X resources.
            // Init files and splash screens are not loaded by this startup.
            "-Q" | "--quick" | "--no-x-resources" => {
                let symbol = host::symbol(i, "inhibit-x-resources");
                let _ = i.set_symbol_default(symbol, Value::t());
            }
            "--batch" => batch = true,
            "--no-init" | "-q" | "-nw" => {}
            "--ieval" => eval_bisect(i, &argument_text(&operand.unwrap())),
            "-L" | "--directory" => {
                let directory = argument_text(&operand.unwrap());
                let result = with_command_line_frames(i, rest, |i| {
                    load_path_options.add(i, Value::string(&directory))
                });
                if let Err(flow) = result {
                    argument_error(i, flow);
                }
            }
            "--eval" | "--execute" | "-f" | "--funcall" => {
                // GNU inhibits the startup screen before evaluating/calling.
                inhibit_startup(i);
                let action = match name {
                    "--eval" | "--execute" => Action::Eval(operand.unwrap()),
                    _ => Action::Funcall(operand.unwrap()),
                };
                finish_action(i, action, rest);
            }
            "--load" | "-l" => {
                finish_action(i, Action::Load(argument_text(&operand.unwrap())), rest);
            }
            "--script" => {
                if !run_action(i, Action::Script(argument_text(&operand.unwrap())), rest) {
                    batch_exit(i, 255);
                }
                batch_exit(i, 0);
            }
            _ => {
                let flow = with_command_line_frames(i, rest, |i| {
                    Err(i.error(&format!("Unknown option `{name}'")))
                })
                .unwrap_err();
                argument_error(i, flow);
            }
        }
    }

    if batch {
        // GNU completes batch processing with kill-emacs, including its hook.
        batch_exit(i, 0);
    }
}

fn argument_error(i: &mut Interp, flow: Flow) -> ! {
    report_flow(i, flow);
    print_batch_backtrace(i);
    batch_exit(i, 255);
}

fn finish_action(i: &mut Interp, action: Action, rest: Value) {
    if !run_action(i, action, rest) {
        batch_exit(i, 255);
    }
    if i.quit_editor {
        remacs_runtime::lifecycle::exit(0);
    }
}

/// The varalias `inhibit-splash-screen' shares this cell.
fn inhibit_startup(i: &mut Interp) {
    let id = host::symbol(i, "inhibit-startup-screen");
    let _ = i.set_symbol_default(id, Value::t());
}

/// Run an actual command-line action with the Lisp-visible entry frames.
fn with_command_line_frames(
    i: &mut Interp,
    rest: Value,
    run: impl FnOnce(&mut Interp) -> EvalResult,
) -> EvalResult {
    i.with_call_frame("normal-top-level", vec![], |i| {
        i.with_call_frame("command-line", vec![], |i| {
            i.with_call_frame("command-line-1", vec![rest], run)
        })
    })
}
