//! List-manipulation subrs: car, cdr, cons, nth, append, member, etc.

use super::{S, arg, eq_values, equal_values, want_cons, want_int, want_list};
use crate::lisp::Interp;
use crate::lisp::error::{EvalResult, Flow};
use crate::lisp::obarray::sym;
use crate::lisp::value::{Subr, Value};
use std::cell::RefCell;
use std::rc::Rc;

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
        4,
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
        "Return t if two Lisp objects have similar structure and contents.\nThis is like `equal' except that it also requires text properties of\nstrings to be identical."
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

fn f_car(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    match &args[0] {
        Value::Cons(c) => Ok(c.borrow().car.clone()),
        Value::Nil => Ok(Value::Nil),
        other => Err(i.wrong_type_mut("listp", other)),
    }
}
fn f_cdr(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    match &args[0] {
        Value::Cons(c) => Ok(c.borrow().cdr.clone()),
        Value::Nil => Ok(Value::Nil),
        other => Err(i.wrong_type_mut("listp", other)),
    }
}
fn f_car_or_marker_p(_i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    Ok(Value::from_bool(matches!(
        &args[0],
        Value::Cons(_) | Value::Marker(_)
    )))
}
fn f_car_safe(_i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    Ok(match &args[0] {
        Value::Cons(c) => c.borrow().car.clone(),
        _ => Value::Nil,
    })
}
fn f_cdr_safe(_i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    Ok(match &args[0] {
        Value::Cons(c) => c.borrow().cdr.clone(),
        _ => Value::Nil,
    })
}
fn f_cons(_i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    Ok(Value::cons(args[0].clone(), args[1].clone()))
}
fn f_list(_i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    Ok(Value::list(args.as_slice().to_vec()))
}
fn f_make_list(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    // GNU CHECK_FIXNAT: non-integer
    // AND negative both report `wholenump'.
    let n = match &args[0] {
        Value::Int(x) if *x >= 0 => *x,
        _ => return Err(i.wrong_type_mut("wholenump", &args[0])),
    };
    if n > 0x2000_0000 {
        return Err(i.error("Memory exhausted--use C-x s then exit and restart Emacs"));
    }
    Ok(Value::list(vec![args[1].clone(); n as usize]))
}
fn f_length(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
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
        // Char-tables cover the whole internal char space
        // (MAX_CHAR + 1 = 4194304); other records report slot count.
        Value::Record(_) if crate::lisp::builtins::misc::is_char_table(i, &args[0]) => Ok(
            Value::Int(crate::lisp::builtins::misc::CT_MAX_CHAR as i128 + 1),
        ),
        // Records (incl. EIEIO instances) report their slot count.
        Value::Record(r) => Ok(Value::Int(r.borrow().len() as i128)),
        // Function objects (closures and `#[...]' byte-code literals)
        // answer `length' with their element count like GNU —
        // `#[...]' uses the literal's count, others the synthesized
        // [args body env] prefix.
        Value::Lambda(l) => Ok(Value::Int(
            crate::lisp::builtins::misc::lambda_slot_values(l).len() as i128,
        )),
        other => Err(i.wrong_type_mut("sequencep", other)),
    }
}
fn f_safe_length(_i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
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
fn f_proper_list_p(_i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
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

/// cdr N times the way GNU does: Cons → cdr, nil → stays nil,
/// any other atom → wrong-type-argument.  GNU's CHECK_LIST_CONS
/// reports the whole list argument, not the offending tail.
pub(crate) fn nthcdr_strict(i: &mut Interp, v: &Value, n: usize) -> Result<Value, Flow> {
    let mut cur = v.clone();
    for _ in 0..n {
        match cur {
            Value::Cons(c) => {
                let next = c.borrow().cdr.clone();
                cur = next;
            }
            Value::Nil => {}
            _ => return Err(i.wrong_type_mut("listp", v)),
        }
    }
    Ok(cur)
}

fn f_nth(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    // Emacs: a negative index behaves like 0; nth is car(nthcdr N).
    let n = want_int(i, &args[0])?.max(0);
    let tail = nthcdr_strict(i, &args[1], n as usize)?;
    match tail {
        Value::Cons(c) => Ok(c.borrow().car.clone()),
        Value::Nil => Ok(Value::Nil),
        ref other => Err(i.wrong_type_mut("listp", other)),
    }
}
fn f_nthcdr(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    let n = want_int(i, &args[0])?.max(0);
    nthcdr_strict(i, &args[1], n as usize)
}
/// GNU subr.el `safe-length' walk: count cons cells, stop at a
/// non-cons tail or on a cycle (tortoise/hare); returns the count.
fn safe_len(v: &Value) -> i128 {
    let mut n = 0i128;
    let mut cur = v.clone();
    let mut slow = v.clone();
    loop {
        match &cur {
            Value::Cons(c) => {
                if n > 0 {
                    if let (Value::Cons(a), Value::Cons(b)) = (&cur, &slow) {
                        if Rc::ptr_eq(a, b) {
                            break;
                        }
                    }
                }
                let next = c.borrow().cdr.clone();
                n += 1;
                cur = next;
                if n % 2 == 0 {
                    if let Value::Cons(h) = &slow {
                        let nxt = h.borrow().cdr.clone();
                        slow = nxt;
                    }
                }
            }
            _ => break,
        }
    }
    n
}

fn f_last(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    // GNU subr.el `last':
    // (if n (and (>= n 0) (let ((m (safe-length list)))
    //             (if (< n m) (nthcdr (- m n) list) list)))
    //   (and list (nthcdr (1- (safe-length list)) list)))
    // `(>= n 0)' on a non-number signals `number-or-marker-p' BEFORE the
    // safe-length walk; a non-list arg returns itself (0 cells) when n
    // is absent or positive, nil when n <= 0.
    match args.get(1) {
        None | Some(Value::Nil) => {
            if args[0].is_nil() {
                return Ok(Value::Nil);
            }
            let m = safe_len(&args[0]);
            Ok(nthcdr_of(&args[0], (m - 1).max(0) as usize))
        }
        Some(v) => {
            let n = crate::editor::want_num_or_mark(i, v)?;
            if n < 0 {
                return Ok(Value::Nil);
            }
            let m = safe_len(&args[0]);
            if (n as i128) < m {
                Ok(nthcdr_of(&args[0], (m - n as i128) as usize))
            } else {
                Ok(args[0].clone())
            }
        }
    }
}
fn f_butlast(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    // GNU subr.el
    // `butlast': (unless n (setq n 1)) → (if (<= n 0) list
    //                                     (take (- (length list) n) list)).
    // `length' on a non-sequence signals `sequencep'; `take' walks
    // `(nthcdr k list)' and `car's each cell, so an atom tail signals
    // `listp' on that tail (strings/vectors take the same path).
    let n = match args.get(1) {
        None | Some(Value::Nil) => 1,
        Some(v) => crate::editor::want_num_or_mark(i, v)?,
    };
    if n <= 0 {
        return Ok(args[0].clone());
    }
    let len = match &args[0] {
        Value::Nil => 0usize,
        Value::Cons(_) => args[0]
            .list_to_vec()
            .map_err(|e| match e {
                // GNU `length' signals `listp' on the improper tail.
                crate::lisp::value::ListError::Dotted(tail) => i.wrong_type_mut("listp", &tail),
                _ => i.wrong_type_mut("listp", &args[0]),
            })?
            .len(),
        Value::Str(s) => s.borrow().chars().count(),
        Value::Vec(v) => v.borrow().len(),
        other => return Err(i.wrong_type_mut("sequencep", other)),
    };
    let keep = len.saturating_sub(n as usize);
    if keep == 0 {
        return Ok(Value::Nil);
    }
    // take: (car (nthcdr i list)) for i in 0..keep — atom elements or a
    // dotted tail signal `listp' like GNU's Fcar path.
    let mut out = Vec::with_capacity(keep);
    let mut cur = args[0].clone();
    for _ in 0..keep {
        match cur {
            Value::Cons(c) => {
                let (elt, next) = {
                    let b = c.borrow();
                    (b.car.clone(), b.cdr.clone())
                };
                out.push(elt);
                cur = next;
            }
            other => return Err(i.wrong_type_mut("listp", &other)),
        }
    }
    Ok(Value::list(out))
}
fn f_nbutlast(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    // GNU subr.el
    // `nbutlast': (let ((m (length list))) (or n (setq n 1))
    //   (and (< n m) (progn (if (plusp n)
    //       (setcdr (nthcdr (- (1- m) n) list) nil)) list)))
    // `length' first — a non-sequence signals `sequencep' even when n
    // would itself be invalid.  Non-nil n then needs number-or-marker-p.
    let m = match &args[0] {
        Value::Nil => 0i128,
        Value::Cons(_) => args[0]
            .list_to_vec()
            .map_err(|e| match e {
                crate::lisp::value::ListError::Dotted(tail) => i.wrong_type_mut("listp", &tail),
                _ => i.wrong_type_mut("listp", &args[0]),
            })?
            .len() as i128,
        Value::Str(s) => s.borrow().chars().count() as i128,
        Value::Vec(v) => v.borrow().len() as i128,
        other => return Err(i.wrong_type_mut("sequencep", other)),
    };
    let n = match args.get(1) {
        None | Some(Value::Nil) => 1i128,
        Some(v) => crate::editor::want_num_or_mark(i, v)? as i128,
    };
    if !(n < m) {
        return Ok(Value::Nil);
    }
    if n > 0 {
        // GNU evaluates `(nthcdr (- (1- m) n) list)' then `setcdr's it:
        // a non-list sequence reaches Fsetcdr and signals `consp'.
        match &args[0] {
            Value::Cons(_) | Value::Nil => {}
            other => return Err(i.wrong_type_mut("consp", other)),
        }
        let tail = nthcdr_of(&args[0], (m - 1 - n) as usize);
        if let Value::Cons(c) = tail {
            c.borrow_mut().cdr = Value::Nil;
        }
    }
    Ok(args[0].clone())
}
fn f_append(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
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
fn f_nconc(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    let mut result = Value::Nil;
    let mut last_cons: Option<crate::lisp::value::ConsRef> = None;
    for a in &args[..] {
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
            other if std::ptr::eq(other, args.last().unwrap()) => {
                // Last arg is the dotted tail (like append).
                match &last_cons {
                    Some(c) => c.borrow_mut().cdr = other.clone(),
                    None => result = other.clone(),
                }
            }
            // GNU `nconc' CHECK_LIST_CONS's every non-final arg:
            // non-nil non-cons signals `consp' on that arg.
            other => return Err(i.wrong_type_mut("consp", other)),
        }
    }
    Ok(result)
}
fn f_reverse(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
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
        Value::Record(_) if crate::lisp::builtins::misc::is_bool_vector(i, &args[0]) => {
            let mut bits = crate::lisp::builtins::misc::bool_vec_of(i, &args[0])?;
            bits.reverse();
            return Ok(Value::list(
                bits.into_iter().map(Value::from_bool).collect(),
            ));
        }
        Value::Nil => return Ok(Value::Nil),
        Value::Cons(_) => {
            let items = want_list(i, &args[0])?;
            return Ok(Value::list(items.into_iter().rev().collect()));
        }
        // GNU `reverse' signals `sequencep' on non-sequence input.
        other => return Err(i.wrong_type_mut("sequencep", other)),
    }
}
fn f_setcar(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    let c = want_cons(i, &args[0])?;
    c.borrow_mut().car = args[1].clone();
    Ok(args[1].clone())
}
fn f_setcdr(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
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
    // Raw-pointer spine walk (see `cdr_ptr'): `cmp' is a pure
    // predicate — `eq'/`eql'/`equal' never run Lisp — so the walk can
    // read cdrs without a borrow/clone per cell.  `cur' still yields
    // the tail `Value' to return by cloning only on a hit.
    let mut cur = list as *const Value;
    let mut tortoise = list as *const Value;
    let mut guard = 0usize;
    loop {
        match unsafe { &*cur } {
            Value::Cons(c) => {
                guard += 1;
                if guard > 500_000 {
                    return Err(err_circular(i));
                }
                let cell = RefCell::as_ptr(c);
                if cmp(i, elt, unsafe { &(*cell).car }) {
                    return Ok(unsafe { &*cur }.clone());
                }
                cur = unsafe { &(*cell).cdr };
                if guard % 2 == 0 {
                    if let Value::Cons(tc) = unsafe { &*tortoise } {
                        tortoise = unsafe { crate::lisp::eval::cdr_ptr(tc) };
                    }
                }
                if guard > 1 && eq_values(unsafe { &*cur }, unsafe { &*tortoise }) {
                    return Err(err_circular(i));
                }
            }
            Value::Nil => return Ok(Value::Nil),
            // GNU reports the whole list argument, not the atom tail.
            _ => return Err(i.wrong_type_mut("listp", list)),
        }
    }
}

fn f_member(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    member_impl(i, &args[0], &args[1], |ii, a, b| equal_values(ii, a, b))
}
fn f_memq(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    member_impl(i, &args[0], &args[1], |_ii, a, b| eq_values(a, b))
}
fn f_memql(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
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

fn f_member_if(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    member_if_impl(i, &args[0], &args[1], true)
}

fn assoc_impl(
    i: &mut Interp,
    key: &Value,
    list: &Value,
    cmp: fn(&Interp, &Value, &Value) -> bool,
    cdr_cmp: bool,
) -> EvalResult {
    // Raw-pointer spine walk like `member_impl'.
    let mut cur = list as *const Value;
    let mut tortoise = list as *const Value;
    let mut guard = 0usize;
    loop {
        match unsafe { &*cur } {
            Value::Cons(c) => {
                guard += 1;
                if guard > 500_000 {
                    return Err(err_circular(i));
                }
                let cell = RefCell::as_ptr(c);
                if let Value::Cons(ec) = unsafe { &(*cell).car } {
                    let eb = ec.borrow();
                    let probe = if cdr_cmp { &eb.cdr } else { &eb.car };
                    if cmp(i, key, probe) {
                        drop(eb);
                        return Ok(unsafe { &*cell }.car.clone());
                    }
                }
                cur = unsafe { &(*cell).cdr };
                if guard % 2 == 0 {
                    if let Value::Cons(tc) = unsafe { &*tortoise } {
                        tortoise = unsafe { crate::lisp::eval::cdr_ptr(tc) };
                    }
                }
                if guard > 1 && eq_values(unsafe { &*cur }, unsafe { &*tortoise }) {
                    return Err(err_circular(i));
                }
            }
            Value::Nil => return Ok(Value::Nil),
            // GNU assoc/assq report the whole LIST argument, not the
            // non-list tail (assq_no_quit's `xsignal listp LIST').
            _ => return Err(i.wrong_type_mut("listp", list)),
        }
    }
}

fn f_assq(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    assoc_impl(i, &args[0], &args[1], |_ii, a, b| eq_values(a, b), false)
}

fn delete_all_by(i: &mut Interp, args: crate::lisp::Argv, on_cdr: bool) -> EvalResult {
    // GNU's Lisp `assq-delete-all'/'rassq-delete-all' (subr.el):
    // skip matching head cells, then splice matches out of the list
    // structure in place — non-head deletions mutate the original
    // list and return it.
    let matches = |item: &Value| -> bool {
        match item {
            Value::Cons(ic) => {
                let part = {
                    let b = ic.borrow();
                    if on_cdr { b.cdr.clone() } else { b.car.clone() }
                };
                eq_values(&part, &args[0])
            }
            _ => false,
        }
    };
    let mut alist = args[1].clone();
    let mut guard = 0usize;
    // (while (and (consp (car alist)) (eq (car (car alist)) key))
    //   (setq alist (cdr alist)))
    loop {
        guard += 1;
        if guard > 500_000 {
            return Err(err_circular(i));
        }
        let head = match &alist {
            Value::Cons(c) => c.borrow().car.clone(),
            Value::Nil => break,
            // GNU: `(car ALIST)' at the head-loop test — a non-nil atom
            // tail (dotted list end or non-list arg) signals listp on it.
            other => return Err(i.wrong_type_mut("listp", other)),
        };
        if matches(&head) {
            let next = match &alist {
                Value::Cons(c) => c.borrow().cdr.clone(),
                _ => break,
            };
            alist = next;
        } else {
            break;
        }
    }
    // (let ((tail alist) tail-cdr)
    //   (while (setq tail-cdr (cdr tail))
    //     (if (and (consp (car tail-cdr)) (eq ...)) (setcdr tail (cdr tail-cdr))
    //       (setq tail tail-cdr))))
    let mut tail = alist.clone();
    loop {
        guard += 1;
        if guard > 500_000 {
            return Err(err_circular(i));
        }
        let next = match &tail {
            Value::Cons(c) => c.borrow().cdr.clone(),
            _ => break,
        };
        match &next {
            Value::Nil => break,
            Value::Cons(nc) => {
                if matches(&nc.borrow().car.clone()) {
                    let nn = nc.borrow().cdr.clone();
                    if let Value::Cons(tc) = &tail {
                        tc.borrow_mut().cdr = nn;
                    }
                } else {
                    tail = next;
                }
            }
            // GNU: (car ATOM) on the dotted tail → listp error.
            other => return Err(i.wrong_type_mut("listp", other)),
        }
    }
    Ok(alist)
}

fn f_assq_delete_all(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    delete_all_by(i, args, false)
}

fn f_rassq_delete_all(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    delete_all_by(i, args, true)
}
fn f_assoc(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
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
                    Value::Nil => return Ok(Value::Nil),
                    other => return Err(i.wrong_type_mut("listp", other)),
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
fn f_rassq(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    assoc_impl(i, &args[0], &args[1], |_ii, a, b| eq_values(a, b), true)
}
fn f_rassoc(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    assoc_impl(
        i,
        &args[0],
        &args[1],
        |ii, a, b| equal_values(ii, a, b),
        true,
    )
}
fn f_assoc_default(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    // (assoc-default KEY ALIST &optional TEST DEFAULT) — GNU's Lisp
    // version: the tested key is (car ELEMENT) for a cons element but
    // the ELEMENT itself for an atom, so `(assoc-default 'c '(... c)
    // nil DEFAULT)' returns DEFAULT.
    let test = arg(&args, 2);
    let mut cur = args[1].clone();
    let mut found = Value::Nil;
    let mut matched = false;
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
                let hit = if test.is_nil() {
                    equal_values(i, &key, &args[0])
                } else {
                    i.call_function(
                        &test,
                        &Value::list(vec![quoted(key), quoted(args[0].clone())]),
                        None,
                    )?
                    .truthy()
                };
                if hit {
                    found = item;
                    matched = true;
                    break;
                }
                cur = rest;
            }
            other => return Err(i.wrong_type_mut("listp", other)),
        }
    }
    if !matched {
        return Ok(Value::Nil);
    }
    match &found {
        // Matching cons element: return its cdr.
        Value::Cons(c) => Ok(c.borrow().cdr.clone()),
        // Atom element matched: return DEFAULT.
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

fn f_delq(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    del_impl(i, &args[0], &args[1], true)
}
fn f_delete(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
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
fn f_copy_alist(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
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
fn f_copy_tree(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    // (copy-tree TREE &optional VECP): only conses are copied; vectors
    // (and records) are copied recursively too when VECP is non-nil
    // (GNU fns.c Fcopy_tree).
    let vecp = arg(&args, 1).truthy();
    fn cp(v: &Value, vecp: bool) -> Value {
        match v {
            Value::Cons(c) => {
                let b = c.borrow();
                Value::cons(cp(&b.car, vecp), cp(&b.cdr, vecp))
            }
            Value::Vec(vec) if vecp => {
                let b = vec.borrow();
                Value::Vec(std::rc::Rc::new(std::cell::RefCell::new(
                    b.iter().map(|x| cp(x, vecp)).collect(),
                )))
            }
            other => other.clone(),
        }
    }
    let _ = i;
    Ok(cp(&args[0], vecp))
}
fn f_list_tail(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    let n = want_int(i, &args[1])?.max(0);
    nthcdr_strict(i, &args[0], n as usize)
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
        fn $fname(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
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

fn f_plist_get(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
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
                    // GNU funcalls PREDICATE on the raw values —
                    // `invalid-function' on e.g. 5, not a quoted form.
                    let m = i.apply(&pred, vec![k.clone(), args[1].clone()])?;
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

fn f_lax_plist_get(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    match plist_scan(i, &args[0], &args[1], true, false)? {
        Some(v) => Ok(v),
        None => Ok(Value::Nil),
    }
}

fn f_plist_member(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    // GNU plist.c Fplist_member: FOR_EACH_TAIL walks key cells;
    // a non-cons tail (atom arg, dotted pair, `(a 1 . b)') exits
    // the loop and `CHECK_TYPE (NILP (tail))' signals `plistp' on
    // the whole argument.  A proper odd-length list ends cleanly
    // (nil tail) → nil.  PREDICATE (default `eq') compares the
    // stored key against PROP via funcall.
    let pred = arg(&args, 2);
    let whole = args[0].clone();
    let mut cur = args[0].clone();
    let mut guard = 0usize;
    loop {
        guard += 1;
        if guard > 500_000 {
            return Err(err_circular(i));
        }
        let (k, rest) = match cur.clone() {
            Value::Cons(c) => {
                let b = c.borrow();
                (b.car.clone(), b.cdr.clone())
            }
            // FOR_EACH_TAIL's `CONSP(tail)' stops on nil; any other
            // atom/dotted tail → `CHECK_TYPE' signals `plistp'.
            Value::Nil => return Ok(Value::Nil),
            _ => return Err(i.wrong_type_mut("plistp", &whole)),
        };
        let matches = if pred.is_nil() {
            eq_values(&k, &args[1])
        } else {
            i.apply(&pred, vec![k.clone(), args[1].clone()])?.truthy()
        };
        if matches {
            return Ok(cur);
        }
        // Advance past the value cell when it exists; GNU stops the
        // scan when the cdr is not a cons.
        cur = if let Value::Cons(c2) = &rest {
            c2.borrow().cdr.clone()
        } else {
            rest
        };
    }
}

fn f_plist_put(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    plist_put(i, args, false)
}

fn f_lax_plist_put(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    plist_put(i, args, true)
}

fn plist_put(i: &mut Interp, args: crate::lisp::Argv, lax: bool) -> EvalResult {
    // Emacs plist.c semantics: find the value cell for prop and set it
    // in place; else append (prop val) onto the last complete pair so
    // the original list object is mutated. Empty list → new list.
    // Malformed plists signal `plistp' on the WHOLE argument; a 4th
    // arg is a comparison predicate (subr-x `plist-put').
    let pred: Option<Value> = if lax {
        Some(Value::Sym(i.intern("equal")))
    } else {
        args.get(3).cloned().filter(|p| p.truthy())
    };
    // Non-list atom arg → `plistp' on it.
    if !matches!(args[0], Value::Nil | Value::Cons(_)) {
        return Err(i.wrong_type_mut("plistp", &args[0]));
    }
    if args[0].is_nil() {
        return Ok(Value::cons(
            args[1].clone(),
            Value::cons(args[2].clone(), Value::Nil),
        ));
    }
    let whole = args[0].clone();
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
                let matches = match &pred {
                    Some(p) => i
                        .apply(p, vec![k.clone(), args[1].clone()])
                        .map(|v| v.truthy())?,
                    None => eq_values(&k, &args[1]),
                };
                if matches {
                    if let Value::Cons(cell) = &rest {
                        cell.borrow_mut().car = args[2].clone();
                        return Ok(args[0].clone());
                    }
                    return Err(i.wrong_type_mut("plistp", &whole));
                }
                // Skip to the next pair; a non-cons tail is malformed.
                match &rest {
                    Value::Cons(c2) => {
                        last_pair_cell = Some(cur.clone());
                        cur = c2.borrow().cdr.clone();
                    }
                    _ => return Err(i.wrong_type_mut("plistp", &whole)),
                }
            }
            _ => return Err(i.wrong_type_mut("plistp", &whole)),
        }
    }
}

// ---------- take / ntake / flatten ----------

fn f_take(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    let n = want_int(i, &args[0])?.max(0) as usize;
    // GNU Ftake is list-only (subr-x `take'): strings/vectors signal
    // wrong-type-argument listp.
    let items = want_list(i, &args[1])?;
    Ok(Value::list(items.into_iter().take(n).collect()))
}

fn f_ntake(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
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
            Value::Nil => return Ok(args[1].clone()),
            // GNU CHECK_LIST reports the original LIST argument.
            _ => return Err(i.wrong_type_mut("listp", &args[1])),
        }
    }
}

fn f_flatten_tree(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
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

fn f_apply_partially(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    // (apply-partially f a b) => closure (&rest rest) => (apply f a b rest).
    // Represent as a small Lambda whose body is a special marker form?
    // Simpler: build (lambda (&rest r) (apply 'f a b r)) — needs eval
    // env; instead build a Lambda directly calling apply via a thunk.
    // Easiest correct approach: a lambda whose body form is
    // (apply f 'a 'b ... r) evaluated at call time.
    // GNU eval.c Fapply_partially builds a real byte-code function:
    // `(apply-partially f a b)' = a #[...] object whose code applies
    // `f' to `(append (a b) MORE-ARGS)'.  `byte-code-function-p' and
    // `type-of' report byte-code-function; `subrp' stays nil.
    let fun = args[0].clone();
    let fixed = args[1..].to_vec();
    // Consts match GNU's layout: [FIXED-LIST FUN apply append].
    let consts = vec![
        Value::list(fixed),
        fun,
        Value::Sym(i.intern("apply")),
        Value::Sym(i.intern("append")),
    ];
    // argdesc for `(&rest ARGS2)' = 128 (req=0, rest bit, max=0).
    // Byte program (remacs opcode set): push consts[2] (apply), consts[1]
    // (fun), then consts[0] (fixed list), stack-ref 4 (the ARGS2 slot —
    // args sit below the pushed constants), append2, call2, return.
    // Byte-code text is a unibyte byte vector in GNU — build it as
    // chars, then `mark_unibyte' moves the ≥0x80 codepoints into the
    // eight-bit proxy range so `prin1' escapes them as `\NNN'.
    let byte_str_s = Value::string(
        [194u8, 193, 195, 192, 4, 34, 34, 135]
            .iter()
            .map(|b| char::from_u32(*b as u32).unwrap())
            .collect::<String>(),
    );
    if let Value::Str(r) = &byte_str_s {
        i.mark_unibyte(r);
    }
    let r_sym = i.intern("--apply-partially-args");
    let lam = crate::lisp::value::Lambda {
        is_macro: false,
        required: Vec::new(),
        optional: Vec::new(),
        rest: Some(r_sym),
        body: Vec::new(),
        env: None,
        // GNU returns the compiled-function tail "\n\n(fn &rest ARGS2)"
        // — the (documentation) string, not a real docstring.
        doc: Some("\n\n(fn &rest ARGS2)".into()),
        interactive: None,
        name: None,
        bad_arglist: false,
        arglist: None,
        plain: false,
        dumped_doc: false,
        compiled: false,
        advice_link: None,
        bc_items: Some(std::rc::Rc::new(std::cell::RefCell::new(vec![
            Value::Int(128),
            byte_str_s,
            Value::Vec(std::rc::Rc::new(std::cell::RefCell::new(consts))),
            Value::Int(6),
            Value::string("\n\n(fn &rest ARGS2)"),
        ]))),
        bc_closure: false,
        bc_prog: std::cell::RefCell::new(None),
        doc_value: None,
        env_value: None,
    };
    Ok(Value::Lambda(std::rc::Rc::new(lam)))
}

fn f_assoc_string(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    // (assoc-string KEY LIST CASE-FOLD) — KEY is a string or symbol
    // (symbols compare by name).  Elements may be bare
    // strings/symbols or conses whose car is a string/symbol;
    // anything else is skipped.  GNU stops at a non-cons tail
    // without a type check.
    // GNU Fassoc_string (fns.c): the key's stringp check happens inside
    // the element loop — only reached when an element (or its car) is a
    // string/symbol to compare against.  Empty or non-string-element
    // lists return nil even for non-string keys.
    let key_string: Option<String> = match &args[0] {
        Value::Str(s) => Some(s.borrow().clone()),
        Value::Sym(s) => Some(i.symbol_name(*s).to_string()),
        _ => None,
    };
    let fold = arg(&args, 2).truthy();
    let name_of = |i: &Interp, v: &Value| -> Option<String> {
        match v {
            Value::Str(s) => Some(s.borrow().clone()),
            Value::Sym(s) => Some(i.symbol_name(*s).to_string()),
            _ => None,
        }
    };
    let matches = |s: &str, key: &str| -> bool {
        if fold {
            s.eq_ignore_ascii_case(key)
        } else {
            s == key
        }
    };
    let mut cur = args[1].clone();
    let mut guard = 0usize;
    loop {
        guard += 1;
        if guard > 500_000 {
            return Err(err_circular(i));
        }
        match cur {
            Value::Cons(c) => {
                let (elem, next) = {
                    let b = c.borrow();
                    (b.car.clone(), b.cdr.clone())
                };
                // Key check only fires once a comparable element appears.
                let check_key = |i: &mut Interp| -> Result<&str, Flow> {
                    match &key_string {
                        Some(k) => Ok(k.as_str()),
                        None => Err(i.wrong_type_mut("stringp", &args[0])),
                    }
                };
                let try_elem = |i: &mut Interp, v: &Value| -> Result<bool, Flow> {
                    if let Some(s) = name_of(i, v) {
                        let k = check_key(i)?;
                        return Ok(matches(&s, k));
                    }
                    Ok(false)
                };
                if try_elem(i, &elem)? {
                    return Ok(elem);
                }
                if let Value::Cons(pair) = &elem {
                    let k = pair.borrow().car.clone();
                    if try_elem(i, &k)? {
                        return Ok(elem);
                    }
                }
                cur = next;
            }
            _ => return Ok(Value::Nil),
        }
    }
}

fn f_equal_incl_props(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    let ok = match (&args[0], &args[1]) {
        (Value::Str(sa), Value::Str(sb)) => {
            let (ta, tb) = (sa.borrow(), sb.borrow());
            *ta == *tb && {
                // GNU's `internal_equal' requires every position's
                // property list to be `equal'.
                let (ia, ib) = (i.str_props(sa).to_vec(), i.str_props(sb).to_vec());
                (0..ta.chars().count()).all(|p| {
                    equal_values(
                        i,
                        &Value::list(crate::buffer::primitives::str_plist_at(&ia, p)),
                        &Value::list(crate::buffer::primitives::str_plist_at(&ib, p)),
                    )
                })
            }
        }
        _ => equal_values(i, &args[0], &args[1]),
    };
    Ok(Value::from_bool(ok))
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

fn f_remove(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    remove_impl(i, &args[0], &args[1], |ii, a, b| equal_values(ii, a, b))
}
fn f_remq(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    // GNU `remq' (subr.el): `(while (and (eq elt (car list)) (setq
    // list (cdr list)))) (if (memq elt list) (delq elt
    // (copy-sequence list)) list)' — when nothing is removed the
    // ORIGINAL list object is returned (cell identity preserved),
    // which fixpoint loops such as `byte-optimize-cond' rely on.
    let elt = &args[0];
    let mut list = args[1].clone();
    loop {
        match &list {
            Value::Cons(c) => {
                let (car, next) = {
                    let b = c.borrow();
                    (b.car.clone(), b.cdr.clone())
                };
                if eq_values(elt, &car) {
                    list = next;
                } else {
                    break;
                }
            }
            Value::Nil => break,
            other => return Err(i.wrong_type_mut("listp", other)),
        }
    }
    // memq the remainder: if ELT still occurs, delq a copy.
    let mut found = false;
    let mut cur = list.clone();
    let mut guard = 0usize;
    loop {
        guard += 1;
        if guard > 500_000 {
            return Err(err_circular(i));
        }
        match &cur {
            Value::Cons(c) => {
                let (car, next) = {
                    let b = c.borrow();
                    (b.car.clone(), b.cdr.clone())
                };
                if eq_values(elt, &car) {
                    found = true;
                    break;
                }
                cur = next;
            }
            // GNU `memq' signals wrong-type-argument on a non-nil
            // atom tail (dotted lists); mirror that instead of
            // silently returning the input.
            Value::Nil => break,
            other => return Err(i.wrong_type_mut("listp", other)),
        }
    }
    if found {
        // copy-sequence + delq (delq splices the copy in place).
        let items = want_list(i, &list)?;
        let copied = Value::list(items);
        del_impl(i, elt, &copied, true)
    } else {
        Ok(list)
    }
}
