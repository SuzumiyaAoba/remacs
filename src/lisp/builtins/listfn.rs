//! List-manipulation subrs: car, cdr, cons, nth, append, member, etc.

use super::{S, arg, eq_values, equal_values, want_cons, want_int, want_list};
use crate::lisp::Interp;
use crate::lisp::error::{EvalResult, Flow};
use crate::lisp::obarray::sym;
use crate::lisp::value::{Subr, Value};

pub(crate) static SUBRS: &[Subr] = &[
    S!("car", 1, 1, f_car, "Return the car of LIST."),
    S!("cdr", 1, 1, f_cdr, "Return the cdr of LIST."),
    S!(
        "car-safe",
        1,
        1,
        f_car_safe,
        "Return the car of OBJECT if it is a cons."
    ),
    S!(
        "cdr-safe",
        1,
        1,
        f_cdr_safe,
        "Return the cdr of OBJECT if it is a cons."
    ),
    S!("cons", 2, 2, f_cons, "Create a new cons (CAR . CDR)."),
    S!("list", many 0, f_list, "Return a new list of the arguments."),
    S!(
        "make-list",
        2,
        2,
        f_make_list,
        "Return a list of LENGTH elements all INIT."
    ),
    S!("length", 1, 1, f_length, "Return the length of SEQUENCE."),
    S!(
        "safe-length",
        1,
        1,
        f_safe_length,
        "Return length of LIST, no error on circle."
    ),
    S!(
        "proper-list-p",
        1,
        1,
        f_proper_list_p,
        "Length of proper list, nil otherwise."
    ),
    S!("nth", 2, 2, f_nth, "Return the Nth element of LIST."),
    S!("nthcdr", 2, 2, f_nthcdr, "Take cdr N times on LIST."),
    S!(
        "last",
        1,
        2,
        f_last,
        "Return the last K elements of LIST as a list."
    ),
    S!(
        "butlast",
        1,
        2,
        f_butlast,
        "Return LIST without its last K elements."
    ),
    S!(
        "nbutlast",
        1,
        2,
        f_nbutlast,
        "Destructively remove last K elements."
    ),
    S!("append", many 0, f_append, "Concatenate lists into one list."),
    S!("nconc", many 0, f_nconc, "Destructively concatenate lists."),
    S!(
        "reverse",
        1,
        1,
        f_reverse,
        "Return a reversed copy of LIST."
    ),
    S!("setcar", 2, 2, f_setcar, "Set the car of CELL to NEWCAR."),
    S!("setcdr", 2, 2, f_setcdr, "Set the cdr of CELL to NEWCDR."),
    S!("rplaca", 2, 2, f_setcar, "Set the car of CELL to NEWCAR."),
    S!("rplacd", 2, 2, f_setcdr, "Set the cdr of CELL to NEWCDR."),
    S!(
        "member",
        2,
        2,
        f_member,
        "Tail of LIST whose car is ELT (equal)."
    ),
    S!("memq", 2, 2, f_memq, "Tail of LIST whose car is ELT (eq)."),
    S!(
        "memql",
        2,
        2,
        f_memql,
        "Tail of LIST whose car is ELT (eql)."
    ),
    S!(
        "member-if",
        2,
        2,
        f_member_if,
        "Tail of LIST whose car satisfies PRED."
    ),
    S!(
        "member-if-not",
        2,
        2,
        f_member_if_not,
        "Tail of LIST whose car fails PRED."
    ),
    S!(
        "assq",
        2,
        2,
        f_assq,
        "Element of ALIST whose car is eq KEY."
    ),
    S!(
        "assoc",
        2,
        3,
        f_assoc,
        "Element of ALIST whose car is equal KEY."
    ),
    S!(
        "rassq",
        2,
        2,
        f_rassq,
        "Element of ALIST whose cdr is eq KEY."
    ),
    S!(
        "rassoc",
        2,
        2,
        f_rassoc,
        "Element of ALIST whose cdr is equal KEY."
    ),
    S!(
        "assoc-default",
        2,
        4,
        f_assoc_default,
        "Element of ALIST whose car matches, eval cdr if fn."
    ),
    S!("delq", 2, 2, f_delq, "Delete elements eq to ELT from LIST."),
    S!(
        "delete",
        2,
        2,
        f_delete,
        "Delete elements equal to ELT from SEQ."
    ),
    S!(
        "copy-alist",
        1,
        1,
        f_copy_alist,
        "Copy ALIST including element conses."
    ),
    S!("copy-tree", 1, 2, f_copy_tree, "Copy a tree of conses."),
    S!(
        "list-tail",
        2,
        2,
        f_list_tail,
        "Return the tail of LIST after N elements."
    ),
    S!("list-length", 1, 1, f_length, "Return the length of LIST."),
    S!(
        "plist-get",
        2,
        3,
        f_plist_get,
        "Extract value from PLIST for PROP (eq)."
    ),
    S!(
        "plist-put",
        3,
        3,
        f_plist_put,
        "Set value in PLIST for PROP (eq)."
    ),
    S!(
        "plist-member",
        2,
        3,
        f_plist_member,
        "Non-nil if PROP in PLIST (eq)."
    ),
    S!(
        "lax-plist-get",
        2,
        2,
        f_lax_plist_get,
        "plist-get using equal."
    ),
    S!(
        "lax-plist-put",
        3,
        3,
        f_lax_plist_put,
        "plist-put using equal."
    ),
    // `lax-plist-member' was removed in GNU Emacs 31.
    S!(
        "take",
        2,
        2,
        f_take,
        "First N elements of LIST (or string prefix)."
    ),
    S!(
        "ntake",
        2,
        2,
        f_ntake,
        "First N elements of LIST, destructively."
    ),
    S!(
        "flatten-tree",
        1,
        1,
        f_flatten_tree,
        "Flatten nested conses into a list."
    ),
    S!("apply-partially", many 1, f_apply_partially, "Return closure prepending ARGS."),
    S!(
        "assoc-string",
        2,
        3,
        f_assoc_string,
        "assoc for string keys."
    ),
    S!(
        "assq-delete-all",
        2,
        2,
        f_assq_delete_all,
        "Delete elements whose car is eq KEY."
    ),
    S!(
        "rassq-delete-all",
        2,
        2,
        f_rassq_delete_all,
        "Delete elements whose cdr is eq KEY."
    ),
    S!(
        "equal-including-properties",
        2,
        2,
        f_equal_incl_props,
        "equal (text props not modeled: same as equal)."
    ),
    S!("caar", 1, 1, f_caar, ""),
    S!("cadr", 1, 1, f_cadr, ""),
    S!("cdar", 1, 1, f_cdar, ""),
    S!("cddr", 1, 1, f_cddr, ""),
    S!("caaar", 1, 1, f_caaar, ""),
    S!("caadr", 1, 1, f_caadr, ""),
    S!("cadar", 1, 1, f_cadar, ""),
    S!("caddr", 1, 1, f_caddr, ""),
    S!("cdaar", 1, 1, f_cdaar, ""),
    S!("cdadr", 1, 1, f_cdadr, ""),
    S!("cddar", 1, 1, f_cddar, ""),
    S!("cdddr", 1, 1, f_cdddr, ""),
    S!("caaaar", 1, 1, f_caaaar, ""),
    S!("caaadr", 1, 1, f_caaadr, ""),
    S!("caadar", 1, 1, f_caadar, ""),
    S!("caaddr", 1, 1, f_caaddr, ""),
    S!("cadaar", 1, 1, f_cadaar, ""),
    S!("cadadr", 1, 1, f_cadadr, ""),
    S!("caddar", 1, 1, f_caddar, ""),
    S!("cadddr", 1, 1, f_cadddr, ""),
    S!("cdaaar", 1, 1, f_cdaaar, ""),
    S!("cdaadr", 1, 1, f_cdaadr, ""),
    S!("cdadar", 1, 1, f_cdadar, ""),
    S!("cdaddr", 1, 1, f_cdaddr, ""),
    S!("cddaar", 1, 1, f_cddaar, ""),
    S!("cddadr", 1, 1, f_cddadr, ""),
    S!("cdddar", 1, 1, f_cdddar, ""),
    S!("cddddr", 1, 1, f_cddddr, ""),
    S!(
        "remove",
        2,
        2,
        f_remove,
        "Copy SEQUENCE with ELT `equal' elements removed."
    ),
    S!(
        "remq",
        2,
        2,
        f_remq,
        "Copy LIST with ELT `eq' elements removed."
    ),
    // `first' .. `tenth' were removed in GNU Emacs 31 (subr-x dropped
    // them); the `cl-*' aliases remain and are defined in the prelude.
    S!("car-or-marker-p", 1, 1, f_car_or_marker_p, ""),
];

