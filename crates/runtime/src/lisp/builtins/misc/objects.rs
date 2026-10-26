//! Ordinary record construction and opaque-array classification.

use super::chartables::CT_MAX_CHAR;
use crate::lisp::{
    eq_values,
    error::Flow,
    value::{Arity, SymId},
};

use super::bool_vectors::bool_vec_of;

use super::is_opaque_record;

use super::functions::lambda_slot_values;

use super::positions::sym_pos_parts;

use super::positions::sympos_enabled_p;

use super::bool_vectors::is_bool_vector;
use super::chartables::is_char_table;
use super::positions::as_sympos;
use crate::lisp::{Interp, error::EvalResult, value::Value};

pub(super) fn f_record(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    crate::lisp::builtins::data::check_record_slots(i, a.len() as i128)?;
    Ok(Value::record(a.as_slice().to_vec()))
}

pub(super) fn f_recordp(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    Ok(Value::from_bool(is_record(i, &a[0])))
}

pub(crate) fn is_record(i: &Interp, value: &Value) -> bool {
    // `symbols-with-pos-enabled' objects are symbols, not records;
    // obarrays are their own pseudovec type in GNU (PVEC_OBARRAY,
    // distinct from PVEC_RECORD) so `recordp' is nil on them.
    // A font-spec is stored as a record internally but is a
    // pseudovector (PVEC_FONT), not a record, in GNU — `recordp' is
    // nil on it (type-of reports `font-spec').
    matches!(value, Value::Record(_))
        && !crate::editor::window_configuration_object(value)
        && value.record_kind().is_none()
        && as_sympos(value).is_none()
        && !crate::lisp::builtins::data::is_obarray(i, value)
}

/// Record-backed objects accepted by GNU's length/element operations.
/// This is broader than the public sequencep predicate (ordinary records
/// are permitted here) but excludes opaque pseudovectors and symbols.
pub(crate) fn record_array_accessible(i: &Interp, value: &Value) -> bool {
    is_record(i, value) || is_bool_vector(i, value) || is_char_table(i, value)
}

pub(crate) fn has_opaque_array_storage(i: &Interp, value: &Value) -> bool {
    i.is_overlay(value) || matches!(value, Value::Record(_)) && !record_array_accessible(i, value)
}

pub(super) fn f_string_or_null_p(_i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    Ok(Value::from_bool(matches!(
        &a[0],
        Value::Str(_) | Value::Nil
    )))
}

pub(super) fn f_vector_or_char_table_p(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU's VECTOR_OR_CHAR_TABLE_P covers vectors and char-tables.
    Ok(Value::from_bool(
        matches!(&a[0], Value::Vec(_)) || is_char_table(i, &a[0]),
    ))
}

pub(super) fn f_make_finalizer(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU CHECK_FUNCTION's the
    // FUNCTION argument → `functionp'.  The function is retained but
    // never invoked (no GC hooks needed for observable behavior).
    if !crate::lisp::builtins::data::functionp_val(i, &a[0]) {
        return Err(i.wrong_type_mut("functionp", &a[0]));
    }
    Ok(Value::Finalizer(std::rc::Rc::new(std::cell::RefCell::new(a[0].clone()))).tracked())
}

