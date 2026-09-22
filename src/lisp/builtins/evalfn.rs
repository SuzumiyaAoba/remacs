//! Evaluation-related subrs: eval, apply, funcall, signal, error, throw,
//! featurep, run-hooks, etc.

use super::{S, arg, want_list, want_sym};
use crate::lisp::Interp;
use crate::lisp::SymId;
use crate::lisp::error::{EvalResult, Flow};
use crate::lisp::obarray::sym;
use crate::lisp::value::{Marker, Subr, Value};
use std::cell::RefCell;
use std::rc::Rc;

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
        "internal--track-mouse",
        1,
        1,
        f_internal_track_mouse,
        "Call BODYFN with mouse-motion tracking enabled."
    ),
    S!(
        "save-mark-and-excursion--save",
        0,
        0,
        f_smae_save,
        "Save the mark state; used by `save-mark-and-excursion'."
    ),
    S!(
        "save-mark-and-excursion--restore",
        1,
        1,
        f_smae_restore,
        "Restore the mark state saved by --save."
    ),
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
        many 2,
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
        "autoload-do-load",
        1,
        3,
        f_autoload_do_load,
        "Load FUNDEF's autoload file, return the new definition."
    ),
    S!(
        "advice-add",
        3,
        4,
        f_advice_add,
        "Add FUNCTION to SYMBOL's advices in the WHERE class."
    ),
    S!(
        "advice-remove",
        2,
        2,
        f_advice_remove,
        "Remove FUNCTION (or the named advice) from SYMBOL."
    ),
    S!(
        "advice-member-p",
        2,
        2,
        f_advice_member_p,
        "Non-nil if FUNCTION was added to SYMBOL."
    ),
    S!(
        "advice-function-member-p",
        2,
        2,
        f_advice_member_p,
        "Non-nil if ADVICE is among FUNCTION-DEF's advices."
    ),
    S!(
        "advice-function-mapc",
        2,
        2,
        f_advice_function_mapc,
        "Apply FUNCTION to each advice of FUNCTION-DEF."
    ),
    S!(
        "cl--advice--apply",
        many 1,
        f_advice_apply_link,
        "Internal trampoline applying one advice wrapper."
    ),
    S!(
        "cl--add-function",
        3,
        4,
        f_add_function,
        "Internal: add FUNCTION at HOW to normalized PLACE."
    ),
    S!(
        "cl--remove-function",
        2,
        2,
        f_remove_function,
        "Internal: remove FUNCTION from normalized PLACE."
    ),
    S!(
        "handler-bind-1",
        many 1,
        f_handler_bind_1,
        "Run BODY with condition handlers bound (see `handler-bind')."
    ),
    S!(
        "access-file",
        1,
        2,
        f_access_file,
        "Access FILENAME for reading; signal `file-missing' on failure."
    ),
    S!(
        "current-message",
        0,
        0,
        f_current_message,
        "String currently shown in the echo area, or nil."
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
        i.specbind(id, Value::t())?;
        let r = i.eval(&args[0]);
        i.unbind(1)?;
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

fn f_function_raw(i: &mut Interp, _args: Vec<Value>) -> EvalResult {
    f_special_form_via_apply(i, Vec::new())
}

/// Special forms are not funcallable (GNU signals `invalid-function').
fn f_special_form_via_apply(i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    Err(i.signal_data(sym::INVALID_FUNCTION, vec![Value::Nil]))
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
            // Rebuild with expanded elements; improper lists pass
            // through unexpanded rather than failing.
            let items = match want_list(i, &expanded) {
                Ok(v) => v,
                Err(_) => return Ok(expanded),
            };
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
                false,
            ));
        }
    }
    Err(Flow::Signal(args[0].clone(), args[1].clone(), false))
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
        false,
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
        false,
    ))
}

/// GNU's `format-message' quote convention: every `` `' `` and `'` in
/// the FORMAT STRING becomes U+2018/U+2019 (curve quoting).  Only the
/// format string is translated, never the substituted arguments.
pub(crate) fn translate_message_quotes(fmt: &str) -> String {
    let mut out = String::with_capacity(fmt.len());
    for c in fmt.chars() {
        match c {
            '`' => out.push('\u{2018}'),
            '\'' => out.push('\u{2019}'),
            _ => out.push(c),
        }
    }
    out
}

