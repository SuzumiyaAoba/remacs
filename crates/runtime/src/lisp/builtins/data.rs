//! Type predicates and symbol-manipulation subrs.

use super::{S, arg, eq_values, equal_values, want_sym};
use crate::lisp::error::{EvalResult, Flow};
use crate::lisp::eval::cons_parts;
use crate::lisp::obarray::sym;
use crate::lisp::value::{Lambda, Subr, Value};
use crate::lisp::{Interp, SymId};
use std::rc::Rc;

pub(crate) static SUBRS: &[Subr] = &[
    S!(
        "eq",
        2,
        2,
        f_eq,
        "t if the two args are the same Lisp object."
    ),
    S!("null", 1, 1, f_null, "t if OBJECT is nil."),
    S!("not", 1, 1, f_null, "t if OBJECT is nil."),
    S!(
        "equal",
        2,
        2,
        f_equal,
        "t if two args have equal structure and contents."
    ),
    S!(
        "cl-equalp",
        2,
        2,
        f_equalp,
        "t if two args have similar structure and contents.\nNumbers compare with `='; characters and strings ignore case."
    ),
    S!("consp", 1, 1, f_consp, "t if OBJECT is a cons cell."),
    S!("atom", 1, 1, f_atom, "t if OBJECT is not a cons cell."),
    S!(
        "listp",
        1,
        1,
        f_listp,
        "t if OBJECT is a list (cons or nil)."
    ),
    S!("nlistp", 1, 1, f_nlistp, "t if OBJECT is not a list."),
    S!("symbolp", 1, 1, f_symbolp, "t if OBJECT is a symbol."),
    S!("stringp", 1, 1, f_stringp, "t if OBJECT is a string."),
    S!("vectorp", 1, 1, f_vectorp, "t if OBJECT is a vector."),
    S!(
        "hash-table-p",
        1,
        1,
        f_hash_table_p,
        "t if OBJECT is a hash table."
    ),
    S!("functionp", 1, 1, f_functionp, "t if OBJECT is a function."),
    S!(
        "subrp",
        1,
        1,
        f_subrp,
        "t if OBJECT is a built-in function."
    ),
    S!(
        "subr-primitive-p",
        1,
        1,
        f_subr_primitive_p,
        "t if OBJECT is a primitive built-in function."
    ),
    S!("subr-type", 1, 1, f_subr_type, "Type of a subr object."),
    S!("macrop", 1, 1, f_macrop, "t if OBJECT is a macro."),
    S!("keywordp", 1, 1, f_keywordp, "t if OBJECT is a keyword."),
    S!("sequencep", 1, 1, f_sequencep, "t if OBJECT is a sequence."),
    S!("seqp", 1, 1, f_sequencep, "t if OBJECT is a sequence."),
    S!("booleanp", 1, 1, f_booleanp, "t if OBJECT is t or nil."),
    S!(
        "characterp",
        1,
        2,
        f_characterp,
        "t if OBJECT is a character."
    ),
    S!(
        "integer-or-marker-p",
        1,
        1,
        f_integer_or_marker_p,
        "t if int or marker."
    ),
    S!("arrayp", 1, 1, f_arrayp, "t if OBJECT is an array."),
    // `user-variable-p' was removed in GNU Emacs 31 (obsoleted in 24.4;
    // `custom-variable-p' is the replacement).
    S!(
        "special-form-p",
        1,
        1,
        f_special_form_p,
        "t if OBJECT is a special form."
    ),
    S!(
        "type-of",
        1,
        1,
        f_type_of,
        "Return a symbol naming the type of OBJECT."
    ),
    S!(
        "symbol-value",
        1,
        1,
        f_symbol_value,
        "Return SYMBOL's value."
    ),
    S!(
        "symbol-name",
        1,
        1,
        f_symbol_name,
        "Return SYMBOL's name string."
    ),
    S!(
        "symbol-function",
        1,
        1,
        f_symbol_function,
        "Return SYMBOL's function definition."
    ),
    S!(
        "symbol-plist",
        1,
        1,
        f_symbol_plist,
        "Return SYMBOL's property list."
    ),
    S!("boundp", 1, 1, f_boundp, "t if SYMBOL's value is not void."),
    S!(
        "fboundp",
        1,
        1,
        f_fboundp,
        "t if SYMBOL's function definition is not void."
    ),
    S!("set", 2, 2, f_set, "Set SYMBOL's value to NEWVAL."),
    S!(
        "add-variable-watcher",
        2,
        2,
        f_add_variable_watcher,
        "Register WATCHER called on changes to SYMBOL."
    ),
    S!(
        "get-variable-watchers",
        1,
        1,
        f_get_variable_watchers,
        "List of watcher functions on SYMBOL."
    ),
    S!(
        "remove-variable-watcher",
        2,
        2,
        f_remove_variable_watcher,
        "Remove WATCHER from SYMBOL."
    ),
    S!(
        "fset",
        2,
        2,
        f_fset,
        "Set SYMBOL's function definition to DEFINITION."
    ),
    S!(
        "defalias",
        2,
        3,
        f_defalias,
        "Set SYMBOL's function cell to DEFINITION."
    ),
    S!(
        "makunbound",
        1,
        1,
        f_makunbound,
        "Make SYMBOL's value void."
    ),
    S!(
        "default-toplevel-value",
        1,
        1,
        f_default_toplevel_value,
        "Toplevel default value of SYMBOL."
    ),
    S!(
        "buffer-local-toplevel-value",
        1,
        2,
        f_buffer_local_toplevel_value,
        "Toplevel buffer-local value of SYMBOL."
    ),
    S!(
        "set-buffer-local-toplevel-value",
        2,
        3,
        f_set_buffer_local_toplevel_value,
        "Set SYMBOL's toplevel buffer-local value."
    ),
    S!(
        "internal-subr-documentation",
        1,
        1,
        f_internal_subr_documentation,
        "Docstring of a primitive subr."
    ),
    S!("defvar-1", 2, 3, f_defvar_1, "Internal defvar helper."),
    S!(
        "internal--define-uninitialized-variable",
        1,
        2,
        f_define_uninitialized_variable,
        "Mark SYMBOL special, unbound."
    ),
    S!(
        "internal-delete-indirect-variable",
        1,
        1,
        f_delete_indirect_variable,
        "Delete an indirect variable."
    ),
    S!(
        "internal--obarray-buckets",
        1,
        1,
        f_obarray_buckets,
        "Internal: obarray bucket vector."
    ),
    S!(
        "defconst-1",
        2,
        3,
        f_defconst_1,
        "Internal defconst helper."
    ),
    S!(
        "make-record",
        3,
        3,
        f_make_record,
        "Create a record of TYPE with LENGTH slots."
    ),
    S!(
        "text-quoting-style",
        0,
        0,
        f_text_quoting_style,
        "Current quoting style."
    ),
    S!(
        "fmakunbound",
        1,
        1,
        f_fmakunbound,
        "Make SYMBOL's function definition void."
    ),
    S!("get", 2, 2, f_get, "Return SYMBOL's PROPNAME property."),
    S!(
        "put",
        3,
        3,
        f_put,
        "Set SYMBOL's PROPNAME property to VALUE."
    ),
    S!(
        "setplist",
        2,
        2,
        f_setplist,
        "Set SYMBOL's property list to NEWPLIST."
    ),
    S!(
        "intern",
        1,
        2,
        f_intern,
        "Intern and return the symbol named STRING."
    ),
    S!(
        "intern-soft",
        1,
        2,
        f_intern_soft,
        "Return symbol named STRING if already interned."
    ),
    S!(
        "unintern",
        2,
        2,
        f_unintern,
        "Remove SYMBOL from the obarray."
    ),
    S!(
        "make-symbol",
        1,
        1,
        f_make_symbol,
        "Return a fresh uninterned symbol named NAME."
    ),
    S!(
        "mapatoms",
        1,
        2,
        f_mapatoms,
        "Map FUNCTION over all symbols in OBARRAY."
    ),
    S!(
        "indirect-function",
        1,
        2,
        f_indirect_function,
        "Return the function at the end of a symbol chain."
    ),
    S!(
        "defvaralias",
        2,
        3,
        f_defvaralias,
        "Make NEW-ALIAS a variable alias for BASE-VARIABLE."
    ),
    S!(
        "indirect-variable",
        1,
        1,
        f_indirect_variable,
        "Return the variable at the end of SYMBOL's alias chain."
    ),
    S!(
        "interactive-form",
        1,
        1,
        f_interactive_form,
        "Return interactive spec of OBJECT."
    ),
    S!(
        "variable-binding-locus",
        1,
        1,
        f_variable_binding_locus,
        "Where VARIABLE's binding lives."
    ),
    S!("subr-name", 1, 1, f_subr_name, "Return the name of a subr."),
    S!("function-equal", 2, 2, f_equal, "Like equal for functions."),
    S!(
        "function-get",
        2,
        3,
        f_function_get,
        "Property PROP on function SYMBOL."
    ),
    S!(
        "function-put",
        3,
        3,
        f_function_put,
        "Set property PROP on function SYMBOL."
    ),
    S!(
        "obarray-make",
        0,
        1,
        f_obarray_make,
        "Make a new obarray (stub: returns vector)."
    ),
    S!("obarrayp", 1, 1, f_obarrayp, "t if OBJECT is an obarray."),
    S!(
        "obarray-clear",
        1,
        1,
        f_obarray_clear,
        "Remove all symbols from OBARRAY."
    ),
];

fn f_eq(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    Ok(Value::from_bool(super::eq_values_with_interp(
        i, &args[0], &args[1],
    )))
}

fn f_null(_i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    Ok(Value::from_bool(args[0].is_nil()))
}

fn f_equal(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    Ok(Value::from_bool(equal_values(i, &args[0], &args[1])?))
}