/// Wrap a computed `Value` in `(quote v)` so `call_function`'s
/// `eval_args` returns it verbatim.
fn quoted(v: Value) -> Value {
    Value::list(vec![Value::Sym(sym::QUOTE), v])
}

fn f_car(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    match &args[0] {
        Value::Cons(c) => Ok(c.borrow().car.clone()),
        Value::Nil => Ok(Value::Nil),
        other => Err(i.wrong_type_mut("listp", other)),
    }
}
fn f_cdr(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    match &args[0] {
        Value::Cons(c) => Ok(c.borrow().cdr.clone()),
        Value::Nil => Ok(Value::Nil),
        other => Err(i.wrong_type_mut("listp", other)),
    }
}
fn f_car_or_marker_p(_i: &mut Interp, args: Vec<Value>) -> EvalResult {
    Ok(Value::from_bool(matches!(
        &args[0],
        Value::Cons(_) | Value::Marker(_)
    )))
}
fn f_car_safe(_i: &mut Interp, args: Vec<Value>) -> EvalResult {
    Ok(match &args[0] {
        Value::Cons(c) => c.borrow().car.clone(),
        _ => Value::Nil,
    })
}
fn f_cdr_safe(_i: &mut Interp, args: Vec<Value>) -> EvalResult {
    Ok(match &args[0] {
        Value::Cons(c) => c.borrow().cdr.clone(),
        _ => Value::Nil,
    })
}
fn f_cons(_i: &mut Interp, args: Vec<Value>) -> EvalResult {
    Ok(Value::cons(args[0].clone(), args[1].clone()))
}
fn f_list(_i: &mut Interp, args: Vec<Value>) -> EvalResult {
    Ok(Value::list(args))
}
fn f_make_list(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let n = want_int(i, &args[0])?;
    if n < 0 {
        return Err(i.wrong_type_mut("wholenump", &args[0]));
    }
    Ok(Value::list(vec![args[1].clone(); n as usize]))
}
fn f_length(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    match &args[0] {
        Value::Nil => Ok(Value::Int(0)),
        Value::Cons(_) => {
            let v = want_list(i, &args[0])?;
            Ok(Value::Int(v.len() as i128))
        }
        Value::Str(s) => Ok(Value::Int(s.borrow().chars().count() as i128)),
        Value::Vec(v) => Ok(Value::Int(v.borrow().len() as i128)),
        other if crate::lisp::builtins::misc::is_bool_vector(i, other) => {
            // Bool-vectors are sequences of bit length.
            if let Value::Record(r) = other {
                if let Some(Value::Vec(b)) = r.borrow().get(1) {
                    return Ok(Value::Int(b.borrow().len() as i128));
                }
            }
            Ok(Value::Int(0))
        }
        // Records (incl. EIEIO instances) report their slot count.
        Value::Record(r) => Ok(Value::Int(r.borrow().len() as i128)),
        other => Err(i.wrong_type_mut("sequencep", other)),
    }
}
fn f_safe_length(_i: &mut Interp, args: Vec<Value>) -> EvalResult {
    // GNU safe_length: tortoise advances every other step; stop when the
    // walk lands back on it (covers circular lists without error).
    let mut n = 0i128;
    let mut cur = args[0].clone();
    let mut halftail = args[0].clone();
    loop {
        match &cur {
            Value::Cons(c) => {
                if n > 0 && eq_values(&cur, &halftail) {
                    break;
                }
                let next = {
                    let b = c.borrow();
                    b.cdr.clone()
                };
                n += 1;
                cur = next;
                if n % 2 == 0 {
                    if let Value::Cons(h) = &halftail {
                        let hnext = h.borrow().cdr.clone();
                        halftail = hnext;
                    }
                }
            }
            _ => break,
        }
    }
    Ok(Value::Int(n))
}
fn f_proper_list_p(_i: &mut Interp, args: Vec<Value>) -> EvalResult {
    match &args[0] {
        Value::Nil => Ok(Value::Int(0)),
        Value::Cons(_) => match args[0].list_to_vec() {
            Ok(v) => Ok(Value::Int(v.len() as i128)),
            Err(_) => Ok(Value::Nil),
        },
        _ => Ok(Value::Nil),
    }
}