pub(super) fn f_cl_type_of(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let name = match &a[0] {
        Value::Int(n)
            if (crate::lisp::value::FIXNUM_MIN..=crate::lisp::value::FIXNUM_MAX).contains(n) =>
        {
            "fixnum"
        }
        Value::Int(_) | Value::BigInt(_) => "bignum",
        Value::Float(_) => "float",
        Value::Sym(_) => "symbol",
        // GNU `cl-type-of' reports `null' for nil and a record's type
        // field (slot 0) when it is a symbol — like `type-of'.
        Value::Nil => "null",
        Value::Cons(_) => "cons",
        Value::Str(_) => "string",
        Value::Vec(_) => "vector",
        Value::Record(r) => {
            let rr = r.borrow();
            match rr.first() {
                Some(Value::Sym(tag)) => return Ok(Value::Sym(*tag)),
                // GNU: when slot 0 is itself a record with size > 1 it
                // is a class object (EIEIO instances mid-`initialize-instance')
                // and the reported type is that object's name field (slot 1).
                Some(Value::Record(t)) if t.borrow().len() > 1 => {
                    return Ok(t.borrow()[1].clone());
                }
                _ => {}
            }
            "record"
        }
        Value::Hash(_) => "hash-table",
        Value::Subr(s) => {
            if matches!(s.arity, Arity::Unevalled) {
                "special-form"
            } else if crate::lisp::builtins::comp::is_native(s).is_some() {
                "native-comp-function"
            } else {
                "primitive-function"
            }
        }
        Value::Lambda(l) => {
            if l.is_bytecode() {
                "byte-code-function"
            } else {
                "interpreted-function"
            }
        }
        Value::Buffer(_) => "buffer",
        Value::Marker(_) => "marker",
        Value::Window(_) => "window",
        Value::Frame(_) => "frame",
        Value::Process(_) => "process",
        Value::Thread(_) => "thread",
        Value::Mutex(_) => "mutex",
        Value::CondVar(_) => "condition-variable",
        Value::Finalizer(_) => "finalizer",
    };
    Ok(Value::Sym(i.intern(name)))
}

pub(super) fn f_memory_limit(_i: &mut Interp, _a: crate::lisp::Argv) -> EvalResult {
    // GNU subr.el: (cdr (assq 'vsize (process-attributes (emacs-pid))))
    // — own process virtual size in KiB via ps(1).
    let pid = std::process::id();
    if let Ok(o) = std::process::Command::new("ps")
        .args(["-p", &pid.to_string(), "-o", "vsz="])
        .output()
    {
        let text = String::from_utf8_lossy(&o.stdout);
        if let Some(v) = text
            .split_whitespace()
            .next()
            .and_then(|s| s.parse::<i128>().ok())
        {
            return Ok(Value::Int(v));
        }
    }
    Ok(Value::Int(0))
}

/// Conses reachable from LIST's head for length=/length</length>:
/// GNU counts elements, stopping silently at a non-list tail (dotted
/// lists don't error), and a circular list is unbounded (reported as
/// i128::MAX so < is false, > is true).
pub(super) fn cons_seq_len(v: &Value) -> i128 {
    let mut n = 0i128;
    let mut cur = v.clone();
    let mut tortoise = v.clone();
    loop {
        match &cur {
            Value::Cons(c) => {
                let next = c.borrow().cdr.clone();
                cur = next;
                n += 1;
                if n % 2 == 0 {
                    if let Value::Cons(tc) = &tortoise {
                        let t = tc.borrow().cdr.clone();
                        tortoise = t;
                    }
                }
                if n > 1 && eq_values(&cur, &tortoise) {
                    return i128::MAX;
                }
            }
            _ => return n,
        }
    }
}

pub(super) fn seq_len(i: &mut Interp, v: &Value) -> Result<i128, Flow> {
    if has_opaque_array_storage(i, v) {
        return Err(i.wrong_type_mut("sequencep", v));
    }
    Ok(match v {
        Value::Nil => 0,
        Value::Cons(_) => cons_seq_len(v),
        Value::Str(s) => s.borrow().chars().count() as i128,
        Value::Vec(x) => x.borrow().len() as i128,
        // Bool-vectors are sequences of their bit count; char-tables
        // cover the whole internal char space (MAX_CHAR + 1).
        Value::Record(_) if is_bool_vector(i, v) => bool_vec_of(i, v)?.len() as i128,
        Value::Record(_) if is_char_table(i, v) => CT_MAX_CHAR as i128 + 1,
        Value::Record(_) if is_opaque_record(i, v) => {
            return Err(i.wrong_type_mut("sequencep", v));
        }
        Value::Record(x) => x.borrow().len() as i128,
        Value::Lambda(l) => lambda_slot_values(l).len() as i128,
        // Hash tables aren't sequences (GNU signals sequencep).
        _ => return Err(i.wrong_type_mut("sequencep", v)),
    })
}

