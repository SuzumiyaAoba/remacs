//! Arithmetic and numeric comparison subrs.

use super::{S, arg, want_int, want_num};
use crate::lisp::Interp;
use crate::lisp::error::{EvalResult, Flow};
use crate::lisp::obarray::sym;
use crate::lisp::value::{Subr, Value};

fn arith_err(i: &Interp, msg: &str) -> Flow {
    i.signal_data(sym::ARITH_ERROR, vec![Value::string(msg)])
}

fn overflow_err(i: &mut Interp, v: &Value) -> Flow {
    let s = i.intern("overflow-error");
    i.signal_data(s, vec![v.clone()])
}

/// Number coercion: int if all-int else float.
enum Num {
    I(i128),
    F(f64),
}

fn to_num(v: &Value) -> Option<Num> {
    match v {
        Value::Int(n) => Some(Num::I(*n)),
        Value::Float(f) => Some(Num::F(*f)),
        _ => None,
    }
}

pub(crate) static SUBRS: &[Subr] = &[
    S!("+", many 0, f_plus, "Return sum of any number of arguments."),
    S!("-", many 1, f_minus, "Negate number or subtract numbers."),
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
    S!("/=", many 1, f_numne, "Return t if no two args are equal."),
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
    S!("ldexp", 1, 2, f_ldexp, "SGNFCAND * 2**EXPONENT."),
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
    S!("isqrt", 1, 1, f_isqrt, "Integer square root."),
];

fn f_plus(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let mut acc_i: i128 = 0;
    let mut acc_f: f64 = 0.0;
    let mut is_float = false;
    for a in &args {
        match to_num(a) {
            Some(Num::I(n)) => {
                if is_float {
                    acc_f += n as f64;
                } else {
                    acc_i = acc_i.checked_add(n).ok_or_else(|| overflow_err(i, a))?;
                }
            }
            Some(Num::F(f)) => {
                if !is_float {
                    acc_f = acc_i as f64;
                    is_float = true;
                }
                acc_f += f;
            }
            None => return Err(i.wrong_type_mut("number-or-marker-p", a)),
        }
    }
    Ok(if is_float {
        Value::Float(acc_f)
    } else {
        Value::Int(acc_i)
    })
}

fn f_minus(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    if args.len() == 1 {
        return match to_num(&args[0]) {
            Some(Num::I(n)) => n
                .checked_neg()
                .map(Value::Int)
                .ok_or_else(|| overflow_err(i, &args[0])),
            Some(Num::F(f)) => Ok(Value::Float(-f)),
            None => Err(i.wrong_type_mut("number-or-marker-p", &args[0])),
        };
    }
    let mut it = args.iter();
    let first = to_num(it.next().unwrap());
    let (mut acc_i, mut acc_f, mut is_float) = match first {
        Some(Num::I(n)) => (n, 0.0, false),
        Some(Num::F(f)) => (0, f, true),
        None => return Err(i.wrong_type_mut("number-or-marker-p", &args[0])),
    };
    for a in it {
        match to_num(a) {
            Some(Num::I(n)) => {
                if is_float {
                    acc_f -= n as f64;
                } else {
                    acc_i = acc_i.checked_sub(n).ok_or_else(|| overflow_err(i, a))?;
                }
            }
            Some(Num::F(f)) => {
                if !is_float {
                    acc_f = acc_i as f64;
                    is_float = true;
                }
                acc_f -= f;
            }
            None => return Err(i.wrong_type_mut("number-or-marker-p", a)),
        }
    }
    Ok(if is_float {
        Value::Float(acc_f)
    } else {
        Value::Int(acc_i)
    })
}

fn f_times(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let mut acc_i: i128 = 1;
    let mut acc_f: f64 = 1.0;
    let mut is_float = false;
    for a in &args {
        match to_num(a) {
            Some(Num::I(n)) => {
                if is_float {
                    acc_f *= n as f64;
                } else {
                    acc_i = acc_i.checked_mul(n).ok_or_else(|| overflow_err(i, a))?;
                }
            }
            Some(Num::F(f)) => {
                if !is_float {
                    acc_f = acc_i as f64;
                    is_float = true;
                }
                acc_f *= f;
            }
            None => return Err(i.wrong_type_mut("number-or-marker-p", a)),
        }
    }
    Ok(if is_float {
        Value::Float(acc_f)
    } else {
        Value::Int(acc_i)
    })
}

