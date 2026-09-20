//! Evaluation-related subrs: eval, apply, funcall, signal, error, throw,
//! featurep, run-hooks, etc.

use super::{S, arg, want_list, want_sym};
use crate::lisp::Interp;
use crate::lisp::error::{EvalResult, Flow};
use crate::lisp::obarray::sym;
use crate::lisp::value::{Subr, Value};

pub(crate) static SUBRS: &[Subr] = &[
    S!("eval", 1, 2, f_eval, "Evaluate FORM and return its value."),
    S!("apply", many 1, f_apply, "Call FUNCTION with args; last arg is a list."),
    S!("funcall", many 1, f_funcall, "Call FUNCTION with the given args."),
    S!("funcall-interactively", many 1, f_funcall_interactively, "Like funcall (for commands)."),
    S!(
        "function",
        raw,
        f_function_raw,
        "Return the function denoted by ARG."
    ),
    S!(
        "macroexpand",
        1,
        2,
        f_macroexpand,
        "Expand a macro call FORM."
    ),
    S!(
        "macroexpand-all",
        1,
        2,
        f_macroexpand_all,
        "Recursively expand all macros in FORM."
    ),
    S!(
        "macroexpand-1",
        1,
        2,
        f_macroexpand_1,
        "Expand a macro call once."
    ),
    S!(
        "signal",
        2,
        2,
        f_signal,
        "Signal an error (ERROR-SYMBOL . DATA)."
    ),
    S!("error", many 1, f_error, "Signal an error with a formatted message."),
    S!("user-error", many 1, f_user_error, "Signal a user-error."),
    S!("throw", 2, 2, f_throw, "Throw to TAG with VALUE."),
    S!(
        "condition-case",
        raw,
        f_condition_case_raw,
        "Handled by special form dispatch."
    ),
    S!(
        "ignore-error",
        raw,
        f_ignore_error_raw,
        "Eval body ignoring errors."
    ),
    S!(
        "with-demoted-errors",
        raw,
        f_with_demoted_errors,
        "Like ignore-error but reports."
    ),
    S!(
        "ignore-errors",
        raw,
        f_ignore_error_raw,
        "Eval body ignoring errors."
    ),
    S!("featurep", 1, 2, f_featurep, "t if FEATURE is provided."),
    S!(
        "provide",
        1,
        2,
        f_provide,
        "Add FEATURE to the features list."
    ),
    S!(
        "require",
        1,
        3,
        f_require,
        "Require FEATURE (load if needed)."
    ),
    S!("run-hooks", many 0, f_run_hooks, "Run each named hook variable."),
    S!("run-hook-with-args", many 1, f_run_hook_with_args, "Run HOOK with ARGS."),
    S!("run-hook-with-args-until-failure", many 1, f_run_hook_until_fail, "Run HOOK until nil."),
    S!("run-hook-with-args-until-success", many 1, f_run_hook_until_success, "Run HOOK until non-nil."),
    S!(
        "run-hook-wrapped",
        4,
        4,
        f_run_hook_wrapped,
        "Run HOOK with wrapper function."
    ),
    S!(
        "add-hook",
        2,
        4,
        f_add_hook,
        "Add FUNCTION to HOOK variable."
    ),
    S!(
        "remove-hook",
        2,
        3,
        f_remove_hook,
        "Remove FUNCTION from HOOK variable."
    ),
    S!("identity", 1, 1, f_identity, "Return ARG."),
    S!("ignore", many 0, f_ignore, "Do nothing, return nil."),
    S!("always", many 0, f_always, "Do nothing, return t."),
    S!("prog1", raw, f_prog1_raw, ""),
    S!("or", raw, f_or_raw, ""),
    S!("and", raw, f_and_raw, ""),
    S!("values", many 0, f_values, "Return the list of arguments (multiple values stub)."),
    S!(
        "eval-expression",
        1,
        2,
        f_eval_expression,
        "Eval EXPR like M-:."
    ),
    S!("load", 1, 5, f_load, "Load a Lisp file."),
    S!("load-file", 1, 1, f_load_file, "Load FILE."),
    S!(
        "locate-library",
        1,
        4,
        f_locate_library,
        "Find LIBRARY on load-path."
    ),
    S!("autoload", 2, 5, f_autoload, "Declare FUNCTION autoloaded."),
    S!(
        "autoloadp",
        1,
        1,
        f_autoloadp,
        "t if OBJECT is an autoload object."
    ),
    S!(
        "eval-buffer",
        0,
        5,
        f_eval_buffer,
        "Eval BUFFER's contents as elisp."
    ),
    S!(
        "eval-region",
        2,
        4,
        f_eval_region,
        "Eval text between START and END."
    ),
    S!(
        "command-execute",
        1,
        4,
        f_command_execute,
        "Execute CMD interactively."
    ),
    S!(
        "called-interactively-p",
        0,
        1,
        f_called_interactively_p,
        "t if called interactively."
    ),
    S!(
        "funcall-with-delayed-message",
        2,
        2,
        f_funcall_with_delayed_message,
        "Call FUNCTION, show message."
    ),
    S!(
        "declare-function",
        raw,
        f_declare_function,
        "Declare external function (no-op)."
    ),
    S!("declare", raw, f_declare, "Declare (no-op)."),
    S!(
        "eval-and-compile",
        raw,
        f_eval_and_compile,
        "Eval body now and at compile time."
    ),
    S!(
        "eval-when-compile",
        raw,
        f_eval_when_compile,
        "Eval body at compile time only."
    ),
    S!(
        "with-no-warnings",
        raw,
        f_with_no_warnings,
        "Eval body without warnings."
    ),
    S!(
        "display-warning",
        2,
        4,
        f_display_warning,
        "Display a warning message."
    ),
    S!("lwarn", 4, 4, f_lwarn, "Display a warning."),
    S!("warn", many 1, f_warn, "Display a warning."),
    S!("message", many 1, f_message, "Display a message in the echo area."),
    S!("ding", 0, 1, f_ding, "Beep."),
    S!("beep", 0, 1, f_ding, "Beep."),
    S!(
        "sleep-for",
        1,
        2,
        f_sleep_for,
        "Sleep SECONDS (+ MILLISECONDS)."
    ),
    S!("sit-for", 1, 3, f_sit_for, "Wait SECONDS or until input."),
    S!(
        "current-time",
        0,
        0,
        f_current_time,
        "Current time as (HIGH LOW USEC PSEC)."
    ),
    S!(
        "current-time-string",
        0,
        1,
        f_current_time_string,
        "Current time as a string."
    ),
    S!(
        "current-time-zone",
        0,
        2,
        f_current_time_zone,
        "Time zone info."
    ),
    S!("float-time", 0, 1, f_float_time, "Time as float seconds."),
    S!(
        "format-time-string",
        1,
        3,
        f_format_time_string,
        "Format time per FORMAT."
    ),
    S!("get-internal-run-time", 0, 0, f_current_time, ""),
    S!(
        "garbage-collect",
        0,
        0,
        f_garbage_collect,
        "GC stats (Rc-based, informational)."
    ),
    S!(
        "memory-info",
        0,
        0,
        f_memory_info,
        "Memory info (informational)."
    ),
    S!("kill-emacs", 0, 2, f_kill_emacs, "Exit remacs."),
    S!(
        "recursive-edit",
        0,
        0,
        f_recursive_edit,
        "Recursive editing level."
    ),
    S!("top-level", 0, 0, f_top_level, "Return to top level."),
    S!(
        "exit-recursive-edit",
        0,
        0,
        f_exit_recursive_edit,
        "Exit a recursive edit level."
    ),
    S!(
        "abort-recursive-edit",
        0,
        0,
        f_abort_recursive_edit,
        "Abort a recursive edit level."
    ),
    S!(
        "recursion-depth",
        0,
        0,
        f_recursion_depth,
        "Current recursive-edit depth."
    ),
    S!("emacs-pid", 0, 0, f_emacs_pid, "Process id."),
    S!("system-name", 0, 0, f_system_name, "Host name."),
    S!("emacs-version", 0, 0, f_emacs_version, "Version string."),
    S!("emacs-build-time", 0, 0, f_emacs_build_time, "Build time."),
    S!("set-message-functions", 0, 0, f_noop, ""),
    S!("set-fill-prefix", 0, 0, f_noop, ""),
    S!(
        "internal-make-interpreted-closure-function",
        3,
        3,
        f_internal_make_closure,
        "Make a lexical closure."
    ),
    S!("internal--set-subr-doc", 0, 0, f_noop, ""),
    S!(
        "macroexp-parse-body",
        1,
        1,
        f_macroexp_parse_body,
        "Parse body into (declares . forms)."
    ),
    S!(
        "macroexp-progn",
        1,
        1,
        f_macroexp_progn,
        "Wrap EXPS in progn if needed."
    ),
    S!("macroexp-let2", 4, 4, f_macroexp_let2, "Build a let form."),
    S!(
        "macroexp-let*",
        2,
        2,
        f_macroexp_let_star,
        "Build a let* form."
    ),
    S!("macroexp-if", 3, 3, f_macroexp_if, "Build an if form."),
    S!(
        "byte-code-function-p",
        1,
        1,
        f_byte_code_function_p,
        "t if OBJECT is byte-compiled."
    ),
    S!(
        "compiled-function-p",
        1,
        1,
        f_compiled_function_p,
        "t if OBJECT is compiled."
    ),
    S!(
        "native-comp-available-p",
        0,
        0,
        f_native_comp_available_p,
        "t if native compilation is available."
    ),
    S!(
        "interactive-p",
        0,
        0,
        f_interactive_p,
        "t if called interactively (obsolete)."
    ),
    S!(
        "byte-code",
        3,
        3,
        f_byte_code,
        "Execute byte code (not supported — eval form)."
    ),
    S!("make-byte-code", many 0, f_make_byte_code, "Make byte-code object (stub lambda)."),
    S!(
        "subr-native-lambda-list",
        1,
        1,
        f_subr_native_lambda_list,
        "Subr arglist."
    ),
    S!("help--docstring-quote", 0, 0, f_noop, ""),
    S!("internal-doc-string-p", 0, 0, f_noop, ""),
    S!("declare-functionp", 1, 1, f_declare_functionp, ""),
];

