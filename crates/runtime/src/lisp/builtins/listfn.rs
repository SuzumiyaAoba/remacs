//! List-manipulation subrs: car, cdr, cons, nth, append, member, etc.

use super::{S, arg, eq_values, equal_values, want_cons, want_int, want_list};
use crate::lisp::Interp;
use crate::lisp::error::{EvalResult, Flow};
use crate::lisp::obarray::sym;
use crate::lisp::value::{Subr, Value};
use num_traits::{ToPrimitive, Zero};
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
    if crate::editor::window_configuration_object(&args[0]) || i.is_overlay(&args[0]) {
        return Err(i.wrong_type_mut("sequencep", &args[0]));
    }
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
        Value::Record(_) if crate::lisp::builtins::misc::is_opaque_record(i, &args[0]) => {
            Err(i.wrong_type_mut("sequencep", &args[0]))
        }
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
    // GNU safe_length: Brent's teleporting tortoise-hare walk —
    // FOR_EACH_TAIL_SAFE with li = { tortoise=LIST, max=2, n=0, q=2 }.
    // The count reaches the detection cons inclusive; on a cycle it
    // can exceed the distinct-element count (GNU guarantees only "at
    // least the number of distinct elements").
    let mut len = 0i128;
    let mut tail = args[0].clone();
    let mut tortoise = args[0].clone();
    let mut max: i64 = 2;
    let mut n: i64 = 0;
    let mut q: u16 = 2;
    while matches!(tail, Value::Cons(_)) {
        len += 1; // the loop body counts each cons first, then steps
        // step: tail = cdr(tail)
        tail = match &tail {
            Value::Cons(c) => c.borrow().cdr.clone(),
            other => other.clone(),
        };
        q = q.wrapping_sub(1);
        let cyclep = if q != 0 {
            // --q != 0 → check BASE_EQ
            eq_values(&tail, &tortoise)
        } else {
            n -= 1;
            if n > 0 {
                eq_values(&tail, &tortoise)
            } else {
                max <<= 1;
                q = max as u16; // li.q = li.n = (max <<= 1) first...
                n = max >> 16; // ...then li.n >>= USHRT_WIDTH
                tortoise = tail.clone();
                false
            }
        };
        if cyclep {
            break;
        }
    }
    Ok(Value::Int(len))
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

/// cdr N times the way GNU does: Cons → cdr, nil → return nil,
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
            Value::Nil => return Ok(Value::Nil),
            _ => return Err(i.wrong_type_mut("listp", v)),
        }
    }
    Ok(cur)
}