/// GNU `cl-equalp' (cl-extra.el): numbers compare via `=' (chars are
/// numbers — no case fold), strings fold case via the buffer's case
/// table, conses/vectors/records recurse, everything else uses `equal'.
fn equalp_values(i: &Interp, down: &Value, a: &Value, b: &Value) -> Result<bool, Flow> {
    if crate::editor::window_configuration_object(a)
        || crate::editor::window_configuration_object(b)
    {
        return equal_values(i, a, b);
    }
    let down1 = |n: i128| match crate::lisp::builtins::misc::char_table_ref(i, down, n as usize) {
        Value::Int(m) => m,
        _ => n,
    };
    let numberp = |v: &Value| matches!(v, Value::Int(_) | Value::BigInt(_) | Value::Float(_));
    Ok(match (a, b) {
        _ if numberp(a) => numberp(b) && num_eq(a, b),
        (Value::Str(x), Value::Str(y)) => {
            let xs = x
                .try_borrow()
                .map_err(|_| i.error("Cannot compare a mutably borrowed string"))?;
            let ys = y
                .try_borrow()
                .map_err(|_| i.error("Cannot compare a mutably borrowed string"))?;
            let xc: Vec<crate::lisp::value::EmacsChar> = xs.chars().collect();
            let yc: Vec<crate::lisp::value::EmacsChar> = ys.chars().collect();
            xc.len() == yc.len()
                && xc
                    .iter()
                    .zip(yc.iter())
                    .all(|(a, b)| down1((*a).code() as i128) == down1((*b).code() as i128))
        }
        (Value::Str(_), _) => false,
        (Value::Cons(_), Value::Cons(_)) => {
            let mut ax = a.clone();
            let mut bx = b.clone();
            let mut guard = 0usize;
            loop {
                guard += 1;
                if guard > 100_000 {
                    return Ok(false);
                }
                match (&ax, &bx) {
                    (Value::Cons(ac), Value::Cons(bc)) => {
                        let (acar, acdr) = {
                            let ab = ac
                                .try_borrow()
                                .map_err(|_| i.error("Cannot compare a mutably borrowed cons cell"))?;
                            (ab.car.clone(), ab.cdr.clone())
                        };
                        let (bcar, bcdr) = {
                            let bb = bc
                                .try_borrow()
                                .map_err(|_| i.error("Cannot compare a mutably borrowed cons cell"))?;
                            (bb.car.clone(), bb.cdr.clone())
                        };
                        if !equalp_values(i, down, &acar, &bcar)? {
                            return Ok(false);
                        }
                        ax = acdr;
                        bx = bcdr;
                    }
                    _ => return equalp_values(i, down, &ax, &bx),
                }
            }
        }
        (Value::Vec(x), Value::Vec(y)) | (Value::Record(x), Value::Record(y)) => {
            let xv = x
                .try_borrow()
                .map(|items| items.clone())
                .map_err(|_| i.error("Cannot compare a mutably borrowed vector-like object"))?;
            let yv = y
                .try_borrow()
                .map(|items| items.clone())
                .map_err(|_| i.error("Cannot compare a mutably borrowed vector-like object"))?;
            if xv.len() != yv.len() {
                return Ok(false);
            }
            // cl-extra.el's `cl-equalp' walks vectors from the final slot
            // down; an early mismatch must short-circuit before earlier slots.
            for index in (0..xv.len()).rev() {
                if !equalp_values(i, down, &xv[index], &yv[index])? {
                    return Ok(false);
                }
            }
            true
        }
        _ => return equal_values(i, a, b),
    })
}

fn num_eq(a: &Value, b: &Value) -> bool {
    super::arith::number_order(a, b) == Some(std::cmp::Ordering::Equal)
}

fn f_equalp(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    let down = i.current_case_table();
    Ok(Value::from_bool(equalp_values(
        i, &down, &args[0], &args[1],
    )?))
}

fn f_consp(_i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    Ok(Value::from_bool(matches!(&args[0], Value::Cons(_))))
}
fn f_atom(_i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    Ok(Value::from_bool(!matches!(&args[0], Value::Cons(_))))
}
fn f_listp(_i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    Ok(Value::from_bool(matches!(
        &args[0],
        Value::Cons(_) | Value::Nil
    )))
}
fn f_nlistp(_i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    Ok(Value::from_bool(!matches!(
        &args[0],
        Value::Cons(_) | Value::Nil
    )))
}
fn f_symbolp(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    Ok(Value::from_bool(match &args[0] {
        Value::Sym(_) | Value::Nil => true,
        // `symbols-with-pos-enabled' objects are real symbols in GNU.
        v => crate::lisp::builtins::misc::symbol_position_for(i, v).is_some(),
    }))
}
fn f_stringp(_i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    Ok(Value::from_bool(matches!(&args[0], Value::Str(_))))
}
fn f_vectorp(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    Ok(Value::from_bool(
        matches!(&args[0], Value::Vec(_)) && !i.is_overlay(&args[0]),
    ))
}
fn f_hash_table_p(_i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    Ok(Value::from_bool(matches!(&args[0], Value::Hash(_))))
}
fn f_functionp(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    functionp_checked(i, &args[0]).map(Value::from_bool)
}

/// GNU `FUNCTIONP': callable objects — subrs that aren't special
/// forms, non-macros lambdas, `(lambda …)' forms, normal autoloads, and
/// symbols whose (possibly aliased) function cell resolves to one of those.
pub(crate) fn functionp_val(i: &mut Interp, arg: &Value) -> bool {
    functionp_checked(i, arg).unwrap_or(false)
}

