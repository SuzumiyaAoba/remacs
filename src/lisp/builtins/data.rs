//! Type predicates and symbol-manipulation subrs.

use super::{S, arg, eq_values, equal_values, want_sym};
use crate::lisp::error::{EvalResult, Flow};
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

fn f_eq(_i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    Ok(Value::from_bool(eq_values(&args[0], &args[1])))
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
    let down1 = |n: i128| match crate::lisp::builtins::misc::char_table_ref(i, down, n as usize) {
        Value::Int(m) => m,
        _ => n,
    };
    let numberp = |v: &Value| matches!(v, Value::Int(_) | Value::Float(_));
    Ok(match (a, b) {
        _ if numberp(a) => numberp(b) && num_eq(a, b),
        (Value::Str(x), Value::Str(y)) => {
            let (xs, ys) = (x.borrow(), y.borrow());
            let xc: Vec<char> = xs.chars().collect();
            let yc: Vec<char> = ys.chars().collect();
            xc.len() == yc.len()
                && xc
                    .iter()
                    .zip(yc.iter())
                    .all(|(a, b)| down1(*a as i128) == down1(*b as i128))
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
                            let ab = ac.borrow();
                            (ab.car.clone(), ab.cdr.clone())
                        };
                        let (bcar, bcdr) = {
                            let bb = bc.borrow();
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
            let xv = x.borrow();
            let yv = y.borrow();
            if xv.len() != yv.len() {
                return Ok(false);
            }
            for (a, b) in xv.iter().zip(yv.iter()) {
                if !equalp_values(i, down, a, b)? {
                    return Ok(false);
                }
            }
            true
        }
        _ => return equal_values(i, a, b),
    })
}

