//! GNU numeric output descriptors, shortest round-tripping floats, and character notation.

use crate::lisp::{Interp, print_text::PrintText, value::Value};
use std::fmt::Write;
use super::render::PrintMode;

impl Interp {
    /// Print a number using the dynamically bound GNU output descriptor.
    pub(crate) fn format_float_output(&self, value: f64) -> String {
        let format = self.print_var("float-output-format");
        if let Value::Str(s) = format {
            if let Some(output) = s
                .borrow()
                .is_ascii()
                .then(|| format_float_descriptor(value, &s.borrow().to_unicode_lossy()))
                .flatten()
            {
                return output;
            }
        }
        format_float(value)
    }

    /// GNU print.c: only independent graphic characters and the six
    /// common named escapes use character notation.  `princ' omits the
    /// extra quoting for delimiter characters, but keeps named escapes.
    pub(super) fn print_integer(&self, n: i128, out: &mut PrintText, mode: PrintMode) {
        if self.print_var("print-integers-as-characters").truthy() && (0..=0x10ffff).contains(&n) {
            let named = match n {
                8 => Some('b'),
                9 => Some('t'),
                10 => Some('n'),
                12 => Some('f'),
                13 => Some('r'),
                32 => Some('s'),
                _ => None,
            };
            let table = self.print_var("unicode-category-table");
            let category = crate::lisp::builtins::misc::char_table_ref(self, &table, n as usize);
            let graphic = matches!(category, Value::Int(cat)
                if (crate::lisp::value::FIXNUM_MIN..=crate::lisp::value::FIXNUM_MAX).contains(&cat)
                    && !matches!(cat, 6..=8 | 23..=28 | 30));
            if let Some(named) = named {
                out.push('?');
                out.push('\\');
                out.push(named);
                return;
            }
            if graphic {
                if let Some(c) = char::from_u32(n as u32) {
                    out.push('?');
                    if mode.escaped()
                        && matches!(
                            c,
                            ';' | '"' | '\'' | '\\' | '(' | ')' | '{' | '}' | '[' | ']'
                        )
                    {
                        out.push('\\');
                    }
                    out.push(c);
                    return;
                }
            }
        }
        let _ = write!(out, "{n}");
    }
}

/// C `%.{prec}g` formatting: `prec` significant digits, scientific
/// notation when the decimal exponent is < -4 or >= prec, trailing
/// zeros stripped.
fn format_g(x: f64, prec: usize) -> String {
    // Get `prec` significant digits plus the decimal exponent.
    let sci = format!("{:.*e}", prec - 1, x); // "-d.ddde-12"
    let epos = sci.find('e').unwrap();
    let exp: i32 = sci[epos + 1..].parse().unwrap();
    let mant = &sci[..epos];
    let neg = mant.starts_with('-');
    let digits: String = mant.chars().filter(|c| c.is_ascii_digit()).collect();

    let mut out = String::new();
    if neg {
        out.push('-');
    }
    if exp < -4 || exp >= prec as i32 {
        // Scientific: d.ddde±XX (exponent at least two digits).
        out.push(digits.as_bytes()[0] as char);
        let frac: String = digits[1..].trim_end_matches('0').into();
        if !frac.is_empty() {
            out.push('.');
            out.push_str(&frac);
        }
        out.push('e');
        out.push(if exp < 0 { '-' } else { '+' });
        let _ = write!(out, "{:02}", exp.abs());
    } else if exp >= 0 {
        // Fixed: digits with decimal point inside (exp < prec).
        let ip = exp as usize + 1;
        let (int_part, frac_part) = if digits.len() > ip {
            (&digits[..ip], &digits[ip..])
        } else {
            (digits.as_str(), "")
        };
        out.push_str(int_part);
        for _ in digits.len()..ip {
            out.push('0');
        }
        let frac: String = frac_part.trim_end_matches('0').into();
        if !frac.is_empty() {
            out.push('.');
            out.push_str(&frac);
        }
    } else {
        // 0.000ddd
        out.push_str("0.");
        for _ in 0..(-exp - 1) {
            out.push('0');
        }
        let frac: String = digits.trim_end_matches('0').into();
        out.push_str(&frac);
    }
    out
}