pub(crate) fn functionp_checked(i: &mut Interp, arg: &Value) -> Result<bool, Flow> {
    let mut value = crate::lisp::builtins::misc::unpos(i, arg);
    let symbol = matches!(value, Value::Sym(_));
    if let Value::Sym(id) = value {
        value = i.symbol_function(id);
        if let Value::Sym(next) = value {
            if next == sym::UNBOUND {
                return Ok(false);
            }
            // Follow the complete chain, with the shared cycle validation.
            value = f_indirect_function(i, vec![arg.clone()].into())?;
        }
    }
    match &value {
        Value::Subr(s) => Ok(!matches!(s.arity, crate::lisp::Arity::Unevalled)),
        Value::Lambda(l) => Ok(!l.is_macro),
        Value::Cons(c) => {
            let head = c
                .try_borrow()
                .map(|cell| cell.car.clone())
                .map_err(|_| i.error("Cannot inspect a mutably borrowed function cell"))?;
            if i.sym_is(&head, sym::LAMBDA) {
                Ok(true)
            } else if symbol
                && i.q_soft("autoload").is_some_and(|id| i.sym_id(&head) == Some(id))
            {
                let mut tail = value.clone();
                for _ in 0..4 {
                    match tail.clone() {
                        Value::Cons(cell) => {
                            tail = cell
                                .try_borrow()
                                .map(|cell| cell.cdr.clone())
                                .map_err(|_| {
                                    i.error("Cannot inspect a mutably borrowed autoload object")
                                })?;
                        }
                        _ => {
                            tail = Value::Nil;
                            break;
                        }
                    }
                }
                match tail {
                    Value::Cons(tail) => Ok(tail
                        .try_borrow()
                        .map(|cell| cell.car.is_nil())
                        .map_err(|_| {
                            i.error("Cannot inspect a mutably borrowed autoload object")
                        })?),
                    // GNU accepts short/dotted autoload cells with no TYPE;
                    // TYPE=nil is a function, while non-nil types are macros
                    // or keymaps and are not functions.
                    _ => Ok(true),
                }
            } else {
                Ok(false)
            }
        }
        _ => Ok(false),
    }
}
fn f_subrp(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    // Emacs: strict type check — (subrp 'car) is nil.  GNU 31.1
    // native-compiles its dumped libraries, so a Lisp-defined
    // `forward-paragraph' is a (nativecomp) subr; remacs marks dumped
    // definitions `compiled' — report those as subrs too. Byte-code
    // objects loaded from `.elc' remain byte-code functions, not subrs.
    Ok(Value::from_bool(match &args[0] {
        Value::Subr(_) => true,
        Value::Lambda(l) => l
            .try_is_native_compiled()
            .map_err(|_| i.error("Cannot inspect a mutably borrowed byte-code object"))?
            && !l.is_macro,
        _ => false,
    }))
}
fn f_subr_primitive_p(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    // No byte-compiled/native-compiled subrs exist: primitive = subr.
    let _ = i;
    Ok(Value::from_bool(matches!(&args[0], Value::Subr(_))))
}
fn f_subr_type(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    // Emacs 31: returns nil for primitives, `built-in'/`special' only
    // for native-compiled subrs — our compiled twins carry no type spec
    // either, so always nil.
    match &args[0] {
        Value::Subr(_) => Ok(Value::Nil),
        // `native-comp-function-p' counts compiled Lambdas as native (they
        // stand in for GNU's native-compiled defuns), so `subr-type' must
        // accept them too — comp-function-type-spec calls it on them.
        Value::Lambda(l) => {
            let native = l
                .try_is_native_compiled()
                .map_err(|_| i.error("Cannot inspect a mutably borrowed byte-code object"))?;
            if native && !l.is_macro {
                Ok(Value::Nil)
            } else {
                Err(i.wrong_type_mut("subrp", &args[0]))
            }
        }
        other => Err(i.wrong_type_mut("subrp", other)),
    }
}
fn f_macrop(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    // GNU subr.el consults indirect-function: aliases (including
    // obsolete pushnew) keep the target macro/autoload type.
    let fun = f_indirect_function(i, vec![args[0].clone()].into())?;
    match &fun {
        Value::Lambda(l) => return Ok(Value::from_bool(l.is_macro)),
        Value::Cons(c) => {
            let (head, _) = c
                .try_borrow()
                .map(|cell| (cell.car.clone(), cell.cdr.clone()))
                .map_err(|_| i.error("Cannot inspect a mutably borrowed function cell"))?;
            if i.sym_is(&head, sym::MACRO) {
                return Ok(Value::t());
            }
            let auto_id = i.q("autoload");
            if i.sym_is(&head, auto_id) {
                // GNU subr.el `macrop': `(memq (nth 4 def) '(macro t))'
                // — the return value is the memq tail, so TYPE `macro'
                // answers (macro t), TYPE `t' answers (t), and other
                // non-nil types like `keymap' answer nil.
                let items = fun.list_to_vec().unwrap_or_default();
                let ty = items.get(4);
                let is_macro_ty = matches!(ty, Some(Value::Sym(s)) if *s == sym::MACRO);
                let is_t = matches!(ty, Some(Value::Sym(s)) if *s == sym::T);
                if is_macro_ty || is_t {
                    let tail: Vec<Value> = if is_macro_ty {
                        vec![Value::Sym(sym::MACRO), Value::t()]
                    } else {
                        vec![Value::t()]
                    };
                    return Ok(Value::list(tail));
                }
            }
        }
        _ => {}
    }
    Ok(Value::Nil)
}
fn f_keywordp(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    match i.sym_id(&args[0]) {
        // GNU: a keyword is an INTERNED symbol whose name starts
        // with ':' — `(make-symbol ":x")' is not a keyword.
        Some(id) => Ok(Value::from_bool(
            !i.obarray.symbol(id).uninterned && i.symbol_name(id).starts_with(':'),
        )),
        _ => Ok(Value::Nil),
    }
}
fn f_sequencep(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    // GNU `SEQUENCEP': lists, strings, vectors — and also
    // char-tables and bool-vectors (both Records here).
    Ok(Value::from_bool(match &args[0] {
        Value::Cons(_) | Value::Nil | Value::Str(_) => true,
        Value::Vec(_) => !i.is_overlay(&args[0]),
        Value::Record(_) => {
            super::misc::is_bool_vector(i, &args[0]) || super::misc::is_char_table(i, &args[0])
        }
        _ => false,
    }))
}
fn f_booleanp(_i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    Ok(Value::from_bool(matches!(
        &args[0],
        Value::Nil | Value::Sym(1) // t
    )))
}
fn f_characterp(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    let _ = i;
    Ok(Value::from_bool(match &args[0] {
        Value::Int(n) => (0..=0x3f_ffff).contains(n),
        _ => false,
    }))
}
fn f_integer_or_marker_p(_i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    Ok(Value::from_bool(matches!(
        &args[0],
        Value::Int(_) | Value::BigInt(_) | Value::Marker(_)
    )))
}
fn f_arrayp(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    // GNU arrays cover strings, vectors, char-tables, byte-code
    // objects, and bool-vectors; char-tables are Records here and
    // `#[...]' byte-code objects are Lambdas tagged by `bc_items'.
    Ok(Value::from_bool(match &args[0] {
        Value::Str(_) => true,
        Value::Vec(_) => !i.is_overlay(&args[0]),
        // GNU `arrayp' excludes every function object — closures and
        // byte-code objects alike (element access still works through
        // `aref'/`length').
        Value::Lambda(_) => false,
        other => super::misc::is_char_table(i, other) || super::misc::is_bool_vector(i, other),
    }))
}
fn f_special_form_p(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    // GNU subr.el: `(and (subrp (indirect-function OBJ)) (eq (cdr
    // (subr-arity OBJ)) 'unevalled))' — the FUNCTION CELL decides, not
    // the evaluator's special table: `defun'/`defmacro'/`prog2'/
    // `lambda' carry macro cells so they report nil even though eval
    // dispatches them by name.
    let cell = match &args[0] {
        Value::Subr(_) => args[0].clone(),
        v => match i.sym_id(v) {
            Some(id) => i.indirect_function_value(&Value::Sym(id)),
            None => return Ok(Value::Nil),
        },
    };
    Ok(Value::from_bool(matches!(
        cell,
        Value::Subr(s) if matches!(s.arity, crate::lisp::Arity::Unevalled)
    )))
}
fn f_type_of(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    let name = match &args[0] {
        Value::Nil => "symbol",
        Value::Int(_) | Value::BigInt(_) => "integer",
        Value::Float(_) => "float",
        Value::Sym(_) => "symbol",
        Value::Cons(_) => "cons",
        Value::Window(_) => "window",
        Value::Frame(_) => "frame",
        Value::Str(_) => "string",
        Value::Vec(_) if i.is_overlay(&args[0]) => "overlay",
        Value::Vec(_) => "vector",
        Value::Record(r) => {
            // `symbols-with-pos-enabled' objects are `symbol'.
            if crate::lisp::builtins::misc::raw_symbol_position(&args[0]).is_some() {
                let name = if i.positioned_symbols_enabled() {
                    "symbol"
                } else {
                    "symbol-with-pos"
                };
                return Ok(Value::Sym(i.q(name)));
            }
            // GNU reports an obarray as `obarray', not its tag-record
            // slot — obarrays are a distinct pseudovec type there.
            if is_obarray(i, &args[0]) {
                return Ok(Value::Sym(i.q("obarray")));
            }
            let rr = r
                .try_borrow()
                .map_err(|_| i.error("Cannot inspect a mutably borrowed record"))?;
            if let Some(first) = rr.first() {
                // GNU (`Fcl_type_of'): when slot 0 is itself a record
                // with size > 1 it is a class object and the reported
                // type is that object's name field (slot 1).
                if let Value::Record(type_record) = first {
                    let type_slots = type_record
                        .try_borrow()
                        .map_err(|_| i.error("Cannot inspect a mutably borrowed record type"))?;
                    if type_slots.len() > 1 {
                        return Ok(type_slots[1].clone());
                    }
                }
                // Otherwise GNU returns the record's type field
                // (slot 0) VERBATIM — a symbol for structs/EIEIO but
                // any object (e.g. `record' of a string) is reported
                // as-is, never folded to `record'.
                return Ok(first.clone());
            }
            "record"
        }
        Value::Hash(_) => "hash-table",
        Value::Subr(_) => "subr",
        Value::Lambda(l) => {
            let bytecode = l
                .try_is_bytecode()
                .map_err(|_| i.error("Cannot inspect a mutably borrowed byte-code object"))?;
            // Dumped source definitions stand in for native-compiled
            // functions. Bundled `.elc' functions remain byte-code objects.
            if l.compiled && !bytecode {
                "subr"
            } else if bytecode {
                "byte-code-function"
            } else {
                "interpreted-function"
            }
        }
        Value::Buffer(_) => "buffer",
        Value::Marker(_) => "marker",
        Value::Process(_) => "process",
        Value::Thread(_) => "thread",
        Value::Mutex(_) => "mutex",
        Value::CondVar(_) => "condition-variable",
        Value::Finalizer(_) => "finalizer",
    };
    Ok(Value::Sym(i.q(name)))
}
fn f_symbol_value(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    let id = want_sym(i, &args[0])?;
    let v = i.symbol_value(id);
    if let Value::Sym(s) = &v {
        if *s == sym::UNBOUND {
            return Err(i.signal_data(sym::VOID_VARIABLE, vec![args[0].clone()]));
        }
    }
    Ok(v)
}
pub(crate) fn f_symbol_name(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    let id = want_sym(i, &args[0])?;
    let symbol = i.obarray.symbol_mut(id);
    let name = symbol
        .name_string
        .get_or_insert_with(|| Value::string_ref(symbol.name.clone()));
    Ok(Value::Str(name.clone()))
}
fn f_symbol_function(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    let id = want_sym(i, &args[0])?;
    let f = i.symbol_function(id);
    // GNU exposes the raw cell: macros are `(macro . fn)' conses (a
    // stable identity `setcdr' can mutate); unbound cells read nil.
    match &f {
        Value::Sym(s) if *s == sym::UNBOUND => Ok(Value::Nil),
        _ => Ok(f),
    }
}
fn f_symbol_plist(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    let id = want_sym(i, &args[0])?;
    Ok(i.obarray.symbol(id).plist.clone())
}
fn f_boundp(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    let id = want_sym(i, &args[0])?;
    Ok(Value::from_bool(i.bound_p(id)))
}
fn f_fboundp(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    let id = want_sym(i, &args[0])?;
    Ok(Value::from_bool(i.fbound_p(id)))
}
fn f_set(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    let id = want_sym(i, &args[0])?;
    i.set_symbol(id, args[1].clone())?;
    Ok(args[1].clone())
}
fn f_add_variable_watcher(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    let id = want_sym(i, &args[0])?;
    let id = i.var_alias_target(id);
    // GNU stores unchecked functions, prepending only when `member'
    // (equal, rather than eq) does not find an existing watcher.
    for (symbol, watcher) in &i.var_watchers {
        if *symbol == id && equal_values(i, watcher, &args[1])? {
            return Ok(Value::Nil);
        }
    }
    i.var_watchers.insert(0, (id, args[1].clone()));
    Ok(Value::Nil)
}
fn f_get_variable_watchers(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    let id = match i.sym_id(&args[0]) {
        Some(id) => i.var_alias_target(id),
        None => return Ok(Value::Nil),
    };
    let list: Vec<Value> = i
        .var_watchers
        .iter()
        .filter(|(s, _)| *s == id)
        .map(|(_, f)| f.clone())
        .collect();
    Ok(Value::list(list))
}
fn f_remove_variable_watcher(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    let id = want_sym(i, &args[0])?;
    let id = i.var_alias_target(id);
    // GNU `delete' removes every equal occurrence.
    let mut pos = 0;
    while pos < i.var_watchers.len() {
        let (symbol, watcher) = &i.var_watchers[pos];
        if *symbol == id && equal_values(i, watcher, &args[1])? {
            i.var_watchers.remove(pos);
        } else {
            pos += 1;
        }
    }
    Ok(Value::Nil)
}

fn check_function_alias_cycle(i: &mut Interp, id: SymId, definition: &Value) -> Result<(), Flow> {
    let Value::Sym(start) = definition else {
        return Ok(());
    };
    let mut seen = std::collections::HashSet::new();
    let mut cur = *start;
    loop {
        if cur == id {
            let signal = i.q("cyclic-function-indirection");
            return Err(i.signal_data(signal, vec![Value::Sym(id)]));
        }
        if !seen.insert(cur) {
            break;
        }
        match i.symbol_function(cur) {
            Value::Sym(next) if next != sym::UNBOUND => cur = next,
            _ => break,
        }
    }
    Ok(())
}

fn f_fset(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    let id = want_sym(i, &args[0])?;
    i.check_function_assignment(id, &args[1])?;
    let def = normalize_fn_def(i, args[1].clone());
    check_function_alias_cycle(i, id, &def)?;
    // GNU's fset writes the cell raw — an advice chain on SYM is
    // discarded entirely (only `defalias' substitutes the base).
    i.advices.retain(|(s, _)| *s != id);
    i.advice_bases.retain(|(s, _)| *s != id);
    i.fset(id, def.clone());
    Ok(def)
}
pub(crate) fn f_defalias(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    let id = want_sym(i, &args[0])?;
    let def = normalize_fn_def(i, args[1].clone());
    // GNU's `Fdefalias' chases the alias chain before installing: a
    // definition that resolves back to SYMBOL signals
    // `cyclic-function-indirection' with the symbol, at defalias time.
    check_function_alias_cycle(i, id, &def)?;
    // GNU data.c `defalias' LOADHIST_ATTACHes `(defun . SYM)' when the
    // definition is installed — source `defun'/`defmacro' forms and
    // `.elc' defalias calls alike (an `(autoload ...)' cell counts too).
    let defun_id = i.q("defun");
    crate::lisp::builtins::evalfn::loadhist_attach(
        i,
        Value::cons(i.sym(defun_id), args[0].clone()),
    );
    i.fset_defalias(id, def)?;
    // GNU: a non-nil DOCSTRING goes on SYMBOL's `function-documentation'
    // property — what `documentation' reads through the alias.
    if let Some(doc) = args.get(2) {
        if doc.truthy() {
            let fd = i.q("function-documentation");
            i.put_prop(id, fd, doc.clone());
        }
    }
    // Emacs returns the aliased symbol, not the definition.
    Ok(args[0].clone())
}
fn f_defvaralias(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    // GNU eval.c Fdefvaralias ordering: CHECK_SYMBOL both, constant /
    // forwarded / localized alias rejection, cyclic chain check on the
    // BASE's C-level indirection chain, bound-alias value propagation to an
    // unbound BASE, let-bound-alias rejection, then install + mark both
    // special + (possibly nil) docstring store.
    let newv = want_sym(i, &args[0])?;
    let base = want_sym(i, &args[1])?;
    if i.obarray.symbol(newv).constant {
        let name = i.symbol_name(newv).to_string();
        return Err(i.error(&format!("Cannot make a constant an alias: {}", name)));
    }
    {
        let mut seen: std::collections::HashSet<SymId> = std::collections::HashSet::new();
        let mut cur = base;
        loop {
            if cur == newv {
                let sig = i.q("cyclic-variable-indirection");
                return Err(i.signal_data(sig, vec![args[1].clone()]));
            }
            if !seen.insert(cur) {
                break;
            }
            match i.obarray.symbol(cur).var_alias {
                Some(next) => cur = next,
                None => break,
            }
        }
    }
    if i.obarray.symbol(newv).forwarded_variable {
        let name = i.symbol_name(newv);
        return Err(i.error(format!("Cannot make a built-in variable an alias: {name}")));
    }
    if i.obarray.symbol(newv).builtin_variable
        || i.obarray.symbol(newv).always_local
        || i.obarray.symbol(newv).make_local_if_set
    {
        let name = i.symbol_name(newv).to_string();
        return Err(i.error(&format!(
            "Don’t know how to make a buffer-local variable an alias: {}",
            name
        )));
    }
    let mut has_buffer_local_value = false;
    for buffer_id in i.buffers.list() {
        if let Some(buffer) = i.buffers.get(buffer_id) {
            let state = buffer
                .try_borrow()
                .map_err(|_| i.error("Cannot inspect a mutably borrowed buffer"))?;
            if state.locals.contains_key(&newv) {
                has_buffer_local_value = true;
                break;
            }
        }
    }
    if has_buffer_local_value {
        let name = i.symbol_name(newv).to_string();
        return Err(i.error(&format!(
            "Don’t know how to make a buffer-local variable an alias: {}",
            name
        )));
    }
    let nv = i.symbol_value(newv);
    if !matches!(nv, Value::Sym(s) if s == sym::UNBOUND) {
        let bv = i.symbol_value(base);
        if matches!(bv, Value::Sym(s) if s == sym::UNBOUND) {
            i.set_symbol(base, nv)?;
        }
    }
    if i.specbind_for(newv) {
        let name = i.symbol_name(newv).to_string();
        return Err(i.error(&format!(
            "Don’t know how to make a let-bound variable an alias: {}",
            name
        )));
    }
    // Notify before installing the alias. Watchers see the old cell,
    // the proposed base as NEWVAL, and can abort the aliasing.
    let watched = i.var_alias_target(newv);
    i.fire_var_watchers(watched, &args[1], "defvaralias", None)?;
    i.obarray.symbol_mut(newv).var_alias = Some(base);
    i.obarray.symbol_mut(newv).special = true;
    i.obarray.symbol_mut(base).special = true;
    i.obarray.symbol_mut(newv).constant = i.obarray.symbol(base).constant;
    // GNU stores docstring unconditionally — nil ERASES an old one.
    let vd = i.q("variable-documentation");
    let doc = args.get(2).cloned().unwrap_or(Value::Nil);
    i.put_prop(newv, vd, doc);
    crate::lisp::builtins::evalfn::loadhist_attach(i, args[0].clone());
    Ok(args[1].clone())
}

