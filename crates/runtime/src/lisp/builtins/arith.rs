//! Arithmetic and numeric comparison subrs.

use super::{S, want_num};
use crate::lisp::Interp;
use crate::lisp::error::{EvalResult, Flow};
use crate::lisp::obarray::sym;
use crate::lisp::value::{Marker, Subr, Value};
use num_bigint::BigInt;
use num_integer::Integer;
use num_traits::{FromPrimitive, Signed, ToPrimitive, Zero};
use std::cell::RefCell;
use std::rc::Rc;

fn arith_err(i: &Interp, _msg: &str) -> Flow {
    // GNU signals bare (arith-error) with no payload.
    i.signal_data(sym::ARITH_ERROR, vec![])
}

fn overflow_err(i: &mut Interp, _v: &Value) -> Flow {
    let s = i.q("overflow-error");
    i.signal_data(s, vec![])
}

/// Small integers keep the allocation-free arithmetic path. Large integers
/// remain exact until an operation actually has a floating-point operand.
#[derive(Clone)]
enum Num {
    I(i128),
    B(Rc<BigInt>),
    F(f64),
}
impl Num {
    fn from_big(n: BigInt) -> Self {
        match n.to_i128() {
            Some(n) => Self::I(n),
            None => Self::B(Rc::new(n)),
        }
    }
    fn integer(&self) -> Option<BigInt> {
        match self {
            Self::I(n) => Some(BigInt::from(*n)),
            Self::B(n) => Some((**n).clone()),
            Self::F(_) => None,
        }
    }
    fn float(&self) -> f64 {
        match self {
            Self::I(n) => *n as f64,
            Self::B(n) => n.to_f64().unwrap_or(if n.is_negative() {
                f64::NEG_INFINITY
            } else {
                f64::INFINITY
            }),
            Self::F(n) => *n,
        }
    }
    fn value(self) -> Value {
        match self {
            Self::I(n) => Value::integer(n),
            Self::B(n) => Value::from_bigint(Rc::unwrap_or_clone(n)),
            Self::F(n) => Value::float(n),
        }
    }
}

fn int_f64_ord(n: i128, f: f64) -> Option<std::cmp::Ordering> {
    use std::cmp::Ordering::*;
    if f.is_nan() {
        return None;
    }
    if f >= 2f64.powi(127) {
        return Some(Less);
    }
    if f < -2f64.powi(127) {
        return Some(Greater);
    }
    Some(match n.cmp(&(f.trunc() as i128)) {
        Equal => 0f64.partial_cmp(&f.fract()).unwrap_or(Equal),
        order => order,
    })
}
fn bigint_f64_ord(n: &BigInt, f: f64) -> Option<std::cmp::Ordering> {
    use std::cmp::Ordering::*;
    if f.is_nan() {
        return None;
    }
    if f == f64::INFINITY {
        return Some(Less);
    }
    if f == f64::NEG_INFINITY {
        return Some(Greater);
    }
    let integer = BigInt::from_f64(f.trunc()).unwrap();
    Some(match n.cmp(&integer) {
        Equal => 0f64.partial_cmp(&f.fract()).unwrap_or(Equal),
        order => order,
    })
}
fn num_ord(a: &Num, b: &Num) -> Option<std::cmp::Ordering> {
    match (a, b) {
        (Num::I(a), Num::I(b)) => Some(a.cmp(b)),
        (Num::F(a), Num::F(b)) => a.partial_cmp(b),
        (Num::I(a), Num::F(b)) => int_f64_ord(*a, *b),
        (Num::F(a), Num::I(b)) => int_f64_ord(*b, *a).map(std::cmp::Ordering::reverse),
        (Num::B(a), Num::F(b)) => bigint_f64_ord(a, *b),
        (Num::F(a), Num::B(b)) => bigint_f64_ord(b, *a).map(std::cmp::Ordering::reverse),
        _ => Some(a.integer().unwrap().cmp(&b.integer().unwrap())),
    }
}
pub(crate) fn number_order(a: &Value, b: &Value) -> Option<std::cmp::Ordering> {
    fn raw(value: &Value) -> Option<Num> {
        match value {
            Value::Int(n) => Some(Num::I(*n)),
            Value::BigInt(n) => Some(Num::B(n.clone())),
            Value::Float(n) => Some(Num::F(**n)),
            _ => None,
        }
    }
    num_ord(&raw(a)?, &raw(b)?)
}
fn marker_pos_err(i: &mut Interp, m: &Rc<RefCell<Marker>>) -> Result<i128, Flow> {
    if m.borrow().buffer.is_none() {
        return Err(i.error("Marker does not point anywhere"));
    }
    Ok(m.borrow().position as i128 + 1)
}
fn to_num(i: &mut Interp, v: &Value) -> Result<Option<Num>, Flow> {
    Ok(match v {
        Value::Int(n) => Some(Num::I(*n)),
        Value::BigInt(n) => Some(match n.to_i128() {
            Some(n) => Num::I(n),
            None => Num::B(n.clone()),
        }),
        Value::Float(n) => Some(Num::F(**n)),
        Value::Marker(m) => Some(Num::I(marker_pos_err(i, m)?)),
        _ => None,
    })
}
fn number(i: &mut Interp, value: &Value) -> Result<Num, Flow> {
    to_num(i, value)?.ok_or_else(|| i.wrong_type_mut("number-or-marker-p", value))
}
fn integer(i: &mut Interp, value: &Value, predicate: &str, markers: bool) -> Result<BigInt, Flow> {
    if let Some(n) = value.as_bigint() {
        return Ok(n);
    }
    if markers {
        if let Value::Marker(m) = value {
            return Ok(BigInt::from(marker_pos_err(i, m)?));
        }
    }
    Err(i.wrong_type_mut(predicate, value))
}
#[derive(Clone, Copy)]
enum Op {
    Add,
    Sub,
    Mul,
    Div,
}
fn binary(i: &Interp, a: Num, b: Num, op: Op) -> Result<Num, Flow> {
    if matches!(a, Num::F(_)) || matches!(b, Num::F(_)) {
        let (a, b) = (a.float(), b.float());
        return Ok(Num::F(match op {
            Op::Add => a + b,
            Op::Sub => a - b,
            Op::Mul => a * b,
            Op::Div => a / b,
        }));
    }
    if let (Num::I(a), Num::I(b)) = (&a, &b) {
        if matches!(op, Op::Div) && *b == 0 {
            return Err(arith_err(i, "Division by zero"));
        }
        let value = match op {
            Op::Add => a.checked_add(*b),
            Op::Sub => a.checked_sub(*b),
            Op::Mul => a.checked_mul(*b),
            Op::Div => a.checked_div(*b),
        };
        if let Some(n) = value {
            return Ok(Num::I(n));
        }
    }
    let (a, b) = (a.integer().unwrap(), b.integer().unwrap());
    if matches!(op, Op::Div) && b.is_zero() {
        return Err(arith_err(i, "Division by zero"));
    }
    Ok(Num::from_big(match op {
        Op::Add => a + b,
        Op::Sub => a - b,
        Op::Mul => a * b,
        Op::Div => a / b,
    }))
}
fn fold(i: &mut Interp, args: &[Value], op: Op) -> EvalResult {
    if args.is_empty() {
        return Ok(Value::Int(if matches!(op, Op::Mul) { 1 } else { 0 }));
    }
    let mut result = number(i, &args[0])?;
    if args.len() == 1 {
        return Ok(if matches!(args[0], Value::Marker(_)) {
            result.value()
        } else {
            args[0].clone()
        });
    }
    for value in &args[1..] {
        let next = number(i, value)?;
        result = binary(i, result, next, op)?;
    }
    Ok(result.value())
}

