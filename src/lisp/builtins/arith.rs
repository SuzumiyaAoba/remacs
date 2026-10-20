//! Arithmetic and numeric comparison subrs.

use super::{S, want_int, want_num};
use crate::lisp::Interp;
use crate::lisp::error::{EvalResult, Flow};
use crate::lisp::obarray::sym;
use crate::lisp::value::{Marker, Subr, Value};
use std::cell::RefCell;
use std::rc::Rc;

fn arith_err(i: &Interp, _msg: &str) -> Flow {
    // GNU signals bare (arith-error) with no payload.
    i.signal_data(sym::ARITH_ERROR, vec![])
}

fn overflow_err(i: &mut Interp, v: &Value) -> Flow {
    let s = i.intern("overflow-error");
    i.signal_data(s, vec![v.clone()])
}

/// Number coercion: int if all-int else float.
#[derive(Clone, Copy)]
enum Num {
    I(i128),
    F(f64),
}

/// Order `n` against `f` without precision loss.  `None` when `f` is
/// NaN (the argument type check happens before this in GNU).
fn int_f64_ord(n: i128, f: f64) -> Option<std::cmp::Ordering> {
    use std::cmp::Ordering::*;
    if f.is_nan() {
        return None;
    }
    // (i128::MAX as f64) rounds up to 2**127 and i128::MIN is exactly
    // -2**127, so only these two boundary comparisons need guarding.
    if f >= 2f64.powi(127) {
        return Some(Less);
    }
    if f < -2f64.powi(127) {
        return Some(Greater);
    }
    let t = f.trunc() as i128;
    Some(match n.cmp(&t) {
        Equal => 0f64.partial_cmp(&f.fract()).unwrap_or(Equal),
        o => o,
    })
}

/// Exact ordering of two numbers (markers already reduced to their
/// positions by `to_num`).  `None` iff a float operand is NaN, which
/// makes every `=</<=/>/>=` comparison false, matching GNU.
fn num_ord(a: Num, b: Num) -> Option<std::cmp::Ordering> {
    match (a, b) {
        (Num::I(x), Num::I(y)) => Some(x.cmp(&y)),
        (Num::F(x), Num::F(y)) => x.partial_cmp(&y),
        (Num::I(x), Num::F(y)) => int_f64_ord(x, y),
        (Num::F(x), Num::I(y)) => int_f64_ord(y, x).map(std::cmp::Ordering::reverse),
    }
}

/// GNU `CHECK_NUMBER_OR_MARKER_COERCE': a marker whose buffer is dead
/// (or that points nowhere) signals "Marker does not point anywhere"
/// rather than yielding a stale position.
fn marker_pos_err(i: &mut Interp, m: &Rc<RefCell<Marker>>) -> Result<i128, Flow> {
    if m.borrow().buffer.is_none() {
        return Err(i.error("Marker does not point anywhere"));
    }
    Ok(m.borrow().position as i128 + 1)
}

fn to_num(i: &mut Interp, v: &Value) -> Result<Option<Num>, Flow> {
    Ok(match v {
        Value::Int(n) => Some(Num::I(*n)),
        Value::Float(f) => Some(Num::F(**f)),
        Value::Marker(m) => Some(Num::I(marker_pos_err(i, m)?)),
        _ => None,
    })
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
    let mut acc_i: i128 = 0;
    let mut acc_f: f64 = 0.0;
    let mut is_float = false;
    for a in args.iter() {
        match to_num(i, a)? {
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
        Value::float(acc_f)
    } else {
        Value::Int(acc_i)
    })
}

fn f_minus(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    // GNU `Fminus': no arguments → 0.
    if args.is_empty() {
        return Ok(Value::Int(0));
    }
    if args.len() == 1 {
        return match to_num(i, &args[0])? {
            Some(Num::I(n)) => n
                .checked_neg()
                .map(Value::Int)
                .ok_or_else(|| overflow_err(i, &args[0])),
            Some(Num::F(f)) => Ok(Value::float(-f)),
            None => Err(i.wrong_type_mut("number-or-marker-p", &args[0])),
        };
    }
    let mut it = args.iter();
    let first = to_num(i, it.next().unwrap())?;
    let (mut acc_i, mut acc_f, mut is_float) = match first {
        Some(Num::I(n)) => (n, 0.0, false),
        Some(Num::F(f)) => (0, f, true),
        None => return Err(i.wrong_type_mut("number-or-marker-p", &args[0])),
    };
    for a in it {
        match to_num(i, a)? {
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
        Value::float(acc_f)
    } else {
        Value::Int(acc_i)
    })
}