fn f_indirect_variable(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    // GNU: non-symbol args pass through unchanged (no type check).
    let id = match &args[0] {
        Value::Sym(s) => *s,
        other => return Ok(other.clone()),
    };
    Ok(i.sym(i.var_alias_target(id)))
}

fn makunbound_watch_matches(name: &str, watch: Result<String, std::env::VarError>) -> bool {
    watch.is_ok_and(|watch| watch.split(',').any(|entry| entry == name))
}

fn f_makunbound(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    let id = want_sym(i, &args[0])?;
    let id = i.var_alias_target(id);
    // GNU refuses to unbind constants (keywords, nil, t).
    if i.obarray.symbol(id).constant {
        return Err(i.signal_data(sym::SETTING_CONSTANT, vec![args[0].clone()]));
    }
    if i.dbg_flags & 0x4000 != 0 {
        let name = i.symbol_name(id);
        if makunbound_watch_matches(&name, std::env::var("WATCH_MAKUNBOUND")) {
            eprintln!(
                "[makunbound] {}\n{}",
                name,
                std::backtrace::Backtrace::capture()
            );
        }
    }
    // GNU refuses to unbind C-backed variables.
    if i.obarray.symbol(id).builtin_variable {
        return Err(i.error(format!(
            "Built-in variable may not be unbound : {}",
            i.symbol_name(id)
        )));
    }
    // GNU: when the variable has a buffer-local binding in the current
    // buffer (or is buffer-local whenever set), only the local binding
    // becomes void — the default value survives.
    let local = i.obarray.symbol(id).make_local_if_set
        || i.buffers
            .get(i.current_buffer)
            .and_then(|b| b.try_borrow().ok().map(|bb| bb.locals.contains_key(&id)))
            .unwrap_or(false);
    if local {
        // GNU's set_internal fires watchers BEFORE the store — a
        // signalling watcher leaves the local binding intact.
        i.fire_var_watchers(id, &Value::Nil, "makunbound", Some(i.current_buffer))?;
        if let Some(b) = i.buffers.get(i.current_buffer) {
            if let Ok(mut bb) = b.try_borrow_mut() {
                bb.locals.insert(id, Value::Sym(sym::UNBOUND));
            }
        }
    } else {
        i.fire_var_watchers(id, &Value::Nil, "makunbound", None)?;
        i.obarray.symbol_mut(id).value = Value::Sym(sym::UNBOUND);
    }
    Ok(args[0].clone())
}

fn f_default_toplevel_value(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    let id = want_sym(i, &args[0])?;
    match i.default_toplevel_value(id) {
        Some(v) => Ok(v),
        None => Err(i.signal_data(sym::VOID_VARIABLE, vec![args[0].clone()])),
    }
}

fn f_buffer_local_toplevel_value(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    // GNU buffer.c Fbuffer_local_toplevel_value:
    // CHECK_BUFFER(BUFFER) on the optional arg — nil = current buffer.
    let buf_id = match args.get(1) {
        None | Some(Value::Nil) => i.current_buffer,
        Some(Value::Buffer(b)) => b
            .try_borrow()
            .map(|buffer| buffer.id)
            .map_err(|_| i.error("Cannot inspect a mutably borrowed buffer"))?,
        Some(other) => return Err(i.wrong_type_mut("bufferp", other)),
    };
    let id = want_sym(i, &args[0])?;
    match i.buffer_local_toplevel_value(id, buf_id) {
        Some(v) => Ok(v),
        None => Err(i.signal_data(sym::VOID_VARIABLE, vec![args[0].clone()])),
    }
}

fn f_set_buffer_local_toplevel_value(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    // GNU eval.c order: a non-nil BUFFER is decoded like
    // Fset_buffer's Fget_buffer first — `stringp' for a non-buffer
    // non-name, "Selecting deleted buffer" for a dead buffer, a plain
    // "No buffer named" error for a missing name — and only afterwards
    // does Fmake_local_variable CHECK_SYMBOL the first arg.
    let buf_id = match args.get(2) {
        None | Some(Value::Nil) => i.current_buffer,
        Some(Value::Buffer(b)) => {
            let buffer = b
                .try_borrow()
                .map_err(|_| i.error("Cannot inspect a mutably borrowed buffer"))?;
            if !buffer.live {
                return Err(i.error("Selecting deleted buffer"));
            }
            buffer.id
        }
        Some(v @ Value::Str(_)) => match i.buffer_id_of(v) {
            Some(id) => id,
            None => return Err(i.error(format!("No buffer named {}", i.princ_to_string(v)))),
        },
        Some(other) => return Err(i.wrong_type_mut("stringp", other)),
    };
    let id = want_sym(i, &args[0])?;
    i.set_buffer_local_toplevel_value(id, buf_id, args[1].clone());
    Ok(Value::Nil)
}

fn f_internal_subr_documentation(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    // GNU doc.c: SUBRP returns the subr's DOC index (a fixnum);
    // everything else — including SYMBOLS (no indirection there) —
    // returns t.
    if let Value::Subr(s) = &args[0] {
        if let Some(off) = crate::lisp::builtins::misc::doc_offset(i, &format!("F{}", s.name)) {
            return Ok(Value::Int(off as i128));
        }
    }
    Ok(Value::t())
}

fn f_defvar_1(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    // (defvar-1 SYM INITVALUE &optional DOCSTRING): defvar with
    // evaluated arguments — same cell-shape semantics as `defvar'
    // (eval.c `defvar' with eval=false).
    let id = want_sym(i, &args[0])?;
    let docstring = match args.get(2) {
        Some(Value::Str(string)) => Some(
            string
                .try_borrow()
                .map(|text| (*text).clone())
                .map_err(|_| i.error("Cannot inspect a mutably borrowed variable docstring"))?,
        ),
        _ => None,
    };
    i.obarray.symbol_mut(id).special = true;
    // GNU `defvar': install INIT only when the DEFAULT binding is void.
    // `bound_p' sees the live cell (a let binding counts), so when bound
    // but the outermost specbind saved `Qunbound', store into that saved
    // slot — the let is shadowing a void default.
    if let Some(v) = args.get(1) {
        if !i.bound_p(id) {
            i.set_symbol_default(id, v.clone())?;
        } else if i.specbind_outermost_unbound(id) {
            i.set_specbind_outermost_old(id, v.clone());
        }
    }
    if let Some(docstring) = docstring {
        // GNU: Fput(symbol, Qvariable_documentation, doc).
        let vd = i.q("variable-documentation");
        i.put_prop(id, vd, Value::string(docstring));
    }
    Ok(args[0].clone())
}

fn f_defconst_1(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    // (defconst-1 SYM INITVALUE &optional DOCSTRING): defconst with
    // evaluated arguments — always sets the default.
    let id = want_sym(i, &args[0])?;
    let docstring = match args.get(2) {
        Some(Value::Str(string)) => Some(
            string
                .try_borrow()
                .map(|text| (*text).clone())
                .map_err(|_| i.error("Cannot inspect a mutably borrowed variable docstring"))?,
        ),
        _ => None,
    };
    i.obarray.symbol_mut(id).special = true;
    i.set_symbol_default(id, args[1].clone())?;
    if let Some(docstring) = docstring {
        // GNU: Fput(symbol, Qvariable_documentation, doc).
        let vd = i.q("variable-documentation");
        i.put_prop(id, vd, Value::string(docstring));
    }
    let rlv = i.q("risky-local-variable");
    i.put_prop(id, rlv, Value::t());
    Ok(args[0].clone())
}

fn f_define_uninitialized_variable(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    // (internal--define-uninitialized-variable SYM &optional DOC): GNU's
    // F-impl uses unchecked XSYMBOL — a non-symbol argument scribbles the
    // "special" bit into whatever object it got and returns nil without
    // complaint.  Only a non-nil DOC reaches a CHECK_SYMBOL via Fput.
    // GNU XSYMBOL covers nil (sym::NIL): nil is a valid symbol arg here —
    // only non-symbol objects short-circuit, and even then only when DOC
    // is present does CHECK_SYMBOL fire (via Fput).
    let id = match i.sym_id(&args[0]) {
        Some(id) => id,
        None => {
            if !matches!(args.get(1), None | Some(Value::Nil)) {
                return Err(i.wrong_type_mut("symbolp", &args[0]));
            }
            return Ok(Value::Nil);
        }
    };
    i.obarray.symbol_mut(id).special = true;
    if let Some(doc) = args.get(1) {
        if !doc.is_nil() {
            // GNU: Fput(symbol, Qvariable_documentation, doc).
            let vd = i.q("variable-documentation");
            i.put_prop(id, vd, doc.clone());
        }
    }
    // GNU eval.c `defvar' LOADHIST_ATTACHes the bare symbol.
    crate::lisp::builtins::evalfn::loadhist_attach(i, args[0].clone());
    Ok(Value::Nil)
}