/// Simple format for error strings: %s → princ, %S → prin1, %% → %.
pub(crate) fn apply_format_simple(i: &Interp, fmt: &str, args: &[Value]) -> String {
    let fmt = translate_message_quotes(fmt);
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

fn f_condition_case_raw(i: &mut Interp, _args: Vec<Value>) -> EvalResult {
    f_special_form_via_apply(i, Vec::new())
}

/// `(ignore-errors BODY...)` and `(with-demoted-errors BODY...)`.
fn f_ignore_error_raw(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let body = args.into_iter().next().unwrap_or(Value::Nil);
    // Register `error' as claimed so handler-bind handlers don't run
    // for signals this form will swallow (GNU suppresses them via the
    // no-debugger-entry rule).
    let err_id = i.intern("error");
    i.case_handlers.push(Value::list(vec![Value::Sym(err_id)]));
    let r = i.eval_progn(&body);
    i.case_handlers.pop();
    match r {
        Ok(v) => Ok(v),
        Err(Flow::Signal(_, _, _)) => Ok(Value::Nil),
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
        // GNU `provide' is `(push feature features)'.
        i.features.insert(0, id);
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
    let noerror = args.get(2).map(|v| v.truthy()).unwrap_or(false);
    match crate::lisp::load::load_library(i, &name) {
        Ok(true) => {
            if !i.features.contains(&id) {
                i.features.push(id);
            }
            Ok(args[0].clone())
        }
        Ok(false) | Err(_) if noerror => Ok(Value::Nil),
        Ok(false) => {
            let fm = i.intern("file-missing");
            Err(i.signal_data(
                fm,
                vec![
                    Value::string("Cannot open load file"),
                    Value::string("No such file or directory"),
                    Value::string(name),
                ],
            ))
        }
        Err(e) => Err(e),
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

fn want_hook_sym(i: &mut Interp, hook: &Value) -> Result<(), super::Flow> {
    match hook {
        Value::Sym(_) | Value::Nil => Ok(()),
        other => Err(i.wrong_type_mut("symbolp", other)),
    }
}

fn f_run_hooks(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    for hook in &args {
        want_hook_sym(i, hook)?;
        let fns = hook_fns(i, hook);
        for f in fns {
            i.apply(&f, vec![])?;
        }
    }
    Ok(Value::Nil)
}

fn f_run_hook_with_args(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let hook = args[0].clone();
    want_hook_sym(i, &hook)?;
    let fns = hook_fns(i, &hook);
    for f in fns {
        i.apply(&f, args[1..].to_vec())?;
    }
    Ok(Value::Nil)
}

fn f_run_hook_until_fail(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let hook = args[0].clone();
    want_hook_sym(i, &hook)?;
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
    want_hook_sym(i, &hook)?;
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
fn f_prog1_raw(i: &mut Interp, _args: Vec<Value>) -> EvalResult {
    f_special_form_via_apply(i, Vec::new())
}
fn f_or_raw(i: &mut Interp, _args: Vec<Value>) -> EvalResult {
    f_special_form_via_apply(i, Vec::new())
}
fn f_and_raw(i: &mut Interp, _args: Vec<Value>) -> EvalResult {
    f_special_form_via_apply(i, Vec::new())
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
        // GNU: file-missing ("Cannot open load file" REASON NAME).
        let fm = i.intern("file-missing");
        Err(i.signal_data(
            fm,
            vec![
                Value::string("Cannot open load file"),
                Value::string("No such file or directory"),
                args[0].clone(),
            ],
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
        // Store GNU's autoload cell shape:
        // (autoload FILE &optional DOCSTRING INTERACTIVE TYPE).
        let auto_id = i.intern("autoload");
        let mut cell = vec![Value::Sym(auto_id), file];
        cell.extend(args[2..].iter().cloned());
        i.fset(fid, Value::list(cell));
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

fn f_autoload_do_load(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let fundef = args[0].clone();
    let macro_only = args.get(1).map(|v| v.truthy()).unwrap_or(false);
    autoload_do_load(i, fundef, macro_only)
}

/// `autoload-do-load` FUNDEF MACRO-ONLY — load the file for an autoload
/// cell and return the resulting function definition. Shared with the
/// evaluator's autoload dispatch.
pub(crate) fn autoload_do_load(
    i: &mut Interp,
    fundef: Value,
    macro_only: bool,
) -> EvalResult {
    let auto_id = i.intern("autoload");
    let is_auto = match &fundef {
        Value::Cons(c) => i.sym_is(&c.borrow().car, auto_id),
        _ => false,
    };
    if !is_auto {
        return Ok(fundef);
    }
    let cell = fundef.list_to_vec().unwrap_or_default();
    let file = cell.get(1).cloned().unwrap_or(Value::Nil);
    // (autoload FILE DOC INTERACTIVE TYPE) — TYPE non-nil = macro.
    let is_macro_autoload = cell.get(4).map(|v| v.truthy()).unwrap_or(false);
    if macro_only && !is_macro_autoload {
        return Ok(fundef);
    }
    let name = match &file {
        Value::Str(s) => s.borrow().clone(),
        _ => return Err(i.wrong_type_mut("stringp", &file)),
    };
    // Find the symbol whose function cell is this autoload object.
    let owner = i
        .obarray
        .all_ids()
        .into_iter()
        .find(|id| super::eq_values(&i.symbol_function(*id), &fundef));
    let _ = crate::lisp::load::load_library(i, &name)?;
    match owner {
        Some(id) => {
            let newdef = i.symbol_function(id);
            // Emacs signals error if loading didn't redefine the autoload.
            let still_auto = match &newdef {
                Value::Cons(c) => i.sym_is(&c.borrow().car, auto_id),
                _ => false,
            };
            if still_auto {
                return Err(i.error(format!(
                    "Autoloading file {} failed to define function {}",
                    name,
                    i.symbol_name(id)
                )));
            }
            Ok(newdef)
        }
        None => Ok(Value::Nil),
    }
}

fn f_handler_bind_1(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    // (handler-bind-1 BODY-FN CONDS HANDLER CONDS HANDLER ...) — bind
    // dynamic signal handlers while running the 0-argument BODY-FN.
    let body = args[0].clone();
    let base = i.handler_bindings.len();
    // Pairs are consulted innermost-first (reverse stack order), so
    // push them in reverse to consult them in argument order, with all
    // of this call's bindings inner to any outer `handler-bind-1'.
    let mut k = if args.len() % 2 == 1 {
        args.len() - 1
    } else {
        args.len() - 2
    };
    while k >= 2 {
        i.handler_bindings
            .push((args[k - 1].clone(), args[k].clone()));
        k -= 2;
    }
    let r = i.apply(&body, vec![]);
    i.handler_bindings.truncate(base);
    r
}

fn f_access_file(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    // (access-file FILENAME ERROR-FORMAT) — signal `file-missing' with
    // (ERROR-FORMAT ERRNO-STRING FILENAME) when FILENAME can't be read.
    let name = match &args[0] {
        Value::Str(s) => s.borrow().clone(),
        other => return Err(i.wrong_type_mut("stringp", other)),
    };
    let expanded = crate::editor::expand_file_name_str(i, &name);
    match std::fs::File::open(&expanded) {
        Ok(_) => Ok(Value::Nil),
        Err(e) => {
            let estr = e.to_string();
            let estr = estr
                .split(" (os error")
                .next()
                .unwrap_or(&estr)
                .to_string();
            let fmt = args.get(1).cloned().unwrap_or(Value::Nil);
            Err(i.signal_data(
                sym::FILE_MISSING,
                vec![fmt, Value::string(estr), args[0].clone()],
            ))
        }
    }
}

fn f_current_message(i: &mut Interp, _args: Vec<Value>) -> EvalResult {
    if i.echo_message.is_empty() {
        Ok(Value::Nil)
    } else {
        Ok(Value::string(i.echo_message.clone()))
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
    let len = i
        .buffers
        .get(i.current_buffer)
        .map(|b| b.borrow().text.len() as i128 + 1)
        .unwrap_or(1);
    if s < 1 || e < 1 || s > len || e > len {
        let sym = i.intern("args-out-of-range");
        return Err(i.signal_data(sym, vec![Value::Int(s), Value::Int(e)]));
    }
    let text = i
        .buffers
        .get(i.current_buffer)
        .map(|b| {
            let bb = b.borrow();
            bb.text.substring((s - 1) as usize, (e - 1) as usize)
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
    let ty = i.princ_to_string(&args[0]);
    let msg = i.princ_to_string(&args[1]);
    let text = format!("Warning ({}): {}", ty, msg);
    i.message(&text);
    Ok(Value::string(text))
}
fn f_lwarn(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let ty = i.princ_to_string(&args[0]);
    let fmt = match &args[2] {
        Value::Str(s) => s.borrow().clone(),
        other => i.princ_to_string(other),
    };
    let msg = apply_format_simple(i, &fmt, &args[3..]);
    let text = format!("Warning ({}): {}", ty, msg);
    i.message(&text);
    Ok(Value::string(text))
}
fn f_warn(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let fmt = match &args[0] {
        Value::Str(s) => s.borrow().clone(),
        _ => "%s".into(),
    };
    let msg = apply_format_simple(i, &fmt, &args[1..]);
    let text = format!("Warning (emacs): {}", msg);
    i.message(&text);
    Ok(Value::string(text))
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
/// keyboard.c `timer_check': run each due, untriggered timer on
/// `timer-list' through `timer-event-handler' (which reschedules
/// repeat timers and swallows handler errors itself).  GNU never
/// fires idle timers without an input loop, so `timer-idle-list' is
/// intentionally not consulted here.
pub(crate) fn timer_check(i: &mut Interp) -> EvalResult {
    let tl = i.intern("timer-list");
    let list = i.symbol_value(tl);
    // Snapshot: the handler mutates `timer-list' while running.
    let timers = list.list_to_vec().unwrap_or_default();
    if timers.is_empty() {
        return Ok(Value::Nil);
    }
    let handler = i.intern("timer-event-handler");
    if let Value::Sym(s) = i.symbol_function(handler) {
        if s == crate::lisp::sym::UNBOUND {
            return Ok(Value::Nil);
        }
    }
    let now = super::misc::lisp_time_to_ns(i, &Value::Nil)?;
    for timer in timers {
        // Slots: [triggered high low usec repeat function args idle psec
        //         integral-multiple]
        let when = match &timer {
            Value::Vec(v) => {
                let v = v.borrow();
                if v.len() < 9 || v[0].truthy() || v[5].is_nil() {
                    None
                } else {
                    Some(Value::list(vec![
                        v[1].clone(),
                        v[2].clone(),
                        v[3].clone(),
                        v[8].clone(),
                    ]))
                }
            }
            _ => None,
        };
        let due = match when {
            Some(t) => super::misc::lisp_time_to_ns(i, &t)
                .map(|ns| ns <= now)
                .unwrap_or(false),
            None => false,
        };
        if due {
            // GNU marks the timer triggered when queueing its
            // timer-event, before `timer-event-handler' runs.
            if let Value::Vec(v) = &timer {
                v.borrow_mut()[0] = Value::t();
            }
            i.call_function(&Value::Sym(handler), &Value::list(vec![timer]), None)?;
        }
    }
    Ok(Value::Nil)
}

/// Sleep for SECS seconds, firing due timers during the wait like
/// GNU's `wait_reading_process_output'.
fn sleep_firing_timers(i: &mut Interp, secs: f64) -> EvalResult {
    if secs <= 0.0 {
        return timer_check(i).map(|_| Value::Nil);
    }
    let deadline =
        std::time::Instant::now() + std::time::Duration::from_secs_f64(secs.min(3600.0));
    loop {
        timer_check(i)?;
        let rest = deadline.saturating_duration_since(std::time::Instant::now());
        if rest.is_zero() {
            break;
        }
        std::thread::sleep(rest.min(std::time::Duration::from_millis(20)));
    }
    timer_check(i)?;
    Ok(Value::Nil)
}

fn f_sleep_for(i: &mut Interp, args: Vec<Value>) -> EvalResult {
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
    sleep_firing_timers(i, secs)
}
fn f_sit_for(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    // Batch: no input to wait for — GNU sleeps the full time, firing timers.
    let secs = match args.get(0) {
        Some(Value::Int(n)) => *n as f64,
        Some(Value::Float(f)) => *f,
        _ => 0.0,
    };
    sleep_firing_timers(i, secs)?;
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
        Some(v) => super::misc::lisp_time_to_ps(i, v)?,
        None => super::misc::lisp_time_to_ps(i, &Value::Nil)?,
    };
    Ok(Value::Float(t as f64 / 1e12))
}
fn f_format_time_string(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let fmt = match &args[0] {
        Value::Str(s) => s.borrow().clone(),
        other => return Err(i.wrong_type_mut("stringp", other)),
    };
    let t = match args.get(1) {
        Some(v) => super::misc::lisp_time_to_us(i, v)?,
        None => super::misc::lisp_time_to_us(i, &Value::Nil)?,
    };
    let secs = t.div_euclid(1_000_000) as i64;
    let nsecs = t.rem_euclid(1_000_000) * 1000;
    // GNU (format-time-string FORMAT TIME ZONE): nil/`wall' = local,
    // integer = fixed offset, string = TZ spec, `t' is treated as UTC.
    let t_sym = i.intern("t");
    let wall_sym = i.intern("wall");
    let tm = match args.get(2) {
        Some(Value::Int(off)) => {
            let mut tm = super::misc::gmt_tm(secs + *off as i64);
            tm.tm_gmtoff = *off as i64;
            tm
        }
        Some(Value::Sym(s)) if *s == wall_sym => super::misc::local_tm(secs),
        Some(Value::Sym(s)) if *s == t_sym => super::misc::gmt_tm(secs),
        Some(Value::Str(z)) => {
            let z = z.borrow().clone();
            if z == "UTC" || z == "t" {
                super::misc::gmt_tm(secs)
            } else {
                super::misc::tz_local_tm(secs, &z)
            }
        }
        Some(v) if v.truthy() => super::misc::gmt_tm(secs),
        _ => super::misc::local_tm(secs),
    };
    let mut out = String::new();
    let chars: Vec<char> = fmt.chars().collect();
    let mut k = 0;
    while k < chars.len() {
        let c = chars[k];
        if c != '%' {
            out.push(c);
            k += 1;
            continue;
        }
        // GNU passes flags/width (e.g. %-d, %_3a, %05Y, %::z) through to
        // the underlying strftime.
        let mut j = k + 1;
        while j < chars.len() && matches!(chars[j], '-' | '_' | '0' | '^' | '#') {
            j += 1;
        }
        while j < chars.len() && (chars[j].is_ascii_digit() || matches!(chars[j], '.' | ',' | ':')) {
            j += 1;
        }
        let spec = chars.get(j).copied();
        let spec_text: String = chars[k..=j.min(chars.len() - 1)].iter().collect();
        k = j + 1;
        match spec {
            Some('N') => out.push_str(&format!("{:09}", nsecs)),
            Some('s') => out.push_str(&format!("{}", secs)),
            // macOS strftime derives %z from the process TZ, not the
            // broken-down time — compute it from tm_gmtoff instead.
            Some('z') => {
                let off = tm.tm_gmtoff;
                let sign = if off < 0 { '-' } else { '+' };
                let a = off.abs();
                out.push_str(&format!("{}{:02}{:02}", sign, a / 3600, (a % 3600) / 60));
            }
            Some(_) => out.push_str(&super::misc::strftime_spec(&spec_text, &tm)),
            None => out.push('%'),
        }
    }
    let _ = i;
    Ok(Value::string(out))
}

fn f_garbage_collect(i: &mut Interp, _args: Vec<Value>) -> EvalResult {
    // GNU shape: (NAME SIZE USED FREE) triples per category, except
    // string-bytes/buffers which lack the FREE count.
    let n = i.obarray.len() as i128;
    let item = |i: &mut Interp, name: &str, vals: &[i128]| {
        let mut v = vec![Value::Sym(i.intern(name))];
        v.extend(vals.iter().map(|x| Value::Int(*x)));
        Value::list(v)
    };
    Ok(Value::list(vec![
        item(i, "conses", &[16, n, 0]),
        item(i, "symbols", &[48, n, 0]),
        item(i, "strings", &[32, 0, 0]),
        item(i, "string-bytes", &[1, 0]),
        item(i, "vectors", &[16, 0]),
        item(i, "vector-slots", &[8, 0, 0]),
        item(i, "floats", &[8, 0, 0]),
        item(i, "intervals", &[56, 0, 0]),
        item(i, "buffers", &[1064, 0]),
    ]))
}
fn f_memory_info(_i: &mut Interp, _args: Vec<Value>) -> EvalResult {
    // GNU returns nil where the kernel gives no memory-info query
    // interface (macOS, and thus our batch probes).
    Ok(Value::Nil)
}
fn f_kill_emacs(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    i.quit_editor = true;
    if i.noninteractive {
        // GNU batch: kill-emacs exits the process immediately; an
        // integer ARG is the exit status, a string ARG is printed.
        let code = match args.first() {
            Some(Value::Int(n)) => *n,
            Some(Value::Str(s)) => {
                let msg = s.borrow().clone();
                let _ = i;
                eprint!("{msg}");
                0
            }
            _ => 0,
        };
        return Err(Flow::Exit(code));
    }
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
    Ok(Value::string(crate::buffer::our_host_name()))
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
fn f_subr_native_lambda_list(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    // Emacs 31: t for primitives, wrong-type-argument otherwise.
    match &args[0] {
        Value::Subr(_) => Ok(Value::t()),
        other => Err(i.wrong_type_mut("subrp", other)),
    }
}
fn f_declare_functionp(_i: &mut Interp, _args: Vec<Value>) -> EvalResult {
    Ok(Value::Nil)
}

/// `internal--track-mouse` — GNU enters mouse-tracking mode then calls
/// BODYFN; tracking is a no-op without a window system, so just apply.
fn f_internal_track_mouse(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    i.apply(&a[0], vec![])
}

/// `save-mark-and-excursion--save` → (MARKER . ACTIVE) — `(nil)' when
/// the mark isn't set, like GNU.
fn f_smae_save(i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    let buf = i.current_buffer;
    let Some(b) = i.buffers.get(buf) else {
        return Ok(Value::cons(Value::Nil, Value::Nil));
    };
    let (mark, active) = {
        let bb = b.borrow();
        (bb.mark, bb.mark_active)
    };
    let m = match mark {
        Some(pos) => {
            let mk = Rc::new(RefCell::new(Marker {
                buffer: Some(buf),
                position: pos,
                insertion_type: false,
            }));
            b.borrow_mut().register_marker(&mk);
            Value::Marker(mk)
        }
        None => Value::Nil,
    };
    Ok(Value::cons(m, if active { Value::t() } else { Value::Nil }))
}

/// `save-mark-and-excursion--restore` — restore mark position (when the
/// saved cons's car is a marker) and activation flag.
fn f_smae_restore(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    if let Value::Cons(c) = &a[0] {
        let (mk, act) = {
            let b = c.borrow();
            (b.car.clone(), b.cdr.clone())
        };
        let buf = i.current_buffer;
        if let Some(b) = i.buffers.get(buf) {
            let mut bb = b.borrow_mut();
            if let Value::Marker(m) = &mk {
                bb.mark = Some(m.borrow().position.min(bb.text.len()));
            }
            bb.mark_active = act.truthy();
        }
    }
    Ok(Value::Nil)
}

const ADVICE_WHERES: &[&str] = &[
    ":around",
    ":before",
    ":after",
    ":override",
    ":before-until",
    ":before-while",
    ":after-until",
    ":after-while",
    ":filter-args",
    ":filter-return",
];

/// The `name' property of an advice PROPS alist.
fn advice_name(i: &mut Interp, props: &Value) -> Value {
    let name_kw = i.intern("name");
    let mut cur = props.clone();
    while let Value::Cons(c) = cur {
        let (car, cdr) = {
            let b = c.borrow();
            (b.car.clone(), b.cdr.clone())
        };
        if let Value::Cons(e) = &car {
            let (k, v) = {
                let b = e.borrow();
                (b.car.clone(), b.cdr.clone())
            };
            if let Value::Sym(s) = k {
                if s == name_kw {
                    return v;
                }
            }
        }
        cur = cdr;
    }
    Value::Nil
}

/// Match an advice entry against `advice-remove'/`advice-member-p''s
/// FUNCTION argument: the function value itself or a non-nil :name.
fn advice_entry_matches(i: &Interp, fun: &Value, name: &Value, sel: &Value) -> bool {
    super::equal_values(i, fun, sel) || (!name.is_nil() && super::equal_values(i, name, sel))
}

fn f_advice_add(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let sym = want_sym(i, &a[0])?;
    let w = want_sym(i, &a[1])?;
    if !ADVICE_WHERES
        .iter()
        .any(|n| i.symbol_name(w).as_str() == *n)
    {
        return Err(i.signal_data(
            sym::ERROR,
            vec![Value::string(format!(
                "Unknown add-function location ‘{}’",
                i.symbol_name(w)
            ))],
        ));
    }
    let name = advice_name(i, &arg(&a, 3));
    advice_push(i, sym, w, a[2].clone(), name);
    Ok(Value::Nil)
}

fn advice_retain(i: &mut Interp, key: SymId, sel: &Value) {
    if let Some(pos) = i.advices.iter().position(|(s, _)| *s == key) {
        let mut list = std::mem::take(&mut i.advices[pos].1);
        list.retain(|(_, f, n)| !advice_entry_matches(i, f, n, sel));
        i.advices[pos].1 = list;
    }
}

fn advice_push(i: &mut Interp, key: SymId, w: SymId, fun: Value, name: Value) {
    let entry = (w, fun.clone(), name.clone());
    let pos = match i.advices.iter().position(|(s, _)| *s == key) {
        Some(p) => p,
        None => {
            i.advices.push((key, Vec::new()));
            i.advices.len() - 1
        }
    };
    let mut list = std::mem::take(&mut i.advices[pos].1);
    if !name.is_nil() {
        list.retain(|(_, _, n)| !super::equal_values(i, n, &name));
    }
    list.retain(|(_, f, _)| !super::equal_values(i, f, &fun));
    list.push(entry);
    i.advices[pos].1 = list;
}

/// The property under which an advice-wrapper gensym records the
/// original place value it replaced.
fn advice_wrap(i: &mut Interp, cur: &Value) -> SymId {
    let orig_key = i.intern("cl--advice--orig");
    if let Value::Sym(k) = cur {
        if matches!(i.get_prop(*k, orig_key), Value::Cons(_)) {
            return *k;
        }
    }
    let k = i.obarray.gensym("cl--advice--");
    i.fset(k, cur.clone());
    i.put_prop(k, orig_key, Value::cons(cur.clone(), Value::Nil));
    k
}

/// If V is an advice-wrapper gensym, return (wrapper-key, orig-value).
fn advice_unwrap(i: &mut Interp, v: &Value) -> Option<(SymId, Value)> {
    if let Value::Sym(k) = v {
        let orig_key = i.intern("cl--advice--orig");
        if let Value::Cons(c) = i.get_prop(*k, orig_key) {
            return Some((*k, c.borrow().car.clone()));
        }
    }
    None
}

fn f_advice_remove(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let sym = want_sym(i, &a[0])?;
    advice_retain(i, sym, &a[1]);
    Ok(Value::Nil)
}

/// `cl--add-function` — (HOW PLACE FUNCTION PROPS) where PLACE is a
/// normalized `(KIND . ARGS)' list produced by the `add-function'
/// macro: (function SYM), (var SYM), or (get SYM PROP).
fn f_add_function(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let w = want_sym(i, &a[0])?;
    if !ADVICE_WHERES
        .iter()
        .any(|n| i.symbol_name(w).as_str() == *n)
    {
        return Err(i.signal_data(
            sym::ERROR,
            vec![Value::string(format!(
                "Unknown add-function location ‘{}’",
                i.symbol_name(w)
            ))],
        ));
    }
    let p = a[1]
        .list_to_vec()
        .map_err(|_| i.wrong_type_mut("listp", &a[1]))?;
    let kind = p
        .first()
        .and_then(|h| i.sym_id(h))
        .map(|s| i.symbol_name(s));
    let key = match kind.as_deref() {
        Some("function") | Some("symbol-function") => want_sym(i, &p[1])?,
        Some("var") | Some("default-value") | Some("local") => {
            let sym = want_sym(i, &p[1])?;
            let cur = i.symbol_value(sym);
            let k = advice_wrap(i, &cur);
            i.set_symbol(sym, Value::Sym(k))?;
            k
        }
        Some("get") => {
            let sym = want_sym(i, &p[1])?;
            let prop = want_sym(i, &p[2])?;
            let cur = i.get_prop(sym, prop);
            let k = advice_wrap(i, &cur);
            i.put_prop(sym, prop, Value::Sym(k));
            k
        }
        // GNU's gv setter for a quoted place is the nonexistent
        // function `(setf quote)' — signal the same void-function.
        Some("setf-quote") => {
            let sf = i.intern("(setf quote)");
            return Err(i.signal_data(sym::VOID_FUNCTION, vec![Value::Sym(sf)]));
        }
        _ => {
            return Err(i.signal_data(
                sym::ERROR,
                vec![Value::string(format!(
                    "Unknown add-function place ‘{}’",
                    i.print_to_string(&a[1])
                ))],
            ))
        }
    };
    let name = advice_name(i, &arg(&a, 3));
    advice_push(i, key, w, a[2].clone(), name);
    Ok(Value::Nil)
}

/// `cl--remove-function` — (PLACE FUNCTION); mirror of the above.
fn f_remove_function(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let p = a[0]
        .list_to_vec()
        .map_err(|_| i.wrong_type_mut("listp", &a[0]))?;
    let kind = p
        .first()
        .and_then(|h| i.sym_id(h))
        .map(|s| i.symbol_name(s));
    let (holder, key) = match kind.as_deref() {
        Some("function") | Some("symbol-function") => (None, want_sym(i, &p[1])?),
        Some("var") | Some("default-value") | Some("local") => {
            let sym = want_sym(i, &p[1])?;
            let cur = i.symbol_value(sym);
            match advice_unwrap(i, &cur) {
                Some((k, orig)) => (Some(("var".to_string(), sym, orig, sym)), k),
                None => return Ok(Value::Nil),
            }
        }
        Some("get") => {
            let sym = want_sym(i, &p[1])?;
            let prop = want_sym(i, &p[2])?;
            match advice_unwrap(i, &i.get_prop(sym, prop)) {
                Some((k, orig)) => (Some(("get".to_string(), sym, orig, prop)), k),
                None => return Ok(Value::Nil),
            }
        }
        Some("setf-quote") => {
            let sf = i.intern("(setf quote)");
            return Err(i.signal_data(sym::VOID_FUNCTION, vec![Value::Sym(sf)]));
        }
        _ => return Ok(Value::Nil),
    };
    advice_retain(i, key, &a[1]);
    if i.advice_list(key).is_empty() {
        if let Some((kind, sym, orig, prop)) = holder {
            match kind.as_str() {
                "var" => i.set_symbol(sym, orig)?,
                _ => i.put_prop(sym, prop, orig),
            }
        }
    }
    Ok(Value::Nil)
}

fn f_advice_member_p(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let sym = match &a[1] {
        Value::Sym(s) => *s,
        _ => return Ok(Value::Nil),
    };
    // GNU returns the found advice object (or nil), not just t.
    let hit = i
        .advice_list(sym)
        .into_iter()
        .find(|(_, f, n)| advice_entry_matches(i, f, n, &a[0]));
    Ok(match hit {
        Some((_, f, _)) => f,
        None => Value::Nil,
    })
}

fn f_advice_function_mapc(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let sym = match &a[1] {
        Value::Sym(s) => *s,
        _ => return Ok(Value::Nil),
    };
    for (w, f, n) in i.advice_list(sym) {
        let name_kw = i.intern("name");
        let props = if n.is_nil() {
            Value::Nil
        } else {
            Value::list(vec![Value::cons(Value::Sym(name_kw), n)])
        };
        i.apply(&a[0], vec![f, props])?;
        let _ = w;
    }
    Ok(Value::Nil)
}

/// `(cl--advice--apply IDX ARGS...)` — apply advice wrapper IDX:
/// entry = (WHERE FUN NEXT).
fn f_advice_apply_link(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let idx = match &a[0] {
        Value::Int(n) => *n as usize,
        _ => return Err(i.wrong_type_mut("fixnump", &a[0])),
    };
    let (w, f, next) = match i.advice_links.get(idx) {
        Some(p) => p.clone(),
        None => return Err(i.error("Invalid advice link")),
    };
    let args: Vec<Value> = a[1..].to_vec();
    let wname = match &w {
        Value::Sym(s) => i.symbol_name(*s),
        _ => return Err(i.wrong_type_mut("symbolp", &w)),
    };
    match wname.as_str() {
        ":around" => {
            let mut argv = vec![next];
            argv.extend(args);
            i.apply(&f, argv)
        }
        ":override" => i.apply(&f, args),
        ":before" => {
            i.apply(&f, args.clone())?;
            i.apply(&next, args)
        }
        ":before-until" => {
            let r = i.apply(&f, args.clone())?;
            if r.truthy() {
                Ok(r)
            } else {
                i.apply(&next, args)
            }
        }
        ":before-while" => {
            if i.apply(&f, args.clone())?.truthy() {
                i.apply(&next, args)
            } else {
                Ok(Value::Nil)
            }
        }
        ":after" => {
            let r = i.apply(&next, args.clone())?;
            i.apply(&f, args)?;
            Ok(r)
        }
        ":after-until" => {
            let r = i.apply(&next, args.clone())?;
            let r2 = i.apply(&f, args)?;
            Ok(if r2.truthy() { r2 } else { r })
        }
        ":after-while" => {
            let r = i.apply(&next, args.clone())?;
            Ok(if i.apply(&f, args)?.truthy() {
                r
            } else {
                Value::Nil
            })
        }
        ":filter-args" => {
            let r = i.apply(&f, vec![Value::list(args)])?;
            let argv = r.list_to_vec().unwrap_or_else(|_| vec![r]);
            i.apply(&next, argv)
        }
        ":filter-return" => {
            let r = i.apply(&next, args)?;
            i.apply(&f, vec![r])
        }
        _ => Err(i.error("Invalid advice class")),
    }
}
