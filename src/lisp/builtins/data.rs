//! Type predicates and symbol-manipulation subrs.

use super::{S, eq_values, equal_values, want_int, want_sym};
use crate::lisp::Interp;
use crate::lisp::error::{EvalResult, Flow};
use crate::lisp::obarray::sym;
use crate::lisp::value::{Subr, Value};

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
        1,
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
    S!(
        "user-variable-p",
        1,
        1,
        f_user_variable_p,
        "t if VARIABLE is a user option."
    ),
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
        1,
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
        "obarray-size",
        1,
        1,
        f_obarray_size,
        "Number of slots in OBARRAY."
    ),
];

fn f_eq(_i: &mut Interp, args: Vec<Value>) -> EvalResult {
    Ok(Value::from_bool(eq_values(&args[0], &args[1])))
}

fn f_null(_i: &mut Interp, args: Vec<Value>) -> EvalResult {
    Ok(Value::from_bool(args[0].is_nil()))
}

fn f_equal(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    Ok(Value::from_bool(equal_values(i, &args[0], &args[1])))
}

fn f_consp(_i: &mut Interp, args: Vec<Value>) -> EvalResult {
    Ok(Value::from_bool(matches!(&args[0], Value::Cons(_))))
}
fn f_atom(_i: &mut Interp, args: Vec<Value>) -> EvalResult {
    Ok(Value::from_bool(!matches!(&args[0], Value::Cons(_))))
}
fn f_listp(_i: &mut Interp, args: Vec<Value>) -> EvalResult {
    Ok(Value::from_bool(matches!(
        &args[0],
        Value::Cons(_) | Value::Nil
    )))
}
fn f_nlistp(_i: &mut Interp, args: Vec<Value>) -> EvalResult {
    Ok(Value::from_bool(!matches!(
        &args[0],
        Value::Cons(_) | Value::Nil
    )))
}
fn f_symbolp(_i: &mut Interp, args: Vec<Value>) -> EvalResult {
    Ok(Value::from_bool(matches!(
        &args[0],
        Value::Sym(_) | Value::Nil
    )))
}
fn f_stringp(_i: &mut Interp, args: Vec<Value>) -> EvalResult {
    Ok(Value::from_bool(matches!(&args[0], Value::Str(_))))
}
fn f_vectorp(_i: &mut Interp, args: Vec<Value>) -> EvalResult {
    Ok(Value::from_bool(matches!(&args[0], Value::Vec(_))))
}
fn f_hash_table_p(_i: &mut Interp, args: Vec<Value>) -> EvalResult {
    Ok(Value::from_bool(matches!(&args[0], Value::Hash(_))))
}
fn f_functionp(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let v = &args[0];
    let r = match v {
        Value::Subr(_) | Value::Lambda(_) => true,
        Value::Sym(id) => {
            // a symbol is a function if its function cell is defined
            // (and isn't a macro).
            let f = i.symbol_function(*id);
            match f {
                // Special forms install pseudo-subrs but are not callable
                // functions: (functionp #'if) -> nil.
                Value::Subr(s) => crate::lisp::special::special_form(i.intern(s.name)).is_none(),
                Value::Lambda(l) => !l.is_macro,
                Value::Cons(c) => {
                    let b = c.borrow();
                    i.sym_is(&b.car, sym::LAMBDA)
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
    Ok(Value::from_bool(r))
}
fn f_subrp(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    // Emacs: strict type check — (subrp 'car) is nil.
    let _ = i;
    Ok(Value::from_bool(matches!(&args[0], Value::Subr(_))))
}
fn f_subr_primitive_p(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    // No byte-compiled/native-compiled subrs exist: primitive = subr.
    let _ = i;
    Ok(Value::from_bool(matches!(&args[0], Value::Subr(_))))
}
fn f_subr_type(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    // Emacs 31: returns nil for primitives, `built-in'/`special' only
    // for native-compiled subrs — we have none, so always nil.
    match &args[0] {
        Value::Subr(_) => Ok(Value::Nil),
        other => Err(i.wrong_type_mut("subrp", other)),
    }
}
fn f_macrop(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let r = match &args[0] {
        Value::Sym(id) => match i.symbol_function(*id) {
            Value::Lambda(l) => l.is_macro,
            Value::Cons(c) => {
                let b = c.borrow();
                i.sym_is(&b.car, sym::MACRO)
            }
            _ => false,
        },
        Value::Cons(c) => {
            let b = c.borrow();
            i.sym_is(&b.car, sym::MACRO)
        }
        _ => false,
    };
    Ok(Value::from_bool(r))
}
fn f_keywordp(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    match &args[0] {
        Value::Sym(id) => Ok(Value::from_bool(i.symbol_name(*id).starts_with(':'))),
        _ => Ok(Value::Nil),
    }
}
fn f_sequencep(_i: &mut Interp, args: Vec<Value>) -> EvalResult {
    Ok(Value::from_bool(matches!(
        &args[0],
        Value::Cons(_) | Value::Nil | Value::Str(_) | Value::Vec(_)
    )))
}
fn f_booleanp(_i: &mut Interp, args: Vec<Value>) -> EvalResult {
    Ok(Value::from_bool(matches!(
        &args[0],
        Value::Nil | Value::Sym(1) // t
    )))
}
fn f_characterp(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let _ = i;
    Ok(Value::from_bool(match &args[0] {
        Value::Int(n) => (0..=0x3f_ffff).contains(n),
        _ => false,
    }))
}
fn f_integer_or_marker_p(_i: &mut Interp, args: Vec<Value>) -> EvalResult {
    Ok(Value::from_bool(matches!(
        &args[0],
        Value::Int(_) | Value::Marker(_)
    )))
}
fn f_arrayp(_i: &mut Interp, args: Vec<Value>) -> EvalResult {
    Ok(Value::from_bool(matches!(
        &args[0],
        Value::Str(_) | Value::Vec(_)
    )))
}
fn f_user_variable_p(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    match &args[0] {
        Value::Sym(id) => {
            // user-variable: defvar'd or has custom-type prop.
            let doc = i.obarray.symbol(*id).variable_documentation.is_some();
            Ok(Value::from_bool(doc))
        }
        _ => Ok(Value::Nil),
    }
}
fn f_special_form_p(_i: &mut Interp, args: Vec<Value>) -> EvalResult {
    match &args[0] {
        Value::Sym(id) => Ok(Value::from_bool(
            crate::lisp::special::special_form(*id).is_some(),
        )),
        _ => Ok(Value::Nil),
    }
}
fn f_type_of(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let name = match &args[0] {
        Value::Nil => "symbol",
        Value::Int(_) => "integer",
        Value::Float(_) => "float",
        Value::Sym(_) => "symbol",
        Value::Cons(_) => "cons",
        Value::Window(_) => "window",
        Value::Frame(_) => "frame",
        Value::Str(_) => "string",
        Value::Vec(_) => "vector",
        Value::Record(r) => {
            let rr = r.borrow();
            if let Some(Value::Sym(tag)) = rr.first() {
                return Ok(Value::Sym(*tag));
            }
            "record"
        }
        Value::Hash(_) => "hash-table",
        Value::Subr(_) => "subr",
        Value::Lambda(_) => "interpreted-function",
        Value::Buffer(_) => "buffer",
        Value::Marker(_) => "marker",
        Value::Process(_) => "process",
    };
    Ok(Value::Sym(i.intern(name)))
}
fn f_symbol_value(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let id = want_sym(i, &args[0])?;
    let v = i.symbol_value(id);
    if let Value::Sym(s) = &v {
        if *s == sym::UNBOUND {
            return Err(i.signal_data(sym::VOID_VARIABLE, vec![args[0].clone()]));
        }
    }
    Ok(v)
}
fn f_symbol_name(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let id = want_sym(i, &args[0])?;
    Ok(Value::string(i.symbol_name(id)))
}
fn f_symbol_function(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let id = want_sym(i, &args[0])?;
    let f = i.symbol_function(id);
    // Present macros as (macro . fn) like Emacs.
    match &f {
        Value::Lambda(l) if l.is_macro => Ok(Value::cons(Value::Sym(i.intern("macro")), f)),
        _ => Ok(f),
    }
}
fn f_symbol_plist(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let id = want_sym(i, &args[0])?;
    Ok(i.obarray.symbol(id).plist.clone())
}
fn f_boundp(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let id = want_sym(i, &args[0])?;
    Ok(Value::from_bool(i.bound_p(id)))
}
fn f_fboundp(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let id = want_sym(i, &args[0])?;
    Ok(Value::from_bool(i.fbound_p(id)))
}
fn f_set(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let id = want_sym(i, &args[0])?;
    i.set_symbol(id, args[1].clone())?;
    Ok(args[1].clone())
}
fn f_add_variable_watcher(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    // (add-variable-watcher SYM WATCH-FN) — watcher is called as
    // (SYM NEWVAL OP WHERE) after each change.
    let id = want_sym(i, &args[0])?;
    let watcher = args[1].clone();
    // Refuse non-function watchers like Emacs does.
    let ok = match &watcher {
        Value::Sym(s) => i.fbound_p(*s),
        Value::Subr(_) | Value::Lambda(_) => true,
        Value::Cons(_) => true,
        _ => false,
    };
    if !ok {
        return Err(i.wrong_type_mut("functionp", &watcher));
    }
    i.var_watchers.push((id, watcher));
    Ok(Value::Nil)
}
fn f_get_variable_watchers(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let id = want_sym(i, &args[0])?;
    let list: Vec<Value> = i
        .var_watchers
        .iter()
        .filter(|(s, _)| *s == id)
        .map(|(_, f)| f.clone())
        .collect();
    Ok(Value::list(list))
}
fn f_remove_variable_watcher(i: &mut Interp, args: Vec<Value>) -> EvalResult {
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
fn f_fset(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let id = want_sym(i, &args[0])?;
    let def = normalize_fn_def(i, args[1].clone());
    i.fset(id, def.clone());
    Ok(def)
}
fn f_defalias(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let id = want_sym(i, &args[0])?;
    let def = normalize_fn_def(i, args[1].clone());
    i.fset(id, def);
    // Emacs returns the aliased symbol, not the definition.
    Ok(args[0].clone())
}
fn f_defvaralias(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let newv = want_sym(i, &args[0])?;
    let _base = want_sym(i, &args[1])?;
    let prop = i.intern("variable-alias");
    i.put_prop(newv, prop, args[1].clone());
    // Emacs returns BASE-VARIABLE.
    Ok(args[1].clone())
}

fn f_indirect_variable(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let id = want_sym(i, &args[0])?;
    Ok(i.sym(i.var_alias_target(id)))
}

fn f_makunbound(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let id = want_sym(i, &args[0])?;
    let id = i.var_alias_target(id);
    i.obarray.symbol_mut(id).value = Value::Sym(sym::UNBOUND);
    i.fire_var_watchers(id, &Value::Nil, "makunbound", None)?;
    Ok(args[0].clone())
}
fn f_fmakunbound(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let id = want_sym(i, &args[0])?;
    i.obarray.symbol_mut(id).function = Value::Sym(sym::UNBOUND);
    Ok(args[0].clone())
}
fn f_get(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let id = want_sym(i, &args[0])?;
    let prop = want_sym(i, &args[1])?;
    Ok(i.get_prop(id, prop))
}
fn f_put(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let id = want_sym(i, &args[0])?;
    let prop = want_sym(i, &args[1])?;
    i.put_prop(id, prop, args[2].clone());
    Ok(args[2].clone())
}
fn f_setplist(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let id = want_sym(i, &args[0])?;
    i.obarray.symbol_mut(id).plist = args[1].clone();
    Ok(args[1].clone())
}
fn f_intern(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    want_obarray(i, args.get(1))?;
    match &args[0] {
        Value::Str(s) => {
            let name = s.borrow().clone();
            let id = i.intern(&name);
            Ok(i.sym(id))
        }
        Value::Sym(_) => Ok(args[0].clone()),
        _ => Err(i.wrong_type_mut("stringp", &args[0])),
    }
}
fn f_intern_soft(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    want_obarray(i, args.get(1))?;
    match &args[0] {
        Value::Str(s) => {
            let name = s.borrow().clone();
            match i.intern_soft(&name) {
                Some(id) => Ok(i.sym(id)),
                None => Ok(Value::Nil),
            }
        }
        Value::Sym(_) => Ok(args[0].clone()),
        _ => Err(i.wrong_type_mut("stringp", &args[0])),
    }
}
fn f_unintern(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    want_obarray(i, args.get(1))?;
    match &args[0] {
        Value::Sym(id) => {
            // Our obarray can't physically remove (indices are stable),
            // but we can drop the name mapping so a fresh intern creates
            // a new symbol — matching unintern semantics.
            let name = i.symbol_name(*id);
            // Only if name actually maps to this symbol.
            if i.intern_soft(&name) == Some(*id) {
                // Remove mapping.
                i.obarray.unintern_by_name(&name);
            }
            Ok(Value::t())
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
        _ => Ok(Value::Nil),
    }
}
fn f_make_symbol(i: &mut Interp, args: Vec<Value>) -> EvalResult {
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

fn is_obarray(i: &Interp, v: &Value) -> bool {
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

/// Check an optional OBARRAY argument: nil means the default obarray,
/// a tagged record is an obarray, anything else is a type error.
fn want_obarray(i: &mut Interp, v: Option<&Value>) -> Result<(), Flow> {
    match v {
        None | Some(Value::Nil) => Ok(()),
        Some(v) if is_obarray(i, v) => Ok(()),
        Some(v) => Err(i.wrong_type_mut("obarrayp", v)),
    }
}

fn f_mapatoms(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let fun = args[0].clone();
    want_obarray(i, args.get(1))?;
    let ids = i.obarray.all_ids();
    for id in ids {
        i.apply(&fun, vec![i.sym(id)])?;
    }
    Ok(Value::Nil)
}
fn f_indirect_function(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let mut cur = args[0].clone();
    let mut hops = 0;
    loop {
        hops += 1;
        if hops > 64 {
            return Err(i.error("Symbol's function alias chain is circular"));
        }
        match cur {
            Value::Sym(id) => {
                let f = i.symbol_function(id);
                if let Value::Sym(s) = &f {
                    if *s == sym::UNBOUND {
                        return Ok(Value::Nil);
                    }
                }
                match f {
                    Value::Sym(next) => cur = i.sym(next),
                    other => return Ok(other),
                }
            }
            other => return Ok(other),
        }
    }
}
fn f_interactive_form(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let f = match &args[0] {
        Value::Sym(id) => i.symbol_function(*id),
        other => other.clone(),
    };
    match f.as_lambda() {
        Some(l) => Ok(l.interactive.clone().unwrap_or(Value::Nil)),
        None => Ok(Value::Nil),
    }
}
fn f_variable_binding_locus(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let id = want_sym(i, &args[0])?;
    if let Some(b) = i.buffers.get(i.current_buffer) {
        if b.borrow().locals.contains_key(&id) {
            let buf = i.buffers.get(i.current_buffer).unwrap().clone();
            return Ok(Value::Buffer(buf));
        }
    }
    Ok(Value::Nil)
}
fn f_subr_name(_i: &mut Interp, args: Vec<Value>) -> EvalResult {
    match &args[0] {
        Value::Subr(s) => Ok(Value::string(s.name)),
        _ => Err(_i.wrong_type_mut("subrp", &args[0])),
    }
}
fn f_function_get(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let id = want_sym(i, &args[0])?;
    let prop = want_sym(i, &args[1])?;
    Ok(i.get_prop(id, prop))
}
fn f_function_put(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let id = want_sym(i, &args[0])?;
    let prop = want_sym(i, &args[1])?;
    i.put_prop(id, prop, args[2].clone());
    Ok(args[2].clone())
}
fn f_obarray_make(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let n = match args.first() {
        Some(v) => want_int(i, v)?.max(0) as usize,
        None => 0,
    };
    Ok(Value::Record(std::rc::Rc::new(std::cell::RefCell::new(
        vec![
            obarray_tag(i),
            Value::Vec(std::rc::Rc::new(std::cell::RefCell::new(vec![
                Value::Nil;
                n
            ]))),
        ],
    ))))
}
fn f_obarrayp(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    Ok(Value::from_bool(is_obarray(i, &args[0])))
}
fn f_obarray_size(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    match &args[0] {
        Value::Record(r) => {
            let rr = r.borrow();
            if !rr
                .first()
                .map(|t| eq_values(t, &obarray_tag(i)))
                .unwrap_or(false)
            {
                return Err(i.wrong_type_mut("obarrayp", &args[0]));
            }
            match rr.get(1) {
                Some(Value::Vec(v)) => Ok(Value::Int(v.borrow().len() as i128)),
                _ => Ok(Value::Int(0)),
            }
        }
        other => Err(i.wrong_type_mut("obarrayp", other)),
    }
}
/// Normalize a function definition for `fset`/`defalias`: `(macro . f)`
/// becomes a macro Lambda when f is a lambda.
fn normalize_fn_def(i: &mut Interp, def: Value) -> Value {
    if let Value::Cons(c) = &def {
        let b = c.borrow();
        if i.sym_is(&b.car, sym::MACRO) {
            let inner = b.cdr.clone();
            drop(b);
            match inner {
                Value::Lambda(l) => {
                    // Mark the lambda as a macro by wrapping in a new
                    // Lambda with is_macro set.
                    let mut l2 = crate::lisp::value::Lambda {
                        is_macro: true,
                        required: l.required.clone(),
                        optional: l.optional.clone(),
                        rest: l.rest,
                        body: l.body.clone(),
                        env: l.env.clone(),
                        doc: l.doc.clone(),
                        interactive: l.interactive.clone(),
                        name: l.name.clone(),
                        bad_arglist: l.bad_arglist,
                        arglist: l.arglist.clone(),
                        plain: l.plain,
                    };
                    l2.is_macro = true;
                    return Value::Lambda(std::rc::Rc::new(l2));
                }
                other => return other,
            }
        }
    }
    def
}
