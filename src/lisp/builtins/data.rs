//! Type predicates and symbol-manipulation subrs.

use super::{S, eq_values, equal_values, want_sym};
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
        1,
        f_buffer_local_toplevel_value,
        "Toplevel buffer-local value of SYMBOL."
    ),
    S!(
        "set-buffer-local-toplevel-value",
        2,
        2,
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
    S!("defvar-1", 1, 3, f_defvar_1, "Internal defvar helper."),
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
    S!("defconst-1", 2, 3, f_defconst_1, "Internal defconst helper."),
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
    let fun = match &args[0] {
        Value::Sym(id) => i.symbol_function(*id),
        other => other.clone(),
    };
    match &fun {
        Value::Lambda(l) => return Ok(Value::from_bool(l.is_macro)),
        Value::Cons(c) => {
            let b = c.borrow();
            if i.sym_is(&b.car, sym::MACRO) {
                return Ok(Value::t());
            }
            let auto_id = i.intern("autoload");
            if i.sym_is(&b.car, auto_id) {
                // GNU: TYPE field is `t' for macro autoloads; the
                // return value is its tail, e.g. (t).
                let items = fun.list_to_vec().unwrap_or_default();
                if items.get(4).map(|v| v.truthy()).unwrap_or(false) {
                    return Ok(Value::list(vec![Value::t()]));
                }
            }
        }
        _ => {}
    }
    Ok(Value::Nil)
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
        Value::Thread(_) => "thread",
        Value::Mutex(_) => "mutex",
        Value::CondVar(_) => "condition-variable",
        Value::Finalizer(_) => "finalizer",
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
    // Present macros as (macro . fn) like Emacs; unbound cells read nil.
    match &f {
        Value::Sym(s) if *s == sym::UNBOUND => Ok(Value::Nil),
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
        if let Some(b) = i.buffers.get(i.current_buffer) {
            if let Ok(mut bb) = b.try_borrow_mut() {
                bb.locals.insert(id, Value::Sym(sym::UNBOUND));
            }
        }
        i.fire_var_watchers(id, &Value::Nil, "makunbound", Some(i.current_buffer))?;
    } else {
        i.obarray.symbol_mut(id).value = Value::Sym(sym::UNBOUND);
        i.fire_var_watchers(id, &Value::Nil, "makunbound", None)?;
    }
    Ok(args[0].clone())
}

fn f_default_toplevel_value(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let id = want_sym(i, &args[0])?;
    match i.default_toplevel_value(id) {
        Some(v) => Ok(v),
        None => Err(i.signal_data(sym::VOID_VARIABLE, vec![args[0].clone()])),
    }
}

fn f_buffer_local_toplevel_value(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let id = want_sym(i, &args[0])?;
    match i.buffer_local_toplevel_value(id, i.current_buffer) {
        Some(v) => Ok(v),
        None => Err(i.signal_data(sym::VOID_VARIABLE, vec![args[0].clone()])),
    }
}

fn f_set_buffer_local_toplevel_value(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let id = want_sym(i, &args[0])?;
    i.set_buffer_local_toplevel_value(id, i.current_buffer, args[1].clone());
    Ok(Value::Nil)
}

fn f_internal_subr_documentation(_i: &mut Interp, _args: Vec<Value>) -> EvalResult {
    // GNU returns t for primitive subrs (docstrings live in etc/DOC).
    Ok(Value::t())
}

fn f_defvar_1(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    // (defvar-1 SYM INITVALUE &optional DOCSTRING): defvar with
    // evaluated arguments.
    let id = want_sym(i, &args[0])?;
    i.obarray.symbol_mut(id).special = true;
    // Like `defvar': set the default only when the variable is void.
    if let Some(v) = args.get(1) {
        if !i.bound_p(id) {
            i.set_symbol_default(id, v.clone())?;
        }
    }
    if let Some(Value::Str(s)) = args.get(2) {
        let doc = s.borrow().clone();
        i.obarray.symbol_mut(id).variable_documentation = Some(doc);
    }
    Ok(args[0].clone())
}

fn f_defconst_1(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    // (defconst-1 SYM INITVALUE &optional DOCSTRING): defconst with
    // evaluated arguments — always sets the default.
    let id = want_sym(i, &args[0])?;
    i.obarray.symbol_mut(id).special = true;
    i.set_symbol_default(id, args[1].clone())?;
    if let Some(Value::Str(s)) = args.get(2) {
        let doc = s.borrow().clone();
        i.obarray.symbol_mut(id).variable_documentation = Some(doc);
    }
    Ok(args[0].clone())
}

fn f_define_uninitialized_variable(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    // (internal--define-uninitialized-variable SYM &optional DOC): mark
    // SYM special without giving it a value; optional doc is stored.
    let id = want_sym(i, &args[0])?;
    i.obarray.symbol_mut(id).special = true;
    if let Some(Value::Str(s)) = args.get(1) {
        let doc = s.borrow().clone();
        i.obarray.symbol_mut(id).variable_documentation = Some(doc);
    }
    Ok(Value::Nil)
}

