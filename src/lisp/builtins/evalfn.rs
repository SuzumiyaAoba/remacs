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
        f_ignore_errors_raw,
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
        4,
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
        "cl--advice--link",
        1,
        1,
        f_advice_link,
        "Internal: (FUN NEXT HOW PROPS) of an advice trampoline, or nil."
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
    // `display-warning'/`lwarn'/`warn' are elisp in GNU's warnings.el
    // (autoloaded via loaddefs); the prelude registers the autoload
    // stubs, and the Rust f_* helpers remain for internal callers.
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
    // GNU macroexp.el: non-nil inside byte compilation; remacs
    // always interprets, so it's nil.  `rx' et al. consult it.
    S!("macroexp-compiling-p", 0, 0, f_noop, ""),
    S!("declare-functionp", 1, 1, f_declare_functionp, ""),
    S!(
        "error-type",
        1,
        1,
        f_error_type,
        "Symbol naming the type of ERROR."
    ),
];

/// GNU cl-preloaded.el: an error object is a list `(TYPE . DATA)';
/// `error-type' is its car (nil stays nil, non-lists signal `listp').
fn f_error_type(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    match &a[0] {
        Value::Cons(c) => Ok(c.borrow().car.clone()),
        Value::Nil => Ok(Value::Nil),
        other => Err(i.wrong_type_mut("listp", other)),
    }
}

fn f_eval(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    // (eval FORM &optional LEXICAL) — GNU binds
    // `internal-interpreter-environment' to LEXICAL when nil (dynamic)
    // or a cons (an actual env), else to `(t)' (fresh lexical env).
    let lex = arg(&args, 1);
    let lexenv = if matches!(lex, Value::Cons(_)) {
        parse_lexenv_spec(i, &lex)
    } else if lex.is_nil() {
        None
    } else {
        crate::lisp::eval::lexenv_root()
    };
    i.explicit_eval_depth += 1;
    let id = i.intern("lexical-binding");
    i.specbind(id, if lex.is_nil() { Value::Nil } else { Value::t() })?;
    let saved = std::mem::replace(&mut i.lexenv, lexenv);
    let r = i.eval(&args[0]);
    i.lexenv = saved;
    i.unbind(1)?;
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
    // GNU: (macroexpand FORM &optional ENVIRONMENT); the env is
    // consulted through `macroexpand-all-environment'.
    if args.len() > 1 {
        let id = i.intern("macroexpand-all-environment");
        i.specbind(id, args[1].clone())?;
        let r = i.macroexpand(&args[0]);
        i.unbind(1)?;
        return r;
    }
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
            // GNU `macroexpand-1' (eval.c): a `(lambda ...)' form in
            // expansion position is a function expression, so its
            // expansion is `(function (lambda ...))'.
            if i.sym_is(&car, sym::LAMBDA) {
                return Ok(Value::list(vec![
                    Value::Sym(sym::FUNCTION),
                    form.clone(),
                ]));
            }
            // GNU `macroexpand-1' consults ENVIRONMENT (its second
            // argument) before the global definition; remacs reads it
            // from the dynamically bound `macroexpand-all-environment'.
            if args.len() > 1 {
                let env = &args[1];
                let mut tail = env.clone();
                loop {
                    match tail {
                        Value::Cons(cc) => {
                            let (a, d) = {
                                let b = cc.borrow();
                                (b.car.clone(), b.cdr.clone())
                            };
                            if let Value::Cons(e) = &a {
                                let (ek, ev) = {
                                    let b = e.borrow();
                                    (b.car.clone(), b.cdr.clone())
                                };
                                if let (Value::Sym(eid), Value::Sym(id)) = (ek, &car) {
                                    if eid == *id {
                                        if ev.is_nil() {
                                            return Ok(form.clone());
                                        }
                                        let argl = want_list(i, &cdr)?;
                                        return i.apply(&ev, argl);
                                    }
                                }
                            }
                            tail = d;
                        }
                        _ => break,
                    }
                }
            }
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
    // GNU: ENVIRONMENT is passed through the dynamic variable
    // `macroexpand-all-environment'; macros such as `rx'/`rx-let'
    // consult it for local definitions.
    let env = args.get(1).cloned().unwrap_or(Value::Nil);
    let id = i.intern("macroexpand-all-environment");
    i.specbind(id, env)?;
    let r = macroexpand_all(i, &args[0]);
    i.unbind(1)?;
    r
}

/// Whether FORM is a `(declare ...)' spec — a data position consumed
/// by `defun'/`lambda' declaration handling, not expanded.
fn is_declare_form(i: &Interp, form: &Value) -> bool {
    if let Value::Cons(c) = form {
        let b = c.borrow();
        if let Value::Sym(id) = b.car {
            return i.symbol_name(id) == "declare";
        }
    }
    false
}

/// GNU `macroexp--expand-all' applies the head symbol's
/// `compiler-macro' property as (apply HANDLER FORM (cdr FORM)),
/// following `symbol-function' aliases like `cl-compiler-macroexpand'.
/// A result `eq' to FORM punts; a handler error warns and punts.
/// Returns the replacement form to re-expand, or None.
fn try_compiler_macro(i: &mut Interp, form: &Value) -> EvalResult {
    let (car, cdr) = match form {
        Value::Cons(c) => {
            let b = c.borrow();
            (b.car.clone(), b.cdr.clone())
        }
        _ => return Ok(Value::Nil),
    };
    let mut func = car;
    let cmacro = i.intern("compiler-macro");
    let mut handler = Value::Nil;
    let mut guard = 0;
    while let Value::Sym(id) = func {
        let h = i.get_prop(id, cmacro);
        if !h.is_nil() {
            handler = h;
            break;
        }
        let sf = i.symbol_function(id);
        match sf {
            Value::Sym(next) => {
                guard += 1;
                if guard > 200 {
                    break;
                }
                func = Value::Sym(next);
            }
            _ => break,
        }
    }
    if handler.is_nil() {
        return Ok(Value::Nil);
    }
    let mut argv = vec![form.clone()];
    match want_list(i, &cdr) {
        Ok(v) => argv.extend(v),
        Err(_) => return Ok(Value::Nil),
    }
    match i.apply(&handler, argv) {
        Ok(newf) => {
            if super::eq_values(&newf, form) {
                Ok(Value::Nil)
            } else {
                Ok(newf)
            }
        }
        Err(_) => Ok(Value::Nil),
    }
}

/// Whether V can be a `setq' target symbol — a symbol other than
/// t/nil or a keyword (GNU `macroexp--expand-all' fast-path check).
fn plain_setq_var(i: &Interp, v: &Value) -> bool {
    match v {
        Value::Sym(sid) if *sid != sym::NIL && *sid != sym::T => {
            !i.symbol_name(*sid).starts_with(':')
        }
        _ => false,
    }
}

/// Whether symbol ID's function cell holds a macro — `macrop' on the
/// symbol, following its cell (including macro-autoload cells).
fn sym_macrop(i: &mut Interp, id: SymId) -> bool {
    match i.symbol_function(id) {
        Value::Lambda(l) => l.is_macro,
        Value::Cons(c) => {
            let (car, form) = {
                let b = c.borrow();
                (b.car.clone(), Value::Cons(c.clone()))
            };
            if i.sym_is(&car, sym::MACRO) {
                return true;
            }
            let auto_id = i.intern("autoload");
            if i.sym_is(&car, auto_id) {
                return form
                    .list_to_vec()
                    .ok()
                    .and_then(|v| v.get(4).map(|x| x.truthy()))
                    .unwrap_or(false);
            }
            false
        }
        _ => false,
    }
}

/// Whether VAR is dynamically bound in `macroexp--expand-all'
/// terms — `special-variable-p', a `macroexp--dynvars' member, or a
/// `byte-compile-bound-variables' member.  Such a formal cannot be
/// alpha-converted to a `let' binding (GNU's dynboundarg punt).
fn macroexp_dynbound_p(i: &mut Interp, var: SymId) -> bool {
    if i.obarray.symbol(var).special {
        return true;
    }
    for name in ["macroexp--dynvars", "byte-compile-bound-variables"] {
        let vid = i.intern(name);
        let v = i.symbol_value(vid);
        if let Value::Cons(_) = &v {
            if let Ok(items) = v.list_to_vec() {
                if items.iter().any(|x| i.sym_is(x, var)) {
                    return true;
                }
            }
        }
    }
    false
}