fn f_delete_indirect_variable(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    let symbol = want_sym(i, &args[0])?;
    if i.obarray.symbol(symbol).var_alias.is_none() {
        // GNU signals (error "Cannot undeclare..." VARIABLE): the datum
        // carries the offending symbol.
        let error = i.q("error");
        return Err(i.signal_data(
            error,
            vec![
                Value::string("Cannot undeclare a variable that is not an alias"),
                args[0].clone(),
            ],
        ));
    }

    let plist = i.obarray.symbol(symbol).plist.clone();
    // GNU detaches the alias before clearing its documentation. Preserve the
    // current value in the now ordinary symbol first: if clearing the plist
    // signals, the alias remains detached and retains the value it exposed.
    let current_value = i.symbol_value(symbol);
    i.obarray.symbol_mut(symbol).var_alias = None;
    i.obarray.symbol_mut(symbol).value = current_value;

    let variable_documentation = i.q("variable-documentation");
    let plist = crate::lisp::eval::plist_put_v_checked(
        i,
        &plist,
        &Value::Sym(variable_documentation),
        Value::Nil,
    )?;
    i.macro_gen += 1;

    // `variable-alias' in a Lisp plist is independent user property data;
    // only the C-level indirection is removed here.
    i.obarray.symbol_mut(symbol).plist = plist;
    i.set_symbol(symbol, Value::Sym(sym::UNBOUND))?;
    Ok(args[0].clone())
}

fn f_obarray_buckets(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    // GNU `check_obarray's the argument first — non-obarrays
    // signal `obarrayp'.  Our obarray isn't bucketed; expose one bucket
    // holding all symbols of the default obarray.
    let ob = Some(check_obarray(i, &args[0])?);
    if let Some(o) = &ob {
        if !is_default_obarray(i, o) {
            return Ok(Value::list(vec![Value::list(obarray_syms(i, o)?)]));
        }
    }
    let names: Vec<Value> = i
        .obarray
        .all_ids()
        .into_iter()
        .filter(|id| !i.obarray.symbol(*id).uninterned)
        .map(Value::Sym)
        .collect();
    Ok(Value::list(vec![Value::list(names)]))
}

/// GNU allocate_record's pseudovector size includes the type slot.
pub(crate) fn check_record_slots(i: &Interp, slots: i128) -> Result<(), Flow> {
    const MAX_SLOTS: i128 = 4095;
    if slots > MAX_SLOTS {
        return Err(i.error(format!(
            "Attempt to allocate a record of {slots} slots; max is {MAX_SLOTS}"
        )));
    }
    Ok(())
}

fn f_make_record(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    // (make-record TYPE LENGTH INITVAL)
    // GNU Fmake_record: LENGTH is CHECK_FIXNAT → `wholenump'.
    let n = match &args[1] {
        Value::Int(n) if args[1].fixnump() && *n >= 0 => *n,
        _ => {
            let wholenump = Value::Sym(i.q("wholenump"));
            return Err(i.signal_data(sym::WRONG_TYPE_ARGUMENT, vec![wholenump, args[1].clone()]));
        }
    };
    check_record_slots(i, n + 1)?;
    let n = n as usize;
    let mut v = Vec::with_capacity(n + 1);
    v.push(args[0].clone());
    v.extend(std::iter::repeat_n(args[2].clone(), n));
    Ok(Value::record(v))
}

fn f_text_quoting_style(i: &mut Interp, _args: crate::lisp::Argv) -> EvalResult {
    // GNU Ftext_quoting_style returns text_quoting_style ().
    use super::evalfn::QuoteStyle;
    let name = match super::evalfn::text_quoting_style(i) {
        QuoteStyle::Grave => "grave",
        QuoteStyle::Straight => "straight",
        QuoteStyle::Curve => "curve",
    };
    Ok(Value::Sym(i.q(name)))
}
fn f_fmakunbound(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    let id = want_sym(i, &args[0])?;
    // Unlike fset, GNU refuses even a nil definition through fmakunbound.
    // Keyword function cells remain mutable.
    if id == sym::NIL || id == sym::T {
        return Err(i.signal_data(sym::SETTING_CONSTANT, vec![args[0].clone()]));
    }
    i.macro_gen += 1;
    i.obarray.symbol_mut(id).function = Value::Sym(sym::UNBOUND);
    Ok(args[0].clone())
}
pub(crate) fn f_get(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    let id = want_sym(i, &args[0])?;
    // GNU accepts any propname (eq-compared), not only symbols.
    let plist = i.obarray.symbol(id).plist.clone();
    crate::lisp::eval::plist_get_v_checked(i, &plist, &args[1])
}
fn f_put(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    let id = want_sym(i, &args[0])?;
    let plist = i.obarray.symbol(id).plist.clone();
    let new_plist = crate::lisp::eval::plist_put_v_checked(i, &plist, &args[1], args[2].clone())?;
    i.macro_gen += 1;
    i.obarray.symbol_mut(id).plist = new_plist;
    Ok(args[2].clone())
}
fn f_setplist(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    let id = want_sym(i, &args[0])?;
    i.macro_gen += 1;
    i.obarray.symbol_mut(id).plist = args[1].clone();
    Ok(args[1].clone())
}
pub(crate) fn string_intern_key(
    i: &Interp,
    string: &crate::lisp::value::StrRef,
) -> Result<crate::lisp::obarray::InternKey, Flow> {
    let bytes = super::strfn::try_gnu_sbytes(i, string)?;
    let characters = string
        .try_borrow()
        .map_err(|_| i.error("Cannot inspect a mutably borrowed symbol name"))?
        .chars()
        .count();
    Ok(crate::lisp::obarray::InternKey { bytes, characters })
}

fn symbol_name_key(
    i: &Interp,
    symbol: crate::lisp::value::SymId,
) -> Result<crate::lisp::obarray::InternKey, Flow> {
    let cell = i.obarray.symbol(symbol);
    match &cell.name_string {
        Some(string) => string_intern_key(i, string),
        None => Ok(cell
            .internment_key
            .clone()
            .unwrap_or_else(|| crate::lisp::obarray::InternKey::from_text(&cell.name))),
    }
}

pub(crate) fn lookup_in_obarray(
    i: &Interp,
    obarray: &Option<Value>,
    key: &crate::lisp::obarray::InternKey,
) -> Result<Option<crate::lisp::value::SymId>, Flow> {
    if let Some(table) = obarray {
        if !is_default_obarray(i, table) {
            for value in obarray_syms(i, table)? {
                let Some(symbol) = i.sym_id(&value) else {
                    continue;
                };
                if symbol_name_key(i, symbol)? == *key {
                    return Ok(Some(symbol));
                }
            }
            return Ok(None);
        }
    }
    for symbol in i.obarray.key_candidates(key).iter().rev().copied() {
        if symbol_name_key(i, symbol)? == *key {
            return Ok(Some(symbol));
        }
    }
    Ok(None)
}

/// The intern_driver operation, after name transformation and obarray checks.
/// It bypasses the public intern cell, just as lread.c does.
pub(crate) fn intern_in_obarray(
    i: &mut Interp,
    name: &crate::lisp::value::StrRef,
    obarray: &Option<Value>,
) -> Result<crate::lisp::value::SymId, Flow> {
    let key = string_intern_key(i, name)?;
    if let Some(symbol) = lookup_in_obarray(i, obarray, &key)? {
        return Ok(symbol);
    }
    let text = name
        .try_borrow()
        .map(|name| (*name).clone())
        .map_err(|_| i.error("Cannot inspect a mutably borrowed symbol name"))?;
    let custom = obarray
        .as_ref()
        .filter(|table| !is_default_obarray(i, table));
    let symbol = if custom.is_some() {
        i.make_symbol(&text)
    } else {
        i.obarray.intern_new_with_key(&text, key)
    };
    i.obarray.symbol_mut(symbol).name_string = Some(name.clone());
    if let Some(table) = custom {
        obarray_push(i, table, &i.sym(symbol));
    }
    Ok(symbol)
}

/// oblookup_considering_shorthand: byte-prefix matching, first valid pair,
/// permissive dotted/invalid entries and GNU's bounded-space safe traversal.
pub(crate) fn expand_symbol_shorthand(
    i: &mut Interp,
    name: &crate::lisp::value::StrRef,
    multibyte: bool,
) -> Result<Option<crate::lisp::value::StrRef>, Flow> {
    let Some(variable) = i.q_soft("read-symbol-shorthands") else {
        return Ok(None);
    };
    let mut tail = i.symbol_value(variable);
    if !matches!(tail, Value::Cons(_)) {
        return Ok(None);
    }
    let bytes = super::strfn::try_gnu_sbytes(i, name)?;
    let mut tortoise = tail.clone();
    let mut span = 2usize;
    let mut blocks = 0isize;
    let mut steps = 2u16;
    while let Value::Cons(cell) = tail {
        let (pair, next) = {
            let cell = cell
                .try_borrow()
                .map_err(|_| i.error("Cannot inspect a mutably borrowed read-symbol-shorthands list"))?;
            (cell.car.clone(), cell.cdr.clone())
        };
        if let Value::Cons(pair) = pair {
            let pair = pair
                .try_borrow()
                .map_err(|_| i.error("Cannot inspect a mutably borrowed symbol shorthand"))?;
            if let (Value::Str(short), Value::Str(long)) = (&pair.car, &pair.cdr) {
                let prefix = super::strfn::try_gnu_sbytes(i, short)?;
                if bytes.starts_with(&prefix) {
                    let mut expanded = super::strfn::try_gnu_sbytes(i, long)?;
                    expanded.extend_from_slice(&bytes[prefix.len()..]);
                    let name_characters = name
                        .try_borrow()
                        .map_err(|_| i.error("Cannot inspect a mutably borrowed symbol name"))?
                        .chars()
                        .count() as isize;
                    let short_characters = short
                        .try_borrow()
                        .map_err(|_| i.error("Cannot inspect a mutably borrowed symbol shorthand"))?
                        .chars()
                        .count() as isize;
                    let long_characters = long
                        .try_borrow()
                        .map_err(|_| i.error("Cannot inspect a mutably borrowed symbol shorthand"))?
                        .chars()
                        .count() as isize;
                    let characters = name_characters - short_characters + long_characters;
                    let text = if multibyte {
                        crate::lisp::value::emacs_string_bytes(&expanded)
                    } else {
                        // make_specified_string's unibyte path uses
                        // the calculated character count as its byte length.
                        expanded
                            .into_iter()
                            .take(characters.max(0) as usize)
                            .map(char::from)
                            .collect()
                    };
                    let string = Value::string_ref_with_kind(text, multibyte);
                    if multibyte {
                        i.mark_multibyte(&string);
                    } else {
                        i.mark_unibyte(&string);
                    }
                    return Ok(Some(string));
                }
            }
        }
        tail = next;
        steps = steps.wrapping_sub(1);
        let check = if steps != 0 {
            true
        } else {
            blocks -= 1;
            if blocks > 0 {
                true
            } else {
                span = span.saturating_mul(2);
                steps = span as u16;
                blocks = (span >> 16) as isize;
                tortoise = tail.clone();
                false
            }
        };
        if check && eq_values(&tail, &tortoise) {
            break;
        }
    }
    Ok(None)
}