pub(super) fn length_cmp(i: &mut Interp, a: &[Value], cmp: i8) -> EvalResult {
    // GNU Flength_less/eq/greater checks LENGTH (arg2, CHECK_FIXNUM)
    // before SEQUENCE (arg1) — `(length< 's 'x)' → fixnump x.
    let len = match &a[1] {
        Value::Int(x) => *x,
        other => return Err(i.wrong_type_mut("fixnump", other)),
    };
    // GNU length= can reject a negative size without inspecting SEQUENCE.
    if cmp == 0 && len < 0 {
        return Ok(Value::Nil);
    }
    let n = seq_len(i, &a[0])?;
    Ok(Value::from_bool(match cmp {
        -1 => n < len,
        1 => n > len,
        _ => n == len,
    }))
}

pub(super) fn f_length_lt(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    length_cmp(i, &a, -1)
}

pub(super) fn f_length_gt(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    length_cmp(i, &a, 1)
}

pub(super) fn f_length_eq(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    length_cmp(i, &a, 0)
}

pub(super) fn value_order_sympos(i: &Interp, value: &Value) -> Option<(SymId, i128)> {
    sym_pos_parts(i, value).or_else(|| as_sympos(value))
}

pub(super) fn value_order_kind(i: &Interp, value: &Value) -> Option<ValueOrderKind> {
    use crate::lisp::value::RecordKind;
    use ValueOrderKind as K;
    if crate::editor::window_configuration_object(value) {
        return Some(K::WindowConfiguration);
    }
    if i.is_overlay(value) {
        return Some(K::Overlay);
    }
    if value_order_sympos(i, value).is_some() {
        return Some(K::SymbolWithPos);
    }
    Some(match value {
        Value::Vec(_) => K::Vector,
        Value::Record(_) => match value.record_kind() {
            Some(RecordKind::BoolVector) => K::BoolVector,
            Some(RecordKind::CharTable) => K::CharTable,
            Some(RecordKind::Obarray) => K::Obarray,
            Some(RecordKind::FontSpec) => K::Font,
            Some(RecordKind::SymbolWithPos) => K::SymbolWithPos,
            None => K::Record,
        },
        Value::Hash(_) => K::Hash,
        Value::Buffer(_) => K::Buffer,
        Value::Marker(_) => K::Marker,
        Value::Window(_) => K::Window,
        Value::Frame(_) => K::Frame,
        Value::Process(_) => K::Process,
        Value::Thread(_) => K::Thread,
        Value::Mutex(_) => K::Mutex,
        Value::CondVar(_) => K::CondVar,
        Value::Finalizer(_) => K::Finalizer,
        Value::Subr(_) => K::Subr,
        Value::Lambda(function) if function.compiled && !function.is_macro => K::Subr,
        Value::Lambda(_) => K::Closure,
        Value::BigInt(_) => K::BigInt,
        _ => return None,
    })
}

pub(super) fn value_mismatch(i: &mut Interp, x: &Value, y: &Value) -> Flow {
    let (a, b) = if matches!(x, Value::BigInt(_))
        && matches!(
            y,
            Value::Nil | Value::Sym(_) | Value::Str(_) | Value::Cons(_)
        ) {
        (y, x)
    } else {
        (x, y)
    };
    let condition = i.q("type-mismatch");
    i.signal_data(condition, vec![a.clone(), b.clone()])
}

pub(super) fn value_array_cmp(
    i: &mut Interp,
    x: &crate::lisp::value::VecRef,
    y: &crate::lisp::value::VecRef,
    depth: u32,
) -> Result<std::cmp::Ordering, Flow> {
    let (nx, ny) = (x.borrow().len(), y.borrow().len());
    for index in 0..nx.min(ny) {
        let a = x.borrow()[index].clone();
        let b = y.borrow()[index].clone();
        let order = value_cmp(i, &a, &b, depth + 1)?;
        if order != std::cmp::Ordering::Equal {
            return Ok(order);
        }
    }
    Ok(nx.cmp(&ny))
}