/// cdr N times (no error on non-list tail).
pub(crate) fn nthcdr_of(v: &Value, n: usize) -> Value {
    let mut cur = v.clone();
    for _ in 0..n {
        match cur {
            Value::Cons(c) => {
                let next = {
                    let b = c.borrow();
                    b.cdr.clone()
                };
                cur = next;
            }
            _ => return Value::Nil,
        }
    }
    cur
}

fn f_nth(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    // Emacs: a negative index behaves like 0.
    let n = want_int(i, &args[0])?.max(0);
    let tail = nthcdr_of(&args[1], n as usize);
    match tail {
        Value::Cons(c) => Ok(c.borrow().car.clone()),
        _ => Ok(Value::Nil),
    }
}
fn f_nthcdr(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let n = want_int(i, &args[0])?.max(0);
    Ok(nthcdr_of(&args[1], n as usize))
}
fn f_last(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let k = args
        .get(1)
        .map(|v| want_int(i, v))
        .transpose()?
        .unwrap_or(1);
    // Emacs: return the last K cons cells, keeping a dotted tail.
    // Walk conses so (last '(1 . 2)) => (1 . 2).
    let mut cells: Vec<Value> = Vec::new();
    let mut cur = args[0].clone();
    let mut guard = 0usize;
    loop {
        guard += 1;
        if guard > 500_000 {
            return Err(err_circular(i));
        }
        match &cur {
            Value::Cons(c) => {
                let next = c.borrow().cdr.clone();
                cells.push(cur.clone());
                cur = next;
            }
            // A non-list tail just ends the walk (dotted list).
            _ => break,
        }
    }
    let n = cells.len();
    let start = if k <= 0 {
        n
    } else {
        n.saturating_sub(k as usize)
    };
    Ok(cells.get(start).cloned().unwrap_or(Value::Nil))
}
fn f_butlast(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let k = args
        .get(1)
        .map(|v| want_int(i, v))
        .transpose()?
        .unwrap_or(1);
    let items = want_list(i, &args[0])?;
    let n = items.len();
    let keep = if k <= 0 {
        n
    } else {
        n.saturating_sub(k as usize)
    };
    Ok(Value::list(items[..keep].to_vec()))
}
fn f_nbutlast(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let k = args
        .get(1)
        .map(|v| want_int(i, v))
        .transpose()?
        .unwrap_or(1);
    let items = want_list(i, &args[0])?;
    let n = items.len();
    let keep = if k <= 0 {
        n
    } else {
        n.saturating_sub(k as usize)
    };
    if keep == 0 {
        return Ok(Value::Nil);
    }
    // Set the cdr of the `keep`th cons to nil.
    let tail = nthcdr_of(&args[0], keep - 1);
    if let Value::Cons(c) = tail {
        c.borrow_mut().cdr = Value::Nil;
    }
    Ok(args[0].clone())
}
fn f_append(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    if args.is_empty() {
        return Ok(Value::Nil);
    }
    let mut out = Vec::new();
    for a in &args[..args.len() - 1] {
        // Emacs accepts any sequence for non-last args.
        out.extend(crate::lisp::builtins::seq::seq_to_vec(i, a)?);
    }
    // Last arg is the tail (may be non-list → dotted).
    let tail = args.last().unwrap().clone();
    let mut result = tail;
    for item in out.into_iter().rev() {
        result = Value::cons(item, result);
    }
    Ok(result)
}
fn f_nconc(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let mut result = Value::Nil;
    let mut last_cons: Option<crate::lisp::value::ConsRef> = None;
    for a in &args {
        match a {
            Value::Nil => continue,
            Value::Cons(_) => {
                // find last cons of `a`
                let mut tail = a.clone();
                let mut guard = 0;
                let last = loop {
                    guard += 1;
                    if guard > 1_000_000 {
                        return Err(i.error("circular list in nconc"));
                    }
                    match &tail {
                        Value::Cons(c) => {
                            let next = {
                                let b = c.borrow();
                                b.cdr.clone()
                            };
                            match next {
                                Value::Cons(_) => tail = next,
                                Value::Nil => break c.clone(),
                                _ => {
                                    // dotted tail inside arg — attach it
                                    // too, then continue with it as the
                                    // effective last cell.
                                    break c.clone();
                                }
                            }
                        }
                        _ => unreachable!(),
                    }
                };
                match &last_cons {
                    Some(c) => c.borrow_mut().cdr = a.clone(),
                    None => result = a.clone(),
                }
                last_cons = Some(last);
            }
            other => {
                // Atom arg becomes the dotted tail (like append).
                match &last_cons {
                    Some(c) => c.borrow_mut().cdr = other.clone(),
                    None => result = other.clone(),
                }
            }
        }
    }
    Ok(result)
}
fn f_reverse(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    // Emacs: reverse works on any sequence, preserving its type.
    match &args[0] {
        Value::Str(s) => {
            return Ok(Value::string(s.borrow().chars().rev().collect::<String>()));
        }
        Value::Vec(v) => {
            return Ok(Value::Vec(std::rc::Rc::new(std::cell::RefCell::new(
                v.borrow().iter().rev().cloned().collect(),
            ))));
        }
        _ => {}
    }
    let items = want_list(i, &args[0])?;
    Ok(Value::list(items.into_iter().rev().collect()))
}
fn f_setcar(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let c = want_cons(i, &args[0])?;
    c.borrow_mut().car = args[1].clone();
    Ok(args[1].clone())
}
fn f_setcdr(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let c = want_cons(i, &args[0])?;
    c.borrow_mut().cdr = args[1].clone();
    Ok(args[1].clone())
}

