//! Typed bool-vector construction, access and bit operations.

use crate::lisp::builtins::arg;
use crate::lisp::{
    Interp,
    error::{EvalResult, Flow},
    value::Value,
};

// ---------- bool vectors ----------
// Represented as a Record `#s(bool-vector [bits])' so `bool-vector-p'
// is exact while element access stays cheap.

pub(crate) fn is_bool_vector(_i: &Interp, v: &Value) -> bool {
    matches!(v, Value::Record(_))
        && v.record_kind() == Some(crate::lisp::value::RecordKind::BoolVector)
}

pub(crate) fn bool_vec_of(i: &mut Interp, v: &Value) -> Result<Vec<bool>, Flow> {
    if !is_bool_vector(i, v) {
        return Err(i.wrong_type_mut("bool-vector-p", v));
    }
    if let Value::Record(r) = v {
        let rr = r.borrow();
        if let Some(Value::Vec(b)) = rr.get(1) {
            return Ok(b
                .borrow()
                .iter()
                .map(|x| matches!(x, Value::Int(n) if *n != 0))
                .collect());
        }
    }
    Err(i.wrong_type_mut("bool-vector-p", v))
}

pub(crate) fn make_bool_vector(i: &mut Interp, bits: Vec<bool>) -> Value {
    let data = Value::vector(
        bits.into_iter()
            .map(|b| Value::Int(if b { 1 } else { 0 }))
            .collect(),
    );
    Value::record_with_kind(
        vec![Value::Sym(i.q("bool-vector")), data],
        crate::lisp::value::RecordKind::BoolVector,
    )
}

pub(super) fn f_bool_vector_p(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    Ok(Value::from_bool(is_bool_vector(i, &a[0])))
}

pub(super) fn f_make_bool_vector(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU Fmake_bool_vector: LENGTH is CHECK_FIXNAT — non-integers signal
    // `wholenump' (not `integerp').
    let n = match &a[0] {
        Value::Int(n) => *n,
        other => return Err(i.wrong_type_mut("wholenump", other)),
    };
    let init = !arg(&a, 1).is_nil();
    // GNU Fmake_bool_vector CHECK_FIXNATs LENGTH → `wholenump', not
    // args-out-of-range.
    if n < 0 {
        return Err(i.wrong_type_mut("wholenump", &a[0]));
    }
    if n > 0x2000_0000 * 8 {
        return Err(i.error("Memory exhausted--use C-x s then exit and restart Emacs"));
    }
    Ok(make_bool_vector(i, vec![init; n as usize]))
}

pub(super) fn f_bool_vector_length(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let b = bool_vec_of(i, &a[0])?;
    Ok(Value::Int(b.len() as i128))
}

pub(super) fn f_bool_vector_subsetp(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let x = bool_vec_of(i, &a[0])?;
    let y = bool_vec_of(i, &a[1])?;
    if x.len() != y.len() {
        // GNU Fbool_vector_subsetp: (wrong-length-argument ALEN BLEN
        // BLEN) — the trailing datum is B's length (where the walk
        // stopped), not the vector objects.
        let s = i.q("wrong-length-argument");
        return Err(i.signal_data(
            s,
            vec![
                Value::Int(x.len() as i128),
                Value::Int(y.len() as i128),
                Value::Int(y.len() as i128),
            ],
        ));
    }
    Ok(Value::from_bool(
        x.iter().zip(&y).all(|(p, q)| !(*p && !*q)),
    ))
}

pub(super) fn f_bool_vector_not(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let x = bool_vec_of(i, &a[0])?;
    let out: Vec<bool> = x.iter().map(|b| !*b).collect();
    let target = arg(&a, 1);
    if !target.is_nil() {
        // GNU: B must be a bool-vector of the same length.
        if !is_bool_vector(i, &target) {
            return Err(i.wrong_type_mut("bool-vector-p", &target));
        }
        if let Value::Record(r) = &target {
            let rr = r.borrow();
            if let Some(Value::Vec(b)) = rr.get(1) {
                let mut bb = b.borrow_mut();
                if bb.len() != out.len() {
                    let s = i.q("wrong-length-argument");
                    return Err(i.signal_data(
                        s,
                        vec![Value::Int(out.len() as i128), Value::Int(bb.len() as i128)],
                    ));
                }
                for (k, v) in bb.iter_mut().zip(&out) {
                    *k = Value::Int(if *v { 1 } else { 0 });
                }
                return Ok(target.clone());
            }
        }
    }
    Ok(make_bool_vector(i, out))
}

fn bv_binop(i: &mut Interp, a: &[Value], f: fn(bool, bool) -> bool) -> EvalResult {
    let x = bool_vec_of(i, &a[0])?;
    let y = bool_vec_of(i, &a[1])?;
    let target = arg(a, 2);
    let dest = if target.is_nil() {
        None
    } else {
        Some(bool_vec_of(i, &target)?)
    };
    if x.len() != y.len() || dest.as_ref().is_some_and(|d| d.len() != x.len()) {
        // GNU includes DEST's length only when a destination was supplied.
        let mut lengths = vec![Value::Int(x.len() as i128), Value::Int(y.len() as i128)];
        if let Some(dest) = &dest {
            lengths.push(Value::Int(dest.len() as i128));
        }
        let symbol = i.q("wrong-length-argument");
        return Err(i.signal_data(symbol, lengths));
    }
    let out: Vec<bool> = x.iter().zip(&y).map(|(p, q)| f(*p, *q)).collect();
    if let Some(dest) = dest {
        // A supplied destination is returned only when its contents change.
        // Snapshots of both sources also make aliasing DEST with either safe.
        if dest == out {
            return Ok(Value::Nil);
        }
        if let Value::Record(r) = &target {
            let rr = r.borrow();
            if let Some(Value::Vec(b)) = rr.get(1) {
                for (bit, value) in b.borrow_mut().iter_mut().zip(out) {
                    *bit = Value::Int(i128::from(value));
                }
            }
        }
        return Ok(target);
    }
    Ok(make_bool_vector(i, out))
}

pub(super) fn f_bool_vector_bin(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    bv_binop(i, &a, |p, q| p != q)
}
pub(super) fn f_bool_vector_union(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    bv_binop(i, &a, |p, q| p || q)
}
pub(super) fn f_bool_vector_inter(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    bv_binop(i, &a, |p, q| p && q)
}
pub(super) fn f_bool_vector_diff(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    bv_binop(i, &a, |p, q| p && !q)
}

pub(super) fn f_bool_vector_count(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let x = bool_vec_of(i, &a[0])?;
    Ok(Value::Int(x.iter().filter(|b| **b).count() as i128))
}

pub(super) fn f_bool_vector_consec(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let x = bool_vec_of(i, &a[0])?;
    let b = !a[1].is_nil();
    let at = match &a[2] {
        Value::Int(n) if (0..=crate::lisp::value::FIXNUM_MAX).contains(n) => *n as usize,
        other => return Err(i.wrong_type_mut("wholenump", other)),
    };
    // GNU allows the index just past the last bit, yielding zero.
    if at > x.len() {
        let symbol = i.q("args-out-of-range");
        return Err(i.signal_data(symbol, vec![a[0].clone(), a[2].clone()]));
    }
    Ok(Value::Int(
        x.iter().skip(at).take_while(|v| **v == b).count() as i128,
    ))
}