pub(super) fn value_cmp(
    i: &mut Interp,
    x: &Value,
    y: &Value,
    depth: u32,
) -> Result<std::cmp::Ordering, Flow> {
    use ValueOrderKind as K;
    use std::cmp::Ordering::{Equal, Greater, Less};
    if depth > 200 {
        return Err(i.error("Maximum depth exceeded in comparison"));
    }
    let (px, py) = (value_order_sympos(i, x), value_order_sympos(i, y));
    // BASE_EQ does not normalize symbol-with-pos objects when disabled.
    if px.is_none() && py.is_none() && eq_values(x, y) {
        return Ok(Equal);
    }
    if matches!(x, Value::Int(_) | Value::BigInt(_) | Value::Float(_))
        && matches!(y, Value::Int(_) | Value::BigInt(_) | Value::Float(_))
    {
        return Ok(crate::lisp::builtins::arith::number_order(x, y).unwrap_or(Equal));
    }
    if let (Some((a, _)), Some((b, _))) = (px, py) {
        let a = i.sym(a);
        let b = i.sym(b);
        return value_cmp(i, &a, &b, depth);
    }
    if let Some((a, _)) = px {
        if value_order_kind(i, y).is_none() && sympos_enabled_p(i) {
            let bare = i.sym(a);
            return value_cmp(i, &bare, y, depth);
        }
    }
    match x {
        Value::Nil | Value::Sym(_) => {
            let right = match y {
                Value::Nil | Value::Sym(_) => y.clone(),
                Value::Cons(_) if x.is_nil() => return Ok(Less),
                _ => match py {
                    Some((symbol, _)) if sympos_enabled_p(i) => i.sym(symbol),
                    _ => return Err(value_mismatch(i, x, y)),
                },
            };
            let a = crate::lisp::builtins::data::f_symbol_name(i, vec![x.clone()].into())?;
            let b = crate::lisp::builtins::data::f_symbol_name(i, vec![right].into())?;
            let (Value::Str(a), Value::Str(b)) = (a, b) else {
                unreachable!()
            };
            return Ok(crate::lisp::builtins::strfn::compare_string_contents(
                i, &a, &b,
            ));
        }
        Value::Str(a) => {
            return if let Value::Str(b) = y {
                Ok(crate::lisp::builtins::strfn::compare_string_contents(
                    i, a, b,
                ))
            } else {
                Err(value_mismatch(i, x, y))
            };
        }
        Value::Cons(_) => {
            let (mut left, mut right) = (x.clone(), y.clone());
            let mut cycle = crate::lisp::builtins::listfn::ListCycle::new(&right);
            loop {
                let Value::Cons(b) = &right else {
                    return if right.is_nil() {
                        Ok(Greater)
                    } else {
                        Err(value_mismatch(i, &left, &right))
                    };
                };
                let Value::Cons(a) = &left else {
                    unreachable!()
                };
                let (a, b) = (a.clone(), b.clone());
                let ac = a.borrow().car.clone();
                let bc = b.borrow().car.clone();
                let order = value_cmp(i, &ac, &bc, depth + 1)?;
                if order != Equal {
                    return Ok(order);
                }
                left = a.borrow().cdr.clone();
                right = b.borrow().cdr.clone();
                if !matches!(left, Value::Cons(_)) {
                    return value_cmp(i, &left, &right, depth);
                }
                cycle.step(i, &right)?;
            }
        }
        _ => {}
    }
    let (Some(a), Some(b)) = (value_order_kind(i, x), value_order_kind(i, y)) else {
        return Err(value_mismatch(i, x, y));
    };
    if a != b {
        return Err(value_mismatch(i, x, y));
    }
    match a {
        K::Vector | K::Record => {
            let (Value::Vec(a) | Value::Record(a), Value::Vec(b) | Value::Record(b)) = (x, y)
            else {
                unreachable!()
            };
            value_array_cmp(i, a, b, depth)
        }
        K::BoolVector => Ok(bool_vec_of(i, x)?.cmp(&bool_vec_of(i, y)?)),
        K::Marker => {
            let a =
                crate::lisp::builtins::call_canonical_builtin(i, "marker-buffer", vec![x.clone()])?;
            let b =
                crate::lisp::builtins::call_canonical_builtin(i, "marker-buffer", vec![y.clone()])?;
            if a.is_nil() {
                return Ok(if b.is_nil() { Equal } else { Less });
            }
            if b.is_nil() {
                return Ok(Greater);
            }
            let order = value_cmp(i, &a, &b, depth + 1)?;
            if order != Equal {
                return Ok(order);
            }
            let a = crate::lisp::builtins::call_canonical_builtin(
                i,
                "marker-position",
                vec![x.clone()],
            )?;
            let b = crate::lisp::builtins::call_canonical_builtin(
                i,
                "marker-position",
                vec![y.clone()],
            )?;
            Ok(crate::lisp::builtins::arith::number_order(&a, &b).unwrap_or(Equal))
        }
        K::Buffer | K::Process => {
            let operation = if a == K::Buffer {
                "buffer-name"
            } else {
                "process-name"
            };
            let a = crate::lisp::builtins::call_canonical_builtin(i, operation, vec![x.clone()])?;
            let b = crate::lisp::builtins::call_canonical_builtin(i, operation, vec![y.clone()])?;
            if a.is_nil() {
                return Ok(if b.is_nil() { Equal } else { Less });
            }
            if b.is_nil() {
                return Ok(Greater);
            }
            value_cmp(i, &a, &b, depth)
        }
        // Other matching pseudovector types have no standard ordering.
        _ => Ok(Equal),
    }
}