fn f_div(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    if args.len() == 1 {
        // (/ x) = 1/x
        return match to_num(&args[0]) {
            Some(Num::I(n)) => {
                if n == 0 {
                    Err(arith_err(i, "Division by zero"))
                } else {
                    Ok(Value::Float(1.0 / n as f64))
                }
            }
            Some(Num::F(f)) => Ok(Value::Float(1.0 / f)),
            None => Err(i.wrong_type_mut("number-or-marker-p", &args[0])),
        };
    }
    let first = to_num(&args[0]);
    let (mut acc_i, mut acc_f, mut is_float) = match first {
        Some(Num::I(n)) => (n, 0.0, false),
        Some(Num::F(f)) => (0, f, true),
        None => return Err(i.wrong_type_mut("number-or-marker-p", &args[0])),
    };
    for a in &args[1..] {
        match to_num(a) {
            Some(Num::I(n)) => {
                if is_float {
                    acc_f /= n as f64;
                } else {
                    acc_i = acc_i
                        .checked_div(n)
                        .ok_or_else(|| arith_err(i, "Division by zero"))?;
                }
            }
            Some(Num::F(f)) => {
                if !is_float {
                    acc_f = acc_i as f64;
                    is_float = true;
                }
                acc_f /= f;
            }
            None => return Err(i.wrong_type_mut("number-or-marker-p", a)),
        }
    }
    Ok(if is_float {
        Value::Float(acc_f)
    } else {
        Value::Int(acc_i)
    })
}

/// `%` — integer remainder only (floats are a type error in Emacs).
fn f_mod(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    match (&args[0], &args[1]) {
        (Value::Int(x), Value::Int(y)) => {
            if *y == 0 {
                Err(arith_err(i, "Division by zero"))
            } else {
                x.checked_rem(*y)
                    .map(Value::Int)
                    .ok_or_else(|| arith_err(i, "Division by zero"))
            }
        }
        (Value::Int(_), other) | (other, Value::Int(_)) | (other, _) => {
            Err(i.wrong_type_mut("integer-or-marker-p", other))
        }
    }
}

/// `mod` — floored remainder (sign of divisor). For ints only in Emacs.
fn f_mod_fn(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    match (to_num(&args[0]), to_num(&args[1])) {
        (Some(Num::I(x)), Some(Num::I(y))) => {
            if y == 0 {
                Err(arith_err(i, "Division by zero"))
            } else {
                // GNU's mod takes the sign of the divisor:
                // rem_euclid is non-negative, so adjust when y < 0.
                let r = x.rem_euclid(y);
                Ok(Value::Int(if y < 0 && r != 0 { r + y } else { r }))
            }
        }
        (Some(x), Some(y)) => {
            let (xf, yf) = match (x, y) {
                (Num::I(a), Num::I(b)) => (a as f64, b as f64),
                (Num::I(a), Num::F(b)) => (a as f64, b),
                (Num::F(a), Num::I(b)) => (a, b as f64),
                (Num::F(a), Num::F(b)) => (a, b),
            };
            // GNU computes fmod directly: a zero float divisor yields
            // NaN, and the result takes the divisor's sign.
            let r = xf.rem_euclid(yf);
            Ok(Value::Float(if yf < 0.0 && r != 0.0 { r + yf } else { r }))
        }
        _ => Err(i.wrong_type_mut("number-or-marker-p", &args[0])),
    }
}

fn f_1plus(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    match to_num(&args[0]) {
        Some(Num::I(n)) => n
            .checked_add(1)
            .map(Value::Int)
            .ok_or_else(|| overflow_err(i, &args[0])),
        Some(Num::F(f)) => Ok(Value::Float(f + 1.0)),
        None => Err(i.wrong_type_mut("number-or-marker-p", &args[0])),
    }
}