fn member_impl(
    i: &mut Interp,
    elt: &Value,
    list: &Value,
    cmp: fn(&Interp, &Value, &Value) -> bool,
) -> EvalResult {
    let mut cur = list.clone();
    let mut tortoise = list.clone();
    let mut guard = 0usize;
    loop {
        match &cur {
            Value::Cons(c) => {
                guard += 1;
                if guard > 500_000 {
                    return Err(err_circular(i));
                }
                let (car, next) = {
                    let b = c.borrow();
                    (b.car.clone(), b.cdr.clone())
                };
                if cmp(i, elt, &car) {
                    return Ok(cur);
                }
                cur = next;
                if guard % 2 == 0 {
                    if let Value::Cons(tc) = &tortoise {
                        let tnext = tc.borrow().cdr.clone();
                        tortoise = tnext;
                    }
                }
                if guard > 1 && eq_values(&cur, &tortoise) {
                    return Err(err_circular(i));
                }
            }
            Value::Nil => return Ok(Value::Nil),
            other => return Err(i.wrong_type_mut("listp", other)),
        }
    }
}

fn f_member(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    member_impl(i, &args[0], &args[1], |ii, a, b| equal_values(ii, a, b))
}
fn f_memq(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    member_impl(i, &args[0], &args[1], |_ii, a, b| eq_values(a, b))
}
fn f_memql(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    member_impl(i, &args[0], &args[1], |_ii, a, b| super::eql_values(a, b))
}

fn member_if_impl(i: &mut Interp, pred: &Value, list: &Value, want: bool) -> EvalResult {
    let mut cur = list.clone();
    let mut guard = 0usize;
    loop {
        match &cur {
            Value::Cons(c) => {
                guard += 1;
                if guard > 500_000 {
                    return Err(err_circular(i));
                }
                let (car, next) = {
                    let b = c.borrow();
                    (b.car.clone(), b.cdr.clone())
                };
                if i.apply(pred, vec![car])?.truthy() == want {
                    return Ok(cur);
                }
                cur = next;
            }
            Value::Nil => return Ok(Value::Nil),
            other => return Err(i.wrong_type_mut("listp", other)),
        }
    }
}

fn f_member_if(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    member_if_impl(i, &args[0], &args[1], true)
}
fn f_member_if_not(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    member_if_impl(i, &args[0], &args[1], false)
}

fn assoc_impl(
    i: &mut Interp,
    key: &Value,
    list: &Value,
    cmp: fn(&Interp, &Value, &Value) -> bool,
    cdr_cmp: bool,
) -> EvalResult {
    let mut cur = list.clone();
    let mut tortoise = list.clone();
    let mut guard = 0usize;
    loop {
        match &cur {
            Value::Cons(c) => {
                guard += 1;
                if guard > 500_000 {
                    return Err(err_circular(i));
                }
                let (elem, next) = {
                    let b = c.borrow();
                    (b.car.clone(), b.cdr.clone())
                };
                if let Value::Cons(ec) = &elem {
                    let eb = ec.borrow();
                    let probe = if cdr_cmp { &eb.cdr } else { &eb.car };
                    if cmp(i, key, probe) {
                        return Ok(elem.clone());
                    }
                }
                cur = next;
                if guard % 2 == 0 {
                    if let Value::Cons(tc) = &tortoise {
                        let tnext = tc.borrow().cdr.clone();
                        tortoise = tnext;
                    }
                }
                if guard > 1 && eq_values(&cur, &tortoise) {
                    return Err(err_circular(i));
                }
            }
            Value::Nil => return Ok(Value::Nil),
            other => return Err(i.wrong_type_mut("listp", other)),
        }
    }
}

fn f_assq(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    assoc_impl(i, &args[0], &args[1], |_ii, a, b| eq_values(a, b), false)
}

fn delete_all_by(i: &mut Interp, args: Vec<Value>, on_cdr: bool) -> EvalResult {
    // Rebuild the alist without elements whose car/cdr is `eq' KEY.
    let mut cur = args[1].clone();
    let mut keep = Vec::new();
    let mut guard = 0usize;
    loop {
        guard += 1;
        if guard > 500_000 {
            return Err(err_circular(i));
        }
        match &cur {
            Value::Nil => break,
            Value::Cons(c) => {
                let (item, rest) = {
                    let b = c.borrow();
                    (b.car.clone(), b.cdr.clone())
                };
                let drop_it = match &item {
                    Value::Cons(ic) => {
                        let part = {
                            let b = ic.borrow();
                            if on_cdr { b.cdr.clone() } else { b.car.clone() }
                        };
                        eq_values(&part, &args[0])
                    }
                    _ => false,
                };
                if !drop_it {
                    keep.push(item);
                }
                cur = rest;
            }
            other => return Err(i.wrong_type_mut("listp", other)),
        }
    }
    Ok(Value::list(keep))
}

