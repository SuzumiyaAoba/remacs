//! GNU version-string decomposition and comparison.

use crate::lisp::builtins::want_list;
use crate::lisp::{
    Interp,
    error::{EvalResult, Flow},
    obarray::sym,
    value::Value,
};
use num_bigint::BigInt;

// ---------- version strings ----------

fn version_list_of(i: &mut Interp, v: &Value) -> Result<Vec<BigInt>, Flow> {
    let s = match v {
        Value::Str(s) => s.borrow().clone(),
        _ => return Err(i.signal_data(sym::ERROR, vec![Value::string("Version must be a string")])),
    };
    let saved_match = i.match_data.clone();
    let bind_depth = i.specbind_depth();
    let case_fold = i.q("case-fold-search");
    let result = (|| {
        i.specbind(case_fold, Value::t())?;
        let separator_sym = i.q("version-separator");
        let separator = if i.bound_p(separator_sym) {
            match i.symbol_value(separator_sym) {
                Value::Str(separator) => separator.borrow().clone(),
                other => return Err(i.wrong_type_mut("stringp", &other)),
            }
        } else {
            crate::lisp::text::EmacsText::from(".")
        };
        let mut version = s.clone();
        if !separator.is_empty() && version.starts_with(&separator) {
            let mut prefixed = crate::lisp::text::EmacsText::from("0");
            prefixed.push_str(&version);
            version = prefixed;
        }
        let invalid_start = || {
            i.error_q(
                "Invalid version syntax: `%s' (must start with a number)",
                &[&version],
            )
        };
        if !version
            .chars()
            .next()
            .map(|c| c.is_ascii_digit())
            .unwrap_or(false)
        {
            return Err(invalid_start());
        }

        let mut out = Vec::new();
        let mut pos = 0usize;
        loop {
            let chars: Vec<crate::lisp::text::EmacsChar> = version.chars().collect();
            if pos >= chars.len() || !chars[pos].is_ascii_digit() {
                break;
            }
            let number_start = pos;
            while pos < chars.len() && chars[pos].is_ascii_digit() {
                pos += 1;
            }
            let number: crate::lisp::text::EmacsText = chars[number_start..pos].iter().collect();
            out.push(
                BigInt::parse_bytes(number.to_external_bytes().as_slice(), 10).unwrap_or_default(),
            );

            let suffix_start = pos;
            while pos < chars.len() && !chars[pos].is_ascii_digit() {
                pos += 1;
            }
            if suffix_start == pos {
                continue;
            }
            let suffix: String = chars[suffix_start..pos]
                .iter()
                .filter_map(|c| c.unicode())
                .collect();
            if suffix == separator {
                continue;
            }

            let alist_sym = i.q("version-regexp-alist");
            let alist = if i.bound_p(alist_sym) {
                i.symbol_value(alist_sym)
            } else {
                Value::Nil
            };
            let entries = match alist {
                Value::Nil => Vec::new(),
                Value::Cons(_) => want_list(i, &alist)?,
                other => return Err(i.wrong_type_mut("listp", &other)),
            };
            let string_match = Value::Sym(i.q("string-match"));
            let mut priority = None;
            for entry in entries {
                let Value::Cons(pair) = entry else {
                    continue;
                };
                let (regexp, value) = {
                    let pair = pair.borrow();
                    (pair.car.clone(), pair.cdr.clone())
                };
                let matched =
                    i.apply(&string_match, vec![regexp, Value::string(suffix.clone())])?;
                if matched.truthy() {
                    priority = Some(match value {
                        Value::Int(n) => BigInt::from(n),
                        Value::BigInt(n) => (*n).clone(),
                        other => return Err(i.wrong_type_mut("integerp", &other)),
                    });
                    break;
                }
            }
            let priority = priority.or_else(|| {
                if i.bound_p(alist_sym) {
                    return None;
                }
                let normalized = suffix
                    .strip_prefix(['-', '.', '_', '+', ' '])
                    .unwrap_or(&suffix)
                    .to_ascii_lowercase();
                match normalized.as_str() {
                    "snapshot" | "cvs" | "git" | "bzr" | "svn" | "hg" | "darcs" | "unknown" => {
                        Some(-4)
                    }
                    "alpha" => Some(-3),
                    "beta" => Some(-2),
                    "pre" | "rc" => Some(-1),
                    _ => None,
                }
                .map(BigInt::from)
            });
            if let Some(priority) = priority {
                out.push(priority);
            } else if pos == chars.len()
                && suffix
                    .strip_prefix(['-', '.', '_', '+', ' '])
                    .unwrap_or(&suffix)
                    .filter(|tail| tail.len() == 1)
                    .and_then(|tail| tail.chars().next())
                    .filter(|c| c.is_ascii_alphabetic())
                    .is_some()
            {
                let letter = suffix.chars().last().unwrap().to_ascii_lowercase() as i128;
                out.push(BigInt::from(letter - 'a' as i128 + 1));
            } else {
                return Err(i.error_q("Invalid version syntax: `%s'", &[&version]));
            }
        }
        Ok(out)
    })();
    let _ = i.unbind_to(bind_depth);
    i.match_data = saved_match;
    result
}