pub(crate) static SUBRS: &[Subr] = &[
    S!("+", many 0, f_plus, "Return sum of any number of arguments."),
    S!("-", many 0, f_minus, "Negate number or subtract numbers."),
    S!("*", many 0, f_times, "Return product of any number of arguments."),
    S!("/", many 1, f_div, "Divide numbers (integer division if all ints)."),
    S!("%", 2, 2, f_mod, "Return remainder of X divided by Y."),
    S!(
        "mod",
        2,
        2,
        f_mod_fn,
        "Return X modulo Y (result has sign of Y)."
    ),
    S!("1+", 1, 1, f_1plus, "Return NUMBER plus one."),
    S!("1-", 1, 1, f_1minus, "Return NUMBER minus one."),
    S!("abs", 1, 1, f_abs, "Return absolute value of NUMBER."),
    S!("min", many 1, f_min, "Return smallest of arguments."),
    S!("max", many 1, f_max, "Return largest of arguments."),
    S!("=", many 1, f_numeq, "Return t if args, all numbers, are equal."),
    S!("/=", 2, 2, f_numne, "Return t if no two args are equal."),
    S!("<", many 1, f_lt, "Return t if args are increasing."),
    S!("<=", many 1, f_le, "Return t if args are non-decreasing."),
    S!(">", many 1, f_gt, "Return t if args are decreasing."),
    S!(">=", many 1, f_ge, "Return t if args are non-increasing."),
    S!("zerop", 1, 1, f_zerop, "Return t if NUMBER is zero."),
    S!(
        "natnump",
        1,
        1,
        f_natnump,
        "Return t if NUMBER is a nonnegative integer."
    ),
    S!(
        "wholenump",
        1,
        1,
        f_natnump,
        "Return t if NUMBER is a nonnegative integer."
    ),
    S!(
        "integerp",
        1,
        1,
        f_integerp,
        "Return t if OBJECT is an integer."
    ),
    S!(
        "numberp",
        1,
        1,
        f_numberp,
        "Return t if OBJECT is a number."
    ),
    S!("floatp", 1, 1, f_floatp, "Return t if OBJECT is a float."),
    S!(
        "number-or-marker-p",
        1,
        1,
        f_number_or_marker_p,
        "t if number or marker."
    ),
    S!(
        "truncate",
        1,
        2,
        f_truncate,
        "Truncate a number to an integer."
    ),
    S!("floor", 1, 2, f_floor, "Round toward negative infinity."),
    S!(
        "ceiling",
        1,
        2,
        f_ceiling,
        "Round toward positive infinity."
    ),
    S!("round", 1, 2, f_round, "Round to nearest integer."),
    S!("float", 1, 1, f_float, "Return NUMBER as a float."),
    S!("expt", 2, 2, f_expt, "Return X raised to power Y."),
    S!("sqrt", 1, 1, f_sqrt, "Return square root of NUMBER."),
    S!(
        "log",
        1,
        2,
        f_log,
        "Return logarithm of NUMBER (base B or e)."
    ),
    S!("exp", 1, 1, f_exp, "Return e raised to NUMBER."),
    S!("sin", 1, 1, f_sin, "Sine of NUMBER."),
    S!("cos", 1, 1, f_cos, "Cosine of NUMBER."),
    S!("tan", 1, 1, f_tan, "Tangent of NUMBER."),
    S!("asin", 1, 1, f_asin, "Arc sine of NUMBER."),
    S!("acos", 1, 1, f_acos, "Arc cosine of NUMBER."),
    S!("atan", 1, 2, f_atan, "Arc tangent of NUMBER (or Y/X)."),
    S!("logand", many 0, f_logand, "Bitwise AND of integers."),
    S!("logior", many 0, f_logior, "Bitwise inclusive OR."),
    S!("logxor", many 0, f_logxor, "Bitwise exclusive OR."),
    S!("lognot", 1, 1, f_lognot, "Bitwise NOT."),
    S!(
        "ash",
        2,
        2,
        f_ash,
        "Arithmetic shift VALUE COUNT bits left."
    ),
    S!("lsh", 2, 2, f_lsh, "Logical shift VALUE COUNT bits."),
    S!("random", 0, 1, f_random, "Random integer below LIMIT."),
    S!("eql", 2, 2, f_eql, "t if objects are eq or equal numbers."),
    S!("cl-plus", many 0, f_plus, ""),
    S!("cl-minus", many 1, f_minus, ""),
    S!("cl-times", many 0, f_times, ""),
    S!(
        "frexp",
        1,
        1,
        f_frexp,
        "Split float into fraction and exponent (FRAC . EXP)."
    ),
    S!("ldexp", 2, 2, f_ldexp, "SGNFCAND * 2**EXPONENT."),
    S!(
        "fround",
        1,
        1,
        f_fround,
        "Round X to nearest integral float."
    ),
    S!(
        "ftruncate",
        1,
        1,
        f_ftruncate,
        "Truncate X toward zero as a float."
    ),
    S!(
        "fceiling",
        1,
        1,
        f_fceiling,
        "Smallest integral float >= X."
    ),
    S!("ffloor", 1, 1, f_ffloor, "Largest integral float <= X."),
    S!("copysign", 2, 2, f_copysign, "X1 with the sign of X2."),
    S!("logb", 1, 1, f_logb, "Binary exponent of float X."),
    S!("isnan", 1, 1, f_isnan, "t if X is NaN."),
    S!("logcount", 1, 1, f_logcount, "Number of 1 bits in integer."),
    S!("plusp", 1, 1, f_plusp, "t if NUMBER is positive."),
    S!("minusp", 1, 1, f_minusp, "t if NUMBER is negative."),
    S!("oddp", 1, 1, f_oddp, "t if INTEGER is odd."),
    S!("evenp", 1, 1, f_evenp, "t if INTEGER is even."),
    S!("fixnump", 1, 1, f_fixnump, "t if OBJECT is a fixnum."),
    S!("bignump", 1, 1, f_bignump, "t if OBJECT is a bignum."),
    // `isqrt' does not exist in GNU Emacs 31; `cl-isqrt' is in cl-extra.
];