fn f_eval(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    // (eval FORM &optional LEXICAL) — lexical arg binds lexical-binding.
    let lex = arg(&args, 1).truthy();
    i.explicit_eval_depth += 1;
    let r = if lex {
        let id = i.intern("lexical-binding");
        i.specbind(id, Value::t());
        let r = i.eval(&args[0]);
        i.unbind(1);
        r
    } else {
        i.eval(&args[0])
    };
    i.explicit_eval_depth -= 1;
    r
}

fn f_apply(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    if args.is_empty() {
        let s = Value::Sym(i.intern("apply"));
        return Err(i.wrong_number_of_args(&s, 0));
    }
    let fun = args[0].clone();
    let mut argv: Vec<Value> = if args.len() > 1 {
        args[1..args.len() - 1].to_vec()
    } else {
        Vec::new()
    };
    // Last arg must be a list.
    if !matches!(args.last(), Some(Value::Nil)) {
        argv.extend(want_list(i, args.last().unwrap())?);
    }
    i.apply(&fun, argv)
}

fn f_funcall(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    if args.is_empty() {
        let s = Value::Sym(i.intern("funcall"));
        return Err(i.wrong_number_of_args(&s, 0));
    }
    let fun = args[0].clone();
    i.apply(&fun, args[1..].to_vec())
}