fn f_times(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    let mut acc_i: i128 = 1;
    let mut acc_f: f64 = 1.0;
    let mut is_float = false;
    for a in args.iter() {
        match to_num(i, a)? {
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
        Value::float(acc_f)
    } else {
        Value::Int(acc_i)
    })
}

fn f_div(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    if args.len() == 1 {
        // (/ x) = 1/x
        return match to_num(i, &args[0])? {
            Some(Num::I(n)) => {
                if n == 0 {
                    Err(arith_err(i, "Division by zero"))
                } else {
                    // GNU: (/ N) with a fixnum is integer division.
                    Ok(Value::Int(1 / n))
                }
            }
            Some(Num::F(f)) => Ok(Value::float(1.0 / f)),
            None => Err(i.wrong_type_mut("number-or-marker-p", &args[0])),
        };
    }
    let first = to_num(i, &args[0])?;
    let (mut acc_i, mut acc_f, mut is_float) = match first {
        Some(Num::I(n)) => (n, 0.0, false),
        Some(Num::F(f)) => (0, f, true),
        None => return Err(i.wrong_type_mut("number-or-marker-p", &args[0])),
    };
    for a in &args[1..] {
        match to_num(i, a)? {
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
        Value::float(acc_f)
    } else {
        Value::Int(acc_i)
    })
}

/// `%` — integer remainder only (floats are a type error in Emacs).
fn f_mod(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
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
fn f_mod_fn(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    match (to_num(i, &args[0])?, to_num(i, &args[1])?) {
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
            Ok(Value::float(if yf < 0.0 && r != 0.0 { r + yf } else { r }))
        }
        _ => {
            let bad = if to_num(i, &args[0])?.is_none() {
                &args[0]
            } else {
                &args[1]
            };
            Err(i.wrong_type_mut("number-or-marker-p", bad))
        }
    }
}

fn f_1plus(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    match to_num(i, &args[0])? {
        Some(Num::I(n)) => n
            .checked_add(1)
            .map(Value::Int)
            .ok_or_else(|| overflow_err(i, &args[0])),
        Some(Num::F(f)) => Ok(Value::float(f + 1.0)),
        None => Err(i.wrong_type_mut("number-or-marker-p", &args[0])),
    }
}

fn f_1minus(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    match to_num(i, &args[0])? {
        Some(Num::I(n)) => n
            .checked_sub(1)
            .map(Value::Int)
            .ok_or_else(|| overflow_err(i, &args[0])),
        Some(Num::F(f)) => Ok(Value::float(f - 1.0)),
        None => Err(i.wrong_type_mut("number-or-marker-p", &args[0])),
    }
}

fn f_abs(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    if matches!(&args[0], Value::Marker(_)) {
        return Err(i.wrong_type_mut("numberp", &args[0]));
    }
    match to_num(i, &args[0])? {
        Some(Num::I(n)) => n
            .checked_abs()
            .map(Value::Int)
            .ok_or_else(|| overflow_err(i, &args[0])),
        Some(Num::F(f)) => Ok(Value::float(f.abs())),
        None => Err(i.wrong_type_mut("numberp", &args[0])),
    }
}