fn f_1minus(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    match to_num(&args[0]) {
        Some(Num::I(n)) => n
            .checked_sub(1)
            .map(Value::Int)
            .ok_or_else(|| overflow_err(i, &args[0])),
        Some(Num::F(f)) => Ok(Value::Float(f - 1.0)),
        None => Err(i.wrong_type_mut("number-or-marker-p", &args[0])),
    }
}

fn f_abs(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    match to_num(&args[0]) {
        Some(Num::I(n)) => n
            .checked_abs()
            .map(Value::Int)
            .ok_or_else(|| overflow_err(i, &args[0])),
        Some(Num::F(f)) => Ok(Value::Float(f.abs())),
        None => Err(i.wrong_type_mut("number-or-marker-p", &args[0])),
    }
}

fn f_min(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let mut best = arg(&args, 0);
    for a in &args[1..] {
        if to_num(a).is_none() {
            return Err(i.wrong_type_mut("number-or-marker-p", a));
        }
        let (af, bf) = (want_num(i, a)?, want_num(i, &best)?);
        if af < bf {
            best = a.clone();
        }
    }
    Ok(best)
}

fn f_max(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let mut best = arg(&args, 0);
    for a in &args[1..] {
        if to_num(a).is_none() {
            return Err(i.wrong_type_mut("number-or-marker-p", a));
        }
        let (af, bf) = (want_num(i, a)?, want_num(i, &best)?);
        if af > bf {
            best = a.clone();
        }
    }
    Ok(best)
}

fn cmp_all(i: &mut Interp, args: &[Value], pred: fn(f64, f64) -> bool) -> EvalResult {
    if args.len() == 1 {
        if to_num(&args[0]).is_none() {
            return Err(i.wrong_type_mut("number-or-marker-p", &args[0]));
        }
        return Ok(Value::t());
    }
    for w in args.windows(2) {
        let a = want_num(i, &w[0])?;
        let b = want_num(i, &w[1])?;
        if !pred(a, b) {
            return Ok(Value::Nil);
        }
    }
    Ok(Value::t())
}

fn f_numeq(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    cmp_all(i, &args, |a, b| a == b)
}
fn f_numne(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    // /= : all distinct
    for x in 0..args.len() {
        for y in (x + 1)..args.len() {
            let a = want_num(i, &args[x])?;
            let b = want_num(i, &args[y])?;
            if a == b {
                return Ok(Value::Nil);
            }
        }
    }
    Ok(Value::t())
}
fn f_lt(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    cmp_all(i, &args, |a, b| a < b)
}
fn f_le(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    cmp_all(i, &args, |a, b| a <= b)
}
fn f_gt(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    cmp_all(i, &args, |a, b| a > b)
}
fn f_ge(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    cmp_all(i, &args, |a, b| a >= b)
}

fn f_zerop(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let n = want_num(i, &args[0])?;
    Ok(Value::from_bool(n == 0.0))
}

fn f_natnump(_i: &mut Interp, args: Vec<Value>) -> EvalResult {
    Ok(Value::from_bool(
        matches!(&args[0], Value::Int(n) if *n >= 0),
    ))
}

fn f_integerp(_i: &mut Interp, args: Vec<Value>) -> EvalResult {
    Ok(Value::from_bool(matches!(&args[0], Value::Int(_))))
}
fn f_numberp(_i: &mut Interp, args: Vec<Value>) -> EvalResult {
    Ok(Value::from_bool(matches!(
        &args[0],
        Value::Int(_) | Value::Float(_)
    )))
}
fn f_floatp(_i: &mut Interp, args: Vec<Value>) -> EvalResult {
    Ok(Value::from_bool(matches!(&args[0], Value::Float(_))))
}
fn f_number_or_marker_p(_i: &mut Interp, args: Vec<Value>) -> EvalResult {
    Ok(Value::from_bool(matches!(
        &args[0],
        Value::Int(_) | Value::Float(_) | Value::Marker(_)
    )))
}

