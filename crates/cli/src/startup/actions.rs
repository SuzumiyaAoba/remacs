//! Evaluation, function calls, and file-loading startup actions.
use super::*;

pub(super) fn run_action(i: &mut Interp, source: &Value, cli_rest: Value, funcall: bool) -> bool {
    if i.output.is_none() {
        i.output = Some(OutputSink::Stdout);
    }
    let result = with_command_line_frames(i, cli_rest, |i| {
        if funcall {
            let function = host::argument_function(i, source.clone())?;
            i.apply(&function, vec![])
        } else {
            host::eval_first_object(i, source)
        }
    });
    match result {
        Ok(_) => true,
        Err(Flow::Exit(code)) => remacs_runtime::lifecycle::exit(code),
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
pub(super) fn cli_truename(i: &mut Interp, path: &str, regular_only: bool) -> String {
    let ex = remacs_runtime::editor::expand_file_name_str(i, path);
    let real = remacs_runtime::buffer::file_truename(&ex);
    let ok = std::fs::metadata(&real)
        .map(|m| !regular_only || m.is_file())
        .unwrap_or(false);
    if ok { real } else { ex }
}

pub(super) fn load_file(i: &mut Interp, path: &str, cli_rest: Value) -> bool {
    if i.output.is_none() {
        i.output = Some(OutputSink::Stdout);
    }
    let path = &cli_truename(i, path, true);
    match with_command_line_frames(i, cli_rest, |i| {
        remacs_runtime::lisp::load::eval_file(i, path)
    }) {
        Ok(_) => true,
        Err(Flow::Exit(code)) => remacs_runtime::lifecycle::exit(code),
        Err(flow) => {
            report_flow(i, flow);
            print_batch_backtrace(i);
            false
        }
    }
}

/// `--script FILE`: batch load with `lexical-binding' forced on,
/// matching GNU's `command-line--load-script'.
pub(super) fn load_file_script(i: &mut Interp, path: &str, cli_rest: Value) -> bool {
    i.noninteractive = true;
    if i.output.is_none() {
        i.output = Some(OutputSink::Stdout);
    }
    let nid = host::symbol(i, "noninteractive");
    let _ = i.set_symbol_default(nid, Value::t());
    let path = &cli_truename(i, path, false);
    let tail = match cli_rest {
        Value::Cons(cell) => cell.borrow().cdr.clone(),
        _ => Value::Nil,
    };
    let script_args = Value::cons(Value::string("-scriptload"), tail);
    let result = with_command_line_frames(i, script_args, |i| {
        let function = Value::Sym(host::symbol(i, "command-line--load-script"));
        i.apply(&function, vec![Value::string(path.clone())])
    });
    match result {
        Ok(_) => true,
        Err(Flow::Exit(code)) => remacs_runtime::lifecycle::exit(code),
        Err(flow) => {
            report_flow(i, flow);
            print_batch_backtrace(i);
            false
        }
    }
}