fn num_eq(a: &Value, b: &Value) -> bool {
    let f = |v: &Value| match v {
        Value::Int(n) => Some(*n as f64),
        Value::Float(x) => Some(**x),
        _ => None,
    };
    match (f(a), f(b)) {
        (Some(x), Some(y)) => x == y,
        _ => false,
    }
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
fn f_symbolp(_i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    Ok(Value::from_bool(match &args[0] {
        Value::Sym(_) | Value::Nil => true,
        // `symbols-with-pos-enabled' objects are real symbols in GNU.
        v => crate::lisp::builtins::misc::as_sympos(v).is_some(),
    }))
}
fn f_stringp(_i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    Ok(Value::from_bool(matches!(&args[0], Value::Str(_))))
}
fn f_vectorp(_i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    Ok(Value::from_bool(matches!(&args[0], Value::Vec(_))))
}
fn f_hash_table_p(_i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    Ok(Value::from_bool(matches!(&args[0], Value::Hash(_))))
}
fn f_functionp(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    Ok(Value::from_bool(functionp_val(i, &args[0])))
}

/// GNU `FUNCTIONP': callable objects — subrs that aren't special
/// forms, non-macros lambdas, `(lambda …)' forms, and symbols whose
/// (possibly aliased) function cell resolves to one of those.
pub(crate) fn functionp_val(i: &mut Interp, arg: &Value) -> bool {
    let v = crate::lisp::builtins::misc::unpos(arg);
    let v = &v;
    let r = match v {
        Value::Subr(s) => crate::lisp::special::special_form(i.intern(s.name)).is_none(),
        // GNU `functionp' on a macro object (`(macro . fn)') is nil.
        Value::Lambda(l) => !l.is_macro,
        Value::Sym(id) => {
            // a symbol is a function if its function cell is defined
            // (and isn't a macro).  GNU follows `defalias' chains:
            // `(functionp 'alias)' tests the alias's final target.
            let mut f = i.symbol_function(*id);
            let mut hops = 0usize;
            while let Value::Sym(next) = f {
                if next == sym::UNBOUND || hops >= 64 {
                    return false;
                }
                hops += 1;
                f = i.symbol_function(next);
            }
            match f {
                // Special forms install pseudo-subrs but are not callable
                // functions: (functionp #'if) -> nil.
                Value::Subr(s) => crate::lisp::special::special_form(i.intern(s.name)).is_none(),
                Value::Lambda(l) => !l.is_macro,
                Value::Cons(c) => {
                    let b = c.borrow();
                    // Macro/keymap autoloads are not callable: GNU
                    // FUNCTIONP accepts an autoload only with nil TYPE.
                    let auto_id = i.intern("autoload");
                    if i.sym_is(&b.car, sym::LAMBDA) {
                        true
                    } else if i.sym_is(&b.car, auto_id) {
                        let tail = super::listfn::nthcdr_of(&Value::Cons(c.clone()), 4);
                        !matches!(tail, Value::Cons(t) if t.borrow().car.truthy())
                    } else {
                        false
                    }
                }
                _ => false,
            }
        }
        Value::Cons(c) => {
            let b = c.borrow();
            i.sym_is(&b.car, sym::LAMBDA)
        }
        _ => false,
    };
    r
}
fn f_subrp(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    // Emacs: strict type check — (subrp 'car) is nil.  GNU 31.1
    // native-compiles its dumped libraries, so a Lisp-defined
    // `forward-paragraph' is a (nativecomp) subr; remacs marks dumped
    // definitions `compiled' — report those as subrs too.  Macros and
    // `#[...]' byte-code objects are still plain closures — unless they
    // were read while loading the bundled `.elc' dump blobs (`compiled'),
    // which stand in for GNU's native-compiled subrs and report t.
    let _ = i;
    Ok(Value::from_bool(match &args[0] {
        Value::Subr(_) => true,
        Value::Lambda(l) => l.compiled && !l.is_macro,
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
        Value::Lambda(l) if l.compiled => Ok(Value::Nil),
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
            let b = c.borrow();
            if i.sym_is(&b.car, sym::MACRO) {
                return Ok(Value::t());
            }
            let auto_id = i.intern("autoload");
            if i.sym_is(&b.car, auto_id) {
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
        Value::Cons(_) | Value::Nil | Value::Str(_) | Value::Vec(_) => true,
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
        Value::Int(_) | Value::Marker(_)
    )))
}
fn f_arrayp(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    // GNU arrays cover strings, vectors, char-tables, byte-code
    // objects, and bool-vectors; char-tables are Records here and
    // `#[...]' byte-code objects are Lambdas tagged by `bc_items'.
    Ok(Value::from_bool(match &args[0] {
        Value::Str(_) | Value::Vec(_) => true,
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
        Value::Int(_) => "integer",
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
            if crate::lisp::builtins::misc::as_sympos(&args[0]).is_some() {
                return Ok(Value::Sym(i.intern("symbol")));
            }
            // GNU reports an obarray as `obarray', not its tag-record
            // slot — obarrays are a distinct pseudovec type there.
            if is_obarray(i, &args[0]) {
                return Ok(Value::Sym(i.intern("obarray")));
            }
            let rr = r.borrow();
            match rr.first() {
                // GNU (`Fcl_type_of'): when slot 0 is itself a record
                // with size > 1 it is a class object and the reported
                // type is that object's name field (slot 1).
                Some(Value::Record(t)) if t.borrow().len() > 1 => {
                    return Ok(t.borrow()[1].clone());
                }
                // Otherwise GNU returns the record's type field
                // (slot 0) VERBATIM — a symbol for structs/EIEIO but
                // any object (e.g. `record' of a string) is reported
                // as-is, never folded to `record'.
                Some(first) => return Ok(first.clone()),
                None => {}
            }
            "record"
        }
        Value::Hash(_) => "hash-table",
        Value::Subr(_) => "subr",
        Value::Lambda(l) => {
            // GNU nativecomp parity: compiled lambdas (dumped source
            // definitions and `#[...]' read from a bundled `.elc') report
            // `subr'; raw `#[...]' literals stay `byte-code-function'.
            if l.compiled {
                "subr"
            } else if l.is_bytecode() {
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
    Ok(Value::Sym(i.intern(name)))
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
fn f_symbol_name(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    let id = want_sym(i, &args[0])?;
    Ok(Value::string(i.symbol_name(id)))
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
    // (add-variable-watcher SYM WATCH-FN) — watcher is called as
    // (SYM NEWVAL OP WHERE) after each change.
    let id = want_sym(i, &args[0])?;
    // GNU add_variable_watcher: SYMBOLP on SYM; the watcher accepts any
    // symbol (it needn't be fbound) or a non-symbol function.
    // GNU add_variable_watcher stores WATCH-FN unchecked — a bogus
    // watcher only fails later, at fire time.
    i.var_watchers.push((id, args[1].clone()));
    Ok(Value::Nil)
}
fn f_get_variable_watchers(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    // GNU eval.c get watchers: non-symbol argument returns nil, no
    // type check (watchers are keyed on symbols only).
    let id = match i.sym_id(&args[0]) {
        Some(id) => id,
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
    if let Some(pos) = i
        .var_watchers
        .iter()
        .position(|(s, f)| *s == id && crate::lisp::builtins::eq_values(f, &args[1]))
    {
        i.var_watchers.remove(pos);
    }
    Ok(Value::Nil)
}
fn f_fset(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    let id = want_sym(i, &args[0])?;
    i.check_function_assignment(id, &args[1])?;
    let def = normalize_fn_def(i, args[1].clone());
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
    if let Value::Sym(start) = &def {
        let mut seen: std::collections::HashSet<SymId> = std::collections::HashSet::new();
        let mut cur = *start;
        loop {
            if cur == id || !seen.insert(cur) {
                if cur == id {
                    let sig = i.intern("cyclic-function-indirection");
                    return Err(i.signal_data(sig, vec![Value::Sym(id)]));
                }
                break;
            }
            match i.symbol_function(cur) {
                Value::Sym(next) if next == sym::UNBOUND => break,
                Value::Sym(next) => cur = next,
                _ => break,
            }
        }
    }
    // GNU data.c `defalias' LOADHIST_ATTACHes `(defun . SYM)' when the
    // definition is installed — source `defun'/`defmacro' forms and
    // `.elc' defalias calls alike (an `(autoload ...)' cell counts too).
    let defun_id = i.intern("defun");
    crate::lisp::builtins::evalfn::loadhist_attach(
        i,
        Value::cons(i.sym(defun_id), args[0].clone()),
    );
    i.fset_defalias(id, def)?;
    // GNU: a non-nil DOCSTRING goes on SYMBOL's `function-documentation'
    // property — what `documentation' reads through the alias.
    if let Some(doc) = args.get(2) {
        if doc.truthy() {
            let fd = i.intern("function-documentation");
            i.put_prop(id, fd, doc.clone());
        }
    }
    // Emacs returns the aliased symbol, not the definition.
    Ok(args[0].clone())
}
fn f_defvaralias(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    // GNU eval.c Fdefvaralias ordering: CHECK_SYMBOL both, constant /
    // forwarded / localized alias rejection, cyclic chain check on the
    // BASE's indirection chain, bound-alias value propagation to an
    // unbound BASE, let-bound-alias rejection, then install + mark
    // both special + (possibly nil) docstring store.
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
                let sig = i.intern("cyclic-variable-indirection");
                return Err(i.signal_data(sig, vec![args[1].clone()]));
            }
            if !seen.insert(cur) {
                break;
            }
            let alias_prop = i.intern("variable-alias");
            match i.get_prop(cur, alias_prop) {
                Value::Sym(next) => cur = next,
                _ => break,
            }
        }
    }
    if i.obarray.symbol(newv).always_local
        || i.obarray.symbol(newv).make_local_if_set
        || i.buffers.list().iter().any(|bid| {
            i.buffers.get(*bid).is_some_and(|b| b.borrow().locals.contains_key(&newv))
        })
    {
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
    let prop = i.intern("variable-alias");
    i.put_prop(newv, prop, args[1].clone());
    i.obarray.symbol_mut(newv).special = true;
    i.obarray.symbol_mut(base).special = true;
    // GNU stores docstring unconditionally — nil ERASES an old one.
    let vd = i.intern("variable-documentation");
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

fn f_makunbound(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    let id = want_sym(i, &args[0])?;
    let id = i.var_alias_target(id);
    // GNU refuses to unbind constants (keywords, nil, t).
    if i.obarray.symbol(id).constant {
        return Err(i.signal_data(sym::SETTING_CONSTANT, vec![args[0].clone()]));
    }
    if i.dbg_flags & 0x4000 != 0 {
        let name = i.symbol_name(id);
        if std::env::var("WATCH_MAKUNBOUND")
            .unwrap()
            .split(',')
            .any(|w| w == name)
        {
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
        Some(Value::Buffer(b)) => b.borrow().id,
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
            if !b.borrow().live {
                return Err(i.error("Selecting deleted buffer"));
            }
            b.borrow().id
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
    if let Some(Value::Str(s)) = args.get(2) {
        // GNU: Fput(symbol, Qvariable_documentation, doc).
        let vd = i.intern("variable-documentation");
        i.put_prop(id, vd, Value::string(s.borrow().clone()));
    }
    Ok(args[0].clone())
}

fn f_defconst_1(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    // (defconst-1 SYM INITVALUE &optional DOCSTRING): defconst with
    // evaluated arguments — always sets the default.
    let id = want_sym(i, &args[0])?;
    i.obarray.symbol_mut(id).special = true;
    i.set_symbol_default(id, args[1].clone())?;
    if let Some(Value::Str(s)) = args.get(2) {
        // GNU: Fput(symbol, Qvariable_documentation, doc).
        let vd = i.intern("variable-documentation");
        i.put_prop(id, vd, Value::string(s.borrow().clone()));
    }
    let rlv = i.intern("risky-local-variable");
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
            let vd = i.intern("variable-documentation");
            i.put_prop(id, vd, doc.clone());
        }
    }
    // GNU eval.c `defvar' LOADHIST_ATTACHes the bare symbol.
    crate::lisp::builtins::evalfn::loadhist_attach(i, args[0].clone());
    Ok(Value::Nil)
}

fn f_delete_indirect_variable(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    // We don't create indirect variable bindings, so this always fails.
    let _ = want_sym(i, &args[0])?;
    // GNU signals (error "Cannot undeclare..." VARIABLE): the datum
    // carries the offending symbol.
    let sym = i.intern("error");
    Err(i.signal_data(
        sym,
        vec![
            Value::string("Cannot undeclare a variable that is not an alias"),
            args[0].clone(),
        ],
    ))
}

fn f_obarray_buckets(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    // GNU `check_obarray's the argument first — non-obarrays
    // signal `obarrayp'.  Our obarray isn't bucketed; expose one bucket
    // holding all symbols of the default obarray.
    let ob = resolve_obarray(i, args.get(0))?;
    if let Some(o) = &ob {
        if !is_default_obarray(i, o) {
            return Ok(Value::list(vec![Value::list(obarray_syms(i, o))]));
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

fn f_make_record(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    // (make-record TYPE LENGTH INITVAL)
    // GNU Fmake_record: LENGTH is CHECK_FIXNAT → `wholenump'.
    let n = match &args[1] {
        Value::Int(n) if *n >= 0 => *n as usize,
        _ => {
            let wholenump = Value::Sym(i.intern("wholenump"));
            return Err(i.signal_data(sym::WRONG_TYPE_ARGUMENT, vec![wholenump, args[1].clone()]));
        }
    };
    let mut v = Vec::with_capacity(n + 1);
    v.push(args[0].clone());
    v.extend(std::iter::repeat_n(args[2].clone(), n));
    Ok(Value::Record(std::rc::Rc::new(std::cell::RefCell::new(v))))
}

fn f_text_quoting_style(i: &mut Interp, _args: crate::lisp::Argv) -> EvalResult {
    // GNU Ftext_quoting_style returns text_quoting_style ().
    use super::evalfn::QuoteStyle;
    let name = match super::evalfn::text_quoting_style(i) {
        QuoteStyle::Grave => "grave",
        QuoteStyle::Straight => "straight",
        QuoteStyle::Curve => "curve",
    };
    Ok(Value::Sym(i.intern(name)))
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
    Ok(crate::lisp::eval::plist_get_v(
        &i.obarray.symbol(id).plist,
        &args[1],
    ))
}
fn f_put(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    let id = want_sym(i, &args[0])?;
    i.macro_gen += 1;
    let plist = i.obarray.symbol(id).plist.clone();
    let new_plist = crate::lisp::eval::plist_put_v(&plist, &args[1], args[2].clone());
    i.obarray.symbol_mut(id).plist = new_plist;
    if matches!(&args[1], Value::Sym(s) if *s == i.var_alias_prop) {
        i.obarray.symbol_mut(id).var_alias = match &args[2] {
            Value::Sym(s) => Some(*s),
            _ => None,
        };
    }
    Ok(args[2].clone())
}
fn f_setplist(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    let id = want_sym(i, &args[0])?;
    i.macro_gen += 1;
    i.obarray.symbol_mut(id).plist = args[1].clone();
    i.obarray.symbol_mut(id).var_alias =
        match crate::lisp::eval::plist_get(&args[1], i.var_alias_prop) {
            Value::Sym(s) => Some(s),
            _ => None,
        };
    Ok(args[1].clone())
}
fn f_intern(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    let ob = resolve_obarray(i, args.get(1))?;
    // GNU resolve_symbol_positions: a sympos argument is stripped to its
    // bare symbol before the string check — the byte compiler's
    // `read-positioning-symbols' reader flows sympos into `intern'/
    // `intern-soft' via cconv--warn-unused-msg and friends.
    let args0 = match crate::lisp::builtins::misc::as_sympos(&args[0]) {
        Some((s, _)) => Value::Sym(s),
        None => args[0].clone(),
    };
    match &args0 {
        Value::Str(s) => {
            let name = s.borrow().clone();
            // A custom obarray keeps its own symbol vector; symbols in
            // it are NOT interned in the global obarray (GNU parity:
            // intern-soft on the default obarray won't find them).
            if let Some(ob) = &ob {
                if !is_default_obarray(i, ob) {
                    if let Some(sym) = obarray_syms(i, ob)
                        .into_iter()
                        .find(|v| matches!(v, Value::Sym(s) if i.symbol_name(*s) == name))
                    {
                        return Ok(sym);
                    }
                    let id = i.make_symbol(&name);
                    let sym = i.sym(id);
                    obarray_push(i, ob, &sym);
                    return Ok(sym);
                }
            }
            let id = i.intern(&name);
            Ok(i.sym(id))
        }
        _ => Err(i.wrong_type_mut("stringp", &args[0])),
    }
}
fn f_intern_soft(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    let ob = resolve_obarray(i, args.get(1))?;
    // GNU resolve_symbol_positions: strip a sympos to its bare symbol.
    let args0 = match crate::lisp::builtins::misc::as_sympos(&args[0]) {
        Some((s, _)) => Value::Sym(s),
        None => args[0].clone(),
    };
    match &args0 {
        Value::Str(s) => {
            let name = s.borrow().clone();
            if let Some(ob) = &ob {
                if !is_default_obarray(i, ob) {
                    return Ok(obarray_syms(i, ob)
                        .into_iter()
                        .find(|v| matches!(v, Value::Sym(s) if i.symbol_name(*s) == name))
                        .unwrap_or(Value::Nil));
                }
            }
            match i.intern_soft(&name) {
                Some(id) => Ok(i.sym(id)),
                None => Ok(Value::Nil),
            }
        }
        Value::Sym(sid) => {
            // GNU: a symbol arg returns itself only when it is actually
            // interned in OBARRAY — `make-symbol' results yield nil.
            let name = i.symbol_name(*sid);
            let hit = match &ob {
                Some(ob) if !is_default_obarray(i, ob) => obarray_syms(i, ob)
                    .into_iter()
                    .any(|v| matches!(v, Value::Sym(x) if i.symbol_name(x) == name)),
                _ => i.intern_soft(&name).is_some(),
            };
            Ok(if hit { args[0].clone() } else { Value::Nil })
        }
        Value::Nil => Ok(Value::Nil), // GNU Fintern_soft: nil → nil, not stringp.
        _ => Err(i.wrong_type_mut("stringp", &args[0])),
    }
}
fn f_unintern(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    let ob = resolve_obarray(i, args.get(1))?;
    if let Some(ob) = &ob {
        if !is_default_obarray(i, ob) {
            // GNU still checks NAME is string-or-symbol before removing.
            if i.sym_id(&args[0]).is_none() && !matches!(args[0], Value::Str(_)) {
                return Err(i.wrong_type_mut("stringp", &args[0]));
            }
            return Ok(Value::from_bool(obarray_remove(i, ob, &args[0])));
        }
    }
    match &args[0] {
        _ if i.sym_id(&args[0]).is_some() => {
            let id = i.sym_id(&args[0]).unwrap();
            if id == sym::NIL || id == sym::T {
                // GNU refuses to unintern `nil'/`t' (returns nil).
                return Ok(Value::Nil);
            }
            // Our obarray can't physically remove (indices are stable),
            // but we can drop the name mapping so a fresh intern creates
            // a new symbol — matching unintern semantics.
            let name = i.symbol_name(id);
            // GNU Funintern returns t only when the obarray actually
            // held THIS symbol: an uninterned same-name twin removes
            // nothing and yields nil.
            if i.intern_soft(&name) == Some(id) {
                i.obarray.unintern_by_name(&name);
                return Ok(Value::t());
            }
            Ok(Value::Nil)
        }
        Value::Str(s) => {
            let name = s.borrow().clone();
            if i.intern_soft(&name).is_some() {
                i.obarray.unintern_by_name(&name);
                Ok(Value::t())
            } else {
                Ok(Value::Nil)
            }
        }
        // GNU Funintern: NAME must be a string-or-symbol — other types
        // signal `stringp' (the sym_id arm already covered symbols).
        _ => Err(i.wrong_type_mut("stringp", &args[0])),
    }
}
fn f_make_symbol(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    let name = match &args[0] {
        Value::Str(s) => s.borrow().clone(),
        _ => return Err(i.wrong_type_mut("stringp", &args[0])),
    };
    Ok(Value::Sym(i.make_symbol(&name)))
}
/// An obarray is represented as a record whose slot 0 is the symbol
/// `obarray` and whose slot 1 is a vector of buckets; GNU's obarrays
/// are a distinct type (vectorp → nil), which the record gives us.
fn obarray_tag(i: &Interp) -> Value {
    Value::Sym(i.intern_soft("obarray").unwrap_or(0))
}

pub(crate) fn is_obarray(i: &Interp, v: &Value) -> bool {
    match v {
        Value::Record(r) => {
            let rr = r.borrow();
            rr.first()
                .map(|t| eq_values(t, &obarray_tag(i)))
                .unwrap_or(false)
        }
        _ => false,
    }
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
pub(crate) fn obarray_syms(i: &Interp, ob: &Value) -> Vec<Value> {
    let _ = i;
    if let Value::Record(r) = ob {
        if let Some(Value::Vec(v)) = r.borrow().get(1) {
            return v
                .borrow()
                .iter()
                .filter(|x| matches!(x, Value::Sym(_)))
                .cloned()
                .collect();
        }
    }
    Vec::new()
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
            hash = hash.wrapping_shl(4).wrapping_add(hash >> 60).wrapping_add(c);
            p += step;
        }
        // Tail: always hash the last 8 bytes (may overlap).
        let c = u64::from_ne_bytes(name[name.len() - 8..].try_into().unwrap());
        hash = hash.wrapping_shl(4).wrapping_add(hash >> 60).wrapping_add(c);
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
        hash = hash.wrapping_shl(4).wrapping_add(hash >> 60).wrapping_add(tail);
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
    let Some(vrc) = vrc else {
        let _ = i;
        return;
    };
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

fn obarray_remove(i: &Interp, ob: &Value, sym: &Value) -> bool {
    if let Value::Record(r) = ob {
        if let Some(Value::Vec(v)) = r.borrow().get(1) {
            let mut v = v.borrow_mut();
            // Strings select by name; symbols (including positioned
            // symbols) must be the actual member, not a same-name twin.
            if let Some(pos) = v.iter().position(|candidate| match (sym, candidate) {
                (Value::Str(name), Value::Sym(id)) => i.symbol_name(*id) == *name.borrow(),
                (_, Value::Sym(id)) => i.sym_id(sym) == Some(*id),
                _ => false,
            }) {
                v.remove(pos);
                return true;
            }
        }
    }
    false
}

/// Check an optional OBARRAY argument and resolve it to the effective
/// obarray: nil/absent means the default obarray (None); a tagged
/// record is used directly; a VECTOR is GNU's legacy vector-as-obarray
/// — every slot must be 0 or a symbol hashed to that bucket
/// (obarray_slow check).  A valid vector is promoted by placing an
/// obarray record in slot 0, carrying over any pre-existing symbols.
/// Anything else is a type error.
pub(crate) fn resolve_obarray(i: &mut Interp, v: Option<&Value>) -> Result<Option<Value>, Flow> {
    match v {
        None | Some(Value::Nil) => Ok(None),
        Some(v) if is_obarray(i, v) => Ok(Some(v.clone())),
        Some(w @ Value::Vec(vec)) => {
            // GNU `check_obarray_slow': a non-empty vector is an
            // obarray iff slot 0 is 0 (fresh obarray object is stored
            // there) or an obarray already — the remaining slots are
            // IGNORED entirely (no per-slot bucket validation).
            let v0 = vec.borrow().first().cloned();
            if let Some(v0) = v0 {
                if is_obarray(i, &v0) {
                    return Ok(Some(v0));
                }
                if matches!(v0, Value::Int(0)) {
                    let rec = Value::Record(std::rc::Rc::new(std::cell::RefCell::new(
                        vec![
                            obarray_tag(i),
                            Value::Vec(std::rc::Rc::new(std::cell::RefCell::new(
                                Vec::new(),
                            ))),
                            // GNU uses make_obarray(0): one bucket.
                            Value::Int(0),
                        ],
                    )));
                    vec.borrow_mut()[0] = rec.clone();
                    return Ok(Some(rec));
                }
            }
            Err(i.wrong_type_mut("obarrayp", w))
        }
        Some(v) => Err(i.wrong_type_mut("obarrayp", v)),
    }
}

fn f_mapatoms(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    let fun = args[0].clone();
    let ob = resolve_obarray(i, args.get(1))?;
    if let Some(ob) = &ob {
        if !is_default_obarray(i, ob) {
            for sym in obarray_syms(i, ob) {
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
    let mut cur = crate::lisp::builtins::misc::unpos(&args[0]);
    let mut seen: std::collections::HashSet<crate::lisp::value::SymId> =
        std::collections::HashSet::new();
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
                        let sig = i.intern("cyclic-function-indirection");
                        return Err(i.signal_data(sig, vec![Value::Sym(id)]));
                    }
                    Value::Sym(next) => cur = i.sym(next),
                    other => return Ok(other),
                }
            }
            other => return Ok(other),
        }
    }
}
pub(crate) fn f_interactive_form(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    let mut f = match &args[0] {
        Value::Sym(id) => i.symbol_function(*id),
        other => other.clone(),
    };
    // Advised functions: GNU's interactive-form delegates through the
    // advice layers to the underlying definition (modulo
    // :interactive-only merging — we expose the base spec).
    f = i.advice_base_value(&f);
    if let Value::Sym(id) = f {
        f = i.symbol_function(id);
    }
    match &f {
        Value::Subr(s) => {
            // GNU returns `(interactive SPEC)' for primitives whose
            // intspec is non-nil.  Specs beginning with `(' are forms
            // read from the string rather than callint letter codes.
            match crate::lisp::eval::subr_interactive_form(i, s.name) {
                Some(spec) => Ok(spec),
                None => Ok(Value::Nil),
            }
        }
        _ => match f.as_lambda() {
            Some(l) => {
                let raw = l.interactive.clone().or_else(|| bc_interactive_form(i, &l));
                match raw {
                    Some(spec) => Ok(normalize_interactive_spec(i, spec)),
                    // GNU data.c: OClosure-typed functions fall through to
                    // `calln(oclosure-interactive-form, fun)' when the
                    // generic exists.
                    None => {
                        let oif = i.intern_soft("oclosure-interactive-form");
                        // OClosure: bc_items[4] (the docstring slot)
                        // holds the type symbol; GNU delegates to
                        // `oclosure-interactive-form' whenever the
                        // generic exists (a symbol tag suffices — the
                        // default method returns nil).
                        let is_oclosure = match &f {
                            Value::Lambda(l) => {
                                l.bc_items
                                    .as_ref()
                                    .and_then(|it| it.borrow().get(4).cloned())
                                    .map(|v| matches!(v, Value::Sym(_)))
                                    .unwrap_or(false)
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
            None => Ok(interactive_form_list(i, &f)),
        },
    }
}

/// The `(interactive SPEC)' form of a `#[argdesc str consts depth
/// doc ispec]' byte-code object — GNU stores the interactive spec as
/// element 5 (Ffetch_bytecode / `interactive-form' read it from there).
pub(crate) fn bc_interactive_form(i: &mut Interp, l: &Rc<Lambda>) -> Option<Value> {
    let items = l.bc_items.as_ref()?.borrow();
    // GNU Fcommandp: `PVSIZE (fun) > CLOSURE_INTERACTIVE' — the mere
    // PRESENCE of slot 5 (even holding nil) makes the object a
    // command; `interactive-form' then reports `(interactive nil)'.
    // A 5-slot `#[args str consts depth doc]' has no spec at all.
    let spec = items.get(5)?;
    // A VECTOR in slot 5 is the command-modes form — the spec is its
    // first element (data.c Finteractive_form).
    let spec = match spec {
        Value::Vec(v) => v.borrow().first().cloned().unwrap_or(Value::Nil),
        s => s.clone(),
    };
    Some(Value::list(vec![Value::Sym(i.intern("interactive")), spec]))
}
/// GNU `documentation-function-or-1' semantics for `(macro . FUN)':
/// unwrap the macro cell so doc/subr lookups see the inner function.
pub(crate) fn macro_inner(i: &mut Interp, f: Value) -> Value {
    let m = i.intern("macro");
    match &f {
        Value::Cons(c) if i.sym_id(&c.borrow().car) == Some(m) => c.borrow().cdr.clone(),
        _ => f,
    }
}

/// GNU `interactive-form' result for an `(interactive ...)' element:
/// a bare `(interactive)' reads back as `(interactive nil)'.
fn normalize_interactive_spec(i: &mut Interp, v: Value) -> Value {
    let interactive = i.intern("interactive");
    match &v {
        Value::Cons(c)
            if i.sym_id(&c.borrow().car) == Some(interactive) && c.borrow().cdr.is_nil() =>
        {
            Value::list(vec![Value::Sym(interactive), Value::Nil])
        }
        _ => v,
    }
}

/// `interactive-form' over a cons-shaped function (unevaluated
/// lambda/closure/macro data), as in GNU's Finteractive_form.
fn interactive_form_list(i: &mut Interp, f: &Value) -> Value {
    let Value::Cons(_) = f else { return Value::Nil };
    let mut items: Vec<Value> = f.list_to_vec().unwrap_or_default();
    // `(macro . FUN)' — step inside the macro wrapper.
    if items
        .first()
        .and_then(|h| i.sym_id(h))
        .map(|s| i.symbol_name(s) == "macro")
        .unwrap_or(false)
    {
        if let Some(d) = items.get(1) {
            items = d.list_to_vec().unwrap_or_default();
        }
    }
    let head = items
        .first()
        .and_then(|h| i.sym_id(h))
        .map(|s| i.symbol_name(s));
    let body_at = match head.as_deref() {
        Some("lambda") => 2,
        Some("closure") => 3,
        _ => return Value::Nil,
    };
    let interactive = i.intern("interactive");
    let declare = i.intern("declare");
    let ideclare = i.intern("interactive-declare");
    for el in &items[body_at.min(items.len())..] {
        if matches!(el, Value::Str(_)) {
            continue;
        }
        let head_id = match el {
            Value::Cons(c) => i.sym_id(&c.borrow().car),
            _ => None,
        };
        if head_id == Some(interactive) {
            // Unevaluated lambda/closure data reads back verbatim —
            // GNU returns the raw `(interactive)' (no nil padding).
            return el.clone();
        }
        if head_id == Some(declare) || head_id == Some(ideclare) {
            continue;
        }
        break;
    }
    Value::Nil
}
fn f_variable_binding_locus(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    let id = want_sym(i, &args[0])?;
    if let Some(b) = i.buffers.get(i.current_buffer) {
        if b.borrow().locals.contains_key(&id) {
            let buf = i.buffers.get(i.current_buffer).unwrap().clone();
            return Ok(Value::Buffer(buf));
        }
    }
    Ok(Value::Nil)
}
fn f_subr_name(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    match &args[0] {
        Value::Subr(s) => Ok(Value::string(s.name)),
        // Compiled lambdas print as `#<subr NAME>' like GNU's native
        // functions; `subr-name' must serve the same name.
        Value::Lambda(l) if l.compiled => match i.lambda_name(l) {
            Some(n) => Ok(Value::string(n)),
            None => Err(i.wrong_type_mut("subrp", &args[0])),
        },
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
                let head = c.borrow().car.clone();
                match i.sym_id(&head) {
                    Some(s) if matches!(i.symbol_name(s).as_str(), "lambda" | "closure") => {
                        return Ok(args[2].clone());
                    }
                    _ => {
                        let swp = i.intern("symbol-with-pos-p");
                        let symbolp = i.intern("symbolp");
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
                let swp = i.intern("symbol-with-pos-p");
                let symbolp = i.intern("symbolp");
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
    Ok(Value::Record(std::rc::Rc::new(std::cell::RefCell::new(
        vec![
            obarray_tag(i),
            Value::Vec(std::rc::Rc::new(std::cell::RefCell::new(Vec::new()))),
            Value::Int(bits as i128),
        ],
    ))))
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
        return Ok(Value::Nil);
    }
    if let Value::Record(r) = &ob {
        if let Some(Value::Vec(v)) = r.borrow().get(1) {
            v.borrow_mut().clear();
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