/// `min`/`max` — GNU's minmax_driver: the winning argument's value is
/// kept (markers coerce to their position first); a NaN argument wins
/// immediately regardless of the comparison.
fn minmax(i: &mut Interp, args: &[Value], lt: bool) -> EvalResult {
    // The winning Num contributes its own Value, except a marker
    // yields its (coerced) position fixnum like GNU.
    fn winner(orig: &Value, n: Num) -> Value {
        match (orig, n) {
            (Value::Marker(_), Num::I(p)) => Value::Int(p),
            _ => orig.clone(),
        }
    }
    let mut best_n = match to_num(i, &args[0])? {
        Some(n) => n,
        None => return Err(i.wrong_type_mut("number-or-marker-p", &args[0])),
    };
    let mut best = winner(&args[0], best_n);
    for a in &args[1..] {
        let an = match to_num(i, a)? {
            Some(n) => n,
            None => return Err(i.wrong_type_mut("number-or-marker-p", a)),
        };
        if matches!(an, Num::F(f) if f.is_nan()) {
            return Ok(a.clone());
        }
        let wins = match num_ord(an, best_n) {
            Some(std::cmp::Ordering::Less) => lt,
            Some(std::cmp::Ordering::Greater) => !lt,
            _ => false,
        };
        if wins {
            best_n = an;
            best = winner(a, an);
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
        if !pred(num_ord(a, b)) {
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
        num_ord(a, b) != Some(std::cmp::Ordering::Equal),
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
        matches!(&args[0], Value::Int(n) if *n >= 0),
    ))
}

fn f_integerp(_i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    Ok(Value::from_bool(matches!(&args[0], Value::Int(_))))
}
fn f_numberp(_i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    Ok(Value::from_bool(matches!(
        &args[0],
        Value::Int(_) | Value::Float(_)
    )))
}
fn f_floatp(_i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    Ok(Value::from_bool(matches!(&args[0], Value::Float(_))))
}
fn f_number_or_marker_p(_i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    Ok(Value::from_bool(matches!(
        &args[0],
        Value::Int(_) | Value::Float(_) | Value::Marker(_)
    )))
}

/// Rounding: f(x) or f(x/y) with optional divisor. Always returns an
/// integer (possibly a bignum) in Emacs, even for float inputs.
fn round_with(i: &mut Interp, args: &[Value], mode: u8) -> EvalResult {
    // Markers fail the `numberp' check in GNU's rounding functions.
    for a in args {
        if matches!(a, Value::Marker(_)) {
            return Err(i.wrong_type_mut("numberp", a));
        }
    }
    let x = match to_num(i, &args[0])? {
        Some(n) => n,
        None => return Err(i.wrong_type_mut("numberp", &args[0])),
    };
    let y = match args.get(1) {
        // GNU rounds with Y = 1 when the divisor is omitted OR nil.
        None | Some(Value::Nil) => None,
        Some(v) => match to_num(i, v)? {
            Some(n) => Some(n),
            None => return Err(i.wrong_type_mut("numberp", v)),
        },
    };
    // Integer inputs take the exact path — routing them through f64
    // loses low bits above 2**53, e.g. (truncate most-positive-fixnum).
    if let Num::I(a) = x {
        match y {
            None => return Ok(Value::Int(a)),
            Some(Num::I(b)) => {
                if b == 0 {
                    return Err(arith_err(i, "Division by zero"));
                }
                let q = a / b;
                let r = a % b;
                let d = match mode {
                    0 => q,
                    1 => {
                        // floor: toward negative infinity.
                        if r != 0 && ((r < 0) != (b < 0)) {
                            q - 1
                        } else {
                            q
                        }
                    }
                    2 => {
                        // ceiling: toward positive infinity.
                        if r != 0 && ((r < 0) == (b < 0)) {
                            q + 1
                        } else {
                            q
                        }
                    }
                    _ => {
                        // round: nearest, ties to even (GNU parity).
                        // Compare 2*|r| vs |b| in u128 so i128::MIN works.
                        let ra = r.unsigned_abs();
                        let ba = b.unsigned_abs();
                        let gt = ra > ba / 2;
                        let eq = ba % 2 == 0 && ra == ba / 2;
                        if gt || (eq && q % 2 != 0) {
                            if (r < 0) == (b < 0) { q + 1 } else { q - 1 }
                        } else {
                            q
                        }
                    }
                };
                return Ok(Value::Int(d));
            }
            _ => {}
        }
    }
    let (xf, yf) = match (x, y) {
        (Num::I(a), None) => (a as f64, 1.0),
        (Num::I(a), Some(Num::I(b))) => (a as f64, b as f64),
        (Num::I(a), Some(Num::F(b))) => (a as f64, b),
        (Num::F(a), None) => (a, 1.0),
        (Num::F(a), Some(Num::I(b))) => (a, b as f64),
        (Num::F(a), Some(Num::F(b))) => (a, b),
    };
    if yf == 0.0 {
        return Err(arith_err(i, "Division by zero"));
    }
    let q = xf / yf;
    let r = match mode {
        0 => q.trunc(),
        1 => q.floor(),
        2 => q.ceil(),
        _ => q.round_ties_even(),
    };
    // GNU signals `overflow-error' when the rounded value exceeds
    // fixnum range (it would cons a bignum — we have none).
    let lo = crate::lisp::value::FIXNUM_MIN as f64;
    let hi = crate::lisp::value::FIXNUM_MAX as f64;
    if !r.is_finite() || r < lo || r > hi {
        let osym = i.intern("overflow-error");
        return Err(i.signal_data(osym, vec![]));
    }
    Ok(Value::Int(r as i128))
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
        Value::Float(f) => Ok(**f),
        _ => Err(i.wrong_type_mut("numberp", v)),
    }
}