fn f_plus(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    fold(i, &args, Op::Add)
}
fn f_minus(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    if args.len() != 1 {
        return fold(i, &args, Op::Sub);
    }
    let n = number(i, &args[0])?;
    Ok(match n {
        Num::F(n) => Value::float(-n),
        n => binary(i, Num::I(0), n, Op::Sub)?.value(),
    })
}
fn f_times(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    fold(i, &args, Op::Mul)
}
fn f_div(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    let mut result = number(i, &args[0])?;
    if args.len() == 1 {
        return Ok(binary(i, Num::I(1), result, Op::Div)?.value());
    }
    // GNU restarts division in floating point when a later divisor is a float.
    if args[2..].iter().any(|n| matches!(n, Value::Float(_))) {
        result = Num::F(result.float());
    }
    for value in &args[1..] {
        let next = number(i, value)?;
        result = binary(i, result, next, Op::Div)?;
    }
    Ok(result.value())
}
fn f_mod(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    let x = integer(i, &args[0], "integer-or-marker-p", true)?;
    let y = integer(i, &args[1], "integer-or-marker-p", true)?;
    if y.is_zero() {
        return Err(arith_err(i, "Division by zero"));
    }
    Ok(Value::from_bigint(x % y))
}
fn f_mod_fn(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    let (x, y) = (number(i, &args[0])?, number(i, &args[1])?);
    if let (Some(x), Some(y)) = (x.integer(), y.integer()) {
        if y.is_zero() {
            return Err(arith_err(i, "Division by zero"));
        }
        return Ok(Value::from_bigint(x.mod_floor(&y)));
    }
    let (x, y) = (x.float(), y.float());
    let r = x.rem_euclid(y);
    Ok(Value::float(if y < 0.0 && r != 0.0 { r + y } else { r }))
}
fn f_1plus(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    let n = number(i, &args[0])?;
    Ok(binary(i, n, Num::I(1), Op::Add)?.value())
}
fn f_1minus(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    let n = number(i, &args[0])?;
    Ok(binary(i, n, Num::I(1), Op::Sub)?.value())
}
fn f_abs(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    if matches!(args[0], Value::Marker(_)) {
        return Err(i.wrong_type_mut("numberp", &args[0]));
    }
    let n = to_num(i, &args[0])?.ok_or_else(|| i.wrong_type_mut("numberp", &args[0]))?;
    match n {
        Num::F(n) if n.is_sign_negative() => Ok(Value::float(-n)),
        Num::F(_) => Ok(args[0].clone()),
        n => {
            let n = n.integer().unwrap();
            if n.is_negative() {
                Ok(Value::from_bigint(-n))
            } else {
                Ok(args[0].clone())
            }
        }
    }
}
fn minmax(i: &mut Interp, args: &[Value], lt: bool) -> EvalResult {
    fn winner(orig: &Value, n: &Num) -> Value {
        if matches!(orig, Value::Marker(_)) {
            n.clone().value()
        } else {
            orig.clone()
        }
    }
    let mut best_n = number(i, &args[0])?;
    let mut best = winner(&args[0], &best_n);
    for a in &args[1..] {
        let n = number(i, a)?;
        if matches!(&n, Num::F(f) if f.is_nan()) {
            return Ok(a.clone());
        }
        let wins = match num_ord(&n, &best_n) {
            Some(std::cmp::Ordering::Less) => lt,
            Some(std::cmp::Ordering::Greater) => !lt,
            _ => false,
        };
        if wins {
            best = winner(a, &n);
            best_n = n;
        }
    }
    Ok(best)
}
fn f_min(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    minmax(i, &args, true)
}
fn f_max(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    minmax(i, &args, false)
}

