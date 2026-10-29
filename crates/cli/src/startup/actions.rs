//! Evaluation, function calls, and file-loading startup actions.
use super::diagnostics::report_startup_flow;
use super::with_command_line_frames;
use remacs_runtime::host;
use remacs_runtime::lisp::{Flow, Interp, OutputSink, Value};

pub(super) enum Action {
    Eval(Value),
    Funcall(Value),
    Load(String),
    Script(String),
}

pub(super) fn run_action(i: &mut Interp, action: Action, rest: Value, batch: bool) -> bool {
    if matches!(action, Action::Script(_)) {
        i.noninteractive = true;
    }
    if i.output.is_none() {
        i.output = Some(OutputSink::Stdout);
    }
    // Resolve paths before entering the Lisp-visible action frames.
    let (action, rest) = match action {
        Action::Load(path) => (Action::Load(cli_truename(i, &path, true)), rest),
        Action::Script(path) => {
            let noninteractive = host::symbol(i, "noninteractive");
            let _ = i.set_symbol_default(noninteractive, Value::t());
            let path = cli_truename(i, &path, false);
            let tail = match rest {
                Value::Cons(cell) => cell.borrow().cdr.clone(),
                _ => Value::Nil,
            };
            let script_args = Value::cons(Value::string("-scriptload"), tail);
            (Action::Script(path), script_args)
        }
        action => (action, rest),
    };
    let result = with_command_line_frames(i, rest, |i| match action {
        Action::Eval(source) => host::eval_first_object(i, &source),
        Action::Funcall(source) => {
            let function = host::argument_function(i, source)?;
            i.apply(&function, vec![])
        }
        Action::Load(path) => remacs_runtime::lisp::load::eval_file(i, &path),
        // GNU command-line--load-script forces lexical binding.
        Action::Script(path) => {
            let function = Value::Sym(host::symbol(i, "command-line--load-script"));
            i.apply(&function, vec![Value::string(path)])
        }
    });
    match result {
        Ok(_) => true,
        Err(Flow::Exit(code)) => remacs_runtime::lifecycle::exit(code),
        Err(flow) => report_startup_flow(i, flow, batch),
    }
}

/// File visits use the captured startup directory, outside action frames.
pub(super) fn visit_file(i: &mut Interp, file: &str, directory: &Value, batch: bool) -> bool {
    let result = (|| {
        let expand = Value::Sym(host::symbol(i, "expand-file-name"));
        let file = i.apply(&expand, vec![Value::string(file), directory.clone()])?;
        let visit = Value::Sym(host::symbol(i, "find-file"));
        i.apply(&visit, vec![file])
    })();
    match result {
        Ok(_) => true,
        Err(Flow::Exit(code)) => remacs_runtime::lifecycle::exit(code),
        Err(flow) => report_startup_flow(i, flow, batch),
    }
}

/// GNU's -l/--script resolve (file-truename (expand-file-name FILE)).
/// -l uses the expanded name unless the truename is a regular file;
/// --script uses it whenever the truename exists. `load' itself keeps
/// the tried path; only the CLI layer truenames.
fn cli_truename(i: &mut Interp, path: &str, regular_only: bool) -> String {
    let expanded = remacs_runtime::editor::expand_file_name_str(i, path);
    let real = remacs_runtime::buffer::file_truename(&expanded);
    let exists = real
        .to_os_string()
        .ok()
        .and_then(|path| std::fs::metadata(path).ok())
        .map(|metadata| !regular_only || metadata.is_file())
        .unwrap_or(false);
    if exists {
        real.to_unicode_lossy()
    } else {
        expanded.to_unicode_lossy()
    }
}