pub(super) fn f_version_to_list(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let v = version_list_of(i, &a[0])?;
    Ok(Value::list(v.into_iter().map(Value::from_bigint).collect()))
}

fn version_cmp(a: &[BigInt], b: &[BigInt]) -> std::cmp::Ordering {
    let n = a.len().max(b.len());
    let zero = BigInt::from(0);
    for k in 0..n {
        let x = a.get(k).unwrap_or(&zero);
        let y = b.get(k).unwrap_or(&zero);
        match x.cmp(&y) {
            std::cmp::Ordering::Equal => {}
            ord => return ord,
        }
    }
    std::cmp::Ordering::Equal
}

fn f_version_cmp(i: &mut Interp, a: &[Value]) -> Result<std::cmp::Ordering, Flow> {
    // `version</<=/=' take *strings* ("Version must be a string").
    Ok(version_cmp(
        &version_list_of(i, &a[0])?,
        &version_list_of(i, &a[1])?,
    ))
}

fn f_version_list_cmp(i: &mut Interp, a: &[Value]) -> Result<std::cmp::Ordering, Flow> {
    // `version-list-*' take int lists: listp on the arg, integerp
    // on elements.
    fn ints(i: &mut Interp, v: &Value) -> Result<Vec<BigInt>, Flow> {
        let items = match v {
            Value::Nil => Vec::new(),
            Value::Cons(_) => want_list(i, v)?,
            other => return Err(i.wrong_type_mut("listp", other)),
        };
        items
            .iter()
            .map(|e| match e {
                Value::Int(n) => Ok(BigInt::from(*n)),
                Value::BigInt(n) => Ok((**n).clone()),
                other => Err(i.wrong_type_mut("integerp", other)),
            })
            .collect()
    }
    Ok(version_cmp(&ints(i, &a[0])?, &ints(i, &a[1])?))
}

pub(super) fn f_version_lt(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    Ok(Value::from_bool(
        f_version_cmp(i, &a)? == std::cmp::Ordering::Less,
    ))
}
pub(super) fn f_version_le(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    Ok(Value::from_bool(
        f_version_cmp(i, &a)? != std::cmp::Ordering::Greater,
    ))
}
pub(super) fn f_version_eq(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    Ok(Value::from_bool(
        f_version_cmp(i, &a)? == std::cmp::Ordering::Equal,
    ))
}
pub(super) fn f_version_list_lt(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    Ok(Value::from_bool(
        f_version_list_cmp(i, &a)? == std::cmp::Ordering::Less,
    ))
}
pub(super) fn f_version_list_le(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    Ok(Value::from_bool(
        f_version_list_cmp(i, &a)? != std::cmp::Ordering::Greater,
    ))
}
pub(super) fn f_version_list_eq(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    Ok(Value::from_bool(
        f_version_list_cmp(i, &a)? == std::cmp::Ordering::Equal,
    ))
}