fn f_assq_delete_all(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    delete_all_by(i, args, false)
}

fn f_rassq_delete_all(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    delete_all_by(i, args, true)
}
fn f_assoc(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    // (assoc KEY ALIST &optional TESTFN)
    if let Some(testfn) = args.get(2) {
        if testfn.truthy() {
            let mut cur = args[1].clone();
            let mut tortoise = args[1].clone();
            let mut guard = 0usize;
            loop {
                match &cur {
                    Value::Cons(c) => {
                        guard += 1;
                        if guard > 500_000 {
                            return Err(err_circular(i));
                        }
                        let (elem, next) = {
                            let b = c.borrow();
                            (b.car.clone(), b.cdr.clone())
                        };
                        if let Value::Cons(ec) = &elem {
                            let k = {
                                let eb = ec.borrow();
                                eb.car.clone()
                            };
                            let r = i.apply(testfn, vec![k, args[0].clone()])?;
                            if r.truthy() {
                                return Ok(elem.clone());
                            }
                        }
                        cur = next;
                        if guard % 2 == 0 {
                            if let Value::Cons(tc) = &tortoise {
                                let tnext = tc.borrow().cdr.clone();
                                tortoise = tnext;
                            }
                        }
                        if guard > 1 && eq_values(&cur, &tortoise) {
                            return Err(err_circular(i));
                        }
                    }
                    _ => return Ok(Value::Nil),
                }
            }
        }
    }
    assoc_impl(
        i,
        &args[0],
        &args[1],
        |ii, a, b| equal_values(ii, a, b),
        false,
    )
}
fn f_rassq(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    assoc_impl(i, &args[0], &args[1], |_ii, a, b| eq_values(a, b), true)
}
fn f_rassoc(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    assoc_impl(
        i,
        &args[0],
        &args[1],
        |ii, a, b| equal_values(ii, a, b),
        true,
    )
}
fn f_assoc_default(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    // (assoc-default KEY ALIST &optional TEST DEFAULT)
    let test = arg(&args, 2);
    let elem = if test.is_nil() {
        assoc_impl(
            i,
            &args[0],
            &args[1],
            |ii, a, b| equal_values(ii, a, b),
            false,
        )?
    } else {
        let mut cur = args[1].clone();
        let mut found = Value::Nil;
        let mut guard = 0usize;
        loop {
            guard += 1;
            if guard > 500_000 {
                return Err(err_circular(i));
            }
            match &cur {
                Value::Nil => break,
                Value::Cons(c) => {
                    let (item, rest) = {
                        let b = c.borrow();
                        (b.car.clone(), b.cdr.clone())
                    };
                    let key = match &item {
                        Value::Cons(ic) => ic.borrow().car.clone(),
                        v => v.clone(),
                    };
                    let m = i.call_function(
                        &test,
                        &Value::list(vec![quoted(key), quoted(args[0].clone())]),
                        None,
                    )?;
                    if m.truthy() {
                        found = item;
                        break;
                    }
                    cur = rest;
                }
                other => return Err(i.wrong_type_mut("listp", other)),
            }
        }
        found
    };
    match &elem {
        // Matching cons element: return its cdr.
        Value::Cons(c) => Ok(c.borrow().cdr.clone()),
        // No match: nil (DEFAULT only applies to non-cons matches).
        Value::Nil => Ok(Value::Nil),
        // Non-cons element matched: return DEFAULT.
        _ => Ok(arg(&args, 3)),
    }
}

fn del_impl(i: &mut Interp, elt: &Value, list: &Value, use_eq: bool) -> EvalResult {
    // `delete`/`delq` splice matching conses out of the original list
    // structure (GNU semantics); callers holding the same tail observe
    // the removal.
    let mut result = list.clone();
    let mut cur = list.clone();
    let mut prev: Option<crate::lisp::value::ConsRef> = None;
    loop {
        match cur {
            Value::Cons(c) => {
                let (car, next) = {
                    let b = c.borrow();
                    (b.car.clone(), b.cdr.clone())
                };
                let matched = if use_eq {
                    eq_values(&car, elt)
                } else {
                    equal_values(i, &car, elt)
                };
                if matched {
                    match &prev {
                        Some(p) => p.borrow_mut().cdr = next.clone(),
                        None => result = next.clone(),
                    }
                    cur = next;
                } else {
                    prev = Some(c.clone());
                    cur = next;
                }
            }
            Value::Nil => break,
            _ => return Err(i.wrong_type_mut("listp", list)),
        }
    }
    Ok(result)
}