/// `=</<=/>/>=`: adjacent pairwise `arithcompare'.  Comparisons use the
/// exact int/float ordering of `num_ord` — coercing through f64 would
/// collapse fixnums whose magnitude exceeds 2**53.  A NaN operand makes
/// every comparison false.  GNU checks arguments lazily in pairs, so a
/// single argument is always t (no type check).
fn cmp_all(
    i: &mut Interp,
    args: &[Value],
    pred: fn(Option<std::cmp::Ordering>) -> bool,
) -> EvalResult {
    for w in args.windows(2) {
        let a = match to_num(i, &w[0])? {
            Some(n) => n,
            None => return Err(i.wrong_type_mut("number-or-marker-p", &w[0])),
        };
        let b = match to_num(i, &w[1])? {
            Some(n) => n,
            None => return Err(i.wrong_type_mut("number-or-marker-p", &w[1])),
        };
        if !pred(num_ord(&a, &b)) {
            return Ok(Value::Nil);
        }
    }
    Ok(Value::t())
}

fn f_numeq(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    cmp_all(i, &args, |o| o == Some(std::cmp::Ordering::Equal))
}
fn f_numne(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    // `/=' takes exactly two args (enforced by arity); t iff they differ.
    let a = match to_num(i, &args[0])? {
        Some(n) => n,
        None => return Err(i.wrong_type_mut("number-or-marker-p", &args[0])),
    };
    let b = match to_num(i, &args[1])? {
        Some(n) => n,
        None => return Err(i.wrong_type_mut("number-or-marker-p", &args[1])),
    };
    Ok(Value::from_bool(
        num_ord(&a, &b) != Some(std::cmp::Ordering::Equal),
    ))
}
fn f_lt(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    cmp_all(i, &args, |o| o == Some(std::cmp::Ordering::Less))
}
fn f_le(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    cmp_all(i, &args, |o| {
        matches!(
            o,
            Some(std::cmp::Ordering::Less) | Some(std::cmp::Ordering::Equal)
        )
    })
}
pub(crate) fn f_gt(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    cmp_all(i, &args, |o| o == Some(std::cmp::Ordering::Greater))
}
fn f_ge(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    cmp_all(i, &args, |o| {
        matches!(
            o,
            Some(std::cmp::Ordering::Greater) | Some(std::cmp::Ordering::Equal)
        )
    })
}

fn f_zerop(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    let n = want_num(i, &args[0])?;
    Ok(Value::from_bool(n == 0.0))
}

fn f_natnump(_i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    Ok(Value::from_bool(
        matches!(&args[0], Value::Int(n) if *n >= 0)
            || matches!(&args[0], Value::BigInt(n) if !n.is_negative()),
    ))
}

fn f_integerp(_i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    Ok(Value::from_bool(args[0].integerp()))
}
fn f_numberp(_i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    Ok(Value::from_bool(matches!(
        &args[0],
        Value::Int(_) | Value::BigInt(_) | Value::Float(_)
    )))
}
fn f_floatp(_i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    Ok(Value::from_bool(matches!(&args[0], Value::Float(_))))
}
fn f_number_or_marker_p(_i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    Ok(Value::from_bool(matches!(
        &args[0],
        Value::Int(_) | Value::BigInt(_) | Value::Float(_) | Value::Marker(_)
    )))
}