fn nthcdr_integer(i: &mut Interp, list: &Value, count: &Value) -> EvalResult {
    if let Value::Int(n) = count {
        if *n <= 127 {
            return nthcdr_strict(i, list, n.max(&0).to_owned() as usize);
        }
    }
    let n = count
        .as_bigint()
        .ok_or_else(|| i.wrong_type_mut("integerp", count))?;
    if n.sign() != num_bigint::Sign::Plus {
        return Ok(list.clone());
    }
    if let Some(small) = n.to_usize().filter(|n| *n <= 127) {
        return nthcdr_strict(i, list, small);
    }
    // Large indices must stop at nil or jump around a detected cycle,
    // rather than converting the index to a wrapping machine counter.
    let limit = n.to_usize();
    let mut nodes: Vec<Value> = Vec::new();
    let mut visited: std::collections::HashMap<usize, usize> = std::collections::HashMap::new();
    let mut tail = list.clone();
    while limit.is_none_or(|limit| nodes.len() < limit) {
        match tail {
            Value::Nil => return Ok(Value::Nil),
            Value::Cons(ref cell) => {
                let pointer = Rc::as_ptr(cell) as usize;
                if let Some(&start) = visited.get(&pointer) {
                    let period = nodes.len() - start;
                    let remaining = (&n - num_bigint::BigInt::from(nodes.len()))
                        % num_bigint::BigInt::from(period);
                    return Ok(nodes[start + remaining.to_usize().unwrap()].clone());
                }
                visited.insert(pointer, nodes.len());
                nodes.push(tail.clone());
                let next = cell.borrow().cdr.clone();
                tail = next;
            }
            _ => return Err(i.wrong_type_mut("listp", list)),
        }
    }
    Ok(tail)
}
pub(crate) fn f_nth(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    let tail = nthcdr_integer(i, &args[1], &args[0])?;
    match tail {
        Value::Cons(cell) => Ok(cell.borrow().car.clone()),
        Value::Nil => Ok(Value::Nil),
        ref other => Err(i.wrong_type_mut("listp", other)),
    }
}
fn f_nthcdr(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    nthcdr_integer(i, &args[1], &args[0])
}
fn f_last(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    // GNU subr.el `last':
    // (if n (and (>= n 0) (let ((m (safe-length list)))
    //             (if (< n m) (nthcdr (- m n) list) list)))
    //   (and list (nthcdr (1- (safe-length list)) list)))
    // `(>= n 0)' on a non-number signals `number-or-marker-p' BEFORE the
    // safe-length walk; a non-list arg returns itself (0 cells) when n
    // is absent or nonnegative, nil when n < 0.
    let n = args.get(1).cloned().unwrap_or(Value::Nil);
    let safe_length = Value::Sym(i.q("safe-length"));
    let nthcdr = Value::Sym(i.q("nthcdr"));
    let greater_equal = Value::Sym(i.q(">="));
    let less = Value::Sym(i.q("<"));
    let subtract = Value::Sym(i.q("-"));
    let one_minus = Value::Sym(i.q("1-"));
    if !n.is_nil() {
        if i.apply(&greater_equal, vec![n.clone(), Value::Int(0)])?.is_nil() {
            return Ok(Value::Nil);
        }
        let length = i.apply(&safe_length, vec![args[0].clone()])?;
        if i.apply(&less, vec![n.clone(), length.clone()])?.truthy() {
            let index = i.apply(&subtract, vec![length, n])?;
            i.apply(&nthcdr, vec![index, args[0].clone()])
        } else {
            Ok(args[0].clone())
        }
    } else if args[0].is_nil() {
        Ok(Value::Nil)
    } else {
        let length = i.apply(&safe_length, vec![args[0].clone()])?;
        let index = i.apply(&one_minus, vec![length])?;
        i.apply(&nthcdr, vec![index, args[0].clone()])
    }
}
fn number_or_marker_value(i: &mut Interp, value: &Value) -> Result<Value, Flow> {
    match value {
        Value::Int(_) | Value::BigInt(_) | Value::Float(_) => Ok(value.clone()),
        Value::Marker(marker) if marker.borrow().buffer.is_some() => {
            Ok(Value::Int(marker.borrow().position as i128 + 1))
        }
        Value::Marker(_) => Err(i.error("Marker does not point anywhere")),
        other => Err(i.wrong_type_mut("number-or-marker-p", other)),
    }
}