fn f_funcall_interactively(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    f_funcall(i, args)
}

fn f_function_raw(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    crate::lisp::special::special_form(crate::lisp::sym::FUNCTION).unwrap()(
        i,
        args.into_iter().next().unwrap_or(Value::Nil),
    )
}

fn f_macroexpand(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    i.macroexpand(&args[0])
}

fn f_macroexpand_1(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    // single-step expansion
    let form = &args[0];
    match form {
        Value::Cons(c) => {
            let (car, cdr) = {
                let b = c.borrow();
                (b.car.clone(), b.cdr.clone())
            };
            if let Value::Sym(id) = car {
                let f = i.symbol_function(id);
                let is_mac = match &f {
                    Value::Lambda(l) => l.is_macro,
                    Value::Cons(cc) => {
                        let b = cc.borrow();
                        i.sym_is(&b.car, sym::MACRO)
                    }
                    _ => false,
                };
                if is_mac {
                    return i.macro_expand_call(&f, &cdr);
                }
            }
            Ok(form.clone())
        }
        _ => Ok(form.clone()),
    }
}

fn f_macroexpand_all(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    macroexpand_all(i, &args[0])
}

/// Recursively expand macros throughout a form.
pub(crate) fn macroexpand_all(i: &mut Interp, form: &Value) -> EvalResult {
    let expanded = i.macroexpand(form)?;
    match &expanded {
        Value::Cons(c) => {
            let (car, cdr) = {
                let b = c.borrow();
                (b.car.clone(), b.cdr.clone())
            };
            // Don't descend into quote.
            if i.sym_is(&car, sym::QUOTE) {
                return Ok(expanded);
            }
            // Rebuild with expanded elements.
            let items = want_list(i, &expanded)?;
            let mut out = Vec::with_capacity(items.len());
            for it in items {
                out.push(macroexpand_all(i, &it)?);
            }
            let _ = (car, cdr);
            Ok(Value::list(out))
        }
        _ => Ok(expanded),
    }
}

fn f_signal(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    // Emacs: signaling a symbol with no `error-conditions' property
    // signals `error' with ("Invalid error symbol" SYM) instead.
    if let Value::Sym(sid) = &args[0] {
        let ec = i.intern("error-conditions");
        if i.get_prop(*sid, ec).is_nil() {
            return Err(Flow::Signal(
                Value::Sym(sym::ERROR),
                Value::list(vec![Value::string("Invalid error symbol"), args[0].clone()]),
            ));
        }
    }
    Err(Flow::Signal(args[0].clone(), args[1].clone()))
}

fn f_error(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let fmt = match &args[0] {
        Value::Str(s) => s.borrow().clone(),
        _ => "%s".into(),
    };
    let msg = apply_format_simple(i, &fmt, &args[1..]);
    Err(Flow::Signal(
        Value::Sym(sym::ERROR),
        Value::list(vec![Value::string(msg)]),
    ))
}

fn f_user_error(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let fmt = match &args[0] {
        Value::Str(s) => s.borrow().clone(),
        _ => "%s".into(),
    };
    let msg = apply_format_simple(i, &fmt, &args[1..]);
    Err(Flow::Signal(
        Value::Sym(sym::USER_ERROR),
        Value::list(vec![Value::string(msg)]),
    ))
}

/// Simple format for error strings: %s → princ, %S → prin1, %% → %.
pub(crate) fn apply_format_simple(i: &Interp, fmt: &str, args: &[Value]) -> String {
    let mut out = String::new();
    let mut ai = 0;
    let mut chars = fmt.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '%' {
            match chars.next() {
                Some('s') => {
                    if let Some(a) = args.get(ai) {
                        ai += 1;
                        out.push_str(&i.princ_to_string(a));
                    }
                }
                Some('S') => {
                    if let Some(a) = args.get(ai) {
                        ai += 1;
                        out.push_str(&i.print_to_string(a));
                    }
                }
                Some('%') => out.push('%'),
                Some(d) if d.is_ascii_digit() => {
                    // skip field-width specs like %5s — read digits then letter
                    let mut n = String::from(d);
                    while let Some(&c2) = chars.peek() {
                        if c2.is_ascii_digit() {
                            n.push(c2);
                            chars.next();
                        } else {
                            break;
                        }
                    }
                    match chars.next() {
                        Some('s') => {
                            if let Some(a) = args.get(ai) {
                                ai += 1;
                                out.push_str(&i.princ_to_string(a));
                            }
                        }
                        Some('d') => {
                            if let Some(a) = args.get(ai) {
                                ai += 1;
                                out.push_str(&i.princ_to_string(a));
                            }
                        }
                        _ => {}
                    }
                }
                Some('d') | Some('x') | Some('o') | Some('c') | Some('e') | Some('f')
                | Some('g') => {
                    if let Some(a) = args.get(ai) {
                        ai += 1;
                        out.push_str(&i.princ_to_string(a));
                    }
                }
                Some(other) => {
                    out.push('%');
                    out.push(other);
                }
                None => out.push('%'),
            }
        } else {
            out.push(c);
        }
    }
    out
}