/// Rounding: f(x) or f(x/y) with optional divisor. Always returns an
/// integer (possibly a bignum) in Emacs, even for float inputs.
fn round_with(i: &mut Interp, args: &[Value], mode: u8) -> EvalResult {
    let (x, y) = (to_num(&args[0]), args.get(1).map(to_num).flatten());
    let (xf, yf) = match (x, y) {
        (Some(Num::I(a)), None) => (a as f64, 1.0),
        (Some(Num::I(a)), Some(Num::I(b))) => (a as f64, b as f64),
        (Some(Num::I(a)), Some(Num::F(b))) => (a as f64, b),
        (Some(Num::F(a)), None) => (a, 1.0),
        (Some(Num::F(a)), Some(Num::I(b))) => (a, b as f64),
        (Some(Num::F(a)), Some(Num::F(b))) => (a, b),
        _ => return Err(i.wrong_type_mut("number-or-marker-p", &args[0])),
    };
    if yf == 0.0 {
        return Err(arith_err(i, "Division by zero"));
    }
    let q = xf / yf;
    if q.is_nan() {
        let s = i.intern("domain-error");
        return Err(i.signal_data(s, vec![Value::string("NaN"), Value::Float(q)]));
    }
    let r = match mode {
        0 => q.trunc(),
        1 => q.floor(),
        2 => q.ceil(),
        _ => q.round_ties_even(),
    };
    Ok(Value::Int(r as i128))
}

fn f_truncate(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    round_with(i, &args, 0)
}
fn f_floor(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    round_with(i, &args, 1)
}
fn f_ceiling(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    round_with(i, &args, 2)
}
fn f_round(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    round_with(i, &args, 3)
}

fn f_float(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    Ok(Value::Float(want_num(i, &args[0])?))
}

fn f_expt(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    match (to_num(&args[0]), to_num(&args[1])) {
        (Some(Num::I(x)), Some(Num::I(y))) => {
            if y >= 0 {
                match x.checked_pow(y.min(u32::MAX as i128) as u32) {
                    Some(r) => return Ok(Value::Int(r)),
                    // Beyond i128 — fall back to float (true bignum
                    // would still be an integer; documented deviation).
                    None => return Ok(Value::Float((x as f64).powf(y as f64))),
                }
            }
            Ok(Value::Float((x as f64).powf(y as f64)))
        }
        (Some(x), Some(y)) => {
            let (xf, yf) = match (x, y) {
                (Num::I(a), Num::I(b)) => (a as f64, b as f64),
                (Num::I(a), Num::F(b)) => (a as f64, b),
                (Num::F(a), Num::I(b)) => (a, b as f64),
                (Num::F(a), Num::F(b)) => (a, b),
            };
            Ok(Value::Float(xf.powf(yf)))
        }
        _ => Err(i.wrong_type_mut("numberp", &args[0])),
    }
}

fn f_sqrt(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    Ok(Value::Float(want_num(i, &args[0])?.sqrt()))
}
fn f_exp(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    Ok(Value::Float(want_num(i, &args[0])?.exp()))
}
fn f_sin(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    Ok(Value::Float(want_num(i, &args[0])?.sin()))
}
fn f_cos(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    Ok(Value::Float(want_num(i, &args[0])?.cos()))
}
fn f_tan(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    Ok(Value::Float(want_num(i, &args[0])?.tan()))
}
fn f_asin(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    Ok(Value::Float(want_num(i, &args[0])?.asin()))
}
fn f_acos(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    Ok(Value::Float(want_num(i, &args[0])?.acos()))
}
fn f_atan(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let y = want_num(i, &args[0])?;
    match args.get(1) {
        Some(x) => Ok(Value::Float(y.atan2(want_num(i, x)?))),
        None => Ok(Value::Float(y.atan())),
    }
}
fn f_log(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let n = want_num(i, &args[0])?;
    match args.get(1) {
        Some(b) => Ok(Value::Float(n.log(want_num(i, b)?))),
        None => Ok(Value::Float(n.ln())),
    }
}