fn f_delq(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    del_impl(i, &args[0], &args[1], true)
}
fn f_delete(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    match &args[1] {
        Value::Str(s) => {
            // delete chars equal to ELT from a string
            let elt = &args[0];
            let out: String = s
                .borrow()
                .chars()
                .filter(|c| {
                    let cv = Value::Int(*c as i128);
                    !equal_values(i, &cv, elt)
                })
                .collect();
            Ok(Value::string(out))
        }
        Value::Vec(v) => {
            let kept: Vec<Value> = v
                .borrow()
                .iter()
                .filter(|x| !equal_values(i, x, &args[0]))
                .cloned()
                .collect();
            Ok(Value::Vec(std::rc::Rc::new(std::cell::RefCell::new(kept))))
        }
        Value::Cons(_) | Value::Nil => del_impl(i, &args[0], &args[1], false),
        other => Err(i.wrong_type_mut("sequencep", other)),
    }
}
fn f_copy_alist(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let items = want_list(i, &args[0])?;
    let copied: Vec<Value> = items
        .into_iter()
        .map(|v| match v {
            Value::Cons(c) => {
                let b = c.borrow();
                Value::cons(b.car.clone(), b.cdr.clone())
            }
            other => other,
        })
        .collect();
    Ok(Value::list(copied))
}
fn f_copy_tree(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let _ = i;
    fn cp(v: &Value) -> Value {
        match v {
            Value::Cons(c) => {
                let b = c.borrow();
                Value::cons(cp(&b.car), cp(&b.cdr))
            }
            Value::Vec(vec) => {
                let b = vec.borrow();
                Value::Vec(std::rc::Rc::new(std::cell::RefCell::new(
                    b.iter().map(cp).collect(),
                )))
            }
            other => other.clone(),
        }
    }
    Ok(cp(&args[0]))
}
fn f_list_tail(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let n = want_int(i, &args[1])?.max(0);
    Ok(nthcdr_of(&args[0], n as usize))
}

// c[ad]+r combinations — implemented via composed car/cdr.
// Like Emacs: each step signals wrong-type-argument on a non-nil atom.
fn cxr(i: &mut Interp, v: &Value, path: &str) -> Result<Value, Flow> {
    let mut cur = v.clone();
    for ch in path.chars().rev() {
        match cur {
            Value::Cons(c) => {
                let b = c.borrow();
                cur = if ch == 'a' {
                    b.car.clone()
                } else {
                    b.cdr.clone()
                };
            }
            Value::Nil => return Ok(Value::Nil),
            ref other => return Err(i.wrong_type_mut("listp", other)),
        }
    }
    Ok(cur)
}

macro_rules! cxr_fn {
    ($fname:ident, $path:literal) => {
        fn $fname(i: &mut Interp, args: Vec<Value>) -> EvalResult {
            cxr(i, &args[0], $path)
        }
    };
}

cxr_fn!(f_caar, "aa");
cxr_fn!(f_cadr, "ad");
cxr_fn!(f_cdar, "da");
cxr_fn!(f_cddr, "dd");
cxr_fn!(f_caaar, "aaa");
cxr_fn!(f_caadr, "aad");
cxr_fn!(f_cadar, "ada");
cxr_fn!(f_caddr, "add");
cxr_fn!(f_cdaar, "daa");
cxr_fn!(f_cdadr, "dad");
cxr_fn!(f_cddar, "dda");
cxr_fn!(f_cdddr, "ddd");
cxr_fn!(f_caaaar, "aaaa");
cxr_fn!(f_caaadr, "aaad");
cxr_fn!(f_caadar, "aada");
cxr_fn!(f_caaddr, "aadd");
cxr_fn!(f_cadaar, "adaa");
cxr_fn!(f_cadadr, "adad");
cxr_fn!(f_caddar, "adda");
cxr_fn!(f_cadddr, "addd");
cxr_fn!(f_cdaaar, "daaa");
cxr_fn!(f_cdaadr, "daad");
cxr_fn!(f_cdadar, "dada");
cxr_fn!(f_cdaddr, "dadd");
cxr_fn!(f_cddaar, "ddaa");
cxr_fn!(f_cddadr, "ddad");
cxr_fn!(f_cdddar, "ddda");
cxr_fn!(f_cddddr, "dddd");

// ---------- plists ----------

/// `plist-get` core with a caller-supplied predicate.
fn plist_scan(
    i: &mut Interp,
    plist: &Value,
    prop: &Value,
    lax: bool,
    want_pair: bool,
) -> Result<Option<Value>, Flow> {
    let mut cur = plist.clone();
    let mut guard = 0usize;
    loop {
        guard += 1;
        if guard > 500_000 {
            return Err(err_circular(i));
        }
        match cur {
            Value::Nil => return Ok(None),
            Value::Cons(c) => {
                let (k, rest) = {
                    let b = c.borrow();
                    (b.car.clone(), b.cdr.clone())
                };
                let v = match &rest {
                    Value::Cons(c2) => {
                        let b = c2.borrow();
                        (b.car.clone(), b.cdr.clone())
                    }
                    _ => {
                        // Odd-length plist: treat last key's value as nil.
                        (Value::Nil, Value::Nil)
                    }
                };
                let matches = if lax {
                    equal_values(i, &k, prop)
                } else {
                    eq_values(&k, prop)
                };
                if matches {
                    return Ok(Some(if want_pair {
                        Value::cons(k, Value::cons(v.0, Value::Nil))
                    } else {
                        v.0
                    }));
                }
                cur = v.1;
            }
            // GNU's plist_get iterates while CONSP: a non-cons tail
            // (or non-list PLIST) ends the scan with no match.
            _ => return Ok(None),
        }
    }
}

pub(crate) fn err_circular(i: &mut Interp) -> Flow {
    i.signal_data(sym::CIRCULAR_LIST, vec![Value::Nil])
}