pub(crate) fn f_intern(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    let obarray = resolve_obarray(i, args.get(1))?;
    let Value::Str(name) = &args[0] else {
        return Err(i.wrong_type_mut("stringp", &args[0]));
    };
    let name = expand_symbol_shorthand(i, name, true)?.unwrap_or_else(|| name.clone());
    let symbol = intern_in_obarray(i, &name, &obarray)?;
    Ok(i.sym(symbol))
}

fn f_intern_soft(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    let obarray = resolve_obarray(i, args.get(1))?;
    if let Some(symbol) = i.sym_id(&args[0]) {
        let key = symbol_name_key(i, symbol)?;
        return Ok(if lookup_in_obarray(i, &obarray, &key)? == Some(symbol) {
            args[0].clone()
        } else {
            Value::Nil
        });
    }
    let Value::Str(name) = &args[0] else {
        return Err(i.wrong_type_mut("stringp", &args[0]));
    };
    let name = expand_symbol_shorthand(i, name, true)?.unwrap_or_else(|| name.clone());
    Ok(lookup_in_obarray(i, &obarray, &string_intern_key(i, &name)?)?
        .map(|symbol| i.sym(symbol))
        .unwrap_or(Value::Nil))
}

fn f_unintern(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    let obarray = resolve_obarray(i, args.get(1))?;
    let symbol = match i.sym_id(&args[0]) {
        Some(symbol) => Some(symbol),
        None => {
            let Value::Str(name) = &args[0] else {
                return Err(i.wrong_type_mut("stringp", &args[0]));
            };
            let name = expand_symbol_shorthand(i, name, true)?.unwrap_or_else(|| name.clone());
            lookup_in_obarray(i, &obarray, &string_intern_key(i, &name)?)?
        }
    };
    let Some(symbol) = symbol else {
        return Ok(Value::Nil);
    };
    if let Some(table) = &obarray {
        if !is_default_obarray(i, table) {
            return Ok(Value::from_bool(obarray_remove(i, table, &i.sym(symbol))?));
        }
    }
    Ok(Value::from_bool(i.obarray.unintern_symbol(symbol)))
}
fn f_make_symbol(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    let name = match &args[0] {
        Value::Str(s) => s
            .try_borrow()
            .map(|name| (*name).clone())
            .map_err(|_| i.error("Cannot inspect a mutably borrowed make-symbol name"))?,
        _ => return Err(i.wrong_type_mut("stringp", &args[0])),
    };
    let id = i.make_symbol(&name);
    if let Value::Str(string) = &args[0] {
        i.obarray.symbol_mut(id).name_string = Some(string.clone());
    }
    Ok(Value::Sym(id))
}
/// Obarrays are record-backed here, but their identity is tracked
/// separately so an ordinary record tagged `obarray` stays a record.
fn obarray_tag(i: &Interp) -> Value {
    i.default_obarray
        .as_ref()
        .and_then(|table| table.borrow().first().cloned())
        .unwrap_or_else(|| Value::Sym(i.q_soft("obarray").unwrap_or(0)))
}

pub(crate) fn is_obarray(_i: &Interp, v: &Value) -> bool {
    matches!(v, Value::Record(_))
        && v.record_kind() == Some(crate::lisp::value::RecordKind::Obarray)
}

/// The value of the `obarray' variable denotes the default symbol
/// table, not a custom obarray: its record's Rc identity was stored on
/// the interpreter at startup.
pub(crate) fn is_default_obarray(i: &Interp, v: &Value) -> bool {
    match (v, &i.default_obarray) {
        (Value::Record(r), Some(d)) => std::rc::Rc::ptr_eq(r, d),
        _ => false,
    }
}

/// The symbols interned in a custom (Record-based) obarray.  The vec
/// is kept in GNU `map_obarray' order: bucket index ascending, and
/// within a bucket most-recently-interned first (head insertion).
pub(crate) fn obarray_syms(i: &Interp, ob: &Value) -> Result<Vec<Value>, Flow> {
    if let Value::Record(r) = ob {
        let slots = r
            .try_borrow()
            .map_err(|_| i.error("Cannot inspect a mutably borrowed obarray record"))?;
        if let Some(Value::Vec(v)) = slots.get(1) {
            return Ok(v
                .try_borrow()
                .map_err(|_| i.error("Cannot inspect a mutably borrowed obarray symbol vector"))?
                .iter()
                .filter(|x| matches!(x, Value::Sym(_)))
                .cloned()
                .collect());
        }
    }
    Ok(Vec::new())
}

/// GNU `obarray_index': hash_char_array + knuth_hash reduction.
fn gnu_obarray_hash(name: &[u8], bits: u32) -> u32 {
    let len = name.len() as u64;
    let mut hash: u64 = len;
    if name.len() >= 8 {
        let step = std::cmp::max(8usize, name.len() >> 3);
        let mut p = 0usize;
        while p + 8 <= name.len() {
            let c = u64::from_ne_bytes(name[p..p + 8].try_into().unwrap());
            hash = hash
                .wrapping_shl(4)
                .wrapping_add(hash >> 60)
                .wrapping_add(c);
            p += step;
        }
        // Tail: always hash the last 8 bytes (may overlap).
        let c = u64::from_ne_bytes(name[name.len() - 8..].try_into().unwrap());
        hash = hash
            .wrapping_shl(4)
            .wrapping_add(hash >> 60)
            .wrapping_add(c);
    } else {
        let mut tail: u64 = 0;
        let mut p = 0usize;
        if name.len() - p >= 4 {
            let c = u32::from_ne_bytes(name[p..p + 4].try_into().unwrap());
            tail = (tail << 32) + c as u64;
            p += 4;
        }
        if name.len() - p >= 2 {
            let c = u16::from_ne_bytes(name[p..p + 2].try_into().unwrap());
            tail = (tail << 16) + c as u64;
            p += 2;
        }
        if p < name.len() {
            tail = (tail << 8) + name[p] as u64;
        }
        hash = hash
            .wrapping_shl(4)
            .wrapping_add(hash >> 60)
            .wrapping_add(tail);
    }
    let h32 = (hash ^ (hash >> 32)) as u32;
    (((h32.wrapping_mul(2654435769)) as u64) >> (32 - bits)) as u32
}

fn obarray_bits(i: &Interp, ob: &Value) -> u32 {
    let _ = i;
    if let Value::Record(r) = ob {
        if let Some(Value::Int(b)) = r.borrow().get(2) {
            return *b as u32;
        }
    }
    3
}

fn set_obarray_bits(ob: &Value, bits: u32) {
    if let Value::Record(r) = ob {
        let mut rr = r.borrow_mut();
        if rr.len() > 2 {
            rr[2] = Value::Int(bits as i128);
        }
    }
}

fn obarray_push(i: &Interp, ob: &Value, sym: &Value) {
    let vrc = match ob {
        Value::Record(r) => match r.borrow().get(1).cloned() {
            Some(Value::Vec(v)) => Some(v),
            _ => None,
        },
        _ => None,
    };
    let Some(mut vrc) = vrc else {
        let _ = i;
        return;
    };
    // Image decoding canonicalizes empty Lisp vectors. Detach an empty
    // internal bucket array before growing it, including restored obarrays.
    if vrc.borrow().is_empty() {
        vrc = Value::allocate_vector_ref(Vec::new());
        if let Value::Record(record) = ob {
            record.borrow_mut()[1] = Value::Vec(vrc.clone());
        }
    }
    let mut v = vrc.borrow_mut();
    if v.iter().any(|x| eq_values(x, sym)) {
        return;
    }
    let bits = obarray_bits(i, ob);
    let name = match sym {
        Value::Sym(id) => i.symbol_name(*id).to_string(),
        _ => String::new(),
    };
    let idx = gnu_obarray_hash(name.as_bytes(), bits);
    // Head-insert into bucket `idx`: before the first
    // element whose bucket is >= idx.
    let mut at = v.len();
    for (k, x) in v.iter().enumerate() {
        let b = match x {
            Value::Sym(id) => gnu_obarray_hash(i.symbol_name(*id).as_bytes(), bits),
            _ => continue,
        };
        if b >= idx {
            at = k;
            break;
        }
    }
    v.insert(at, sym.clone());
    // GNU grows when count > size, rehashing head-to-tail.
    if v.len() > (1usize << bits) && bits < 31 {
        let nbits = bits + 1;
        let old: Vec<Value> = v.drain(..).collect();
        for x in old {
            let b = match &x {
                Value::Sym(id) => gnu_obarray_hash(i.symbol_name(*id).as_bytes(), nbits),
                _ => 0,
            };
            let mut at = v.len();
            for (k, y) in v.iter().enumerate() {
                let yb = match y {
                    Value::Sym(id) => gnu_obarray_hash(i.symbol_name(*id).as_bytes(), nbits),
                    _ => continue,
                };
                if yb >= b {
                    at = k;
                    break;
                }
            }
            v.insert(at, x);
        }
        drop(v);
        set_obarray_bits(ob, nbits);
    }
}

fn obarray_remove(i: &Interp, ob: &Value, sym: &Value) -> Result<bool, Flow> {
    if let Value::Record(r) = ob {
        let slots = r
            .try_borrow()
            .map_err(|_| i.error("Cannot inspect a mutably borrowed obarray record"))?;
        if let Some(Value::Vec(v)) = slots.get(1) {
            let mut v = v
                .try_borrow_mut()
                .map_err(|_| i.error("Cannot update a mutably borrowed obarray symbol vector"))?;
            // Strings select by name; symbols (including positioned
            // symbols) must be the actual member, not a same-name twin.
            if let Some(pos) = v.iter().position(|candidate| match (sym, candidate) {
                (Value::Str(name), Value::Sym(id)) => i.symbol_name(*id) == *name.borrow(),
                (_, Value::Sym(id)) => i.sym_id(sym) == Some(*id),
                _ => false,
            }) {
                v.remove(pos);
                return Ok(true);
            }
        }
    }
    Ok(false)
}

/// Check an optional OBARRAY argument and resolve it to the effective
/// obarray: nil/absent means the value of `obarray'; a tagged
/// record is used directly; a VECTOR is GNU's legacy vector-as-obarray
/// — every slot must be 0 or a symbol hashed to that bucket
/// (obarray_slow check).  A valid vector is promoted by placing an
/// obarray record in slot 0, carrying over any pre-existing symbols.
/// Anything else is a type error.
pub(crate) fn resolve_obarray(i: &mut Interp, v: Option<&Value>) -> Result<Option<Value>, Flow> {
    let value = match v {
        None | Some(Value::Nil) => {
            let symbol = i.sym_id(&obarray_tag(i)).unwrap();
            i.symbol_value(symbol)
        }
        Some(value) => value.clone(),
    };
    check_obarray(i, &value).map(Some)
}