/// GNU `macroexp--unfold-lambda': `(funcall #'(lambda (X...) BODY...)
/// A...)' unfolds to `(let ((X A)...) BODY...)' when arity and
/// scoping allow — handling `&optional' (missing actuals bind nil)
/// and `&rest' (the formal binds `(list . remaining-actuals)').
/// Returns None when the lambda is not unfoldable (the caller then
/// keeps the original `funcall' form, like GNU's too-many/few-args
/// or dynamic-var punts).
fn unfold_funcall_lambda(
    i: &mut Interp,
    lam: &Value,
    actuals: &[Value],
) -> Result<Option<Value>, Flow> {
    let litems = match lam.list_to_vec() {
        Ok(v) if v.len() >= 2 => v,
        _ => return Ok(None),
    };
    let formals = match litems[1].list_to_vec() {
        Ok(v) => v,
        Err(_) => return Ok(None),
    };
    // `macroexp-parse-body': skip leading docstring/declare/interactive.
    let mut body_start = 2;
    while body_start < litems.len() {
        match &litems[body_start] {
            Value::Str(_) => body_start += 1,
            v if is_declare_form(i, v) => body_start += 1,
            Value::Cons(c) => {
                let is_ia = {
                    let b = c.borrow();
                    i.sym_is(&b.car, sym::INTERACTIVE)
                };
                if is_ia {
                    body_start += 1;
                } else {
                    break;
                }
            }
            _ => break,
        }
    }
    let mut bindings: Vec<Value> = Vec::with_capacity(formals.len());
    let mut rest: &[Value] = actuals;
    let mut optionalp = false;
    let mut restp = false;
    let mut exhausted = false;
    for (k, fo) in formals.iter().enumerate() {
        let sid = match i.sym_id(fo) {
            Some(sid) => sid,
            None => return Ok(None),
        };
        if sid == sym::OPTIONAL {
            if restp || k + 1 == formals.len() {
                return Ok(None);
            }
            optionalp = true;
            continue;
        }
        if sid == sym::REST {
            if k + 1 == formals.len() || k + 2 != formals.len() {
                return Ok(None);
            }
            restp = true;
            continue;
        }
        // Other `&'-keywords (`&key' etc.) are not unfoldable.
        if i.symbol_name(sid).starts_with('&') {
            return Ok(None);
        }
        if macroexp_dynbound_p(i, sid) {
            return Ok(None);
        }
        if restp {
            // The `&rest' formal soaks up the remaining actuals.
            let tail = if rest.is_empty() {
                Value::Nil
            } else {
                let mut l = vec![Value::Sym(i.intern("list"))];
                l.extend(rest.iter().cloned());
                Value::list(l)
            };
            bindings.push(Value::list(vec![fo.clone(), tail]));
            rest = &[];
            continue;
        }
        if !optionalp && rest.is_empty() {
            // Too few arguments: GNU warns and keeps the call form.
            exhausted = true;
            break;
        }
        let head = rest.first().cloned().unwrap_or(Value::Nil);
        bindings.push(Value::list(vec![fo.clone(), head]));
        if !rest.is_empty() {
            rest = &rest[1..];
        }
    }
    if exhausted || !rest.is_empty() {
        return Ok(None);
    }
    let body = &litems[body_start..];
    if bindings.is_empty() {
        // `(macroexp-progn body)'
        if body.len() == 1 {
            return Ok(Some(body[0].clone()));
        }
        let mut out = vec![Value::Sym(sym::PROGN)];
        out.extend(body.iter().cloned());
        return Ok(Some(Value::list(out)));
    }
    let mut out = vec![Value::Sym(sym::LET), Value::list(bindings)];
    out.extend(body.iter().cloned());
    Ok(Some(Value::list(out)))
}