fn f_plist_get(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    // Emacs: (plist-get PLIST PROP &optional PREDICATE) — PREDICATE is a
    // comparison function, default `eq'.
    let pred = arg(&args, 2);
    if !pred.is_nil() {
        let mut cur = args[0].clone();
        let mut guard = 0usize;
        loop {
            guard += 1;
            if guard > 500_000 {
                return Err(err_circular(i));
            }
            match cur {
                Value::Nil => return Ok(Value::Nil),
                Value::Cons(c) => {
                    let (k, rest) = {
                        let b = c.borrow();
                        (b.car.clone(), b.cdr.clone())
                    };
                    let m = i.call_function(
                        &pred,
                        &Value::list(vec![quoted(k), quoted(args[1].clone())]),
                        None,
                    )?;
                    if m.truthy() {
                        return Ok(match &rest {
                            Value::Cons(c2) => c2.borrow().car.clone(),
                            _ => Value::Nil,
                        });
                    }
                    cur = match &rest {
                        Value::Cons(c2) => c2.borrow().cdr.clone(),
                        _ => Value::Nil,
                    };
                }
                // GNU: non-list PLIST ends the scan (returns nil).
                _ => return Ok(Value::Nil),
            }
        }
    }
    match plist_scan(i, &args[0], &args[1], false, false)? {
        Some(v) => Ok(v),
        None => Ok(Value::Nil),
    }
}

fn f_lax_plist_get(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    match plist_scan(i, &args[0], &args[1], true, false)? {
        Some(v) => Ok(v),
        None => Ok(Value::Nil),
    }
}

fn f_plist_member(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let pred = arg(&args, 2);
    let lax = pred.truthy();
    // Return the tail starting at the matching key, like Emacs.
    let mut cur = args[0].clone();
    let mut guard = 0usize;
    loop {
        guard += 1;
        if guard > 500_000 {
            return Err(err_circular(i));
        }
        match cur {
            Value::Nil => return Ok(Value::Nil),
            Value::Cons(c) => {
                let (k, rest, this) = {
                    let b = c.borrow();
                    (b.car.clone(), b.cdr.clone(), Value::Cons(c.clone()))
                };
                let matches = if lax {
                    equal_values(i, &k, &args[1])
                } else {
                    eq_values(&k, &args[1])
                };
                if matches {
                    return Ok(this);
                }
                // Skip key + value.
                cur = match &rest {
                    Value::Cons(c2) => c2.borrow().cdr.clone(),
                    _ => Value::Nil,
                };
            }
            // GNU signals `plistp' (not `listp') on a non-list tail.
            other => return Err(i.wrong_type_mut("plistp", &other)),
        }
    }
}

fn f_plist_put(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    plist_put(i, args, false)
}

fn f_lax_plist_put(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    plist_put(i, args, true)
}

fn plist_put(i: &mut Interp, args: Vec<Value>, lax: bool) -> EvalResult {
    // Emacs plist.c semantics: find the value cell for prop and set it
    // in place; else append (prop val) onto the last complete pair so
    // the original list object is mutated. Empty list → new list.
    if args[0].is_nil() {
        return Ok(Value::cons(
            args[1].clone(),
            Value::cons(args[2].clone(), Value::Nil),
        ));
    }
    let mut cur = args[0].clone();
    let mut last_pair_cell: Option<Value> = None;
    let mut guard = 0usize;
    loop {
        guard += 1;
        if guard > 500_000 {
            return Err(err_circular(i));
        }
        match &cur {
            Value::Nil => {
                // Append (prop val) after the last pair's value cell.
                let new_pair =
                    Value::cons(args[1].clone(), Value::cons(args[2].clone(), Value::Nil));
                if let Some(Value::Cons(pc)) = &last_pair_cell {
                    let value_cell = pc.borrow().cdr.clone();
                    if let Value::Cons(vc) = &value_cell {
                        vc.borrow_mut().cdr = new_pair;
                        return Ok(args[0].clone());
                    }
                }
                // Degenerate plist (no complete pair): return new plist.
                return Ok(Value::cons(
                    args[1].clone(),
                    Value::cons(args[2].clone(), args[0].clone()),
                ));
            }
            Value::Cons(c) => {
                let (k, rest) = {
                    let b = c.borrow();
                    (b.car.clone(), b.cdr.clone())
                };
                let matches = if lax {
                    equal_values(i, &k, &args[1])
                } else {
                    eq_values(&k, &args[1])
                };
                if matches {
                    if let Value::Cons(cell) = &rest {
                        cell.borrow_mut().car = args[2].clone();
                        return Ok(args[0].clone());
                    }
                    return Err(i.wrong_type_mut("listp", &rest));
                }
                if matches!(rest, Value::Cons(_)) {
                    last_pair_cell = Some(cur.clone());
                }
                let next = match &rest {
                    Value::Cons(c2) => c2.borrow().cdr.clone(),
                    _ => Value::Nil,
                };
                cur = next;
            }
            other => return Err(i.wrong_type_mut("listp", other)),
        }
    }
}

// ---------- take / ntake / flatten ----------

fn f_take(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let n = want_int(i, &args[0])?.max(0) as usize;
    match &args[1] {
        Value::Str(s) => {
            let out: String = s.borrow().chars().take(n).collect();
            Ok(Value::string(out))
        }
        Value::Vec(v) => {
            let out: Vec<Value> = v.borrow().iter().take(n).cloned().collect();
            Ok(Value::Vec(std::rc::Rc::new(std::cell::RefCell::new(out))))
        }
        _ => {
            let items = want_list(i, &args[1])?;
            Ok(Value::list(items.into_iter().take(n).collect()))
        }
    }
}

fn f_ntake(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let n = want_int(i, &args[0])?.max(0) as usize;
    // ntake destructively truncates the list after N elements.
    let mut cur = args[1].clone();
    if n == 0 {
        return Ok(Value::Nil);
    }
    let mut k = 1;
    loop {
        match &cur {
            Value::Cons(c) => {
                let next = c.borrow().cdr.clone();
                if k == n {
                    c.borrow_mut().cdr = Value::Nil;
                    return Ok(args[1].clone());
                }
                cur = next;
                k += 1;
            }
            _ => return Ok(args[1].clone()),
        }
    }
}