/// Rounding: f(x) or f(x/y) with optional divisor. Always returns an
/// integer (possibly a bignum) in Emacs, even for float inputs.
fn round_with(i: &mut Interp, args: &[Value], mode: u8) -> EvalResult {
    let mut nums = Vec::with_capacity(args.len());
    for value in args {
        if value.is_nil() && !nums.is_empty() {
            break;
        }
        if matches!(value, Value::Marker(_)) {
            return Err(i.wrong_type_mut("numberp", value));
        }
        nums.push(to_num(i, value)?.ok_or_else(|| i.wrong_type_mut("numberp", value))?);
    }
    let x = &nums[0];
    let y = nums.get(1);
    if let Some(a) = x.integer() {
        if y.is_none() {
            return Ok(args[0].clone());
        }
        if let Some(b) = y.and_then(Num::integer) {
            if b.is_zero() {
                return Err(arith_err(i, "Division by zero"));
            }
            let (mut q, r) = a.div_rem(&b);
            match mode {
                1 if !r.is_zero() && r.sign() != b.sign() => q -= 1,
                2 if !r.is_zero() && r.sign() == b.sign() => q += 1,
                3 => {
                    let twice = r.abs() << 1usize;
                    let magnitude = b.abs();
                    if twice > magnitude || (twice == magnitude && q.is_odd()) {
                        if r.sign() == b.sign() {
                            q += 1;
                        } else {
                            q -= 1;
                        }
                    }
                }
                _ => {}
            }
            return Ok(Value::from_bigint(q));
        }
    }
    let divisor = y.map_or(1.0, Num::float);
    if divisor == 0.0 {
        return Err(arith_err(i, "Division by zero"));
    }
    let q = x.float() / divisor;
    let rounded = match mode {
        0 => q.trunc(),
        1 => q.floor(),
        2 => q.ceil(),
        _ => q.round_ties_even(),
    };
    BigInt::from_f64(rounded)
        .map(Value::from_bigint)
        .ok_or_else(|| {
            let condition = i.q("overflow-error");
            i.signal_data(condition, vec![])
        })
}

fn f_truncate(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    round_with(i, &args, 0)
}
fn f_floor(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    round_with(i, &args, 1)
}
fn f_ceiling(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    round_with(i, &args, 2)
}
fn f_round(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    round_with(i, &args, 3)
}

/// Like `want_num` but signals `numberp' — GNU's pure-math functions
/// (float, sqrt, expt, ...) reject markers with that predicate.
fn want_number(i: &mut Interp, v: &Value) -> Result<f64, Flow> {
    match v {
        Value::Int(n) => Ok(*n as f64),
        Value::BigInt(n) => Ok(n.to_f64().unwrap_or(if n.is_negative() {
            f64::NEG_INFINITY
        } else {
            f64::INFINITY
        })),
        Value::Float(f) => Ok(**f),
        _ => Err(i.wrong_type_mut("numberp", v)),
    }
}

/// Integer or marker → position.  GNU's bitwise/oddp functions use
/// `integer-or-marker-p' as their error predicate.
pub(crate) fn want_int_or_marker(i: &mut Interp, v: &Value) -> Result<i128, Flow> {
    match v {
        Value::Int(n) => Ok(*n),
        Value::BigInt(n) => n.to_i128().ok_or_else(|| {
            let condition = i.q("args-out-of-range");
            i.signal_data(condition, vec![v.clone()])
        }),
        Value::Marker(m) => marker_pos_err(i, m),
        _ => Err(i.wrong_type_mut("integer-or-marker-p", v)),
    }
}

fn f_float(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    if matches!(args[0], Value::Float(_)) {
        return Ok(args[0].clone());
    }
    Ok(Value::float(want_number(i, &args[0])?))
}

fn f_expt(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    let mut values = Vec::new();
    for value in args.iter() {
        if matches!(value, Value::Marker(_)) {
            return Err(i.wrong_type_mut("numberp", value));
        }
        values.push(to_num(i, value)?.ok_or_else(|| i.wrong_type_mut("numberp", value))?);
    }
    if let (Some(x), Some(y)) = (values[0].integer(), values[1].integer()) {
        if !y.is_negative() {
            if y.is_zero() {
                return Ok(Value::Int(1));
            }
            if x.is_zero() {
                return Ok(Value::Int(0));
            }
            if x == BigInt::from(1) {
                return Ok(Value::Int(1));
            }
            if x == BigInt::from(-1) {
                return Ok(Value::Int(if y.is_odd() { -1 } else { 1 }));
            }
            // GNU emacs_mpz_pow_ui limits estimated 64-bit limb counts.
            let limbs = x.bits().div_ceil(64);
            let Some(power) = y.to_u32().filter(|power| {
                limbs
                    .checked_mul(u64::from(*power))
                    .is_some_and(|n| n <= i32::MAX as u64 - 5)
            }) else {
                return Err(overflow_err(i, &args[1]));
            };
            return Ok(Value::from_bigint(x.pow(power)));
        }
    }
    Ok(Value::float(values[0].float().powf(values[1].float())))
}

