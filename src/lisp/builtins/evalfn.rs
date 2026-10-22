//! Evaluation-related subrs: eval, apply, funcall, signal, error, throw,
//! featurep, run-hooks, etc.

use super::{S, arg, want_list, want_sym};
use crate::lisp::Interp;
use crate::lisp::SymId;
use crate::lisp::error::{EvalResult, Flow};
use crate::lisp::obarray::sym;
use crate::lisp::value::{Marker, Subr, Value};
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
        "macroexpand--all-toplevel",
        1,
        2,
        f_macroexpand_all_toplevel,
        ""
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
        1,
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
        "Run BODY with condition handlers bound (see ‘handler-bind’)."
    ),
    S!(
        "access-file",
        2,
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
        3,
        3,
        f_funcall_with_delayed_message,
        "Call FUNCTION, show message."
    ),
    S!(
        "declare-function",
        raw,
        f_declare_function,
        "Declare external function (no-op)."
    ),
    // The prelude's `(defmacro declare ...)' (GNU subr.el, verbatim)
    // replaces this raw subr at init — a macro cell is required so
    // advising `declare' composes through `advice--normalize''s
    // `(macro . fn)' path instead of re-evaluating the specs.
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
    // GNU 31.1's nativecomp build makes byte-run.el's `with-no-warnings'
    // a real subr `(fn &rest BODY)': `apply' reaches it, `special-form-p'
    // is nil, args are evaluated and the last one's value returned.
    S!(
        "with-no-warnings",
        many 0,
        f_with_no_warnings,
        "Like `progn', but prevents compiler warnings in the body.\n\n(fn &rest BODY)"
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
    S!("sit-for", 1, 2, f_sit_for, "Wait SECONDS or until input."),
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
        2,
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
    S!("set-message-functions", 1, 1, f_set_message_functions, ""),
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
    S!("make-byte-code", many 4, f_make_byte_code, "Make byte-code object (stub lambda)."),
    S!(
        "subr-native-lambda-list",
        1,
        1,
        f_subr_native_lambda_list,
        "Subr arglist."
    ),
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
fn f_error_type(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    match &a[0] {
        Value::Cons(c) => Ok(c.borrow().car.clone()),
        Value::Nil => Ok(Value::Nil),
        other => Err(i.wrong_type_mut("listp", other)),
    }
}

fn f_eval(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    // (eval FORM &optional LEXICAL) — GNU binds
    // `internal-interpreter-environment' to LEXICAL when nil (dynamic)
    // or a cons (an actual env), else to `(t)' (fresh lexical env).
    let lex = arg(&args, 1);
    // GNU `Feval': nil LEXICAL means dynamic evaluation — the dynamic
    // `lexical-binding' value is NOT consulted (verified on 31.1:
    // `(eval '(let ((x 5)) (symbol-value 'x)))' returns 5 even with
    // lexical-binding=t).  Non-nil non-cons LEXICAL uses a fresh
    // lexical environment; a cons is an actual env.
    let lexenv = if matches!(lex, Value::Cons(_)) {
        crate::lisp::eval::lexenv_from_value(&lex)
    } else if lex.truthy() {
        crate::lisp::eval::lexenv_root()
    } else {
        None
    };
    i.explicit_eval_depth += 1;
    // GNU `Feval' does NOT bind the `lexical-binding' variable (only
    // `internal-interpreter-environment' is set): `(eval
    // 'lexical-binding t)' reads the ordinary dynamic value
    // (verified on 31.1 → nil).  The `lexical-binding' specbind GNU
    // installs lives solely around eval-driven macroexpansion
    // (`eval_macro_expand').
    if std::env::var("REMACS_TRACE_EVAL").is_ok() {
        let s = i.prin1_to_string(&args[0]);
        if s.contains("rect") {
            eprintln!(
                "[eval-rect] {} | depth={}",
                &s[..s.len().min(200)],
                i.lisp_stack.len()
            );
            for (f, _) in i.lisp_stack.iter().rev().take(30).rev() {
                eprintln!("   {}", i.princ_to_string(f));
            }
        }
    }
    let saved = std::mem::replace(&mut i.lexenv, lexenv);
    let r = i.with_eval_context(false, |i| i.eval(&args[0]));
    i.lexenv = saved;
    i.explicit_eval_depth -= 1;
    r
}

fn f_apply(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
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

fn f_funcall(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    if args.is_empty() {
        let s = Value::Sym(i.intern("funcall"));
        return Err(i.wrong_number_of_args(&s, 0));
    }
    let fun = args[0].clone();
    i.apply(&fun, args[1..].to_vec())
}

fn f_funcall_interactively(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    // GNU Ffuncall_interactively counts as an interactive call for
    // `called-interactively-p' (the evaluator sees a `funcall-interactively'
    // stack frame).
    i.interactive_call_depth += 1;
    let r = f_funcall(i, args);
    i.interactive_call_depth = i.interactive_call_depth.saturating_sub(1);
    r
}

fn f_function_raw(i: &mut Interp, _args: crate::lisp::Argv) -> EvalResult {
    f_special_form_via_apply(i, Vec::new().into())
}

/// Special forms are not funcallable (GNU signals `invalid-function').
fn f_special_form_via_apply(i: &mut Interp, _a: crate::lisp::Argv) -> EvalResult {
    Err(i.signal_data(sym::INVALID_FUNCTION, vec![Value::Nil]))
}

fn f_macroexpand(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    // GNU: (macroexpand FORM &optional ENVIRONMENT) consults only the
    // explicit ENVIRONMENT alist; the dynamic variable
    // `macroexpand-all-environment' is `macroexpand-all''s business.
    // GNU inspects ENVIRONMENT only for a list with a symbol head.
    // Atoms and non-symbol-headed lists return unchanged (#293).
    let env = args.get(1).cloned().unwrap_or(Value::Nil);
    if matches!(&args[0], Value::Cons(c) if i.sym_id(&c.borrow().car).is_some()) {
        match &env {
            Value::Nil | Value::Cons(_) => {}
            other => return Err(i.wrong_type_mut("listp", other)),
        }
    }
    i.macroexpand_env(&args[0], &env)
}

fn f_macroexpand_1(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    // GNU's single-step expander checks ENVIRONMENT only for conses.
    if matches!(&args[0], Value::Cons(_))
        && let Some(env) = args.get(1)
    {
        match env {
            Value::Nil | Value::Cons(_) => {}
            other => return Err(i.wrong_type_mut("listp", other)),
        }
    }
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
                return Ok(Value::list(vec![Value::Sym(sym::FUNCTION), form.clone()]));
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
                                // Sympos env keys unwrap to their bare
                                // symbol id (GNU's `assq' matches `eq').
                                if let (Some(eid), Some(id)) = (i.sym_id(&ek), i.sym_id(&car)) {
                                    if eid == id {
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
            if let Some(id) = i.sym_id(&car) {
                // GNU macroexp.el: `autoload-do-load' resolves an
                // autoload cell (macro-only); then aliases to macros
                // re-expanda, and `(macro . FN)' cells call FN.
                let def = i.symbol_function(id);
                let def = autoload_do_load(i, def, true)?;
                // (and (symbolp def) (macrop def)) → (cons def cdr):
                // let the outer macroexpand re-enter on the alias.
                if let Value::Sym(did) = def {
                    if did != sym::UNBOUND {
                        let f = i.symbol_function(did);
                        let is_mac = match &f {
                            Value::Lambda(l) => l.is_macro,
                            Value::Cons(cc) => i.sym_is(&cc.borrow().car, sym::MACRO),
                            _ => false,
                        };
                        if is_mac {
                            return Ok(Value::cons(Value::Sym(did), cdr));
                        }
                    }
                    return Ok(form.clone());
                }
                match &def {
                    Value::Cons(cc) if i.sym_is(&cc.borrow().car, sym::MACRO) => {
                        let fn_ = cc.borrow().cdr.clone();
                        let argv = cdr.list_to_vec().unwrap_or_default();
                        return i.apply(&fn_, argv);
                    }
                    Value::Lambda(l) if l.is_macro => {
                        return i.macro_expand_call(&def, &cdr);
                    }
                    _ => return Ok(form.clone()),
                }
            }
            Ok(form.clone())
        }
        _ => Ok(form.clone()),
    }
}

fn f_macroexpand_all(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    macroexpand_all_entry(i, args, true)
}

fn f_macroexpand_all_toplevel(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    macroexpand_all_entry(i, args, false)
}

fn macroexpand_all_entry(
    i: &mut Interp,
    args: crate::lisp::Argv,
    scope_dynvars: bool,
) -> EvalResult {
    // GNU: ENVIRONMENT is passed through the dynamic variable
    // `macroexpand-all-environment'; macros such as `rx'/`rx-let'
    // consult it for local definitions. Atoms do not inspect it (#293).
    let env = args.get(1).cloned().unwrap_or(Value::Nil);
    if matches!(&args[0], Value::Cons(_)) {
        match &env {
            Value::Nil | Value::Cons(_) => {}
            other => return Err(i.wrong_type_mut("listp", other)),
        }
    }
    let id = i.intern("macroexpand-all-environment");
    let mark = i.specbind_depth();
    let result = (|| {
        i.specbind(id, env)?;
        if scope_dynvars {
            let dynvars = i.intern("macroexp--dynvars");
            let old = if i.bound_p(dynvars) {
                i.symbol_value(dynvars)
            } else {
                Value::Nil
            };
            i.specbind(dynvars, old)?;
        }
        macroexpand_all(i, &args[0])
    })();
    i.unbind_to(mark)?;
    result
}

/// Whether FORM is a `(declare ...)' spec — a data position consumed
/// by `defun'/`lambda' declaration handling, not expanded.
fn is_declare_form(i: &Interp, form: &Value) -> bool {
    if let Value::Cons(c) = form {
        let b = c.borrow();
        if let Some(id) = i.sym_id(&b.car) {
            return i.symbol_name(id) == "declare";
        }
    }
    false
}

/// GNU `macroexp--expand-all' applies the head symbol's
/// `compiler-macro' property as (apply HANDLER FORM (cdr FORM)),
/// following `symbol-function' aliases like `cl-compiler-macroexpand'.
fn compiler_macro_handler(i: &mut Interp, form: &Value) -> Option<Value> {
    let car = match form {
        Value::Cons(c) => {
            let b = c.borrow();
            b.car.clone()
        }
        _ => return None,
    };
    // GNU handles these data/special-form positions before considering
    // a compiler-macro property on an ordinary call.
    if i.sym_id(&car).is_some_and(|id| {
        matches!(
            id,
            sym::QUOTE
                | sym::FUNCTION
                | sym::DEFVAR
                | sym::DEFCONST
                | sym::LET
                | sym::LET_STAR
                | sym::WHILE
                | sym::UNWIND_PROTECT
                | sym::SETQ
                | sym::COND
                | sym::CONDITION_CASE
        )
    }) {
        return None;
    }
    let mut func = crate::lisp::builtins::misc::unpos(&car);
    let cmacro = i.intern("compiler-macro");
    let mut guard = 0;
    while let Value::Sym(id) = func {
        let h = i.get_prop(id, cmacro);
        if !h.is_nil() {
            return Some(h);
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
    None
}

/// None means the handler returned FORM unchanged; Some(nil) is a valid
/// expansion.  Retain HANDLER for the retry after expanding arguments.
fn expand_compiler_macro(
    i: &mut Interp,
    handler: &Value,
    form: &Value,
) -> Result<Option<Value>, Flow> {
    let helper = i.intern("macroexp--compiler-macro");
    let expanded = if i.fbound_p(helper) {
        // The GNU helper honors inhibition/debug settings, warns on
        // optimization failures and preserves nonlocal exits.
        i.apply(&Value::Sym(helper), vec![handler.clone(), form.clone()])?
    } else {
        // Early loadup, before macroexp.el's wrapper is defined.
        if i.intern_soft("macroexp-inhibit-compiler-macros")
            .is_some_and(|id| i.bound_p(id) && i.symbol_value(id).truthy())
        {
            return Ok(None);
        }
        let rest = match form {
            Value::Cons(c) => c.borrow().cdr.clone(),
            _ => return Ok(None),
        };
        let mut argv = vec![form.clone()];
        argv.extend(want_list(i, &rest)?);
        match i.apply(handler, argv) {
            Ok(value) => value,
            Err(Flow::Signal(..)) => return Ok(None),
            Err(flow) => return Err(flow),
        }
    };
    Ok((!super::eq_values(&expanded, form)).then_some(expanded))
}

/// Whether V can be a `setq' target symbol — a symbol other than
/// t/nil or a keyword (GNU `macroexp--expand-all' fast-path check).
fn plain_setq_var(i: &Interp, v: &Value) -> bool {
    // `symbols-with-pos-enabled' objects are settable symbols: GNU's
    // `(symbolp var)' accepts them, so unwrap via `sym_id'.
    match i.sym_id(v) {
        Some(sid) if sid != sym::NIL && sid != sym::T => !i.symbol_name(sid).starts_with(':'),
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
    // Memoize on the input cons's identity: `pcase' and other big
    // macros duplicate shared subforms across their expansion, so
    // recursing on each copy re-expands the same cons exponentially.
    // Entries validate against `macro_gen' so definition/property
    // changes re-expand once.  Expansion depends on
    // `macroexpand-all-environment'.  Eager load expansion must visit
    // each occurrence even when source forms share conses: user macros
    // and compiler macros may have observable expansion-time effects.
    let env_nil = i.macroexp_call_depth == 0 && {
        let env_id = i.intern("macroexpand-all-environment");
        i.symbol_value(env_id).is_nil()
    };
    let cons_key = match form {
        Value::Cons(c) if env_nil => Some(std::rc::Rc::as_ptr(c) as usize),
        _ => None,
    };
    if let Some(k) = cons_key {
        let mut cache = i.macroexpand_all_cache.borrow_mut();
        if cache.0 != i.macro_gen {
            cache.0 = i.macro_gen;
            cache.1.clear();
        }
        if let Some(hit) = cache.1.get(&k) {
            return Ok(hit.1.clone());
        }
    }
    let out = macroexpand_all_inner(i, form)?;
    if let Some(k) = cons_key {
        i.macroexpand_all_cache
            .borrow_mut()
            .1
            .insert(k, (form.clone(), out.clone()));
    }
    Ok(out)
}

/// Look up SYM_ID in ENV's `(:cl-symbol-macros . BINDINGS)' entry.
/// BINDINGS are `(SYM EXP)' pairs (GNU `cl-symbol-macrolet'), where
/// `cl--sm-macroexpand-1' alpha-renaming also pushes `(VAR . NVAR)'
/// dotted pairs — both shapes handled by taking `car' when the value
/// is a cons.
fn sm_lookup(i: &mut Interp, env: &Value, sym_id: SymId) -> Option<Value> {
    let kw = i.intern(":cl-symbol-macros");
    let mut tail = env.clone();
    loop {
        match tail {
            Value::Cons(c) => {
                let (a, d) = {
                    let b = c.borrow();
                    (b.car.clone(), b.cdr.clone())
                };
                if let Value::Cons(e) = &a {
                    let (ek, ev) = {
                        let b = e.borrow();
                        (b.car.clone(), b.cdr.clone())
                    };
                    if i.sym_id(&ek) == Some(kw) {
                        // Found the :cl-symbol-macros entry; scan bindings.
                        let mut bt = ev;
                        loop {
                            match bt {
                                Value::Cons(bc) => {
                                    let (ba, bd) = {
                                        let b = bc.borrow();
                                        (b.car.clone(), b.cdr.clone())
                                    };
                                    if let Value::Cons(binding) = &ba {
                                        let (bs, bv) = {
                                            let b = binding.borrow();
                                            (b.car.clone(), b.cdr.clone())
                                        };
                                        if i.sym_id(&bs) == Some(sym_id) {
                                            return Some(match bv {
                                                Value::Cons(vc) => vc.borrow().car.clone(),
                                                other => other,
                                            });
                                        }
                                    }
                                    bt = bd;
                                }
                                _ => break,
                            }
                        }
                        return None;
                    }
                }
                tail = d;
            }
            _ => break,
        }
    }
    None
}

/// Remove BINDINGS whose car is any of `shadowed` from the env's
/// `:cl-symbol-macros' entry; used for let/lambda/condition-case
/// binder shadowing (GNU `cl--sm-macroexpand-1' alpha-renames;
/// removing the entry is equivalent for the shadowed scope).
fn sm_filter_env(i: &mut Interp, env: &Value, shadowed: &[SymId]) -> Value {
    if shadowed.is_empty() {
        return env.clone();
    }
    let kw = i.intern(":cl-symbol-macros");
    let mut out = Value::Nil;
    let mut tail_cell: Option<Value> = None;
    let mut cur = env.clone();
    loop {
        match cur {
            Value::Cons(c) => {
                let (a, d) = {
                    let b = c.borrow();
                    (b.car.clone(), b.cdr.clone())
                };
                let mut keep = a.clone();
                if let Value::Cons(e) = &a {
                    let (ek, ev) = {
                        let b = e.borrow();
                        (b.car.clone(), b.cdr.clone())
                    };
                    if i.sym_id(&ek) == Some(kw) {
                        // Rebuild binding list without shadowed names.
                        let mut newbinds = Vec::new();
                        let mut bt = ev;
                        loop {
                            match bt {
                                Value::Cons(bc) => {
                                    let (ba, bd) = {
                                        let b = bc.borrow();
                                        (b.car.clone(), b.cdr.clone())
                                    };
                                    let keep_b = match &ba {
                                        Value::Cons(binding) => {
                                            let bs = binding.borrow().car.clone();
                                            i.sym_id(&bs)
                                                .map(|sid| !shadowed.contains(&sid))
                                                .unwrap_or(true)
                                        }
                                        _ => true,
                                    };
                                    if keep_b {
                                        newbinds.push(ba);
                                    }
                                    bt = bd;
                                }
                                _ => break,
                            }
                        }
                        keep = Value::cons(Value::Sym(kw), Value::list(newbinds));
                    }
                }
                let cell = Value::cons(keep, Value::Nil);
                if let Some(Value::Cons(prev)) = &tail_cell {
                    prev.borrow_mut().cdr = cell.clone();
                } else {
                    out = cell.clone();
                }
                tail_cell = Some(cell);
                cur = d;
            }
            other => {
                if let Some(Value::Cons(prev)) = &tail_cell {
                    prev.borrow_mut().cdr = other;
                } else {
                    out = other;
                }
                break;
            }
        }
    }
    out
}

/// True when ENV carries a `:cl-symbol-macros' binding list.
fn sm_active(i: &mut Interp, env: &Value) -> bool {
    let kw = i.intern(":cl-symbol-macros");
    let mut tail = env.clone();
    while let Value::Cons(c) = tail {
        let (a, d) = {
            let b = c.borrow();
            (b.car.clone(), b.cdr.clone())
        };
        if let Value::Cons(e) = &a {
            if i.sym_id(&e.borrow().car) == Some(kw) {
                return true;
            }
        }
        tail = d;
    }
    false
}

/// Symbol ids bound by a `let'/`let*' varlist or a `lambda' arglist —
/// the names that shadow symbol macros inside the body.
fn binder_syms(i: &mut Interp, vars: &Value) -> Vec<SymId> {
    let mut out = Vec::new();
    let mut cur = vars.clone();
    while let Value::Cons(c) = cur {
        let (a, d) = {
            let b = c.borrow();
            (b.car.clone(), b.cdr.clone())
        };
        let v = match &a {
            Value::Cons(bc) => bc.borrow().car.clone(),
            other => other.clone(),
        };
        if let Some(id) = i.sym_id(&v) {
            let n = i.symbol_name(id);
            if !n.starts_with('&') {
                out.push(id);
            }
        }
        cur = d;
    }
    out
}

/// Expand FORM with `macroexpand-all-environment' rebound to a
/// filtered copy of ENV when symbol macros are active — mirroring
/// GNU's alpha-renaming scope shrink for shadowed binders.  Without
/// `:cl-symbol-macros' in ENV this is a plain `macroexpand_all'.
fn expand_body_filtered(
    i: &mut Interp,
    env_id: SymId,
    env: &Value,
    sm: bool,
    shadowed: &[SymId],
    form: &Value,
) -> EvalResult {
    if sm && !shadowed.is_empty() {
        let e2 = sm_filter_env(i, env, shadowed);
        i.specbind(env_id, e2)?;
        let r = macroexpand_all(i, form);
        i.unbind(1)?;
        r
    } else {
        macroexpand_all(i, form)
    }
}

fn macroexpand_all_inner(i: &mut Interp, form: &Value) -> EvalResult {
    // GNU `macroexp--expand-all' passes `macroexpand-all-environment'
    // (bound by `macroexpand-all') to `macroexpand' explicitly.
    let env_id = i.intern("macroexpand-all-environment");
    let env = i.symbol_value(env_id);
    // `cl-symbol-macrolet' works by advising `macroexpand'/
    // `macroexpand-1' (`cl--sm-macroexpand{,-1}') during its own
    // expansion: going through the Lisp-level function cell picks up
    // that advice so symbol macros substitute, `setq' rewrites to
    // `setf', and let/lambda bindings alpha-rename.  When nothing is
    // advised the cell is still the subr — call the Rust fast path.
    let mac_sym = i.intern("macroexpand");
    let cell = i.symbol_function(mac_sym);
    // `expand_unadvised_depth` unwraps advice during dumped-code
    // expansion — GNU compiled these sites before advice existed, so
    // mirror that by using the base cell for the dispatch test too.
    let cell = if i.expand_unadvised_depth > 0 {
        i.unadvised_base(&cell)
    } else {
        cell
    };
    let expanded = match cell {
        Value::Subr(_) => i.macroexpand_env(form, &env)?,
        _ => i.apply(&Value::Sym(mac_sym), vec![form.clone(), env.clone()])?,
    };
    let sm = sm_active(i, &env);
    // GNU `cl--sm-macroexpand-1': a symbol bound by `cl-symbol-macrolet'
    // substitutes to its expansion form.
    if sm {
        if let Some(sid) = i.sym_id(&expanded) {
            if let Some(subst) = sm_lookup(i, &env, sid) {
                return macroexpand_all(i, &subst);
            }
        }
    }
    // `macroexpand'/`macroexpand-1'.
    let compiler_macro = compiler_macro_handler(i, &expanded);
    if let Some(handler) = &compiler_macro {
        if let Some(newf) = expand_compiler_macro(i, handler, &expanded)? {
            return macroexpand_all(i, &newf);
        }
    }
    let mark = i.specbind_depth();
    let scoped = match &expanded {
        Value::Cons(c) => i.sym_id(&c.borrow().car).is_some_and(|id| {
            matches!(
                id,
                sym::LET | sym::LET_STAR | sym::LAMBDA | sym::FUNCTION | sym::DEFUN | sym::DEFMACRO
            ) || i.symbol_name(id) == "closure"
        }),
        _ => false,
    };
    if scoped {
        let id = i.intern("macroexp--dynvars");
        let old = if i.bound_p(id) {
            i.symbol_value(id)
        } else {
            Value::Nil
        };
        i.specbind(id, old)?;
    }
    let result = macroexpand_all_expanded(i, expanded, env_id, env, sm, compiler_macro);
    i.unbind_to(mark)?;
    result
}

fn macroexpand_all_expanded(
    i: &mut Interp,
    expanded: Value,
    env_id: SymId,
    env: Value,
    sm: bool,
    compiler_macro: Option<Value>,
) -> EvalResult {
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
            // through unexpanded rather than failing.  (Plain
            // `list_to_vec' — `want_list' builds a `listp' signal that
            // is only discarded here, which also pollutes the
            // REMACS_BT_ERR log.)
            let items = match expanded.list_to_vec() {
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
                        let bound = binder_syms(i, &items[2]);
                        let mut body = items[3..].iter();
                        for it in &mut body {
                            if matches!(it, Value::Str(_)) || is_declare_form(i, it) {
                                out.push(it.clone());
                            } else {
                                out.push(expand_body_filtered(i, env_id, &env, sm, &bound, it)?);
                                break;
                            }
                        }
                        for it in body {
                            out.push(expand_body_filtered(i, env_id, &env, sm, &bound, it)?);
                        }
                        return Ok(Value::list(out));
                    }
                    sym::DEFVAR | sym::DEFCONST
                        if items.len() >= 2
                            && items[1].truthy()
                            && i.sym_id(&items[1]).is_some() =>
                    {
                        let dynvars = i.intern("macroexp--dynvars");
                        let old = if i.bound_p(dynvars) {
                            i.symbol_value(dynvars)
                        } else {
                            Value::Nil
                        };
                        i.set_symbol(dynvars, Value::cons(items[1].clone(), old))?;
                        let mut out = items[..2].to_vec();
                        for item in &items[2..] {
                            out.push(macroexpand_all(i, item)?);
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
                        let bound = binder_syms(i, &items[2]);
                        let mut body = items[3..].iter();
                        for it in &mut body {
                            if matches!(it, Value::Str(_)) || is_declare_form(i, it) {
                                out.push(it.clone());
                            } else {
                                out.push(expand_body_filtered(i, env_id, &env, sm, &bound, it)?);
                                break;
                            }
                        }
                        for it in body {
                            out.push(expand_body_filtered(i, env_id, &env, sm, &bound, it)?);
                        }
                        return Ok(Value::list(out));
                    }
                    sym::LAMBDA if items.len() >= 2 => {
                        let mut out = items[..2].to_vec();
                        // A bare lambda's declarations are stray macro
                        // forms, unlike defun/defmacro metadata above.
                        let bound = binder_syms(i, &items[1]);
                        let mut body = items[2..].iter();
                        for it in &mut body {
                            if matches!(it, Value::Str(_)) {
                                out.push(it.clone());
                            } else {
                                out.push(expand_body_filtered(i, env_id, &env, sm, &bound, it)?);
                                break;
                            }
                        }
                        for it in body {
                            out.push(expand_body_filtered(i, env_id, &env, sm, &bound, it)?);
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
                                let bound = binder_syms(i, &litems[head - 1]);
                                let mut lbody = litems[head..].iter();
                                for it in &mut lbody {
                                    if matches!(it, Value::Str(_))
                                        || (is_clo && is_declare_form(i, it))
                                    {
                                        lout.push(it.clone());
                                    } else {
                                        lout.push(expand_body_filtered(
                                            i, env_id, &env, sm, &bound, it,
                                        )?);
                                        break;
                                    }
                                }
                                for it in lbody {
                                    lout.push(expand_body_filtered(
                                        i, env_id, &env, sm, &bound, it,
                                    )?);
                                }
                                return Ok(Value::list(vec![items[0].clone(), Value::list(lout)]));
                            }
                        }
                        return Ok(expanded);
                    }
                    sym::LET | sym::LET_STAR | sym::AND_LET_STAR if items.len() >= 2 => {
                        // GNU `cl--sm-macroexpand-1' (let/let* arm):
                        // bound names shadow symbol macros inside their
                        // scope.  `let' inits see the outer env; `let*'
                        // inits see the env filtered by the bindings
                        // already walked; the body is filtered by all.
                        let bound = binder_syms(i, &items[1]);
                        let is_letstar = i.sym_is(&items[0], sym::LET_STAR)
                            || i.sym_is(&items[0], sym::AND_LET_STAR);
                        let body_env = sm_filter_env(i, &env, &bound);
                        let mut out = vec![items[0].clone()];
                        match &items[1] {
                            Value::Cons(_) => {
                                let mut newvarlist = Value::Nil;
                                let mut tail_cell: Option<Value> = None;
                                let mut cur = items[1].clone();
                                // Progressive shadow env for let* inits.
                                let mut seen: Vec<SymId> = Vec::new();
                                loop {
                                    match cur {
                                        Value::Cons(c) => {
                                            let (b, next) = {
                                                let bb = c.borrow();
                                                (bb.car.clone(), bb.cdr.clone())
                                            };
                                            // Env governing this binding's
                                            // init value.
                                            let init_env = if !sm {
                                                None
                                            } else if is_letstar {
                                                Some(sm_filter_env(i, &env, &seen))
                                            } else {
                                                // `let' inits expand in
                                                // the OUTER env (parallel
                                                // bindings, GNU sm arm).
                                                Some(env.clone())
                                            };
                                            if is_letstar && sm {
                                                let bv = match &b {
                                                    Value::Cons(bc) => bc.borrow().car.clone(),
                                                    other => other.clone(),
                                                };
                                                if let Some(sid) = i.sym_id(&bv) {
                                                    if !i.symbol_name(sid).starts_with('&') {
                                                        seen.push(sid);
                                                    }
                                                }
                                            }
                                            let nb = {
                                                let keep_head = matches!(
                                                    &b,
                                                    Value::Cons(bc)
                                                        if i.sym_id(&bc.borrow().car).is_some()
                                                );
                                                // For non-letstar plain-let,
                                                // all bound names shadow.
                                                let init_env = init_env.clone();
                                                let expand_init =
                                                    |i: &mut Interp, p: &Value| -> EvalResult {
                                                        if let Some(e2) = &init_env {
                                                            i.specbind(env_id, e2.clone())?;
                                                            let r = macroexpand_all(i, p);
                                                            i.unbind(1)?;
                                                            r
                                                        } else {
                                                            macroexpand_all(i, p)
                                                        }
                                                    };
                                                if keep_head {
                                                    let parts =
                                                        want_list(i, &b).unwrap_or_default();
                                                    if parts.is_empty() {
                                                        b.clone()
                                                    } else {
                                                        let mut nb = vec![parts[0].clone()];
                                                        for p in &parts[1..] {
                                                            nb.push(expand_init(i, p)?);
                                                        }
                                                        Value::list(nb)
                                                    }
                                                } else {
                                                    expand_init(i, &b)?
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
                        // Body under the fully filtered env.
                        if sm {
                            i.specbind(env_id, body_env)?;
                            let mut body_out = Vec::new();
                            for it in &items[2..] {
                                match macroexpand_all(i, it) {
                                    Ok(e) => body_out.push(e),
                                    Err(e) => {
                                        i.unbind(1)?;
                                        return Err(e);
                                    }
                                }
                            }
                            i.unbind(1)?;
                            out.extend(body_out);
                        } else {
                            for it in &items[2..] {
                                out.push(macroexpand_all(i, it)?);
                            }
                        }
                        return Ok(Value::list(out));
                    }
                    sym::SETQ if sm => {
                        // GNU `cl--sm-macroexpand-1': `(setq ...)' with a
                        // symbol-macro'd variable becomes `(setf ...)'
                        // (the symbol stays the place; `setf' expansion
                        // of a non-symbol place handles the rest).
                        let args = &items[1..];
                        if args.len() % 2 == 0 && !args.is_empty() {
                            let mut any_sm = false;
                            for pair in args.chunks(2) {
                                if let Some(vid) = i.sym_id(&pair[0]) {
                                    if sm_lookup(i, &env, vid).is_some() {
                                        any_sm = true;
                                        break;
                                    }
                                }
                            }
                            if any_sm {
                                // Emit `(setf SUBST EXPR)' pairs directly —
                                // emitting `(setf SYM ...)' would bounce
                                // setf→setq→setf forever.
                                let setf_sym = Value::Sym(i.intern("setf"));
                                let mut assigns = Vec::new();
                                for pair in args.chunks(2) {
                                    let place = i
                                        .sym_id(&pair[0])
                                        .and_then(|vid| sm_lookup(i, &env, vid))
                                        .unwrap_or_else(|| pair[0].clone());
                                    let expr = macroexpand_all(i, &pair[1])?;
                                    assigns.push(Value::list(vec![setf_sym.clone(), place, expr]));
                                }
                                let out = if assigns.len() == 1 {
                                    assigns.pop().unwrap()
                                } else {
                                    Value::cons(Value::Sym(sym::PROGN), Value::list(assigns))
                                };
                                return macroexpand_all(i, &out);
                            }
                        }
                        // Fall through to the normal `setq' path when no
                        // variable is symbol-macro'd.
                        let args = &items[1..];
                        let nargs = args.len();
                        if nargs == 2 && plain_setq_var(i, &args[0]) {
                            let expr = macroexpand_all(i, &args[1])?;
                            return Ok(Value::list(vec![items[0].clone(), args[0].clone(), expr]));
                        }
                        let mut assigns = Vec::new();
                        for pair in args.chunks(2) {
                            let var = &pair[0];
                            let expr = macroexpand_all(i, &pair[1])?;
                            assigns.push(Value::list(vec![items[0].clone(), var.clone(), expr]));
                        }
                        return Ok(Value::cons(Value::Sym(sym::PROGN), Value::list(assigns)));
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
                            return Ok(Value::list(vec![items[0].clone(), args[0].clone(), expr]));
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
                                    Value::list(vec![items[0].clone(), Value::Int(nargs as i128)]),
                                ]),
                            ]));
                        }
                        let mut assigns = Vec::new();
                        for pair in args.chunks(2) {
                            let var = &pair[0];
                            let expr = macroexpand_all(i, &pair[1])?;
                            let assignment = if plain_setq_var(i, var) {
                                Value::list(vec![items[0].clone(), var.clone(), expr])
                            } else if i
                                .sym_id(var)
                                .map(|sid| i.symbol_name(sid).starts_with(':'))
                                .unwrap_or(false)
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
                                            Value::list(vec![Value::Sym(sym::QUOTE), var.clone()]),
                                        ]),
                                    ]),
                                ])
                            } else {
                                // `(signal 'setting-constant (list 'VAR))'
                                // for t/nil, else a wrong-type-argument.
                                let tag = match i.sym_id(var) {
                                    Some(_) => sym::SETTING_CONSTANT,
                                    _ => sym::WRONG_TYPE_ARGUMENT,
                                };
                                let data = match i.sym_id(var) {
                                    Some(_) => Value::list(vec![
                                        Value::Sym(i.intern("list")),
                                        Value::list(vec![Value::Sym(sym::QUOTE), var.clone()]),
                                    ]),
                                    _ => Value::list(vec![
                                        Value::Sym(i.intern("list")),
                                        Value::list(vec![
                                            Value::Sym(sym::QUOTE),
                                            Value::Sym(i.intern("symbolp")),
                                        ]),
                                        Value::list(vec![Value::Sym(sym::QUOTE), var.clone()]),
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
                                    let is_sf = crate::lisp::special::special_form(fsid).is_some();
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
                        // GNU `cl--sm-macroexpand-1': the error var
                        // shadows a same-named symbol macro in the
                        // protected form AND the handler bodies.
                        let bound: Vec<SymId> = i.sym_id(&items[1]).into_iter().collect();
                        let mut out = vec![
                            items[0].clone(),
                            items[1].clone(),
                            expand_body_filtered(i, env_id, &env, sm, &bound, &items[2])?,
                        ];
                        for h in &items[3..] {
                            let is_cons = matches!(h, Value::Cons(_));
                            if is_cons {
                                let parts = want_list(i, h).unwrap_or_default();
                                if !parts.is_empty() {
                                    let mut nh = vec![parts[0].clone()];
                                    for p in &parts[1..] {
                                        nh.push(expand_body_filtered(
                                            i, env_id, &env, sm, &bound, p,
                                        )?);
                                    }
                                    out.push(Value::list(nh));
                                    continue;
                                }
                            }
                            out.push(h.clone());
                        }
                        return Ok(Value::list(out));
                    }
                    sym::COND => {
                        // GNU `macroexp--expand-all': `(cond CLAUSES...)'
                        // runs `macroexp--all-clauses' (skip 0) — every
                        // element of every clause, test included, gets
                        // expanded.  Without this arm the generic path
                        // descends only into clause bodies, leaving a
                        // `cl-macrolet'-bound call in test position
                        // unexpanded (comp-cstr's `with-comp-cstr-
                        // accessors' inside `cond' broke on this).
                        let mut out = vec![items[0].clone()];
                        for clause in &items[1..] {
                            if let Value::Cons(_) = clause {
                                let parts = clause.list_to_vec().unwrap_or_default();
                                let mut nc = Vec::with_capacity(parts.len());
                                for p in &parts {
                                    nc.push(macroexpand_all(i, p)?);
                                }
                                out.push(Value::list(nc));
                            } else {
                                out.push(clause.clone());
                            }
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
            if i.sym_id(&items[0]).is_none() {
                // GNU `macroexp--expand-all' falls through to `(_ form)'
                // for a non-symbol, non-lambda head: the whole form —
                // arguments included — passes through unexpanded.
                return Ok(expanded);
            }
            let mut out = Vec::with_capacity(items.len());
            // The head position is a function/macro name, not a variable:
            // symbol macros never substitute it (GNU `cl--sm-subst'
            // parity — a bound `car' in `(car x)' stays `car').
            out.push(items[0].clone());
            for it in &items[1..] {
                out.push(macroexpand_all(i, it)?);
            }
            let result = Value::list(out);
            if let Some(handler) = &compiler_macro {
                if super::equal_values(i, &expanded, &result)? {
                    return Ok(expanded);
                }
                if let Some(newf) = expand_compiler_macro(i, handler, &result)? {
                    return macroexpand_all(i, &newf);
                }
            }
            Ok(result)
        }
        _ => Ok(expanded),
    }
}

pub(crate) fn f_signal(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    // GNU Fsignal: with DATA nil, a cons ERROR-SYMBOL unpacks to
    // (car as the symbol, cdr as the data) — `(signal err)' inside a
    // condition-case handler passes the caught (SYM . DATA) list this
    // way (e.g. `files--ensure-directory').  A non-nil DATA makes the
    // cons itself the checked symbol.  A non-symbol condition name
    // signals `wrong-type-argument symbolp NAME'; a symbol with no
    // `error-conditions' property signals `error' ("Invalid error
    // symbol" NAME).
    let (symv, data) = match (&args[0], &arg(&args, 1)) {
        (Value::Cons(c), Value::Nil) => {
            let cc = c.borrow();
            (cc.car.clone(), cc.cdr.clone())
        }
        (v, d) => (v.clone(), d.clone()),
    };
    match i.sym_id(&symv) {
        Some(sid) => {
            let ec = i.intern("error-conditions");
            if i.get_prop(sid, ec).is_nil() {
                return Err(Flow::Signal(
                    Value::Sym(sym::ERROR),
                    Value::list(vec![Value::string("Invalid error symbol"), symv.clone()]),
                    false,
                ));
            }
        }
        None => return Err(i.wrong_type_mut("symbolp", &symv)),
    }
    Err(Flow::Signal(symv, data, false))
}

fn f_error(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    // GNU subr.el: (signal 'error (list (apply #'format-message ARGS))) —
    // full `format' semantics (flags/width/precision, format errors
    // propagate, non-string formats signal stringp).
    let msg = super::strfn::f_format_message(i, args)?;
    Err(Flow::Signal(
        Value::Sym(sym::ERROR),
        Value::list(vec![msg]),
        false,
    ))
}

fn f_user_error(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    // GNU subr.el: (signal 'user-error (list (apply #'format-message
    // FORMAT ARGS))).
    let msg = super::strfn::f_format_message(i, args)?;
    Err(Flow::Signal(
        Value::Sym(sym::USER_ERROR),
        Value::list(vec![msg]),
        false,
    ))
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum QuoteStyle {
    Grave,
    Straight,
    Curve,
}

/// GNU doc.c `text_quoting_style': `grave'/`straight' as named; a nil
/// `text-quoting-style' means grave when `internal--text-quoting-flag'
/// is nil (GNU main() seeds it from `using_utf8', so a non-UTF-8 batch
/// locale gets ASCII quotes) or when `standard-display-table' shows ‘
/// as `; every other value is curve.
pub(crate) fn text_quoting_style(i: &Interp) -> QuoteStyle {
    let var = |name: &str| {
        i.intern_soft(name)
            .map(|id| i.symbol_value(id))
            .unwrap_or(Value::Nil)
    };
    match var("text-quoting-style") {
        Value::Nil => {
            if !var("internal--text-quoting-flag").truthy() {
                return QuoteStyle::Grave;
            }
            let dt = var("standard-display-table");
            let graves = super::misc::is_char_table(i, &dt)
                && match super::misc::char_table_ref(i, &dt, 0x2018) {
                    Value::Vec(v) => {
                        let v = v.borrow();
                        v.len() == 1 && matches!(v[0], Value::Int(0x60))
                    }
                    _ => false,
                };
            if graves {
                QuoteStyle::Grave
            } else {
                QuoteStyle::Curve
            }
        }
        Value::Sym(s) if i.symbol_name(s) == "grave" => QuoteStyle::Grave,
        Value::Sym(s) if i.symbol_name(s) == "straight" => QuoteStyle::Straight,
        _ => QuoteStyle::Curve,
    }
}

/// GNU's `format-message' quote convention (`text_quoting_style'):
/// grave leaves `` `' `` and `'` verbatim, straight maps both to ASCII
/// `'', curve turns them into U+2018/U+2019.  Only the format string
/// is translated, never the substituted arguments.
pub(crate) fn translate_message_quotes(i: &Interp, fmt: &str) -> String {
    let mut out = String::with_capacity(fmt.len());
    match text_quoting_style(i) {
        QuoteStyle::Grave => out.push_str(fmt),
        QuoteStyle::Straight => {
            for c in fmt.chars() {
                out.push(if c == '`' { '\'' } else { c });
            }
        }
        QuoteStyle::Curve => {
            for c in fmt.chars() {
                match c {
                    '`' => out.push('\u{2018}'),
                    '\'' => out.push('\u{2019}'),
                    _ => out.push(c),
                }
            }
        }
    }
    out
}

fn f_throw(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
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

fn f_condition_case_raw(i: &mut Interp, _args: crate::lisp::Argv) -> EvalResult {
    f_special_form_via_apply(i, Vec::new().into())
}

/// `(ignore-errors BODY...)` and `(with-demoted-errors BODY...)`.
fn f_ignore_errors_raw(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    let body = args.iter().next().cloned().unwrap_or(Value::Nil);
    let err = Value::Sym(i.intern("error"));
    ignore_body_forms(i, &err, &body)
}

/// `(ignore-error CONDITION BODY...)` — GNU's two-arg form: the first
/// raw arg element is the error-condition spec, the rest is body.
fn f_ignore_error_raw(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    let all = args.iter().next().cloned().unwrap_or(Value::Nil);
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

fn f_with_demoted_errors(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    f_ignore_errors_raw(i, args)
}

/// GNU `require'/`featurep' test: membership in the Lisp-visible
/// `features' variable — which dynamic binds (e.g. Gnus's
/// `(dlet ((features (cons 'gnus-group features))) ...)' trick to
/// break require cycles) can shadow — or in the Rust mirror kept by
/// `provide'.
/// `featurep' on the dynamic `features' list — GNU consults the plain
/// variable, so a `dlet' binding (the Gnus cycle-breaker) counts and a
/// `delq' removal hides the feature.  `i.features' is boot bookkeeping
/// only, never the authority here.
fn feature_present(i: &mut Interp, id: SymId) -> bool {
    let fid = i.intern("features");
    match i.symbol_value(fid) {
        Value::Cons(_) | Value::Nil => i
            .symbol_value(fid)
            .list_to_vec()
            .map(|items| items.iter().any(|v| i.sym_id(v) == Some(id)))
            .unwrap_or(false),
        _ => false,
    }
}

/// GNU's LOADHIST_ATTACH: push `elt' onto `current-load-list' when a
/// load is in progress (and the list is still list-shaped).
pub(crate) fn loadhist_attach(i: &mut Interp, entry: Value) {
    let cll = i.intern("current-load-list");
    let cur = i.symbol_value(cll);
    if matches!(cur, Value::Sym(s) if s == crate::lisp::sym::UNBOUND) {
        return;
    }
    // GNU conses onto the raw value — a non-list `t' tail produces the
    // same dotted chain Fcons would.
    i.obarray.symbol_mut(cll).value = Value::cons(entry, cur);
}

/// Find the first `equal' match without swallowing comparison errors.
fn equal_position(i: &Interp, items: &[Value], needle: &Value) -> Result<Option<usize>, Flow> {
    for (pos, item) in items.iter().enumerate() {
        if super::equal_values(i, item, needle)? {
            return Ok(Some(pos));
        }
    }
    Ok(None)
}

fn f_featurep(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    let id = want_sym(i, &args[0])?;
    // GNU Ffeaturep: SUBFEATURE non-nil looks it up in the feature's
    // `subfeatures' plist entry instead of the features list.
    let sub = arg(&args, 1);
    if !sub.is_nil() {
        let prop = i.intern("subfeatures");
        let subs = i.get_prop(id, prop);
        let items = subs.list_to_vec().unwrap_or_default();
        let found = equal_position(i, &items, &sub)?.is_some();
        return Ok(Value::from_bool(found));
    }
    Ok(Value::from_bool(feature_present(i, id)))
}

fn f_provide(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    let id = want_sym(i, &args[0])?;
    // GNU Fprovide: CHECK_LIST on SUBFEATURES, then record it on the
    // feature's `subfeatures' plist.
    let sub = arg(&args, 1);
    if !sub.is_nil() && sub.list_to_vec().is_err() {
        return Err(i.wrong_type_mut("listp", &sub));
    }
    if !sub.is_nil() {
        let prop = i.intern("subfeatures");
        i.put_prop(id, prop, sub);
    }
    // `remacs--provided': `features' is seeded at boot with GNU's dump
    // state, which includes libraries remacs never actually loads
    // (`cl-preloaded' & friends).  `eval-after-load' uses this mark to
    // tell "feature really provided" (GNU evaluates immediately) from
    // a seed-only entry (defer — the deferred forms may need machinery
    // the never-loaded file would have defined).
    let prov_mark = i.intern("remacs--provided");
    i.put_prop(id, prov_mark, Value::t());
    // Symbol-keyed `after-load-alist' entries run their delayed
    // wrappers now (they defer to `after-load-functions' when a file
    // load is in progress, per GNU's eval-after-load indirection).
    let alist_sym = i.intern("after-load-alist");
    let entries = i.symbol_value(alist_sym).list_to_vec().unwrap_or_default();
    let feat_v = i.sym(id);
    for entry in entries {
        let mut parts = entry.list_to_vec().unwrap_or_default();
        if parts.is_empty() {
            continue;
        }
        let key = parts.remove(0);
        if super::eq_values(&key, &feat_v) {
            for f in parts {
                i.apply(&f, vec![])?;
            }
        }
    }
    // GNU records `(provide . FEATURE)' on `current-load-list' at
    // provide time (catches nested provides too).
    let provide_id = i.intern("provide");
    loadhist_attach(i, Value::cons(i.sym(provide_id), feat_v.clone()));
    // `(push feature features)' on the current binding, deduped.
    if !feature_present(i, id) {
        let fid = i.intern("features");
        let cur = i.symbol_value(fid);
        let mut items = cur.list_to_vec().unwrap_or_default();
        items.insert(0, feat_v.clone());
        i.set_symbol(fid, Value::list(items))?;
    }
    if !i.features.contains(&id) {
        i.features.insert(0, id);
    }
    Ok(args[0].clone())
}

fn f_require(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    let id = want_sym(i, &args[0])?;
    // GNU fns.c Frequire: when `load_in_progress' (or a `load' stream is
    // mid-read — current-load-list's last element is a file-name string),
    // attach `(require . FEATURE)' BEFORE the `memq feature features'
    // early-exit, deduplicated by `member' — so a `(require 'seq)' in a
    // loaded file lands in load-history even though seq is dumped.
    let require_id = i.intern("require");
    let cll = i.intern("current-load-list");
    let tem = Value::cons(i.sym(require_id), i.sym(id));
    let cll_val = i.symbol_value(cll);
    let lip = i.intern("load-in-progress");
    let from_file = i.symbol_value(lip).truthy()
        || cll_val
            .list_to_vec()
            .ok()
            .and_then(|v| v.last().cloned())
            .map(|last| matches!(last, Value::Str(_)))
            .unwrap_or(false);
    // Bundled sources stand in for GNU's precompiled libraries. Their
    // eager macro expansion needs compiler helpers, but those requires
    // are not runtime dependencies of the resulting library. User
    // source loads still record expansion-time requires like GNU.
    if from_file && !(i.loading_compiled_library && i.macroexp_call_depth > 0) {
        let items = cll_val.list_to_vec().unwrap_or_default();
        let dup = equal_position(i, &items, &tem)?.is_some();
        if !dup {
            loadhist_attach(i, tem);
        }
    }
    // GNU `require' is `(unless (memq feature features) (load ...))':
    // the check consults the Lisp `features' variable, so a dynamic
    // binding (the Gnus `dlet' cycle-breaker) counts as provided.
    if feature_present(i, id) {
        return Ok(args[0].clone());
    }
    // Try to load feature file from load-path.
    let name = match args.get(1) {
        Some(Value::Str(s)) => s.borrow().clone(),
        _ => i.symbol_name(id),
    };
    let noerror = args.get(2).map(|v| v.truthy()).unwrap_or(false);
    // GNU Frequire loads with NOMESSAGE = t: no `Loading FILE...' echo.
    match crate::lisp::load::load_library_opts_msg(i, &name, false, false, true, true) {
        Ok(crate::lisp::load::LoadOutcome::Loaded(found)) => {
            // GNU's `require': after loading FILE, FEATURE must be
            // provided — a file that loads without `(provide FEATURE)'
            // signals `error' with the RESOLVED file name ("Loading
            // file FILE failed to provide feature ‘FEATURE’"), and
            // NOERROR does not suppress it.
            if !feature_present(i, id) {
                return Err(i.signal_data(
                    sym::ERROR,
                    vec![Value::string(format!(
                        "Loading file {} failed to provide feature ‘{}’",
                        found,
                        i.symbol_name(id)
                    ))],
                ));
            }
            Ok(args[0].clone())
        }
        Ok(crate::lisp::load::LoadOutcome::Missing) if noerror => Ok(Value::Nil),
        Ok(crate::lisp::load::LoadOutcome::Dir(_)) if noerror => Ok(Value::Nil),
        Ok(crate::lisp::load::LoadOutcome::Missing) => {
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
        Ok(crate::lisp::load::LoadOutcome::Dir(d)) => Err(i.signal_data(
            crate::lisp::sym::FILE_ERROR,
            vec![
                Value::string("Cannot open load file"),
                Value::string("Is a directory"),
                Value::string(d),
            ],
        )),
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
    // GNU `find_symbol_value' walks the variable-alias chain — the
    // hook's binding lives on the BASE symbol; the alias's own cell
    // is a stale pre-alias remnant (`defvaralias' transfers the
    // value but leaves the old cell in place).
    let id = match i.sym_id(hook) {
        Some(s) => i.var_alias_target(s),
        None => return Vec::new(),
    };
    // A local `t' sentinel runs the default hook at that position;
    // local functions after the sentinel still run afterwards.
    let local = i.buffers.get(i.current_buffer).and_then(|b| {
        b.try_borrow()
            .ok()
            .and_then(|bb| bb.locals.get(&id).cloned())
    });
    match local {
        Some(v) => {
            let mut fns = Vec::new();
            for function in hook_list(&v) {
                if i.sym_is(&function, sym::T) {
                    fns.extend(hook_list(&i.obarray.symbol(id).value));
                } else {
                    fns.push(function);
                }
            }
            fns
        }
        None => hook_list(&i.obarray.symbol(id).value),
    }
}

fn want_hook_sym(i: &mut Interp, hook: &Value) -> Result<(), super::Flow> {
    match i.sym_id(hook) {
        Some(_) => Ok(()),
        None => Err(i.wrong_type_mut("symbolp", hook)),
    }
}

fn f_run_hooks(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    for hook in args.iter() {
        want_hook_sym(i, hook)?;
        let fns = hook_fns(i, hook);
        for f in fns {
            i.apply(&f, vec![])?;
        }
    }
    Ok(Value::Nil)
}

pub(crate) fn f_run_hook_with_args(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    let hook = args[0].clone();
    want_hook_sym(i, &hook)?;
    let fns = hook_fns(i, &hook);
    for f in fns {
        i.apply(&f, args[1..].to_vec())?;
    }
    Ok(Value::Nil)
}

fn f_run_hook_until_fail(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
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

fn f_run_hook_until_success(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
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

fn f_run_hook_wrapped(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    // (run-hook-wrapped HOOK WRAP-FUNCTION &rest ARGS)
    // GNU eval.c: each hook function is called through WRAP-FUNCTION;
    // if a call returns non-nil, hook processing aborts and returns it.
    // HOOK must be a symbol like the other run-hook-* entry points.
    let _ = want_sym(i, &args[0])?;
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

/// GNU `textget (plist, prop)' on a buffer overlay: direct plist hit,
/// else fall back through the `category' symbol's own plist like
/// `Foverlay_get'.
fn overlay_prop(i: &mut Interp, plist: &Value, prop: &str) -> Value {
    let pid = i.intern(prop);
    match crate::lisp::eval::plist_lookup_v(plist, &Value::Sym(pid)) {
        Some(v) => v,
        None => {
            let cat = i.intern("category");
            match crate::lisp::eval::plist_get(plist, cat) {
                Value::Sym(cs) => i.get_prop(cs, pid),
                _ => Value::Nil,
            }
        }
    }
}

/// GNU `call_mod_hooks': each element of LIST applied to (START END).
/// A non-list tail signals like GNU's `Fcar'.
fn call_mod_hooks(i: &mut Interp, list: &Value, beg1: usize, end1: usize) -> EvalResult {
    let mut cur = list.clone();
    loop {
        match cur {
            Value::Nil => return Ok(Value::Nil),
            Value::Cons(c) => {
                let (f, next) = {
                    let b = c.borrow();
                    (b.car.clone(), b.cdr.clone())
                };
                i.apply(&f, vec![Value::Int(beg1 as i128), Value::Int(end1 as i128)])?;
                cur = next;
            }
            other => return Err(i.wrong_type_mut("listp", &other)),
        }
    }
}

/// Whether the buffer's shared text ever gained an interval tree
/// (GNU `buffer_intervals (buf) != NULL' — it persists after all
/// properties are removed).
fn buffer_has_intervals(i: &Interp) -> bool {
    i.buffers
        .get(i.current_buffer)
        .map(|b| {
            let bb = b.borrow();
            bb.text.had_intervals.get() || !bb.text.props.borrow().is_empty()
        })
        .unwrap_or(false)
}

fn buffer_has_overlays(i: &Interp) -> bool {
    i.buffers
        .get(i.current_buffer)
        .map(|b| {
            let bid = b.borrow().id;
            b.borrow()
                .overlays
                .iter()
                .any(|o| o.buffer == Some(bid))
        })
        .unwrap_or(false)
}

/// Push VAL onto LIST unless the tail element is `eq' (GNU's
/// `! EQ (mod_hooks, prev_mod_hooks)' — consecutive intervals sharing
/// one hook list collapse to a single call).
fn push_dedup(list: &mut Vec<Value>, val: Value) {
    if !val.is_nil()
        && list
            .last()
            .map(|l| !super::eq_values(l, &val))
            .unwrap_or(true)
    {
        list.push(val);
    }
}

/// GNU `verify_interval_modification' insert branch: collect the
/// `insert-behind-hooks' of props covering the char before POS and the
/// `insert-in-front-hooks' of props covering the char at POS
/// (0-based POS == GNU 1-based START).
fn collect_insert_hooks(i: &mut Interp, pos: usize) {
    // GNU reads textget on the two interval plists — full
    // lookup_char_property (direct → category → alias → defaults),
    // which `prop_at_bb' mirrors on the flat store.
    let bh = i.intern("insert-behind-hooks");
    let fh = i.intern("insert-in-front-hooks");
    let Some(b) = i.buffers.get(i.current_buffer) else {
        return;
    };
    let (begv, zv) = {
        let bb = b.borrow();
        (bb.begv, bb.zv)
    };
    if pos > begv {
        let bb = b.borrow();
        let v = crate::buffer::primitives::prop_at_bb(i, &bb, pos - 1, bh);
        push_dedup(&mut i.pending_insert_behind, v);
    }
    if pos < zv {
        let bb = b.borrow();
        let v = crate::buffer::primitives::prop_at_bb(i, &bb, pos, fh);
        push_dedup(&mut i.pending_insert_front, v);
    }
}

/// GNU `verify_interval_modification' range branch: the
/// `modification-hooks' lists of every prop record overlapping
/// [S0,E0) (0-based), in position order with consecutive identical
/// lists collapsed.
fn collect_mod_hook_lists(i: &mut Interp, s0: usize, e0: usize) -> Vec<Value> {
    let Some(b) = i.buffers.get(i.current_buffer) else {
        return Vec::new();
    };
    let mh = i.intern("modification-hooks");
    let bounds: Vec<usize> = {
        let bb = b.borrow();
        // GNU iterates the interval set; on the flat store an
        // "interval" is a maximal span of constant coverage — segment
        // starts at s0 plus every record boundary inside [S0,E0).
        let mut cuts: Vec<usize> = vec![s0];
        for tp in bb.text.props.borrow().iter() {
            if tp.start > s0 && tp.start < e0 {
                cuts.push(tp.start);
            }
            if tp.end > s0 && tp.end < e0 {
                cuts.push(tp.end);
            }
        }
        cuts.sort_unstable();
        cuts.dedup();
        cuts
    };
    let mut out = Vec::new();
    for seg in bounds {
        let bb = b.borrow();
        // textget semantics: direct → category → alias → defaults.
        let v = crate::buffer::primitives::prop_at_bb(i, &bb, seg, mh);
        drop(bb);
        push_dedup(&mut out, v);
    }
    out
}

/// GNU `report_overlay_modification' before-phase: collect the
/// (hooklist . overlay) pairs the change will trigger.  Positions
/// arrive 1-based; overlay bounds are 0-based offsets so GNU's
/// `begin_arg == obegin' compares `beg1 == ov.start + 1'.
fn collect_overlay_hooks(i: &mut Interp, beg1: usize, end1: usize) {
    let insertion = beg1 == end1;
    let bid = i.current_buffer;
    let mut ovs: Vec<crate::buffer::Overlay> = i
        .buffers
        .get(bid)
        .map(|b| {
            let mut v: Vec<crate::buffer::Overlay> = b
                .borrow()
                .overlays
                .iter()
                .filter(|o| o.buffer == Some(bid))
                .cloned()
                .collect();
            // GNU's itree iterates overlays in ascending position.
            v.sort_by_key(|o| (o.start, o.end));
            v
        })
        .unwrap_or_default();
    let mut pending = Vec::new();
    for ov in ovs.drain(..) {
        let (os1, oe1) = (ov.start + 1, ov.end + 1);
        if insertion && (beg1 == os1 || end1 == os1) {
            let prop = overlay_prop(i, &ov.plist, "insert-in-front-hooks");
            if !prop.is_nil() {
                pending.push((prop, ov.handle.clone()));
            }
        }
        if insertion && (beg1 == oe1 || end1 == oe1) {
            let prop = overlay_prop(i, &ov.plist, "insert-behind-hooks");
            if !prop.is_nil() {
                pending.push((prop, ov.handle.clone()));
            }
        }
        // Strict intersection — covers deletion/replace ranges and
        // insertions strictly inside an overlay.
        if end1 > os1 && beg1 < oe1 {
            let prop = overlay_prop(i, &ov.plist, "modification-hooks");
            if !prop.is_nil() {
                pending.push((prop, ov.handle.clone()));
            }
        }
    }
    i.pending_overlay_hooks = pending;
}

/// Decode a `[overlay BID IDX]' handle without validating.
fn overlay_handle_parts(i: &Interp, v: &Value) -> Option<(usize, usize)> {
    if let Value::Vec(vec) = v {
        let vv = vec.borrow();
        if vv.len() == 3 {
            if let (Value::Sym(tag), Value::Int(bid), Value::Int(idx)) =
                (&vv[0], &vv[1], &vv[2])
            {
                if Some(*tag) == i.intern_soft("overlay") {
                    return Some((*bid as usize, *idx as usize));
                }
            }
        }
    }
    None
}

/// GNU `call_overlay_mod_hooks' over the recorded pairs:
/// (OVERLAY AFTER-P BEG END [&optional LEN]).  Entries whose overlay
/// was detached (or belongs to another buffer) are skipped like GNU's
/// `OVERLAY_BUFFER (overlay) == current_buffer' test.
fn run_overlay_hooks(
    i: &mut Interp,
    after: bool,
    beg1: usize,
    end1: usize,
    arg3: &Value,
) -> EvalResult {
    let pairs = i.pending_overlay_hooks.clone();
    for (list, handle) in pairs {
        let attached = overlay_handle_parts(i, &handle)
            .and_then(|(bid, idx)| {
                i.buffers
                    .get(bid)
                    .and_then(|b| b.borrow().overlays.get(idx).map(|o| o.buffer))
            })
            .map(|b| b == Some(i.current_buffer))
            .unwrap_or(false);
        if !attached {
            continue;
        }
        let mut cur = list;
        loop {
            match cur {
                Value::Nil => break,
                Value::Cons(c) => {
                    let (f, next) = {
                        let b = c.borrow();
                        (b.car.clone(), b.cdr.clone())
                    };
                    let mut argv = vec![
                        handle.clone(),
                        Value::from_bool(after),
                        Value::Int(beg1 as i128),
                        Value::Int(end1 as i128),
                    ];
                    if !arg3.is_nil() {
                        argv.push(arg3.clone());
                    }
                    i.apply(&f, argv)?;
                    cur = next;
                }
                other => return Err(i.wrong_type_mut("listp", &other)),
            }
        }
    }
    Ok(Value::Nil)
}

/// GNU `report_interval_modification': after an insertion, run the
/// `insert-behind-hooks' collected before it, then the
/// `insert-in-front-hooks' (unless the lists are eq — one call total).
fn report_interval_modification(i: &mut Interp, beg1: usize, end1: usize) -> EvalResult {
    // GNU's interval_insert_*_hooks are globals: the values STAY SET
    // after firing (only the next verify_interval_modification on a
    // buffer with an interval tree resets them), so inserts into a
    // property-free buffer replay whatever the last verify left —
    // including in a different buffer.  Clone, don't take.
    let behind = i.pending_insert_behind.clone();
    let front = i.pending_insert_front.clone();
    for list in behind.iter() {
        call_mod_hooks(i, list, beg1, end1)?;
    }
    let same = behind.len() == front.len()
        && behind
            .iter()
            .zip(front.iter())
            .all(|(a, b)| super::eq_values(a, b));
    if !same {
        for list in front.iter() {
            call_mod_hooks(i, list, beg1, end1)?;
        }
    }
    Ok(Value::Nil)
}

/// GNU's `combine_after_change_calls' gate: deferrable only when
/// `before-change-functions' is nil or the `(t syntax-ppss-flush-cache)'
/// local shape with a nil default, and the buffer has no overlays.
fn combine_gate_ok(i: &mut Interp) -> bool {
    let cacc = match i.intern_soft("combine-after-change-calls") {
        Some(s) => i.symbol_value(s).truthy(),
        None => return false,
    };
    if !cacc {
        return false;
    }
    let bcf_id = i.intern("before-change-functions");
    let bcf = i.symbol_value(bcf_id);
    let shape_ok = if bcf.is_nil() {
        true
    } else if let Value::Cons(c) = &bcf {
        let (car, cdr) = {
            let b = c.borrow();
            (b.car.clone(), b.cdr.clone())
        };
        let car_t = i.sym_is(&car, sym::T);
        let default_nil = match i.obarray.symbol(bcf_id).value {
            Value::Nil => true,
            Value::Sym(s) => s == sym::UNBOUND,
            _ => false,
        };
        let ppss = i.intern("syntax-ppss-flush-cache");
        let tail_ok = match &cdr {
            Value::Cons(c2) => {
                let b2 = c2.borrow();
                i.sym_is(&b2.car, ppss) && b2.cdr.is_nil()
            }
            _ => false,
        };
        car_t && default_nil && tail_ok
    } else {
        false
    };
    shape_ok && !buffer_has_overlays(i)
}

/// GNU `Fcombine_after_change_execute': merge the deferred changes of
/// the recorded buffer into one `signal_after_change' call.  The GNU
/// record_unwind_protect(Fcombine_after_change_execute_1,
/// Vcombine_after_change_calls) stores the CURRENT value back at
/// unwind — it never binds the flag to nil, so while
/// `combine-after-change-calls' stays bound the merged signal simply
/// re-defers (observed: hooks never run inside the `let').  Calling
/// `signal_after_change' directly reproduces that shape: under the
/// binding we re-record a single merged entry; once the binding drops,
/// the next real change flushes it.
pub(crate) fn combine_after_change_execute(i: &mut Interp) -> EvalResult {
    if i.combine_after_change_list.is_empty() {
        return Ok(Value::Nil);
    }
    let live = i
        .combine_after_change_buffer
        .and_then(|bid| i.buffers.get(bid).map(|b| b.borrow().live))
        .unwrap_or(false);
    if !live {
        i.combine_after_change_list.clear();
        i.combine_after_change_buffer = None;
        return Ok(Value::Nil);
    }
    let bid = i.combine_after_change_buffer.unwrap();
    let old = i.current_buffer;
    i.set_current_buffer(bid);
    let zv0 = i
        .buffers
        .get(bid)
        .map(|b| b.borrow().zv)
        .unwrap_or(0);
    let (mut beg, mut end, mut change) = (zv0, zv0, 0i128);
    for (tb, te, tc) in &i.combine_after_change_list {
        beg = beg.min(*tb);
        end = end.min(*te);
        change += *tc;
    }
    i.combine_after_change_list.clear();
    i.combine_after_change_buffer = None;
    // begpos0/endpos0 are the current 0-based bounds of the union
    // (GNU begpos = BEG + beg, endpos = Z - end).
    let (begpos0, endpos0) = (beg, zv0.saturating_sub(end));
    let lenins = endpos0.saturating_sub(begpos0) as i128;
    let lendel = lenins - change;
    let r = signal_after_change(i, begpos0 + 1, endpos0 + 1, lendel);
    i.set_current_buffer(old);
    r
}

 /// GNU `signal_before_change': run `first-change-hook' (when the
 /// buffer was unmodified) then `before-change-functions' with the
 /// 1-based region about to change.  An error aborts the modification.
/// Folded-in GNU `prepare_to_modify_buffer_1' pieces: the
/// `barf-if-buffer-read-only' check, `verify_interval_modification'
/// (text-property read-only, `modification-hooks' calls for
/// deletions/replacements, insert-hook collection for insertions),
/// and the overlay before-hooks — all under the GNU
/// `specbind (inhibit-modification-hooks)' scope for the hook calls.
 pub(crate) fn signal_before_change(i: &mut Interp, beg1: usize, end1: usize) -> EvalResult {
    // GNU `prepare_to_modify_buffer_1': barf-if-buffer-read-only
    // runs before any hook work, even under `inhibit-modification-hooks'.
    crate::buffer::primitives::check_writable(i)?;
    // `verify_interval_modification' — only when the buffer has an
    // interval tree; resets the saved insert-hook lists either way.
    if buffer_has_intervals(i) {
        i.pending_insert_behind.clear();
        i.pending_insert_front.clear();
        if beg1 == end1 {
            // Insertion: read-only stickiness around the position.
            crate::buffer::primitives::text_ro_insert(i, beg1 - 1)?;
            collect_insert_hooks(i, beg1 - 1);
        } else {
            crate::buffer::primitives::text_ro_range(i, beg1 - 1, end1 - 1)?;
            if !mod_hooks_inhibited(i) {
                for list in collect_mod_hook_lists(i, beg1 - 1, end1 - 1) {
                    call_mod_hooks(i, &list, beg1, end1)?;
                }
            }
        }
    }
     if mod_hooks_inhibited(i) {
         return Ok(Value::Nil);
     }
    let mark = i.specbind_depth();
    let imh = i.intern("inhibit-modification-hooks");
    let _ = i.specbind(imh, Value::t());
    let result = signal_before_change_inner(i, beg1, end1);
    let _ = i.unbind_to(mark);
    result
}

fn signal_before_change_inner(i: &mut Interp, beg1: usize, end1: usize) -> EvalResult {
     let unmodified = !i
         .buffers
         .get(i.current_buffer)
         .map(|b| b.borrow().is_modified())
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
        if let Err(fl) = i.apply(&f, args.clone()) {
            // GNU `reset_var_on_error': an erroring hook function
            // clears `before-change-functions'.
            let _ = i.set_symbol(id, Value::Nil);
            return Err(fl);
        }
     }
    if buffer_has_overlays(i) {
        collect_overlay_hooks(i, beg1, end1);
        run_overlay_hooks(i, false, beg1, end1, &Value::Nil)?;
    }
     Ok(Value::Nil)
 }
 
 /// GNU `report_after_change': `after-change-functions' with
 /// (BEG END LEN); LEN is the char length of the replaced text.
/// Handles the `combine-after-change-calls' deferral, replays the
/// recorded overlay hooks, and — for pure insertions — runs the
/// `insert-behind'/`insert-in-front' text-property hooks collected by
/// `signal_before_change', all inside the GNU
/// `specbind (inhibit-modification-hooks)' scope.
 pub(crate) fn signal_after_change(
     i: &mut Interp,
     beg1: usize,
     end1: usize,
    lendel: i128,
 ) -> EvalResult {
     if mod_hooks_inhibited(i) {
         return Ok(Value::Nil);
     }
    // GNU defers the whole after-report while
    // `combine-after-change-calls' is bound and the gate conditions
    // hold — the change just joins `combine_after_change_list'.
    if combine_gate_ok(i) {
        if !i.combine_after_change_list.is_empty()
            && i.combine_after_change_buffer != Some(i.current_buffer)
        {
            combine_after_change_execute(i)?;
        }
        let zv0 = i
            .buffers
            .get(i.current_buffer)
            .map(|b| b.borrow().zv)
            .unwrap_or(0);
        let lenins = (end1 - beg1) as i128;
        i.combine_after_change_list.push((
            beg1 - 1,
            zv0 + 1 - end1 + lendel as usize,
            lenins - lendel,
        ));
        i.combine_after_change_buffer = Some(i.current_buffer);
        return Ok(Value::Nil);
    }
    // Saved across the after-change calls: hook functions could
    // clobber the pending lists by modifying text properties.
    let saved_behind = std::mem::take(&mut i.pending_insert_behind);
    let saved_front = std::mem::take(&mut i.pending_insert_front);
    if !i.combine_after_change_list.is_empty() {
        combine_after_change_execute(i)?;
    }
    let mark = i.specbind_depth();
    let imh = i.intern("inhibit-modification-hooks");
    let _ = i.specbind(imh, Value::t());
    let args = vec![
        Value::Int(beg1 as i128),
        Value::Int(end1 as i128),
        Value::Int(lendel),
    ];
    let id = i.intern("after-change-functions");
    let fns = hook_fns(i, &Value::Sym(id));
    let mut result = Ok(Value::Nil);
    for f in fns {
        if let Err(fl) = i.apply(&f, args.clone()) {
            // GNU `reset_var_on_error' clears the hook on error.
            let _ = i.set_symbol(id, Value::Nil);
            result = Err(fl);
            break;
        }
    }
    i.pending_insert_behind = saved_behind;
    i.pending_insert_front = saved_front;
    if result.is_ok() {
        // GNU passes make_fixnum(lendel) — a fixnum 0 is not NILP, so
        // insert-after hooks get five args with LEN=0, matching the
        // before-phase's Qnil arg3 that yields four args.
        let arg3 = Value::Int(lendel);
        result = run_overlay_hooks(i, true, beg1, end1, &arg3);
    }
    if result.is_ok() && lendel == 0 {
        result = report_interval_modification(i, beg1, end1);
    }
    let _ = i.unbind_to(mark);
    result
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
/// if present there, else from its global (default) value.  GNU uses
/// identity here, removing every occurrence of the failing function.
fn safe_remove_hook_fn(i: &mut Interp, hook_id: SymId, f: &Value) {
    if let Some(b) = i.buffers.get(i.current_buffer) {
        let mut bb = b.borrow_mut();
        if let Some(v) = bb.locals.get(&hook_id).cloned() {
            let mut l = hook_list(&v);
            let before = l.len();
            l.retain(|x| !super::eq_values(x, f));
            if l.len() != before {
                bb.locals.insert(hook_id, Value::list(l));
                return;
            }
        }
    }
    let v = i.obarray.symbol(hook_id).value.clone();
    let mut l = hook_list(&v);
    let before = l.len();
    l.retain(|x| !super::eq_values(x, f));
    if l.len() != before {
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
    // GNU `local-variable-if-set-p' is per-buffer: `defvar' /
    // `permanent-local' globals count, but a `make-local-variable'
    // performed in another buffer does not.
    s.make_local_if_set
        || s.always_local
        || i.buffers
            .get(i.current_buffer)
            .map(|b| b.borrow().locals.contains_key(&id))
            .unwrap_or(false)
}

fn hook_boundp(i: &Interp, id: SymId) -> bool {
    !matches!(i.symbol_value(id), Value::Sym(s) if s == sym::UNBOUND)
}

fn hook_default_boundp(i: &Interp, id: SymId) -> bool {
    !matches!(i.obarray.symbol(id).value, Value::Sym(s) if s == sym::UNBOUND)
}

/// `(make-local-variable SYM)': create the buffer-local binding (seeded
/// with the default value) when absent.  GNU's `make-local-variable'
/// does NOT mark the symbol auto-local: `local-variable-if-set-p' is
/// nil in other buffers and `set' there stays global.
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

fn f_add_hook(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
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
    // GNU resolves the variable-alias chain before touching value
    // cells (`default-boundp'→default_value, `set'→set_internal both
    // follow SYMBOL_VAL redirection); the alias's own cell is a
    // legacy remnant.  `vid' below is BASE for value reads/writes.
    let vid = i.var_alias_target(hook_id);
    if !hook_boundp(i, hook_id) {
        i.set_symbol(hook_id, Value::Nil)?;
    }
    if !hook_default_boundp(i, vid) {
        i.set_symbol_default(hook_id, Value::Nil)?;
    }
    if local {
        // Unless the var is automatically buffer-local, the binding
        // is created fresh holding (t) -- the global value is not
        // copied into the local list.
        if !hook_local_if_set(i, vid) {
            hook_make_local(i, vid);
            i.set_symbol(hook_id, Value::list(vec![Value::Sym(sym::T)]))?;
        }
    } else if hook_force_local(i, vid) {
        local = true;
    }
    let mut hook_value = if local {
        i.symbol_value(hook_id)
    } else {
        i.obarray.symbol(vid).value.clone()
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
    if equal_position(i, &list, &fun)?.is_none() {
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
        i.set_symbol(hook_id, Value::list(list.clone()))?;
    } else {
        i.set_symbol_default(hook_id, Value::list(list.clone()))?;
    }
    // GNU `add-hook' returns the resulting hook list.
    Ok(Value::list(list))
}

fn f_remove_hook(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    let hook_id = want_sym(i, &args[0])?;
    let fun = args[1].clone();
    let mut local = args.get(2).map(|v| v.truthy()).unwrap_or(false);
    // GNU `remove-hook' reads `(default-value hook)' / `(symbol-value
    // hook)' and writes via `set-default'/`set', all of which resolve
    // the variable-alias chain.  `vid' is BASE for raw cell reads.
    let vid = i.var_alias_target(hook_id);
    if !hook_boundp(i, hook_id) {
        i.set_symbol(hook_id, Value::Nil)?;
    }
    if !hook_default_boundp(i, vid) {
        i.set_symbol_default(hook_id, Value::Nil)?;
    }
    // Do nothing if LOCAL is t but this hook has no local binding.
    if local && !hook_is_local(i, vid) {
        return Ok(Value::Nil);
    }
    if hook_force_local(i, vid) {
        local = true;
    }
    let hook_value = if local {
        i.symbol_value(hook_id)
    } else {
        i.obarray.symbol(vid).value.clone()
    };
    let (list, removed) = match hook_value.list_to_vec() {
        // A non-list value or a bare lambda form compares with `equal'.
        Err(_) => {
            let removed = if super::equal_values(i, &hook_value, &fun)? {
                Some(hook_value.clone())
            } else {
                None
            };
            (Vec::new(), removed)
        }
        Ok(items) => {
            let is_lambda = matches!(items.first(), Some(Value::Sym(s)) if *s == sym::LAMBDA);
            if is_lambda && super::equal_values(i, &hook_value, &fun)? {
                (Vec::new(), Some(hook_value.clone()))
            } else {
                // GNU: `(car (member ...))' locates with `equal';
                // `remq' then removes that same object.
                match equal_position(i, &items, &fun)? {
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
    if let Some(old) = &removed {
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
        i.set_symbol_default(hook_id, Value::list(list.clone()))?;
    } else if list.len() == 1 && matches!(&list[0], Value::Sym(s) if *s == sym::T) {
        // A local value of exactly (t) kills the local binding.
        if let Some(b) = i.buffers.get(i.current_buffer) {
            if let Ok(mut bb) = b.try_borrow_mut() {
                bb.locals.shift_remove(&hook_id);
            }
        }
    } else {
        i.set_symbol(hook_id, Value::list(list.clone()))?;
    }
    // GNU `remove-hook' returns the resulting hook list (or the
    // untouched non-list value when nothing matched).
    if removed.is_none() && hook_value.list_to_vec().is_err() {
        return Ok(hook_value);
    }
    Ok(Value::list(list))
}

fn f_identity(_i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    Ok(args[0].clone())
}
fn f_ignore(_i: &mut Interp, _args: crate::lisp::Argv) -> EvalResult {
    Ok(Value::Nil)
}
fn f_always(_i: &mut Interp, _args: crate::lisp::Argv) -> EvalResult {
    Ok(Value::t())
}
fn f_prog1_raw(i: &mut Interp, _args: crate::lisp::Argv) -> EvalResult {
    f_special_form_via_apply(i, Vec::new().into())
}
fn f_or_raw(i: &mut Interp, _args: crate::lisp::Argv) -> EvalResult {
    f_special_form_via_apply(i, Vec::new().into())
}
fn f_and_raw(i: &mut Interp, _args: crate::lisp::Argv) -> EvalResult {
    f_special_form_via_apply(i, Vec::new().into())
}
fn f_values(_i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    Ok(Value::list(args.as_slice().to_vec()))
}
fn f_eval_expression(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
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
fn f_load(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    // (load FILE &optional NOERROR NOMESSAGE NOSUFFIX MUST-SUFFIX)
    let name = match &args[0] {
        Value::Str(s) => s.borrow().clone(),
        _ => return Err(i.wrong_type_mut("stringp", &args[0])),
    };
    let noerror = arg(&args, 1).truthy();
    let nomessage = arg(&args, 2).truthy();
    let nosuffix = arg(&args, 3).truthy();
    let mustsuffix = arg(&args, 4).truthy();
    let outcome =
        crate::lisp::load::load_library_opts_msg(i, &name, nosuffix, mustsuffix, true, nomessage)?;
    match outcome {
        crate::lisp::load::LoadOutcome::Loaded(_) => Ok(Value::t()),
        crate::lisp::load::LoadOutcome::Missing | crate::lisp::load::LoadOutcome::Dir(_)
            if noerror =>
        {
            Ok(Value::Nil)
        }
        crate::lisp::load::LoadOutcome::Missing => {
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
        // GNU reports the original FILE argument verbatim — openp
        // successfully found the (possibly `~`-expanded) name.
        crate::lisp::load::LoadOutcome::Dir(_) => Err(i.signal_data(
            crate::lisp::sym::FILE_ERROR,
            vec![
                Value::string("Cannot open load file"),
                Value::string("Is a directory"),
                args[0].clone(),
            ],
        )),
    }
}
fn f_load_file(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
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
    let outcome = crate::lisp::load::load_library_opts(i, &name, true, false, true)?;
    match outcome {
        crate::lisp::load::LoadOutcome::Loaded(_) => Ok(Value::t()),
        crate::lisp::load::LoadOutcome::Dir(d) => Err(i.signal_data(
            crate::lisp::sym::FILE_ERROR,
            vec![
                Value::string("Cannot open load file"),
                Value::string("Is a directory"),
                Value::string(d),
            ],
        )),
        crate::lisp::load::LoadOutcome::Missing => {
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
}
fn f_locate_library(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    // (locate-library LIBRARY &optional NOSUFFIX PATH INTERACTIVE-CALL)
    // GNU subr.el: `locate-file' over PATH (default `load-path') with
    // suffixes (get-load-suffixes) ∪ load-file-rep-suffixes when
    // NOSUFFIX is nil, else just load-file-rep-suffixes.
    let name = match &args[0] {
        Value::Str(s) => s.borrow().clone(),
        _ => return Err(i.wrong_type_mut("stringp", &args[0])),
    };
    let nosuffix = arg(&args, 1).truthy();
    let path = arg(&args, 2);
    let interactive = arg(&args, 3).truthy();
    let default_path = path.is_nil();
    let search_path = if default_path {
        let id = i.intern("load-path");
        i.symbol_value(id)
    } else {
        path
    };
    let mut file = locate_library_in(i, &name, nosuffix, &search_path)?;
    if default_path {
        file = match file {
            Some(path) => Some(crate::lisp::load::located_source(i, &name, path)?),
            None => {
                let suffixes = locate_library_suffixes(i, nosuffix);
                crate::lisp::load::locate_bundled_source(i, &name, &suffixes)?
            }
        };
    }
    if interactive {
        // GNU reports through `message' for M-x calls.
        let msg = match &file {
            Some(f) => format!("Library is file {f}"),
            None => format!("No library {name} in search path"),
        };
        let msg_sym = i.intern("message");
        let _ = i.apply(
            &Value::Sym(msg_sym),
            vec![Value::string("%s"), Value::string(msg)],
        );
    }
    Ok(match file {
        Some(f) => Value::string(f),
        None => Value::Nil,
    })
}

/// `locate-library''s PATH-given search: scan DIRS only (no bundled
/// `builtin_dirs` fallback — an explicit PATH is authoritative, so a
/// bogus dir list yields nil like GNU's `locate-file').
fn locate_library_in(
    i: &mut Interp,
    name: &str,
    nosuffix: bool,
    path: &Value,
) -> Result<Option<String>, Flow> {
    // GNU `locate-file' iterates PATH as a list; a non-list PATH
    // contributes no directories (returns nil, no error).
    let dirs = path.list_to_vec().unwrap_or_default();
    let suffixes = locate_library_suffixes(i, nosuffix);
    if std::path::Path::new(name).is_absolute() || name.starts_with('~') {
        // GNU `locate-file': absolute names only get suffix variants.
        let name = crate::editor::expand_file_name_str(i, name);
        for suf in &suffixes {
            let cand = format!("{name}{suf}");
            if std::path::Path::new(&cand).is_file() {
                return Ok(Some(cand));
            }
        }
        return Ok(None);
    }
    for dir_v in &dirs {
        let directory = match dir_v {
            Value::Str(value) => crate::editor::expand_file_name_str(i, &value.borrow()),
            Value::Nil => crate::editor::expand_file_name_str(i, ""),
            _ => continue,
        };
        let base = std::path::Path::new(&directory)
            .join(name)
            .to_string_lossy()
            .into_owned();
        let base = crate::editor::expand_file_name_str(i, &base);
        for suffix in &suffixes {
            let candidate = format!("{base}{suffix}");
            if std::path::Path::new(&candidate).is_file() {
                return Ok(Some(candidate));
            }
        }
        if let Some(path) = crate::lisp::load::locate_cache_source(
            i, std::path::Path::new(&directory), name, &suffixes,
        )? {
            return Ok(Some(path));
        }
    }
    Ok(None)
}

/// Suffix list GNU's `locate-library' feeds to `locate-file':
/// `get-load-suffixes' (NOSUFFIX drops it) then `load-file-rep-suffixes'.
fn locate_library_suffixes(i: &mut Interp, nosuffix: bool) -> Vec<String> {
    let str_list = |i: &mut Interp, var: &str| -> Vec<String> {
        let id = i.intern(var);
        i.symbol_value(id)
            .list_to_vec()
            .unwrap_or_default()
            .iter()
            .filter_map(|v| match v {
                Value::Str(s) => Some(s.borrow().clone()),
                _ => None,
            })
            .collect()
    };
    let mut sufs: Vec<String> = Vec::new();
    if !nosuffix {
        // get-load-suffixes: load-suffixes with rep-suffixes attached
        // to the .elc/.el entries, then `load-file-rep-suffixes' (the
        // "" entry gives the bare-name try).
        for s in str_list(i, "load-suffixes") {
            if s.ends_with(".elc") || s.ends_with(".el") {
                for rep in str_list(i, "load-file-rep-suffixes") {
                    sufs.push(format!("{s}{rep}"));
                }
            } else {
                sufs.push(s);
            }
        }
    }
    sufs.extend(str_list(i, "load-file-rep-suffixes"));
    if sufs.is_empty() {
        sufs.push(String::new());
    }
    sufs
}

fn f_autoload(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    // (autoload FUNCTION FILE &optional DOCSTRING INTERACTIVE TYPE)
    // Register an autoload stub: on first call, load FILE then call.
    let fid = want_sym(i, &args[0])?;
    let file = args[1].clone();
    // GNU `Fautoload' checks FILE's type before the fbound early-out.
    if !matches!(file, Value::Str(_)) {
        return Err(i.wrong_type_mut("stringp", &file));
    }
    let existing = i.symbol_function(fid);
    let auto_id = i.intern("autoload");
    let replaceable = matches!(&existing, Value::Cons(c) if i.sym_is(&c.borrow().car,auto_id));
    if i.fbound_p(fid) && !replaceable {
        // GNU replaces existing autoload cells, while a real definition
        // (including an alias) is a no-op returning nil.
        return Ok(Value::Nil);
    }
    // GNU's autoload cell is always (autoload FILE DOC INTERACTIVE TYPE)
    // — absent optional fields are stored as nil.
    let mut cell = vec![Value::Sym(auto_id), file];
    for k in 2..5 {
        cell.push(args.get(k).cloned().unwrap_or(Value::Nil));
    }
    let definition = Value::list(cell);
    // Fautoload delegates to Fdefalias for history and advice handling.
    crate::lisp::builtins::data::f_defalias(i, vec![args[0].clone(), definition].into())
}
fn f_autoloadp(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    let auto_id = i.intern("autoload");
    let macro_id = i.intern("macro");
    // Macro autoload cells are (macro . (autoload ...)): GNU's
    // Fautoloadp unwraps the `macro' cons before the car check.
    let mut v = &args[0];
    let cdr;
    if let Value::Cons(c) = v {
        let b = c.borrow();
        if i.sym_is(&b.car, macro_id) {
            cdr = b.cdr.clone();
            drop(b);
            v = &cdr;
        }
    }
    match v {
        Value::Cons(c) => {
            let b = c.borrow();
            Ok(Value::from_bool(i.sym_is(&b.car, auto_id)))
        }
        _ => Ok(Value::Nil),
    }
}

fn f_autoload_do_load(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    let fundef = args[0].clone();
    // GNU: (autoload-do-load FUNDEF &optional FUNNAME MACRO-ONLY) —
    // MACRO-ONLY is the THIRD argument.
    let macro_only = args.get(2).map(|v| v.truthy()).unwrap_or(false);
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
    // GNU inspects TYPE with nth before the macro-only early return.
    // A dotted prefix reports listp on the original cell; a dotted tail
    // beyond TYPE is never traversed and does not prevent loading.
    let kind = super::listfn::f_nth(i, vec![Value::Int(4), fundef.clone()].into())?;
    let is_macro_autoload = kind.truthy();
    if macro_only && !is_macro_autoload {
        return Ok(fundef);
    }
    let file = super::listfn::f_nth(i, vec![Value::Int(1), fundef.clone()].into())?;
    let name = match &file {
        Value::Str(s) => s.borrow().clone(),
        _ => return Err(i.wrong_type_mut("stringp", &file)),
    };
    // Find the symbol whose function cell is this autoload object.
    let mut owner = i
        .obarray
        .all_ids()
        .into_iter()
        .find(|id| super::eq_values(&i.symbol_function(*id), &fundef));
    // Interpreted code can hold a structurally identical but
    // distinct `(autoload ...)' cons — e.g. one read afresh from a
    // loaddefs form while a different cons sits in the function
    // cell.  Fall back to an `equal' match so we still attribute
    // the autoload to its symbol (and to its dumped stash).
    if owner.is_none() {
        for id in i.obarray.all_ids() {
            if super::equal_values(i, &i.symbol_function(id), &fundef)? {
                owner = Some(id);
                break;
            }
        }
    }
    if i.dbg_flags & 0x800 != 0 {
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
    // GNU Fload compares the autoload's resolved FILE against
    // Vloads_in_progress' resolved paths — never basenames: a user
    // file named "sort.el" must not make `sort-lines' look recursive.
    // The in-progress stack stores `builtin:NAME' virtual paths for
    // embedded libraries and disk `found' paths otherwise, so resolve
    // the target to the same key space (`load_key' normalizes
    // bundled-dir hits to their `builtin:<stem>').
    let resolved_key = match crate::lisp::load::locate_full(i, &name, false, false) {
        Some(crate::lisp::load::Found::File(p)) => crate::lisp::load::load_key(&p),
        Some(crate::lisp::load::Found::Dir(_)) => String::new(),
        None => crate::lisp::load::load_key(&format!("builtin:{name}")),
    };
    let loading_same = i
        .loads_in_progress
        .iter()
        .any(|f| crate::lisp::load::load_key(f) == resolved_key);
    let stem = |s: &str| {
        let s = s.strip_prefix("builtin:").unwrap_or(s);
        let base = s.rsplit('/').next().unwrap_or(s);
        let base = base.strip_suffix(".el").unwrap_or(base);
        base.strip_suffix(".elc").unwrap_or(base).to_string()
    };
    if let Some(id) = owner {
        let pk = i.intern("remacs--dump-fn");
        let hidden = i.get_prop(id, pk);
        let has_hidden =
            !hidden.is_nil() && !matches!(hidden, Value::Sym(s) if s == crate::lisp::sym::UNBOUND);
        // Only the SELF-autoload case (loading the same file) may
        // resolve through the stash: for a foreign target GNU really
        // loads the file, and skipping that load leaves the file's
        // non-function state (e.g. pcase.el's `pcase--memoize'
        // defconst) unbound while its macros still work.
        if (loading_same && is_macro_autoload) && has_hidden {
            return Ok(hidden);
        }
        if loading_same {
            if i.dbg_flags & 0x1000 != 0 {
                eprintln!(
                    "=== recursive autoload of {} for {}; last 30 frames ===",
                    name,
                    i.symbol_name(id)
                );
                for (f, a) in i.lisp_stack.iter().rev().take(30).rev() {
                    eprintln!(
                        "  {} {}",
                        i.princ_to_string(f).chars().take(80).collect::<String>(),
                        i.princ_to_string(&Value::list(a.as_slice().to_vec()))
                            .chars()
                            .take(120)
                            .collect::<String>()
                    );
                }
            }
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
    // If the target's feature is already provided, the file's real
    // definitions ran before the loaddefs pass reinstalled this
    // autoload cell — loading it again would just re-evaluate the
    // file.  The dump-time stash holds the definition GNU's .elc
    // would have installed; resolve through it directly.
    let feat = i.intern(&stem(&name));
    if feature_present(i, feat) {
        if let Some(id) = owner {
            let pk = i.intern("remacs--dump-fn");
            let hidden = i.get_prop(id, pk);
            if !hidden.is_nil()
                && !matches!(hidden, Value::Sym(s) if s == crate::lisp::sym::UNBOUND)
            {
                i.fset(id, hidden.clone());
                return Ok(hidden);
            }
        }
    }
    // GNU autoload_do_load → load_with_autoload_queue passes
    // nomessage=t to Fload: autoload-triggered loads don't echo
    // "Loading ..." (stderr or *Messages*).
    match crate::lisp::load::load_library_opts_msg(i, &name, false, false, true, true)? {
        crate::lisp::load::LoadOutcome::Missing => {
            return Err(i.signal_data(
                crate::lisp::sym::FILE_MISSING,
                vec![
                    Value::string("Cannot open load file"),
                    Value::string("No such file or directory"),
                    Value::string(&name),
                ],
            ));
        }
        crate::lisp::load::LoadOutcome::Dir(d) => {
            return Err(i.signal_data(
                crate::lisp::sym::FILE_ERROR,
                vec![
                    Value::string("Cannot open load file"),
                    Value::string("Is a directory"),
                    Value::string(d),
                ],
            ));
        }
        crate::lisp::load::LoadOutcome::Loaded(_) => {}
    }
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
                // install it: GNU resolves an autoload cell once, so
                // leaving it in place would reload the file on every
                // call (each interpreted load costs ~0.4s).
                let pk = i.intern("remacs--dump-fn");
                let hidden = i.get_prop(id, pk);
                if !hidden.is_nil()
                    && !matches!(hidden, Value::Sym(s) if s == crate::lisp::sym::UNBOUND)
                {
                    i.fset(id, hidden.clone());
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

fn f_handler_bind_1(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    // (handler-bind-1 BODY-FN CONDS HANDLER CONDS HANDLER ...) — bind
    // dynamic signal handlers while running the 0-argument BODY-FN.
    let body = args[0].clone();
    let base = i.handler_bindings.len();
    // Pairs are consulted innermost-first (reverse stack order), so
    // push them in reverse to consult them in argument order, with all
    // of this call's bindings inner to any outer `handler-bind-1'.
    // Pairs need an even count after BODY-FN: len = 1 + 2k, so an even
    // raw arity means a dangling CONDITIONS — GNU's ‘handler-bind’
    // signals "Trailing CONDITIONS without HANDLER".
    if args.len() % 2 == 0 {
        return Err(i.error("Trailing CONDITIONS without HANDLER in ‘handler-bind’"));
    }
    // Pairs sit at indices 1,2 / 3,4 / …; start at the last pair and
    // push back-to-front so the FIRST pair lands innermost.
    let mut k = args.len().saturating_sub(1);
    while k >= 2 {
        i.handler_bindings
            .push((args[k - 1].clone(), args[k].clone()));
        k -= 2;
    }
    let r = i.apply(&body, vec![]);
    i.handler_bindings.truncate(base);
    r
}

fn f_access_file(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
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

fn f_current_message(i: &mut Interp, _args: crate::lisp::Argv) -> EvalResult {
    if i.echo_message.is_empty() {
        Ok(Value::Nil)
    } else {
        Ok(Value::string(i.echo_message.clone()))
    }
}
fn f_eval_buffer(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    // (eval-buffer &optional BUFFER ...) — GNU lread.c:
    // `Fget_buffer' on a non-nil arg (LOOKUP ONLY — never creates);
    // unresolvable → plain error "No such buffer".
    let buf_id = match args.get(0) {
        Some(v) if !v.is_nil() => match v {
            Value::Buffer(b) => b.borrow().id,
            Value::Str(s) => {
                let name = s.borrow().clone();
                match i.buffers.by_name(&name) {
                    Some(id) => id,
                    None => return Err(i.error("No such buffer")),
                }
            }
            other => return Err(i.wrong_type_mut("stringp", other)),
        },
        _ => i.current_buffer,
    };
    let Some(buffer) = i.buffers.get(buf_id) else {
        return Err(i.error("No such buffer"));
    };
    let filename = match arg(&args, 2) {
        Value::Nil => buffer
            .borrow()
            .file_name
            .as_ref()
            .map(Value::string)
            .unwrap_or(Value::Nil),
        filename @ Value::Str(_) => filename,
        other => return Err(i.wrong_type_mut("stringp", &other)),
    };
    let saved = i.save_excursion_state(false);
    let begv = buffer.borrow().begv;
    buffer.borrow_mut().set_point(begv);
    let r = eval_buffer_source(
        i,
        &buffer,
        Value::Nil,
        Value::Nil,
        arg(&args, 1),
        Value::Nil,
        arg(&args, 4).truthy(),
        true,
        filename,
    );
    i.restore_excursion_state(saved);
    r
}

fn f_eval_region(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    let buffer = i.buffers.get(i.current_buffer).unwrap();
    let filename = buffer
        .borrow()
        .file_name
        .as_ref()
        .map(Value::string)
        .unwrap_or(Value::Nil);
    eval_buffer_source(
        i,
        &buffer,
        args[0].clone(),
        args[1].clone(),
        arg(&args, 2),
        arg(&args, 3),
        false,
        false,
        filename,
    )
}

/// GNU lread.c's lisp_file_lexical_cookie examines only the first comment
/// line, or the comment immediately after a shebang. Return the stream
/// position consumed by that examination so the fallback hook sees it.
fn buffer_lexical_cookie(source: &str) -> (Option<bool>, usize) {
    let chars: Vec<_> = source.chars().collect();
    let mut start = 0;
    if chars.starts_with(&['#', '!']) {
        start = chars
            .iter()
            .position(|&c| c == '\n')
            .map_or(chars.len(), |p| p + 1);
    }
    if chars.get(start) != Some(&';') {
        return (None, start);
    }
    let end = chars[start..]
        .iter()
        .position(|&c| c == '\n')
        .map_or(chars.len(), |p| start + p);
    let consumed = (end + 1).min(chars.len());
    let line: String = chars[start..end].iter().collect();
    let Some(open) = line.find("-*-") else {
        return (None, consumed);
    };
    let variables = &line[open + 3..];
    let variables = variables.split("-*-").next().unwrap_or("");
    let mut remaining = variables;
    while let Some((name, rest)) = remaining.split_once(':') {
        let (value, next) = rest.split_once(';').map_or((rest, ""), |parts| parts);
        if name.trim_matches([' ', '\t']) == "lexical-binding" {
            return (Some(value.trim_matches([' ', '\t']) != "nil"), consumed);
        }
        remaining = next;
    }
    (None, consumed)
}

/// Read the live buffer, as in GNU lread.c's readevalloop.  In particular,
/// START is a goto-char position, not a substring bound: it is clipped to
/// the accessible range.  A nil START reads at point and ignores END.
fn eval_buffer_source(
    i: &mut Interp,
    buffer: &crate::lisp::value::BufferRef,
    mut start: Value,
    mut end: Value,
    printflag: Value,
    read_function: Value,
    allow_print: bool,
    read_lexical_cookie: bool,
    filename: Value,
) -> EvalResult {
    let filename = if let Value::Str(text) = &filename {
        let text = text.borrow().clone();
        if std::path::Path::new(&text).is_absolute() || text.starts_with('~') {
            Value::string(crate::editor::expand_file_name_str(i, &text))
        } else {
            filename
        }
    } else {
        filename
    };
    let mut entire = None;
    let depth = i.specbind_depth();
    let saved_lexenv = i.lexenv.clone();
    let mut result = i.with_eval_context(false, |i| {
        let output = i.intern("standard-output");
        let sink = if printflag.is_nil() && !allow_print {
            Value::Sym(i.intern("symbolp"))
        } else {
            printflag.clone()
        };
        i.specbind(output, sink)?;
        let list = i.intern("eval-buffer-list");
        let old_list = i.symbol_value(list);
        i.specbind(list, Value::cons(Value::Buffer(buffer.clone()), old_list))?;
        let lexical = i.intern("lexical-binding");
        if read_lexical_cookie {
            // GNU eval-buffer only scans a cookie when the source buffer
            // has no local binding. eval-region never scans a header.
            if !buffer.borrow().locals.contains_key(&lexical) {
                let (cookie, consumed) = {
                    let b = buffer.borrow();
                    buffer_lexical_cookie(&b.text.substring(b.begv, b.zv))
                };
                let begv = buffer.borrow().begv;
                buffer.borrow_mut().set_point(begv + consumed);
                let value = match cookie {
                    Some(enabled) => Value::from_bool(enabled),
                    None => {
                        let hook = i.intern("internal--get-default-lexical-binding-function");
                        let function = i.symbol_value(hook);
                        if function.is_nil() {
                            i.default_toplevel_value(lexical).unwrap_or(Value::Nil)
                        } else {
                            i.apply(&function, vec![Value::Buffer(buffer.clone())])?
                        }
                    }
                };
                i.specbind(lexical, value)?;
            }
            let begv = buffer.borrow().begv;
            buffer.borrow_mut().set_point(begv);
        }
        i.lexenv = if i.symbol_value(lexical).truthy() {
            crate::lisp::eval::lexenv_root()
        } else {
            None
        };
        let current = i.intern("current-load-list");
        i.specbind(current, Value::list(vec![filename.clone()]))?;
        let input = i.intern("standard-input");
        i.specbind(input, Value::Buffer(buffer.clone()))?;
        let mut source_version = None;
        let mut source_tick = None;
        let mut chars = Rc::new(Vec::new());
        loop {
            if !buffer.borrow().live {
                return Err(i.error("Reading from killed buffer"));
            }
            // Excursions create short-lived markers.  Drop their expired
            // registrations so reading many forms does not accumulate them.
            buffer
                .borrow_mut()
                .markers
                .retain(|m| m.strong_count() != 0);
            let buffer_id = buffer.borrow().id;
            if i.current_buffer != buffer_id {
                if let Some(caller) = i.buffers.get(i.current_buffer) {
                    caller
                        .borrow_mut()
                        .markers
                        .retain(|m| m.strong_count() != 0);
                }
            }
            // A region only temporarily changes point and narrowing while
            // reading.  Evaluated forms see the original restriction and
            // point, and edits move the saved stream positions via markers.
            let saved = if !start.is_nil() {
                let caller = i.save_excursion_state(false);
                let buffer_id = buffer.borrow().id;
                i.set_current_buffer(buffer_id);
                Some((
                    caller,
                    i.save_excursion_state(false),
                    i.save_restriction_state(),
                ))
            } else {
                None
            };
            let read = (|| {
                if !start.is_nil() {
                    let pos = super::want_int_or_mark(i, &start)?;
                    // Fgoto_char accepts fixnums/markers, not bignums.
                    if !(-(1i128 << 61)..(1i128 << 61)).contains(&pos) {
                        return Err(i.wrong_type_mut("integer-or-marker-p", &start));
                    }
                    let (begv, zv) = {
                        let b = buffer.borrow();
                        (b.begv, b.zv)
                    };
                    buffer
                        .borrow_mut()
                        .set_point(pos.clamp(begv as i128 + 1, zv as i128 + 1) as usize - 1);
                    if !end.is_nil() {
                        let limit = super::want_int_or_mark(i, &end)?;
                        let len = buffer.borrow().text.len();
                        if limit < 1 || limit > len as i128 + 1 {
                            return Err(i.signal_data(
                                sym::ARGS_OUT_OF_RANGE,
                                vec![Value::Int(begv as i128 + 1), Value::Int(limit)],
                            ));
                        }
                        let limit = limit as usize - 1;
                        let mut b = buffer.borrow_mut();
                        b.begv = begv.min(limit);
                        b.zv = begv.max(limit);
                        let point = b.point();
                        b.set_point(point);
                        if matches!(end, Value::Int(_)) {
                            let marker = crate::lisp::memory::allocate(
                                Marker {
                                    buffer: Some(b.id),
                                    position: b.zv,
                                    insertion_type: false,
                                },
                                crate::lisp::Value::Marker,
                            );
                            b.register_marker(&marker);
                            end = Value::Marker(marker);
                        }
                    }
                }
                if entire.is_none() {
                    let buffer = buffer.borrow();
                    entire = Some(buffer.point() == 0 && buffer.zv == buffer.text.len());
                }
                // Read only BEGV..ZV, keeping offsets relative to BEGV for
                // invalid-read-syntax locations.  Reuse unchanged source
                // text; forms can edit it or change its restriction.
                let (begv, mut pos) = {
                    let b = buffer.borrow();
                    let version = (b.text.chars_mod_tick.get(), b.begv, b.zv);
                    if source_version != Some(version)
                        || source_tick
                            .as_ref()
                            .is_none_or(|tick| !Rc::ptr_eq(tick, &b.text.chars_mod_tick))
                    {
                        source_tick = Some(b.text.chars_mod_tick.clone());
                        chars = Rc::new(b.text.substring(b.begv, b.zv).chars().collect::<Vec<_>>());
                        source_version = Some(version);
                    }
                    (b.begv, b.point().saturating_sub(b.begv))
                };
                // readevalloop peeks through layout before calling a custom
                // reader, so whitespace-only input never calls READ-FUNCTION.
                loop {
                    while pos < chars.len()
                        && matches!(chars[pos], ' ' | '\t' | '\n' | '\x0c' | '\r' | '\u{a0}')
                    {
                        pos += 1;
                    }
                    if pos < chars.len() && chars[pos] == ';' {
                        while pos < chars.len() && chars[pos] != '\n' {
                            pos += 1;
                        }
                    } else {
                        break;
                    }
                }
                buffer.borrow_mut().set_point(begv + pos);
                if pos >= chars.len() {
                    return Ok(None);
                }
                let lrf = i.intern("load-read-function");
                let reader_function = if read_function.is_nil() {
                    i.symbol_value(lrf)
                } else {
                    read_function.clone()
                };
                let builtin_read = match &reader_function {
                    Value::Nil => true,
                    Value::Subr(s) => s.name == "read",
                    Value::Sym(s) => {
                        matches!(i.symbol_function(*s), Value::Subr(f) if f.name == "read")
                    }
                    _ => false,
                };
                let form = if builtin_read {
                    let (result, position) =
                        super::readfn::read_parser_at(i, chars.clone(), pos, None, true);
                    buffer.borrow_mut().set_point(begv + position);
                    match result {
                        Ok(Some(form)) => form,
                        Ok(None) => return Ok(None),
                        Err(Flow::Signal(Value::Sym(s), _, flags)) if s == sym::END_OF_FILE => {
                            return Err(Flow::Signal(
                                Value::Sym(s),
                                Value::list(vec![Value::Buffer(buffer.clone())]),
                                flags,
                            ));
                        }
                        Err(error) => return Err(error),
                    }
                } else {
                    i.apply(&reader_function, vec![Value::Buffer(buffer.clone())])?
                };
                let stop = !read_function.is_nil() && buffer.borrow().point() == buffer.borrow().zv;
                if !start.is_nil() && !stop {
                    let mut b = buffer.borrow_mut();
                    let marker = crate::lisp::memory::allocate(
                        Marker {
                            buffer: Some(b.id),
                            position: b.point(),
                            insertion_type: false,
                        },
                        crate::lisp::Value::Marker,
                    );
                    b.register_marker(&marker);
                    start = Value::Marker(marker);
                }
                Ok(Some((form, stop)))
            })();
            if let Some((caller, point, restriction)) = saved {
                i.restore_restriction_state(restriction);
                i.restore_excursion_state(point);
                i.restore_excursion_state(caller);
            }
            let Some((form, stop)) = read? else {
                return Ok(Value::Nil);
            };
            let value = i.eval(&form)?;
            if printflag.truthy() {
                let values = i.intern("values");
                let old = i.symbol_value(values);
                i.set_symbol(values, Value::cons(value.clone(), old))?;
                let destination = i.symbol_value(output);
                let printer = i.intern(if matches!(destination, Value::Sym(s) if s == sym::T) {
                    "prin1"
                } else {
                    "print"
                });
                i.apply(&Value::Sym(printer), vec![value])?;
            }
            if stop {
                return Ok(Value::Nil);
            }
        }
    });
    if result.is_ok() {
        if let Err(error) = crate::lisp::load::build_history(i, filename, entire.unwrap_or(false)) {
            result = Err(error);
        }
    }
    i.lexenv = saved_lexenv;
    i.unbind_to(depth)?;
    result
}

fn f_command_execute(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    // GNU simple.el
    // `command-execute': unless SPECIAL, the pending `prefix-arg' first
    // becomes `current-prefix-arg' and is cleared (the terminal command
    // loop does this itself before its direct `Interp::command_execute').
    if !args.get(3).is_some_and(|v| v.truthy()) {
        let pa = i.intern("prefix-arg");
        let prefix = i.symbol_value(pa);
        let cpa = i.intern("current-prefix-arg");
        i.set_symbol(cpa, prefix.clone())?;
        i.set_symbol(pa, Value::Nil)?;
        let upd = i.intern("prefix-command-update");
        if prefix.truthy() && i.fbound_p(upd) {
            i.apply(&Value::Sym(upd), vec![])?;
        }
    }
    // GNU `command-execute' (simple.el defun): an arrayp CMD — a
    // keyboard-macro string or vector — goes to `execute-kbd-macro'
    // and never reaches `call-interactively', so KEYS is unchecked
    // and `commandp' never consulted for these.
    if matches!(args[0], Value::Str(_) | Value::Vec(_)) {
        return crate::editor::f_execute_kbd_macro_pub(i, vec![args[0].clone()].into());
    }
    // GNU `command-execute' falls into `call-interactively'
    // (callint.c), which runs CHECK_VECTOR on a given KEYS argument
    // before the `commandp' error — the check order is observable
    // in the error datum.
    if let Some(keys) = args.get(2) {
        if !matches!(keys, Value::Vec(_) | Value::Nil) {
            return Err(i.wrong_type_mut("vectorp", keys));
        }
    }
    i.command_execute(&args[0])
}
fn f_called_interactively_p(i: &mut Interp, _args: crate::lisp::Argv) -> EvalResult {
    // True while a `call-interactively'/command-loop call is on the
    // stack (the KIND argument selects among frames; the depth flag
    // collapses them like a batch GNU where only `any' matters).
    Ok(Value::from_bool(i.interactive_call_depth > 0))
}
fn f_funcall_with_delayed_message(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    // ARGS =
    // (TIMEOUT MESSAGE FUNCTION): GNU CHECK_NUMBER's TIMEOUT and
    // CHECK_STRING's MESSAGE before scheduling an idle timer for the
    // message; the function itself runs in place here.
    match &args[0] {
        Value::Int(_) | Value::Float(_) => {}
        other => return Err(i.wrong_type_mut("numberp", other)),
    }
    match &args[1] {
        Value::Str(_) => {}
        other => return Err(i.wrong_type_mut("stringp", other)),
    }
    i.apply(&args[2], vec![])
}
fn f_declare_function(i: &mut Interp, _args: crate::lisp::Argv) -> EvalResult {
    let _ = i;
    Ok(Value::Nil)
}
fn f_declare(i: &mut Interp, _args: crate::lisp::Argv) -> EvalResult {
    if i.dbg_flags & 0x80000 != 0 {
        eprintln!("[declare-call] args={:?}", _args.first());
    }
    let _ = i;
    Ok(Value::Nil)
}
fn f_eval_and_compile(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    let body = args.iter().next().cloned().unwrap_or(Value::Nil);
    i.eval_progn(&body)
}
fn f_eval_when_compile(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    // GNU: when the code is not being byte-compiled, `eval-when-compile'
    // evaluates its body like `progn' (the compile-time-only behavior only
    // applies inside the byte compiler, which remacs does not have).
    let body = args.iter().next().cloned().unwrap_or(Value::Nil);
    i.eval_progn(&body)
}
fn f_with_no_warnings(_i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    // GNU's byte-run subr evaluates BODY args and returns the last value.
    Ok(args.iter().last().cloned().unwrap_or(Value::Nil))
}
pub(crate) fn f_message(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    // GNU Fmessage: nil or "" just clears (returning the arg); anything
    // else goes through Fformat_message, whose errors propagate.
    match &args[0] {
        Value::Nil => return Ok(Value::Nil),
        Value::Str(s) if s.borrow().is_empty() => return Ok(args[0].clone()),
        _ => {}
    }
    let v = super::strfn::f_format_message(i, args)?;
    let msg = match &v {
        Value::Str(s) => s.borrow().clone(),
        other => i.princ_to_string(other),
    };
    i.message(&msg);
    Ok(v)
}
fn f_ding(i: &mut Interp, _args: crate::lisp::Argv) -> EvalResult {
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
            i.apply(&Value::Sym(handler), vec![timer])?;
        }
    }
    Ok(Value::Nil)
}

/// The next active ordinary timer bounds wait_reading_process_output's
/// select timeout, even when the caller requested an enormous delay.
pub(crate) fn next_timer_delay(i: &mut Interp) -> Option<f64> {
    let symbol = i.intern("timer-list");
    let timers = i.symbol_value(symbol).list_to_vec().ok()?;
    let now = super::misc::lisp_time_to_ns(i, &Value::Nil).ok()?;
    timers
        .into_iter()
        .filter_map(|timer| {
            let when = match timer {
                Value::Vec(value) => {
                    let value = value.borrow();
                    if value.len() < 9 || value[0].truthy() || value[5].is_nil() {
                        return None;
                    }
                    Value::list(vec![
                        value[1].clone(),
                        value[2].clone(),
                        value[3].clone(),
                        value[8].clone(),
                    ])
                }
                _ => return None,
            };
            let when = super::misc::lisp_time_to_ns(i, &when).ok()?;
            Some(when.saturating_sub(now).max(0) as f64 / 1_000_000_000.0)
        })
        .min_by(f64::total_cmp)
}

/// Sleep for SECS seconds, firing due timers during the wait like
/// GNU's `wait_reading_process_output'.
pub(crate) fn thread_wait_events(i: &mut Interp) -> Result<(), Flow> {
    crate::lisp::eval::threads::drain_main_events(i)?;
    timer_check(i)?;
    crate::lisp::process::poll_all(i)?;
    crate::lisp::builtins::filenotify::drain(i)?;
    crate::lisp::builtins::dbus::drain(i)?;
    Ok(())
}

fn sleep_firing_timers(i: &mut Interp, secs: f64) -> EvalResult {
    if secs <= 0.0 || secs.is_nan() {
        // GNU enters its wait/event pump only for a positive duration.
        return Ok(Value::Nil);
    }
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs_f64(secs.min(3600.0));
    if crate::lisp::eval::threads::sleep_worker(i, deadline)? {
        return Ok(Value::Nil);
    }
    loop {
        crate::lisp::eval::threads::yield_now(i)?;
        timer_check(i)?;
        // GNU's wait also drains subprocess output (filters/buffers/
        // sentinels) — sit-for/sleep-for both pump.
        crate::lisp::process::poll_all(i)?;
        // File-notification events queued by the kqueue reader thread.
        crate::lisp::builtins::filenotify::drain(i)?;
        // D-Bus messages queued on open bus connections.
        crate::lisp::builtins::dbus::drain(i)?;
        let rest = deadline.saturating_duration_since(std::time::Instant::now());
        if rest.is_zero() {
            break;
        }
        std::thread::sleep(rest.min(std::time::Duration::from_millis(20)));
    }
    timer_check(i)?;
    crate::lisp::process::poll_all(i)?;
    crate::lisp::builtins::filenotify::drain(i)?;
    crate::lisp::builtins::dbus::drain(i)?;
    Ok(Value::Nil)
}

fn f_sleep_for(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    // GNU checks seconds as a number and optional milliseconds as
    // a fixnum; nil milliseconds means zero.
    let secs = match &args[0] {
        Value::Int(n) => *n as f64,
        Value::Float(f) => **f,
        other => return Err(i.wrong_type_mut("numberp", other)),
    } + match args.get(1) {
        Some(Value::Int(n)) => *n as f64 / 1000.0,
        Some(v) if v.is_nil() => 0.0,
        Some(other) => return Err(i.wrong_type_mut("fixnump", other)),
        None => 0.0,
    };
    sleep_firing_timers(i, secs)
}
fn f_sit_for(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    // Batch: no input to wait for — GNU sleeps the full time, firing
    // timers.  Arg is CHECK_NUMBER'd the same as `sleep-for'.
    let secs = match args.get(0) {
        Some(Value::Int(n)) => *n as f64,
        Some(Value::Float(f)) => **f,
        Some(other) => return Err(i.wrong_type_mut("numberp", other)),
        None => 0.0,
    };
    sleep_firing_timers(i, secs)?;
    Ok(Value::t())
}
fn f_current_time(i: &mut Interp, _args: crate::lisp::Argv) -> EvalResult {
    let us = super::misc::lisp_time_to_us(i, &Value::Nil)?;
    // GNU `make_lisp_time': `current-time-list' nil renders as
    // (TICKS . HZ) with nanosecond ticks.
    if !super::misc::current_time_list(i) {
        return Ok(Value::cons(Value::Int(us * 1000), Value::Int(1_000_000_000)));
    }
    Ok(super::misc::us_to_lisp_time(us))
}
fn f_current_time_string(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    let secs = super::misc::lisp_time_seconds(i, &arg(&args, 0))?;
    let tm = super::misc::time_tm(i, secs, &arg(&args, 1))?;
    let wday = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"][tm.tm_wday.clamp(0, 6) as usize];
    let mon = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ][tm.tm_mon.clamp(0, 11) as usize];
    // GNU uses ctime's format: the day-of-month is space-padded
    // ("%e"), so single digits print as " 1" not "01".
    Ok(Value::string(format!(
        "{} {} {:2} {:02}:{:02}:{:02} {}",
        wday,
        mon,
        tm.tm_mday,
        tm.tm_hour,
        tm.tm_min,
        tm.tm_sec,
        tm.tm_year + 1900
    )))
}
/// `current-time-zone' — (OFFSET NAME) for TIME under ZONE.  GNU's
/// decode_time_zone: nil/`wall' = ambient local zone, t and
/// "UTC"/"t" and zero = Universal Time, nonzero integer = fixed seconds-east offset
/// (named "+HH[MM[SS]]"), other strings name a TZ spec, a cons uses
/// its car as the fixed offset (cadr as the name when a string).
/// Anything else signals "Invalid time zone specification".
fn f_current_time_zone(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    /// GNU's name for an integer zone: sign plus HH, then MM/SS only
    /// when non-zero ("+01", "+0130", "+010101").
    fn offset_name(off: i64) -> String {
        let sign = if off < 0 { '-' } else { '+' };
        let a = off.abs();
        let (h, m, s) = (a / 3600, (a % 3600) / 60, a % 60);
        if s != 0 {
            format!("{}{:02}{:02}{:02}", sign, h, m, s)
        } else if m != 0 {
            format!("{}{:02}{:02}", sign, h, m)
        } else {
            format!("{}{:02}", sign, h)
        }
    }
    let secs = super::misc::lisp_time_seconds(i, &arg(&args, 0))?;
    let t_sym = i.intern("t");
    let wall_sym = i.intern("wall");
    let (off, name) = match args.get(1) {
        // GNU tzlookup treats numeric zero like t, not a named +00 offset.
        Some(Value::Int(0)) => (0, "UTC".to_string()),
        Some(Value::Int(off)) => (*off as i64, offset_name(*off as i64)),
        Some(Value::Sym(s)) if *s == t_sym => (0, "UTC".to_string()),
        Some(Value::Sym(s)) if *s == wall_sym => {
            // `wall' = startup wall clock, unaffected by
            // `set-time-zone-rule' (GNU's `wall_tz').
            let tm = super::misc::local_tm(secs);
            (tm.tm_gmtoff, tm_zone_name(&tm))
        }
        Some(Value::Str(z)) => {
            let z = z.borrow().clone();
            if z == "UTC" || z == "t" {
                (0, "UTC".to_string())
            } else {
                let (tm, name) = super::misc::tz_local_tm(secs, &z);
                (tm.tm_gmtoff, name)
            }
        }
        Some(Value::Cons(c)) => {
            let (off, name) = {
                let b = c.borrow();
                let off = match &b.car {
                    Value::Int(n) => *n as i64,
                    _ => 0,
                };
                let name = match &b.cdr {
                    Value::Cons(c2) => match &c2.borrow().car {
                        Value::Str(s) => s.borrow().clone(),
                        _ => String::new(),
                    },
                    Value::Str(s) => s.borrow().clone(),
                    _ => String::new(),
                };
                (off, name)
            };
            (
                off,
                if name.is_empty() {
                    offset_name(off)
                } else {
                    name
                },
            )
        }
        Some(Value::Nil) | None => {
            // Under a tz_rule the Tm's tm_zone pointer dangles after
            // tz_local_tm's restore tzset — use the captured name.
            let (tm, name) = super::misc::interp_local_tm_named(i, secs);
            (tm.tm_gmtoff, name)
        }
        Some(v) => {
            return Err(i.signal_data(
                crate::lisp::obarray::sym::ERROR,
                vec![Value::string("Invalid time zone specification"), v.clone()],
            ));
        }
    };
    Ok(Value::list(vec![
        Value::Int(off as i128),
        Value::string(name),
    ]))
}

/// `tm_zone' as an owned string (empty when the field is null).
fn tm_zone_name(tm: &super::misc::Tm) -> String {
    if tm.tm_zone.is_null() {
        String::new()
    } else {
        unsafe { std::ffi::CStr::from_ptr(tm.tm_zone as *const i8) }
            .to_string_lossy()
            .into_owned()
    }
}
fn f_float_time(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    let value = args.get(0).unwrap_or(&Value::Nil);
    if let Value::Float(n) = value {
        if n.is_finite() {
            return Ok(Value::float(**n));
        }
    }
    let (ticks, hz) = super::misc::decode_big_ticks_hz(i, value)?;
    let ratio = num_rational::BigRational::new(ticks, hz);
    let value = num_traits::ToPrimitive::to_f64(&ratio).unwrap_or_else(|| {
        if num_traits::Signed::is_negative(&ratio) {
            f64::NEG_INFINITY
        } else {
            f64::INFINITY
        }
    });
    Ok(Value::float(value))
}

/// How `format-time-string' renders %Z for a given ZONE argument.
enum ZoneLabel {
    /// Fixed numeric offset — GNU prints a sign plus two-digit hour
    /// count (plus minutes when non-multiple of an hour).
    Offset(i64),
    /// Literal name ("UTC" for t, the TZ name for a named zone).
    Named(String),
    /// Use tm_zone from the C library breakdown.
    Libc,
}

fn f_format_time_string(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    // GNU decodes TIME first (lisp_time_argument before
    // CHECK_STRING): (format-time-string 42 'bad) reports "Invalid
    // time specification", not (stringp 42).  Work in picoseconds so
    // normalization retains subsecond values before %N takes nanoseconds.
    let t = match args.get(1) {
        Some(v) => super::misc::lisp_time_to_ps(i, v)?,
        None => super::misc::lisp_time_to_ps(i, &Value::Nil)?,
    };
    let fmt = match &args[0] {
        Value::Str(s) => s.borrow().clone(),
        other => return Err(i.wrong_type_mut("stringp", other)),
    };
    let secs = t.div_euclid(1_000_000_000_000) as i64;
    // sub-second digits as a 12-digit picosecond fraction.
    let frac12 = t.rem_euclid(1_000_000_000_000);
    // GNU (format-time-string FORMAT TIME ZONE): nil/`wall' = local,
    // integer = fixed offset, string = TZ spec, `t' is UTC; anything
    // else signals "Invalid time zone specification".
    let t_sym = i.intern("t");
    let wall_sym = i.intern("wall");
    let (tm, zlabel) = match args.get(2) {
        Some(Value::Int(0)) => (
            super::misc::gmt_tm(secs),
            ZoneLabel::Named("UTC".to_string()),
        ),
        Some(Value::Int(off)) => {
            let mut tm = super::misc::gmt_tm(secs + *off as i64);
            tm.tm_gmtoff = *off as i64;
            (tm, ZoneLabel::Offset(*off as i64))
        }
        Some(Value::Sym(s)) if *s == t_sym => (
            super::misc::gmt_tm(secs),
            ZoneLabel::Named("UTC".to_string()),
        ),
        Some(Value::Sym(s)) if *s == wall_sym => (
            // `wall' = startup wall clock; the interp's tz_rule is the
            // nil/default zone, not `wall'.
            super::misc::local_tm(secs),
            ZoneLabel::Libc,
        ),
        Some(Value::Str(z)) => {
            let z = z.borrow().clone();
            if z == "UTC" || z == "t" {
                let mut tm = super::misc::gmt_tm(secs);
                tm.tm_gmtoff = 0;
                (tm, ZoneLabel::Named("UTC".to_string()))
            } else {
                // GNU decode_time_zone: the TZ spec sets the zone and
                // %Z prints the resolved tm_zone abbreviation (e.g.
                // "UTC0" → "UTC"), not the spec text.  The name is
                // copied by tz_local_tm before TZ is restored.
                let (tm, name) = super::misc::tz_local_tm(secs, &z);
                (tm, ZoneLabel::Named(name))
            }
        }
        // GNU's decode_time_zone accepts `current-time-zone' output —
        // a (OFFSET NAME) cons — and uses the car as the fixed offset.
        Some(Value::Cons(c)) => {
            let (off, name) = {
                let b = c.borrow();
                let off = match &b.car {
                    Value::Int(n) => *n as i64,
                    _ => 0,
                };
                let name = match &b.cdr {
                    Value::Cons(c2) => match &c2.borrow().car {
                        Value::Str(s) => s.borrow().clone(),
                        _ => String::new(),
                    },
                    Value::Str(s) => s.borrow().clone(),
                    _ => String::new(),
                };
                (off, name)
            };
            let mut tm = super::misc::gmt_tm(secs + off);
            tm.tm_gmtoff = off;
            (
                tm,
                if name.is_empty() {
                    ZoneLabel::Offset(off)
                } else {
                    ZoneLabel::Named(name)
                },
            )
        }
        Some(Value::Nil) | None => {
            let (tm, name) = super::misc::interp_local_tm_named(i, secs);
            // Under a tz_rule tm.tm_zone dangles (restore tzset), so
            // %Z must use the name captured before the restore.
            let label = if i.tz_rule.is_some() {
                ZoneLabel::Named(name)
            } else {
                ZoneLabel::Libc
            };
            (tm, label)
        }
        Some(v) => {
            return Err(i.signal_data(
                crate::lisp::obarray::sym::ERROR,
                vec![Value::string("Invalid time zone specification"), v.clone()],
            ));
        }
    };
    // `system-time-locale' overrides LC_TIME for this call, falling
    // back to "C" for names the C library rejects (like GNU).
    let locale: Option<String> = i
        .intern_soft("system-time-locale")
        .filter(|id| i.bound_p(*id))
        .and_then(|id| match i.symbol_value(id) {
            Value::Str(s) => Some(s.borrow().clone()),
            _ => None,
        });
    // Own the zone name before any locale/TZ operation can invalidate libc's
    // tm_zone. All GNU modifiers are rendered in Rust, independently of libc.
    let zone = match zlabel {
        ZoneLabel::Named(name) => name,
        ZoneLabel::Offset(off) => {
            let sign = if off < 0 { '-' } else { '+' };
            let a = off.unsigned_abs();
            let (h, m, s) = (a / 3600, a / 60 % 60, a % 60);
            if s != 0 {
                format!("{sign}{h:02}{m:02}{s:02}")
            } else if m != 0 {
                format!("{sign}{h:02}{m:02}")
            } else {
                format!("{sign}{h:02}")
            }
        }
        ZoneLabel::Libc => {
            if tm.tm_zone.is_null() {
                String::new()
            } else {
                unsafe { std::ffi::CStr::from_ptr(tm.tm_zone as *const i8) }
                    .to_string_lossy()
                    .into_owned()
            }
        }
    };
    // tz_local_tm restores TZ before returning; its tm_zone can therefore
    // dangle. Native locale composites must see the same owned name as %Z.
    let czone = std::ffi::CString::new(zone.split('\0').next().unwrap_or("")).unwrap();
    let mut tm = tm;
    tm.tm_zone = czone.as_ptr().cast();
    let out = super::timefmt::format_time(
        &fmt,
        &tm,
        secs,
        (frac12 / 1000) as u32,
        &zone,
        &mut |spec| super::misc::strftime_spec(spec, &tm, locale.as_deref()),
    )
    .map_err(|()| i.error("Invalid time specification"))?;
    Ok(Value::string(out))
}

fn f_garbage_collect(i: &mut Interp, _args: crate::lisp::Argv) -> EvalResult {
    if !crate::lisp::memory::collect_interp(i) {
        return Ok(Value::Nil);
    }
    Ok(crate::lisp::memory::report(i))
}

fn f_memory_info(_i: &mut Interp, _args: crate::lisp::Argv) -> EvalResult {
    // GNU returns nil where the kernel gives no memory-info query
    // interface (macOS, and thus our batch probes).
    Ok(Value::Nil)
}
fn f_kill_emacs(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
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
fn f_recursive_edit(i: &mut Interp, _args: crate::lisp::Argv) -> EvalResult {
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
fn f_top_level(_i: &mut Interp, _args: crate::lisp::Argv) -> EvalResult {
    Err(Flow::Throw(Value::Sym(sym::TOP_LEVEL), Value::Nil))
}
fn f_exit_recursive_edit(i: &mut Interp, _args: crate::lisp::Argv) -> EvalResult {
    // GNU: `(throw 'exit nil)' when a command loop or minibuffer is
    // active, else user-error.
    if i.recursion_depth == 0 && i.minibuf_level <= 0 {
        let ue = i.intern("user-error");
        return Err(i.signal_data(ue, vec![Value::string("No recursive edit is in progress")]));
    }
    Err(Flow::Throw(Value::Sym(i.intern("exit")), Value::Nil))
}
fn f_abort_recursive_edit(i: &mut Interp, _args: crate::lisp::Argv) -> EvalResult {
    // GNU: `(throw 'exit t)' — the `exit' catch maps t to `quit'.
    if i.recursion_depth == 0 && i.minibuf_level <= 0 {
        let ue = i.intern("user-error");
        return Err(i.signal_data(ue, vec![Value::string("No recursive edit is in progress")]));
    }
    Err(Flow::Throw(Value::Sym(i.intern("exit")), Value::t()))
}
fn f_recursion_depth(i: &mut Interp, _args: crate::lisp::Argv) -> EvalResult {
    // GNU: command_loop_level + minibuf_level.
    Ok(Value::Int(
        (i.recursion_depth + i.minibuf_level.max(0) as usize) as i128,
    ))
}
fn f_emacs_pid(_i: &mut Interp, _args: crate::lisp::Argv) -> EvalResult {
    Ok(Value::Int(std::process::id() as i128))
}
fn f_system_name(_i: &mut Interp, _args: crate::lisp::Argv) -> EvalResult {
    Ok(Value::string(crate::buffer::our_host_name()))
}
fn f_emacs_version(_i: &mut Interp, _args: crate::lisp::Argv) -> EvalResult {
    Ok(Value::string(format!(
        "remacs {} (Emacs-compatible)",
        env!("CARGO_PKG_VERSION")
    )))
}
fn f_emacs_build_time(_i: &mut Interp, _args: crate::lisp::Argv) -> EvalResult {
    Ok(Value::Int(0))
}
fn f_noop(_i: &mut Interp, _args: crate::lisp::Argv) -> EvalResult {
    Ok(Value::Nil)
}

/// GNU minibuffer.el `set-message-functions': run the
/// `set-message-functions' hook list over MESSAGE — each function
/// sees the current message string and may replace it with its
/// return value — then return the (possibly rewritten) message.
fn f_set_message_functions(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    let mut msg = args[0].clone();
    let sym = i.intern("set-message-functions");
    let hooks = hook_fns(i, &Value::Sym(sym));
    for f in hooks {
        if matches!(msg, Value::Str(_)) {
            let ret = i.apply(&f, vec![msg.clone()])?;
            if ret.truthy() {
                msg = ret;
            }
        }
    }
    Ok(msg)
}
fn f_internal_make_closure(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    // (internal-make-interpreted-closure-function ARGS BODY ENV)
    let params = &args[0];
    let body_v = &args[1];
    let env_v = &args[2];
    let body = match body_v {
        Value::Cons(_) => body_v.list_to_vec().unwrap_or_default(),
        _ => vec![body_v.clone()],
    };
    let env = crate::lisp::eval::lexenv_from_value(env_v);
    let mut lam = i.parse_lambda(params, &body, None)?;
    lam.env = env;
    Ok(Value::Lambda(std::rc::Rc::new(lam)).tracked())
}

fn f_macroexp_parse_body(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
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

fn f_byte_code_function_p(_i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    // `#[...]' byte-code literals (string code slot) answer t;
    // interpreted closures (cons code slot), lambdas AND native subrs
    // answer nil (GNU's nativecomp build: subrp ≠ byte-code-function-p).
    Ok(Value::from_bool(
        matches!(&args[0], Value::Lambda(l) if l.is_bytecode()),
    ))
}
fn f_compiled_function_p(_i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    Ok(Value::from_bool(match &args[0] {
        Value::Subr(_) => true,
        // GNU nativecomp reports every dumped def as compiled; our
        // twins are `compiled' interpreted lambdas.  Macros don't
        // count (GNU's `compiled-function-p' is nil on them too —
        // `subrp' excludes them, but plain lambdas don't reach here).
        Value::Lambda(l) => (l.compiled || l.is_bytecode()) && !l.is_macro,
        _ => false,
    }))
}
fn f_interactive_p(_i: &mut Interp, _args: crate::lisp::Argv) -> EvalResult {
    Ok(Value::Nil)
}
/// GNU validates the same three payload fields at both byte-code entry
/// points, with a different error message for the object constructor.
pub(crate) fn valid_byte_code_fields(code: &Value, constants: &Value, depth: &Value) -> bool {
    matches!(code, Value::Str(_))
        && matches!(constants, Value::Vec(_))
        && depth.fixnump()
        && depth.int().is_some_and(|depth| depth >= 0)
}

fn f_byte_code(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    // GNU `Fbyte_code': run a raw (bytecode, constants, depth) triple
    // as a zero-argument byte-code object; malformed operands signal
    // (error "Invalid byte-code").
    if !valid_byte_code_fields(&args[0], &args[1], &args[2]) {
        return Err(i.error("Invalid byte-code"));
    }
    // GNU accepts old multibyte byte-code strings at this entry point,
    // converting their storage bytes before constructing a compiled object.
    let code = if super::strfn::string_multibyte_p(i, &args[0]) {
        super::strfn::f_string_as_unibyte(i, vec![args[0].clone()].into())?
    } else {
        args[0].clone()
    };
    crate::lisp::bytecode::exec_raw(i, &code, &args[1])
}

fn f_make_byte_code(_i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    // GNU `make-byte-code': pack ARGLIST BYTE-CODE CONSTANTS DEPTH
    // (and optional doc/interactive slots) into a `#[...]' object.
    // The Lambda keeps the raw slots in `bc_items' so `aref'/`arrayp'
    // see GNU's element view and `bytecode::exec' can run it.
    let argdesc = args[0].clone();
    // GNU's `validate_byte_code' (comp.c): BYTE-CODE must be unibyte,
    // CONSTANTS a vector, DEPTH a non-negative fixnum — any violation
    // signals `(error "Invalid byte-code object")'.  ARGLIST and the
    // trailing docstring/interactive slots are unchecked (argdesc `5'
    // produces the `#[5 ...]' object GNU also makes).
    if !valid_byte_code_fields(&args[1], &args[2], &args[3])
        || super::strfn::string_multibyte_p(_i, &args[1])
    {
        return Err(_i.error("Invalid byte-code object"));
    }
    let doc = args.get(4).and_then(|v| match v {
        Value::Str(s) => Some(s.borrow().clone()),
        _ => None,
    });
    let interactive = args.get(5).map(|spec| {
        // GNU's `interactive-form'/callint read byte-code slot 5 as the
        // bare spec inside an implicit `(interactive ...)'; every other
        // use of `Lambda.interactive' expects that wrapper.
        Value::list(vec![Value::Sym(_i.intern("interactive")), spec.clone()])
    });
    Ok(Value::Lambda(std::rc::Rc::new(crate::lisp::value::Lambda {
        is_macro: false,
        required: Vec::new(),
        optional: Vec::new(),
        rest: None,
        body: Vec::new(),
        env: None,
        doc,
        interactive,
        name: None,
        bad_arglist: false,
        arglist: Some(argdesc),
        plain: false,
        dumped_doc: false,
        compiled: false,
        advice_link: None,
        bc_items: Some(crate::lisp::Value::vector_ref(args.as_slice().to_vec())),
        bc_closure: false,
        bc_prog: std::cell::RefCell::new(None),
        doc_value: None,
        env_value: None,
    }))
    .tracked())
}
fn f_subr_native_lambda_list(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    // Emacs 31: t for primitives, wrong-type-argument otherwise.
    match &args[0] {
        Value::Subr(_) => Ok(Value::t()),
        other => Err(i.wrong_type_mut("subrp", other)),
    }
}
fn f_declare_functionp(_i: &mut Interp, _args: crate::lisp::Argv) -> EvalResult {
    Ok(Value::Nil)
}

/// `internal--track-mouse` — GNU enters mouse-tracking mode then calls
/// BODYFN; tracking is a no-op without a window system, so just apply.
fn f_internal_track_mouse(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    i.apply(&a[0], vec![])
}

/// `save-mark-and-excursion--save` → (MARKER . ACTIVE) — `(nil)' when
/// the mark isn't set, like GNU.
fn f_smae_save(i: &mut Interp, _a: crate::lisp::Argv) -> EvalResult {
    let buf = i.current_buffer;
    let Some(b) = i.buffers.get(buf) else {
        return Ok(Value::cons(Value::Nil, Value::Nil));
    };
    let mark = {
        let mark = b.borrow_mut().mark_marker();
        let mark = mark.borrow();
        mark.buffer.map(|buffer| (buffer, mark.position))
    };
    let m = match mark {
        Some((buffer, pos)) => {
            let mk = crate::lisp::memory::allocate(
                Marker {
                    buffer: Some(buffer),
                    position: pos,
                    insertion_type: false,
                },
                crate::lisp::Value::Marker,
            );
            if let Some(buffer) = i.buffers.get(buffer) {
                buffer.borrow_mut().register_marker(&mk);
            }
            Value::Marker(mk)
        }
        None => Value::Nil,
    };
    let active = i.intern("mark-active");
    Ok(Value::cons(m, i.symbol_value(active)))
}

/// `save-mark-and-excursion--restore` — restore mark position (when the
/// saved cons's car is a marker) and activation flag.
fn f_smae_restore(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU simple.el: `(car SAVED)'/`(cdr SAVED)' force `listp' — a
    // non-list, non-nil arg signals wrong-type-argument.
    let saved = &a[0];
    if !matches!(saved, Value::Cons(_) | Value::Nil) {
        return Err(i.wrong_type_mut("listp", saved));
    }
    let (saved_mark, saved_active) = match saved {
        Value::Cons(c) => {
            let b = c.borrow();
            (b.car.clone(), b.cdr.clone())
        }
        _ => (Value::Nil, Value::Nil),
    };
    let buf = i.current_buffer;
    let Some(b) = i.buffers.get(buf) else {
        return Ok(Value::Nil);
    };
    // GNU: (omark (marker-position (mark-marker))) read before mutating.
    let (omark, markm) = {
        let mut bb = b.borrow_mut();
        let m = bb.mark_marker();
        let pos = {
            let mm = m.borrow();
            mm.buffer.is_some().then(|| mm.position)
        };
        (pos, m)
    };
    let nmark = if saved_mark.is_nil() {
        // (set-marker (mark-marker) nil): detach the mark marker.
        markm.borrow_mut().buffer = None;
        None
    } else {
        let mm = match &saved_mark {
            Value::Marker(m) => m.clone(),
            other => return Err(i.wrong_type_mut("markerp", other)),
        };
        // (setf nmark (marker-position saved-mark)) then
        // (set-marker (mark-marker) nmark): a detached saved marker
        // yields nil → the mark marker is detached too.
        let np = mm.borrow().buffer.map(|_| mm.borrow().position);
        match np {
            Some(p) => {
                let len = b.borrow().text.len();
                b.borrow_mut().set_mark(p.min(len));
            }
            None => markm.borrow_mut().buffer = None,
        }
        // (set-marker saved-mark nil): saved marker is detached.
        mm.borrow_mut().buffer = None;
        np
    };
    // Read activation after moving the mark, then assign through normal
    // variable binding/watch semantics, preserving a non-boolean value.
    let ma = i.intern("mark-active");
    let cur_active = i.symbol_value(ma);
    i.set_symbol(ma, saved_active.clone())?;
    // GNU runs activate-mark-hook when the restored flag is non-nil and
    // the mark moved or was inactive; deactivate-mark-hook when the flag
    // went nil from non-nil.
    let rh = i.intern("run-hooks");
    if saved_active.truthy() {
        if !cur_active.truthy() || omark != nmark {
            let h = Value::Sym(i.intern("activate-mark-hook"));
            i.apply(&Value::Sym(rh), vec![h])?;
        }
    } else if cur_active.truthy() {
        let h = Value::Sym(i.intern("deactivate-mark-hook"));
        i.apply(&Value::Sym(rh), vec![h])?;
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
fn advice_entry_matches(i: &Interp, fun: &Value, name: &Value, sel: &Value) -> Result<bool, Flow> {
    Ok(super::equal_values(i, fun, sel)? || (!name.is_nil() && super::equal_values(i, name, sel)?))
}

fn f_advice_add(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
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
        // Like GNU's advice--tweak, finish the replacement chain
        // before publishing it: a comparison error leaves it intact.
        let mut list = Vec::with_capacity(i.advices[pos].1.len());
        for entry @ (_, f, n) in &i.advices[pos].1 {
            if !advice_entry_matches(i, f, n, sel)? {
                list.push(entry.clone());
            }
        }
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
    let mut list = Vec::with_capacity(i.advices[pos].1.len() + 1);
    // GNU's advice--add-function replaces an existing entry by :name
    // when PROPS supplies one; otherwise it matches on the function
    // itself.  Same-function-different-name pieces coexist.
    for existing @ (_, f, n) in &i.advices[pos].1 {
        let matches = if !name.is_nil() {
            super::equal_values(i, n, &name)?
        } else {
            super::equal_values(i, f, &fun)?
        };
        if !matches {
            list.push(existing.clone());
        }
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

fn f_advice_remove(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let sym = want_sym(i, &a[0])?;
    advice_retain(i, sym, &a[1])?;
    Ok(Value::Nil)
}

/// `cl--add-function` — (HOW PLACE FUNCTION PROPS) where PLACE is a
/// normalized `(KIND . ARGS)' list produced by the `add-function'
/// macro: (function SYM), (var SYM), or (get SYM PROP).
fn f_add_function(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
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
fn f_remove_function(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
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

fn f_advice_member_p(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU's advice-member-p requires a symbol for FUNCTION-DEF
    // (it goes through the advice--symbol-function machinery).
    if i.sym_id(&a[1]).is_none() {
        return Err(i.wrong_type_mut("symbolp", &a[1]));
    }
    let mut cur = match &a[1] {
        Value::Sym(s) => i.symbol_function(*s),
        v => v.clone(),
    };
    loop {
        match i.advice_link_entry(&cur) {
            Some((_, f, next, n)) => {
                // GNU advice--member-p checks the name before the
                // function; advice--remove-function uses the reverse order.
                if (!n.is_nil() && super::equal_values(i, &n, &a[0])?)
                    || super::equal_values(i, &f, &a[0])?
                {
                    return Ok(cur);
                }
                cur = next;
            }
            None => return Ok(Value::Nil),
        }
    }
}

fn f_advice_function_mapc(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
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
fn f_advice_link(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
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
fn f_advice_apply_link(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
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