fn f_throw(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    // Emacs: Fthrow scans the catch chain; with no matching catch it
    // signals `no-catch' at the throw site (so condition-case can catch
    // it, but an outer matching catch would have won instead).
    if i.catch_tags
        .iter()
        .any(|t| crate::lisp::eq_values(t, &args[0]))
    {
        Err(Flow::Throw(args[0].clone(), args[1].clone()))
    } else {
        let nc = i.intern("no-catch");
        Err(i.signal_data(nc, vec![args[0].clone(), args[1].clone()]))
    }
}

fn f_condition_case_raw(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    crate::lisp::special::special_form(sym::CONDITION_CASE).unwrap()(
        i,
        args.into_iter().next().unwrap_or(Value::Nil),
    )
}

/// `(ignore-errors BODY...)` and `(with-demoted-errors BODY...)`.
fn f_ignore_error_raw(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let body = args.into_iter().next().unwrap_or(Value::Nil);
    match i.eval_progn(&body) {
        Ok(v) => Ok(v),
        Err(Flow::Signal(_, _)) => Ok(Value::Nil),
        Err(e) => Err(e),
    }
}

fn f_with_demoted_errors(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    f_ignore_error_raw(i, args)
}

fn f_featurep(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let id = want_sym(i, &args[0])?;
    // consult the `features' variable (seeded with `emacs')
    let fid = i.intern("features");
    let in_list = match i.symbol_value(fid) {
        Value::Cons(_) | Value::Nil => i
            .symbol_value(fid)
            .list_to_vec()
            .map(|items| items.iter().any(|v| matches!(v, Value::Sym(s) if *s == id)))
            .unwrap_or(false),
        _ => false,
    };
    Ok(Value::from_bool(in_list || i.features.contains(&id)))
}

fn f_provide(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let id = want_sym(i, &args[0])?;
    // SUBFEATURES must be a list of symbols.
    if let Some(sub) = args.get(1) {
        match sub {
            Value::Nil => {}
            Value::Cons(_) => {
                // must be a proper list of symbols
                let mut ok = matches!(sub.list_to_vec(), Ok(_));
                if ok {
                    if let Ok(items) = sub.list_to_vec() {
                        ok = items.iter().all(|v| matches!(v, Value::Sym(_)));
                    }
                }
                if !ok {
                    return Err(i.wrong_type_mut("listp", sub));
                }
            }
            other => return Err(i.wrong_type_mut("listp", other)),
        }
    }
    if !i.features.contains(&id) {
        i.features.push(id);
    }
    let flist = Value::list(i.features.iter().map(|s| i.sym(*s)).collect::<Vec<_>>());
    let fid = i.intern("features");
    i.obarray.symbol_mut(fid).value = flist;
    Ok(args[0].clone())
}

fn f_require(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let id = want_sym(i, &args[0])?;
    if i.features.contains(&id) {
        return Ok(args[0].clone());
    }
    // Try to load feature file from load-path.
    let name = match args.get(1) {
        Some(Value::Str(s)) => s.borrow().clone(),
        _ => i.symbol_name(id),
    };
    let loaded = crate::lisp::load::load_library(i, &name)?;
    if loaded {
        if !i.features.contains(&id) {
            i.features.push(id);
        }
        Ok(args[0].clone())
    } else {
        Err(i.signal_data(
            sym::FILE_ERROR,
            vec![Value::string("Cannot open load file"), Value::string(name)],
        ))
    }
}

fn hook_fns(i: &Interp, hook: &Value) -> Vec<Value> {
    let id = match i.sym_id(hook) {
        Some(s) => s,
        None => return Vec::new(),
    };
    let v = i.symbol_value(id);
    // A hook var may hold a single function or a list.
    match &v {
        Value::Cons(_) => v.list_to_vec().unwrap_or_default(),
        Value::Sym(s) if *s == sym::UNBOUND => Vec::new(),
        Value::Nil => Vec::new(),
        other => vec![other.clone()],
    }
}

fn f_run_hooks(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    for hook in &args {
        let fns = hook_fns(i, hook);
        for f in fns {
            i.apply(&f, vec![])?;
        }
    }
    Ok(Value::Nil)
}

fn f_run_hook_with_args(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let hook = args[0].clone();
    let fns = hook_fns(i, &hook);
    for f in fns {
        i.apply(&f, args[1..].to_vec())?;
    }
    Ok(Value::Nil)
}

fn f_run_hook_until_fail(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let hook = args[0].clone();
    let fns = hook_fns(i, &hook);
    for f in fns {
        if i.apply(&f, args[1..].to_vec())?.is_nil() {
            return Ok(Value::Nil);
        }
    }
    Ok(Value::t())
}

fn f_run_hook_until_success(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let hook = args[0].clone();
    let fns = hook_fns(i, &hook);
    for f in fns {
        let r = i.apply(&f, args[1..].to_vec())?;
        if r.truthy() {
            return Ok(r);
        }
    }
    Ok(Value::Nil)
}

fn f_run_hook_wrapped(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    // (run-hook-wrapped HOOK WRAP-FUNCTION &rest ARGS)
    let hook = args[0].clone();
    let wrap = args[1].clone();
    let fns = hook_fns(i, &hook);
    for f in fns {
        let mut call_args = vec![f];
        call_args.extend(args[2..].iter().cloned());
        i.apply(&wrap, call_args)?;
    }
    Ok(Value::Nil)
}