fn f_sqrt(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    Ok(Value::float(want_number(i, &args[0])?.sqrt()))
}
fn f_exp(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    Ok(Value::float(want_number(i, &args[0])?.exp()))
}
fn f_sin(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    Ok(Value::float(want_number(i, &args[0])?.sin()))
}
fn f_cos(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    Ok(Value::float(want_number(i, &args[0])?.cos()))
}
fn f_tan(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    Ok(Value::float(want_number(i, &args[0])?.tan()))
}
fn f_asin(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    Ok(Value::float(want_number(i, &args[0])?.asin()))
}
fn f_acos(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    Ok(Value::float(want_number(i, &args[0])?.acos()))
}
fn f_atan(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    let y = want_number(i, &args[0])?;
    match args.get(1) {
        Some(x) => Ok(Value::float(y.atan2(want_number(i, x)?))),
        None => Ok(Value::float(y.atan())),
    }
}
fn f_log(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    let n = want_number(i, &args[0])?;
    match args.get(1) {
        Some(b) => {
            let b = want_number(i, b)?;
            // GNU's Flog calls log10() directly for base 10, so
            // (log 1000 10) yields exactly 3.0.
            Ok(Value::float(if b == 10.0 { n.log10() } else { n.log(b) }))
        }
        None => Ok(Value::float(n.ln())),
    }
}

// GNU arith_driver for the A_AND/A_OR/A_XOR ops: arg0 is
// CHECK_INTEGER (integer-or-marker-p); args≥1 are
// CHECK_NUMBER_OR_MARKER_COERCE (number-or-marker-p) followed by
// CHECK_INTEGER for surviving non-int non-marker values (floats →
// integer-or-marker-p).
fn logcoerce(i: &mut Interp, value: &Value, first: bool) -> Result<BigInt, Flow> {
    if let Some(n) = value.as_bigint() {
        return Ok(n);
    }
    if let Value::Marker(m) = value {
        if m.borrow().buffer.is_some() {
            return Ok(BigInt::from(m.borrow().position as i128 + 1));
        }
    }
    if first || matches!(value, Value::Float(_) | Value::Marker(_)) {
        Err(i.wrong_type_mut("integer-or-marker-p", value))
    } else {
        Err(i.wrong_type_mut("number-or-marker-p", value))
    }
}
fn bitwise(i: &mut Interp, args: &[Value], operation: u8) -> EvalResult {
    if args.len() == 1 {
        if args[0].integerp() {
            return Ok(args[0].clone());
        }
        return Ok(Value::from_bigint(logcoerce(i, &args[0], true)?));
    }
    let mut small = if operation == 0 { -1i128 } else { 0 };
    let mut large: Option<BigInt> = None;
    for (index, value) in args.iter().enumerate() {
        let next = match value {
            Value::Int(n) => Some(*n),
            Value::BigInt(n) => n.to_i128(),
            Value::Marker(m) if m.borrow().buffer.is_some() => {
                Some(m.borrow().position as i128 + 1)
            }
            _ => None,
        };
        if let (Some(next), None) = (next, &large) {
            small = match operation {
                0 => small & next,
                1 => small | next,
                _ => small ^ next,
            };
            continue;
        }
        let next = logcoerce(i, value, index == 0)?;
        let accumulator = large.get_or_insert_with(|| BigInt::from(small));
        match operation {
            0 => *accumulator &= next,
            1 => *accumulator |= next,
            _ => *accumulator ^= next,
        }
    }
    Ok(large
        .map(Value::from_bigint)
        .unwrap_or_else(|| Value::integer(small)))
}
fn f_logand(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    bitwise(i, &args, 0)
}
fn f_logior(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    bitwise(i, &args, 1)
}
pub(crate) fn f_logxor(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    bitwise(i, &args, 2)
}
fn f_lognot(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    if let Value::Int(n) = args[0] {
        return Ok(Value::integer(!n));
    }
    Ok(Value::from_bigint(!integer(
        i, &args[0], "integerp", false,
    )?))
}

fn shift(i: &mut Interp, n: BigInt, count: BigInt) -> EvalResult {
    if n.is_zero() {
        return Ok(Value::Int(0));
    }
    if count.is_negative() {
        let right = -count;
        if right >= BigInt::from(n.bits()) {
            return Ok(Value::Int(if n.is_negative() { -1 } else { 0 }));
        }
        return Ok(Value::from_bigint(n >> right.to_usize().unwrap()));
    }
    let Some(left) = count.to_usize().filter(|bits| {
        n.bits().div_ceil(64).saturating_add((*bits / 64) as u64) <= i32::MAX as u64 - 1
    }) else {
        return Err(overflow_err(i, &Value::from_bigint(count)));
    };
    Ok(Value::from_bigint(n << left))
}
fn f_ash(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    let n = integer(i, &args[0], "integerp", false)?;
    let count = integer(i, &args[1], "integerp", false)?;
    shift(i, n, count)
}
fn f_lsh(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    let n = match &args[0] {
        Value::Int(_) | Value::BigInt(_) => args[0].as_bigint().unwrap(),
        Value::Marker(m) => {
            marker_pos_err(i, m)?;
            return Err(i.wrong_type_mut("integerp", &args[0]));
        }
        Value::Float(_) => return Err(i.wrong_type_mut("integerp", &args[0])),
        other => return Err(i.wrong_type_mut("number-or-marker-p", other)),
    };
    let count = integer(i, &args[1], "integerp", false)?;
    if args[0].fixnump() && count.is_negative() {
        if count <= BigInt::from(-62) {
            return Ok(Value::Int(0));
        }
        let value =
            (args[0].int().unwrap() as u64 & 0x3fff_ffff_ffff_ffff) >> (-count).to_u32().unwrap();
        return Ok(Value::integer(value as i128));
    }
    shift(i, n, count)
}
// GNU `random' = Gnulib BSD `random()' (TYPE_3: degree 31, sep 3,
// Schrage-seeded table, 310-step warm-up) threaded through sysdep.c's
// get_random bit-assembly.  State is per-interp thread like GNU's
// global random_state.
struct Rand3 {
    state: [i32; 31],
    f: usize,
    r: usize,
}