fn check_obarray(i: &mut Interp, value: &Value) -> Result<Value, Flow> {
    let v = Some(value);
    match v {
        Some(v) if is_obarray(i, v) => Ok(v.clone()),
        Some(w @ Value::Vec(vec)) => {
            // GNU `check_obarray_slow': a non-empty vector is an
            // obarray iff slot 0 is 0 (fresh obarray object is stored
            // there) or an obarray already — the remaining slots are
            // IGNORED entirely (no per-slot bucket validation).
            let v0 = vec
                .try_borrow()
                .map_err(|_| i.error("Cannot inspect a mutably borrowed obarray vector"))?
                .first()
                .cloned();
            if let Some(v0) = v0 {
                if is_obarray(i, &v0) {
                    return Ok(v0);
                }
                if matches!(v0, Value::Int(0)) {
                    let rec = Value::record_with_kind(
                        vec![
                            obarray_tag(i),
                            Value::Vec(Value::allocate_vector_ref(Vec::new())),
                            // GNU uses make_obarray(0): one bucket.
                            Value::Int(0),
                        ],
                        crate::lisp::value::RecordKind::Obarray,
                    );
                    let mut entries = vec
                        .try_borrow_mut()
                        .map_err(|_| i.error("Cannot promote a mutably borrowed obarray vector"))?;
                    entries[0] = rec.clone();
                    return Ok(rec);
                }
            }
            Err(i.wrong_type_mut("obarrayp", w))
        }
        Some(v) => Err(i.wrong_type_mut("obarrayp", v)),
        None => unreachable!(),
    }
}

fn f_mapatoms(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    let fun = args[0].clone();
    let ob = resolve_obarray(i, args.get(1))?;
    if let Some(ob) = &ob {
        if !is_default_obarray(i, ob) {
            for sym in obarray_syms(i, ob)? {
                i.apply(&fun, vec![sym])?;
            }
            return Ok(Value::Nil);
        }
    }
    let ids = i.obarray.all_ids();
    for id in ids {
        i.apply(&fun, vec![i.sym(id)])?;
    }
    Ok(Value::Nil)
}
pub(crate) fn f_indirect_function(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    let mut cur = crate::lisp::builtins::misc::unpos(i, &args[0]);
    let mut seen: std::collections::HashSet<crate::lisp::value::SymId> =
        std::collections::HashSet::new();
    let mut until_quit = 1024usize;
    loop {
        match cur {
            Value::Sym(id) => {
                seen.insert(id);
                let f = i.symbol_function(id);
                if let Value::Sym(s) = &f {
                    if *s == sym::UNBOUND {
                        return Ok(Value::Nil);
                    }
                }
                match f {
                    // GNU's `indirect_function' signals
                    // `cyclic-function-indirection' with the symbol
                    // whose alias points back into the chain.
                    Value::Sym(next) if seen.contains(&next) => {
                        let sig = i.q("cyclic-function-indirection");
                        return Err(i.signal_data(sig, vec![Value::Sym(id)]));
                    }
                    Value::Sym(next) => cur = i.sym(next),
                    other => return Ok(other),
                }
                until_quit -= 1;
                if until_quit == 0 {
                    i.maybe_quit()?;
                    until_quit = 1024;
                }
            }
            other => return Ok(other),
        }
    }
}
pub(crate) fn f_interactive_form(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    // Validate alias cycles first, then consult properties along the chain,
    // matching GNU data.c before it inspects the function definition.
    let resolved = f_indirect_function(i, vec![args[0].clone()].into())?;
    if resolved.is_nil() {
        return Ok(Value::Nil);
    }
    let property = i.q("interactive-form");
    let mut f = args[0].clone();
    while let Value::Sym(id) = f {
        let form = i.get_prop(id, property);
        if !form.is_nil() {
            return Ok(form);
        }
        f = i.symbol_function(id);
    }
    f = i.advice_base_value(&f);
    if matches!(f, Value::Sym(_)) {
        return f_interactive_form(i, vec![f].into());
    }
    let autoload = i.q("autoload");
    let is_autoload = match &f {
        Value::Cons(c) => i.sym_is(&cons_parts(i, c)?.0, autoload),
        _ => false,
    };
    if is_autoload {
        // GNU passes CMD as FUNNAME; a bare autoload cons is not a symbol.
        if !matches!(args[0], Value::Sym(_)) {
            return Err(i.wrong_type_mut("symbolp", &args[0]));
        }
        let loaded = super::evalfn::autoload_do_load(i, f, false)?;
        return f_interactive_form(i, vec![loaded].into());
    }
    match &f {
        Value::Subr(s) => {
            // GNU returns `(interactive SPEC)' for primitives whose
            // intspec is non-nil.  Specs beginning with `(' are forms
            // read from the string rather than callint letter codes.
            match crate::lisp::eval::subr_interactive_form(i, s) {
                Some(spec) => Ok(spec),
                None => Ok(Value::Nil),
            }
        }
        _ => match f.as_lambda() {
            Some(l) => {
                // For byte-code/reader function objects, slot 5 is the
                // authoritative interactive spec. Reader circle fixups can
                // replace a raw slot after the cached Lambda field is built.
                let raw = if l.bc_items.is_some() {
                    bc_interactive_form(i, l)?
                } else {
                    l.interactive.clone()
                };
                match raw {
                    Some(spec) => normalize_interactive_spec(i, spec),
                    // GNU data.c: OClosure-typed functions fall through to
                    // `calln(oclosure-interactive-form, fun)' when the
                    // generic exists.
                    None => {
                        let oif = i.q_soft("oclosure-interactive-form");
                        // OClosure: bc_items[4] (the docstring slot)
                        // holds the type symbol; GNU delegates to
                        // `oclosure-interactive-form' whenever the
                        // generic exists (a symbol tag suffices — the
                        // default method returns nil).
                        let is_oclosure = match &f {
                            Value::Lambda(l) => {
                                let tag = match &l.bc_items {
                                    Some(items) => items
                                        .try_borrow()
                                        .map(|items| items.get(4).cloned())
                                        .map_err(|_| {
                                            i.error("Cannot inspect a mutably borrowed byte-code object")
                                        })?,
                                    None => None,
                                };
                                tag.map(|v| matches!(v, Value::Sym(_))).unwrap_or(false)
                                    // Interpreted oclosures store the type
                                    // symbol in the doc slot too (eval.c
                                    // `make-interpreted-closure' layout).
                                    || matches!(&l.doc_value, Some(Value::Sym(_)))
                            }
                            _ => false,
                        };
                        if is_oclosure {
                            if let Some(gf) = oif {
                                if i.fbound_p(gf) {
                                    return i.apply(
                                        &i.symbol_function(gf),
                                        crate::lisp::Argv::from(vec![f.clone()]),
                                    );
                                }
                            }
                        }
                        Ok(Value::Nil)
                    }
                }
            }
            // GNU scans the body of a `(lambda ...)'/`(closure ...)'
            // /`(macro . (lambda ...))' form: docstring and `declare'
            // /`interactive-declare' elements are skipped and the first
            // remaining form is returned when it is `(interactive ...)'.
            None => interactive_form_list(i, &f),
        },
    }
}

/// The `(interactive SPEC)' form of a `#[argdesc str consts depth
/// doc ispec]' byte-code object — GNU stores the interactive spec as
/// element 5 (Ffetch_bytecode / `interactive-form' read it from there).
pub(crate) fn bc_interactive_form(
    i: &mut Interp,
    l: &Rc<Lambda>,
) -> Result<Option<Value>, Flow> {
    let Some(items) = &l.bc_items else {
        return Ok(None);
    };
    let items = items
        .try_borrow()
        .map_err(|_| i.error("Cannot inspect a mutably borrowed byte-code object"))?;
    // GNU Fcommandp: `PVSIZE (fun) > CLOSURE_INTERACTIVE' — the mere
    // PRESENCE of slot 5 (even holding nil) makes the object a
    // command; `interactive-form' then reports `(interactive nil)'.
    // A 5-slot `#[args str consts depth doc]' has no spec at all.
    let Some(spec) = items.get(5) else {
        return Ok(None);
    };
    // A VECTOR in slot 5 is the command-modes form — the spec is its
    // first element (data.c Finteractive_form).
    let spec = match spec {
        Value::Vec(v) => v
            .try_borrow()
            .map(|values| values.first().cloned().unwrap_or(Value::Nil))
            .map_err(|_| i.error("Cannot inspect a mutably borrowed interactive spec vector"))?,
        s => s.clone(),
    };
    Ok(Some(Value::list(vec![Value::Sym(i.q("interactive")), spec])))
}
/// GNU `documentation-function-or-1' semantics for `(macro . FUN)':
/// unwrap the macro cell so doc/subr lookups see the inner function.
pub(crate) fn macro_inner(i: &mut Interp, f: Value) -> Value {
    let m = i.q("macro");
    match &f {
        Value::Cons(c) if i.sym_id(&c.borrow().car) == Some(m) => c.borrow().cdr.clone(),
        _ => f,
    }
}

/// GNU `interactive-form' result for an `(interactive ...)' element:
/// a bare `(interactive)' reads back as `(interactive nil)'.
fn normalize_interactive_spec(i: &mut Interp, v: Value) -> EvalResult {
    let interactive = i.q("interactive");
    if let Value::Cons(cell) = &v {
        let (head, tail) = cons_parts(i, cell)?;
        if i.sym_id(&head) == Some(interactive) && tail.is_nil() {
            return Ok(Value::list(vec![Value::Sym(interactive), Value::Nil]));
        }
    }
    Ok(v)
}

