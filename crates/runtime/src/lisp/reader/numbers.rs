//! Full-token GNU integer/float syntax, independent of reader input state.

use crate::lisp::value::Value;

/// Try parsing a token as a number (int or float).
/// Returns `Some(value)` if it parses fully as a number.
///
/// Emacs quirk: `1.` reads as *integer* 1, while `1.e3`, `1.5`, `.5`,
/// `1e3` are floats.
pub fn parse_number(tok: &str) -> Option<Value> {
    if tok.is_empty() {
        return None;
    }
    if let Ok(i) = tok.parse::<i128>() {
        return Some(Value::integer(i));
    }
    let digits = tok.strip_prefix(['+', '-']).unwrap_or(tok);
    if !digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit()) {
        if let Ok(i) = tok.parse::<num_bigint::BigInt>() {
            return Some(Value::from_bigint(i));
        }
    }
    if let Some(f) = parse_float(tok) {
        return Some(Value::float(f));
    }
    // `123.` (digits + trailing dot, no exponent) => integer.
    if let Some(head) = tok.strip_suffix('.') {
        if !head.is_empty()
            && head
                .bytes()
                .all(|b| b.is_ascii_digit() || b == b'+' || b == b'-')
        {
            if let Ok(i) = head.parse::<num_bigint::BigInt>() {
                return Some(Value::from_bigint(i));
            }
        }
    }
    None
}

fn parse_float(tok: &str) -> Option<f64> {
    let bytes = tok.as_bytes();
    let mut i = 0usize;
    if i < bytes.len() && (bytes[i] == b'+' || bytes[i] == b'-') {
        i += 1;
    }
    let start_digits = i;
    while i < bytes.len() && bytes[i].is_ascii_digit() {
        i += 1;
    }
    let int_digits = i - start_digits;
    let mut frac_digits = 0usize;
    let mut saw_dot = false;
    if i < bytes.len() && bytes[i] == b'.' {
        saw_dot = true;
        i += 1;
        let fs = i;
        while i < bytes.len() && bytes[i].is_ascii_digit() {
            i += 1;
        }
        frac_digits = i - fs;
    }
    if int_digits + frac_digits == 0 {
        return None;
    }
    let mut saw_exp = false;
    if i < bytes.len() && (bytes[i] == b'e' || bytes[i] == b'E') {
        saw_exp = true;
        i += 1;
        let mut exp_sign = None;
        if i < bytes.len() && (bytes[i] == b'+' || bytes[i] == b'-') {
            exp_sign = Some(bytes[i]);
            i += 1;
        }
        // GNU's special literals: `[eE]+NaN' and `[eE]+INF' — the
        // exponent sign must be `+', the suffix is case-sensitive.
        // `float_error' semantics: the mantissa's truncated value
        // becomes the NaN payload (quiet bit excluded) and the
        // literal's sign the NaN's sign, so `3.5e+NaN' prints back
        // as `3.0e+NaN'.
        if exp_sign == Some(b'+') {
            let rest = &tok[i..];
            if rest == "NaN" || rest == "INF" {
                let neg = tok.starts_with('-');
                return Some(if rest == "NaN" {
                    let mantissa: f64 = tok[..i - 2].parse().unwrap_or(0.0);
                    let payload = (mantissa.abs() as u64) & ((1u64 << 51) - 1);
                    f64::from_bits(0x7ff8_0000_0000_0000 | (neg as u64) << 63 | payload)
                } else if neg {
                    f64::NEG_INFINITY
                } else {
                    f64::INFINITY
                });
            }
        }
        let es = i;
        while i < bytes.len() && bytes[i].is_ascii_digit() {
            i += 1;
        }
        if i == es {
            return None; // `1e` with no exponent digits isn't a number
        }
    }
    if i != bytes.len() {
        return None;
    }
    // `1.` (dot but no fraction digits and no exponent) is an integer in
    // elisp — handled by parse_number's integer fallback.
    if saw_dot && frac_digits == 0 && !saw_exp {
        return None;
    }
    tok.parse::<f64>().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trailing_dot_is_integer_but_fraction_and_exponent_are_floats() {
        assert!(matches!(parse_number("123."), Some(Value::Int(123))));
        for token in ["1.e3", "1.5", ".5", "1e3", "+3.5"] {
            assert!(
                matches!(parse_number(token), Some(Value::Float(_))),
                "{token}"
            );
        }
    }

    #[test]
    fn decimal_integer_overflow_uses_bignums_without_accepting_partial_tokens() {
        assert!(matches!(
            parse_number("1606938044258990275541962092341162602522202993782792835301376"),
            Some(Value::BigInt(_))
        ));
        for token in ["", "+", ".", "1e", "12garbage", "0x10"] {
            assert!(parse_number(token).is_none(), "{token}");
        }
    }
}