impl Rand3 {
    fn seeded(seed: u32) -> Self {
        let mut st = [0i32; 31];
        st[0] = if seed == 0 { 1 } else { seed as i32 };
        for i in 1..31 {
            let hi = st[i - 1] / 127773;
            let lo = st[i - 1] % 127773;
            let w = 16807i64 * lo as i64 - 2836i64 * hi as i64;
            st[i] = (if w < 0 { w + 2147483647 } else { w }) as i32;
        }
        let mut g = Rand3 {
            state: st,
            f: 3,
            r: 0,
        };
        for _ in 0..310 {
            g.step();
        }
        g
    }
    fn step(&mut self) {
        let t = self.state[self.f].wrapping_add(self.state[self.r]);
        self.state[self.f] = t;
        self.f = (self.f + 1) % 31;
        self.r = (self.r + 1) % 31;
    }
    fn next(&mut self) -> u64 {
        self.step();
        // BSD random(): the updated *fptr value, shifted down.
        let prev = (self.f + 30) % 31;
        ((self.state[prev] as u32) >> 1) as u64
    }
}

thread_local! {
    static GEN: RefCell<Rand3> = RefCell::new(Rand3::seeded(1));
}

/// sysdep.c `get_random': FIXNUM_BITS=62, RAND_BITS=31 → two BSD
/// random() draws folded together.  Shared with
/// `generate-new-buffer-name' (buffer.c calls get_random for
/// space-prefixed names) so its `-N' suffixes come from the same
/// seeded stream as `random'.
pub(crate) fn get_random() -> i64 {
    GEN.with(|g| {
        let mut g = g.borrow_mut();
        let mut val: u64 = 0;
        for _ in 0..2 {
            val = g.next() ^ (val << 31) ^ (val >> 33);
        }
        val ^= val >> 2;
        (val & 0x3fff_ffff_ffff_ffff) as i64
    })
}

fn random_word(draws: usize) -> u64 {
    GEN.with(|generator| {
        let mut generator = generator.borrow_mut();
        let mut value = 0u64;
        for _ in 0..draws {
            value = generator.next() ^ value.wrapping_shl(31) ^ (value >> 33);
        }
        value
    })
}
fn random_fixnum_bits() -> u64 {
    let value = random_word(2);
    (value ^ (value >> 2)) & 0x3fff_ffff_ffff_ffff
}
fn random_limb(limit: u64) -> u64 {
    let difference_limit = 0u64.wrapping_sub(limit);
    loop {
        let value = random_word(3);
        if limit == 0 {
            return value;
        }
        let remainder = value % limit;
        if value - remainder <= difference_limit {
            return remainder;
        }
    }
}
fn random_bignum(limit: &BigInt) -> Value {
    let limbs = limit.to_u64_digits().1;
    let last = limbs.len() - 1;
    loop {
        let high = random_limb(limbs[last].wrapping_add(u64::from(last > 0)));
        let mut edge = high == limbs[last];
        let mut result = vec![0; limbs.len()];
        result[last] = high;
        let mut rejected = false;
        for index in (0..last).rev() {
            let value = random_limb(0);
            if edge {
                if value > limbs[index] {
                    rejected = true;
                    break;
                }
                edge = value == limbs[index];
            }
            result[index] = value;
        }
        if rejected || edge {
            continue;
        }
        let bytes: Vec<u8> = result.into_iter().flat_map(u64::to_le_bytes).collect();
        return Value::from_bigint(BigInt::from_bytes_le(num_bigint::Sign::Plus, &bytes));
    }
}
fn f_random(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    let t_sym = i.q_soft("t").unwrap_or(u32::MAX);
    match args.get(0) {
        Some(Value::Sym(s)) if *s == t_sym => {
            let time = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.subsec_nanos() as u64 ^ (d.as_secs() << 16))
                .unwrap_or(0xabcdef);
            GEN.with(|g| *g.borrow_mut() = Rand3::seeded(time as u32));
        }
        Some(Value::Str(s)) => {
            let mut seed = 0u32;
            for (index, byte) in s.borrow().as_bytes().iter().enumerate() {
                seed ^= u32::from(*byte) << ((index % 4) * 8);
            }
            GEN.with(|g| *g.borrow_mut() = Rand3::seeded(seed));
        }
        Some(value) if value.integerp() => {
            let limit = value.as_bigint().unwrap();
            if limit <= BigInt::from(0) {
                let condition = i.q("args-out-of-range");
                return Err(i.signal_data(condition, vec![value.clone()]));
            }
            if !value.fixnump() {
                return Ok(random_bignum(&limit));
            }
            let limit = limit.to_u64().unwrap();
            let difference_limit = 0x4000_0000_0000_0000 - limit;
            loop {
                let value = random_fixnum_bits();
                let remainder = value % limit;
                if value - remainder <= difference_limit {
                    return Ok(Value::Int(remainder as i128));
                }
            }
        }
        _ => {}
    }
    let value = random_fixnum_bits();
    let signed = if value > 0x1fff_ffff_ffff_ffff {
        value as i128 - 0x4000_0000_0000_0000
    } else {
        value as i128
    };
    Ok(Value::Int(signed))
}