fn f_delete_indirect_variable(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    // We don't create indirect variable bindings, so this always fails.
    let _ = want_sym(i, &args[0])?;
    Err(i.error("Cannot undeclare a variable that is not an alias"))
}

fn f_obarray_buckets(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    // GNU returns a list of buckets, each a list of interned symbols.
    // Our obarray isn't bucketed; expose one bucket holding all symbols
    // of the named obarray (only `obarray' itself is meaningful).
    let _ = &args;
    let names: Vec<Value> = i
        .obarray
        .all_ids()
        .into_iter()
        .filter(|id| !i.obarray.symbol(*id).uninterned)
        .map(Value::Sym)
        .collect();
    Ok(Value::list(vec![Value::list(names)]))
}

fn f_make_record(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    // (make-record TYPE LENGTH INITVAL)
    let n = match &args[1] {
        Value::Int(n) if *n >= 0 => *n as usize,
        _ => {
            let natnump = Value::Sym(i.intern("natnump"));
            return Err(i.signal_data(
                sym::WRONG_TYPE_ARGUMENT,
                vec![natnump, args[1].clone()],
            ));
        }
    };
    let mut v = Vec::with_capacity(n + 1);
    v.push(args[0].clone());
    v.extend(std::iter::repeat_n(args[2].clone(), n));
    Ok(Value::Record(std::rc::Rc::new(std::cell::RefCell::new(v))))
}

fn f_text_quoting_style(i: &mut Interp, _args: Vec<Value>) -> EvalResult {
    let id = i.intern("text-quoting-style");
    Ok(i.symbol_value(id))
}
fn f_fmakunbound(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let id = want_sym(i, &args[0])?;
    i.obarray.symbol_mut(id).function = Value::Sym(sym::UNBOUND);
    Ok(args[0].clone())
}
pub(crate) fn f_get(i: &mut Interp, args: Vec<Value>) -> EvalResult {
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
            // A custom obarray keeps its own symbol vector; symbols in
            // it are NOT interned in the global obarray (GNU parity:
            // intern-soft on the default obarray won't find them).
            if let Some(ob) = args.get(1) {
                if is_obarray(i, ob) {
                    if let Some(sym) = obarray_syms(i, ob)
                        .into_iter()
                        .find(|v| {
                            matches!(v, Value::Sym(s) if i.symbol_name(*s) == name)
                        })
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
        Value::Sym(_) => Ok(args[0].clone()),
        _ => Err(i.wrong_type_mut("stringp", &args[0])),
    }
}
fn f_intern_soft(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    want_obarray(i, args.get(1))?;
    match &args[0] {
        Value::Str(s) => {
            let name = s.borrow().clone();
            if let Some(ob) = args.get(1) {
                if is_obarray(i, ob) {
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
        Value::Sym(_) => Ok(args[0].clone()),
        _ => Err(i.wrong_type_mut("stringp", &args[0])),
    }
}
fn f_unintern(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    want_obarray(i, args.get(1))?;
    if let Some(ob) = args.get(1) {
        if is_obarray(i, ob) {
            return Ok(Value::from_bool(obarray_remove(i, ob, &args[0])));
        }
    }
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
/// The symbols interned in a custom (Record-based) obarray.
fn obarray_syms(i: &Interp, ob: &Value) -> Vec<Value> {
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

fn obarray_push(i: &Interp, ob: &Value, sym: &Value) {
    if let Value::Record(r) = ob {
        if let Some(Value::Vec(v)) = r.borrow().get(1) {
            let mut v = v.borrow_mut();
            if !v.iter().any(|x| eq_values(x, sym)) {
                v.push(sym.clone());
            }
            return;
        }
    }
    let _ = i;
}

fn obarray_remove(i: &Interp, ob: &Value, sym: &Value) -> bool {
    let name = match sym {
        Value::Sym(s) => i.symbol_name(*s),
        Value::Str(s) => s.borrow().clone(),
        _ => return false,
    };
    if let Value::Record(r) = ob {
        if let Some(Value::Vec(v)) = r.borrow().get(1) {
            let mut v = v.borrow_mut();
            if let Some(pos) = v.iter().position(|x| {
                matches!(x, Value::Sym(s) if i.symbol_name(*s) == name)
            }) {
                v.remove(pos);
                return true;
            }
        }
    }
    false
}

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
    if let Some(ob) = args.get(1) {
        if is_obarray(i, ob) {
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
fn f_obarray_make(i: &mut Interp, _args: Vec<Value>) -> EvalResult {
    // GNU's growable obarrays always start at size 4 regardless of the
    // SIZE argument.
    let n = 4usize;
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