/// Recursively expand macros throughout a form.
pub(crate) fn macroexpand_all(i: &mut Interp, form: &Value) -> EvalResult {
    let expanded = i.macroexpand(form)?;
    // Compiler macros expand through `macroexpand-all' but not
    // `macroexpand'/`macroexpand-1'.
    if let Value::Cons(_) = expanded {
        let newf = try_compiler_macro(i, &expanded)?;
        if !newf.is_nil() {
            return macroexpand_all(i, &newf);
        }
    }
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
            let _ = cdr;
            // Special forms whose non-code positions must not be
            // expanded: a parameter or binding variable can share a
            // name with a macro (e.g. `(defun f (rx) ...)'), and
            // expanding it would rewrite the arglist into the macro's
            // expansion — GNU's `macroexp--expand-all' keeps these
            // positions verbatim.
            let closure_id = i.intern("closure");
            let funcall_id = i.intern("funcall");
            if let Some(id) = i.sym_id(&car) {
                match id {
                    // `(closure ENV ARGLIST BODY...)': env and arglist
                    // are data positions like `lambda''s arglist.
                    _ if id == closure_id && items.len() >= 3 => {
                        let mut out = items[..3].to_vec();
                        let mut body = items[3..].iter();
                        for it in &mut body {
                            if matches!(it, Value::Str(_)) || is_declare_form(i, it) {
                                out.push(it.clone());
                            } else {
                                out.push(macroexpand_all(i, it)?);
                                break;
                            }
                        }
                        for it in body {
                            out.push(macroexpand_all(i, it)?);
                        }
                        return Ok(Value::list(out));
                    }
                    sym::DEFUN | sym::DEFMACRO if items.len() >= 3 => {
                        let mut out = items[..3].to_vec();
                        // GNU `macroexp--defun' consumes leading
                        // `(declare ...)' specs itself; they are data,
                        // not code, and must not be descended into
                        // (an `ftype' spec holds a `(function TYPE)'
                        // form that is not a function quote).
                        let mut body = items[3..].iter();
                        for it in &mut body {
                            if matches!(it, Value::Str(_)) || is_declare_form(i, it) {
                                out.push(it.clone());
                            } else {
                                out.push(macroexpand_all(i, it)?);
                                break;
                            }
                        }
                        for it in body {
                            out.push(macroexpand_all(i, it)?);
                        }
                        return Ok(Value::list(out));
                    }
                    sym::LAMBDA if items.len() >= 2 => {
                        let mut out = items[..2].to_vec();
                        // Same for `macroexp--lambda': leading
                        // `(declare ...)' forms pass through verbatim.
                        let mut body = items[2..].iter();
                        for it in &mut body {
                            if matches!(it, Value::Str(_)) || is_declare_form(i, it) {
                                out.push(it.clone());
                            } else {
                                out.push(macroexpand_all(i, it)?);
                                break;
                            }
                        }
                        for it in body {
                            out.push(macroexpand_all(i, it)?);
                        }
                        return Ok(Value::list(out));
                    }
                    sym::FUNCTION if items.len() == 2 => {
                        // GNU `macroexp--expand-all' only descends into
                        // a lambda-shaped function argument; any other
                        // `(function ...)' form is data (e.g. an `ftype'
                        // spec inside `declare') and passes through
                        // verbatim, extra elements included.
                        if let Value::Cons(lc) = &items[1] {
                            let is_lam = {
                                let b = lc.borrow();
                                i.sym_is(&b.car, sym::LAMBDA) || i.sym_is(&b.car, closure_id)
                            };
                            if is_lam {
                                // Expand the lambda's elements the way
                                // `macroexp--all-forms' does (arglist is
                                // data, body elements are code); do NOT
                                // recur through `macroexpand_all' on the
                                // whole `(lambda ...)' — `macroexpand-1'
                                // would just re-wrap it in `function'.
                                let is_clo = {
                                    let b = lc.borrow();
                                    i.sym_is(&b.car, closure_id)
                                };
                                let head = if is_clo { 3 } else { 2 };
                                let litems = match want_list(i, &items[1]) {
                                    Ok(v) if v.len() >= head => v,
                                    _ => return Ok(expanded),
                                };
                                let mut lout = litems[..head].to_vec();
                                let mut lbody = litems[head..].iter();
                                for it in &mut lbody {
                                    if matches!(it, Value::Str(_)) || is_declare_form(i, it) {
                                        lout.push(it.clone());
                                    } else {
                                        lout.push(macroexpand_all(i, it)?);
                                        break;
                                    }
                                }
                                for it in lbody {
                                    lout.push(macroexpand_all(i, it)?);
                                }
                                return Ok(Value::list(vec![
                                    items[0].clone(),
                                    Value::list(lout),
                                ]));
                            }
                        }
                        return Ok(expanded);
                    }
                    sym::LET | sym::LET_STAR | sym::AND_LET_STAR if items.len() >= 2 => {
                        let mut out = vec![items[0].clone()];
                        match &items[1] {
                            Value::Cons(_) => {
                                // GNU's `macroexp--expand-all' walks the
                                // varlist as a cons chain: each binding's
                                // value-forms expand while the list
                                // structure (including a dotted tail like
                                // `((x 1) . 2)') is preserved verbatim.
                                let mut newvarlist = Value::Nil;
                                let mut tail_cell: Option<Value> = None;
                                let mut cur = items[1].clone();
                                loop {
                                    match cur {
                                        Value::Cons(c) => {
                                            let (b, next) = {
                                                let bb = c.borrow();
                                                (bb.car.clone(), bb.cdr.clone())
                                            };
                                            let nb = {
                                                let keep_head = matches!(
                                                    &b,
                                                    Value::Cons(bc)
                                                        if matches!(bc.borrow().car, Value::Sym(_))
                                                );
                                                if keep_head {
                                                    let parts =
                                                        want_list(i, &b).unwrap_or_default();
                                                    if parts.is_empty() {
                                                        b.clone()
                                                    } else {
                                                        let mut nb = vec![parts[0].clone()];
                                                        for p in &parts[1..] {
                                                            nb.push(macroexpand_all(i, p)?);
                                                        }
                                                        Value::list(nb)
                                                    }
                                                } else {
                                                    macroexpand_all(i, &b)?
                                                }
                                            };
                                            let cell = Value::cons(nb, Value::Nil);
                                            if let Some(Value::Cons(prev)) = &tail_cell {
                                                prev.borrow_mut().cdr = cell.clone();
                                            } else {
                                                newvarlist = cell.clone();
                                            }
                                            tail_cell = Some(cell);
                                            cur = next;
                                        }
                                        other => {
                                            // Improper tail: kept verbatim.
                                            if let Some(Value::Cons(prev)) = &tail_cell {
                                                prev.borrow_mut().cdr = other;
                                            } else {
                                                newvarlist = other;
                                            }
                                            break;
                                        }
                                    }
                                }
                                out.push(newvarlist);
                            }
                            v => out.push(v.clone()),
                        }
                        for it in &items[2..] {
                            out.push(macroexpand_all(i, it)?);
                        }
                        return Ok(Value::list(out));
                    }
                    sym::SETQ | sym::SETQ_DEFAULT => {
                        // GNU `macroexp--expand-all' normalizes `setq'
                        // so the byte compiler only ever sees
                        // 3-element `(setq VAR EXPR)' forms; multi-pair
                        // setqs become `(progn (setq A E1) (setq B E2))'.
                        let args = &items[1..];
                        let nargs = args.len();
                        if nargs == 2 && plain_setq_var(i, &args[0]) {
                            let expr = macroexpand_all(i, &args[1])?;
                            return Ok(Value::list(vec![
                                items[0].clone(),
                                args[0].clone(),
                                expr,
                            ]));
                        }
                        if nargs % 2 == 1 {
                            // `(signal 'wrong-number-of-arguments
                            //          '(setq NARGS))'
                            return Ok(Value::list(vec![
                                Value::Sym(i.intern("signal")),
                                Value::list(vec![
                                    Value::Sym(sym::QUOTE),
                                    Value::Sym(sym::WRONG_NUMBER_OF_ARGUMENTS),
                                ]),
                                Value::list(vec![
                                    Value::Sym(sym::QUOTE),
                                    Value::list(vec![
                                        items[0].clone(),
                                        Value::Int(nargs as i128),
                                    ]),
                                ]),
                            ]));
                        }
                        let mut assigns = Vec::new();
                        for pair in args.chunks(2) {
                            let var = &pair[0];
                            let expr = macroexpand_all(i, &pair[1])?;
                            let assignment = if plain_setq_var(i, var) {
                                Value::list(vec![items[0].clone(), var.clone(), expr])
                            } else if matches!(var, Value::Sym(sid) if i.symbol_name(*sid).starts_with(':'))
                            {
                                // `(if (eq :k E) :k (signal 'setting-constant
                                //                          (list ':k)))'
                                Value::list(vec![
                                    Value::Sym(sym::IF),
                                    Value::list(vec![
                                        Value::Sym(i.intern("eq")),
                                        var.clone(),
                                        expr,
                                    ]),
                                    var.clone(),
                                    Value::list(vec![
                                        Value::Sym(i.intern("signal")),
                                        Value::list(vec![
                                            Value::Sym(sym::QUOTE),
                                            Value::Sym(sym::SETTING_CONSTANT),
                                        ]),
                                        Value::list(vec![
                                            Value::Sym(i.intern("list")),
                                            Value::list(vec![
                                                Value::Sym(sym::QUOTE),
                                                var.clone(),
                                            ]),
                                        ]),
                                    ]),
                                ])
                            } else {
                                // `(signal 'setting-constant (list 'VAR))'
                                // for t/nil, else a wrong-type-argument.
                                let tag = match var {
                                    Value::Sym(_) => sym::SETTING_CONSTANT,
                                    _ => sym::WRONG_TYPE_ARGUMENT,
                                };
                                let data = match var {
                                    Value::Sym(_) => Value::list(vec![
                                        Value::Sym(i.intern("list")),
                                        Value::list(vec![
                                            Value::Sym(sym::QUOTE),
                                            var.clone(),
                                        ]),
                                    ]),
                                    _ => Value::list(vec![
                                        Value::Sym(i.intern("list")),
                                        Value::list(vec![
                                            Value::Sym(sym::QUOTE),
                                            Value::Sym(i.intern("symbolp")),
                                        ]),
                                        Value::list(vec![
                                            Value::Sym(sym::QUOTE),
                                            var.clone(),
                                        ]),
                                    ]),
                                };
                                Value::list(vec![
                                    Value::Sym(i.intern("signal")),
                                    Value::list(vec![Value::Sym(sym::QUOTE), Value::Sym(tag)]),
                                    data,
                                ])
                            };
                            assigns.push(assignment);
                        }
                        return Ok(Value::cons(Value::Sym(sym::PROGN), Value::list(assigns)));
                    }
                    _ if id == funcall_id => {
                        // GNU `macroexp--expand-all' rewrites
                        // `(funcall #'F A...)' to `(F A...)' when F is a
                        // plain function symbol, and unfolds
                        // `(funcall #'(lambda (X) E) A)' to
                        // `(let ((X A)) E)'.
                        if items.len() < 2 {
                            // `(funcall)' alone: GNU keeps it verbatim
                            // (bug#53227).
                            return Ok(expanded);
                        }
                        let eexp = macroexpand_all(i, &items[1])?;
                        let mut eargs = Vec::with_capacity(items.len() - 2);
                        for a in &items[2..] {
                            eargs.push(macroexpand_all(i, a)?);
                        }
                        let mut f: Option<Value> = None;
                        if let Value::Cons(c) = &eexp {
                            let is_fn = {
                                let b = c.borrow();
                                i.sym_is(&b.car, sym::FUNCTION)
                            };
                            if is_fn {
                                if let Ok(v) = eexp.list_to_vec() {
                                    if v.len() == 2 {
                                        f = Some(v[1].clone());
                                    }
                                }
                            }
                        }
                        if let Some(fv) = f {
                            match i.sym_id(&fv) {
                                Some(fsid) => {
                                    // `(funcall #'sym A...)' → `(sym A...)'
                                    // unless sym is a special form or macro.
                                    let is_sf = crate::lisp::special::special_form(fsid)
                                        .is_some();
                                    if !is_sf && !sym_macrop(i, fsid) {
                                        let mut call = vec![fv.clone()];
                                        call.extend(eargs);
                                        return macroexpand_all(i, &Value::list(call));
                                    }
                                }
                                None => {
                                    if let Value::Cons(lc) = &fv {
                                        let is_lam = {
                                            let b = lc.borrow();
                                            i.sym_is(&b.car, sym::LAMBDA)
                                        };
                                        if is_lam {
                                            if let Some(unfolded) =
                                                unfold_funcall_lambda(i, &fv, &eargs)?
                                            {
                                                return Ok(unfolded);
                                            }
                                            // Unfoldable-but-unsafe shapes
                                            // keep the whole original form.
                                            return Ok(expanded);
                                        }
                                    }
                                }
                            }
                        }
                        let mut out = vec![items[0].clone(), eexp];
                        out.extend(eargs);
                        return Ok(Value::list(out));
                    }
                    sym::CONDITION_CASE if items.len() >= 3 => {
                        let mut out = vec![
                            items[0].clone(),
                            items[1].clone(),
                            macroexpand_all(i, &items[2])?,
                        ];
                        for h in &items[3..] {
                            let is_cons = matches!(h, Value::Cons(_));
                            if is_cons {
                                let parts = want_list(i, h).unwrap_or_default();
                                if !parts.is_empty() {
                                    let mut nh = vec![parts[0].clone()];
                                    for p in &parts[1..] {
                                        nh.push(macroexpand_all(i, p)?);
                                    }
                                    out.push(Value::list(nh));
                                    continue;
                                }
                            }
                            out.push(h.clone());
                        }
                        return Ok(Value::list(out));
                    }
                    _ => {}
                }
            }
            // GNU `macroexp--expand-all' case `(,(lambda ...) . ,args)':
            // a lambda in function position keeps its `(lambda ARGLIST)'
            // head verbatim (no `function' wrap) while its body and the
            // call arguments expand.
            if let Value::Cons(lc) = &items[0] {
                let is_lam = {
                    let b = lc.borrow();
                    i.sym_is(&b.car, sym::LAMBDA)
                };
                if is_lam {
                    let litems = match want_list(i, &items[0]) {
                        Ok(v) if v.len() >= 2 => v,
                        _ => return Ok(expanded),
                    };
                    let mut fun_items = litems[..2].to_vec();
                    for it in &litems[2..] {
                        fun_items.push(macroexpand_all(i, it)?);
                    }
                    let mut res = vec![Value::list(fun_items)];
                    for it in &items[1..] {
                        res.push(macroexpand_all(i, it)?);
                    }
                    return Ok(Value::list(res));
                }
            }
            let mut out = Vec::with_capacity(items.len());
            for it in items {
                out.push(macroexpand_all(i, &it)?);
            }
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
                Some(l @ ('d' | 'x' | 'o' | 'c' | 'e' | 'f' | 'g')) => {
                    if let Some(a) = args.get(ai) {
                        ai += 1;
                        // GNU's error/message path uses the full format
                        // semantics for numeric specs (e.g. %c renders a
                        // character); delegate to `format'.
                        let spec = format!("%{}", l);
                        let spec_args = [Value::Nil, a.clone()];
                        match super::strfn::format_impl(i, &spec, &spec_args) {
                            Ok(Value::Str(s)) => out.push_str(&s.borrow()),
                            _ => out.push_str(&i.princ_to_string(a)),
                        }
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
fn f_ignore_errors_raw(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let body = args.into_iter().next().unwrap_or(Value::Nil);
    let err = Value::Sym(i.intern("error"));
    ignore_body_forms(i, &err, &body)
}

/// `(ignore-error CONDITION BODY...)` — GNU's two-arg form: the first
/// raw arg element is the error-condition spec, the rest is body.
fn f_ignore_error_raw(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let all = args.into_iter().next().unwrap_or(Value::Nil);
    let (cond, body) = match &all {
        Value::Cons(c) => {
            let b = c.borrow();
            (b.car.clone(), b.cdr.clone())
        }
        _ => (Value::Nil, Value::Nil),
    };
    ignore_body_forms(i, &cond, &body)
}

fn ignore_body_forms(i: &mut Interp, cond: &Value, body: &Value) -> EvalResult {
    // Register the claimed condition so handler-bind handlers don't
    // run for signals this form will swallow (GNU suppresses them via
    // the no-debugger-entry rule).
    i.case_handlers.push(Value::list(vec![cond.clone()]));
    let r = i.eval_progn(body);
    i.case_handlers.pop();
    match r {
        Ok(v) => Ok(v),
        Err(Flow::Signal(sig, data, offered)) => {
            if i.signal_matches(&sig, cond) {
                Ok(Value::Nil)
            } else {
                Err(Flow::Signal(sig, data, offered))
            }
        }
        Err(e) => Err(e),
    }
}

fn f_with_demoted_errors(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    f_ignore_errors_raw(i, args)
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
            // GNU's `require': after loading FILE, FEATURE must be
            // provided — a file that loads without `(provide FEATURE)'
            // signals `error' ("Loading file FILE failed to provide
            // feature ‘FEATURE’", e.g. userlock.el, buff-menu.el).
            if !i.features.contains(&id) {
                return Err(i.signal_data(
                    sym::ERROR,
                    vec![Value::string(format!(
                        "Loading file {} failed to provide feature ‘{}’",
                        name,
                        i.symbol_name(id)
                    ))],
                ));
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

/// Normalize a hook value to a function list (a single non-list
/// function counts as one element).
fn hook_list(v: &Value) -> Vec<Value> {
    match v {
        Value::Cons(_) => v.list_to_vec().unwrap_or_default(),
        Value::Sym(s) if *s == sym::UNBOUND => Vec::new(),
        Value::Nil => Vec::new(),
        other => vec![other.clone()],
    }
}

fn hook_fns(i: &Interp, hook: &Value) -> Vec<Value> {
    let id = match i.sym_id(hook) {
        Some(s) => s,
        None => return Vec::new(),
    };
    // GNU `run-hooks': when the buffer-local binding contains `t' as
    // an element, the global (default) value is run at the end.
    let local = i.buffers.get(i.current_buffer).and_then(|b| {
        b.try_borrow()
            .ok()
            .and_then(|bb| bb.locals.get(&id).cloned())
    });
    match local {
        Some(v) => {
            let mut fns = hook_list(&v);
            if fns
                .iter()
                .any(|f| matches!(f, Value::Sym(s) if *s == sym::T))
            {
                fns.retain(|f| !matches!(f, Value::Sym(s) if *s == sym::T));
                let mut globals = hook_list(&i.obarray.symbol(id).value);
                fns.append(&mut globals);
            }
            fns
        }
        None => hook_list(&i.obarray.symbol(id).value),
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
    // GNU eval.c: each hook function is called through WRAP-FUNCTION;
    // if a call returns non-nil, hook processing aborts and returns it.
    let hook = args[0].clone();
    let wrap = args[1].clone();
    let fns = hook_fns(i, &hook);
    for f in fns {
        let mut call_args = vec![f];
        call_args.extend(args[2..].iter().cloned());
        let res = i.apply(&wrap, call_args)?;
        if res.truthy() {
            return Ok(res);
        }
    }
    Ok(Value::Nil)
}

/// GNU `inhibit_modification_hooks': `inhibit-modification-hooks'
/// (bound by `with-silent-modifications' and `combine-after-change-*'
/// internals) suppresses the before/after-change hooks entirely.
pub(crate) fn mod_hooks_inhibited(i: &Interp) -> bool {
    i.intern_soft("inhibit-modification-hooks")
        .map(|sid| i.symbol_value(sid).truthy())
        .unwrap_or(false)
}

/// GNU `signal_before_change': run `first-change-hook' (when the
/// buffer was unmodified) then `before-change-functions' with the
/// 1-based region about to change.  An error aborts the modification.
pub(crate) fn signal_before_change(i: &mut Interp, beg1: usize, end1: usize) -> EvalResult {
    if mod_hooks_inhibited(i) {
        return Ok(Value::Nil);
    }
    let unmodified = !i
        .buffers
        .get(i.current_buffer)
        .map(|b| b.borrow().modified)
        .unwrap_or(false);
    if unmodified {
        let id = i.intern("first-change-hook");
        let fns = hook_fns(i, &Value::Sym(id));
        for f in fns {
            i.apply(&f, vec![])?;
        }
    }
    let args = vec![Value::Int(beg1 as i128), Value::Int(end1 as i128)];
    let id = i.intern("before-change-functions");
    let fns = hook_fns(i, &Value::Sym(id));
    for f in fns {
        i.apply(&f, args.clone())?;
    }
    Ok(Value::Nil)
}

/// GNU `report_after_change': `after-change-functions' with
/// (BEG END LEN); LEN is the char length of the replaced text.
pub(crate) fn signal_after_change(
    i: &mut Interp,
    beg1: usize,
    end1: usize,
    len: usize,
) -> EvalResult {
    if mod_hooks_inhibited(i) {
        return Ok(Value::Nil);
    }
    let args = vec![
        Value::Int(beg1 as i128),
        Value::Int(end1 as i128),
        Value::Int(len as i128),
    ];
    let id = i.intern("after-change-functions");
    let fns = hook_fns(i, &Value::Sym(id));
    for f in fns {
        i.apply(&f, args.clone())?;
    }
    Ok(Value::Nil)
}

/// GNU `run_hook_with_args' with no args: apply each function bound
/// on the hook variable NAME (buffer-local `t' splicing included).
pub(crate) fn call_hook(i: &mut Interp, name: &str) -> EvalResult {
    let id = i.intern(name);
    let fns = hook_fns(i, &Value::Sym(id));
    for f in fns {
        i.apply(&f, vec![])?;
    }
    Ok(Value::Nil)
}

/// GNU `safe_run_hooks' (keyboard.c): `inhibit-quit' is bound, and
/// each hook function runs inside an error handler that messages
/// "Error in HOOK (FUN): ERROR" and removes FUN from the hook.
/// Signals and quits are swallowed; throws/exits still propagate.
pub(crate) fn safe_call_hook(i: &mut Interp, name: &str) -> EvalResult {
    let mark = i.specbind_depth();
    let iq = i.intern("inhibit-quit");
    if let Err(f) = i.specbind(iq, Value::t()) {
        return Err(f);
    }
    let hook_id = i.intern(name);
    let fns = hook_fns(i, &Value::Sym(hook_id));
    for f in fns {
        match i.apply(&f, vec![]) {
            Ok(_) | Err(Flow::Quit) => {}
            Err(Flow::Signal(sig, data, _)) => {
                let err = Value::cons(sig.clone(), data.clone());
                let msg = format!(
                    "Error in {} ({}): {}",
                    name,
                    i.prin1_to_string(&f),
                    i.prin1_to_string(&err)
                );
                i.message(&msg);
                safe_remove_hook_fn(i, hook_id, &f);
            }
            Err(flow) => {
                let _ = i.unbind_to(mark);
                return Err(flow);
            }
        }
    }
    let _ = i.unbind_to(mark);
    Ok(Value::Nil)
}

/// GNU `safe_run_hooks_error': drop FUN from HOOK's buffer-local list
/// if present there, else from its global (default) value.
fn safe_remove_hook_fn(i: &mut Interp, hook_id: SymId, f: &Value) {
    if let Some(b) = i.buffers.get(i.current_buffer) {
        let mut bb = b.borrow_mut();
        if let Some(v) = bb.locals.get(&hook_id).cloned() {
            let mut l = hook_list(&v);
            if let Some(pos) = l.iter().position(|x| super::equal_values(i, x, f)) {
                l.remove(pos);
                bb.locals.insert(hook_id, Value::list(l));
                return;
            }
        }
    }
    let v = i.obarray.symbol(hook_id).value.clone();
    let mut l = hook_list(&v);
    if let Some(pos) = l.iter().position(|x| super::equal_values(i, x, f)) {
        l.remove(pos);
        let _ = i.set_symbol(hook_id, Value::list(l));
    }
}

// `add-hook'/`remove-hook' mirror GNU 31.1's Lisp definitions in
// subr.el, including depth bookkeeping via the `hook--depth-alist'
// symbol property (an uninterned symbol whose value is an alist).

fn hook_is_local(i: &Interp, id: SymId) -> bool {
    i.obarray.symbol(id).always_local || {
        i.buffers
            .get(i.current_buffer)
            .map(|b| b.borrow().locals.contains_key(&id))
            .unwrap_or(false)
    }
}

fn hook_local_if_set(i: &Interp, id: SymId) -> bool {
    let s = i.obarray.symbol(id);
    s.make_local_if_set || s.always_local
}

fn hook_boundp(i: &Interp, id: SymId) -> bool {
    !matches!(i.symbol_value(id), Value::Sym(s) if s == sym::UNBOUND)
}

fn hook_default_boundp(i: &Interp, id: SymId) -> bool {
    !matches!(i.obarray.symbol(id).value, Value::Sym(s) if s == sym::UNBOUND)
}

/// `(make-local-variable SYM)': create the buffer-local binding (seeded
/// with the default value) when absent.
fn hook_make_local(i: &mut Interp, id: SymId) {
    let seed = i.obarray.symbol(id).value.clone();
    if let Some(b) = i.buffers.get(i.current_buffer) {
        if let Ok(mut bb) = b.try_borrow_mut() {
            bb.locals.entry(id).or_insert(seed);
        }
    }
}

/// `(local-variable-p HOOK) and value lacks a `t' member' — GNU treats
/// a global add/remove on such a hook as local (make-local-variable
/// compatibility).
fn hook_force_local(i: &mut Interp, id: SymId) -> bool {
    if !hook_is_local(i, id) {
        return false;
    }
    let v = i.symbol_value(id);
    match v.list_to_vec() {
        Ok(list) => !list
            .iter()
            .any(|x| matches!(x, Value::Sym(s) if *s == sym::T)),
        Err(_) => true,
    }
}

/// The `hook--depth-alist' property holds an uninterned symbol whose
/// (possibly buffer-local) value is the fn->depth alist.
fn hook_depth_alist(
    i: &mut Interp,
    hook_id: SymId,
    local: bool,
    create: bool,
) -> Option<Vec<(Value, i128)>> {
    let prop_sym = i.intern("hook--depth-alist");
    let prop = i.get_prop(hook_id, prop_sym);
    let dsym = match prop {
        Value::Sym(s) => s,
        _ => {
            if !create {
                return None;
            }
            let s = i.obarray.make_symbol("depth-alist");
            i.set_symbol_default(s, Value::Nil).ok()?;
            i.put_prop(hook_id, prop_sym, Value::Sym(s));
            s
        }
    };
    if local && create {
        hook_make_local(i, dsym);
    }
    let v = if local {
        i.symbol_value(dsym)
    } else {
        i.obarray.symbol(dsym).value.clone()
    };
    match v.list_to_vec() {
        Ok(items) => Some(
            items
                .iter()
                .filter_map(|x| match x {
                    Value::Cons(c) => {
                        let b = c.borrow();
                        match b.cdr {
                            Value::Int(n) => Some((b.car.clone(), n)),
                            _ => None,
                        }
                    }
                    _ => None,
                })
                .collect(),
        ),
        Err(_) => Some(Vec::new()),
    }
}

fn hook_set_depth_alist(i: &mut Interp, hook_id: SymId, local: bool, alist: Vec<(Value, i128)>) {
    let prop_sym = i.intern("hook--depth-alist");
    let prop = i.get_prop(hook_id, prop_sym);
    let dsym = match prop {
        Value::Sym(s) => s,
        _ => return,
    };
    let items: Vec<Value> = alist
        .into_iter()
        .map(|(f, d)| Value::cons(f, Value::Int(d)))
        .collect();
    let v = Value::list(items);
    if local {
        if let Some(b) = i.buffers.get(i.current_buffer) {
            if let Ok(mut bb) = b.try_borrow_mut() {
                if bb.locals.contains_key(&dsym) {
                    bb.locals.insert(dsym, v);
                    return;
                }
            }
        }
    }
    i.obarray.symbol_mut(dsym).value = v;
}

fn f_add_hook(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let hook_id = want_sym(i, &args[0])?;
    let fun = args[1].clone();
    let depth = match args.get(2) {
        Some(Value::Int(n)) => *n,
        Some(Value::Nil) | None => 0,
        Some(_) => 90, // any other non-nil symbol/object counts as 90
    };
    let mut local = args.get(3).map(|v| v.truthy()).unwrap_or(false);
    // (or (boundp hook) (set hook nil))
    // (or (default-boundp hook) (set-default hook nil))
    if !hook_boundp(i, hook_id) {
        i.set_symbol(hook_id, Value::Nil)?;
    }
    if !hook_default_boundp(i, hook_id) {
        i.set_symbol_default(hook_id, Value::Nil)?;
    }
    if local {
        // Unless the var is automatically buffer-local, the binding
        // is created fresh holding (t) -- the global value is not
        // copied into the local list.
        if !hook_local_if_set(i, hook_id) {
            hook_make_local(i, hook_id);
            i.set_symbol(hook_id, Value::list(vec![Value::Sym(sym::T)]))?;
        }
    } else if hook_force_local(i, hook_id) {
        local = true;
    }
    let mut hook_value = if local {
        i.symbol_value(hook_id)
    } else {
        i.obarray.symbol(hook_id).value.clone()
    };
    // GNU: `(or (not (listp hook-value)) (functionp hook-value))' is
    // wrapped in a list -- a bare function value becomes one element.
    if matches!(hook_value, Value::Sym(s) if s == sym::UNBOUND) {
        hook_value = Value::Nil;
    }
    if hook_value.list_to_vec().is_err() || i.function_p(&hook_value) {
        hook_value = Value::list(vec![hook_value]);
    }
    let mut list = hook_value.list_to_vec().unwrap_or_default();
    // The membership test uses `equal' (GNU `member').
    if !list.iter().any(|f| super::equal_values(i, f, &fun)) {
        let mut alist = hook_depth_alist(i, hook_id, local, depth != 0).unwrap_or_default();
        if depth != 0 {
            alist.retain(|(f, _)| !crate::lisp::eq_values(f, &fun));
            alist.push((fun.clone(), depth));
            hook_set_depth_alist(i, hook_id, local, alist.clone());
        }
        if depth > 0 {
            list.push(fun.clone());
        } else {
            list.insert(0, fun.clone());
        }
        // When a depth alist exists, stable-sort by recorded depth.
        let depth_alist = hook_depth_alist(i, hook_id, local, false).unwrap_or_default();
        if !depth_alist.is_empty() {
            let d_of = |f: &Value| -> i128 {
                depth_alist
                    .iter()
                    .find(|(x, _)| crate::lisp::eq_values(x, f))
                    .map(|(_, d)| *d)
                    .unwrap_or(0)
            };
            // stable sort keeps original order for equal depths only
            // when the new element appended (>0); for <=0 GNU's `sort'
            // puts equal-depth items after the consed newcomer anyway,
            // matching: copy before sort when depth<=0 (GNU does
            // copy-sequence for <=0 and sorts the fresh append for >0;
            // sort is stable in both cases).
            list.sort_by_key(|f| d_of(f));
        }
    }
    if local {
        // permanent-local-hook marking for mode-survival.
        if let Value::Sym(fs) = fun {
            let plh_sym = i.intern("permanent-local-hook");
            let pl_sym = i.intern("permanent-local");
            if i.get_prop(fs, plh_sym).truthy() && !i.get_prop(hook_id, pl_sym).truthy() {
                i.put_prop(hook_id, pl_sym, Value::Sym(plh_sym));
            }
        }
        i.set_symbol(hook_id, Value::list(list))?;
    } else {
        i.set_symbol_default(hook_id, Value::list(list))?;
    }
    Ok(Value::Nil)
}

fn f_remove_hook(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let hook_id = want_sym(i, &args[0])?;
    let fun = args[1].clone();
    let mut local = args.get(2).map(|v| v.truthy()).unwrap_or(false);
    if !hook_boundp(i, hook_id) {
        i.set_symbol(hook_id, Value::Nil)?;
    }
    if !hook_default_boundp(i, hook_id) {
        i.set_symbol_default(hook_id, Value::Nil)?;
    }
    // Do nothing if LOCAL is t but this hook has no local binding.
    if local && !hook_is_local(i, hook_id) {
        return Ok(Value::Nil);
    }
    if hook_force_local(i, hook_id) {
        local = true;
    }
    let hook_value = if local {
        i.symbol_value(hook_id)
    } else {
        i.obarray.symbol(hook_id).value.clone()
    };
    let (list, removed) = match hook_value.list_to_vec() {
        // A non-list value or a bare lambda form compares with `equal'.
        Err(_) if super::equal_values(i, &hook_value, &fun) => {
            (Vec::new(), Some(hook_value.clone()))
        }
        Err(_) => (Vec::new(), None),
        Ok(items) => {
            let is_lambda = matches!(items.first(), Some(Value::Sym(s)) if *s == sym::LAMBDA);
            if is_lambda && super::equal_values(i, &hook_value, &fun) {
                (Vec::new(), Some(hook_value.clone()))
            } else {
                // GNU: `(car (member ...))' locates with `equal';
                // `remq' then removes that same object.
                match items.iter().position(|f| super::equal_values(i, f, &fun)) {
                    Some(p) => {
                        let old = items[p].clone();
                        let mut v = items;
                        v.retain(|x| !crate::lisp::eq_values(x, &old));
                        (v, Some(old))
                    }
                    None => (items, None),
                }
            }
        }
    };
    if let Some(old) = removed {
        // Drop the removed function's depth record.
        if let Some(mut alist) = hook_depth_alist(i, hook_id, local, false) {
            let before = alist.len();
            alist.retain(|(f, _)| !crate::lisp::eq_values(f, &old));
            if alist.len() != before {
                hook_set_depth_alist(i, hook_id, local, alist);
            }
        }
    }
    if !local {
        i.set_symbol_default(hook_id, Value::list(list))?;
    } else if list.len() == 1 && matches!(&list[0], Value::Sym(s) if *s == sym::T) {
        // A local value of exactly (t) kills the local binding.
        if let Some(b) = i.buffers.get(i.current_buffer) {
            if let Ok(mut bb) = b.try_borrow_mut() {
                bb.locals.remove(&hook_id);
            }
        }
    } else {
        i.set_symbol(hook_id, Value::list(list))?;
    }
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
    // GNU (simple.el): the value is consed onto `values'; INSERT-VALUE
    // non-nil inserts the printed value into the buffer instead of
    // echoing it.
    let v = i.eval(&args[0])?;
    let printed = i.print_to_string(&v);
    if arg(&args, 1).truthy() {
        let ins = Value::Sym(i.intern("insert"));
        i.apply(&ins, vec![Value::string(printed)])?;
    } else {
        i.message(&printed);
    }
    let vals = i.intern("values");
    let cur = i.symbol_value(vals);
    let _ = i.set_symbol(vals, Value::cons(v.clone(), cur));
    Ok(v)
}
fn f_load(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    // (load FILE &optional NOERROR NOMESSAGE NOSUFFIX MUST-SUFFIX)
    let name = match &args[0] {
        Value::Str(s) => s.borrow().clone(),
        _ => return Err(i.wrong_type_mut("stringp", &args[0])),
    };
    let noerror = arg(&args, 1).truthy();
    let nomessage = arg(&args, 2).truthy();
    let nosuffix = arg(&args, 3).truthy();
    let mustsuffix = arg(&args, 4).truthy();
    let ok = crate::lisp::load::load_library_opts(i, &name, nosuffix, mustsuffix, nomessage)?;
    if ok {
        Ok(Value::t())
    } else if noerror {
        Ok(Value::Nil)
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
    // GNU's `load-file': (load (expand-file-name FILE) nil nil t) —
    // NOSUFFIX, so the literal name is opened (or fails).
    let name = match &args[0] {
        Value::Str(s) => s.borrow().clone(),
        _ => return Err(i.wrong_type_mut("stringp", &args[0])),
    };
    let efn = i.intern("expand-file-name");
    let expanded = i.apply(&Value::Sym(efn), vec![args[0].clone()])?;
    let name = match &expanded {
        Value::Str(s) => s.borrow().clone(),
        _ => name,
    };
    let ok = crate::lisp::load::load_library_opts(i, &name, true, false, false)?;
    if ok {
        Ok(Value::t())
    } else {
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
    // GNU's `autoload' returns FUNCTION.
    Ok(args[0].clone())
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
pub(crate) fn autoload_do_load(i: &mut Interp, fundef: Value, macro_only: bool) -> EvalResult {
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
        .find(|id| super::eq_values(&i.symbol_function(*id), &fundef))
        // Interpreted code can hold a structurally identical but
        // distinct `(autoload ...)' cons — e.g. one read afresh from a
        // loaddefs form while a different cons sits in the function
        // cell.  Fall back to an `equal' match so we still attribute
        // the autoload to its symbol (and to its dumped stash).
        .or_else(|| {
            i.obarray
                .all_ids()
                .into_iter()
                .find(|id| super::equal_values(i, &i.symbol_function(*id), &fundef))
        });
    if std::env::var_os("REMACS_TRACE_AUTOLOAD").is_some() {
        eprintln!(
            "[autoload {} -> {}]{}",
            owner.map(|id| i.symbol_name(id)).unwrap_or("?".into()),
            name,
            if owner.is_none() {
                let cl = i.intern("cl-loop");
                format!(
                    " cell={} | cl-loop-cell={}",
                    i.print_to_string(&fundef),
                    i.print_to_string(&i.symbol_function(cl))
                )
            } else {
                String::new()
            }
        );
    }
    // Interpreted built-ins can meet a self-autoload: cl-loaddefs marks
    // macros such as `cl-loop' as (autoload "cl-macs"), and loading
    // cl-macs.el then expands bodies containing `cl-loop'.  GNU's .elc
    // files are already expanded, so this cycle does not exist there.
    // Resolve the dump-time definition instead of recursively loading
    // the same file.
    let stem = |s: &str| {
        let s = s.strip_prefix("builtin:").unwrap_or(s);
        let base = s.rsplit('/').next().unwrap_or(s);
        let base = base.strip_suffix(".el").unwrap_or(base);
        base.strip_suffix(".elc").unwrap_or(base).to_string()
    };
    let lfn = i.intern("load-file-name");
    let loading_same = match i.symbol_value(lfn) {
        Value::Str(s) => stem(&s.borrow()) == stem(&name),
        _ => false,
    };
    if let Some(id) = owner {
        let pk = i.intern("remacs--dump-fn");
        let hidden = i.get_prop(id, pk);
        let has_hidden =
            !hidden.is_nil() && !matches!(hidden, Value::Sym(s) if s == crate::lisp::sym::UNBOUND);
        if (macro_only || i.macroexp_call_depth > 0 || (loading_same && is_macro_autoload))
            && has_hidden
        {
            return Ok(hidden);
        }
        if loading_same {
            return Err(i.error(format!(
                "Autoloading file {} recursively for {}",
                name,
                i.symbol_name(id)
            )));
        }
    } else if loading_same && macro_only {
        // Orphan autoload cell (a structurally distinct cons that no
        // symbol's function cell points at) targeting the file being
        // loaded.  GNU errors here; but our interpreted .el expansion
        // reaches these through embedded `(autoload ...)' literals in
        // expansions, so leave the cell in place: callers treat the
        // still-autoload result as "not expandable" and the real
        // dispatch resolves the owning symbol's cell instead.
        return Ok(fundef);
    } else if loading_same {
        return Err(i.error(format!("Autoloading file {name} recursively")));
    }
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
                // Some autoload cells point at a reduced source whose
                // GNU .elc equivalent defines the function inline.  If
                // the dump-time definition was stashed for -Q parity,
                // keep the public autoload cell but use that definition
                // for this call rather than failing the autoload.
                let pk = i.intern("remacs--dump-fn");
                let hidden = i.get_prop(id, pk);
                if !hidden.is_nil()
                    && !matches!(hidden, Value::Sym(s) if s == crate::lisp::sym::UNBOUND)
                {
                    return Ok(hidden);
                }
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
            let estr = estr.split(" (os error").next().unwrap_or(&estr).to_string();
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
    if std::env::var_os("REMACS_TRACE_DECLARE").is_some() {
        eprintln!("[declare-call] args={:?}", _args.first());
    }
    let _ = i;
    Ok(Value::Nil)
}
fn f_eval_and_compile(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let body = args.into_iter().next().unwrap_or(Value::Nil);
    i.eval_progn(&body)
}
fn f_eval_when_compile(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    // GNU: when the code is not being byte-compiled, `eval-when-compile'
    // evaluates its body like `progn' (the compile-time-only behavior only
    // applies inside the byte compiler, which remacs does not have).
    let body = args.into_iter().next().unwrap_or(Value::Nil);
    i.eval_progn(&body)
}
fn f_with_no_warnings(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let body = args.into_iter().next().unwrap_or(Value::Nil);
    i.eval_progn(&body)
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
fn f_ding(i: &mut Interp, _args: Vec<Value>) -> EvalResult {
    // GNU's Fding: noninteractive sessions write \a to stdout.  It
    // stays in stdio's block buffer, so a piped reader sees it merged
    // with the next stdout write — `print!' without flush reproduces
    // that ordering exactly.
    if i.noninteractive {
        print!("\x07");
    }
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
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs_f64(secs.min(3600.0));
    loop {
        timer_check(i)?;
        // GNU's wait also drains subprocess output (filters/buffers/
        // sentinels) — sit-for/sleep-for both pump.
        crate::lisp::process::poll_all(i)?;
        let rest = deadline.saturating_duration_since(std::time::Instant::now());
        if rest.is_zero() {
            break;
        }
        std::thread::sleep(rest.min(std::time::Duration::from_millis(20)));
    }
    timer_check(i)?;
    crate::lisp::process::poll_all(i)?;
    Ok(Value::Nil)
}

fn f_sleep_for(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let secs = match &args[0] {
        Value::Int(n) => *n as f64,
        Value::Float(f) => **f,
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
        Some(Value::Float(f)) => **f,
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
    Ok(Value::float(t as f64 / 1e12))
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
        // GNU's decode_time_zone accepts `current-time-zone' output —
        // a (OFFSET NAME) cons — and uses the car as the fixed offset.
        Some(Value::Cons(c)) => {
            let off = match &c.borrow().car {
                Value::Int(n) => *n as i64,
                _ => 0,
            };
            let mut tm = super::misc::gmt_tm(secs + off);
            tm.tm_gmtoff = off;
            tm
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
        while j < chars.len() && (chars[j].is_ascii_digit() || matches!(chars[j], '.' | ',' | ':'))
        {
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
    // GNU's kill_emacs runs `kill-emacs-hook' before exiting in every
    // case; a signaling hook function aborts the exit.
    call_hook(i, "kill-emacs-hook")?;
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
    // GNU's recursive_edit_1 specbinds command_loop_level and installs
    // an `exit' catch around a command loop; both unwind when it
    // returns.  There's no callable command loop here yet, so just
    // keep the depth/catch scoped like the specbind.
    i.recursion_depth += 1;
    let exit_sym = Value::Sym(i.intern("exit"));
    i.catch_tags.push(exit_sym);
    i.catch_tags.pop();
    i.recursion_depth -= 1;
    Ok(Value::Nil)
}
fn f_top_level(_i: &mut Interp, _args: Vec<Value>) -> EvalResult {
    Err(Flow::Throw(Value::Sym(sym::TOP_LEVEL), Value::Nil))
}
fn f_exit_recursive_edit(i: &mut Interp, _args: Vec<Value>) -> EvalResult {
    // GNU: `(throw 'exit nil)' when a command loop or minibuffer is
    // active, else user-error.
    if i.recursion_depth == 0 && i.minibuf_level <= 0 {
        let ue = i.intern("user-error");
        return Err(i.signal_data(ue, vec![Value::string("No recursive edit is in progress")]));
    }
    Err(Flow::Throw(Value::Sym(i.intern("exit")), Value::Nil))
}
fn f_abort_recursive_edit(i: &mut Interp, _args: Vec<Value>) -> EvalResult {
    // GNU: `(throw 'exit t)' — the `exit' catch maps t to `quit'.
    if i.recursion_depth == 0 && i.minibuf_level <= 0 {
        let ue = i.intern("user-error");
        return Err(i.signal_data(ue, vec![Value::string("No recursive edit is in progress")]));
    }
    Err(Flow::Throw(Value::Sym(i.intern("exit")), Value::t()))
}
fn f_recursion_depth(i: &mut Interp, _args: Vec<Value>) -> EvalResult {
    // GNU: command_loop_level + minibuf_level.
    Ok(Value::Int(
        (i.recursion_depth + i.minibuf_level.max(0) as usize) as i128,
    ))
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

/// Convert the alist-ish ENV argument used by `eval'/byte-compiled
/// closures into a LexEnv. Elements that are (VAR . VAL) pairs go in
/// the innermost frame; elements that are proper lists of such pairs
/// become enclosing frames.
fn parse_lexenv_spec(i: &mut Interp, env_v: &Value) -> crate::lisp::LexEnv {
    use crate::lisp::eval::LexFrame;
    use std::collections::HashMap;
    let mut inner: Option<std::rc::Rc<LexFrame>> = None;
    let mut parents: Vec<std::rc::Rc<LexFrame>> = Vec::new();
    let is_pair = |v: &Value| -> bool {
        matches!(v, Value::Cons(c) if matches!(c.borrow().car, Value::Sym(_))
            && !matches!(c.borrow().cdr, Value::Cons(_)))
    };
    let frame_of = |vars: Vec<(Value, Value)>, markers: Vec<SymId>| -> std::rc::Rc<LexFrame> {
        let mut m = HashMap::new();
        for (k, v) in vars {
            if let Value::Sym(id) = k {
                m.insert(id, v);
            }
        }
        std::rc::Rc::new(LexFrame {
            vars: std::cell::RefCell::new(m),
            declared: std::cell::RefCell::new(markers.into_iter().collect()),
            parent: None,
        })
    };
    let mut first: Vec<(Value, Value)> = Vec::new();
    let mut first_markers: Vec<SymId> = Vec::new();
    for elt in env_v.list_to_vec().unwrap_or_default() {
        if is_pair(&elt) {
            if let Value::Cons(c) = &elt {
                let b = c.borrow();
                first.push((b.car.clone(), b.cdr.clone()));
            }
        } else if let Value::Sym(id) = elt {
            // A bare symbol is a scoped `defvar' marker (GNU's `memq'
            // check at binding time), not a binding.
            first_markers.push(id);
        } else if let Value::Cons(_) = elt {
            // A nested list of pairs = an enclosing let frame.
            let vars: Vec<(Value, Value)> = elt
                .list_to_vec()
                .unwrap_or_default()
                .into_iter()
                .filter_map(|p| {
                    if let Value::Cons(c) = &p {
                        let b = c.borrow();
                        Some((b.car.clone(), b.cdr.clone()))
                    } else {
                        None
                    }
                })
                .collect();
            parents.push(frame_of(vars, Vec::new()));
        }
    }
    if !first.is_empty() || !first_markers.is_empty() || !parents.is_empty() {
        inner = Some(frame_of(first, first_markers));
    }
    // Chain: innermost first, then the enclosing frames in order.
    for p in parents.into_iter().rev() {
        let q = std::rc::Rc::new(LexFrame {
            vars: p.vars.clone(),
            declared: std::cell::RefCell::new(p.declared.borrow().clone()),
            parent: inner.take(),
        });
        inner = Some(q);
    }
    let _ = i;
    inner
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

fn f_byte_code_function_p(_i: &mut Interp, args: Vec<Value>) -> EvalResult {
    // `#[...]' byte-code literals are the only byte-code objects we
    // have; interpreted lambdas (bc_items = None) answer nil like GNU.
    Ok(Value::from_bool(matches!(&args[0], Value::Lambda(l) if l.bc_items.is_some())))
}
fn f_compiled_function_p(_i: &mut Interp, args: Vec<Value>) -> EvalResult {
    Ok(Value::from_bool(match &args[0] {
        Value::Subr(_) => true,
        Value::Lambda(l) => l.bc_items.is_some(),
        _ => false,
    }))
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
        let ma = i.intern_soft("mark-active").unwrap_or(0);
        (
            bb.mark,
            bb.locals.get(&ma).map(|v| v.truthy()).unwrap_or(false),
        )
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
            let ma = i.intern_soft("mark-active").unwrap_or(0);
            let mut bb = b.borrow_mut();
            if let Value::Marker(m) = &mk {
                bb.mark = Some(m.borrow().position.min(bb.text.len()));
            }
            bb.locals
                .insert(ma, if act.truthy() { Value::t() } else { Value::Nil });
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
    advice_push(i, sym, w, a[2].clone(), name)?;
    Ok(Value::Nil)
}

fn advice_retain(i: &mut Interp, key: SymId, sel: &Value) -> Result<(), Flow> {
    if let Some(pos) = i.advices.iter().position(|(s, _)| *s == key) {
        let mut list = std::mem::take(&mut i.advices[pos].1);
        list.retain(|(_, f, n)| !advice_entry_matches(i, f, n, sel));
        i.advices[pos].1 = list;
    }
    i.recompose_advice(key)
}

fn advice_push(i: &mut Interp, key: SymId, w: SymId, fun: Value, name: Value) -> Result<(), Flow> {
    // The first advice on KEY captures the current cell as the chain's
    // base (GNU's `advice--make' layers onto the existing definition).
    if !i.advice_bases.iter().any(|(s, _)| *s == key) {
        let base = i.symbol_function(key);
        i.advice_bases.push((key, base));
    }
    let entry = (w, fun.clone(), name.clone());
    let pos = match i.advices.iter().position(|(s, _)| *s == key) {
        Some(p) => p,
        None => {
            i.advices.push((key, Vec::new()));
            i.advices.len() - 1
        }
    };
    let mut list = std::mem::take(&mut i.advices[pos].1);
    // GNU's advice--add-function replaces an existing entry by :name
    // when PROPS supplies one; otherwise it matches on the function
    // itself.  Same-function-different-name pieces coexist.
    if !name.is_nil() {
        list.retain(|(_, _, n)| !super::equal_values(i, n, &name));
    } else {
        list.retain(|(_, f, _)| !super::equal_values(i, f, &fun));
    }
    list.push(entry);
    i.advices[pos].1 = list;
    i.recompose_advice(key)
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
    advice_retain(i, sym, &a[1])?;
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
            ));
        }
    };
    let name = advice_name(i, &arg(&a, 3));
    advice_push(i, key, w, a[2].clone(), name)?;
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
    advice_retain(i, key, &a[1])?;
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
    // GNU's advice-member-p resolves FUNCTION-DEF through
    // `advice--symbol-function' and walks the advice chain, returning
    // the matching layer (our trampoline stands in for the oclosure).
    let mut cur = match &a[1] {
        Value::Sym(s) => i.symbol_function(*s),
        v => v.clone(),
    };
    loop {
        match i.advice_link_entry(&cur) {
            Some((_, f, next, n)) => {
                if advice_entry_matches(i, &f, &n, &a[0]) {
                    return Ok(cur);
                }
                cur = next;
            }
            None => return Ok(Value::Nil),
        }
    }
}

fn f_advice_function_mapc(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    // GNU walks FUNCTION-DEF's `advice--p' chain — a bare symbol is not
    // an advice object, so it iterates zero times there too.
    let name_kw = i.intern("name");
    let mut cur = a[1].clone();
    while let Some((_, f, next, n)) = i.advice_link_entry(&cur) {
        let props = if n.is_nil() {
            Value::Nil
        } else {
            Value::list(vec![Value::cons(Value::Sym(name_kw), n)])
        };
        i.apply(&a[0], vec![f, props])?;
        cur = next;
    }
    Ok(Value::Nil)
}

/// `(cl--advice--link OBJ)` — when OBJ is an advice trampoline,
/// `(FUN NEXT HOW PROPS)' describing its layer; else nil.  The Lisp
/// `advice--car'/`advice--cdr'/`advice--how'/`advice--props' accessors
/// read off this list, mirroring GNU's oclosure slot accessors.
fn f_advice_link(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    match i.advice_link_entry(&a[0]) {
        Some((w, f, next, n)) => {
            let name_kw = i.intern("name");
            let props = if n.is_nil() {
                Value::Nil
            } else {
                Value::list(vec![Value::cons(Value::Sym(name_kw), n)])
            };
            Ok(Value::list(vec![f, next, w, props]))
        }
        None => Ok(Value::Nil),
    }
}

/// `(cl--advice--apply IDX ARGS...)` — apply advice wrapper IDX:
/// entry = (WHERE FUN NEXT).
fn f_advice_apply_link(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let idx = match &a[0] {
        Value::Int(n) => *n as usize,
        _ => return Err(i.wrong_type_mut("fixnump", &a[0])),
    };
    let (w, f, next, _name) = match i.advice_links.get(idx) {
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
        // GNU: (or OLDFUN ADVICE)
        ":after-until" => {
            let r = i.apply(&next, args.clone())?;
            Ok(if r.truthy() { r } else { i.apply(&f, args)? })
        }
        // GNU: (and OLDFUN ADVICE)
        ":after-while" => {
            let r = i.apply(&next, args.clone())?;
            if r.truthy() {
                i.apply(&f, args)
            } else {
                Ok(Value::Nil)
            }
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