/// `interactive-form' over a cons-shaped function (unevaluated
/// lambda/closure/macro data), as in GNU's Finteractive_form.
fn interactive_form_list(i: &mut Interp, f: &Value) -> EvalResult {
    let mut function = f.clone();
    // `(macro . FUN)' — step inside the macro wrapper.
    if let Value::Cons(cell) = &function {
        let (head, tail) = cons_parts(i, cell)?;
        if i.sym_id(&head) == Some(sym::MACRO) {
            function = tail;
        }
    }
    let Value::Cons(head_cell) = &function else {
        return Ok(Value::Nil);
    };
    let (head, mut body) = cons_parts(i, head_cell)?;
    let body_at = match i.sym_id(&head) {
        Some(sym::LAMBDA) => 2,
        Some(id) if i.q_soft("closure") == Some(id) => 3,
        _ => return Ok(Value::Nil),
    };
    // Only step through the fixed header before scanning body forms. GNU
    // returns an interactive declaration as soon as it sees one, even if a
    // later dotted or circular tail makes the entire function form invalid.
    for _ in 1..body_at {
        let Value::Cons(cell) = body else {
            return Ok(Value::Nil);
        };
        body = cons_parts(i, &cell)?.1;
    }
    let interactive = i.q("interactive");
    let declare = i.q("declare");
    let ideclare = i.q("interactive-declare");
    let mut seen = std::collections::HashSet::new();
    while let Value::Cons(cell) = body.clone() {
        if !seen.insert(Rc::as_ptr(&cell) as usize) {
            return Ok(Value::Nil);
        }
        let (el, next) = cons_parts(i, &cell)?;
        body = next;
        if matches!(el, Value::Str(_)) {
            continue;
        }
        let head_id = match &el {
            Value::Cons(c) => i.sym_id(&cons_parts(i, c)?.0),
            _ => None,
        };
        if head_id == Some(interactive) {
            // Unevaluated lambda/closure data reads back verbatim —
            // GNU returns the raw `(interactive)' (no nil padding).
            return Ok(el);
        }
        if head_id == Some(declare) || head_id == Some(ideclare) {
            continue;
        }
        break;
    }
    Ok(Value::Nil)
}
fn f_variable_binding_locus(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    let id = want_sym(i, &args[0])?;
    if let Some(b) = i.buffers.get(i.current_buffer) {
        let has_local_value = b
            .try_borrow()
            .map_err(|_| i.error("Cannot inspect a mutably borrowed buffer"))?
            .locals
            .contains_key(&id);
        if has_local_value {
            let buf = i.buffers.get(i.current_buffer).unwrap().clone();
            return Ok(Value::Buffer(buf));
        }
    }
    Ok(Value::Nil)
}
fn f_subr_name(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    match &args[0] {
        Value::Subr(s) => Ok(match super::comp::native_name(s) {
            Some(bytes) => i.auto_string_value(&bytes),
            None => Value::string(s.name),
        }),
        // Native-compiled lambdas print as `#<subr NAME>' like GNU's
        // native functions; byte-code objects are not subrs.
        Value::Lambda(l) => {
            let native = l
                .try_is_native_compiled()
                .map_err(|_| i.error("Cannot inspect a mutably borrowed byte-code object"))?;
            if !native {
                return Err(i.wrong_type_mut("subrp", &args[0]));
            }
            match i.lambda_name(l) {
                Some(n) => Ok(Value::string(n)),
                None => Err(i.wrong_type_mut("subrp", &args[0])),
            }
        }
        _ => Err(i.wrong_type_mut("subrp", &args[0])),
    }
}
fn f_function_get(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    let id = want_sym(i, &args[0])?;
    let prop = want_sym(i, &args[1])?;
    Ok(i.get_prop(id, prop))
}
fn f_function_put(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    // GNU's FUNCTION arg is resolved through `indirect-function':
    // function objects (subrs, lambdas, closures) are accepted as-is
    // and the call returns VALUE without storing anywhere; only a
    // bare non-function atom signals the symbol-with-pos predicate.
    let id = match i.sym_id(&args[0]) {
        Some(s) => s,
        None => match &args[0] {
            Value::Subr(_) | Value::Lambda(_) => {
                return Ok(args[2].clone());
            }
            // A (lambda ...) / (closure ...) form is a function too.
            Value::Cons(c) => {
                let head = c
                    .try_borrow()
                    .map(|cell| cell.car.clone())
                    .map_err(|_| i.error("Cannot inspect a mutably borrowed function form"))?;
                let closure = i.q_soft("closure");
                match i.sym_id(&head) {
                    Some(s) if s == sym::LAMBDA || Some(s) == closure => {
                        return Ok(args[2].clone());
                    }
                    _ => {
                        let swp = i.q("symbol-with-pos-p");
                        let symbolp = i.q("symbolp");
                        return Err(i.signal_data(
                            sym::WRONG_TYPE_ARGUMENT,
                            vec![
                                Value::list(vec![Value::Sym(symbolp), Value::Sym(swp)]),
                                args[0].clone(),
                            ],
                        ));
                    }
                }
            }
            _ => {
                let swp = i.q("symbol-with-pos-p");
                let symbolp = i.q("symbolp");
                return Err(i.signal_data(
                    sym::WRONG_TYPE_ARGUMENT,
                    vec![
                        Value::list(vec![Value::Sym(symbolp), Value::Sym(swp)]),
                        args[0].clone(),
                    ],
                ));
            }
        },
    };
    // PROP is any symbol-or-atom per GNU's get/put lookup semantics.
    match i.sym_id(&args[1]) {
        Some(prop) => i.put_prop(id, prop, args[2].clone()),
        None => {}
    }
    Ok(args[2].clone())
}
fn f_obarray_make(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    // GNU Fobarray_make: SIZE nil -> 3 bits; else CHECK_FIXNAT and
    // bits = elogb(n)+1 (0 -> 0, 1 -> 1, 7 -> 3, 8 -> 4).
    // bits > obarray_max_bits (31) -> args-out-of-range.
    let bits: u32 = match args.get(0) {
        None | Some(Value::Nil) => 3,
        Some(Value::Int(n)) if *n >= 0 => {
            let n = *n as u64;
            let b = if n == 0 { 0u32 } else { 64 - n.leading_zeros() };
            if b > 31 {
                // GNU: xsignal(Qargs_out_of_range, size) — the datum is
                // the bare fixnum, so (cdr err) is the number itself.
                return Err(i.signal(
                    crate::lisp::obarray::sym::ARGS_OUT_OF_RANGE,
                    args[0].clone(),
                ));
            }
            b
        }
        Some(v) => return Err(i.wrong_type_mut("wholenump", v)),
    };
    Ok(Value::record_with_kind(
        vec![
            obarray_tag(i),
            Value::Vec(Value::allocate_vector_ref(Vec::new())),
            Value::Int(bits as i128),
        ],
        crate::lisp::value::RecordKind::Obarray,
    ))
}
fn f_obarrayp(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    Ok(Value::from_bool(is_obarray(i, &args[0])))
}
/// `(obarray-clear OBARRAY)' — unintern every symbol in OBARRAY.
/// GNU Fobarray_clear: fixed arity 1, `CHECK_OBARRAY' — nil signals
/// `obarrayp' (no default-obarray shorthand, unlike intern/intern-soft).
fn f_obarray_clear(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    let ob_val = arg(&args, 0);
    if ob_val.is_nil() || !is_obarray(i, &ob_val) {
        return Err(i.wrong_type_mut("obarrayp", &ob_val));
    }
    let ob = ob_val.clone();
    if is_default_obarray(i, &ob) {
        i.obarray.clear_namespace();
        return Ok(Value::Nil);
    }
    if let Value::Record(r) = &ob {
        {
            let slots = r
                .try_borrow()
                .map_err(|_| i.error("Cannot inspect a mutably borrowed obarray record"))?;
            if let Some(Value::Vec(v)) = slots.get(1) {
                v.try_borrow_mut()
                    .map_err(|_| {
                        i.error("Cannot clear a mutably borrowed obarray symbol vector")
                    })?
                    .clear();
            }
        }
        // GNU resets to the default size (8 buckets).
        set_obarray_bits(&ob, 3);
    }
    Ok(Value::Nil)
}

/// Normalize a function definition for `fset`/`defalias`: `(macro . f)'
/// conses are stored verbatim — GNU keeps that cons as the literal
/// function cell so `symbol-function'/`setcdr' mutations on it are
/// visible to the interpreter.  (A bare macro Lambda gets wrapped by
/// `fset' instead.)
fn normalize_fn_def(_i: &mut Interp, def: Value) -> Value {
    def
}

#[cfg(test)]
mod plist_cycle_tests {
    use super::*;
    use crate::lisp::{error::Flow, obarray::Obarray, sym};

    #[test]
    fn put_signals_circular_list_for_a_missing_property() {
        let mut interp = Interp::vacant();
        interp.obarray = Obarray::new();
        let owner = interp.intern("plist-put-cycle-owner");
        let existing = interp.intern("plist-put-cycle-existing");
        let requested = interp.intern("plist-put-cycle-requested");
        let key = Value::cons(Value::Sym(existing), Value::Nil);
        let value = Value::cons(Value::Int(1), key.clone());
        if let Value::Cons(cell) = &key {
            cell.borrow_mut().cdr = value;
        }
        interp.obarray.symbol_mut(owner).plist = key;
        let old_plist = interp.obarray.symbol(owner).plist.clone();
        let old_macro_gen = interp.macro_gen;

        let result = f_put(
            &mut interp,
            vec![Value::Sym(owner), Value::Sym(requested), Value::Int(42)].into(),
        );
        assert!(matches!(
            result,
            Err(Flow::Signal(ref signal)) if signal.is_symbol(sym::CIRCULAR_LIST)
        ));
        assert_eq!(interp.macro_gen, old_macro_gen);
        assert!(crate::lisp::builtins::eq_values(
            &interp.obarray.symbol(owner).plist,
            &old_plist
        ));
    }

    #[test]
    fn put_still_appends_to_a_proper_symbol_plist() {
        let mut interp = Interp::vacant();
        interp.obarray = Obarray::new();
        let owner = interp.intern("plist-put-owner");
        let existing = interp.intern("plist-put-existing");
        let new_property = interp.intern("plist-put-new");
        interp.obarray.symbol_mut(owner).plist =
            Value::list(vec![Value::Sym(existing), Value::Int(1)]);

        let result = f_put(
            &mut interp,
            vec![Value::Sym(owner), Value::Sym(new_property), Value::Int(42)].into(),
        )
        .unwrap();
        assert!(matches!(result, Value::Int(42)));
        assert!(matches!(
            crate::lisp::eval::plist_get_v(
                &interp.obarray.symbol(owner).plist,
                &Value::Sym(new_property),
            ),
            Value::Int(42)
        ));
    }
}

#[cfg(test)]
mod makunbound_watch_tests {
    use super::makunbound_watch_matches;

    #[test]
    fn missing_or_invalid_watch_values_do_not_panic_or_match() {
        assert!(!makunbound_watch_matches(
            "sample-variable",
            Err(std::env::VarError::NotPresent)
        ));
        assert!(makunbound_watch_matches(
            "sample-variable",
            Ok("other-variable,sample-variable".to_string())
        ));
        assert!(!makunbound_watch_matches(
            "sample-variable-extra",
            Ok("sample-variable".to_string())
        ));
    }

    #[cfg(unix)]
    #[test]
    fn non_unicode_watch_value_is_ignored() {
        use std::os::unix::ffi::OsStringExt;

        let invalid = std::ffi::OsString::from_vec(vec![0xff]);
        assert!(!makunbound_watch_matches(
            "sample-variable",
            Err(std::env::VarError::NotUnicode(invalid))
        ));
    }
}

#[cfg(test)]
mod type_of_symbol_identity_tests {
    use super::*;
    use crate::lisp::obarray::Obarray;

    #[test]
    fn type_of_returns_the_original_type_symbol_after_unintern() {
        let mut interp = Interp::vacant();
        interp.obarray = Obarray::new();
        let original = interp.q("symbol");
        assert!(interp.obarray.unintern_symbol(original));

        let replacement = interp.intern("symbol");
        let result = f_type_of(&mut interp, vec![Value::Sym(sym::T)].into()).unwrap();

        assert_ne!(original, replacement);
        assert!(matches!(result, Value::Sym(id) if id == original));
    }
}

#[cfg(test)]
mod text_quoting_style_identity_tests {
    use super::*;
    use crate::lisp::obarray::Obarray;

    #[test]
    fn text_quoting_style_returns_the_original_style_symbol_after_unintern() {
        let mut interp = Interp::vacant();
        interp.obarray = Obarray::new();
        let style_variable = interp.q("text-quoting-style");
        let original = interp.q("grave");
        interp.obarray.symbol_mut(style_variable).value = Value::Sym(original);
        assert!(interp.obarray.unintern_symbol(original));
        let replacement = interp.intern("grave");

        let result = f_text_quoting_style(&mut interp, crate::lisp::Argv::new(Vec::new())).unwrap();

        assert_ne!(original, replacement);
        assert!(matches!(result, Value::Sym(symbol) if symbol == original));
    }
}