pub(in crate::lisp::builtins) fn f_value_lt(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    Ok(Value::from_bool(
        value_cmp(i, &a[0], &a[1], 0)? == std::cmp::Ordering::Less,
    ))
}

pub(super) fn f_memory_use_counts(_i: &mut Interp, _a: crate::lisp::Argv) -> EvalResult {
    let counts = crate::lisp::memory::counts();
    Ok(Value::list(
        counts.into_iter().map(|n| Value::Int(n as i128)).collect(),
    ))
}

/// GNU's FACTOR must be a nonnegative fixnum. Zero never collects.
pub(super) fn f_garbage_collect_maybe(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let factor = match &a[0] {
        Value::Int(n) if *n >= 0 && *n <= crate::lisp::value::FIXNUM_MAX => *n as u64,
        other => return Err(i.wrong_type_mut("wholenump", other)),
    };
    if factor == 0 {
        return Ok(Value::Nil);
    }
    let threshold = i.q("gc-cons-threshold");
    let mut threshold = match i.symbol_value(threshold) {
        Value::Int(n) => n.max(0) as u64,
        _ => 800_000,
    };
    let percentage = i.q("gc-cons-percentage");
    if let Value::Float(percentage) = i.symbol_value(percentage) {
        if *percentage > 0.0 {
            threshold = threshold
                .max((crate::lisp::memory::heap_stats().bytes as f64 * *percentage) as u64);
        }
    }
    if crate::lisp::memory::allocated_since_gc() > threshold / factor {
        return Ok(if crate::lisp::memory::collect_interp(i) {
            Value::t()
        } else {
            Value::Nil
        });
    }
    Ok(Value::Nil)
}

pub(super) fn f_gc_heapsize(i: &mut Interp, _a: crate::lisp::Argv) -> EvalResult {
    Ok(crate::lisp::memory::report(i))
}

/// `profiler-memory-running-p` — whether the memory profiler is on.
pub(super) fn f_profiler_memory_running_p(i: &mut Interp, _a: crate::lisp::Argv) -> EvalResult {
    Ok(if i.memory_profiler {
        Value::t()
    } else {
        Value::Nil
    })
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ValueOrderKind {
    Vector,
    Record,
    BoolVector,
    CharTable,
    Obarray,
    Font,
    WindowConfiguration,
    Overlay,
    Hash,
    Buffer,
    Marker,
    Window,
    Frame,
    Process,
    Thread,
    Mutex,
    CondVar,
    Finalizer,
    Subr,
    Closure,
    BigInt,
    SymbolWithPos,
}