fn f_add_hook(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let hook_id = want_sym(i, &args[0])?;
    let fun = args[1].clone();
    // &optional depth local — we support depth ordering by index.
    let depth = args.get(2).map(|v| match v {
        Value::Int(n) => *n,
        _ => 0,
    });
    let cur = i.symbol_value(hook_id);
    let mut list = match &cur {
        Value::Cons(_) => cur.list_to_vec().unwrap_or_default(),
        Value::Sym(s) if *s == sym::UNBOUND => Vec::new(),
        Value::Nil => Vec::new(),
        other => vec![other.clone()],
    };
    // Don't add duplicates.
    if !list.iter().any(|f| super::eq_values(f, &fun)) {
        match depth {
            Some(d) if d > 0 => list.push(fun),
            Some(d) if d < 0 => list.insert(0, fun),
            _ => list.push(fun),
        }
    }
    i.obarray.symbol_mut(hook_id).value = Value::list(list);
    Ok(Value::Nil)
}

fn f_remove_hook(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let hook_id = want_sym(i, &args[0])?;
    let fun = args[1].clone();
    let cur = i.symbol_value(hook_id);
    let list: Vec<Value> = match &cur {
        Value::Cons(_) => cur
            .list_to_vec()
            .unwrap_or_default()
            .into_iter()
            .filter(|f| !super::eq_values(f, &fun))
            .collect(),
        _ => Vec::new(),
    };
    i.obarray.symbol_mut(hook_id).value = Value::list(list);
    Ok(Value::Nil)
}