fn f_logand(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let mut acc = -1i128;
    for a in &args {
        acc &= want_int(i, a)?;
    }
    Ok(Value::Int(acc))
}
fn f_logior(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let mut acc = 0i128;
    for a in &args {
        acc |= want_int(i, a)?;
    }
    Ok(Value::Int(acc))
}
fn f_logxor(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let mut acc = 0i128;
    for a in &args {
        acc ^= want_int(i, a)?;
    }
    Ok(Value::Int(acc))
}
fn f_lognot(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    Ok(Value::Int(!want_int(i, &args[0])?))
}
fn f_ash(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let v = want_int(i, &args[0])?;
    let c = want_int(i, &args[1])?;
    Ok(Value::Int(if c >= 0 {
        if c >= 128 {
            return Err(overflow_err(i, &args[0]));
        }
        match v.checked_mul(1i128 << c) {
            Some(r) => r,
            None => return Err(overflow_err(i, &args[0])),
        }
    } else if c <= -128 {
        if v < 0 { -1 } else { 0 }
    } else {
        v >> (-c) as u32
    }))
}
/// `lsh` — logical shift. Right shift masks the value to a 62-bit
/// unsigned fixnum then shifts; left shift is a signed shift whose
/// result may promote to a bignum. (Matches Emacs's mixed semantics.)
fn f_lsh(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let v = want_int(i, &args[0])?;
    let c = want_int(i, &args[1])?;
    const U62: u64 = 0x3fff_ffff_ffff_ffff; // 62-bit mask
    Ok(Value::Int(if c >= 0 {
        // Signed left shift: may produce a bignum.
        if c >= 128 {
            return Err(overflow_err(i, &args[0]));
        }
        match v.checked_mul(1i128 << c) {
            Some(r) => r,
            None => return Err(overflow_err(i, &args[0])),
        }
    } else {
        let k = -c;
        if (crate::lisp::value::FIXNUM_MIN..=crate::lisp::value::FIXNUM_MAX).contains(&v) {
            if k >= 62 {
                0
            } else {
                (((v as u64) & U62) >> k as u32) as i128
            }
        } else if k >= 128 {
            if v < 0 { -1 } else { 0 }
        } else {
            v >> k as u32
        }
    }))
}
fn f_random(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    // Deterministic PRNG (xorshift64*); (random t) reseeds from time.
    use std::cell::Cell;
    thread_local! {
        static SEED: Cell<u64> = const { Cell::new(0x9e3779b97f4a7c15) };
    }
    let is_t = args
        .get(0)
        .map(|v| {
            matches!(v, Value::Sym(_)) && i.sym_is(v, i.intern_soft("t").unwrap_or(u32::MAX))
        })
        .unwrap_or(false);
    if is_t {
        let t = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.subsec_nanos() as u64 ^ (d.as_secs() << 16))
            .unwrap_or(0xabcdef);
        SEED.with(|s| s.set(t | 1));
    }
    let next = SEED.with(|s| {
        let mut x = s.get();
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        s.set(x);
        x.wrapping_mul(0x2545F4914F6CDD1D)
    });
    let limit = match args.get(0) {
        None => crate::lisp::value::FIXNUM_MAX,
        // (random t) reseeds and returns a full-range fixnum.
        Some(v @ Value::Sym(_)) if i.sym_is(v, i.intern_soft("t").unwrap_or(u32::MAX)) => {
            crate::lisp::value::FIXNUM_MAX
        }
        Some(v @ Value::Sym(_)) => return Err(i.wrong_type_mut("integerp", v)),
        Some(Value::Int(n)) if *n > 0 => *n,
        Some(Value::Int(_)) => {
            let s = i.intern("args-out-of-range");
            return Err(i.signal_data(s, vec![args[0].clone()]));
        }
        Some(other) => return Err(i.wrong_type_mut("integerp", other)),
    };
    Ok(Value::Int(((next >> 1) as i128) % limit))
}
fn f_eql(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let _ = i;
    Ok(Value::from_bool(super::eql_values(&args[0], &args[1])))
}

// ---------- float decomposition & misc ----------