/// GNU accepts only %.PRECISION followed by e, f or g, at most DBL_DIG
/// digits of precision.  An absent precision is accepted too, as in C printf.
fn format_float_descriptor(value: f64, descriptor: &str) -> Option<String> {
    if !value.is_finite() {
        return None;
    }
    // GNU validates the C string, so text after a NUL is not part of the spec.
    let spec = descriptor.split('\0').next()?.as_bytes();
    if !spec.starts_with(b"%.") {
        return None;
    }
    let mut pos = 2;
    let mut precision = None;
    while let Some(digit @ b'0'..=b'9') = spec.get(pos) {
        let n = precision.unwrap_or(0) * 10 + usize::from(*digit - b'0');
        if n > 15 {
            return None;
        }
        precision = Some(n);
        pos += 1;
    }
    let kind = *spec.get(pos)?;
    if pos + 1 != spec.len() || !matches!(kind, b'e' | b'f' | b'g') {
        return None;
    }
    if precision == Some(0) && kind != b'f' {
        return None;
    }
    let digits = precision.unwrap_or(0);
    let mut output = match kind {
        b'f' => format!("{:.*}", digits, value),
        b'e' => {
            let scientific = format!("{:.*e}", digits, value);
            let (mantissa, exponent) = scientific.split_once('e')?;
            let exponent: i32 = exponent.parse().ok()?;
            format!(
                "{}e{}{:02}",
                mantissa,
                if exponent < 0 { '-' } else { '+' },
                exponent.abs()
            )
        }
        b'g' => format_g(value, digits.max(1)),
        _ => unreachable!(),
    };
    // Only an explicitly zero precision with f may print an integer token.
    if precision != Some(0) && !output.contains('.') && !output.contains('e') {
        output.push_str(".0");
    }
    Some(output)
}

/// Emacs-style float printing (`float_to_string`): shortest of
/// `%.15g`–`%.17g` that reads back to the same double.  Subnormals
/// start at one significant digit, as in gnulib's dtoastr.
/// `1.5`, `1.0`, `1e+20`, `1.0e+INF`, `-1.0e+INF`, `0.0e+NaN`.
/// NaNs print the numeric mantissa GNU derives from the NaN payload
/// (`float_error' notation): `get_nan_payload' strips the quiet bit,
/// and the payload value truncates to the printed integer — literals
/// like `3.5e+NaN' read back as payload 3 and print `3.0e+NaN'.
pub fn format_float(f: f64) -> String {
    if f.is_nan() {
        let payload = f.to_bits() & ((1u64 << 51) - 1);
        let sign = if f.is_sign_negative() { "-" } else { "" };
        // f64 prints payloads ≥2^53 exactly enough for the integral
        // mantissa GNU emits.
        return format!("{}{}.0e+NaN", sign, payload);
    }
    if f.is_infinite() {
        return if f > 0.0 { "1.0e+INF" } else { "-1.0e+INF" }.into();
    }
    if f == 0.0 {
        return if f.is_sign_negative() { "-0.0" } else { "0.0" }.into();
    }
    let mut s = String::new();
    let first_precision = if f.is_subnormal() { 1 } else { 15 };
    for prec in first_precision..=17 {
        s = format_g(f, prec);
        if s.parse::<f64>().ok() == Some(f) {
            break;
        }
    }
    // A float must always print with a decimal point or exponent.
    if !s.contains('.') && !s.contains('e') {
        s.push_str(".0");
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finite_output_round_trips_and_remains_float_syntax() {
        for value in [
            f64::from_bits(1),
            f64::MIN_POSITIVE,
            f64::MAX,
            0.1,
            1.5,
            1e20,
        ] {
            let text = format_float(value);
            assert_eq!(text.parse::<f64>().unwrap().to_bits(), value.to_bits());
            assert!(text.contains('.') || text.contains('e'));
        }
    }

    #[test]
    fn signed_zero_infinities_and_nan_payloads_keep_gnu_notation() {
        assert_eq!(format_float(0.0), "0.0");
        assert_eq!(format_float(-0.0), "-0.0");
        assert_eq!(format_float(f64::INFINITY), "1.0e+INF");
        assert_eq!(format_float(f64::NEG_INFINITY), "-1.0e+INF");
        assert_eq!(
            format_float(f64::from_bits(
                (1u64 << 63) | (0x7ffu64 << 52) | (1u64 << 51) | 3
            )),
            "-3.0e+NaN"
        );
    }
}