fn f_identity(_i: &mut Interp, args: Vec<Value>) -> EvalResult {
    Ok(args[0].clone())
}
fn f_ignore(_i: &mut Interp, _args: Vec<Value>) -> EvalResult {
    Ok(Value::Nil)
}
fn f_always(_i: &mut Interp, _args: Vec<Value>) -> EvalResult {
    Ok(Value::t())
}
fn f_prog1_raw(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    crate::lisp::special::special_form(sym::PROG1).unwrap()(
        i,
        args.into_iter().next().unwrap_or(Value::Nil),
    )
}
fn f_or_raw(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    crate::lisp::special::special_form(sym::OR).unwrap()(
        i,
        args.into_iter().next().unwrap_or(Value::Nil),
    )
}
fn f_and_raw(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    crate::lisp::special::special_form(sym::AND).unwrap()(
        i,
        args.into_iter().next().unwrap_or(Value::Nil),
    )
}
fn f_values(_i: &mut Interp, args: Vec<Value>) -> EvalResult {
    Ok(Value::list(args))
}
fn f_eval_expression(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let v = i.eval(&args[0])?;
    i.message(&format!("{}", i.print_to_string(&v)));
    Ok(v)
}
fn f_load(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let name = match &args[0] {
        Value::Str(s) => s.borrow().clone(),
        _ => return Err(i.wrong_type_mut("stringp", &args[0])),
    };
    let ok = crate::lisp::load::load_library(i, &name)?;
    if ok {
        Ok(Value::t())
    } else {
        Err(i.signal_data(
            sym::FILE_ERROR,
            vec![Value::string("Cannot open load file"), args[0].clone()],
        ))
    }
}
fn f_load_file(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    f_load(i, args)
}
fn f_locate_library(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let name = match &args[0] {
        Value::Str(s) => s.borrow().clone(),
        _ => return Err(i.wrong_type_mut("stringp", &args[0])),
    };
    match crate::lisp::load::locate(i, &name) {
        Some(path) => Ok(Value::string(path)),
        None => Ok(Value::Nil),
    }
}
fn f_autoload(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    // (autoload FUNCTION FILE &optional DOCSTRING INTERACTIVE TYPE)
    // Register an autoload stub: on first call, load FILE then call.
    let fid = want_sym(i, &args[0])?;
    let file = args[1].clone();
    if !i.fbound_p(fid) {
        // Build a lambda that loads the file and re-dispatches.
        let fname = i.symbol_name(fid);
        let _ = file;
        let _ = fname;
        // Store (autoload file interactive) on the function cell as a
        // cons — call_function will resolve it later. For now, mark as
        // defined-but-autoload via a small lambda wrapper symbol.
        let auto_id = i.intern("autoload");
        i.fset(fid, Value::list(vec![Value::Sym(auto_id), file]));
    }
    Ok(Value::Nil)
}
fn f_autoloadp(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let auto_id = i.intern("autoload");
    match &args[0] {
        Value::Cons(c) => {
            let b = c.borrow();
            Ok(Value::from_bool(i.sym_is(&b.car, auto_id)))
        }
        _ => Ok(Value::Nil),
    }
}
fn f_eval_buffer(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    // (eval-buffer &optional BUFFER ...) — eval buffer text.
    let buf_id = match args.get(0) {
        Some(v) if !v.is_nil() => i.buffer_id_of(v).unwrap_or(i.current_buffer),
        _ => i.current_buffer,
    };
    let text = i
        .buffers
        .get(buf_id)
        .map(|b| b.borrow().text.text())
        .unwrap_or_default();
    i.eval_str(&text)
}
fn f_eval_region(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    // (eval-region START END) — eval region of current buffer.
    let (s, e) = match (args.get(0), args.get(1)) {
        (Some(Value::Int(a)), Some(Value::Int(b))) => (*a, *b),
        _ => return Err(i.wrong_type_mut("integerp", &args[0])),
    };
    let text = i
        .buffers
        .get(i.current_buffer)
        .map(|b| {
            let bb = b.borrow();
            bb.text
                .substring((s - 1).max(0) as usize, (e - 1).max(0) as usize)
        })
        .unwrap_or_default();
    i.eval_str(&text)
}
fn f_command_execute(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    i.command_execute(&args[0])
}
fn f_called_interactively_p(_i: &mut Interp, _args: Vec<Value>) -> EvalResult {
    Ok(Value::Nil)
}
fn f_funcall_with_delayed_message(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    i.apply(&args[1], vec![])
}
fn f_declare_function(i: &mut Interp, _args: Vec<Value>) -> EvalResult {
    let _ = i;
    Ok(Value::Nil)
}
fn f_declare(i: &mut Interp, _args: Vec<Value>) -> EvalResult {
    let _ = i;
    Ok(Value::Nil)
}
fn f_eval_and_compile(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let body = args.into_iter().next().unwrap_or(Value::Nil);
    i.eval_progn(&body)
}
fn f_eval_when_compile(_i: &mut Interp, _args: Vec<Value>) -> EvalResult {
    Ok(Value::Nil)
}
fn f_with_no_warnings(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let body = args.into_iter().next().unwrap_or(Value::Nil);
    i.eval_progn(&body)
}
fn f_display_warning(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let msg = i.princ_to_string(&args[1]);
    i.message(&format!("Warning: {}", msg));
    Ok(Value::Nil)
}
fn f_lwarn(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let msg = i.princ_to_string(&args[3]);
    i.message(&format!("Warning: {}", msg));
    Ok(Value::Nil)
}
fn f_warn(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let fmt = match &args[0] {
        Value::Str(s) => s.borrow().clone(),
        _ => "%s".into(),
    };
    let msg = apply_format_simple(i, &fmt, &args[1..]);
    i.message(&format!("Warning: {}", msg));
    Ok(Value::Nil)
}
pub(crate) fn f_message(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let fmt = match &args[0] {
        Value::Str(s) => s.borrow().clone(),
        Value::Nil => return Ok(Value::Nil),
        other => i.princ_to_string(other),
    };
    let msg = apply_format_simple(i, &fmt, &args[1..]);
    i.message(&msg);
    Ok(Value::string(msg))
}
fn f_ding(_i: &mut Interp, _args: Vec<Value>) -> EvalResult {
    Ok(Value::Nil)
}
fn f_sleep_for(_i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let secs = match &args[0] {
        Value::Int(n) => *n as f64,
        Value::Float(f) => *f,
        _ => 0.0,
    } + args
        .get(1)
        .map(|v| match v {
            Value::Int(n) => *n as f64 / 1000.0,
            _ => 0.0,
        })
        .unwrap_or(0.0);
    if secs > 0.0 {
        std::thread::sleep(std::time::Duration::from_secs_f64(secs.min(3600.0)));
    }
    Ok(Value::Nil)
}
fn f_sit_for(_i: &mut Interp, args: Vec<Value>) -> EvalResult {
    // In the editor loop this polls input; standalone → sleep briefly.
    if let Some(Value::Int(n)) = args.get(0) {
        std::thread::sleep(std::time::Duration::from_secs((*n).min(10) as u64));
    }
    Ok(Value::t())
}
fn f_current_time(i: &mut Interp, _args: Vec<Value>) -> EvalResult {
    let us = super::misc::lisp_time_to_us(i, &Value::Nil)?;
    Ok(super::misc::us_to_lisp_time(us))
}
fn f_current_time_string(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let t = match args.get(0) {
        Some(v) => super::misc::lisp_time_to_us(i, v)?,
        None => super::misc::lisp_time_to_us(i, &Value::Nil)?,
    };
    let secs = (t / 1_000_000) as i64;
    let tm = super::misc::local_tm(secs);
    let wday = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"][tm.tm_wday.clamp(0, 6) as usize];
    let mon = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ][tm.tm_mon.clamp(0, 11) as usize];
    Ok(Value::string(format!(
        "{} {} {:02} {:02}:{:02}:{:02} {}",
        wday,
        mon,
        tm.tm_mday,
        tm.tm_hour,
        tm.tm_min,
        tm.tm_sec,
        tm.tm_year + 1900
    )))
}
fn f_current_time_zone(_i: &mut Interp, _args: Vec<Value>) -> EvalResult {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;
    let tm = super::misc::local_tm(secs);
    let zone = if tm.tm_zone.is_null() {
        String::new()
    } else {
        unsafe { std::ffi::CStr::from_ptr(tm.tm_zone as *const i8) }
            .to_string_lossy()
            .into_owned()
    };
    Ok(Value::list(vec![
        Value::Int(tm.tm_gmtoff as i128),
        Value::string(zone),
    ]))
}
fn f_float_time(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let t = match args.get(0) {
        Some(v) => super::misc::lisp_time_to_us(i, v)?,
        None => super::misc::lisp_time_to_us(i, &Value::Nil)?,
    };
    Ok(Value::Float(t as f64 / 1e6))
}
fn f_format_time_string(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let fmt = match &args[0] {
        Value::Str(s) => s.borrow().clone(),
        _ => String::new(),
    };
    let t = match args.get(1) {
        Some(v) => super::misc::lisp_time_to_us(i, v)?,
        None => super::misc::lisp_time_to_us(i, &Value::Nil)?,
    };
    // %z needs the local offset — format in local time.
    let secs = (t / 1_000_000) as i64;
    let tm = super::misc::local_tm(secs);
    let (y, mo, d) = (
        tm.tm_year as i128 + 1900,
        (tm.tm_mon + 1) as u64,
        tm.tm_mday as u64,
    );
    let (h, mi, s) = (tm.tm_hour as u64, tm.tm_min as u64, tm.tm_sec as u64);
    let mut out = String::new();
    let mut ch = fmt.chars().peekable();
    while let Some(c) = ch.next() {
        if c == '%' {
            match ch.next() {
                Some('Y') => out.push_str(&format!("{}", y)),
                Some('m') => out.push_str(&format!("{:02}", mo)),
                Some('d') => out.push_str(&format!("{:02}", d)),
                Some('e') => out.push_str(&format!("{}", d)),
                Some('H') => out.push_str(&format!("{:02}", h)),
                Some('M') => out.push_str(&format!("{:02}", mi)),
                Some('S') => out.push_str(&format!("{:02}", s)),
                Some('s') => out.push_str(&format!("{}", secs)),
                Some('F') => out.push_str(&format!("{}-{:02}-{:02}", y, mo, d)),
                Some('T') => out.push_str(&format!("{:02}:{:02}:{:02}", h, mi, s)),
                Some('a') => out.push_str(
                    ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"]
                        [tm.tm_wday.clamp(0, 6) as usize],
                ),
                Some('A') => out.push_str(
                    [
                        "Sunday",
                        "Monday",
                        "Tuesday",
                        "Wednesday",
                        "Thursday",
                        "Friday",
                        "Saturday",
                    ][tm.tm_wday.clamp(0, 6) as usize],
                ),
                Some('b') | Some('h') => out.push_str(
                    [
                        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct",
                        "Nov", "Dec",
                    ][tm.tm_mon.clamp(0, 11) as usize],
                ),
                Some('B') => out.push_str(
                    [
                        "January",
                        "February",
                        "March",
                        "April",
                        "May",
                        "June",
                        "July",
                        "August",
                        "September",
                        "October",
                        "November",
                        "December",
                    ][tm.tm_mon.clamp(0, 11) as usize],
                ),
                Some('j') => out.push_str(&format!("{:03}", tm.tm_yday + 1)),
                Some('w') => out.push_str(&format!("{}", tm.tm_wday)),
                Some('u') => {
                    out.push_str(&format!("{}", if tm.tm_wday == 0 { 7 } else { tm.tm_wday }))
                }
                Some('y') => out.push_str(&format!("{:02}", (tm.tm_year + 1900) % 100)),
                Some('Z') => {
                    let z = if tm.tm_zone.is_null() {
                        String::new()
                    } else {
                        unsafe { std::ffi::CStr::from_ptr(tm.tm_zone as *const i8) }
                            .to_string_lossy()
                            .into_owned()
                    };
                    out.push_str(&z);
                }
                Some('z') => {
                    let off = tm.tm_gmtoff;
                    let sign = if off < 0 { '-' } else { '+' };
                    let a = off.abs();
                    out.push_str(&format!("{}{:02}{:02}", sign, a / 3600, (a % 3600) / 60));
                }
                Some('%') => out.push('%'),
                Some(o) => {
                    out.push('%');
                    out.push(o);
                }
                None => out.push('%'),
            }
        } else {
            out.push(c);
        }
    }
    let _ = i;
    Ok(Value::string(out))
}