fn f_flatten_tree(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let _ = i;
    let mut out = Vec::new();
    let mut stack = vec![args[0].clone()];
    let mut guard = 0usize;
    while let Some(v) = stack.pop() {
        guard += 1;
        if guard > 2_000_000 {
            break;
        }
        match v {
            Value::Cons(c) => {
                let b = c.borrow();
                // Push cdr first so car pops first (depth-first order).
                stack.push(b.cdr.clone());
                stack.push(b.car.clone());
            }
            Value::Nil => {}
            other => out.push(other),
        }
    }
    Ok(Value::list(out))
}

fn f_apply_partially(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    // (apply-partially f a b) => closure (&rest rest) => (apply f a b rest).
    // Represent as a small Lambda whose body is a special marker form?
    // Simpler: build (lambda (&rest r) (apply 'f a b r)) — needs eval
    // env; instead build a Lambda directly calling apply via a thunk.
    // Easiest correct approach: a lambda whose body form is
    // (apply f 'a 'b ... r) evaluated at call time.
    let fun = args[0].clone();
    let fixed = args[1..].to_vec();
    // Build the body form: (apply FUN 'F1 'F2 ... r)
    let r_sym = i.intern("--apply-partially-args");
    let mut apply_args = vec![Value::list(vec![Value::Sym(sym::QUOTE), fun])];
    for v in &fixed {
        apply_args.push(Value::list(vec![Value::Sym(sym::QUOTE), v.clone()]));
    }
    apply_args.push(Value::Sym(r_sym));
    let body_form = Value::cons(Value::Sym(i.intern("apply")), Value::list(apply_args));
    let lam = crate::lisp::value::Lambda {
        is_macro: false,
        required: Vec::new(),
        optional: Vec::new(),
        rest: Some(r_sym),
        body: vec![body_form],
        env: None,
        doc: Some("Function created by apply-partially.".into()),
        interactive: None,
        name: None,
        bad_arglist: false,
        arglist: None,
        plain: false,
        dumped_doc: false,
    };
    Ok(Value::Lambda(std::rc::Rc::new(lam)))
}

fn f_assoc_string(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    // (assoc-string KEY LIST CASE-FOLD) — KEY must be a string (or symbol
    // name? Emacs requires string). Compare with equal or case-folded.
    let key = match &args[0] {
        Value::Str(s) => s.borrow().clone(),
        _ => return Err(i.wrong_type_mut("stringp", &args[0])),
    };
    let fold = arg(&args, 2).truthy();
    let mut cur = args[1].clone();
    let mut guard = 0usize;
    loop {
        guard += 1;
        if guard > 500_000 {
            return Err(err_circular(i));
        }
        match cur {
            Value::Nil => return Ok(Value::Nil),
            Value::Cons(c) => {
                let (elem, next) = {
                    let b = c.borrow();
                    (b.car.clone(), b.cdr.clone())
                };
                if let Value::Cons(pair) = &elem {
                    let k = pair.borrow().car.clone();
                    if let Value::Str(ks) = &k {
                        let ks = ks.borrow().clone();
                        let eq = if fold {
                            ks.eq_ignore_ascii_case(&key)
                        } else {
                            ks == key
                        };
                        if eq {
                            return Ok(elem);
                        }
                    }
                }
                cur = next;
            }
            other => return Err(i.wrong_type_mut("listp", &other)),
        }
    }
}

fn f_equal_incl_props(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    // Text properties on strings aren't modeled yet, so this is `equal`.
    Ok(Value::from_bool(equal_values(i, &args[0], &args[1])))
}

// ---------- remove/delete (sequence-generic) ----------

/// `remove`/`delete`/`remq`/`delq` shared core: filter SEQUENCE by
/// `cmp`, preserving the sequence type. `delete`/`delq` mutate list
/// structure; other sequences are copied (as in Emacs).
fn remove_impl(
    i: &mut Interp,
    elt: &Value,
    seq: &Value,
    cmp: fn(&Interp, &Value, &Value) -> bool,
) -> EvalResult {
    match seq {
        Value::Str(s) => {
            let kept: String = s
                .borrow()
                .chars()
                .filter(|c| !cmp(i, elt, &Value::Int(*c as i128)))
                .collect();
            Ok(Value::string(kept))
        }
        Value::Vec(v) => {
            let kept: Vec<Value> = v
                .borrow()
                .iter()
                .filter(|x| !cmp(i, elt, x))
                .cloned()
                .collect();
            Ok(Value::Vec(std::rc::Rc::new(std::cell::RefCell::new(kept))))
        }
        Value::Cons(_) | Value::Nil => {
            // List (or nil): GNU `remove`/`remq` copy the spine — the
            // original list is left untouched.
            let mut kept: Vec<Value> = Vec::new();
            let mut cur = seq.clone();
            loop {
                match cur {
                    Value::Cons(c) => {
                        let (car, next) = {
                            let b = c.borrow();
                            (b.car.clone(), b.cdr.clone())
                        };
                        if !cmp(i, elt, &car) {
                            kept.push(car);
                        }
                        cur = next;
                    }
                    Value::Nil => break,
                    other => return Err(i.wrong_type_mut("listp", &other)),
                }
            }
            Ok(Value::list(kept))
        }
        other => Err(i.wrong_type_mut("sequencep", other)),
    }
}

fn f_remove(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    remove_impl(i, &args[0], &args[1], |ii, a, b| equal_values(ii, a, b))
}
fn f_remq(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    // GNU `remq` is list-only (unlike sequence-generic `remove`).
    match args[1] {
        Value::Cons(_) | Value::Nil => {
            remove_impl(i, &args[0], &args[1], |_ii, a, b| eq_values(a, b))
        }
        ref other => Err(i.wrong_type_mut("listp", other)),
    }
}