fn f_butlast(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    // GNU subr.el: nonpositive N returns LIST before evaluating LENGTH;
    // positive N computes (take (- (length LIST) N) LIST), leaving a
    // fractional result for `take' to reject as integerp.
    let n = match args.get(1) {
        None | Some(Value::Nil) => Value::Int(1),
        Some(v) => number_or_marker_value(i, v)?,
    };
    let zero = Value::Int(0);
    if crate::lisp::builtins::arith::number_order(&n, &zero)
        .is_some_and(|order| order != std::cmp::Ordering::Greater)
    {
        return Ok(args[0].clone());
    }
    let len = match &args[0] {
        Value::Nil => 0usize,
        Value::Cons(_) => args[0]
            .list_to_vec()
            .map_err(|e| match e {
                crate::lisp::value::ListError::Dotted(tail) => i.wrong_type_mut("listp", &tail),
                _ => i.wrong_type_mut("listp", &args[0]),
            })?
            .len(),
        Value::Str(s) => s.borrow().chars().count(),
        Value::Vec(v) => v.borrow().len(),
        other => return Err(i.wrong_type_mut("sequencep", other)),
    };
    let take_count = match n {
        Value::Int(n) => Value::integer(len as i128 - n),
        Value::BigInt(n) => Value::from_bigint(num_bigint::BigInt::from(len) - &*n),
        Value::Float(n) => Value::float(len as f64 - *n),
        _ => unreachable!(),
    };
    f_take(i, vec![take_count, args[0].clone()].into())
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
        None | Some(Value::Nil) => Value::Int(1),
        Some(v) => number_or_marker_value(i, v)?,
    };
    if crate::lisp::builtins::arith::number_order(&n, &Value::Int(m))
        != Some(std::cmp::Ordering::Less)
    {
        return Ok(Value::Nil);
    }
    if crate::lisp::builtins::arith::number_order(&n, &Value::Int(0))
        == Some(std::cmp::Ordering::Greater)
    {
        let index = match &n {
            Value::Int(n) => Value::integer(m - 1 - *n),
            Value::BigInt(n) => Value::from_bigint(num_bigint::BigInt::from(m - 1) - &**n),
            Value::Float(n) => Value::float((m - 1) as f64 - **n),
            _ => unreachable!(),
        };
        if !matches!(index, Value::Int(_) | Value::BigInt(_)) {
            return Err(i.wrong_type_mut("integerp", &index));
        }
        let index = index
            .as_bigint()
            .and_then(|n| n.to_usize())
            .ok_or_else(|| i.wrong_type_mut("integerp", &index))?;
        // GNU evaluates `(nthcdr (- (1- m) n) list)' then `setcdr's it:
        // a non-list sequence reaches Fsetcdr and signals `consp'.
        match &args[0] {
            Value::Cons(_) | Value::Nil => {}
            other => return Err(i.wrong_type_mut("consp", other)),
        }
        let tail = nthcdr_of(&args[0], index);
        if let Value::Cons(c) = tail {
            c.borrow_mut().cdr = Value::Nil;
        }
    }
    Ok(args[0].clone())
}
pub(crate) fn f_append(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    if args.is_empty() {
        return Ok(Value::Nil);
    }
    let mut out = Vec::new();
    for a in &args[..args.len() - 1] {
        match a {
            Value::Cons(cell) => {
                // GNU concat_to_list copies CAR before running
                // FOR_EACH_TAIL on CDR. Starting the detector at the
                // original head would signal a different circular tail.
                let (head, rest) = {
                    let cell = cell.borrow();
                    (cell.car.clone(), cell.cdr.clone())
                };
                out.push(head);
                out.extend(want_list(i, &rest)?);
            }
            // GNU also accepts closure slots here despite sequencep
            // being nil for function objects.
            Value::Lambda(function) => out.extend(super::misc::lambda_slot_values(function)),
            _ => out.extend(super::seq::seq_to_vec(i, a)?),
        }
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
    if args.is_empty() {
        return Ok(Value::Nil);
    }
    let mut result = Value::Nil;
    let mut last_cons: Option<crate::lisp::value::ConsRef> = None;
    for a in &args[..args.len() - 1] {
        match a {
            Value::Nil => continue,
            Value::Cons(_) => {
                // Link each new non-final list before walking it. GNU's
                // nconc leaves this connection observable if that list is
                // circular and the walk signals.
                match &last_cons {
                    Some(cell) => cell.borrow_mut().cdr = a.clone(),
                    None => result = a.clone(),
                }
                // find last cons of `a`
                let mut tail = a.clone();
                let mut cycle = ListCycle::new(a);
                let last = loop {
                    match &tail {
                        Value::Cons(c) => {
                            let next = {
                                let b = c.borrow();
                                b.cdr.clone()
                            };
                            match next {
                                Value::Cons(_) => {
                                    tail = next;
                                    cycle.step(i, &tail)?;
                                }
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
                last_cons = Some(last);
            }
            // GNU `nconc' CHECK_LIST_CONS's every non-final arg:
            // non-nil non-cons signals `consp' on that arg.
            other => return Err(i.wrong_type_mut("consp", other)),
        }
    }
    // The final argument is the tail itself. In particular, do not walk
    // it: a circular list is a valid final value and retains its identity.
    let tail = args.last().unwrap().clone();
    match &last_cons {
        Some(cell) => cell.borrow_mut().cdr = tail,
        None => result = tail,
    }
    Ok(result)
}
pub(crate) fn f_reverse(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    // Emacs: reverse works on any sequence, preserving its type.
    match &args[0] {
        Value::Str(s) => {
            // GNU uses unibyte storage when SCHARS == SBYTES, even for
            // explicitly multibyte ASCII/empty input. Properties are omitted.
            let multibyte = super::strfn::string_multibyte_p(i, &args[0]) && !s.borrow().is_ascii();
            if s.borrow().is_empty() && !super::strfn::string_multibyte_p(i, &args[0]) {
                return Ok(args[0].clone());
            }
            let result = Value::string(s.borrow().chars().rev().collect::<String>());
            if let Value::Str(output) = &result {
                if multibyte {
                    i.mark_multibyte(output);
                } else {
                    i.mark_unibyte(output);
                }
            }
            return Ok(result);
        }
        Value::Vec(v) => {
            if v.borrow().is_empty() {
                return Ok(i.zero_vector.clone());
            }
            return Ok(Value::vector(v.borrow().iter().rev().cloned().collect()));
        }
        Value::Record(_) if crate::lisp::builtins::misc::is_bool_vector(i, &args[0]) => {
            let mut bits = crate::lisp::builtins::misc::bool_vec_of(i, &args[0])?;
            bits.reverse();
            return Ok(crate::lisp::builtins::misc::make_bool_vector(i, bits));
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

/// GNU FOR_EACH_TAIL's Brent detector. Call after advancing the tail,
/// so successful searches can return before inspecting a circular suffix.
/// Keeping the tortoise as a Value also roots it while an assoc callback
/// mutates the spine. There is no arbitrary maximum proper-list length.
struct ListCycle {
    tortoise: Value,
    max: usize,
    n: isize,
    q: u16,
}
impl ListCycle {
    fn new(list: &Value) -> Self {
        Self {
            tortoise: list.clone(),
            max: 2,
            n: 0,
            q: 2,
        }
    }
    fn step(&mut self, i: &mut Interp, tail: &Value) -> Result<(), Flow> {
        self.q = self.q.wrapping_sub(1);
        if self.q == 0 {
            // GNU checks for quit whenever the low counter expires.
            i.maybe_quit()?;
            self.n -= 1;
            if self.n <= 0 {
                self.max = self.max.saturating_mul(2);
                self.q = self.max as u16;
                self.n = (self.max >> 16) as isize;
                self.tortoise = tail.clone();
                return Ok(());
            }
        }
        if eq_values(tail, &self.tortoise) {
            return Err(i.signal_data(sym::CIRCULAR_LIST, vec![tail.clone()]));
        }
        Ok(())
    }
}

/// copy-sequence copies the first cons before initializing FOR_EACH_TAIL
/// at its cdr. Match that phase when remove/remq copy a list, including
/// the exact source tail reported for a cycle or dotted-list error.
pub(super) fn copy_list_items(i: &mut Interp, list: &Value) -> Result<Vec<Value>, Flow> {
    let (mut items, mut tail) = match list {
        Value::Nil => return Ok(Vec::new()),
        Value::Cons(c) => {
            let c = c.borrow();
            (vec![c.car.clone()], c.cdr.clone())
        }
        value => return Err(i.wrong_type_mut("listp", value)),
    };
    let mut cycle = ListCycle::new(&tail);
    loop {
        match &tail {
            Value::Cons(c) => {
                let next = {
                    let c = c.borrow();
                    items.push(c.car.clone());
                    c.cdr.clone()
                };
                tail = next;
                cycle.step(i, &tail)?;
            }
            Value::Nil => return Ok(items),
            value => return Err(i.wrong_type_mut("listp", value)),
        }
    }
}

fn member_impl(
    i: &mut Interp,
    elt: &Value,
    list: &Value,
    cmp: fn(&Interp, &Value, &Value) -> Result<bool, Flow>,
) -> EvalResult {
    // Raw-pointer spine walk (see `cdr_ptr'): `cmp' is a pure
    // predicate — `eq'/`eql'/`equal' never run Lisp — so the walk can
    // read cdrs without a borrow/clone per cell.  `cur' still yields
    // the tail `Value' to return by cloning only on a hit.
    let mut cur = list as *const Value;
    let mut cycle = ListCycle::new(list);
    loop {
        match unsafe { &*cur } {
            Value::Cons(c) => {
                let cell = RefCell::as_ptr(c);
                if cmp(i, elt, unsafe { &(*cell).car })? {
                    return Ok(unsafe { &*cur }.clone());
                }
                cur = unsafe { &(*cell).cdr };
                cycle.step(i, unsafe { &*cur })?;
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
    member_impl(i, &args[0], &args[1], |_ii, a, b| Ok(eq_values(a, b)))
}
fn f_memql(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    member_impl(i, &args[0], &args[1], |_ii, a, b| {
        Ok(super::eql_values(a, b))
    })
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
    cmp: fn(&Interp, &Value, &Value) -> Result<bool, Flow>,
    cdr_cmp: bool,
) -> EvalResult {
    // Raw-pointer spine walk like `member_impl'.
    let mut cur = list as *const Value;
    let mut cycle = ListCycle::new(list);
    loop {
        match unsafe { &*cur } {
            Value::Cons(c) => {
                let cell = RefCell::as_ptr(c);
                if let Value::Cons(ec) = unsafe { &(*cell).car } {
                    let eb = ec.borrow();
                    let probe = if cdr_cmp { &eb.cdr } else { &eb.car };
                    if cmp(i, key, probe)? {
                        drop(eb);
                        return Ok(unsafe { &*cell }.car.clone());
                    }
                }
                cur = unsafe { &(*cell).cdr };
                cycle.step(i, unsafe { &*cur })?;
            }
            Value::Nil => return Ok(Value::Nil),
            // GNU assoc/assq report the whole LIST argument, not the
            // non-list tail (assq_no_quit's `xsignal listp LIST').
            _ => return Err(i.wrong_type_mut("listp", list)),
        }
    }
}

fn f_assq(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    assoc_impl(
        i,
        &args[0],
        &args[1],
        |_ii, a, b| Ok(eq_values(a, b)),
        false,
    )
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
    // A TESTFN can change the current cell's cdr. Read it after the
    // callback, like FOR_EACH_TAIL, with all RefCell borrows released.
    if let Some(testfn) = args.get(2).filter(|v| v.truthy()) {
        let mut tail = args[1].clone();
        let mut cycle = ListCycle::new(&tail);
        loop {
            match &tail {
                Value::Cons(c) => {
                    let current = c.clone();
                    let element = current.borrow().car.clone();
                    if let Value::Cons(pair) = &element {
                        let key = pair.borrow().car.clone();
                        if i.apply(testfn, vec![key, args[0].clone()])?.truthy() {
                            return Ok(element);
                        }
                    }
                    tail = current.borrow().cdr.clone();
                    cycle.step(i, &tail)?;
                }
                Value::Nil => return Ok(Value::Nil),
                _ => return Err(i.wrong_type_mut("listp", &args[1])),
            }
        }
    }
    assoc_impl(
        i,
        &args[0],
        &args[1],
        |i, a, b| equal_values(i, a, b),
        false,
    )
}

fn f_rassq(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    assoc_impl(i, &args[0], &args[1], |_ii, a, b| Ok(eq_values(a, b)), true)
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
                    equal_values(i, &key, &args[0])?
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
    let mut cycle = ListCycle::new(list);
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
                    equal_values(i, &car, elt)?
                };
                if matched {
                    match &prev {
                        Some(p) => p.borrow_mut().cdr = next.clone(),
                        None => result = next.clone(),
                    }
                } else {
                    prev = Some(c.clone());
                }
                cur = c.borrow().cdr.clone();
                cycle.step(i, &cur)?;
            }
            Value::Nil => break,
            _ => return Err(i.wrong_type_mut("listp", &result)),
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
            let source = s.borrow().clone();
            let unibyte = i.is_unibyte_str(s)
                || !crate::lisp::builtins::strfn::string_multibyte_p(i, &args[1]);
            let mut out = String::new();
            let mut changed = false;
            for c in source.chars() {
                let code = if unibyte {
                    crate::lisp::value::eight_bit_byte(c)
                        .map(|byte| byte as i128)
                        .unwrap_or(c as i128)
                } else {
                    crate::lisp::value::lisp_char_code(c)
                };
                let cv = Value::Int(code);
                if !equal_values(i, &cv, elt)? {
                    out.push(c);
                } else {
                    changed = true;
                }
            }
            if !changed {
                return Ok(args[1].clone());
            }
            let result = crate::lisp::Value::string_ref(out);
            if unibyte {
                i.mark_unibyte(&result);
            } else {
                i.mark_multibyte(&result);
            }
            Ok(Value::Str(result))
        }
        Value::Vec(v) => {
            let mut kept = Vec::new();
            let mut changed = false;
            for x in v.borrow().iter() {
                if !equal_values(i, x, &args[0])? {
                    kept.push(x.clone());
                } else {
                    changed = true;
                }
            }
            if !changed {
                return Ok(args[1].clone());
            }
            Ok(Value::vector(kept))
        }
        Value::Cons(_) | Value::Nil => del_impl(i, &args[0], &args[1], false),
        other => Err(i.wrong_type_mut("sequencep", other)),
    }
}
fn f_copy_alist(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    let items = copy_list_items(i, &args[0])?;
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
    fn cp(i: &Interp, v: &Value, vecp: bool) -> Value {
        match v {
            Value::Cons(c) => {
                let b = c.borrow();
                Value::cons(cp(i, &b.car, vecp), cp(i, &b.cdr, vecp))
            }
            Value::Vec(vec) if vecp => {
                let b = vec.borrow();
                Value::vector(b.iter().map(|x| cp(i, x, vecp)).collect())
            }
            Value::Record(record) if vecp && super::misc::recordp(i, v) => {
                let slots = record.borrow();
                Value::record(slots.iter().map(|slot| cp(i, slot, vecp)).collect())
            }
            other => other.clone(),
        }
    }
    Ok(cp(i, &args[0], vecp))
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
                    equal_values(i, &k, prop)?
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
        Some(Value::Sym(i.q("equal")))
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
    let mut remaining = match &args[0] {
        Value::Int(n) => num_bigint::BigInt::from(*n),
        Value::BigInt(n) => (**n).clone(),
        other => return Err(i.wrong_type_mut("integerp", other)),
    };
    if remaining <= num_bigint::BigInt::zero() {
        return Ok(Value::Nil);
    }
    let mut cur = args[1].clone();
    let mut items = Vec::new();
    // Only inspect the requested prefix. A dotted tail is an error only
    // when more elements are requested; nonpositive N never checks LIST.
    while !remaining.is_zero() {
        match cur {
            Value::Cons(cell) => {
                let cell = cell.borrow();
                items.push(cell.car.clone());
                cur = cell.cdr.clone();
                remaining -= 1;
            }
            Value::Nil => break,
            other => return Err(i.wrong_type_mut("listp", &other)),
        }
    }
    Ok(Value::list(items))
}

fn f_ntake(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    let mut remaining = match &args[0] {
        Value::Int(n) => num_bigint::BigInt::from(*n),
        Value::BigInt(n) => (**n).clone(),
        other => return Err(i.wrong_type_mut("integerp", other)),
    };
    // ntake destructively truncates the list after N elements.
    let mut cur = args[1].clone();
    if remaining <= num_bigint::BigInt::zero() {
        return Ok(Value::Nil);
    }
    loop {
        match &cur {
            Value::Cons(c) => {
                let next = c.borrow().cdr.clone();
                remaining -= 1;
                if remaining.is_zero() {
                    c.borrow_mut().cdr = Value::Nil;
                    return Ok(args[1].clone());
                }
                cur = next;
            }
            Value::Nil => return Ok(args[1].clone()),
            // GNU CHECK_LIST reports the original LIST argument.
            _ => return Err(i.wrong_type_mut("listp", &args[1])),
        }
    }
}

fn f_flatten_tree(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    enum Task {
        Visit(Value),
        Leave(usize),
    }
    let mut out = Vec::new();
    let mut stack = vec![Task::Visit(args[0].clone())];
    let mut active = std::collections::HashSet::new();
    while let Some(task) = stack.pop() {
        match task {
            Task::Leave(pointer) => {
                active.remove(&pointer);
            }
            Task::Visit(Value::Cons(cell)) => {
                let pointer = Rc::as_ptr(&cell) as usize;
                if !active.insert(pointer) {
                    return Err(i.signal_data(
                        sym::CIRCULAR_LIST,
                        vec![Value::Cons(cell.clone())],
                    ));
                }
                let (car, cdr) = {
                    let cell = cell.borrow();
                    (cell.car.clone(), cell.cdr.clone())
                };
                stack.push(Task::Leave(pointer));
                // Push cdr first so car pops first (depth-first order).
                stack.push(Task::Visit(cdr));
                stack.push(Task::Visit(car));
            }
            Task::Visit(Value::Nil) => {}
            Task::Visit(other) => out.push(other),
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
        Value::Sym(i.q("apply")),
        Value::Sym(i.q("append")),
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
    let r_sym = i.q("--apply-partially-args");
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
        bc_items: Some(crate::lisp::Value::vector_ref(vec![
            Value::Int(128),
            byte_str_s,
            Value::vector(consts),
            Value::Int(6),
            Value::string("\n\n(fn &rest ARGS2)"),
        ])),
        bc_closure: false,
        bc_prog: std::cell::RefCell::new(None),
        doc_value: None,
        env_value: None,
    };
    Ok(Value::Lambda(std::rc::Rc::new(lam)).tracked())
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
    super::equal_values_including_properties(i, &args[0], &args[1]).map(Value::from_bool)
}

// ---------- remove/delete (sequence-generic) ----------

/// `remove`/`delete`/`remq`/`delq` shared core: filter SEQUENCE by
/// `cmp`, preserving the sequence type. `delete`/`delq` mutate list
/// structure; other sequences are copied (as in Emacs).
fn remove_impl(
    i: &mut Interp,
    elt: &Value,
    seq: &Value,
    cmp: fn(&Interp, &Value, &Value) -> Result<bool, Flow>,
) -> EvalResult {
    match seq {
        Value::Str(s) => {
            let mut kept = String::new();
            for c in s.borrow().chars() {
                if !cmp(i, elt, &Value::Int(c as i128))? {
                    kept.push(c);
                }
            }
            Ok(Value::string(kept))
        }
        Value::Vec(v) => {
            let mut kept = Vec::new();
            for x in v.borrow().iter() {
                if !cmp(i, elt, x)? {
                    kept.push(x.clone());
                }
            }
            Ok(Value::vector(kept))
        }
        Value::Cons(_) | Value::Nil => {
            let items = copy_list_items(i, seq)?;
            let mut kept = Vec::with_capacity(items.len());
            for value in items {
                if !cmp(i, elt, &value)? {
                    kept.push(value);
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
    let found = member_impl(i, elt, &list, |_, a, b| Ok(eq_values(a, b)))?.truthy();
    if found {
        // copy-sequence + delq (delq splices the copy in place).
        let items = copy_list_items(i, &list)?;
        let copied = Value::list(items);
        del_impl(i, elt, &copied, true)
    } else {
        Ok(list)
    }
}