fn f_garbage_collect(i: &mut Interp, _args: Vec<Value>) -> EvalResult {
    let n = i.obarray.len() as i128;
    Ok(Value::list(vec![
        Value::cons(Value::Sym(i.intern("conses")), Value::Int(n)),
        Value::cons(Value::Sym(i.intern("symbols")), Value::Int(n)),
        Value::cons(Value::Sym(i.intern("strings")), Value::Int(0)),
        Value::cons(Value::Sym(i.intern("miscs")), Value::Int(0)),
        Value::cons(Value::Sym(i.intern("vector-cells")), Value::Int(0)),
        Value::cons(Value::Sym(i.intern("floats")), Value::Int(0)),
        Value::cons(Value::Sym(i.intern("intervals")), Value::Int(0)),
        Value::cons(Value::Sym(i.intern("buffers")), Value::Int(0)),
        Value::cons(Value::Sym(i.intern("string-chars")), Value::Int(0)),
        Value::cons(Value::Sym(i.intern("cons-cells")), Value::Int(n)),
    ]))
}
fn f_memory_info(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    f_garbage_collect(i, args)
}
fn f_kill_emacs(i: &mut Interp, _args: Vec<Value>) -> EvalResult {
    i.quit_editor = true;
    Ok(Value::Nil)
}
fn f_recursive_edit(i: &mut Interp, _args: Vec<Value>) -> EvalResult {
    i.recursion_depth += 1;
    Ok(Value::Nil)
}
fn f_top_level(_i: &mut Interp, _args: Vec<Value>) -> EvalResult {
    Err(Flow::Throw(Value::Sym(sym::TOP_LEVEL), Value::Nil))
}
fn f_exit_recursive_edit(i: &mut Interp, _args: Vec<Value>) -> EvalResult {
    if i.recursion_depth == 0 {
        return Err(i.error("No recursive edit is in progress"));
    }
    Err(Flow::Throw(
        Value::Sym(sym::EXIT_RECURSIVE_EDIT),
        Value::Nil,
    ))
}
fn f_abort_recursive_edit(_i: &mut Interp, _args: Vec<Value>) -> EvalResult {
    Err(Flow::Throw(Value::Sym(sym::QUIT), Value::Nil))
}
fn f_recursion_depth(i: &mut Interp, _args: Vec<Value>) -> EvalResult {
    Ok(Value::Int(i.recursion_depth as i128))
}
fn f_emacs_pid(_i: &mut Interp, _args: Vec<Value>) -> EvalResult {
    Ok(Value::Int(std::process::id() as i128))
}
fn f_system_name(_i: &mut Interp, _args: Vec<Value>) -> EvalResult {
    Ok(Value::string("localhost"))
}
fn f_emacs_version(_i: &mut Interp, _args: Vec<Value>) -> EvalResult {
    Ok(Value::string(format!(
        "remacs {} (Emacs-compatible)",
        env!("CARGO_PKG_VERSION")
    )))
}
fn f_emacs_build_time(_i: &mut Interp, _args: Vec<Value>) -> EvalResult {
    Ok(Value::Int(0))
}
fn f_noop(_i: &mut Interp, _args: Vec<Value>) -> EvalResult {
    Ok(Value::Nil)
}
fn f_internal_make_closure(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    // (internal-make-interpreted-closure-function ARGS BODY ENV)
    let params = &args[0];
    let body_v = &args[1];
    let env_v = &args[2];
    let body = match body_v {
        Value::Cons(_) => body_v.list_to_vec().unwrap_or_default(),
        _ => vec![body_v.clone()],
    };
    let env = parse_lexenv_spec(i, env_v);
    let mut lam = i.parse_lambda(params, &body, None)?;
    lam.env = env;
    Ok(Value::Lambda(std::rc::Rc::new(lam)))
}

