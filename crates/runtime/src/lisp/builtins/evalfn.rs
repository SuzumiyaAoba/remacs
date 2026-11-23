//! Evaluation-related subrs: eval, apply, funcall, signal, error, throw,
//! featurep, run-hooks, etc.

use super::{S, arg, want_list, want_sym};
use crate::lisp::SymId;
use crate::lisp::error::{EvalResult, Flow};
use crate::lisp::obarray::sym;
use crate::lisp::value::{Marker, Subr, Value};
use crate::lisp::{Interp, eval::SitForOptions};
use rustc_hash::FxHashSet;
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
    S!("get-internal-run-time", 0, 0, f_get_internal_run_time, ""),
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
        Value::Cons(c) => Ok(crate::lisp::eval::cons_parts(i, c)?.0),
        Value::Nil => Ok(Value::Nil),
        other => Err(i.wrong_type_mut("listp", other)),
    }
}

pub(crate) fn f_eval(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
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
        let s = Value::Sym(i.q("apply"));
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

fn filename_text_arg(
    i: &mut Interp,
    value: &Value,
) -> Result<crate::lisp::text::EmacsText, Flow> {
    crate::lisp::load::filename_text(i, value)?
        .ok_or_else(|| i.wrong_type_mut("stringp", value))
}

fn f_funcall(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    if args.is_empty() {
        let s = Value::Sym(i.q("funcall"));
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
    let symbol_head = match &args[0] {
        Value::Cons(c) => i
            .sym_id(&crate::lisp::eval::cons_parts(i, c)?.0)
            .is_some(),
        _ => false,
    };
    if symbol_head {
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
            let (car, cdr) = crate::lisp::eval::cons_parts(i, c)?;
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
                let mut env_seen = FxHashSet::default();
                loop {
                    match tail.clone() {
                        Value::Nil => break,
                        Value::Cons(cc) => {
                            if !env_seen.insert(Rc::as_ptr(&cc) as usize) {
                                return Err(i.signal_data(
                                    sym::CIRCULAR_LIST,
                                    vec![tail.clone()],
                                ));
                            }
                            let (a, d) = crate::lisp::eval::cons_parts(i, &cc)?;
                            if let Value::Cons(e) = &a {
                                let (ek, ev) = crate::lisp::eval::cons_parts(i, e)?;
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
                        // Like GNU's `assq', report the original environment
                        // if lookup reaches an improper tail. Keep this lazy:
                        // an earlier match does not inspect the remaining list.
                        _ => return Err(i.wrong_type_mut("listp", env)),
                    }
                }
            }
            if let Some(id) = i.sym_id(&car) {
                // GNU macroexp.el: `autoload-do-load' resolves an
                // autoload cell (macro-only); then aliases to macros
                // re-expanda, and `(macro . FN)' cells call FN.
                let compiled_input = i.compiled_macro_expansion || i.boot_loading;
                let def = if compiled_input {
                    i.form_function(id)
                } else {
                    i.symbol_function(id)
                };
                let def = autoload_do_load(i, def, true)?;
                let def = if compiled_input {
                    i.unadvised_base(&def)
                } else {
                    def
                };
                // (and (symbolp def) (macrop def)) → (cons def cdr):
                // let the outer macroexpand re-enter on the alias.
                if let Value::Sym(did) = def {
                    if did != sym::UNBOUND {
                        let f = if compiled_input {
                            i.form_function(did)
                        } else {
                            i.symbol_function(did)
                        };
                        let f = if compiled_input {
                            i.unadvised_base(&f)
                        } else {
                            f
                        };
                        let is_mac = match &f {
                            Value::Lambda(l) => l.is_macro,
                            Value::Cons(cc) => {
                                let car = crate::lisp::eval::cons_parts(i, cc)?.0;
                                i.sym_is(&car, sym::MACRO)
                            }
                            _ => false,
                        };
                        if is_mac {
                            return Ok(Value::cons(Value::Sym(did), cdr));
                        }
                    }
                    return Ok(form.clone());
                }
                match &def {
                    Value::Cons(cc) => {
                        let (car, fn_) = crate::lisp::eval::cons_parts(i, cc)?;
                        if i.sym_is(&car, sym::MACRO) {
                            let argv = want_list(i, &cdr)?;
                            return i.apply(&fn_, argv);
                        }
                        return Ok(form.clone());
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
    let id = i.q("macroexpand-all-environment");
    let mark = i.specbind_depth();
    let result = (|| {
        i.specbind(id, env)?;
        if scope_dynvars {
            let dynvars = i.q("macroexp--dynvars");
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
fn is_declare_form(i: &Interp, form: &Value) -> Result<bool, Flow> {
    if let Value::Cons(c) = form {
        let car = crate::lisp::eval::cons_parts(i, c)?.0;
        if let Some(id) = i.sym_id(&car) {
            return Ok(i.q_soft("declare") == Some(id));
        }
    }
    Ok(false)
}

/// GNU `macroexp--expand-all' applies the head symbol's
/// `compiler-macro' property as (apply HANDLER FORM (cdr FORM)),
/// following `symbol-function' aliases like `cl-compiler-macroexpand'.
fn compiler_macro_handler(i: &mut Interp, form: &Value) -> Result<Option<Value>, Flow> {
    let car = match form {
        Value::Cons(c) => crate::lisp::eval::cons_parts(i, c)?.0,
        _ => return Ok(None),
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
        return Ok(None);
    }
    let mut func = crate::lisp::builtins::misc::unpos(i, &car);
    let cmacro = i.q("compiler-macro");
    let mut seen = FxHashSet::default();
    while let Value::Sym(id) = func {
        if !seen.insert(id) {
            break;
        }
        let h = i.get_prop(id, cmacro);
        if !h.is_nil() {
            return Ok(Some(h));
        }
        let sf = i.symbol_function(id);
        match sf {
            Value::Sym(next) if next != sym::UNBOUND => func = Value::Sym(next),
            _ => break,
        }
    }
    Ok(None)
}

/// None means the handler returned FORM unchanged; Some(nil) is a valid
/// expansion.  Retain HANDLER for the retry after expanding arguments.
fn expand_compiler_macro(
    i: &mut Interp,
    handler: &Value,
    form: &Value,
) -> Result<Option<Value>, Flow> {
    let helper = i.q("macroexp--compiler-macro");
    let expanded = if i.fbound_p(helper) {
        // The GNU helper honors inhibition/debug settings, warns on
        // optimization failures and preserves nonlocal exits.
        i.apply(&Value::Sym(helper), vec![handler.clone(), form.clone()])?
    } else {
        // Early loadup, before macroexp.el's wrapper is defined.
        if i.q_soft("macroexp-inhibit-compiler-macros")
            .is_some_and(|id| i.bound_p(id) && i.symbol_value(id).truthy())
        {
            return Ok(None);
        }
        let rest = match form {
            Value::Cons(c) => crate::lisp::eval::cons_parts(i, c)?.1,
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
fn is_keyword_id(i: &Interp, sid: SymId) -> bool {
    !i.obarray.symbol(sid).uninterned && i.symbol_name(sid).starts_with(':')
}

fn plain_setq_var(i: &Interp, v: &Value) -> bool {
    // `symbols-with-pos-enabled' objects are settable symbols: GNU's
    // `(symbolp var)' accepts them, so unwrap via `sym_id'.
    match i.sym_id(v) {
        Some(sid) if sid != sym::NIL && sid != sym::T => !is_keyword_id(i, sid),
        _ => false,
    }
}

/// Whether symbol ID's function cell holds a macro — `macrop' on the
/// symbol, following its cell (including macro-autoload cells).
fn sym_macrop(i: &mut Interp, id: SymId) -> Result<bool, Flow> {
    match i.symbol_function(id) {
        Value::Lambda(l) => Ok(l.is_macro),
        Value::Cons(c) => {
            let (car, _) = crate::lisp::eval::cons_parts(i, &c)?;
            if i.sym_is(&car, sym::MACRO) {
                return Ok(true);
            }
            let auto_id = i.q("autoload");
            if i.sym_is(&car, auto_id) {
                let form = Value::Cons(c);
                return Ok(macroexpand_list_to_vec(i, &form)?
                    .and_then(|v| v.get(4).map(|x| x.truthy()))
                    .unwrap_or(false));
            }
            Ok(false)
        }
        _ => Ok(false),
    }
}

/// Whether VAR is dynamically bound in `macroexp--expand-all'
/// terms — `special-variable-p', a `macroexp--dynvars' member, or a
/// `byte-compile-bound-variables' member.  Such a formal cannot be
/// alpha-converted to a `let' binding (GNU's dynboundarg punt).
fn macroexp_dynbound_p(i: &mut Interp, var: SymId) -> Result<bool, Flow> {
    if i.obarray.symbol(var).special {
        return Ok(true);
    }
    for name in ["macroexp--dynvars", "byte-compile-bound-variables"] {
        let vid = i.q(name);
        let v = i.symbol_value(vid);
        if let Some(items) = macroexpand_list_to_vec(i, &v)? {
            if items.iter().any(|x| i.sym_is(x, var)) {
                return Ok(true);
            }
        }
    }
    Ok(false)
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
    let Some(litems) = macroexpand_list_to_vec(i, lam)? else {
        return Ok(None);
    };
    if litems.len() < 2 {
        return Ok(None);
    }
    let Some(formals) = macroexpand_list_to_vec(i, &litems[1])? else {
        return Ok(None);
    };
    // `macroexp-parse-body': skip leading docstring/declare/interactive.
    let mut body_start = 2;
    while body_start < litems.len() {
        let form = &litems[body_start];
        if matches!(form, Value::Str(_)) || is_declare_form(i, form)? {
            body_start += 1;
            continue;
        }
        if let Value::Cons(c) = form {
            let car = crate::lisp::eval::cons_parts(i, c)?.0;
            if i.sym_is(&car, sym::INTERACTIVE) {
                body_start += 1;
                continue;
            }
        }
        break;
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
            if restp {
                return Err(i.error("&optional found after &rest in anonymous lambda"));
            }
            if k + 1 == formals.len() {
                return Err(i.error("Nothing after &optional in anonymous lambda"));
            }
            optionalp = true;
            continue;
        }
        if sid == sym::REST {
            if k + 1 == formals.len() {
                return Err(i.error("Nothing after &rest in anonymous lambda"));
            }
            if k + 2 != formals.len() {
                return Err(i.error("Multiple vars after &rest in anonymous lambda"));
            }
            restp = true;
            continue;
        }
        // Other `&'-keywords (`&key' etc.) are not unfoldable.
        if i.symbol_name(sid).starts_with('&') {
            return Ok(None);
        }
        if macroexp_dynbound_p(i, sid)? {
            return Ok(None);
        }
        if restp {
            // The `&rest' formal soaks up the remaining actuals.
            let tail = if rest.is_empty() {
                Value::Nil
            } else {
                let mut l = vec![Value::Sym(i.q("list"))];
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
        if body.is_empty() {
            return Ok(Some(Value::Nil));
        }
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
    // Expansion is observable: two occurrences can share the same cons,
    // but GNU still invokes their macro/compiler-macro expanders twice.
    // Reuse by cons identity skips those effects and yields the wrong form.
    macroexpand_all_inner(i, form)
}

/// Recursive `macroexpand-all' walks count against the Lisp depth limit.
/// The outer entry is already counted by eval/funcall; its Rust recursion
/// must add the frames GNU's recursive Lisp helpers would have consumed.
fn macroexpand_all_recur(i: &mut Interp, form: &Value) -> EvalResult {
    i.with_lisp_eval_depth(false, |i| macroexpand_all_inner(i, form))
}

/// Convert a proper list without panicking if a Rust caller holds a
/// mutable borrow of one of its cons cells. Malformed/circular lists retain
/// the callers' existing fallback behavior; borrow conflicts become Lisp
/// errors through `cons_parts`.
fn macroexpand_list_to_vec(i: &mut Interp, list: &Value) -> Result<Option<Vec<Value>>, Flow> {
    let mut items = Vec::new();
    let mut cur = list.clone();
    let mut seen = FxHashSet::default();
    let mut until_quit = 4096usize;
    loop {
        match cur {
            Value::Nil => return Ok(Some(items)),
            Value::Cons(cell) => {
                if !seen.insert(Rc::as_ptr(&cell) as usize) {
                    return Ok(None);
                }
                let (car, cdr) = crate::lisp::eval::cons_parts(i, &cell)?;
                items.push(car);
                cur = cdr;
                until_quit -= 1;
                if until_quit == 0 {
                    i.maybe_quit()?;
                    until_quit = 4096;
                }
            }
            _ => return Ok(None),
        }
    }
}

/// Read and expand each cons cell as it is reached.  In particular, do not
/// snapshot a scoped body before expanding its first form: a macro expander
/// can mutate a later body cell, and GNU observes that mutation when it gets
/// to the cell.  Preserve the original spine when no element was expanded.
fn macroexpand_live_spine(
    i: &mut Interp,
    list: &Value,
    mut expand: impl FnMut(&mut Interp, &Value) -> EvalResult,
) -> EvalResult {
    let mut cursor = list.clone();
    let mut cars = Vec::new();
    let mut seen = FxHashSet::default();
    let mut unchanged = true;
    let mut until_quit = 4096usize;
    let tail = loop {
        match cursor {
            Value::Nil => break Value::Nil,
            Value::Cons(cell) => {
                if !seen.insert(Rc::as_ptr(&cell) as usize) {
                    // Keep the existing fallback for circular input.
                    return Ok(list.clone());
                }
                let original = crate::lisp::eval::cons_parts(i, &cell)?.0;
                let expanded = expand(i, &original)?;
                if !crate::lisp::builtins::eq_values_with_interp(i, &original, &expanded) {
                    unchanged = false;
                }
                cars.push(expanded);
                // Read the cdr after expansion so mutations to the live
                // spine are visible to the next iteration.
                cursor = crate::lisp::eval::cons_parts(i, &cell)?.1;
                until_quit -= 1;
                if until_quit == 0 {
                    i.maybe_quit()?;
                    until_quit = 4096;
                }
            }
            other => break other,
        }
    };
    if unchanged {
        Ok(list.clone())
    } else {
        Ok(cars
            .into_iter()
            .rev()
            .fold(tail, |tail, car| Value::cons(car, tail)))
    }
}

/// Keep ORIGINAL when REBUILT has the same current elements by `eq'.  The
/// comparison is deliberately made after recursive expansion, since a macro
/// may have changed a later cell of ORIGINAL in place.
fn macroexpand_preserve_spine(i: &mut Interp, original: &Value, rebuilt: Value) -> EvalResult {
    if crate::lisp::builtins::eq_values_with_interp(i, original, &rebuilt) {
        return Ok(original.clone());
    }
    let Some(original_items) = macroexpand_list_to_vec(i, original)? else {
        return Ok(rebuilt);
    };
    let Some(rebuilt_items) = macroexpand_list_to_vec(i, &rebuilt)? else {
        return Ok(rebuilt);
    };
    if original_items.len() == rebuilt_items.len()
        && original_items
            .iter()
            .zip(&rebuilt_items)
            .all(|(a, b)| crate::lisp::builtins::eq_values_with_interp(i, a, b))
    {
        Ok(original.clone())
    } else {
        Ok(rebuilt)
    }
}

fn macroexpand_prepend_preserving(
    i: &mut Interp,
    original: &Value,
    mut prefix: Vec<Value>,
    tail: Value,
) -> EvalResult {
    let rebuilt = prefix
        .drain(..)
        .rev()
        .fold(tail, |tail, item| Value::cons(item, tail));
    macroexpand_preserve_spine(i, original, rebuilt)
}

fn macroexpand_list_tail(i: &mut Interp, list: &Value, count: usize) -> Result<Value, Flow> {
    let mut tail = list.clone();
    for _ in 0..count {
        match tail {
            Value::Cons(ref cell) => tail = crate::lisp::eval::cons_parts(i, cell)?.1,
            _ => break,
        }
    }
    Ok(tail)
}

fn macroexpand_live_filtered_body(
    i: &mut Interp,
    body: &Value,
    env_id: SymId,
    env: &Value,
    sm: bool,
    bound: &[SymId],
    keep_declarations: bool,
) -> EvalResult {
    let mut before_code = true;
    macroexpand_live_spine(i, body, |i, form| {
        let metadata =
            matches!(form, Value::Str(_)) || (keep_declarations && is_declare_form(i, form)?);
        if before_code && metadata {
            Ok(form.clone())
        } else {
            before_code = false;
            expand_body_filtered(i, env_id, env, sm, bound, form)
        }
    })
}

/// Expand a general call form while walking its live cons spine.  Reading
/// each car immediately before expansion lets an earlier macro expander's
/// edits to later cells affect the result.  If every car is unchanged, keep
/// the original cons cells instead of rebuilding an equal list.
fn macroexpand_live_form_spine(
    i: &mut Interp,
    form: &Value,
    compiler_macro: Option<Value>,
) -> EvalResult {
    let mut first = true;
    let result = macroexpand_live_spine(i, form, |i, original| {
        if first {
            first = false;
            Ok(original.clone())
        } else {
            macroexpand_all_recur(i, original)
        }
    })?;
    if let Some(handler) = &compiler_macro {
        if super::equal_values(i, form, &result)? {
            return Ok(form.clone());
        }
        if let Some(expanded) = expand_compiler_macro(i, handler, &result)? {
            return macroexpand_all_recur(i, &expanded);
        }
    }
    Ok(result)
}

/// Whether COND's TEST is known to be non-nil without evaluating it.
/// GNU `macroexp--expand-all' uses this check before expanding any clause,
/// so a macro that later expands to a constant does not hide warnings about
/// unreachable clauses.
fn macroexpand_cond_default_test(i: &mut Interp, test: &Value) -> Result<bool, Flow> {
    match test {
        Value::Nil => Ok(false),
        Value::Sym(id) => Ok(*id == sym::T || is_keyword_id(i, *id)),
        Value::Cons(cell) => {
            let (head, tail) = crate::lisp::eval::cons_parts(i, cell)?;
            if !i.sym_is(&head, sym::QUOTE) && !i.sym_is(&head, sym::FUNCTION) {
                return Ok(false);
            }
            let value = match tail {
                Value::Cons(cell) => crate::lisp::eval::cons_parts(i, &cell)?.0,
                _ => Value::Nil,
            };
            Ok(value.truthy())
        }
        _ => Ok(true),
    }
}

/// Whether CLAUSE has a test GNU recognizes as an unconditional `cond'
/// default.  Non-list clauses are ignored by `macroexp--all-clauses'.
fn macroexpand_cond_default_clause(i: &mut Interp, clause: &Value) -> Result<bool, Flow> {
    let Value::Cons(cell) = clause else {
        return Ok(false);
    };
    let test = crate::lisp::eval::cons_parts(i, cell)?.0;
    macroexpand_cond_default_test(i, &test)
}

/// Look up SYM_ID in ENV's `(:cl-symbol-macros . BINDINGS)' entry.
/// BINDINGS are `(SYM EXP)' pairs (GNU `cl-symbol-macrolet'), where
/// `cl--sm-macroexpand-1' alpha-renaming also pushes `(VAR . NVAR)'
/// dotted pairs — both shapes handled by taking `car' when the value
/// is a cons.
fn sm_lookup(i: &mut Interp, env: &Value, sym_id: SymId) -> Result<Option<Value>, Flow> {
    let kw = i.q(":cl-symbol-macros");
    let mut tail = env.clone();
    let mut env_cycle = crate::lisp::builtins::listfn::ListCycle::new(env);
    loop {
        match tail {
            Value::Cons(c) => {
                let (a, d) = crate::lisp::eval::cons_parts(i, &c)?;
                if let Value::Cons(e) = &a {
                    let (ek, ev) = crate::lisp::eval::cons_parts(i, e)?;
                    if i.sym_id(&ek) == Some(kw) {
                        // Found the :cl-symbol-macros entry; scan bindings.
                        let mut bt = ev;
                        let mut binding_cycle = crate::lisp::builtins::listfn::ListCycle::new(&bt);
                        loop {
                            match bt {
                                Value::Cons(bc) => {
                                    let (ba, bd) = crate::lisp::eval::cons_parts(i, &bc)?;
                                    if let Value::Cons(binding) = &ba {
                                        let (bs, bv) =
                                            crate::lisp::eval::cons_parts(i, binding)?;
                                        if i.sym_id(&bs) == Some(sym_id) {
                                            return Ok(Some(match bv {
                                                Value::Cons(vc) => {
                                                    crate::lisp::eval::cons_parts(i, &vc)?.0
                                                }
                                                other => other,
                                            }));
                                        }
                                    }
                                    bt = bd;
                                    binding_cycle.step(i, &bt)?;
                                }
                                _ => break,
                            }
                        }
                        return Ok(None);
                    }
                }
                tail = d;
                env_cycle.step(i, &tail)?;
            }
            _ => return Ok(None),
        }
    }
}

/// Remove BINDINGS whose car is any of `shadowed` from the env's
/// `:cl-symbol-macros' entry; used for let/lambda/condition-case
/// binder shadowing (GNU `cl--sm-macroexpand-1' alpha-renames;
/// removing the entry is equivalent for the shadowed scope).
fn sm_filter_env(i: &mut Interp, env: &Value, shadowed: &[SymId]) -> Result<Value, Flow> {
    if shadowed.is_empty() {
        return Ok(env.clone());
    }
    let kw = i.q(":cl-symbol-macros");
    let mut out = Value::Nil;
    let mut tail_cell: Option<Value> = None;
    let mut cur = env.clone();
    let mut env_cycle = crate::lisp::builtins::listfn::ListCycle::new(env);
    loop {
        match cur {
            Value::Cons(c) => {
                let (a, d) = crate::lisp::eval::cons_parts(i, &c)?;
                let mut keep = a.clone();
                if let Value::Cons(e) = &a {
                    let (ek, ev) = crate::lisp::eval::cons_parts(i, e)?;
                    if i.sym_id(&ek) == Some(kw) {
                        // Rebuild binding list without shadowed names.
                        let mut newbinds = Vec::new();
                        let mut bt = ev;
                        let mut binding_cycle = crate::lisp::builtins::listfn::ListCycle::new(&bt);
                        loop {
                            match bt {
                                Value::Cons(bc) => {
                                    let (ba, bd) = crate::lisp::eval::cons_parts(i, &bc)?;
                                    let keep_b = match &ba {
                                        Value::Cons(binding) => {
                                            let bs = crate::lisp::eval::cons_parts(i, binding)?.0;
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
                                    binding_cycle.step(i, &bt)?;
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
                env_cycle.step(i, &cur)?;
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
    Ok(out)
}

/// True when ENV carries a `:cl-symbol-macros' binding list.
fn sm_active(i: &mut Interp, env: &Value) -> Result<bool, Flow> {
    let kw = i.q(":cl-symbol-macros");
    let mut tail = env.clone();
    let mut cycle = crate::lisp::builtins::listfn::ListCycle::new(env);
    while let Value::Cons(c) = tail {
        let (a, d) = crate::lisp::eval::cons_parts(i, &c)?;
        if let Value::Cons(e) = &a {
            if i.sym_id(&crate::lisp::eval::cons_parts(i, e)?.0) == Some(kw) {
                return Ok(true);
            }
        }
        tail = d;
        cycle.step(i, &tail)?;
    }
    Ok(false)
}

/// Symbol ids bound by a `let'/`let*' varlist or a `lambda' arglist —
/// the names that shadow symbol macros inside the body.
fn binder_syms(i: &mut Interp, vars: &Value) -> Result<Vec<SymId>, Flow> {
    let mut out = Vec::new();
    let mut cur = vars.clone();
    let mut cycle = crate::lisp::builtins::listfn::ListCycle::new(vars);
    while let Value::Cons(c) = cur {
        let (a, d) = crate::lisp::eval::cons_parts(i, &c)?;
        let v = match &a {
            Value::Cons(bc) => crate::lisp::eval::cons_parts(i, bc)?.0,
            other => other.clone(),
        };
        if let Some(id) = i.sym_id(&v) {
            let n = i.symbol_name(id);
            if !n.starts_with('&') {
                out.push(id);
            }
        }
        cur = d;
        cycle.step(i, &cur)?;
    }
    Ok(out)
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
        let e2 = sm_filter_env(i, env, shadowed)?;
        let mark = i.specbind_depth();
        if let Err(error) = i.specbind(env_id, e2) {
            i.unbind_to(mark)?;
            return Err(error);
        }
        let r = macroexpand_all_recur(i, form);
        i.unbind_to(mark)?;
        r
    } else {
        macroexpand_all_recur(i, form)
    }
}

fn macroexpand_all_inner(i: &mut Interp, form: &Value) -> EvalResult {
    // GNU `macroexp--expand-all' passes `macroexpand-all-environment'
    // (bound by `macroexpand-all') to `macroexpand' explicitly.
    let env_id = i.q("macroexpand-all-environment");
    let mut form = form.clone();
    let backquote_list_star = i.q("backquote-list*");
    let is_backquote_list_star = match &form {
        Value::Cons(cell) => {
            let head = crate::lisp::eval::cons_parts(i, cell)?.0;
            i.sym_id(&head) == Some(backquote_list_star)
        }
        _ => false,
    };
    if is_backquote_list_star {
        // GNU processes this macro in reverse: recursively expand its
        // arguments first, then macroexpand the outer form. Keep the
        // argument spine live so an earlier argument macro can replace a
        // later cell before it is read.
        let args = macroexpand_list_tail(i, &form, 1)?;
        let args = macroexpand_live_spine(i, &args, macroexpand_all_recur)?;
        let head = match &form {
            Value::Cons(cell) => crate::lisp::eval::cons_parts(i, cell)?.0,
            _ => unreachable!(),
        };
        form = macroexpand_prepend_preserving(i, &form, vec![head], args)?;
    }
    let env = i.symbol_value(env_id);
    // `cl-symbol-macrolet' works by advising `macroexpand'/
    // `macroexpand-1' (`cl--sm-macroexpand{,-1}') during its own
    // expansion: going through the Lisp-level function cell picks up
    // that advice so symbol macros substitute, `setq' rewrites to
    // `setf', and let/lambda bindings alpha-rename.  When nothing is
    // advised the cell is still the subr — call the Rust fast path.
    let mac_sym = i.q("macroexpand");
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
        Value::Subr(_) => i.macroexpand_env(&form, &env)?,
        _ => i.apply(&Value::Sym(mac_sym), vec![form.clone(), env.clone()])?,
    };
    let sm = sm_active(i, &env)?;
    // GNU `cl--sm-macroexpand-1': a symbol bound by `cl-symbol-macrolet'
    // substitutes to its expansion form.
    if sm {
        if let Some(sid) = i.sym_id(&expanded) {
            if let Some(subst) = sm_lookup(i, &env, sid)? {
                // A symbol macro may be bound to itself. GNU leaves that
                // identity unchanged instead of recursively expanding it.
                if crate::lisp::builtins::eq_values(&subst, &expanded) {
                    return Ok(expanded);
                }
                return macroexpand_all_recur(i, &subst);
            }
        }
    }
    // `macroexpand'/`macroexpand-1'.
    let compiler_macro = compiler_macro_handler(i, &expanded)?;
    if let Some(handler) = &compiler_macro {
        if let Some(newf) = expand_compiler_macro(i, handler, &expanded)? {
            return macroexpand_all_recur(i, &newf);
        }
    }
    let mark = i.specbind_depth();
    let closure_id = i.q_soft("closure");
    let scoped = match &expanded {
        Value::Cons(c) => {
            let car = crate::lisp::eval::cons_parts(i, c)?.0;
            i.sym_id(&car).is_some_and(|id| {
                matches!(
                    id,
                    sym::LET
                        | sym::LET_STAR
                        | sym::LAMBDA
                        | sym::FUNCTION
                        | sym::DEFUN
                        | sym::DEFMACRO
                ) || Some(id) == closure_id
            })
        }
        _ => false,
    };
    if scoped {
        let id = i.q("macroexp--dynvars");
        let old = if i.bound_p(id) {
            i.symbol_value(id)
        } else {
            Value::Nil
        };
        if let Err(error) = i.specbind(id, old) {
            i.unbind_to(mark)?;
            return Err(error);
        }
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
            let (car, cdr) = crate::lisp::eval::cons_parts(i, c)?;
            // Don't descend into quote.
            if i.sym_is(&car, sym::QUOTE) {
                return Ok(expanded);
            }
            // Rebuild with expanded elements; improper lists pass
            // through unexpanded rather than failing.  (Plain
            // `list_to_vec' — `want_list' builds a `listp' signal that
            // is only discarded here, which also pollutes the
            // REMACS_BT_ERR log.)
            let Some(items) = macroexpand_list_to_vec(i, &expanded)? else {
                if i.sym_is(&car, sym::SETQ) {
                    let mut tail = cdr;
                    let mut seen = FxHashSet::default();
                    loop {
                        match &tail {
                            Value::Nil => break,
                            Value::Cons(cell) => {
                                if !seen.insert(Rc::as_ptr(cell) as usize) {
                                    break;
                                }
                                let next = crate::lisp::eval::cons_parts(i, cell)?.1;
                                tail = next;
                            }
                            _ => return Err(i.wrong_type_mut("listp", &tail)),
                        }
                    }
                }
                if i.sym_id(&car).is_none() {
                    return Ok(expanded);
                }
                return macroexpand_live_form_spine(i, &expanded, compiler_macro);
            };
            let _ = cdr;
            // Special forms whose non-code positions must not be
            // expanded: a parameter or binding variable can share a
            // name with a macro (e.g. `(defun f (rx) ...)'), and
            // expanding it would rewrite the arglist into the macro's
            // expansion — GNU's `macroexp--expand-all' keeps these
            // positions verbatim.
            let closure_id = i.q("closure");
            let funcall_id = i.q("funcall");
            if let Some(id) = i.sym_id(&car) {
                if matches!(id, sym::LET | sym::LET_STAR) && items.len() < 3 {
                    let mut normalized = items.clone();
                    if normalized.len() == 1 {
                        normalized.push(Value::Nil);
                    }
                    normalized.push(Value::Nil);
                    return macroexpand_prepend_preserving(
                        i,
                        &expanded,
                        normalized,
                        Value::Nil,
                    );
                }
                match id {
                    // `(closure ENV ARGLIST BODY...)': env and arglist
                    // are data positions like `lambda''s arglist.
                    _ if id == closure_id && items.len() >= 3 => {
                        let bound = binder_syms(i, &items[2])?;
                        let body = macroexpand_list_tail(i, &expanded, 3)?;
                        let body = macroexpand_live_filtered_body(
                            i, &body, env_id, &env, sm, &bound, true,
                        )?;
                        return macroexpand_prepend_preserving(
                            i,
                            &expanded,
                            items[..3].to_vec(),
                            body,
                        );
                    }
                    sym::DEFVAR | sym::DEFCONST
                        if items.len() >= 2
                            && items[1].truthy()
                            && i.sym_id(&items[1]).is_some() =>
                    {
                        let dynvars = i.q("macroexp--dynvars");
                        let old = if i.bound_p(dynvars) {
                            i.symbol_value(dynvars)
                        } else {
                            Value::Nil
                        };
                        i.set_symbol(dynvars, Value::cons(items[1].clone(), old))?;
                        let body = macroexpand_list_tail(i, &expanded, 2)?;
                        let body = macroexpand_live_spine(i, &body, macroexpand_all_recur)?;
                        return macroexpand_prepend_preserving(
                            i,
                            &expanded,
                            items[..2].to_vec(),
                            body,
                        );
                    }
                    sym::DEFVAR | sym::DEFCONST => return Ok(expanded),
                    sym::DEFUN | sym::DEFMACRO if items.len() >= 3 => {
                        // GNU `macroexp--defun' consumes leading
                        // `(declare ...)' specs itself; they are data,
                        // not code, and must not be descended into
                        // (an `ftype' spec holds a `(function TYPE)'
                        // form that is not a function quote).
                        let bound = binder_syms(i, &items[2])?;
                        let body = macroexpand_list_tail(i, &expanded, 3)?;
                        let body = macroexpand_live_filtered_body(
                            i, &body, env_id, &env, sm, &bound, true,
                        )?;
                        return macroexpand_prepend_preserving(
                            i,
                            &expanded,
                            items[..3].to_vec(),
                            body,
                        );
                    }
                    sym::LAMBDA if items.len() >= 2 => {
                        // A bare lambda's declarations are stray macro
                        // forms, unlike defun/defmacro metadata above.
                        let bound = binder_syms(i, &items[1])?;
                        let body = macroexpand_list_tail(i, &expanded, 2)?;
                        let body = macroexpand_live_filtered_body(
                            i, &body, env_id, &env, sm, &bound, false,
                        )?;
                        return macroexpand_prepend_preserving(
                            i,
                            &expanded,
                            items[..2].to_vec(),
                            body,
                        );
                    }
                    sym::FUNCTION if items.len() == 2 => {
                        // GNU `macroexp--expand-all' only descends into
                        // a lambda-shaped function argument; any other
                        // `(function ...)' form is data (e.g. an `ftype'
                        // spec inside `declare') and passes through
                        // verbatim, extra elements included.
                        if let Value::Cons(lc) = &items[1] {
                            let (head, _) = crate::lisp::eval::cons_parts(i, lc)?;
                            if i.sym_is(&head, sym::LAMBDA) {
                                // Expand the lambda's elements the way
                                // `macroexp--all-forms' does (arglist is
                                // data, body elements are code); do NOT
                                // recur through `macroexpand_all' on the
                                // whole `(lambda ...)' — `macroexpand-1'
                                // would just re-wrap it in `function'.
                                let head = 2;
                                let Some(litems) = macroexpand_list_to_vec(i, &items[1])? else {
                                    return Ok(expanded);
                                };
                                if litems.len() < head {
                                    return Ok(expanded);
                                }
                                let bound = binder_syms(i, &litems[head - 1])?;
                                let body = macroexpand_list_tail(i, &items[1], head)?;
                                let body = macroexpand_live_filtered_body(
                                    i, &body, env_id, &env, sm, &bound, false,
                                )?;
                                let lambda = macroexpand_prepend_preserving(
                                    i,
                                    &items[1],
                                    litems[..head].to_vec(),
                                    body,
                                )?;
                                return macroexpand_prepend_preserving(
                                    i,
                                    &expanded,
                                    vec![items[0].clone(), lambda],
                                    Value::Nil,
                                );
                            }
                        }
                        return Ok(expanded);
                    }
                    sym::FUNCTION => return Ok(expanded),
                    sym::LET | sym::LET_STAR | sym::AND_LET_STAR if items.len() >= 2 => {
                        // GNU `cl--sm-macroexpand-1' (let/let* arm):
                        // bound names shadow symbol macros inside their
                        // scope.  `let' inits see the outer env; `let*'
                        // inits see the env filtered by the bindings
                        // already walked; the body is filtered by all.
                        let bound = binder_syms(i, &items[1])?;
                        let is_letstar = i.sym_is(&items[0], sym::LET_STAR)
                            || i.sym_is(&items[0], sym::AND_LET_STAR);
                        let body_env = sm_filter_env(i, &env, &bound)?;
                        let body_tail = macroexpand_list_tail(i, &expanded, 2)?;
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
                                            let (b, _) = crate::lisp::eval::cons_parts(i, &c)?;
                                            // Env governing this binding's
                                            // init value.
                                            let init_env = if !sm {
                                                None
                                            } else if is_letstar {
                                                Some(sm_filter_env(i, &env, &seen)?)
                                            } else {
                                                // `let' inits expand in
                                                // the OUTER env (parallel
                                                // bindings, GNU sm arm).
                                                Some(env.clone())
                                            };
                                            if is_letstar && sm {
                                                let bv = match &b {
                                                    Value::Cons(bc) => {
                                                        crate::lisp::eval::cons_parts(i, bc)?.0
                                                    }
                                                    other => other.clone(),
                                                };
                                                if let Some(sid) = i.sym_id(&bv) {
                                                    if !i.symbol_name(sid).starts_with('&') {
                                                        seen.push(sid);
                                                    }
                                                }
                                            }
                                            let nb = {
                                                let keep_head = match &b {
                                                    Value::Cons(bc) => i
                                                        .sym_id(
                                                            &crate::lisp::eval::cons_parts(i, bc)?
                                                                .0,
                                                        )
                                                        .is_some(),
                                                    _ => false,
                                                };
                                                // For non-letstar plain-let,
                                                // all bound names shadow.
                                                let init_env = init_env.clone();
                                                let expand_init =
                                                    |i: &mut Interp, p: &Value| -> EvalResult {
                                                        if let Some(e2) = &init_env {
                                                            let mark = i.specbind_depth();
                                                            if let Err(error) =
                                                                i.specbind(env_id, e2.clone())
                                                            {
                                                                i.unbind_to(mark)?;
                                                                return Err(error);
                                                            }
                                                            let r = macroexpand_all_recur(i, p);
                                                            i.unbind_to(mark)?;
                                                            r
                                                        } else {
                                                            macroexpand_all_recur(i, p)
                                                        }
                                                    };
                                                if keep_head {
                                                    let parts =
                                                        want_list(i, &b).unwrap_or_default();
                                                    if parts.is_empty() {
                                                        b.clone()
                                                    } else {
                                                        let init_tail =
                                                            macroexpand_list_tail(i, &b, 1)?;
                                                        let init_tail = macroexpand_live_spine(
                                                            i,
                                                            &init_tail,
                                                            |i, p| expand_init(i, p),
                                                        )?;
                                                        macroexpand_prepend_preserving(
                                                            i,
                                                            &b,
                                                            vec![parts[0].clone()],
                                                            init_tail,
                                                        )?
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
                                            // An initializer macro may have
                                            // changed this binding list's
                                            // live cdr while it was expanded.
                                            cur = crate::lisp::eval::cons_parts(i, &c)?.1;
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
                                newvarlist = macroexpand_preserve_spine(i, &items[1], newvarlist)?;
                                out.push(newvarlist);
                            }
                            v => out.push(v.clone()),
                        }
                        // Body under the fully filtered env. Walk its live
                        // cons spine so earlier macros can update later body
                        // cells before they are expanded.
                        let body = if sm {
                            let mark = i.specbind_depth();
                            if let Err(error) = i.specbind(env_id, body_env) {
                                i.unbind_to(mark)?;
                                return Err(error);
                            }
                            let body = macroexpand_live_spine(i, &body_tail, macroexpand_all_recur);
                            i.unbind_to(mark)?;
                            body?
                        } else {
                            macroexpand_live_spine(i, &body_tail, macroexpand_all_recur)?
                        };
                        return macroexpand_prepend_preserving(i, &expanded, out, body);
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
                                    if sm_lookup(i, &env, vid)?.is_some() {
                                        any_sm = true;
                                        break;
                                    }
                                }
                            }
                            if any_sm {
                                // Emit `(setf SUBST EXPR)' pairs directly —
                                // emitting `(setf SYM ...)' would bounce
                                // setf→setq→setf forever.
                                let setf_sym = Value::Sym(i.q("setf"));
                                let mut assigns = Vec::new();
                                for pair in args.chunks(2) {
                                    let place = match i.sym_id(&pair[0]) {
                                        Some(vid) => sm_lookup(i, &env, vid)?
                                            .unwrap_or_else(|| pair[0].clone()),
                                        None => pair[0].clone(),
                                    };
                                    let expr = macroexpand_all_recur(i, &pair[1])?;
                                    assigns.push(Value::list(vec![setf_sym.clone(), place, expr]));
                                }
                                let out = if assigns.len() == 1 {
                                    assigns.pop().unwrap()
                                } else {
                                    Value::cons(Value::Sym(sym::PROGN), Value::list(assigns))
                                };
                                return macroexpand_all_recur(i, &out);
                            }
                        }
                        // Fall through to the normal `setq' path when no
                        // variable is symbol-macro'd.
                        let args = &items[1..];
                        let nargs = args.len();
                        if nargs == 2 && plain_setq_var(i, &args[0]) {
                            let expr = macroexpand_all_recur(i, &args[1])?;
                            return macroexpand_prepend_preserving(
                                i,
                                &expanded,
                                vec![items[0].clone(), args[0].clone(), expr],
                                Value::Nil,
                            );
                        }
                        let mut assigns = Vec::new();
                        for pair in args.chunks(2) {
                            let var = &pair[0];
                            let expr = macroexpand_all_recur(i, &pair[1])?;
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
                            let expr = macroexpand_all_recur(i, &args[1])?;
                            return macroexpand_prepend_preserving(
                                i,
                                &expanded,
                                vec![items[0].clone(), args[0].clone(), expr],
                                Value::Nil,
                            );
                        }
                        if nargs % 2 == 1 {
                            // `(signal 'wrong-number-of-arguments
                            //          '(setq NARGS))'
                            return Ok(Value::list(vec![
                                Value::Sym(i.q("signal")),
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
                            let expr = macroexpand_all_recur(i, &pair[1])?;
                            let assignment = if plain_setq_var(i, var) {
                                Value::list(vec![items[0].clone(), var.clone(), expr])
                            } else if i.sym_id(var).is_some_and(|sid| is_keyword_id(i, sid)) {
                                // `(if (eq :k E) :k (signal 'setting-constant
                                //                          (list ':k)))'
                                Value::list(vec![
                                    Value::Sym(sym::IF),
                                    Value::list(vec![Value::Sym(i.q("eq")), var.clone(), expr]),
                                    var.clone(),
                                    Value::list(vec![
                                        Value::Sym(i.q("signal")),
                                        Value::list(vec![
                                            Value::Sym(sym::QUOTE),
                                            Value::Sym(sym::SETTING_CONSTANT),
                                        ]),
                                        Value::list(vec![
                                            Value::Sym(i.q("list")),
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
                                        Value::Sym(i.q("list")),
                                        Value::list(vec![Value::Sym(sym::QUOTE), var.clone()]),
                                    ]),
                                    _ => Value::list(vec![
                                        Value::Sym(i.q("list")),
                                        Value::list(vec![
                                            Value::Sym(sym::QUOTE),
                                            Value::Sym(i.q("symbolp")),
                                        ]),
                                        Value::list(vec![Value::Sym(sym::QUOTE), var.clone()]),
                                    ]),
                                };
                                Value::list(vec![
                                    Value::Sym(i.q("signal")),
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
                        let eexp = macroexpand_all_recur(i, &items[1])?;
                        let mut eargs = Vec::with_capacity(items.len() - 2);
                        for a in &items[2..] {
                            eargs.push(macroexpand_all_recur(i, a)?);
                        }
                        let mut f: Option<Value> = None;
                        if let Value::Cons(c) = &eexp {
                            let is_fn =
                                i.sym_is(&crate::lisp::eval::cons_parts(i, c)?.0, sym::FUNCTION);
                            if is_fn {
                                if let Some(v) = macroexpand_list_to_vec(i, &eexp)? {
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
                                    if !is_sf && !sym_macrop(i, fsid)? {
                                        let mut call = vec![fv.clone()];
                                        call.extend(eargs);
                                        return macroexpand_all_recur(i, &Value::list(call));
                                    }
                                }
                                None => {
                                    if let Value::Cons(lc) = &fv {
                                        let is_lam = i.sym_is(
                                            &crate::lisp::eval::cons_parts(i, lc)?.0,
                                            sym::LAMBDA,
                                        );
                                        if is_lam {
                                            if let Some(unfolded) =
                                                unfold_funcall_lambda(i, &fv, &eargs)?
                                            {
                                                return Ok(unfolded);
                                            }
                                            // GNU retains the call after
                                            // expanding its function and
                                            // actual arguments, even when
                                            // lambda unfolding is unsafe.
                                            return macroexpand_prepend_preserving(
                                                i,
                                                &expanded,
                                                vec![items[0].clone(), eexp],
                                                Value::list(eargs),
                                            );
                                        }
                                    }
                                }
                            }
                        }
                        let mut out = vec![items[0].clone(), eexp];
                        out.extend(eargs);
                        return Ok(Value::list(out));
                    }
                    sym::CONDITION_CASE if items.len() == 3 => {
                        return macroexpand_all_recur(i, &items[2]);
                    }
                    sym::CONDITION_CASE if items.len() >= 4 => {
                        // GNU `cl--sm-macroexpand-1': the error var
                        // shadows a same-named symbol macro in the
                        // protected form AND the handler bodies.
                        let bound: Vec<SymId> = i.sym_id(&items[1]).into_iter().collect();
                        let mut first = true;
                        let body_and_handlers = macroexpand_list_tail(i, &expanded, 2)?;
                        let body_and_handlers =
                            macroexpand_live_spine(i, &body_and_handlers, |i, form| {
                                if first {
                                    first = false;
                                    return expand_body_filtered(i, env_id, &env, sm, &bound, form);
                                }
                                if matches!(form, Value::Cons(_)) {
                                    if let Some(parts) = macroexpand_list_to_vec(i, form)? {
                                        if !parts.is_empty() {
                                            let tail = macroexpand_list_tail(i, form, 1)?;
                                            let tail = macroexpand_live_spine(i, &tail, |i, form| {
                                                expand_body_filtered(
                                                    i, env_id, &env, sm, &bound, form,
                                                )
                                            })?;
                                            return macroexpand_prepend_preserving(
                                                i,
                                                form,
                                                vec![parts[0].clone()],
                                                tail,
                                            );
                                        }
                                    }
                                }
                                Ok(form.clone())
                            })?;
                        return macroexpand_prepend_preserving(
                            i,
                            &expanded,
                            items[..2].to_vec(),
                            body_and_handlers,
                        );
                    }
                    sym::WHILE if items.len() == 1 => {
                        let quote = Value::Sym(sym::QUOTE);
                        let signal = Value::Sym(i.q("signal"));
                        let error = Value::Sym(i.q("wrong-number-of-arguments"));
                        let data = Value::list(vec![Value::Sym(sym::WHILE), Value::Int(0)]);
                        return Ok(Value::list(vec![
                            signal,
                            Value::list(vec![quote.clone(), error]),
                            Value::list(vec![quote, data]),
                        ]));
                    }
                    sym::UNWIND_PROTECT => {
                        if items.len() == 1 {
                            return Ok(Value::Nil);
                        }
                        if items.len() == 2 {
                            return macroexpand_all_recur(i, &items[1]);
                        }
                        let body = macroexpand_all_recur(i, &items[1])?;
                        let cleanup = macroexpand_list_tail(i, &expanded, 2)?;
                        let cleanup =
                            macroexpand_live_spine(i, &cleanup, macroexpand_all_recur)?;
                        return macroexpand_prepend_preserving(
                            i,
                            &expanded,
                            vec![items[0].clone(), body],
                            cleanup,
                        );
                    }
                    sym::COND => {
                        // GNU `macroexp--expand-all': `(cond CLAUSES...)'
                        // first trims clauses after an unconditional
                        // default, then runs `macroexp--all-clauses' (skip
                        // 0) on the live clauses that remain.
                        let original_clauses = macroexpand_list_tail(i, &expanded, 1)?;
                        let mut clauses = original_clauses.clone();
                        let mut default_tail = None;
                        for clause_index in 0..items.len().saturating_sub(2) {
                            let clause = &items[clause_index + 1];
                            if macroexpand_cond_default_clause(i, clause)? {
                                // GNU's `take' makes a fresh outer spine
                                // when clauses after a default are removed.
                                // Clause objects themselves stay shared, so
                                // mutations within an earlier clause remain
                                // visible during expansion.
                                default_tail = Some(macroexpand_list_tail(
                                    i,
                                    &original_clauses,
                                    clause_index + 1,
                                )?);
                                clauses = Value::list(items[1..=clause_index + 1].to_vec());
                                break;
                            }
                        }
                        let clauses = macroexpand_live_spine(i, &clauses, |i, clause| {
                            if let Value::Cons(_) = clause {
                                let expanded_clause =
                                    macroexpand_live_spine(i, clause, macroexpand_all_recur)?;
                                macroexpand_preserve_spine(i, clause, expanded_clause)
                            } else {
                                Ok(clause.clone())
                            }
                        })?;
                        let result = macroexpand_prepend_preserving(
                            i,
                            &expanded,
                            vec![items[0].clone()],
                            clauses,
                        )?;
                        if let Some(default_tail) = default_tail {
                            let warn = i.q("macroexp-warn-and-return");
                            if i.fbound_p(warn) {
                                let suspicious = i.q("suspicious");
                                return i.apply(
                                    &Value::Sym(warn),
                                    vec![
                                        Value::string(
                                            "Useless clause following default `cond' clause",
                                        ),
                                        result,
                                        Value::list(vec![
                                            Value::Sym(suspicious),
                                            Value::Sym(sym::COND),
                                        ]),
                                        Value::Sym(sym::T),
                                        default_tail,
                                    ],
                                );
                            }
                        }
                        return Ok(result);
                    }
                    _ => {}
                }
            }
            // GNU `macroexp--expand-all' case `(,(lambda ...) . ,args)':
            // a lambda in function position keeps its `(lambda ARGLIST)'
            // head verbatim (no `function' wrap) while its body and the
            // call arguments expand.
            if let Value::Cons(lc) = &items[0] {
                let is_lam =
                    i.sym_is(&crate::lisp::eval::cons_parts(i, lc)?.0, sym::LAMBDA);
                if is_lam {
                    let litems = match want_list(i, &items[0]) {
                        Ok(v) if v.len() >= 2 => v,
                        _ => return Ok(expanded),
                    };
                    let mut fun_items = litems[..2].to_vec();
                    for it in &litems[2..] {
                        fun_items.push(macroexpand_all_recur(i, it)?);
                    }
                    let mut res = vec![Value::list(fun_items)];
                    for it in &items[1..] {
                        res.push(macroexpand_all_recur(i, it)?);
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
            // The head position is a function/macro name, not a variable:
            // symbol macros never substitute it (GNU `cl--sm-subst'
            // parity — a bound `car' in `(car x)' stays `car').
            macroexpand_live_form_spine(i, &expanded, compiler_macro)
        }
        _ => Ok(expanded),
    }
}

pub(crate) fn f_signal(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    let symbol = args[0].clone();
    let data = arg(&args, 1);
    // Fsignal reserves (signal nil CONDITION) for memory-full delivery.
    // Other nil names become `error'; a non-symbol with nil DATA is a
    // condition object. Validation follows the observable signal hook.
    let out_of_memory = symbol.is_nil() && matches!(&data, Value::Cons(_));
    let condition = if out_of_memory {
        data
    } else if symbol.is_nil() {
        Value::cons(Value::Sym(sym::ERROR), data)
    } else if i.sym_id(&symbol).is_none() && data.is_nil() {
        symbol
    } else {
        Value::cons(symbol, data)
    };
    let mut signal = crate::lisp::error::Signal::from_condition(condition);
    signal.out_of_memory = out_of_memory;
    Err(Flow::Signal(signal))
}

fn f_error(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    // GNU subr.el: (signal 'error (list (apply #'format-message ARGS))) —
    // full `format' semantics (flags/width/precision, format errors
    // propagate, non-string formats signal stringp).
    let msg = super::strfn::f_format_message(i, args)?;
    Err(i.signal_data(sym::ERROR, vec![msg]))
}

fn f_user_error(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    // GNU subr.el: (signal 'user-error (list (apply #'format-message
    // FORMAT ARGS))).
    let msg = super::strfn::f_format_message(i, args)?;
    Err(i.signal_data(sym::USER_ERROR, vec![msg]))
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
        i.q_soft(name)
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
                        v.try_borrow().is_ok_and(|v| {
                            v.len() == 1 && matches!(v[0], Value::Int(0x60))
                        })
                    }
                    _ => false,
                };
            if graves {
                QuoteStyle::Grave
            } else {
                QuoteStyle::Curve
            }
        }
        Value::Sym(s) if i.q_soft("grave") == Some(s) => QuoteStyle::Grave,
        Value::Sym(s) if i.q_soft("straight") == Some(s) => QuoteStyle::Straight,
        _ => QuoteStyle::Curve,
    }
}

/// GNU's `format-message' quote convention (`text_quoting_style'):
/// grave leaves `` `' `` and `'` verbatim, straight maps both to ASCII
/// `'', curve turns them into U+2018/U+2019.  Only the format string
/// is translated, never the substituted arguments.
pub(crate) fn translate_message_quotes<T: crate::lisp::text::TextSource + ?Sized>(
    i: &Interp,
    fmt: &T,
) -> crate::lisp::text::EmacsText {
    use crate::lisp::text::{EmacsChar, EmacsText};
    fmt.with_text(|fmt| {
        let mut out = EmacsText::with_capacity(fmt.len());
        let style = text_quoting_style(i);
        for c in fmt.chars() {
            let replacement = match style {
                QuoteStyle::Grave => c,
                QuoteStyle::Straight if c == '`' => EmacsChar::from('\''),
                QuoteStyle::Curve if c == '`' => EmacsChar::from('\u{2018}'),
                QuoteStyle::Curve if c == '\'' => EmacsChar::from('\u{2019}'),
                _ => c,
            };
            out.push(replacement);
        }
        out
    })
}

pub(crate) fn f_throw(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    // Emacs: Fthrow scans the catch chain; with no matching catch it
    // signals `no-catch' at the throw site (so condition-case can catch
    // it, but an outer matching catch would have won instead).
    let target = if args[0].is_nil() {
        None
    } else {
        i.catch_tags
            .iter()
            .rev()
            .find(|tag| crate::lisp::builtins::eq_values_with_interp(i, tag, &args[0]))
            .cloned()
    };
    if let Some(target) = target {
        i.begin_quit_unwind();
        // Carry the exact matching tag through unwind.  Re-comparing a
        // positioned symbol after its dynamic comparison mode was restored
        // can select a different catch frame.
        Err(Flow::Throw(target, args[1].clone()))
    } else {
        let nc = i.q("no-catch");
        Err(i.signal_data(nc, vec![args[0].clone(), args[1].clone()]))
    }
}

fn f_condition_case_raw(i: &mut Interp, _args: crate::lisp::Argv) -> EvalResult {
    f_special_form_via_apply(i, Vec::new().into())
}

/// `(ignore-errors BODY...)` and `(with-demoted-errors BODY...)`.
fn f_ignore_errors_raw(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    let body = args.iter().next().cloned().unwrap_or(Value::Nil);
    let err = Value::Sym(i.q("error"));
    ignore_body_forms(i, &err, &body)
}

/// `(ignore-error CONDITION BODY...)` — GNU's two-arg form: the first
/// raw arg element is the error-condition spec, the rest is body.
fn f_ignore_error_raw(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    let all = args.iter().next().cloned().unwrap_or(Value::Nil);
    let (cond, body) = match &all {
        Value::Cons(c) => crate::lisp::eval::cons_parts(i, c)?,
        _ => (Value::Nil, Value::Nil),
    };
    ignore_body_forms(i, &cond, &body)
}

fn ignore_body_forms(i: &mut Interp, cond: &Value, body: &Value) -> EvalResult {
    // Register the claimed condition so handler-bind handlers don't
    // run for signals this form will swallow (GNU suppresses them via
    // the no-debugger-entry rule).
    let case_depth = i.case_handlers.len();
    i.case_handlers
        .push(crate::lisp::error::CaseHandler::single(cond.clone()));
    let r = i.eval_progn(body);
    let r = i.offer_flow(r);
    i.case_handlers.pop();
    match r {
        Ok(v) => Ok(v),
        Err(Flow::Signal(signal)) => {
            if signal.claims_frame(case_depth) {
                i.finish_quit_unwind();
                Ok(Value::Nil)
            } else {
                i.rebase_quit_unwind();
                Err(Flow::Signal(signal))
            }
        }
        Err(e) => {
            i.rebase_quit_unwind();
            Err(e)
        }
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
fn feature_present(i: &mut Interp, id: SymId) -> Result<bool, Flow> {
    let fid = i.q("features");
    let features = i.symbol_value(fid);
    let mut cur = features.clone();
    let mut cycle = crate::lisp::builtins::listfn::ListCycle::new(&features);
    loop {
        match cur {
            Value::Nil => return Ok(false),
            Value::Cons(cell) => {
                let (candidate, next) = cell
                    .try_borrow()
                    .map(|cell| (cell.car.clone(), cell.cdr.clone()))
                    .map_err(|_| i.error("Cannot traverse a mutably borrowed cons cell"))?;
                if super::eq_values_with_interp(i, &candidate, &Value::Sym(id)) {
                    return Ok(true);
                }
                cur = next;
                cycle.step(i, &cur)?;
            }
            _ => return Err(i.wrong_type_mut("listp", &features)),
        }
    }
}

/// GNU's LOADHIST_ATTACH: push `elt' onto `current-load-list' when a
/// load is in progress (and the list is still list-shaped).
pub(crate) fn loadhist_attach(i: &mut Interp, entry: Value) {
    let cll = i.q("current-load-list");
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
    let present = feature_present(i, id)?;
    let sub = arg(&args, 1);
    if !sub.is_nil() {
        if !present {
            return Ok(Value::Nil);
        }
        let prop = i.q("subfeatures");
        let subs = i.get_prop(id, prop);
        // Search live, as GNU does: a match before an improper/circular tail
        // returns immediately, while a full miss reports the bad list shape.
        let mut cur = subs.clone();
        let mut cycle = crate::lisp::builtins::listfn::ListCycle::new(&subs);
        loop {
            match cur {
                Value::Nil => return Ok(Value::Nil),
                Value::Cons(cell) => {
                    let (candidate, next) = cell
                        .try_borrow()
                        .map(|cell| (cell.car.clone(), cell.cdr.clone()))
                        .map_err(|_| i.error("Cannot traverse a mutably borrowed cons cell"))?;
                    if super::equal_values(i, &candidate, &sub)? {
                        return Ok(Value::t());
                    }
                    cur = next;
                    cycle.step(i, &cur)?;
                }
                _ => return Err(i.wrong_type_mut("listp", &subs)),
            }
        }
    }
    Ok(Value::from_bool(present))
}

fn f_provide(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    let id = want_sym(i, &args[0])?;
    // GNU Fprovide: CHECK_LIST on SUBFEATURES, then record it on the
    // feature's `subfeatures' plist.
    let sub = arg(&args, 1);
    if !matches!(&sub, Value::Nil | Value::Cons(_)) {
        return Err(i.wrong_type_mut("listp", &sub));
    }
    // GNU checks `features' before installing subfeatures or running
    // symbol-keyed after-load forms. Keep the initial membership result so
    // callbacks can observe and mutate the feature list in provide order.
    let already_present = feature_present(i, id)?;
    if !sub.is_nil() {
        let prop = i.q("subfeatures");
        i.put_prop(id, prop, sub);
    }
    // `remacs--provided': `features' is seeded at boot with GNU's dump
    // state, which includes libraries remacs never actually loads
    // (`cl-preloaded' & friends).  `eval-after-load' uses this mark to
    // tell "feature really provided" (GNU evaluates immediately) from
    // a seed-only entry (defer — the deferred forms may need machinery
    // the never-loaded file would have defined).
    let prov_mark = i.q("remacs--provided");
    i.put_prop(id, prov_mark, Value::t());
    let feat_v = args[0].clone();
    if !already_present {
        let fid = i.q("features");
        let cur = i.symbol_value(fid);
        let mut items = cur.list_to_vec().unwrap_or_default();
        items.insert(0, feat_v.clone());
        // GNU Fprovide updates Vfeatures directly; this C-side registry
        // update does not invoke Lisp variable watchers.
        i.store_internal_value(fid, Value::list(items));
    }
    if !i.features.contains(&id) {
        i.features.insert(0, id);
    }
    // GNU records `(provide . FEATURE)' on `current-load-list' at
    // provide time (catches nested provides too); this is visible to
    // symbol-keyed after-load forms.
    let provide_id = i.q("provide");
    loadhist_attach(i, Value::cons(i.sym(provide_id), feat_v.clone()));

    // Symbol-keyed `after-load-alist' entries run after feature and load
    // history state is installed; they defer to `after-load-functions'
    // when a file load is in progress.
    let alist_sym = i.q("after-load-alist");
    let alist = i.symbol_value(alist_sym);
    match alist.list_to_vec() {
        Ok(_) => {}
        Err(crate::lisp::value::ListError::Circular(tail)) => {
            return Err(i.signal_data(sym::CIRCULAR_LIST, vec![tail]));
        }
        // GNU's `assoc' reports an improper alist with the whole argument.
        Err(crate::lisp::value::ListError::Dotted(_)) => {
            return Err(i.wrong_type_mut("listp", &alist));
        }
        Err(crate::lisp::value::ListError::Borrowed) => {
            return Err(i.error("Cannot inspect a mutably borrowed after-load list"));
        }
    };
    let funcall = crate::lisp::builtins::evalfn::SUBRS
        .iter()
        .find(|subr| subr.name == "funcall")
        .map(|subr| Value::Subr(subr))
        .ok_or_else(|| i.error("funcall subr not registered"))?;
    // Walk the live alist spine.  A callback may remove or replace later
    // entries by changing the current cons's cdr; fetching that cdr only
    // after the callback makes those edits visible to the traversal.
    let mut cursor = alist;
    let mut seen = FxHashSet::default();
    loop {
        let Value::Cons(cell) = cursor else {
            if cursor.is_nil() {
                break;
            }
            return Err(i.wrong_type_mut("listp", &cursor));
        };
        if !seen.insert(Rc::as_ptr(&cell) as usize) {
            return Err(i.signal_data(
                sym::CIRCULAR_LIST,
                vec![Value::Cons(cell)],
            ));
        }
        let (entry, _) = crate::lisp::eval::cons_parts(i, &cell)?;
        if let Value::Cons(entry_cell) = entry {
            let (key, forms) = entry_cell
                .try_borrow()
                .map(|entry| (entry.car.clone(), entry.cdr.clone()))
                .map_err(|_| i.error("Cannot inspect a mutably borrowed after-load entry"))?;
            if super::eq_values_with_interp(i, &key, &feat_v) {
                crate::lisp::builtins::seq::f_mapc(i, vec![funcall.clone(), forms].into())?;
            }
        }
        cursor = crate::lisp::eval::cons_parts(i, &cell)?.1;
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
    let require_id = i.q("require");
    let cll = i.q("current-load-list");
    let tem = Value::cons(i.sym(require_id), args[0].clone());
    let cll_val = i.symbol_value(cll);
    let lip = i.q("load-in-progress");
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
    if feature_present(i, id)? {
        return Ok(args[0].clone());
    }
    // Try to load feature file from load-path.
    let name = match args.get(1) {
        Some(value @ Value::Str(_)) => filename_text_arg(i, value)?,
        _ => i.symbol_name_text(id),
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
            if !feature_present(i, id)? {
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
            let fm = i.q("file-missing");
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

/// GNU `run_hook_with_args': retain the current cons, but fetch its cdr
/// after the callback. A `t' reads the current default then and skips any
/// nested `t' entries. CALLBACK returns non-nil to stop the walk.
fn run_hook_functions(
    i: &mut Interp,
    hook: &Value,
    mut callback: impl FnMut(&mut Interp, Value) -> EvalResult,
) -> EvalResult {
    want_hook_sym(i, hook)?;
    let id = i.sym_id(hook).unwrap();
    let mut value = i.symbol_value(id);
    if value.is_nil() || matches!(&value, Value::Sym(s) if *s == sym::UNBOUND) {
        return Ok(Value::Nil);
    }
    if !matches!(&value, Value::Cons(_)) || hook_lambda(i, &value)? {
        return callback(i, value);
    }
    while let Value::Cons(cell) = value {
        let function = crate::lisp::eval::cons_parts(i, &cell)?.0;
        let result = if i.sym_is(&function, sym::T) {
            let base = i.var_alias_target(id);
            let mut defaults = i.obarray.symbol(base).value.clone();
            if matches!(&defaults, Value::Sym(s) if *s == sym::UNBOUND) {
                return Err(i.signal_data(sym::VOID_VARIABLE, vec![hook.clone()]));
            }
            let mut result = Value::Nil;
            if defaults.is_nil() {
                // No global functions at this sentinel.
            } else if !matches!(&defaults, Value::Cons(_)) || hook_lambda(i, &defaults)? {
                result = callback(i, defaults)?;
            } else {
                while let Value::Cons(global) = defaults {
                    let function = crate::lisp::eval::cons_parts(i, &global)?.0;
                    if !i.sym_is(&function, sym::T) {
                        result = callback(i, function)?;
                        if result.truthy() {
                            break;
                        }
                    }
                    // Fetch the next cell after the callback: it may have
                    // mutated the live default hook list.
                    defaults = crate::lisp::eval::cons_parts(i, &global)?.1;
                }
            }
            result
        } else {
            callback(i, function)?
        };
        if result.truthy() {
            return Ok(result);
        }
        // Hooks can mutate their own list cells while running.
        value = crate::lisp::eval::cons_parts(i, &cell)?.1;
    }
    Ok(Value::Nil)
}

fn hook_lambda(i: &Interp, value: &Value) -> Result<bool, Flow> {
    let Value::Cons(cell) = value else {
        return Ok(false);
    };
    let head = crate::lisp::eval::cons_parts(i, cell)?.0;
    Ok(i.sym_is(&head, sym::LAMBDA))
}

fn want_hook_sym(i: &mut Interp, hook: &Value) -> Result<(), super::Flow> {
    match i.sym_id(hook) {
        Some(_) => Ok(()),
        None => Err(i.wrong_type_mut("symbolp", hook)),
    }
}

fn f_run_hooks(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    for hook in args.iter() {
        run_hook_functions(i, hook, |i, f| {
            i.apply(&f, vec![])?;
            Ok(Value::Nil)
        })?;
    }
    Ok(Value::Nil)
}

pub(crate) fn f_run_hook_with_args(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    let hook = args[0].clone();
    run_hook_functions(i, &hook, |i, f| {
        i.apply(&f, args[1..].to_vec())?;
        Ok(Value::Nil)
    })
}

fn f_run_hook_until_fail(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    let hook = args[0].clone();
    let failed = run_hook_functions(i, &hook, |i, f| {
        Ok(Value::from_bool(i.apply(&f, args[1..].to_vec())?.is_nil()))
    })?;
    Ok(Value::from_bool(failed.is_nil()))
}

fn f_run_hook_until_success(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    let hook = args[0].clone();
    run_hook_functions(i, &hook, |i, f| i.apply(&f, args[1..].to_vec()))
}

fn f_run_hook_wrapped(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    // (run-hook-wrapped HOOK WRAP-FUNCTION &rest ARGS)
    // GNU eval.c: each hook function is called through WRAP-FUNCTION;
    // if a call returns non-nil, hook processing aborts and returns it.
    // HOOK must be a symbol like the other run-hook-* entry points.
    let hook = args[0].clone();
    let wrap = args[1].clone();
    run_hook_functions(i, &hook, |i, f| {
        let mut call_args = vec![f];
        call_args.extend(args[2..].iter().cloned());
        i.apply(&wrap, call_args)
    })
}

/// GNU `inhibit_modification_hooks': `inhibit-modification-hooks'
/// (bound by `with-silent-modifications' and `combine-after-change-*'
/// internals) suppresses the before/after-change hooks entirely.
pub(crate) fn mod_hooks_inhibited(i: &Interp) -> bool {
    i.q_soft("inhibit-modification-hooks")
        .map(|sid| i.symbol_value(sid).truthy())
        .unwrap_or(false)
}

/// GNU `textget (plist, prop)' on a buffer overlay: direct plist hit,
/// else fall back through the `category' symbol's own plist like
/// `Foverlay_get'.
fn overlay_prop(i: &mut Interp, plist: &Value, prop: &str) -> Value {
    let Some(pid) = i.q_soft(prop) else {
        return Value::Nil;
    };
    match crate::lisp::eval::plist_lookup_v(plist, &Value::Sym(pid)) {
        Some(v) => v,
        None => {
            let cat = i.q("category");
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
                let (f, next) = crate::lisp::eval::cons_parts(i, &c)?;
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
fn buffer_has_intervals(i: &Interp) -> Result<bool, Flow> {
    let Some(buffer) = i.buffers.get(i.current_buffer) else {
        return Ok(false);
    };
    let buffer = buffer
        .try_borrow()
        .map_err(|_| i.error("Cannot inspect a mutably borrowed buffer"))?;
    if buffer.text.had_intervals.get() {
        return Ok(true);
    }
    buffer
        .text
        .props
        .try_borrow()
        .map(|properties| !properties.is_empty())
        .map_err(|_| i.error("Cannot inspect mutably borrowed buffer text properties"))
}

fn buffer_has_overlays(i: &Interp) -> Result<bool, Flow> {
    let Some(buffer) = i.buffers.get(i.current_buffer) else {
        return Ok(false);
    };
    let buffer = buffer
        .try_borrow()
        .map_err(|_| i.error("Cannot inspect a mutably borrowed buffer"))?;
    Ok(buffer
        .overlays
        .iter()
        .any(|overlay| overlay.buffer == Some(buffer.id)))
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
    let bh = i.q("insert-behind-hooks");
    let fh = i.q("insert-in-front-hooks");
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
    let mh = i.q("modification-hooks");
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
fn overlay_handle_parts(i: &Interp, v: &Value) -> Result<Option<(usize, usize)>, Flow> {
    if let Value::Vec(vec) = v {
        let vv = vec
            .try_borrow()
            .map_err(|_| i.error("Cannot inspect a mutably borrowed overlay handle"))?;
        if vv.len() == 3 {
            if let (Value::Sym(tag), Value::Int(bid), Value::Int(idx)) = (&vv[0], &vv[1], &vv[2]) {
                if Some(*tag) == i.q_soft("overlay") {
                    return Ok(Some((*bid as usize, *idx as usize)));
                }
            }
        }
    }
    Ok(None)
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
        let attached = match overlay_handle_parts(i, &handle)? {
            Some((bid, idx)) => match i.buffers.get(bid) {
                Some(buffer) => {
                    let buffer = buffer
                        .try_borrow()
                        .map_err(|_| i.error("Cannot inspect a mutably borrowed buffer"))?;
                    buffer
                        .overlays
                        .get(idx)
                        .is_some_and(|overlay| overlay.buffer == Some(i.current_buffer))
                }
                None => false,
            },
            None => false,
        };
        if !attached {
            continue;
        }
        let mut cur = list;
        loop {
            match cur {
                Value::Nil => break,
                Value::Cons(c) => {
                    let (f, next) = crate::lisp::eval::cons_parts(i, &c)?;
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
fn combine_gate_ok(i: &mut Interp) -> Result<bool, Flow> {
    let cacc = match i.q_soft("combine-after-change-calls") {
        Some(s) => i.symbol_value(s).truthy(),
        None => return Ok(false),
    };
    if !cacc {
        return Ok(false);
    }
    let bcf_id = i.q("before-change-functions");
    let bcf = i.symbol_value(bcf_id);
    let shape_ok = if bcf.is_nil() {
        true
    } else if let Value::Cons(c) = &bcf {
        let (car, cdr) = crate::lisp::eval::cons_parts(i, c)?;
        let car_t = i.sym_is(&car, sym::T);
        let default_nil = match i.obarray.symbol(bcf_id).value {
            Value::Nil => true,
            Value::Sym(s) => s == sym::UNBOUND,
            _ => false,
        };
        let ppss = i.q("syntax-ppss-flush-cache");
        let tail_ok = match &cdr {
            Value::Cons(c2) => {
                let (head, tail) = crate::lisp::eval::cons_parts(i, c2)?;
                i.sym_is(&head, ppss) && tail.is_nil()
            }
            _ => false,
        };
        car_t && default_nil && tail_ok
    } else {
        false
    };
    Ok(shape_ok && !buffer_has_overlays(i)?)
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
    let zv0 = i.buffers.get(bid).map(|b| b.borrow().zv).unwrap_or(0);
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
    // runs at the change start before any hook work, even under
    // `inhibit-modification-hooks'.
    crate::buffer::primitives::check_writable_at(i, beg1 - 1)?;
    // `verify_interval_modification' — only when the buffer has an
    // interval tree; resets the saved insert-hook lists either way.
    if buffer_has_intervals(i)? {
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
    let imh = i.q("inhibit-modification-hooks");
    if let Err(error) = i.specbind(imh, Value::t()) {
        i.unbind_to(mark)?;
        return Err(error);
    }
    let result = signal_before_change_inner(i, beg1, end1);
    i.unbind_to(mark)?;
    result?;
    // GNU `prepare_to_modify_buffer_1' sets this after before-change hooks
    // succeed and before the buffer is changed. Watcher errors abort edits.
    let deactivate = i.q("deactivate-mark");
    i.set_symbol(deactivate, Value::t())?;
    Ok(Value::Nil)
}

fn signal_before_change_inner(i: &mut Interp, beg1: usize, end1: usize) -> EvalResult {
    let unmodified = !i
        .buffers
        .get(i.current_buffer)
        .map(|b| b.borrow().is_modified())
        .unwrap_or(false);
    if unmodified {
        call_hook(i, "first-change-hook")?;
    }
    let args = vec![Value::Int(beg1 as i128), Value::Int(end1 as i128)];
    let id = i.q("before-change-functions");
    run_hook_functions(i, &Value::Sym(id), |i, f| {
        if let Err(fl) = i.apply(&f, args.clone()) {
            // GNU `reset_var_on_error': an erroring hook function
            // clears `before-change-functions' without notifying variable
            // watchers; resetting is part of the failed hook's unwind.
            reset_hook_variable_on_error(i, id)?;
            return Err(fl);
        }
        Ok(Value::Nil)
    })?;
    if buffer_has_overlays(i)? {
        collect_overlay_hooks(i, beg1, end1);
        run_overlay_hooks(i, false, beg1, end1, &Value::Nil)?;
    }
    Ok(Value::Nil)
}

/// GNU's `reset_var_on_error' writes the active hook binding directly.
/// It must not invoke variable watchers: a watcher error would abort the
/// reset and leave the failing hook installed for the next modification.
fn reset_hook_variable_on_error(i: &mut Interp, id: SymId) -> Result<(), Flow> {
    i.set_symbol_no_watchers(id, Value::Nil)
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
    if combine_gate_ok(i)? {
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
    let imh = i.q("inhibit-modification-hooks");
    if let Err(error) = i.specbind(imh, Value::t()) {
        i.pending_insert_behind = saved_behind;
        i.pending_insert_front = saved_front;
        i.unbind_to(mark)?;
        return Err(error);
    }
    let args = vec![
        Value::Int(beg1 as i128),
        Value::Int(end1 as i128),
        Value::Int(lendel),
    ];
    let id = i.q("after-change-functions");
    let mut result = run_hook_functions(i, &Value::Sym(id), |i, f| {
        if let Err(fl) = i.apply(&f, args.clone()) {
            // GNU `reset_var_on_error' clears the hook on error.
            reset_hook_variable_on_error(i, id)?;
            return Err(fl);
        }
        Ok(Value::Nil)
    });
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
    i.unbind_to(mark)?;
    result
}

/// GNU `run_hook_with_args' with no args: apply each function bound
/// on the hook variable NAME (buffer-local `t' splicing included).
pub(crate) fn call_hook(i: &mut Interp, name: &str) -> EvalResult {
    let id = i.q(name);
    run_hook_functions(i, &Value::Sym(id), |i, f| {
        i.apply(&f, vec![])?;
        Ok(Value::Nil)
    })
}

/// GNU keyboard.c's cmd_error_internal. The reporter is Lisp-visible and
/// runs after the callback's internal condition-case has been removed.
pub(crate) fn command_error(i: &mut Interp, data: Value, context: &str) -> EvalResult {
    if let Value::Cons(condition) = &data {
        let quit = Value::Sym(i.q("quit"));
        let symbol = crate::lisp::eval::cons_parts(i, condition)?.0;
        if i.signal_matches(&symbol, &quit) {
            i.signaling_function = Value::Nil;
        }
    }
    i.clear_quit_flag();
    let inhibit = i.q("inhibit-quit");
    i.obarray.symbol_mut(inhibit).value = Value::t();
    let reporter = i.q("command-error-function");
    let reporter = i.symbol_value(reporter);
    if !reporter.is_nil() {
        let origin = i.signaling_function.clone();
        i.apply(&reporter, vec![data, Value::string(context), origin])?;
    }
    i.signaling_function = Value::Nil;
    Ok(Value::Nil)
}

/// GNU `safe_run_hooks' (keyboard.c): `inhibit-quit' is bound, and
/// each hook function runs inside an error handler that messages
/// "Error in HOOK (FUN): ERROR" and removes FUN from the hook.
/// Signals and quits are swallowed; throws/exits still propagate.
pub(crate) fn safe_call_hook(i: &mut Interp, name: &str) -> EvalResult {
    let mark = i.specbind_depth();
    let iq = i.q("inhibit-quit");
    if let Err(f) = i.specbind(iq, Value::t()) {
        let result = i.offer_flow(Err(f));
        return i.unbind_to(mark).and(result);
    }
    let hook_id = i.q(name);
    let result = run_hook_functions(i, &Value::Sym(hook_id), |i, f| {
        let case_depth = i.case_handlers.len();
        i.case_handlers
            .push(crate::lisp::error::CaseHandler::single(Value::t()));
        let result = i.apply(&f, vec![]);
        let result = i.offer_flow(result);
        i.case_handlers.truncate(case_depth);
        match result {
            Ok(_) | Err(Flow::Quit) => Ok(Value::Nil),
            Err(Flow::Signal(signal)) if signal.claims_frame(case_depth) => {
                i.finish_quit_unwind();
                let err = signal.condition;
                let msg = format!(
                    "Error in {} ({}): {}",
                    name,
                    i.prin1_to_string(&f),
                    i.prin1_to_string(&err)
                );
                i.message(&msg);
                safe_remove_hook_fn(i, hook_id, &f)
            }
            Err(flow) => {
                i.rebase_quit_unwind();
                Err(flow)
            }
        }
    });
    let result = i.offer_flow(result);
    i.unbind_to(mark).and(result)
}

/// GNU `safe_run_hooks_error': drop FUN from HOOK's buffer-local list
/// if present there, else from its global (default) value.  GNU uses
/// identity here, removing every occurrence of the failing function.
fn safe_remove_hook_fn(i: &mut Interp, hook_id: SymId, f: &Value) -> EvalResult {
    // Fset notifies watchers even for a local binding. If a local `t'
    // supplied the failing function, Fset_default changes only its default.
    if let Some(value) = hook_list_without_function(i, i.symbol_value(hook_id), f)? {
        i.set_symbol(hook_id, value)?;
    } else {
        let base = i.var_alias_target(hook_id);
        if let Some(value) =
            hook_list_without_function(i, i.obarray.symbol(base).value.clone(), f)?
        {
            i.set_symbol_default(hook_id, value)?;
        }
    }
    Ok(Value::Nil)
}

fn hook_list_without_function(
    i: &Interp,
    mut value: Value,
    function: &Value,
) -> Result<Option<Value>, Flow> {
    let mut found = false;
    let mut functions = Vec::new();
    while let Value::Cons(cell) = value {
        let (car, cdr) = crate::lisp::eval::cons_parts(i, &cell)?;
        if super::eq_values(&car, function) {
            found = true;
        } else {
            functions.push(car);
        }
        value = cdr;
    }
    Ok(found.then(|| Value::list(functions)))
}

// `add-hook'/`remove-hook' mirror GNU 31.1's Lisp definitions in
// subr.el, including depth bookkeeping via the `hook--depth-alist'
// symbol property (an uninterned symbol whose value is an alist).

fn hook_is_local(i: &Interp, id: SymId) -> Result<bool, Flow> {
    if i.obarray.symbol(id).always_local {
        return Ok(true);
    }
    match i.buffers.get(i.current_buffer) {
        Some(buffer) => buffer
            .try_borrow()
            .map(|buffer| buffer.locals.contains_key(&id))
            .map_err(|_| i.error("Cannot inspect a mutably borrowed buffer")),
        None => Ok(false),
    }
}

fn hook_local_if_set(i: &Interp, id: SymId) -> Result<bool, Flow> {
    let s = i.obarray.symbol(id);
    // GNU `local-variable-if-set-p' is per-buffer: `defvar' /
    // `permanent-local' globals count, but a `make-local-variable'
    // performed in another buffer does not.
    if s.make_local_if_set || s.always_local {
        return Ok(true);
    }
    hook_is_local(i, id)
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
fn hook_make_local(i: &mut Interp, id: SymId) -> Result<(), Flow> {
    let seed = i.obarray.symbol(id).value.clone();
    if let Some(b) = i.buffers.get(i.current_buffer) {
        b.try_borrow_mut()
            .map_err(|_| i.error("Cannot update a mutably borrowed buffer"))?
            .locals
            .entry(id)
            .or_insert(seed);
    }
    Ok(())
}

/// `(local-variable-p HOOK) and value lacks a `t' member' — GNU treats
/// a global add/remove on such a hook as local (make-local-variable
/// compatibility).
fn hook_force_local(i: &mut Interp, id: SymId) -> Result<bool, Flow> {
    if !hook_is_local(i, id)? {
        return Ok(false);
    }
    let v = i.symbol_value(id);
    match macroexpand_list_to_vec(i, &v)? {
        Some(list) => Ok(!list
            .iter()
            .any(|x| matches!(x, Value::Sym(s) if *s == sym::T))),
        None => Ok(true),
    }
}

/// The `hook--depth-alist' property holds an uninterned symbol whose
/// (possibly buffer-local) value is the fn->depth alist.
fn hook_depth_alist(
    i: &mut Interp,
    hook_id: SymId,
    local: bool,
    create: bool,
) -> Result<Option<Vec<(Value, i128)>>, Flow> {
    let prop_sym = i.q("hook--depth-alist");
    let prop = i.get_prop(hook_id, prop_sym);
    let dsym = match prop {
        Value::Sym(s) => s,
        _ => {
            if !create {
                return Ok(None);
            }
            let s = i.obarray.make_symbol("depth-alist");
            i.set_symbol_default(s, Value::Nil)?;
            i.put_prop(hook_id, prop_sym, Value::Sym(s));
            s
        }
    };
    if local && create {
        hook_make_local(i, dsym)?;
    }
    let v = if local {
        i.symbol_value(dsym)
    } else {
        i.obarray.symbol(dsym).value.clone()
    };
    let Some(items) = macroexpand_list_to_vec(i, &v)? else {
        return Ok(Some(Vec::new()));
    };
    let mut result = Vec::new();
    for item in items {
        if let Value::Cons(cell) = item {
            let (function, depth) = crate::lisp::eval::cons_parts(i, &cell)?;
            if let Value::Int(depth) = depth {
                result.push((function, depth));
            }
        }
    }
    Ok(Some(result))
}

fn hook_set_depth_alist(
    i: &mut Interp,
    hook_id: SymId,
    local: bool,
    alist: Vec<(Value, i128)>,
) -> Result<(), Flow> {
    let prop_sym = i.q("hook--depth-alist");
    let prop = i.get_prop(hook_id, prop_sym);
    let dsym = match prop {
        Value::Sym(s) => s,
        _ => return Ok(()),
    };
    let items: Vec<Value> = alist
        .into_iter()
        .map(|(f, d)| Value::cons(f, Value::Int(d)))
        .collect();
    let v = Value::list(items);
    if local {
        if let Some(b) = i.buffers.get(i.current_buffer) {
            let mut bb = b
                .try_borrow_mut()
                .map_err(|_| i.error("Cannot update a mutably borrowed buffer"))?;
            if bb.locals.contains_key(&dsym) {
                bb.locals.insert(dsym, v);
                return Ok(());
            }
        }
    }
    i.obarray.symbol_mut(dsym).value = v;
    Ok(())
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
        if !hook_local_if_set(i, vid)? {
            hook_make_local(i, vid)?;
            i.set_symbol(hook_id, Value::list(vec![Value::Sym(sym::T)]))?;
        }
    } else if hook_force_local(i, vid)? {
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
        let mut alist = hook_depth_alist(i, hook_id, local, depth != 0)?.unwrap_or_default();
        if depth != 0 {
            alist.retain(|(f, _)| !crate::lisp::eq_values(f, &fun));
            alist.push((fun.clone(), depth));
            hook_set_depth_alist(i, hook_id, local, alist.clone())?;
        }
        if depth > 0 {
            list.push(fun.clone());
        } else {
            list.insert(0, fun.clone());
        }
        // When a depth alist exists, stable-sort by recorded depth.
        let depth_alist = hook_depth_alist(i, hook_id, local, false)?.unwrap_or_default();
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
            let plh_sym = i.q("permanent-local-hook");
            let pl_sym = i.q("permanent-local");
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
    if local && !hook_is_local(i, vid)? {
        return Ok(Value::Nil);
    }
    if hook_force_local(i, vid)? {
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
        if let Some(mut alist) = hook_depth_alist(i, hook_id, local, false)? {
            let before = alist.len();
            alist.retain(|(f, _)| !crate::lisp::eq_values(f, &old));
            if alist.len() != before {
                hook_set_depth_alist(i, hook_id, local, alist)?;
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
fn f_eval_expression(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    // GNU (simple.el): the value is consed onto `values'; INSERT-VALUE
    // non-nil inserts the printed value into the buffer instead of
    // echoing it.
    let v = i.eval(&args[0])?;
    let printed = i.print_to_string(&v);
    if arg(&args, 1).truthy() {
        let ins = Value::Sym(i.q("insert"));
        i.apply(&ins, vec![Value::string(printed)])?;
    } else {
        i.message(&printed);
    }
    let vals = i.q("values");
    let cur = i.symbol_value(vals);
    i.set_symbol(vals, Value::cons(v.clone(), cur))?;
    Ok(v)
}
fn f_load(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    // (load FILE &optional NOERROR NOMESSAGE NOSUFFIX MUST-SUFFIX)
    let name = filename_text_arg(i, &args[0])?;
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
            let fm = i.q("file-missing");
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
    let name = filename_text_arg(i, &args[0])?;
    let efn = i.q("expand-file-name");
    let expanded = i.apply(&Value::Sym(efn), vec![args[0].clone()])?;
    let name = match &expanded {
        Value::Str(_) => filename_text_arg(i, &expanded)?,
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
            let fm = i.q("file-missing");
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
    let name = filename_text_arg(i, &args[0])?;
    let nosuffix = arg(&args, 1).truthy();
    let path = arg(&args, 2);
    let interactive = arg(&args, 3).truthy();
    let default_path = path.is_nil();
    let search_path = if default_path {
        let id = i.q("load-path");
        i.symbol_value(id)
    } else {
        path
    };
    let mut file = locate_library_in(i, &name, nosuffix, &search_path)?;
    if default_path {
        file = match file {
            Some(path) => Some(crate::lisp::load::located_source(i, &name, path)?),
            None => {
                let suffixes = locate_library_suffixes(i, nosuffix)?;
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
        let msg_sym = i.q("message");
        i.apply(
            &Value::Sym(msg_sym),
            vec![Value::string("%s"), Value::string(msg)],
        )?;
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
    name: &crate::lisp::text::EmacsStr,
    nosuffix: bool,
    path: &Value,
) -> Result<Option<crate::lisp::text::EmacsText>, Flow> {
    // GNU `locate-file' iterates PATH as a list; a non-list PATH
    // contributes no directories (returns nil, no error).
    let dirs = path.list_to_vec().unwrap_or_default();
    let suffixes = locate_library_suffixes(i, nosuffix)?;
    if crate::editor::native_path(i, name)?.is_absolute() || name.starts_with('~') {
        // GNU `locate-file': absolute names only get suffix variants.
        let name = crate::editor::expand_file_name_str(i, name);
        for suf in &suffixes {
            let mut cand = name.clone();
            cand.push_str(suf);
            if crate::editor::native_path(i, &cand)?.is_file() {
                return Ok(Some(cand));
            }
        }
        return Ok(None);
    }
    for dir_v in &dirs {
        let directory = match dir_v {
            Value::Str(_) => {
                let directory = filename_text_arg(i, dir_v)?;
                crate::editor::expand_file_name_str(i, &directory)
            }
            Value::Nil => crate::editor::expand_file_name_str(i, ""),
            _ => continue,
        };
        let mut base = directory.clone();
        if !base.ends_with('/') {
            base.push('/');
        }
        base.push_str(name);
        let base = crate::editor::expand_file_name_str(i, &base);
        for suffix in &suffixes {
            let mut candidate = base.clone();
            candidate.push_str(suffix);
            if crate::editor::native_path(i, &candidate)?.is_file() {
                return Ok(Some(candidate));
            }
        }
        if let Some(path) = crate::lisp::load::locate_cache_source(
            i,
            &crate::editor::native_path(i, &directory)?,
            name,
            &suffixes,
        )? {
            return Ok(Some(path));
        }
    }
    Ok(None)
}

/// Suffix list GNU's `locate-library' feeds to `locate-file':
/// `get-load-suffixes' (NOSUFFIX drops it) then `load-file-rep-suffixes'.
fn locate_library_suffixes(i: &mut Interp, nosuffix: bool) -> Result<Vec<String>, Flow> {
    let str_list = |i: &mut Interp, var: &str| -> Result<Vec<String>, Flow> {
        let Some(id) = i.q_soft(var) else {
            return Ok(Vec::new());
        };
        let values = i.symbol_value(id).list_to_vec().unwrap_or_default();
        let mut strings = Vec::new();
        for value in values {
            if let Value::Str(string) = value {
                let string = string
                    .try_borrow()
                    .map_err(|_| i.error("Cannot inspect a mutably borrowed load suffix"))?;
                strings.push(string.to_unicode_lossy());
            }
        }
        Ok(strings)
    };
    let mut sufs: Vec<String> = Vec::new();
    if !nosuffix {
        // get-load-suffixes: load-suffixes with rep-suffixes attached
        // to the .elc/.el entries, then `load-file-rep-suffixes' (the
        // "" entry gives the bare-name try).
        for s in str_list(i, "load-suffixes")? {
            if s.ends_with(".elc") || s.ends_with(".el") {
                for rep in str_list(i, "load-file-rep-suffixes")? {
                    sufs.push(format!("{s}{rep}"));
                }
            } else {
                sufs.push(s);
            }
        }
    }
    sufs.extend(str_list(i, "load-file-rep-suffixes")?);
    if sufs.is_empty() {
        sufs.push(String::new());
    }
    Ok(sufs)
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
    let auto_id = i.q("autoload");
    let replaceable = match &existing {
        Value::Cons(cell) => {
            let head = crate::lisp::eval::cons_parts(i, cell)?.0;
            i.sym_is(&head, auto_id)
        }
        _ => false,
    };
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
    let auto_id = i.q("autoload");
    let macro_id = i.q("macro");
    // Macro autoload cells are (macro . (autoload ...)): GNU's
    // Fautoloadp unwraps the `macro' cons before the car check.
    let mut v = &args[0];
    let cdr;
    if let Value::Cons(c) = v {
        let (car, tail) = crate::lisp::eval::cons_parts(i, c)?;
        if i.sym_is(&car, macro_id) {
            cdr = tail;
            v = &cdr;
        }
    }
    match v {
        Value::Cons(c) => {
            let head = crate::lisp::eval::cons_parts(i, c)?.0;
            Ok(Value::from_bool(i.sym_is(&head, auto_id)))
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
    let auto_id = i.q("autoload");
    let is_auto = match &fundef {
        Value::Cons(c) => {
            let head = crate::lisp::eval::cons_parts(i, c)?.0;
            i.sym_is(&head, auto_id)
        }
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
        Value::Str(_) => filename_text_arg(i, &file)?,
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
                let cl = i.q("cl-loop");
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
    let resolved_key = match crate::lisp::load::locate_full(i, &name, false, false)? {
        Some(crate::lisp::load::Found::File(p)) => crate::lisp::load::load_key(&p),
        Some(crate::lisp::load::Found::Dir(_)) => crate::lisp::text::EmacsText::new(),
        None => {
            let mut key = crate::lisp::text::EmacsText::from("builtin:");
            key.push_str(&name);
            crate::lisp::load::load_key(&key)
        }
    };
    let loading_same = i
        .loads_in_progress
        .iter()
        .any(|f| crate::lisp::load::load_key(f) == resolved_key);
    let stem = |s: &crate::lisp::text::EmacsStr| {
        let s = s.strip_prefix("builtin:").unwrap_or(s);
        let base = s.rsplit('/').next().unwrap_or(&s);
        let base = base.strip_suffix(".el").unwrap_or(&base);
        base.strip_suffix(".elc").unwrap_or(&base).to_string()
    };
    if let Some(id) = owner {
        let pk = i.q("remacs--dump-fn");
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
                        i.princ_to_string(f)
                            .chars()
                            .take(80)
                            .collect::<crate::lisp::text::EmacsText>(),
                        i.princ_to_string(&Value::list(a.as_slice().to_vec()))
                            .chars()
                            .take(120)
                            .collect::<crate::lisp::text::EmacsText>()
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
    if feature_present(i, feat)? {
        if let Some(id) = owner {
            let pk = i.q("remacs--dump-fn");
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
                Value::Cons(c) => {
                    let head = crate::lisp::eval::cons_parts(i, c)?.0;
                    i.sym_is(&head, auto_id)
                }
                _ => false,
            };
            if still_auto {
                // Some autoload cells point at a reduced source whose
                // GNU .elc equivalent defines the function inline.  If
                // the dump-time definition was stashed for -Q parity,
                // install it: GNU resolves an autoload cell once, so
                // leaving it in place would reload the file on every
                // call (each interpreted load costs ~0.4s).
                let pk = i.q("remacs--dump-fn");
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
    let case_depth = i.case_handlers.len();
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
            .push((args[k - 1].clone(), args[k].clone(), case_depth));
        k -= 2;
    }
    let r = i.apply(&body, vec![]);
    if r.is_err() && i.handler_bindings.len() > base {
        i.rebase_quit_unwind();
    }
    i.handler_bindings.truncate(base);
    r
}

fn f_access_file(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    // (access-file FILENAME ERROR-FORMAT) — signal `file-missing' with
    // (ERROR-FORMAT ERRNO-STRING FILENAME) when FILENAME can't be read.
    let name = match &args[0] {
        Value::Str(s) => s
            .try_borrow()
            .map(|text| text.clone())
            .map_err(|_| i.error("Cannot inspect a mutably borrowed filename string"))?,
        other => return Err(i.wrong_type_mut("stringp", other)),
    };
    let expanded = crate::editor::expand_file_name_str(i, &name);
    let fmt = args.get(1).cloned().unwrap_or(Value::Nil);
    if !matches!(&fmt, Value::Str(_)) {
        return Err(i.wrong_type_mut("stringp", &fmt));
    }
    if let Some((handler, op)) = crate::editor::file_handler(i, "access-file", &[&expanded]) {
        return i.apply(
            &handler,
            vec![
                op,
                crate::editor::file_name_argument(&args[0], &expanded),
                fmt,
            ],
        );
    }
    match std::fs::File::open(crate::editor::native_path(i, &expanded)?) {
        Ok(_) => Ok(Value::Nil),
        Err(e) => {
            let estr = e.to_string();
            let estr = estr.split(" (os error").next().unwrap_or(&estr).to_string();
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
            Value::Buffer(b) => b
                .try_borrow()
                .map(|buffer| buffer.id)
                .map_err(|_| i.error("Cannot inspect a mutably borrowed buffer"))?,
            Value::Str(s) => {
                let name = s
                    .try_borrow()
                    .map(|name| name.clone())
                    .map_err(|_| i.error("Cannot inspect a mutably borrowed buffer name"))?;
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
    let (default_filename, begv) = {
        let buffer_state = buffer
            .try_borrow()
            .map_err(|_| i.error("Cannot inspect a mutably borrowed buffer"))?;
        (
            buffer_state
                .file_name
                .as_ref()
                .map(Value::string)
                .unwrap_or(Value::Nil),
            buffer_state.begv,
        )
    };
    let filename = match arg(&args, 2) {
        Value::Nil => default_filename,
        filename @ Value::Str(_) => filename,
        other => return Err(i.wrong_type_mut("stringp", &other)),
    };
    let saved = i.save_excursion_state(false);
    buffer
        .try_borrow_mut()
        .map_err(|_| i.error("Cannot update a mutably borrowed buffer"))?
        .set_point(begv);
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
    let Some(buffer) = i.buffers.get(i.current_buffer) else {
        return Err(i.error("No current buffer"));
    };
    let filename = buffer
        .try_borrow()
        .map_err(|_| i.error("Cannot inspect a mutably borrowed buffer"))?
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

fn read_buffer_state<T>(
    i: &Interp,
    buffer: &crate::lisp::value::BufferRef,
    read: impl FnOnce(&crate::buffer::Buffer) -> T,
) -> Result<T, Flow> {
    let buffer = buffer
        .try_borrow()
        .map_err(|_| i.error("Cannot inspect a mutably borrowed buffer"))?;
    Ok(read(&buffer))
}

fn update_buffer_state<T>(
    i: &Interp,
    buffer: &crate::lisp::value::BufferRef,
    update: impl FnOnce(&mut crate::buffer::Buffer) -> T,
) -> Result<T, Flow> {
    let mut buffer = buffer
        .try_borrow_mut()
        .map_err(|_| i.error("Cannot update a mutably borrowed buffer"))?;
    Ok(update(&mut buffer))
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
    let mut compiled_filename = false;
    let filename = if let Value::Str(source_name) = &filename {
        let text = source_name
            .try_borrow()
            .map(|text| text.clone())
            .map_err(|_| i.error("Cannot inspect a mutably borrowed source filename"))?;
        compiled_filename = text.ends_with(".elc");
        if crate::editor::native_path(i, &text)?.is_absolute() || text.starts_with('~') {
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
        let output = i.q("standard-output");
        let sink = if printflag.is_nil() && !allow_print {
            Value::Sym(i.q("symbolp"))
        } else {
            printflag.clone()
        };
        i.specbind(output, sink)?;
        let list = i.q("eval-buffer-list");
        let old_list = i.symbol_value(list);
        i.specbind(list, Value::cons(Value::Buffer(buffer.clone()), old_list))?;
        let lexical = i.q("lexical-binding");
        if read_lexical_cookie {
            // GNU eval-buffer only scans a cookie when the source buffer
            // has no local binding. eval-region never scans a header.
            if !buffer.borrow().locals.contains_key(&lexical) {
                let (cookie, consumed) = {
                    let b = buffer.borrow();
                    crate::lisp::load::lexical_cookie(&b.text.substring(b.begv, b.zv))
                };
                let begv = buffer.borrow().begv;
                buffer.borrow_mut().set_point(begv + consumed);
                let value = crate::lisp::load::lexical_binding_value(
                    i,
                    cookie,
                    Some(Value::Buffer(buffer.clone())),
                )?;
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
        let current = i.q("current-load-list");
        i.specbind(current, Value::list(vec![filename.clone()]))?;
        let input = i.q("standard-input");
        i.specbind(input, Value::Buffer(buffer.clone()))?;
        if compiled_filename {
            let variable = i.q("read-symbol-shorthands");
            i.specbind(variable, Value::Nil)?;
        }
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
                        && chars[pos].unicode().is_some_and(|c| {
                            matches!(c, ' ' | '\t' | '\n' | '\x0c' | '\r' | '\u{a0}')
                        })
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
                let lrf = i.q("load-read-function");
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
                    // A preceding form may call `set-buffer-multibyte'.
                    // GNU's read-from-buffer observes the live buffer mode
                    // for every object, not the mode at eval-buffer entry.
                    let source_multibyte =
                        super::readfn::buffer_source_multibyte(i, &buffer)?;
                    let (result, position) = super::readfn::read_parser_at(
                        i,
                        chars.clone(),
                        pos,
                        None,
                        true,
                        source_multibyte,
                    );
                    buffer.borrow_mut().set_point(begv + position);
                    match result {
                        Ok(Some(form)) => form,
                        Ok(None) => return Ok(None),
                        Err(Flow::Signal(mut signal)) if signal.is_symbol(sym::END_OF_FILE) => {
                            signal.set_data(Value::list(vec![Value::Buffer(buffer.clone())]));
                            return Err(Flow::Signal(signal));
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
                let values = i.q("values");
                let old = i.symbol_value(values);
                i.set_symbol(values, Value::cons(value.clone(), old))?;
                let destination = i.symbol_value(output);
                let printer = i.q(if matches!(destination, Value::Sym(s) if s == sym::T) {
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
        let pa = i.q("prefix-arg");
        let prefix = i.symbol_value(pa);
        let cpa = i.q("current-prefix-arg");
        i.set_symbol(cpa, prefix.clone())?;
        i.set_symbol(pa, Value::Nil)?;
        let upd = i.q("prefix-command-update");
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
fn f_called_interactively_p(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    if i.interactive_call_depth == 0 {
        return Ok(Value::Nil);
    }
    if i.sym_is(&arg(&args, 0), sym::INTERACTIVE) {
        let batch = i.q("noninteractive");
        let keyboard_macro = i.q("executing-kbd-macro");
        if i.symbol_value(batch).truthy() || i.symbol_value(keyboard_macro).truthy() {
            return Ok(Value::Nil);
        }
    }
    // Port subr.el's frame walk. Pending special-form frames do not count
    // as containing functions, and interactive-p is only a wrapper.
    let frames: Vec<_> = i.backtrace_snapshot().into_iter().rev().collect();
    let wrapper = i.q("interactive-p");
    let skip_hook = Value::Sym(i.q("called-interactively-p-functions"));
    let run_hook = Value::Sym(i.q("run-hook-with-args-until-success"));
    let frame_value = |frame: &crate::lisp::backtrace::Frame| {
        Value::cons(
            Value::from_bool(frame.evaluated),
            Value::cons(frame.function.clone(), frame.arguments.clone()),
        )
    };
    let mut index = 1; // Frame zero is this checker.
    while let Some(frame) = frames.get(index) {
        if !frame.evaluated || i.sym_is(&frame.function, wrapper) {
            index += 1;
            continue;
        }
        let next = frames.get(index + 1);
        let skip = i.apply(
            &run_hook,
            vec![
                skip_hook.clone(),
                Value::Int((index + 2) as i128),
                frame_value(frame),
                next.map(|frame| frame_value(frame)).unwrap_or(Value::Nil),
            ],
        )?;
        if !skip.is_nil() {
            let Some(count) = skip.int().filter(|count| *count >= 0) else {
                return Err(i.wrong_type_mut("natnump", &skip));
            };
            let count = usize::try_from(count).unwrap_or(usize::MAX);
            index = index.saturating_add(count.saturating_add(1));
            continue;
        }
        let caller = i.indirect_function_value(&frame.function);
        // This bridge represents GNU's Lisp interactive-p wrapper, and
        // native-compiled subrs are functions rather than C primitives.
        if matches!(&caller, Value::Subr(subr)
            if subr.name != "interactive-p" && super::comp::is_native(subr).is_none())
        {
            return Ok(Value::Nil);
        }
        let Some(next) = next else {
            return Ok(Value::Nil);
        };
        let dispatcher = i.indirect_function_value(&next.function);
        let original = i.q("internal--funcall-interactively");
        return Ok(Value::from_bool(super::eq_values(
            &dispatcher,
            &i.symbol_value(original),
        )));
    }
    Ok(Value::Nil)
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
        _ => {}
    }
    if let Value::Str(s) = &args[0] {
        let empty = s
            .try_borrow()
            .map(|text| text.is_empty())
            .map_err(|_| i.error("Cannot inspect a mutably borrowed message string"))?;
        if empty {
            return Ok(args[0].clone());
        }
    }
    let v = super::strfn::f_format_message(i, args)?;
    let (msg, properties) = match &v {
        Value::Str(s) => {
            let message = s
                .try_borrow()
                .map(|text| text.clone())
                .map_err(|_| i.error("Cannot inspect a mutably borrowed message string"))?;
            (message, i.str_props(s).to_vec())
        }
        other => (i.princ_to_string(other), Vec::new()),
    };

    // GNU calls `set-message-function' before displaying a Lisp `message'.
    // The handler can display it elsewhere (for example, as an overlay in
    // the active minibuffer), replace its text, or suppress the echo-area
    // display by returning a non-string non-nil value.  Rust-originated
    // messages keep using `Interp::message' directly.
    let mut display_message = msg.clone();
    let mut display_properties = properties;
    let mut display = true;
    if !i.boot_loading {
        let handler = i
            .q_soft("set-message-function")
            .filter(|symbol| i.bound_p(*symbol))
            .map(|symbol| i.symbol_value(symbol));
        if let Some(handler) =
            handler.filter(|value| crate::lisp::builtins::data::functionp_val(i, value))
        {
            match i.apply(&handler, vec![v.clone()])? {
                Value::Nil => {}
                Value::Str(s) => {
                    display_message = s
                        .try_borrow()
                        .map(|text| text.clone())
                        .map_err(|_| i.error("Cannot inspect a mutably borrowed message string"))?;
                    display_properties = i.str_props(&s).to_vec();
                }
                _ => display = false,
            }
        }
    }

    if display {
        i.message_with_properties(&display_message, display_properties);
    } else {
        // A handled message is still recorded in *Messages*.
        i.message_log(&msg);
    }
    Ok(v)
}
pub(crate) fn f_ding(i: &mut Interp, _args: crate::lisp::Argv) -> EvalResult {
    // GNU gives a user-provided ring-bell-function precedence over both
    // visible-bell and the host's audible bell.
    let ring_bell_function = i
        .q_soft("ring-bell-function")
        .filter(|symbol| i.bound_p(*symbol))
        .map(|symbol| i.symbol_value(symbol))
        .unwrap_or(Value::Nil);
    if ring_bell_function.truthy() {
        i.apply(&ring_bell_function, Vec::<Value>::new())?;
        return Ok(Value::Nil);
    }

    // GNU's Fding: noninteractive sessions write \a to stdout.  It
    // stays in stdio's block buffer, so a piped reader sees it merged
    // with the next stdout write.
    if i.noninteractive {
        crate::stdio::write(b"\x07");
        return Ok(Value::Nil);
    }

    let visible = i
        .q_soft("visible-bell")
        .filter(|symbol| i.bound_p(*symbol))
        .is_some_and(|symbol| i.symbol_value(symbol).truthy());
    if let Some(backend) = i.bell_backend.clone() {
        let frame_id = i.selected_frame.as_ref().map(|frame| frame.borrow().id);
        backend.ring_bell(frame_id, visible);
    } else {
        crate::stdio::write(b"\x07");
    }
    Ok(Value::Nil)
}
/// keyboard.c `timer_check': run each due, untriggered timer on
/// `timer-list' through `timer-event-handler' (which reschedules
/// repeat timers and swallows handler errors itself).  GNU never
/// fires idle timers without an input loop, so `timer-idle-list' is
/// intentionally not consulted here.
pub(crate) fn timer_check(i: &mut Interp) -> Result<bool, Flow> {
    let tl = i.q("timer-list");
    let list = i.symbol_value(tl);
    // Snapshot: the handler mutates `timer-list' while running.
    let timers = list.list_to_vec().unwrap_or_default();
    if timers.is_empty() {
        return Ok(false);
    }
    let handler = i.q("timer-event-handler");
    if let Value::Sym(s) = i.symbol_function(handler) {
        if s == crate::lisp::sym::UNBOUND {
            return Ok(false);
        }
    }
    let now = super::misc::lisp_time_to_ns(i, &Value::Nil)?;
    let mut did_work = false;
    for timer in timers {
        // Slots: [triggered high low usec repeat function args idle psec
        //         integral-multiple]
        let when = match &timer {
            Value::Vec(v) => {
                let v = v
                    .try_borrow()
                    .map_err(|_| i.error("Cannot inspect a mutably borrowed timer"))?;
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
                let mut v = v
                    .try_borrow_mut()
                    .map_err(|_| i.error("Cannot update a mutably borrowed timer"))?;
                let Some(triggered) = v.first_mut() else {
                    return Err(i.error("Invalid timer vector"));
                };
                *triggered = Value::t();
            }
            i.apply(&Value::Sym(handler), vec![timer])?;
            did_work = true;
        }
    }
    Ok(did_work)
}

/// The next active ordinary timer bounds wait_reading_process_output's
/// select timeout, even when the caller requested an enormous delay.
pub(crate) fn next_timer_delay(i: &mut Interp) -> Option<f64> {
    let symbol = i.q("timer-list");
    let timers = i.symbol_value(symbol).list_to_vec().ok()?;
    let now = super::misc::lisp_time_to_ns(i, &Value::Nil).ok()?;
    timers
        .into_iter()
        .filter_map(|timer| {
            let when = match timer {
                Value::Vec(value) => {
                    let value = value.try_borrow().ok()?;
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
pub(crate) fn thread_wait_events(i: &mut Interp) -> Result<bool, Flow> {
    thread_wait_events_with_timer_policy(i, true)
}

/// Pump frontend/process events without firing Lisp timers. GNU does not run
/// a due timer for a zero-duration `sit-for', but its frontend still needs a
/// nonblocking input poll and redisplay.
pub(crate) fn thread_wait_events_without_timers(i: &mut Interp) -> Result<bool, Flow> {
    thread_wait_events_with_timer_policy(i, false)
}

fn thread_wait_events_with_timer_policy(i: &mut Interp, check_timers: bool) -> Result<bool, Flow> {
    let mut did_work = crate::lisp::eval::threads::drain_main_events(i)?;
    if check_timers {
        did_work |= timer_check(i)?;
    }
    did_work |= crate::lisp::process::poll_all(i)?;
    did_work |= crate::lisp::builtins::filenotify::drain(i)?;
    did_work |= crate::lisp::builtins::dbus::drain(i)?;
    Ok(did_work)
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

pub(crate) fn f_sleep_for(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
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
    // Arg is CHECK_NUMBER'd the same as `sleep-for'.
    let secs = match args.get(0) {
        Some(Value::Int(n)) => *n as f64,
        Some(Value::Float(f)) => **f,
        Some(other) => return Err(i.wrong_type_mut("numberp", other)),
        None => 0.0,
    };
    if i.sit_for_waiter.is_some() && crate::editor::check_input_pending(i, false)? {
        return Ok(Value::Nil);
    }
    if let Some(waiter) = i.sit_for_waiter.clone() {
        let duration = if secs.is_finite() && secs > 0.0 {
            std::time::Duration::from_secs_f64(secs.min(3600.0))
        } else {
            std::time::Duration::ZERO
        };
        let input_pending = waiter(
            i,
            duration,
            SitForOptions {
                process_timers: secs > 0.0,
                redisplay: !arg(&args, 1).truthy(),
            },
        )?;
        return Ok(Value::from_bool(!input_pending));
    }
    // Batch has no interactive frontend to poll, so it waits the full time
    // while still servicing timers, subprocesses, and other runtime events.
    sleep_firing_timers(i, secs)?;
    Ok(Value::t())
}
fn f_current_time(i: &mut Interp, _args: crate::lisp::Argv) -> EvalResult {
    let us = super::misc::lisp_time_to_us(i, &Value::Nil)?;
    // GNU `make_lisp_time': `current-time-list' nil renders as
    // (TICKS . HZ) with nanosecond ticks.
    if !super::misc::current_time_list(i) {
        return Ok(Value::cons(
            Value::Int(us * 1000),
            Value::Int(1_000_000_000),
        ));
    }
    Ok(super::misc::us_to_lisp_time(us))
}

fn f_get_internal_run_time(i: &mut Interp, _args: crate::lisp::Argv) -> EvalResult {
    let elapsed = i.runtime_started.elapsed();
    let micros = (elapsed.as_secs() as i128) * 1_000_000 + elapsed.subsec_micros() as i128;
    Ok(super::misc::us_to_lisp_time(micros))
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
    fn offset_name(off: i128) -> String {
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
    let t_sym = i.q("t");
    let wall_sym = i.q("wall");
    let (off, name) = match args.get(1) {
        // GNU tzlookup treats numeric zero like t, not a named +00 offset.
        Some(Value::Int(0)) => (0, "UTC".to_string()),
        Some(Value::Int(off)) if super::misc::valid_fixed_zone_offset(*off) => {
            (*off as i64, offset_name(*off))
        }
        Some(Value::Int(_)) => (0, "UTC".to_string()),
        Some(Value::Sym(s)) if *s == t_sym => (0, "UTC".to_string()),
        Some(Value::Sym(s)) if *s == wall_sym => {
            // `wall' = startup wall clock, unaffected by
            // `set-time-zone-rule' (GNU's `wall_tz').
            let tm = super::misc::local_tm(secs);
            (tm.tm_gmtoff, tm_zone_name(&tm))
        }
        Some(Value::Str(z)) => {
            let z = z
                .try_borrow()
                .map(|zone| zone.clone())
                .map_err(|_| i.error("Cannot inspect a mutably borrowed time-zone string"))?;
            if z == "UTC" || z == "t" {
                (0, "UTC".to_string())
            } else {
                let (tm, name) = super::misc::tz_local_tm(secs, &(z).to_unicode_lossy());
                (tm.tm_gmtoff, name)
            }
        }
        Some(zone @ Value::Cons(_)) => {
            let (off, name) = super::misc::checked_time_zone_cons(i, zone)?;
            let off = off as i64;
            (
                off,
                if name.is_empty() {
                    offset_name(off as i128)
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
        Value::Str(s) => s
            .try_borrow()
            .map(|format| format.clone())
            .map_err(|_| i.error("Cannot inspect a mutably borrowed time-format string"))?,
        other => return Err(i.wrong_type_mut("stringp", other)),
    };
    let secs = i64::try_from(t.div_euclid(1_000_000_000_000))
        .map_err(|_| i.error("Specified time is not representable"))?;
    // sub-second digits as a 12-digit picosecond fraction.
    let frac12 = t.rem_euclid(1_000_000_000_000);
    // GNU (format-time-string FORMAT TIME ZONE): nil/`wall' = local,
    // integer = fixed offset, string = TZ spec, `t' is UTC; anything
    // else signals "Invalid time zone specification".
    let t_sym = i.q("t");
    let wall_sym = i.q("wall");
    let (tm, zlabel) = match args.get(2) {
        Some(Value::Int(0)) => (
            super::misc::gmt_tm(secs),
            ZoneLabel::Named("UTC".to_string()),
        ),
        Some(Value::Int(off)) if super::misc::valid_fixed_zone_offset(*off) => {
            let offset = *off as i64;
            let Some(local_seconds) = secs.checked_add(offset) else {
                return Err(i.error("Specified time is not representable"));
            };
            let mut tm = super::misc::gmt_tm(local_seconds);
            tm.tm_gmtoff = offset;
            (tm, ZoneLabel::Offset(offset))
        }
        Some(Value::Int(_)) => (
            super::misc::gmt_tm(secs),
            ZoneLabel::Named("UTC".to_string()),
        ),
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
            let z = z
                .try_borrow()
                .map(|zone| zone.clone())
                .map_err(|_| i.error("Cannot inspect a mutably borrowed time-zone string"))?;
            if z == "UTC" || z == "t" {
                let mut tm = super::misc::gmt_tm(secs);
                tm.tm_gmtoff = 0;
                (tm, ZoneLabel::Named("UTC".to_string()))
            } else {
                // GNU decode_time_zone: the TZ spec sets the zone and
                // %Z prints the resolved tm_zone abbreviation (e.g.
                // "UTC0" → "UTC"), not the spec text.  The name is
                // copied by tz_local_tm before TZ is restored.
                let (tm, name) = super::misc::tz_local_tm(secs, &(z).to_unicode_lossy());
                (tm, ZoneLabel::Named(name))
            }
        }
        // GNU's decode_time_zone accepts `current-time-zone' output —
        // a (OFFSET NAME) cons — and uses the car as the fixed offset.
        Some(zone @ Value::Cons(_)) => {
            let (off, name) = super::misc::checked_time_zone_cons(i, zone)?;
            let off = off as i64;
            let Some(local_seconds) = secs.checked_add(off) else {
                return Err(i.error("Specified time is not representable"));
            };
            let mut tm = super::misc::gmt_tm(local_seconds);
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
    // gmtime/localtime return null when the platform cannot represent a
    // time_t as `struct tm`; the zeroed result has tm_mday == 0.
    if tm.tm_mday == 0 {
        return Err(i.error("Specified time is not representable"));
    }
    // `system-time-locale' overrides LC_TIME for this call, falling
    // back to "C" for names the C library rejects (like GNU).
    let locale: Option<String> = match i
        .q_soft("system-time-locale")
        .filter(|id| i.bound_p(*id))
        .map(|id| i.symbol_value(id))
    {
        Some(Value::Str(s)) => Some(
            s.try_borrow()
                .map(|locale| locale.to_unicode_lossy())
                .map_err(|_| i.error("Cannot inspect a mutably borrowed system-time-locale"))?,
        ),
        _ => None,
    };
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
        &(fmt).to_unicode_lossy(),
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
pub(crate) fn f_kill_emacs(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    let restart = args.get(1).is_some_and(Value::truthy);
    if restart {
        use crate::lifecycle::RestartError;
        crate::lifecycle::validate_restart().map_err(|error| match error {
            RestartError::NoArguments => i.error_q::<str>(
                "No command line arguments known; unable to re-execute Emacs",
                &[],
            ),
            RestartError::UnknownExecutable => i.error_q::<str>("Unknown Emacs executable", &[]),
            RestartError::MissingExecutable(name) => i.error_q(
                "Emacs executable \"%s\" can't be found",
                &[&name.to_string_lossy()],
            ),
        })?;
    }
    // GNU reports/removes failing batch hooks and continues with the rest.
    // Interactive shutdown uses subr.el's timed confirmation wrapper.
    let hook = i.q("kill-emacs-hook");
    if i.symbol_value(hook).truthy() {
        if i.noninteractive {
            safe_call_hook(i, "kill-emacs-hook")?;
        } else {
            let runner = Value::Sym(i.q("run-hook-query-error-with-timeout"));
            i.apply(&runner, vec![Value::Sym(hook)])?;
        }
    }
    i.quit_editor = true;
    // Both front ends stop evaluating Lisp once shutdown has been accepted.
    let code = match args.first() {
        Some(Value::Int(n)) => *n,
        // GNU only stuffs a string into a supported interactive terminal;
        // it does not print it as a shutdown diagnostic.
        _ => 0,
    };
    crate::lifecycle::accept_shutdown(code, restart);
    Err(Flow::Exit(code))
}
fn f_recursive_edit(i: &mut Interp, _args: crate::lisp::Argv) -> EvalResult {
    // GNU's recursive_edit_1 specbinds command_loop_level and installs
    // an `exit' catch around a command loop; both unwind when it
    // returns.  There's no callable command loop here yet, so just
    // keep the depth/catch scoped like the specbind.
    i.recursion_depth += 1;
    let exit_sym = Value::Sym(i.q("exit"));
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
        let ue = i.q("user-error");
        return Err(i.signal_data(ue, vec![Value::string("No recursive edit is in progress")]));
    }
    Err(Flow::Throw(Value::Sym(i.q("exit")), Value::Nil))
}
fn f_abort_recursive_edit(i: &mut Interp, _args: crate::lisp::Argv) -> EvalResult {
    // GNU: `(throw 'exit t)' — the `exit' catch maps t to `quit'.
    if i.recursion_depth == 0 && i.minibuf_level <= 0 {
        let ue = i.q("user-error");
        return Err(i.signal_data(ue, vec![Value::string("No recursive edit is in progress")]));
    }
    Err(Flow::Throw(Value::Sym(i.q("exit")), Value::t()))
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
    let sym = i.q("set-message-functions");
    run_hook_functions(i, &Value::Sym(sym), |i, f| {
        if matches!(msg, Value::Str(_)) {
            let ret = i.apply(&f, vec![msg.clone()])?;
            if ret.truthy() {
                msg = ret;
            }
        }
        Ok(Value::Nil)
    })?;
    Ok(msg)
}
fn f_macroexp_parse_body(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    let items = want_list(i, &args[0])?;
    // split leading (declare ...) forms and strings
    let mut declares = Vec::new();
    let mut rest_start = items.len();
    for (k, f) in items.iter().enumerate() {
        let is_decl = match f {
            Value::Cons(c) => {
                let decl_id = i.q("declare");
                let head = crate::lisp::eval::cons_parts(i, c)?.0;
                i.sym_is(&head, decl_id)
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

fn f_byte_code_function_p(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    // `#[...]' byte-code literals (string code slot) answer t;
    // interpreted closures (cons code slot), lambdas AND native subrs
    // answer nil (GNU's nativecomp build: subrp ≠ byte-code-function-p).
    let bytecode = match &args[0] {
        Value::Lambda(lambda) => lambda
            .try_is_bytecode()
            .map_err(|_| i.error("Cannot inspect a mutably borrowed byte-code object"))?,
        _ => false,
    };
    Ok(Value::from_bool(bytecode))
}
fn f_compiled_function_p(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    let compiled = match &args[0] {
        Value::Subr(_) => true,
        // GNU nativecomp reports every dumped def as compiled; our
        // twins are `compiled' interpreted lambdas.  Macros don't
        // count (GNU's `compiled-function-p' is nil on them too —
        // `subrp' excludes them, but plain lambdas don't reach here).
        Value::Lambda(lambda) => {
            (lambda.compiled
                || lambda
                    .try_is_bytecode()
                    .map_err(|_| i.error("Cannot inspect a mutably borrowed byte-code object"))?)
                && !lambda.is_macro
        }
        _ => false,
    };
    Ok(Value::from_bool(compiled))
}
fn f_interactive_p(i: &mut Interp, _args: crate::lisp::Argv) -> EvalResult {
    // Keep the GNU Lisp wrapper's distinct frame, including through aliases.
    let checker = Value::Sym(i.q("called-interactively-p"));
    i.apply(&checker, vec![Value::Sym(sym::INTERACTIVE)])
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
    let depth = args[2]
        .int()
        .and_then(|depth| usize::try_from(depth).ok())
        .ok_or_else(|| i.error("Invalid byte-code"))?;
    crate::lisp::bytecode::exec_raw(i, &code, &args[1], depth)
}

fn f_make_byte_code(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    // GNU `make-byte-code': pack ARGLIST BYTE-CODE CONSTANTS DEPTH
    // (and optional doc/interactive slots) into a `#[...]' object.
    // The Lambda keeps the raw slots in `bc_items' so `aref'/`arrayp'
    // see GNU's element view and `bytecode::exec' can run it.
    let argdesc = args[0].clone();
    // GNU's `validate_byte_code' (comp.c): ARGLIST must be a fixnum
    // descriptor or a list, BYTE-CODE must be unibyte, CONSTANTS a
    // vector, and DEPTH a non-negative fixnum.  List contents and the
    // trailing docstring/interactive slots are checked when used.
    let valid_argdesc = argdesc.fixnump() || matches!(&argdesc, Value::Nil | Value::Cons(_));
    if !valid_argdesc
        || !valid_byte_code_fields(&args[1], &args[2], &args[3])
        || super::strfn::string_multibyte_p(i, &args[1])
    {
        return Err(i.error("Invalid byte-code object"));
    }
    let doc = match args.get(4) {
        Some(Value::Str(s)) => Some(
            s.try_borrow()
                .map(|text| text.to_unicode_lossy())
                .map_err(|_| i.error("Cannot inspect a mutably borrowed byte-code docstring"))?,
        ),
        _ => None,
    };
    let interactive = args.get(5).map(|spec| {
        // GNU's `interactive-form'/callint read byte-code slot 5 as the
        // bare spec inside an implicit `(interactive ...)'; every other
        // use of `Lambda.interactive' expects that wrapper.
        Value::list(vec![Value::Sym(i.q("interactive")), spec.clone()])
    });
    Ok(Value::Lambda(std::rc::Rc::new(crate::lisp::value::Lambda {
        is_macro: false,
        required: Vec::new(),
        optional: Vec::new(),
        rest: None,
        body: Vec::new(),
        body_value: None,
        env: None,
        doc: doc.map(Into::into),
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
        compiled_macro_cache: crate::lisp::value::CompiledMacroCache::default(),
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
/// `internal--track-mouse` — GNU temporarily enables `track-mouse' while
/// calling BODYFN, then restores the previous value even when BODYFN signals.
fn f_internal_track_mouse(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let track_mouse = i.q("track-mouse");
    let previous = i.symbol_value(track_mouse);
    // GNU's keyboard.c updates this forwarded variable directly and restores
    // it with an unwind-protect, so avoid Lisp-level variable watcher calls.
    i.store_internal_value(track_mouse, Value::t());
    let result = i.apply(&a[0], vec![]);
    i.store_internal_value(track_mouse, previous);
    result
}

/// `save-mark-and-excursion--save` → (MARKER . ACTIVE) — `(nil)' when
/// the mark isn't set, like GNU.
fn f_smae_save(i: &mut Interp, _a: crate::lisp::Argv) -> EvalResult {
    let buf = i.current_buffer;
    let Some(b) = i.buffers.get(buf) else {
        return Ok(Value::cons(Value::Nil, Value::Nil));
    };
    let marker = b
        .try_borrow_mut()
        .map_err(|_| i.error("Cannot inspect a mutably borrowed buffer"))?
        .mark_marker();
    let mark = {
        let mark = marker
            .try_borrow()
            .map_err(|_| i.error("Cannot inspect a mutably borrowed mark marker"))?;
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
                buffer
                    .try_borrow_mut()
                    .map_err(|_| i.error("Cannot update a mutably borrowed buffer"))?
                    .register_marker(&mk);
            }
            Value::Marker(mk)
        }
        None => Value::Nil,
    };
    let active = i.q("mark-active");
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
        Value::Cons(c) => crate::lisp::eval::cons_parts(i, c)?,
        _ => (Value::Nil, Value::Nil),
    };
    let buf = i.current_buffer;
    let Some(b) = i.buffers.get(buf) else {
        return Ok(Value::Nil);
    };
    // GNU: (omark (marker-position (mark-marker))) read before mutating.
    let (omark, markm) = {
        let mut bb = b
            .try_borrow_mut()
            .map_err(|_| i.error("Cannot inspect a mutably borrowed buffer"))?;
        let m = bb.mark_marker();
        let pos = {
            let mm = m
                .try_borrow()
                .map_err(|_| i.error("Cannot inspect a mutably borrowed mark marker"))?;
            mm.buffer.is_some().then(|| mm.position)
        };
        (pos, m)
    };
    let nmark = if saved_mark.is_nil() {
        // (set-marker (mark-marker) nil): detach the mark marker.
        markm
            .try_borrow_mut()
            .map_err(|_| i.error("Cannot update a mutably borrowed mark marker"))?
            .buffer = None;
        None
    } else {
        let mm = match &saved_mark {
            Value::Marker(m) => m.clone(),
            other => return Err(i.wrong_type_mut("markerp", other)),
        };
        // (setf nmark (marker-position saved-mark)) then
        // (set-marker (mark-marker) nmark): a detached saved marker
        // yields nil → the mark marker is detached too.
        let np = {
            let marker = mm
                .try_borrow()
                .map_err(|_| i.error("Cannot inspect a mutably borrowed saved marker"))?;
            marker.buffer.map(|_| marker.position)
        };
        match np {
            Some(p) => {
                let len = b
                    .try_borrow()
                    .map_err(|_| i.error("Cannot inspect a mutably borrowed buffer"))?
                    .text
                    .len();
                b.try_borrow_mut()
                    .map_err(|_| i.error("Cannot update a mutably borrowed buffer"))?
                    .set_mark(p.min(len));
            }
            None => {
                markm
                    .try_borrow_mut()
                    .map_err(|_| i.error("Cannot update a mutably borrowed mark marker"))?
                    .buffer = None;
            }
        }
        // (set-marker saved-mark nil): saved marker is detached.
        mm.try_borrow_mut()
            .map_err(|_| i.error("Cannot update a mutably borrowed saved marker"))?
            .buffer = None;
        np
    };
    // Read activation after moving the mark, then assign through normal
    // variable binding/watch semantics, preserving a non-boolean value.
    let ma = i.q("mark-active");
    let cur_active = i.symbol_value(ma);
    i.set_symbol(ma, saved_active.clone())?;
    // GNU runs activate-mark-hook when the restored flag is non-nil and
    // the mark moved or was inactive; deactivate-mark-hook when the flag
    // went nil from non-nil.
    let rh = i.q("run-hooks");
    if saved_active.truthy() {
        if !cur_active.truthy() || omark != nmark {
            let h = Value::Sym(i.q("activate-mark-hook"));
            i.apply(&Value::Sym(rh), vec![h])?;
        }
    } else if cur_active.truthy() {
        let h = Value::Sym(i.q("deactivate-mark-hook"));
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
    ":interactive-only",
];

/// The `name' property of an advice PROPS alist.
fn advice_name(i: &mut Interp, props: &Value) -> Result<Value, Flow> {
    let name_kw = i.q("name");
    let mut cur = props.clone();
    let mut seen = FxHashSet::default();
    while let Value::Cons(c) = cur.clone() {
        if !seen.insert(Rc::as_ptr(&c) as usize) {
            return Err(crate::lisp::builtins::listfn::err_circular(i, &cur));
        }
        let (car, cdr) = crate::lisp::eval::cons_parts(i, &c)?;
        if let Value::Cons(e) = &car {
            let (k, v) = crate::lisp::eval::cons_parts(i, e)?;
            if let Value::Sym(s) = k {
                if s == name_kw {
                    return Ok(v);
                }
            }
        }
        cur = cdr;
    }
    Ok(Value::Nil)
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
    let name = advice_name(i, &arg(&a, 3))?;
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
    let orig_key = i.q("cl--advice--orig");
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
fn advice_unwrap(i: &mut Interp, v: &Value) -> Result<Option<(SymId, Value)>, Flow> {
    if let Value::Sym(k) = v {
        let orig_key = i.q("cl--advice--orig");
        if let Value::Cons(c) = i.get_prop(*k, orig_key) {
            return Ok(Some((*k, crate::lisp::eval::cons_parts(i, &c)?.0)));
        }
    }
    Ok(None)
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
            let sf = i.q("(setf quote)");
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
    let name = advice_name(i, &arg(&a, 3))?;
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
            match advice_unwrap(i, &cur)? {
                Some((k, orig)) => (Some(("var".to_string(), sym, orig, sym)), k),
                None => return Ok(Value::Nil),
            }
        }
        Some("get") => {
            let sym = want_sym(i, &p[1])?;
            let prop = want_sym(i, &p[2])?;
            let current = i.get_prop(sym, prop);
            match advice_unwrap(i, &current)? {
                Some((k, orig)) => (Some(("get".to_string(), sym, orig, prop)), k),
                None => return Ok(Value::Nil),
            }
        }
        Some("setf-quote") => {
            let sf = i.q("(setf quote)");
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
    let name_kw = i.q("name");
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
            let name_kw = i.q("name");
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
            // `nadvice.el' expands this as `(apply NEXT (funcall ADVICE
            // ARGS))'.  A non-list or dotted result must therefore keep
            // `apply''s listp error instead of becoming one argument.
            let argv = want_list(i, &r)?;
            i.apply(&next, argv)
        }
        ":filter-return" => {
            let r = i.apply(&next, args)?;
            i.apply(&f, vec![r])
        }
        ":interactive-only" => i.apply(&next, args),
        _ => Err(i.error("Invalid advice class")),
    }
}

#[cfg(test)]
mod modification_hook_binding_tests {
    use super::*;

    #[test]
    fn change_hook_binding_errors_unwind_without_losing_pending_hooks() {
        std::thread::Builder::new()
            .stack_size(64 * 1024 * 1024)
            .spawn(|| {
                let mut interp = Interp::new();
                interp
                    .eval_str(
                        "(defun change-hook-binding-watcher (_symbol _value operation _where)\
                           (if (eq operation 'let) (signal 'file-error '(\"bind\")) nil))\
                         (add-variable-watcher 'inhibit-modification-hooks\
                                               #'change-hook-binding-watcher)",
                    )
                    .unwrap();

                let mark = interp.specbind_depth();
                let before = signal_before_change(&mut interp, 1, 1);
                assert!(
                    matches!(before, Err(Flow::Signal(ref signal)) if signal.is_symbol(sym::FILE_ERROR))
                );
                assert_eq!(interp.specbind_depth(), mark);

                interp.pending_insert_behind = vec![Value::Int(11)];
                interp.pending_insert_front = vec![Value::Int(22)];
                let after = signal_after_change(&mut interp, 1, 1, 0);
                assert!(
                    matches!(after, Err(Flow::Signal(ref signal)) if signal.is_symbol(sym::FILE_ERROR))
                );
                assert_eq!(interp.specbind_depth(), mark);
                assert!(matches!(
                    interp.pending_insert_behind.as_slice(),
                    [Value::Int(11)]
                ));
                assert!(matches!(
                    interp.pending_insert_front.as_slice(),
                    [Value::Int(22)]
                ));
            })
            .unwrap()
            .join()
            .unwrap();
    }
}

#[cfg(test)]
mod macroexpand_scope_binding_tests {
    use super::*;

    #[test]
    fn macroexpand_all_unwinds_a_failed_dynvars_binding() {
        std::thread::Builder::new()
            .stack_size(128 * 1024 * 1024)
            .spawn(|| {
                let mut interp = Interp::new();
                interp
                    .eval_str(
                        "(defun macroexpand-scope-watcher (_symbol _value operation _where)\
                           (if (eq operation 'let) (signal 'file-error '(\"bind\")) nil))\
                         (add-variable-watcher 'macroexp--dynvars #'macroexpand-scope-watcher)",
                    )
                    .unwrap();
                let (form, _) = interp.read_from_string("(let ((x 1)) x)", 0).unwrap();
                let mark = interp.specbind_depth();
                let result = macroexpand_all_inner(&mut interp, &form);
                assert!(
                    matches!(result, Err(Flow::Signal(ref signal)) if signal.is_symbol(sym::FILE_ERROR)),
                    "unexpected macroexpand_all result: {result:?}"
                );
                assert_eq!(interp.specbind_depth(), mark);
            })
            .unwrap()
            .join()
            .unwrap();
    }

    #[test]
    fn filtered_symbol_macro_scope_unwinds_a_failed_environment_binding() {
        std::thread::Builder::new()
            .stack_size(128 * 1024 * 1024)
            .spawn(|| {
                let mut interp = Interp::new();
                let env = interp.q("macroexpand-all-environment");
                let shadowed = interp.intern("macroexpand-filtered-env-variable");
                let watcher = Value::Sym(interp.q("signal"));
                interp.var_watchers.push((env, watcher));
                let mark = interp.specbind_depth();
                let result = expand_body_filtered(
                    &mut interp,
                    env,
                    &Value::Nil,
                    true,
                    &[shadowed],
                    &Value::Sym(shadowed),
                );
                assert!(
                    matches!(result, Err(Flow::Signal(_))),
                    "expected the variable watcher to signal, got {result:?}"
                );
                assert_eq!(interp.specbind_depth(), mark);
            })
            .unwrap()
            .join()
            .unwrap();
    }

    #[test]
    fn macroexpand_all_unwinds_failed_let_init_and_body_environment_bindings() {
        std::thread::Builder::new()
            .stack_size(128 * 1024 * 1024)
            .spawn(|| {
                for source in [
                    "(let ((macroexpand-filtered-env-variable 1)) macroexpand-filtered-env-variable)",
                    "(let nil macroexpand-filtered-env-variable)",
                ] {
                    let mut interp = Interp::new();
                    let env_id = interp.q("macroexpand-all-environment");
                    let keyword = interp.q(":cl-symbol-macros");
                    let shadowed = interp.intern("macroexpand-filtered-env-variable");
                    let env = Value::list(vec![Value::cons(
                        Value::Sym(keyword),
                        Value::list(vec![Value::cons(Value::Sym(shadowed), Value::Int(9))]),
                    )]);
                    let base = interp.specbind_depth();
                    interp.specbind(env_id, env).unwrap();
                    let env_depth = interp.specbind_depth();
                    let watcher = Value::Sym(interp.q("signal"));
                    interp.var_watchers.push((env_id, watcher));
                    let (form, _) = interp.read_from_string(source, 0).unwrap();
                    let result = macroexpand_all_inner(&mut interp, &form);
                    assert!(
                        matches!(result, Err(Flow::Signal(_))),
                        "expected the variable watcher to signal for {source}, got {result:?}"
                    );
                    assert_eq!(
                        interp.specbind_depth(),
                        env_depth,
                        "macroexpand-all leaked a nested binding for {source}"
                    );
                    interp.var_watchers.retain(|(id, _)| *id != env_id);
                    interp.unbind_to(base).unwrap();
                }
            })
            .unwrap()
            .join()
            .unwrap();
    }
}

#[cfg(test)]
mod eval_expression_value_tests {
    use super::*;

    #[test]
    fn eval_expression_propagates_a_values_watcher_error() {
        let mut interp = Interp::new();
        interp
            .eval_str(
                "(defun eval-expression-values-watcher (_symbol _value operation _where)\
                   (if (eq operation 'set) (signal 'file-error '(\"values\")) nil))\
                 (add-variable-watcher 'values #'eval-expression-values-watcher)",
            )
            .unwrap();
        let result = f_eval_expression(&mut interp, vec![Value::Int(7), Value::Nil].into());
        assert!(
            matches!(result, Err(Flow::Signal(ref signal)) if signal.is_symbol(sym::FILE_ERROR)),
            "unexpected eval-expression result: {result:?}"
        );
    }
}

#[cfg(test)]
mod locate_library_message_tests {
    use super::*;

    #[test]
    fn interactive_locate_library_propagates_message_errors() {
        std::thread::Builder::new()
            .stack_size(128 * 1024 * 1024)
            .spawn(|| {
                let mut interp = Interp::new();
                interp
                    .eval_str(
                        "(defun qa-locate-library-message (_format _argument)\
                           (signal 'file-error '(\"locate-library message\")))\
                         (fset 'message #'qa-locate-library-message)",
                    )
                    .unwrap();
                let result = f_locate_library(
                    &mut interp,
                    vec![Value::string("simple"), Value::Nil, Value::Nil, Value::t()].into(),
                );
                assert!(matches!(
                    result,
                    Err(Flow::Signal(ref signal)) if signal.is_symbol(sym::FILE_ERROR)
                ));
            })
            .unwrap()
            .join()
            .unwrap();
    }
}

#[cfg(test)]
mod advice_filter_args_tests {
    use super::*;

    #[test]
    fn rust_advice_trampoline_requires_filter_args_to_return_a_proper_list() {
        std::thread::Builder::new()
            .stack_size(128 * 1024 * 1024)
            .spawn(|| {
                let mut interp = Interp::new();
                interp
                    .eval_str(
                        "(defun qa-rust-advice-target (x) x)\
                         (defun qa-rust-advice-non-list (_args) 7)\
                         (defun qa-rust-advice-dotted (_args) '(1 . bad))",
                    )
                    .unwrap();
                let target = interp.q("qa-rust-advice-target");
                let how = interp.q(":filter-args");
                let non_list = interp.q("qa-rust-advice-non-list");
                let dotted = interp.q("qa-rust-advice-dotted");

                f_advice_add(
                    &mut interp,
                    vec![Value::Sym(target), Value::Sym(how), Value::Sym(non_list)].into(),
                )
                .unwrap();
                let result = interp.apply(&Value::Sym(target), vec![Value::Int(3)]);
                let signal = match result {
                    Err(Flow::Signal(signal)) => signal,
                    other => panic!("non-list filter result did not signal: {other:?}"),
                };
                assert!(signal.is_symbol(sym::WRONG_TYPE_ARGUMENT));
                assert_eq!(interp.prin1_to_string(&signal.data()), "(listp 7)");
                f_advice_remove(
                    &mut interp,
                    vec![Value::Sym(target), Value::Sym(non_list)].into(),
                )
                .unwrap();

                f_advice_add(
                    &mut interp,
                    vec![Value::Sym(target), Value::Sym(how), Value::Sym(dotted)].into(),
                )
                .unwrap();
                let result = interp.apply(&Value::Sym(target), vec![Value::Int(3)]);
                let signal = match result {
                    Err(Flow::Signal(signal)) => signal,
                    other => panic!("dotted filter result did not signal: {other:?}"),
                };
                assert!(signal.is_symbol(sym::WRONG_TYPE_ARGUMENT));
                assert_eq!(interp.prin1_to_string(&signal.data()), "(listp bad)");
            })
            .unwrap()
            .join()
            .unwrap();
    }
}

#[cfg(test)]
mod advice_interactive_only_tests {
    use super::*;

    #[test]
    fn rust_advice_trampoline_accepts_interactive_only_as_a_passthrough() {
        std::thread::Builder::new()
            .stack_size(128 * 1024 * 1024)
            .spawn(|| {
                let mut interp = Interp::new();
                interp
                    .eval_str(
                        "(defun qa-rust-interactive-target (x) x)\
                         (defun qa-rust-interactive-advice (_x)\
                           (signal 'file-error '(\"interactive advice ran\")))",
                    )
                    .unwrap();
                let target = interp.q("qa-rust-interactive-target");
                let how = interp.q(":interactive-only");
                let advice = interp.q("qa-rust-interactive-advice");

                f_advice_add(
                    &mut interp,
                    vec![Value::Sym(target), Value::Sym(how), Value::Sym(advice)].into(),
                )
                .unwrap();
                let result = interp
                    .apply(&Value::Sym(target), vec![Value::Int(3)])
                    .unwrap();
                assert_eq!(interp.prin1_to_string(&result), "3");
            })
            .unwrap()
            .join()
            .unwrap();
    }
}

#[cfg(test)]
mod circular_symbol_macro_environment_tests {
    use super::*;
    use crate::lisp::obarray::Obarray;

    fn circular_spine(items: Vec<Value>) -> Value {
        let spine = Value::list(items);
        let mut tail = spine.clone();
        while let Value::Cons(cell) = tail {
            let next = cell.borrow().cdr.clone();
            if next.is_nil() {
                cell.borrow_mut().cdr = spine.clone();
                break;
            }
            tail = next;
        }
        spine
    }

    fn is_circular<T>(result: &Result<T, Flow>) -> bool {
        matches!(result, Err(Flow::Signal(signal)) if signal.is_symbol(sym::CIRCULAR_LIST))
    }

    #[test]
    fn symbol_macro_environment_and_binding_walks_reject_cycles() {
        let mut interp = Interp::vacant();
        interp.obarray = Obarray::new();
        let target = interp.intern("qa-symbol-macro-target");
        let other = interp.intern("qa-symbol-macro-other");
        let unrelated_entry = Value::cons(Value::Sym(other), Value::Nil);
        let env = circular_spine(vec![unrelated_entry]);

        assert!(is_circular(&sm_active(&mut interp, &env)));
        assert!(is_circular(&sm_lookup(&mut interp, &env, target)));
        assert!(is_circular(&sm_filter_env(&mut interp, &env, &[target])));
        assert!(is_circular(&binder_syms(&mut interp, &env)));

        let keyword = interp.q(":cl-symbol-macros");
        let binding = Value::cons(Value::Sym(other), Value::Sym(other));
        let bindings = circular_spine(vec![binding]);
        let entry = Value::cons(Value::Sym(keyword), bindings);
        let env = Value::list(vec![entry]);
        assert!(is_circular(&sm_lookup(&mut interp, &env, target)));
        assert!(is_circular(&sm_filter_env(&mut interp, &env, &[target])));
    }

    #[test]
    fn symbol_macro_binding_lists_reject_cycles() {
        let mut interp = Interp::vacant();
        interp.obarray = Obarray::new();
        let target = interp.intern("qa-symbol-macro-target");
        let other = interp.intern("qa-symbol-macro-other");
        let keyword = interp.q(":cl-symbol-macros");
        let binding = Value::cons(Value::Sym(other), Value::Sym(other));
        let bindings = circular_spine(vec![binding]);
        let entry = Value::cons(Value::Sym(keyword), bindings);
        let env = Value::list(vec![entry]);

        assert!(is_circular(&sm_lookup(&mut interp, &env, target)));
        assert!(is_circular(&sm_filter_env(&mut interp, &env, &[target])));
    }
}

#[cfg(test)]
mod provide_order_tests {
    use super::*;

    #[test]
    fn provide_publishes_features_and_load_history_before_after_load_forms() {
        std::thread::Builder::new()
            .stack_size(64 * 1024 * 1024)
            .spawn(|| {
                let mut interp = Interp::new();
                let visible = interp
                    .eval_str(
                        r##"(progn
                              (setq features nil
                                    current-load-list '("/tmp/qa-provide-order.el")
                                    load-in-progress nil
                                    after-load-alist nil)
                              (eval-after-load 'qa-provide-order-visible
                                '(setq qa-provide-order-observed
                                       (list (featurep 'qa-provide-order-visible)
                                             current-load-list)))
                              (provide 'qa-provide-order-visible)
                              (list qa-provide-order-observed
                                    (featurep 'qa-provide-order-visible)))"##,
                    )
                    .unwrap();
                assert_eq!(
                    interp.prin1_to_string(&visible),
                    "((t ((provide . qa-provide-order-visible) \"/tmp/qa-provide-order.el\")) t)"
                );

                let removed = interp
                    .eval_str(
                        "(progn\
                           (setq features nil load-in-progress nil after-load-alist nil)\
                           (eval-after-load 'qa-provide-order-removes\
                             '(setq features (delq 'qa-provide-order-removes features)))\
                           (provide 'qa-provide-order-removes)\
                           (featurep 'qa-provide-order-removes))",
                    )
                    .unwrap();
                assert!(removed.is_nil());

                let invalid = match interp.eval_str(
                        r##"(progn
                              (condition-case err
                                  (progn
                                    (setq current-load-list '("/tmp/qa-provide-invalid.el")
                                          load-in-progress nil
                                          after-load-alist nil
                                          qa-provide-invalid-ran nil)
                                    (eval-after-load 'qa-provide-invalid
                                      '(setq qa-provide-invalid-ran t))
                                    (setq features 'atom)
                                    (provide 'qa-provide-invalid '(sub)))
                                (error
                                 (list (car err) (cdr err)
                                       (get 'qa-provide-invalid 'subfeatures)
                                       qa-provide-invalid-ran
                                       current-load-list))))"##,
                    ) {
                    Ok(value) => value,
                    Err(Flow::Signal(signal)) => panic!(
                        "unexpected provide signal: {} {}",
                        interp.prin1_to_string(&signal.symbol()),
                        interp.prin1_to_string(&signal.data())
                    ),
                    Err(flow) => panic!("unexpected provide flow: {flow:?}"),
                };
                assert_eq!(
                    interp.prin1_to_string(&invalid),
                    "(wrong-type-argument (listp atom) nil nil (\"/tmp/qa-provide-invalid.el\"))"
                );
            })
            .unwrap()
            .join()
            .unwrap();
    }
}

#[cfg(test)]
mod macroexpand_list_poll_tests {
    use super::*;
    use std::sync::Arc;
    use std::time::Duration;

    #[test]
    fn macroexpand_list_conversion_observes_host_quit_requests() {
        std::thread::Builder::new()
            .stack_size(64 * 1024 * 1024)
            .spawn(|| {
                let mut interp = Interp::new();
                let mut list = Value::Nil;
                for _ in 0..2_000_000 {
                    list = Value::cons(Value::Nil, list);
                }
                let request = Arc::new(crate::frontend::QuitRequest::default());
                interp.set_quit_request(Some(request.clone()));

                let input_request = request.clone();
                let input = std::thread::spawn(move || {
                    std::thread::sleep(Duration::from_millis(20));
                    input_request.request();
                });
                let result = macroexpand_list_to_vec(&mut interp, &list);
                input.join().unwrap();

                assert!(matches!(
                    result,
                    Err(Flow::Signal(ref signal)) if signal.is_symbol(sym::QUIT)
                ));
                assert!(request.discard_queued_key());
            })
            .unwrap()
            .join()
            .unwrap();
    }
}

#[cfg(test)]
mod overlay_property_identity_tests {
    use super::*;
    use crate::lisp::obarray::Obarray;

    #[test]
    fn overlay_property_lookup_keeps_its_original_symbol_after_unintern() {
        let mut interp = Interp::vacant();
        interp.obarray = Obarray::new();
        let property = interp.q("insert-in-front-hooks");
        let plist = Value::list(vec![Value::Sym(property), Value::Int(7)]);
        assert!(interp.obarray.unintern_symbol(property));

        let replacement = interp.intern("insert-in-front-hooks");

        assert_ne!(property, replacement);
        assert!(matches!(
            overlay_prop(&mut interp, &plist, "insert-in-front-hooks"),
            Value::Int(7)
        ));
    }
}

#[cfg(test)]
mod locate_library_suffix_identity_tests {
    use super::*;
    use crate::lisp::obarray::Obarray;

    #[test]
    fn suffix_lookup_keeps_original_option_symbols_after_unintern() {
        let mut interp = Interp::vacant();
        interp.obarray = Obarray::new();
        let suffixes = interp.q("load-suffixes");
        let representations = interp.q("load-file-rep-suffixes");
        interp.obarray.symbol_mut(suffixes).value = Value::list(vec![Value::string(".el")]);
        interp.obarray.symbol_mut(representations).value = Value::list(vec![Value::string(".gz")]);
        assert!(interp.obarray.unintern_symbol(suffixes));
        assert!(interp.obarray.unintern_symbol(representations));

        let suffix_replacement = interp.intern("load-suffixes");
        let representation_replacement = interp.intern("load-file-rep-suffixes");
        interp.obarray.symbol_mut(suffix_replacement).value = Value::Nil;
        interp.obarray.symbol_mut(representation_replacement).value = Value::Nil;

        assert_ne!(suffixes, suffix_replacement);
        assert_ne!(representations, representation_replacement);
        assert_eq!(
            locate_library_suffixes(&mut interp, false).unwrap(),
            vec![".el.gz".to_owned(), ".gz".to_owned()]
        );
    }
}