fn f_eql(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    let _ = i;
    Ok(Value::from_bool(super::eql_values(&args[0], &args[1])))
}

fn f_frexp(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    let x = want_number(i, &args[0])?;
    if x == 0.0 || x.is_nan() || x.is_infinite() {
        return Ok(Value::cons(Value::float(x), Value::Int(0)));
    }
    let e = x.abs().log2().floor() as i128 + 1;
    let m = x / 2f64.powi(e as i32);
    Ok(Value::cons(Value::float(m), Value::Int(e)))
}

fn f_ldexp(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    // GNU checks the exponent first and saturates it to the C int range.
    let e = match &args[1] {
        Value::Int(n) => {
            (*n).clamp(libc::c_int::MIN as i128, libc::c_int::MAX as i128) as libc::c_int
        }
        other => return Err(i.wrong_type_mut("fixnump", other)),
    };
    let x = want_number(i, &args[0])?;
    #[cfg_attr(unix, link(name = "m"))]
    unsafe extern "C" {
        fn ldexp(x: f64, exponent: libc::c_int) -> f64;
    }
    // Multiplying by powi would round 2^-1074 to zero before scaling X,
    // and can turn zero/infinity inputs into NaNs at extreme exponents.
    Ok(Value::float(unsafe { ldexp(x, e) }))
}

/// GNU's ffloor/fceiling/fround/ftruncate require a FLOAT argument.
fn want_float(i: &mut Interp, v: &Value) -> Result<f64, Flow> {
    match v {
        Value::Float(x) => Ok(**x),
        other => Err(i.wrong_type_mut("floatp", other)),
    }
}

fn f_fround(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    let x = want_float(i, &args[0])?;
    Ok(Value::float(x.round_ties_even()))
}

fn f_ftruncate(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    let x = want_float(i, &args[0])?;
    Ok(Value::float(x.trunc()))
}

fn f_fceiling(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    let x = want_float(i, &args[0])?;
    Ok(Value::float(x.ceil()))
}

fn f_ffloor(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    let x = want_float(i, &args[0])?;
    Ok(Value::float(x.floor()))
}

fn f_copysign(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    // Emacs requires both args to be floats.
    let (x, y) = match (&args[0], &args[1]) {
        (Value::Float(a), Value::Float(b)) => (**a, **b),
        (Value::Float(_), other) | (other, _) => return Err(i.wrong_type_mut("floatp", other)),
    };
    Ok(Value::float(x.copysign(y)))
}

fn f_logb(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    if let Value::BigInt(n) = &args[0] {
        return Ok(Value::integer(n.bits() as i128 - 1));
    }
    if let Value::Int(n) = &args[0] {
        if *n == 0 {
            return Ok(Value::float(f64::NEG_INFINITY));
        }
        // A float conversion can round an integer below a power of two
        // up to that power.  Work on its magnitude directly, including
        // i128::MIN, whose absolute value does not fit in a signed i128.
        return Ok(Value::Int(n.unsigned_abs().ilog2() as i128));
    }
    let x = want_number(i, &args[0])?;
    if x == 0.0 {
        return Ok(Value::float(f64::NEG_INFINITY));
    }
    if x.is_nan() {
        return Ok(Value::float(f64::NAN));
    }
    if x.is_infinite() {
        return Ok(Value::float(f64::INFINITY));
    }
    Ok(Value::Int(x.abs().log2().floor() as i128))
}

fn f_isnan(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    let x = want_float(i, &args[0])?;
    Ok(Value::from_bool(x.is_nan()))
}

fn f_logcount(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    let mut n = integer(i, &args[0], "integerp", false)?;
    if n.is_negative() {
        n = !n;
    }
    let count: u64 = n
        .to_u64_digits()
        .1
        .iter()
        .map(|word| u64::from(word.count_ones()))
        .sum();
    Ok(Value::integer(count as i128))
}
fn f_plusp(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    Ok(Value::from_bool(want_num(i, &args[0])? > 0.0))
}
fn f_minusp(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    Ok(Value::from_bool(want_num(i, &args[0])? < 0.0))
}
fn f_oddp(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    Ok(Value::from_bool(
        integer(i, &args[0], "integer-or-marker-p", true)?.is_odd(),
    ))
}
fn f_evenp(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    Ok(Value::from_bool(
        integer(i, &args[0], "integer-or-marker-p", true)?.is_even(),
    ))
}

fn f_fixnump(_i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    Ok(Value::from_bool(args[0].fixnump()))
}
fn f_bignump(_i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    Ok(Value::from_bool(args[0].integerp() && !args[0].fixnump()))
}