/// Convert the alist-ish ENV argument used by byte-compiled closures
/// into a LexEnv.
fn parse_lexenv_spec(_i: &mut Interp, _env_v: &Value) -> crate::lisp::LexEnv {
    None
}
fn f_macroexp_parse_body(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let items = want_list(i, &args[0])?;
    // split leading (declare ...) forms and strings
    let mut declares = Vec::new();
    let mut rest_start = items.len();
    for (k, f) in items.iter().enumerate() {
        let is_decl = match f {
            Value::Cons(c) => {
                let b = c.borrow();
                let decl_id = i.intern("declare");
                i.sym_is(&b.car, decl_id)
            }
            Value::Str(_) => k < items.len() - 1,
            _ => false,
        };
        if is_decl {
            declares.push(f.clone());
        } else {
            rest_start = k;
            break;
        }
    }
    Ok(Value::cons(
        Value::list(declares),
        Value::list(items[rest_start..].to_vec()),
    ))
}
fn f_macroexp_progn(_i: &mut Interp, args: Vec<Value>) -> EvalResult {
    match &args[0] {
        Value::Cons(c) => {
            let b = c.borrow();
            if b.cdr.is_nil() {
                return Ok(b.car.clone());
            }
        }
        Value::Nil => return Ok(Value::Nil),
        _ => {}
    }
    Ok(Value::cons(
        Value::Sym(crate::lisp::sym::PROGN),
        args[0].clone(),
    ))
}
fn f_macroexp_let2(_i: &mut Interp, args: Vec<Value>) -> EvalResult {
    // (macroexp-let2 VAR SYM VALUE EXP) → (let ((SYM VALUE)) EXP)
    let binding = Value::list(vec![args[1].clone(), args[2].clone()]);
    Ok(Value::list(vec![
        Value::Sym(sym::LET),
        Value::list(vec![binding]),
        args[3].clone(),
    ]))
}
fn f_macroexp_let_star(_i: &mut Interp, args: Vec<Value>) -> EvalResult {
    Ok(Value::cons(
        Value::Sym(sym::LET_STAR),
        Value::cons(args[0].clone(), args[1].clone()),
    ))
}
fn f_macroexp_if(_i: &mut Interp, args: Vec<Value>) -> EvalResult {
    Ok(Value::list(vec![
        Value::Sym(sym::IF),
        args[0].clone(),
        args[1].clone(),
        args[2].clone(),
    ]))
}
fn f_byte_code_function_p(_i: &mut Interp, _args: Vec<Value>) -> EvalResult {
    Ok(Value::Nil)
}
fn f_compiled_function_p(_i: &mut Interp, args: Vec<Value>) -> EvalResult {
    Ok(Value::from_bool(matches!(&args[0], Value::Subr(_))))
}
fn f_native_comp_available_p(_i: &mut Interp, _args: Vec<Value>) -> EvalResult {
    Ok(Value::Nil)
}
fn f_interactive_p(_i: &mut Interp, _args: Vec<Value>) -> EvalResult {
    Ok(Value::Nil)
}
fn f_byte_code(_i: &mut Interp, args: Vec<Value>) -> EvalResult {
    // We don't have a byte-compiler; treat (byte-code template consts)
    // as an error to surface unsupported paths.
    let _ = args;
    Err(_i.error("byte-code not supported (no byte compiler)"))
}
fn f_make_byte_code(_i: &mut Interp, _args: Vec<Value>) -> EvalResult {
    Ok(Value::Nil)
}
fn f_subr_native_lambda_list(_i: &mut Interp, _args: Vec<Value>) -> EvalResult {
    Ok(Value::Nil)
}
fn f_declare_functionp(_i: &mut Interp, _args: Vec<Value>) -> EvalResult {
    Ok(Value::Nil)
}