/// Integer or marker → position.  GNU's bitwise/oddp functions use
/// `integer-or-marker-p' as their error predicate.
pub(crate) fn want_int_or_marker(i: &mut Interp, v: &Value) -> Result<i128, Flow> {
    match v {
        Value::Int(n) => Ok(*n),
        Value::Marker(m) => marker_pos_err(i, m),
        _ => Err(i.wrong_type_mut("integer-or-marker-p", v)),
    }
}

fn f_float(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    Ok(Value::float(want_number(i, &args[0])?))
}

fn f_expt(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    for a in args.iter() {
        if matches!(a, Value::Marker(_)) {
            return Err(i.wrong_type_mut("numberp", a));
        }
    }
    match (to_num(i, &args[0])?, to_num(i, &args[1])?) {
        (Some(Num::I(x)), Some(Num::I(y))) => {
            if y >= 0 {
                match x.checked_pow(y.min(u32::MAX as i128) as u32) {
                    Some(r) => return Ok(Value::Int(r)),
                    // Beyond i128 — fall back to float (true bignum
                    // would still be an integer; documented deviation).
                    None => return Ok(Value::float((x as f64).powf(y as f64))),
                }
            }
            Ok(Value::float((x as f64).powf(y as f64)))
        }
        (Some(x), Some(y)) => {
            let (xf, yf) = match (x, y) {
                (Num::I(a), Num::I(b)) => (a as f64, b as f64),
                (Num::I(a), Num::F(b)) => (a as f64, b),
                (Num::F(a), Num::I(b)) => (a, b as f64),
                (Num::F(a), Num::F(b)) => (a, b),
            };
            Ok(Value::float(xf.powf(yf)))
        }
        _ => {
            let bad = if to_num(i, &args[0])?.is_none() {
                &args[0]
            } else {
                &args[1]
            };
            Err(i.wrong_type_mut("numberp", bad))
        }
    }
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
fn logcoerce(i: &mut Interp, v: &Value, first: bool) -> Result<i128, Flow> {
    match v {
        Value::Int(n) => Ok(*n),
        Value::Marker(m) if m.borrow().buffer.is_some() => Ok(m.borrow().position as i128 + 1),
        _ if first => Err(i.wrong_type_mut("integer-or-marker-p", v)),
        Value::Float(_) | Value::Marker(_) => Err(i.wrong_type_mut("integer-or-marker-p", v)),
        other => Err(i.wrong_type_mut("number-or-marker-p", other)),
    }
}
fn f_logand(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    let mut acc = -1i128;
    for (ix, a) in args.iter().enumerate() {
        acc &= logcoerce(i, a, ix == 0)?;
    }
    Ok(Value::Int(acc))
}
fn f_logior(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    let mut acc = 0i128;
    for (ix, a) in args.iter().enumerate() {
        acc |= logcoerce(i, a, ix == 0)?;
    }
    Ok(Value::Int(acc))
}
pub(crate) fn f_logxor(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    let mut acc = 0i128;
    for (ix, a) in args.iter().enumerate() {
        acc ^= logcoerce(i, a, ix == 0)?;
    }
    Ok(Value::Int(acc))
}
fn f_lognot(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    // GNU CHECK_INTEGER: markers are rejected, unlike logand/logior.
    Ok(Value::Int(!want_int(i, &args[0])?))
}
/// Left shift with overflow detection: `wrapping_shl' yields the
/// wrapped result even at c = 127 (where 1 << 127 becomes the
/// negative MIN), so round-trip a right shift to verify the value
/// survived — exactly the bits GNU's bignum would keep.
fn shl_checked(v: i128, c: u32) -> Option<i128> {
    let r = v.wrapping_shl(c);
    if r >> c == v { Some(r) } else { None }
}

fn f_ash(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    let v = want_int(i, &args[0])?;
    let c = want_int(i, &args[1])?;
    Ok(Value::Int(if c >= 0 {
        if c >= 128 {
            return Err(overflow_err(i, &args[0]));
        }
        match shl_checked(v, c as u32) {
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
fn f_lsh(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    // GNU `Flsh' coerces markers via CHECK_INTEGER_OR_MARKER: a dead
    // marker signals "Marker does not point anywhere" before the
    // integer check; a live one supplies its position.
    let v = match &args[0] {
        Value::Int(n) => *n,
        Value::Marker(m) => {
            let _ = marker_pos_err(i, m)?;
            return Err(i.wrong_type_mut("integerp", &args[0]));
        }
        Value::Float(_) => {
            return Err(i.wrong_type_mut("integerp", &args[0]));
        }
        other => return Err(i.wrong_type_mut("number-or-marker-p", other)),
    };
    let c = want_int(i, &args[1])?;
    const U62: u64 = 0x3fff_ffff_ffff_ffff; // 62-bit mask
    Ok(Value::Int(if c >= 0 {
        // Signed left shift: may produce a bignum.
        if c >= 128 {
            return Err(overflow_err(i, &args[0]));
        }
        match shl_checked(v, c as u32) {
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

fn f_random(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    let t_sym = i.intern_soft("t").unwrap_or(u32::MAX);
    let mut limit: Option<i64> = None;
    match args.get(0) {
        Some(Value::Sym(s)) if *s == t_sym => {
            // init_random: seed from OS entropy.
            let t = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.subsec_nanos() as u64 ^ (d.as_secs() << 16))
                .unwrap_or(0xabcdef);
            GEN.with(|g| *g.borrow_mut() = Rand3::seeded(t as u32));
        }
        Some(Value::Str(s)) => {
            // seed_random: XOR string bytes into a u32 seed
            // (little-endian argp[i % 4] layout).
            let b = s.borrow().as_bytes().to_vec();
            let mut arg = 0u32;
            for (k, &by) in b.iter().enumerate() {
                arg ^= (by as u32) << ((k % 4) * 8);
            }
            GEN.with(|g| *g.borrow_mut() = Rand3::seeded(arg));
        }
        Some(Value::Int(n)) => {
            if *n <= 0 {
                let s = i.intern("args-out-of-range");
                return Err(i.signal_data(s, vec![args[0].clone()]));
            }
            limit = Some(*n as i64);
        }
        _ => {}
    }
    // sysdep.c get_random: FIXNUM_BITS=62, RAND_BITS=31 → 2 draws per
    // value; INTMASK = 2^62-1.  Seeding arms fall through to this same
    // return in GNU, so (random "s")/(random t) also return a value.
    let get = || -> i64 {
        GEN.with(|g| {
            let mut g = g.borrow_mut();
            let mut val: u64 = 0;
            for _ in 0..2 {
                val = g.next() ^ (val << 31) ^ (val >> 33);
            }
            val ^= val >> 2;
            (val & 0x3fff_ffff_ffff_ffff) as i64
        })
    };
    match limit {
        Some(lim) => {
            // get_random_fixnum: remainder rejection near INTMASK.
            let difflim: i64 = 0x3fff_ffff_ffff_ffff - lim + 1;
            loop {
                let r = get();
                let rem = r % lim;
                if r - rem <= difflim {
                    return Ok(Value::Int(rem as i128));
                }
            }
        }
        None => {
            // make_ufixnum: values past most-positive-fixnum wrap into
            // the negative fixnum range (61-bit signed fixnums).
            let v = get();
            let signed = if v > 0x1fff_ffff_ffff_ffff {
                v - 0x4000_0000_0000_0000
            } else {
                v
            };
            Ok(Value::Int(signed as i128))
        }
    }
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
    // GNU Fildexp: SGN is FIXNUM-checked first, then X is number-checked.
    let e = match &args[1] {
        Value::Int(n) => *n as i32,
        other => return Err(i.wrong_type_mut("fixnump", other)),
    };
    let x = want_number(i, &args[0])?;
    Ok(Value::float(x * 2f64.powi(e)))
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
    let n = want_int(i, &args[0])?;
    // Emacs counts 1-bits for nonneg, 0-bits for negative.
    let c = if n >= 0 {
        n.count_ones()
    } else {
        (!n).count_ones()
    };
    Ok(Value::Int(c as i128))
}

fn f_plusp(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    Ok(Value::from_bool(want_num(i, &args[0])? > 0.0))
}
fn f_minusp(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    Ok(Value::from_bool(want_num(i, &args[0])? < 0.0))
}
fn f_oddp(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    Ok(Value::from_bool(want_int_or_marker(i, &args[0])? % 2 != 0))
}
fn f_evenp(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    Ok(Value::from_bool(want_int_or_marker(i, &args[0])? % 2 == 0))
}
fn f_fixnump(_i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    Ok(Value::from_bool(args[0].fixnump()))
}
fn f_bignump(_i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    Ok(Value::from_bool(matches!(
        &args[0],
        Value::Int(n) if !(crate::lisp::value::FIXNUM_MIN..=crate::lisp::value::FIXNUM_MAX).contains(n)
    )))
}