fn f_frexp(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let x = want_num(i, &args[0])?;
    if x == 0.0 || x.is_nan() || x.is_infinite() {
        return Ok(Value::cons(Value::Float(x), Value::Int(0)));
    }
    let e = x.abs().log2().floor() as i128 + 1;
    let m = x / 2f64.powi(e as i32);
    Ok(Value::cons(Value::Float(m), Value::Int(e)))
}

fn f_ldexp(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let x = want_num(i, &args[0])?;
    let e = match args.get(1) {
        Some(Value::Int(n)) => *n as i32,
        _ => 0,
    };
    Ok(Value::Float(x * 2f64.powi(e)))
}

/// GNU's ffloor/fceiling/fround/ftruncate require a FLOAT argument.
fn want_float(i: &mut Interp, v: &Value) -> Result<f64, Flow> {
    match v {
        Value::Float(x) => Ok(*x),
        other => Err(i.wrong_type_mut("floatp", other)),
    }
}

fn f_fround(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let x = want_float(i, &args[0])?;
    Ok(Value::Float(x.round_ties_even()))
}

fn f_ftruncate(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let x = want_float(i, &args[0])?;
    Ok(Value::Float(x.trunc()))
}

fn f_fceiling(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let x = want_float(i, &args[0])?;
    Ok(Value::Float(x.ceil()))
}

fn f_ffloor(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let x = want_float(i, &args[0])?;
    Ok(Value::Float(x.floor()))
}

fn f_copysign(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    // Emacs requires both args to be floats.
    let (x, y) = match (&args[0], &args[1]) {
        (Value::Float(a), Value::Float(b)) => (*a, *b),
        (Value::Float(_), other) | (other, _) => return Err(i.wrong_type_mut("floatp", other)),
    };
    Ok(Value::Float(x.copysign(y)))
}

fn f_logb(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let x = want_num(i, &args[0])?;
    if x == 0.0 {
        return Ok(Value::Float(f64::NEG_INFINITY));
    }
    if x.is_nan() {
        return Ok(Value::Float(f64::NAN));
    }
    if x.is_infinite() {
        return Ok(Value::Float(f64::INFINITY));
    }
    Ok(Value::Int(x.abs().log2().floor() as i128))
}

fn f_isnan(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let x = want_num(i, &args[0])?;
    Ok(Value::from_bool(x.is_nan()))
}

fn f_logcount(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let n = want_int(i, &args[0])?;
    // Emacs counts 1-bits for nonneg, 0-bits for negative.
    let c = if n >= 0 {
        n.count_ones()
    } else {
        (!n).count_ones()
    };
    Ok(Value::Int(c as i128))
}

fn f_plusp(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    Ok(Value::from_bool(want_num(i, &args[0])? > 0.0))
}
fn f_minusp(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    Ok(Value::from_bool(want_num(i, &args[0])? < 0.0))
}
fn f_oddp(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    Ok(Value::from_bool(want_int(i, &args[0])? % 2 != 0))
}
fn f_evenp(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    Ok(Value::from_bool(want_int(i, &args[0])? % 2 == 0))
}
fn f_fixnump(_i: &mut Interp, args: Vec<Value>) -> EvalResult {
    Ok(Value::from_bool(args[0].fixnump()))
}
fn f_bignump(_i: &mut Interp, args: Vec<Value>) -> EvalResult {
    Ok(Value::from_bool(matches!(
        &args[0],
        Value::Int(n) if !(crate::lisp::value::FIXNUM_MIN..=crate::lisp::value::FIXNUM_MAX).contains(n)
    )))
}
/// `isqrt` — integer square root (Newton's method on i128).
fn f_isqrt(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let n = want_int(i, &args[0])?;
    if n < 0 {
        let s = i.intern("arith-error");
        return Err(i.signal_data(s, vec![args[0].clone()]));
    }
    if n < 2 {
        return Ok(Value::Int(n));
    }
    let mut x = 1i128 << ((128 - n.leading_zeros() as i128 + 1) / 2);
    loop {
        let y = (x + n / x) / 2;
        if y >= x {
            break;
        }
        x = y;
    }
    Ok(Value::Int(x))
}
