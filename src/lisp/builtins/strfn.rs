//! String subrs: concat, substring, comparison, case, format, etc.

use super::{S, arg, want_int, want_string};
use crate::lisp::Interp;
use crate::lisp::error::{EvalResult, Flow};
use crate::lisp::obarray::sym;
use crate::lisp::value::{Subr, Value};

pub(crate) static SUBRS: &[Subr] = &[
    S!("string", many 0, f_string, "Concatenate characters into a string."),
    S!("concat", many 0, f_concat, "Concatenate sequences into a string."),
    S!("vconcat", many 0, f_vconcat, "Concatenate sequences into a vector."),
    S!(
        "substring",
        1,
        3,
        f_substring,
        "Substring of STRING from FROM to TO."
    ),
    S!(
        "substring-no-properties",
        1,
        3,
        f_substring,
        "Substring without text props."
    ),
    S!(
        "string=",
        2,
        2,
        f_string_eq,
        "t if two strings have identical contents."
    ),
    S!(
        "string<",
        2,
        2,
        f_string_lt,
        "t if S1 is less than S2 lexicographically."
    ),
    S!("string>", 2, 2, f_string_gt, "t if S1 is greater than S2."),
    S!("string<=", 2, 2, f_string_le, "t if S1 <= S2."),
    S!("string>=", 2, 2, f_string_ge, "t if S1 >= S2."),
    S!("string-lessp", 2, 2, f_string_lt, "Alias for string<."),
    S!("string-greaterp", 2, 2, f_string_gt, "t if S1 > S2."),
    S!(
        "string-empty-p",
        1,
        1,
        f_string_empty_p,
        "t if STRING has zero length."
    ),
    S!(
        "string-blank-p",
        1,
        1,
        f_string_blank_p,
        "t if STRING is all whitespace."
    ),
    S!(
        "string-prefix-p",
        2,
        3,
        f_string_prefix_p,
        "t if S1 is a prefix of S2."
    ),
    S!(
        "string-suffix-p",
        2,
        3,
        f_string_suffix_p,
        "t if S1 is a suffix of S2."
    ),
    S!(
        "string-compare",
        3,
        5,
        f_string_compare,
        "Compare substrings of S1 and S2."
    ),
    S!("upcase", 1, 1, f_upcase, "Uppercase a string or char."),
    S!("downcase", 1, 1, f_downcase, "Lowercase a string or char."),
    S!(
        "capitalize",
        1,
        1,
        f_capitalize,
        "Capitalize a string or char."
    ),
    S!(
        "upcase-initials",
        1,
        1,
        f_upcase_initials,
        "Uppercase each word's initial."
    ),
    S!(
        "string-to-number",
        1,
        2,
        f_string_to_number,
        "Parse NUMBER from STRING."
    ),
    S!(
        "number-to-string",
        1,
        1,
        f_number_to_string,
        "Return NUMBER as a string."
    ),
    S!(
        "string-to-char",
        1,
        1,
        f_string_to_char,
        "First char of STRING, or 0."
    ),
    S!(
        "char-to-string",
        1,
        1,
        f_char_to_string,
        "String containing CHAR."
    ),
    S!("format", many 1, f_format, "Format a string (printf-style)."),
    S!("format-message", many 1, f_format, "Format with quoting conventions."),
    S!(
        "string-trim",
        1,
        3,
        f_string_trim,
        "Trim STRING of TRIM regexps."
    ),
    S!("string-trim-left", 1, 2, f_string_trim_left, "Trim left."),
    S!(
        "string-trim-right",
        1,
        2,
        f_string_trim_right,
        "Trim right."
    ),
    S!("string-pad", 2, 4, f_string_pad, "Pad STRING to LENGTH."),
    S!(
        "string-join",
        1,
        2,
        f_string_join,
        "Join STRINGS with SEPARATOR."
    ),
    S!(
        "split-string",
        1,
        4,
        f_split_string,
        "Split STRING on SEPARATORS regexp."
    ),
    S!(
        "string-replace",
        3,
        3,
        f_string_replace,
        "Replace all FROM with TO."
    ),
    S!(
        "string-chop-newline",
        1,
        1,
        f_string_chop_newline,
        "Strip trailing newline."
    ),
    S!(
        "string-width",
        1,
        3,
        f_string_width,
        "Display width of STRING."
    ),
    S!(
        "truncate-string-to-width",
        2,
        4,
        f_truncate_string_to_width,
        "Truncate STRING to WIDTH columns."
    ),
    S!(
        "string-fill",
        2,
        2,
        f_string_fill,
        "Placeholder: return STRING."
    ),
    S!(
        "string-lines",
        1,
        2,
        f_string_lines,
        "Split STRING on newlines."
    ),
    S!("upcase-initials-region", 2, 2, f_region_stub, ""),
    S!(
        "string-to-multibyte",
        1,
        1,
        f_identity,
        "Return STRING unchanged."
    ),
    S!(
        "string-to-unibyte",
        1,
        1,
        f_identity,
        "Return STRING unchanged."
    ),
    S!(
        "string-as-unibyte",
        1,
        1,
        f_identity,
        "Return STRING unchanged."
    ),
    S!(
        "string-as-multibyte",
        1,
        1,
        f_identity,
        "Return STRING unchanged."
    ),
    S!(
        "string-make-unibyte",
        1,
        1,
        f_identity,
        "Return STRING unchanged."
    ),
    S!(
        "string-make-multibyte",
        1,
        1,
        f_identity,
        "Return STRING unchanged."
    ),
    S!(
        "string-equal-ignore-case",
        2,
        2,
        f_string_equal_ignore_case,
        "t if strings match ignoring case."
    ),
    S!(
        "string-collate-equalp",
        2,
        3,
        f_string_eq,
        "Collation equality (simple)."
    ),
    S!(
        "string-collate-lessp",
        2,
        3,
        f_string_lt,
        "Collation lessp (simple)."
    ),
    S!(
        "char-equal",
        2,
        2,
        f_char_equal,
        "t if two chars are equal (case-fold-aware)."
    ),
    S!("multibyte-char-to-unibyte", 1, 1, f_char_identity, ""),
    S!("unibyte-char-to-multibyte", 1, 1, f_char_identity, ""),
    S!("char-or-string-p", 1, 1, f_char_or_string_p, ""),
    S!(
        "string-search",
        2,
        3,
        f_string_search,
        "Search for NEEDLE in HAYSTACK."
    ),
    S!(
        "string-version-lessp",
        2,
        2,
        f_string_version_lessp,
        "Compare version strings."
    ),
    S!(
        "string-distance",
        2,
        3,
        f_string_distance,
        "Levenshtein distance."
    ),
    S!(
        "string-pixel-width",
        1,
        2,
        f_string_width,
        "Width (in columns here)."
    ),
    S!(
        "string-glyph-split",
        1,
        1,
        f_string_glyph_split,
        "Split into grapheme clusters (chars)."
    ),
    S!("sxhash-equal", 1, 1, f_sxhash, "Hash of OBJECT."),
    S!("sxhash", 1, 1, f_sxhash, "Hash of OBJECT."),
    S!("sxhash-eq", 1, 1, f_sxhash, "Hash of OBJECT."),
    S!("sxhash-eql", 1, 1, f_sxhash, "Hash of OBJECT."),
    S!("sxhash-equal-including-properties", 1, 1, f_sxhash, ""),
    S!(
        "clear-string",
        1,
        1,
        f_clear_string,
        "Make STRING empty (fill with NUL)."
    ),
    S!(
        "store-substring",
        3,
        3,
        f_store_substring,
        "Store OBJ into STRING at IDX."
    ),
    S!(
        "string-aref",
        2,
        2,
        f_string_aref,
        "Return char of STRING at IDX."
    ),
    S!(
        "make-string",
        2,
        2,
        f_make_string,
        "String of LENGTH copies of INIT char."
    ),
    S!(
        "string-to-list",
        1,
        1,
        f_string_to_list,
        "List of chars in STRING."
    ),
    S!(
        "string-to-vector",
        1,
        1,
        f_string_to_vector,
        "Vector of chars in STRING."
    ),
    S!(
        "string-bytes",
        1,
        2,
        f_string_bytes,
        "Number of bytes in STRING (utf-8)."
    ),
    S!(
        "subst-char-in-string",
        3,
        4,
        f_subst_char_in_string,
        "Replace FROM with TO in STRING."
    ),
    S!(
        "compare-strings",
        6,
        7,
        f_compare_strings,
        "Compare string slices."
    ),
    S!("char-width", 1, 1, f_char_width, "Display width of CHAR."),
    S!(
        "format-spec",
        2,
        3,
        f_format_spec,
        "Format string with %-specs from ALIST."
    ),
    S!(
        "multibyte-string-p",
        1,
        1,
        f_multibyte_string_p,
        "t if STRING is multibyte (always)."
    ),
    S!("unibyte-string", many 0, f_unibyte_string, "String from byte values."),
    S!("make-char", 1, 5, f_make_char, "Char for CHARSET + codes."),
    S!(
        "split-char",
        1,
        1,
        f_split_char,
        "Decompose CHAR into charset/code."
    ),
    S!(
        "encode-char",
        2,
        2,
        f_encode_char,
        "Code of CH in CODING-SYSTEM (utf-8 identity)."
    ),
    S!(
        "decode-char",
        2,
        2,
        f_decode_char,
        "Char for CODE in CODING-SYSTEM (utf-8)."
    ),
    S!("char-charset", 1, 2, f_char_charset, "Charset of CH."),
];

fn f_region_stub(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let _ = args;
    Err(i.error("region function not available outside a buffer"))
}

fn f_identity(_i: &mut Interp, args: Vec<Value>) -> EvalResult {
    Ok(args[0].clone())
}

fn f_char_identity(_i: &mut Interp, args: Vec<Value>) -> EvalResult {
    Ok(args[0].clone())
}

fn f_string(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let mut out = String::new();
    for a in &args {
        match a {
            Value::Int(n) => match char::from_u32(*n as u32) {
                Some(c) => out.push(c),
                None => return Err(i.wrong_type_mut("characterp", a)),
            },
            _ => return Err(i.wrong_type_mut("characterp", a)),
        }
    }
    Ok(Value::string(out))
}

/// Concatenate a sequence value's characters into `out`.
fn concat_seq(i: &mut Interp, v: &Value, out: &mut String) -> Result<(), super::Flow> {
    match v {
        Value::Nil => Ok(()),
        Value::Str(s) => {
            out.push_str(&s.borrow());
            Ok(())
        }
        Value::Cons(_) => {
            let items = super::want_list(i, v)?;
            for item in items {
                match item {
                    Value::Int(n) => match char::from_u32(n as u32) {
                        Some(c) => out.push(c),
                        None => return Err(i.wrong_type_mut("characterp", &item)),
                    },
                    _ => return Err(i.wrong_type_mut("characterp", &item)),
                }
            }
            Ok(())
        }
        Value::Vec(vec) => {
            for item in vec.borrow().iter() {
                match item {
                    Value::Int(n) => match char::from_u32(*n as u32) {
                        Some(c) => out.push(c),
                        None => return Err(i.wrong_type_mut("characterp", item)),
                    },
                    _ => return Err(i.wrong_type_mut("characterp", item)),
                }
            }
            Ok(())
        }
        _ => Err(i.wrong_type_mut("sequencep", v)),
    }
}

fn f_concat(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let mut out = String::new();
    for a in &args {
        concat_seq(i, a, &mut out)?;
    }
    Ok(Value::string(out))
}

fn f_vconcat(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let mut out: Vec<Value> = Vec::new();
    for a in &args {
        match a {
            Value::Nil => {}
            Value::Cons(_) => {
                let items = super::want_list(i, a)?;
                out.extend(items);
            }
            Value::Vec(v) => out.extend(v.borrow().iter().cloned()),
            Value::Str(s) => {
                for c in s.borrow().chars() {
                    out.push(Value::Int(c as i128));
                }
            }
            other => return Err(i.wrong_type_mut("sequencep", other)),
        }
    }
    Ok(Value::Vec(std::rc::Rc::new(std::cell::RefCell::new(out))))
}

fn f_substring(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    // Emacs: substring works on vectors too (returns a new vector).
    let is_vec = matches!(&args[0], Value::Vec(_));
    let chars: Vec<Value> = match &args[0] {
        Value::Str(s) => s.borrow().chars().map(|c| Value::Int(c as i128)).collect(),
        Value::Vec(v) => v.borrow().clone(),
        Value::Nil => Vec::new(),
        other => return Err(i.wrong_type_mut("sequencep", other)),
    };
    let len = chars.len() as i128;
    let mut int_or = |v: Option<&Value>, d: i128| -> Result<i128, Flow> {
        match v {
            None | Some(Value::Nil) => Ok(d),
            Some(v) => want_int(i, v),
        }
    };
    let from = int_or(args.get(1), 0)?;
    let to = int_or(args.get(2), len)?;
    // Emacs allows negative indices counting from the end.
    let f = if from < 0 { len + from } else { from };
    let t = if to < 0 { len + to } else { to };
    if f < 0 || t < f || t > len {
        return Err(i.signal_data(
            sym::ARGS_OUT_OF_RANGE,
            vec![args[0].clone(), Value::Int(f), Value::Int(t)],
        ));
    }
    let slice: Vec<Value> = chars[f as usize..t as usize].to_vec();
    if is_vec {
        Ok(Value::Vec(std::rc::Rc::new(std::cell::RefCell::new(slice))))
    } else {
        Ok(Value::string(
            slice
                .iter()
                .filter_map(|v| match v {
                    Value::Int(n) => char::from_u32(*n as u32),
                    _ => None,
                })
                .collect::<String>(),
        ))
    }
}

fn f_string_eq(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let a = want_string(i, &args[0])?;
    let b = want_string(i, &args[1])?;
    Ok(Value::from_bool(a == b))
}
fn f_string_lt(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let a = want_string(i, &args[0])?;
    let b = want_string(i, &args[1])?;
    Ok(Value::from_bool(a < b))
}
fn f_string_gt(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let a = want_string(i, &args[0])?;
    let b = want_string(i, &args[1])?;
    Ok(Value::from_bool(a > b))
}
fn f_string_le(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let a = want_string(i, &args[0])?;
    let b = want_string(i, &args[1])?;
    Ok(Value::from_bool(a <= b))
}
fn f_string_ge(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let a = want_string(i, &args[0])?;
    let b = want_string(i, &args[1])?;
    Ok(Value::from_bool(a >= b))
}
fn f_string_empty_p(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    match &args[0] {
        Value::Nil => Ok(Value::t()),
        Value::Str(s) => Ok(Value::from_bool(s.borrow().is_empty())),
        other => Err(i.wrong_type_mut("stringp", other)),
    }
}
fn f_string_blank_p(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    // Emacs (subr-x): returns the match position 0 or nil.
    match &args[0] {
        Value::Str(s) => Ok(if s.borrow().chars().all(|c| c.is_whitespace()) {
            Value::Int(0)
        } else {
            Value::Nil
        }),
        other => Err(i.wrong_type_mut("stringp", other)),
    }
}
fn f_string_prefix_p(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let a = want_string(i, &args[0])?;
    let b = want_string(i, &args[1])?;
    let ic = args.get(2).map(|v| v.truthy()).unwrap_or(false);
    Ok(Value::from_bool(if ic {
        b.to_lowercase().starts_with(&a.to_lowercase())
    } else {
        b.starts_with(&a)
    }))
}
fn f_string_suffix_p(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let a = want_string(i, &args[0])?;
    let b = want_string(i, &args[1])?;
    let ic = args.get(2).map(|v| v.truthy()).unwrap_or(false);
    Ok(Value::from_bool(if ic {
        b.to_lowercase().ends_with(&a.to_lowercase())
    } else {
        b.ends_with(&a)
    }))
}
fn f_string_compare(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let a = want_string(i, &args[0])?;
    let b = want_string(i, &args[1])?;
    Ok(if a < b {
        Value::Int(-1)
    } else if a > b {
        Value::Int(1)
    } else {
        Value::t()
    })
}
fn f_upcase(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    match &args[0] {
        Value::Str(s) => Ok(Value::string(s.borrow().to_uppercase())),
        Value::Int(n) => {
            let c = char::from_u32(*n as u32).unwrap_or('\0');
            Ok(Value::Int(c.to_uppercase().next().unwrap_or(c) as i128))
        }
        other => Err(i.wrong_type_mut("char-or-string-p", other)),
    }
}
fn f_downcase(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    match &args[0] {
        Value::Str(s) => Ok(Value::string(s.borrow().to_lowercase())),
        Value::Int(n) => {
            let c = char::from_u32(*n as u32).unwrap_or('\0');
            Ok(Value::Int(c.to_lowercase().next().unwrap_or(c) as i128))
        }
        other => Err(i.wrong_type_mut("char-or-string-p", other)),
    }
}
fn f_capitalize(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    match &args[0] {
        Value::Str(s) => {
            let mut out = String::new();
            let mut new_word = true;
            for c in s.borrow().chars() {
                if c.is_alphanumeric() {
                    if new_word {
                        out.extend(c.to_uppercase());
                        new_word = false;
                    } else {
                        out.extend(c.to_lowercase());
                    }
                } else {
                    new_word = true;
                    out.push(c);
                }
            }
            Ok(Value::string(out))
        }
        Value::Int(n) => {
            let c = char::from_u32(*n as u32).unwrap_or('\0');
            Ok(Value::Int(c.to_uppercase().next().unwrap_or(c) as i128))
        }
        other => Err(i.wrong_type_mut("char-or-string-p", other)),
    }
}
fn f_upcase_initials(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    match &args[0] {
        Value::Str(s) => {
            let mut out = String::new();
            let mut new_word = true;
            for c in s.borrow().chars() {
                if c.is_alphanumeric() {
                    if new_word {
                        out.extend(c.to_uppercase());
                        new_word = false;
                    } else {
                        out.push(c);
                    }
                } else {
                    new_word = true;
                    out.push(c);
                }
            }
            Ok(Value::string(out))
        }
        other => Err(i.wrong_type_mut("stringp", other)),
    }
}

fn f_string_to_number(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let s = want_string(i, &args[0])?;
    let base = args
        .get(1)
        .map(|v| want_int(i, v))
        .transpose()?
        .unwrap_or(10);
    let t = s.trim();
    if t.is_empty() {
        return Ok(Value::Int(0));
    }
    // Emacs: BASE is 0 (auto) or 2..16.
    if base != 0 && !(2..=16).contains(&base) {
        let sym = i.intern("args-out-of-range");
        return Err(i.signal_data(sym, vec![Value::Int(base)]));
    }
    let base = if base == 0 { 10 } else { base };
    if base != 10 {
        let neg = t.starts_with('-');
        let digits = t.trim_start_matches(['+', '-']);
        // Parse the longest valid digit prefix for the base.
        let take: String = digits
            .chars()
            .take_while(|c| c.to_digit(base as u32).is_some())
            .collect();
        let v = i128::from_str_radix(&take, base as u32)
            .map(|n| if neg { -n } else { n })
            .unwrap_or(0);
        return Ok(Value::Int(v));
    }
    // Emacs parses the longest valid numeric prefix ("10abc" -> 10).
    let prefix = number_prefix(t);
    if prefix.is_empty() {
        return Ok(Value::Int(0));
    }
    match super::super::reader::parse_number(&prefix) {
        Some(v) => Ok(v),
        None => Ok(Value::Int(0)),
    }
}

/// The longest valid [+-]? digits [. digits] [eE [+-] digits] prefix
/// of `t`, or "" if it doesn't start with a number.
fn number_prefix(t: &str) -> String {
    let cs: Vec<char> = t.chars().collect();
    let mut p = 0usize;
    if matches!(cs.first(), Some('+') | Some('-')) {
        p = 1;
    }
    let int_digits = scan_digits(&cs, p);
    p += int_digits;
    let mut end = p;
    let mut float = false;
    // Fractional part: `.' followed by at least one digit, OR bare `.'
    // after int digits (which just ends the int in Emacs).
    if cs.get(p) == Some(&'.') {
        let frac = scan_digits(&cs, p + 1);
        if frac > 0 {
            p += 1 + frac;
            end = p;
            float = true;
        }
    }
    if int_digits == 0 && !float {
        return String::new();
    }
    // Exponent: e[+-]?digits — only consumed when digits follow.
    if matches!(cs.get(p), Some('e') | Some('E')) {
        let mut q = p + 1;
        if matches!(cs.get(q), Some('+') | Some('-')) {
            q += 1;
        }
        let ed = scan_digits(&cs, q);
        if ed > 0 {
            end = q + ed;
        }
    }
    cs[..end].iter().collect()
}

fn scan_digits(cs: &[char], mut p: usize) -> usize {
    let start = p;
    while matches!(cs.get(p), Some(d) if d.is_ascii_digit()) {
        p += 1;
    }
    p - start
}
fn f_number_to_string(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    match &args[0] {
        Value::Int(n) => Ok(Value::string(n.to_string())),
        Value::Float(f) => Ok(Value::string(crate::lisp::print::format_float(*f))),
        other => Err(i.wrong_type_mut("numberp", other)),
    }
}
fn f_string_to_char(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let s = want_string(i, &args[0])?;
    Ok(Value::Int(s.chars().next().map(|c| c as i128).unwrap_or(0)))
}
fn f_char_to_string(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let n = want_int(i, &args[0])?;
    let c = char::from_u32((n & 0x3f_ffff) as u32).unwrap_or('\0');
    Ok(Value::string(c.to_string()))
}
/// `string-trim-left/right` use regexps (subr-x): strip the longest
/// prefix/suffix matching `(?:TRIM)+`. TRIM defaults to whitespace.
fn trim_re(i: &mut Interp, args: &[Value], idx: usize) -> Result<String, Flow> {
    args.get(idx)
        .map(|v| want_string(i, v))
        .transpose()
        .map(|o| o.unwrap_or_else(|| "[ \t\n\r]+".into()))
}

fn f_string_trim(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let s = want_string(i, &args[0])?;
    let left = trim_re(i, &args, 1)?;
    let right = trim_re(i, &args, 2)?;
    let after_left = trim_left_re(i, &s, &left)?;
    Ok(Value::string(trim_right_re(i, &after_left, &right)?))
}
fn f_string_trim_left(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let s = want_string(i, &args[0])?;
    let re = trim_re(i, &args, 1)?;
    Ok(Value::string(trim_left_re(i, &s, &re)?))
}
fn f_string_trim_right(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let s = want_string(i, &args[0])?;
    let re = trim_re(i, &args, 1)?;
    Ok(Value::string(trim_right_re(i, &s, &re)?))
}

fn trim_left_re(i: &mut Interp, s: &str, trim: &str) -> Result<String, Flow> {
    let re = crate::lisp::regexp::compile(&format!("\\`\\(?:{}\\)+", trim))
        .map_err(|_| i.error("Invalid regexp"))?;
    let chars: Vec<char> = s.chars().collect();
    match crate::lisp::regexp::looking_at(&re, &chars, 0) {
        Some(regs) => {
            let end = regs.get(1).copied().flatten().unwrap_or(0);
            Ok(chars[end..].iter().collect())
        }
        None => Ok(s.to_string()),
    }
}

fn trim_right_re(i: &mut Interp, s: &str, trim: &str) -> Result<String, Flow> {
    let re = crate::lisp::regexp::compile(&format!("\\(?:{}\\)+\\'", trim))
        .map_err(|_| i.error("Invalid regexp"))?;
    let chars: Vec<char> = s.chars().collect();
    match crate::lisp::regexp::search(&re, &chars, 0) {
        Some((ms, _me)) => Ok(chars[..ms].iter().collect()),
        None => Ok(s.to_string()),
    }
}
fn f_string_pad(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let s = want_string(i, &args[0])?;
    let len = want_int(i, &args[1])?.max(0) as usize;
    let pad_char = args
        .get(2)
        .and_then(|v| match v {
            Value::Int(n) => char::from_u32(*n as u32),
            _ => None,
        })
        .unwrap_or(' ');
    let pad_start = args.get(3).map(|v| v.truthy()).unwrap_or(false);
    let cur = s.chars().count();
    if cur >= len {
        return Ok(Value::string(s));
    }
    let pad: String = std::iter::repeat(pad_char).take(len - cur).collect();
    Ok(Value::string(if pad_start {
        format!("{}{}", pad, s)
    } else {
        format!("{}{}", s, pad)
    }))
}
fn f_string_join(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let items = super::want_list(i, &args[0])?;
    let sep = args
        .get(1)
        .map(|v| want_string(i, v))
        .transpose()?
        .unwrap_or_default();
    let mut parts = Vec::new();
    for item in items {
        parts.push(want_string(i, &item)?);
    }
    Ok(Value::string(parts.join(&sep)))
}
fn f_split_string(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let s = want_string(i, &args[0])?;
    let sep = match args.get(1) {
        None | Some(Value::Nil) => "[ \u{0C}\t\n\r\u{0B}]+".into(),
        Some(v) => want_string(i, v)?,
    };
    let omit_nulls = args.get(2).map(|v| v.truthy()).unwrap_or(false);
    // If sep is a regex-ish, use our regexp engine; else literal split.
    let is_regex = sep.contains('\\')
        || sep.contains('[')
        || sep.contains('^')
        || sep.contains('$')
        || sep.contains('.')
        || sep.contains('*')
        || sep.contains('+')
        || sep.contains('?');
    let parts: Vec<String> = if is_regex {
        match crate::lisp::regexp::compile(&sep) {
            Ok(re) => {
                let mut out = Vec::new();
                let mut pos = 0usize;
                let chars: Vec<char> = s.chars().collect();
                while pos <= chars.len() {
                    match crate::lisp::regexp::search(&re, &chars, pos) {
                        Some((ms, me)) => {
                            out.push(chars[pos..ms].iter().collect());
                            pos = me.max(ms + 1);
                            if ms == me && pos > chars.len() {
                                break;
                            }
                        }
                        None => {
                            out.push(chars[pos..].iter().collect());
                            break;
                        }
                    }
                }
                out
            }
            Err(_) => s.split(&sep as &str).map(|x| x.to_string()).collect(),
        }
    } else {
        s.split(&sep as &str).map(|x| x.to_string()).collect()
    };
    let filtered: Vec<Value> = parts
        .into_iter()
        .filter(|p| !omit_nulls || !p.is_empty())
        .map(Value::string)
        .collect();
    Ok(Value::list(filtered))
}
fn f_string_replace(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let from = want_string(i, &args[0])?;
    let to = want_string(i, &args[1])?;
    let s = want_string(i, &args[2])?;
    if from.is_empty() {
        return Ok(Value::string(s));
    }
    Ok(Value::string(s.replace(&from, &to)))
}
fn f_string_chop_newline(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let s = want_string(i, &args[0])?;
    Ok(Value::string(s.trim_end_matches('\n').to_string()))
}
fn f_string_width(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    use unicode_width::UnicodeWidthChar;
    let s = match &args[0] {
        Value::Str(s) => s.borrow().clone(),
        Value::Nil => String::new(),
        other => return Err(i.wrong_type_mut("stringp", other)),
    };
    let (from, to) = (
        args.get(1)
            .map(|v| want_int(i, v))
            .transpose()?
            .unwrap_or(0),
        args.get(2)
            .map(|v| want_int(i, v))
            .transpose()?
            .unwrap_or(-1),
    );
    let chars: Vec<char> = s.chars().collect();
    let t = if to < 0 { chars.len() } else { to as usize };
    let w: usize = chars[from as usize..t.min(chars.len())]
        .iter()
        .map(|c| {
            if *c == '\t' || *c == '\n' || *c == 0x7f as char {
                0
            } else {
                UnicodeWidthChar::width(*c).unwrap_or(0)
            }
        })
        .sum();
    Ok(Value::Int(w as i128))
}
fn f_truncate_string_to_width(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let s = want_string(i, &args[0])?;
    let width = want_int(i, &args[1])?.max(0) as usize;
    use unicode_width::UnicodeWidthChar;
    let mut out = String::new();
    let mut w = 0;
    for c in s.chars() {
        let cw = UnicodeWidthChar::width(c).unwrap_or(0);
        if w + cw > width {
            break;
        }
        out.push(c);
        w += cw;
    }
    Ok(Value::string(out))
}
fn f_string_fill(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let _ = i;
    Ok(args[0].clone())
}
fn f_string_lines(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let s = want_string(i, &args[0])?;
    Ok(Value::list(
        s.split('\n').map(|l| Value::string(l)).collect(),
    ))
}
fn f_string_equal_ignore_case(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let a = want_string(i, &args[0])?;
    let b = want_string(i, &args[1])?;
    Ok(Value::from_bool(a.to_lowercase() == b.to_lowercase()))
}
fn f_char_equal(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let a = match &args[0] {
        Value::Int(n) => *n,
        v => return Err(i.wrong_type_mut("characterp", v)),
    };
    let b = match &args[1] {
        Value::Int(n) => *n,
        v => return Err(i.wrong_type_mut("characterp", v)),
    };
    // Emacs consults `case-fold-search' (default t in -Q).
    let fold = i
        .symbol_value(i.intern_soft("case-fold-search").unwrap_or(0))
        .truthy();
    let eq = a == b
        || (fold
            && char::from_u32(a as u32).and_then(|c| c.to_lowercase().next())
                == char::from_u32(b as u32).and_then(|c| c.to_lowercase().next()));
    Ok(Value::from_bool(eq))
}
fn f_char_or_string_p(_i: &mut Interp, args: Vec<Value>) -> EvalResult {
    Ok(Value::from_bool(matches!(
        &args[0],
        Value::Int(_) | Value::Str(_)
    )))
}
fn f_string_search(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let needle = want_string(i, &args[0])?;
    let hay = want_string(i, &args[1])?;
    let start = args
        .get(2)
        .map(|v| want_int(i, v))
        .transpose()?
        .unwrap_or(0);
    let hchars: Vec<char> = hay.chars().collect();
    let nchars: Vec<char> = needle.chars().collect();
    if nchars.is_empty() {
        return Ok(Value::Int(start.min(hchars.len() as i128)));
    }
    let mut idx = start.max(0) as usize;
    while idx + nchars.len() <= hchars.len() {
        if hchars[idx..idx + nchars.len()] == nchars[..] {
            return Ok(Value::Int(idx as i128));
        }
        idx += 1;
    }
    Ok(Value::Nil)
}
fn f_string_version_lessp(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let a = want_string(i, &args[0])?;
    let b = want_string(i, &args[1])?;
    // Simple component-wise compare of digits.
    let parse = |s: &str| -> Vec<i128> {
        s.split(|c: char| !c.is_ascii_digit())
            .filter(|p| !p.is_empty())
            .filter_map(|p| p.parse().ok())
            .collect()
    };
    let (va, vb) = (parse(&a), parse(&b));
    Ok(Value::from_bool(va < vb))
}
fn f_string_distance(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let a: Vec<char> = want_string(i, &args[0])?.chars().collect();
    let b: Vec<char> = want_string(i, &args[1])?.chars().collect();
    let (n, m) = (a.len(), b.len());
    if n == 0 {
        return Ok(Value::Int(m as i128));
    }
    if m == 0 {
        return Ok(Value::Int(n as i128));
    }
    let mut prev: Vec<usize> = (0..=m).collect();
    let mut cur = vec![0; m + 1];
    for x in 1..=n {
        cur[0] = x;
        for y in 1..=m {
            let cost = if a[x - 1] == b[y - 1] { 0 } else { 1 };
            cur[y] = (prev[y] + 1).min(cur[y - 1] + 1).min(prev[y - 1] + cost);
        }
        std::mem::swap(&mut prev, &mut cur);
    }
    Ok(Value::Int(prev[m] as i128))
}
fn f_string_glyph_split(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let s = want_string(i, &args[0])?;
    Ok(Value::list(
        s.chars().map(|c| Value::string(c.to_string())).collect(),
    ))
}
fn f_sxhash(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::Hasher;
    let mut h = DefaultHasher::new();
    hash_value(i, &args[0], &mut h, 0);
    Ok(Value::Int((h.finish() & (i128::MAX as u64)) as i128))
}

fn hash_value(i: &Interp, v: &Value, h: &mut impl std::hash::Hasher, depth: usize) {
    use std::hash::Hash;
    if depth > 32 {
        return;
    }
    match v {
        Value::Nil => 0u8.hash(h),
        Value::Int(n) => n.hash(h),
        Value::Float(f) => f.to_bits().hash(h),
        Value::Sym(id) => i.symbol_name(*id).hash(h),
        Value::Str(s) => s.borrow().hash(h),
        Value::Cons(c) => {
            let b = c.borrow();
            hash_value(i, &b.car, h, depth + 1);
            hash_value(i, &b.cdr, h, depth + 1);
        }
        Value::Vec(vec) => {
            for item in vec.borrow().iter() {
                hash_value(i, item, h, depth + 1);
            }
        }
        _ => format!("{:?}", v).hash(h),
    }
}
fn f_clear_string(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    match &args[0] {
        Value::Str(s) => {
            s.borrow_mut().clear();
            Ok(args[0].clone())
        }
        other => Err(i.wrong_type_mut("stringp", other)),
    }
}
fn f_store_substring(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let idx = want_int(i, &args[1])?;
    match &args[0] {
        Value::Str(s) => {
            let mut chars: Vec<char> = s.borrow().chars().collect();
            let rep: Vec<char> = match &args[2] {
                Value::Str(rs) => rs.borrow().chars().collect(),
                Value::Int(n) => vec![char::from_u32(*n as u32).unwrap_or('\0')],
                other => return Err(i.wrong_type_mut("char-or-string-p", other)),
            };
            let iu = idx.max(0) as usize;
            for (k, c) in rep.iter().enumerate() {
                if iu + k < chars.len() {
                    chars[iu + k] = *c;
                }
            }
            *s.borrow_mut() = chars.into_iter().collect();
            Ok(args[2].clone())
        }
        other => Err(i.wrong_type_mut("stringp", other)),
    }
}
fn f_string_aref(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let chars: Vec<char> = want_string(i, &args[0])?.chars().collect();
    let idx = want_int(i, &args[1])?;
    if idx < 0 || idx as usize >= chars.len() {
        return Err(i.signal_data(
            sym::ARGS_OUT_OF_RANGE,
            vec![args[0].clone(), Value::Int(idx)],
        ));
    }
    Ok(Value::Int(chars[idx as usize] as i128))
}

// ---------- format ----------

/// `format` — printf-style with elisp conventions:
/// `%s` princ, `%S` prin1, `%d`/`%o`/`%x`/`%X`/`%e`/`%f`/`%g` numeric,
/// `%c` char, `%%` literal. Supports `%Nd`, `%-Ns`, `%0Nd`, `%.Nf`.
fn f_format(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let fmt = want_string(i, &args[0])?;
    let mut out = String::new();
    let fchars: Vec<char> = fmt.chars().collect();
    let mut ai = 1usize; // next arg index
    let mut p = 0usize;
    while p < fchars.len() {
        let c = fchars[p];
        if c != '%' {
            out.push(c);
            p += 1;
            continue;
        }
        p += 1;
        if p >= fchars.len() {
            out.push('%');
            break;
        }
        // Positional arg: `%N$spec` uses arg N (1-based) without
        // advancing the sequential arg counter.
        let mut pos_arg: Option<usize> = None;
        {
            let mut q = p;
            let mut n = 0usize;
            while let Some(d) = fchars.get(q) {
                if d.is_ascii_digit() {
                    n = n * 10 + (*d as usize - '0' as usize);
                    q += 1;
                } else {
                    break;
                }
            }
            if q > p && fchars.get(q) == Some(&'$') {
                pos_arg = Some(n);
                p = q + 1;
            }
        }
        // parse spec: [-+ #0]*[width][.precision]<letter>
        let mut left = false;
        let mut pad0 = false;
        let mut plus = false;
        let mut space_sign = false;
        loop {
            match fchars.get(p) {
                Some('-') => {
                    left = true;
                    p += 1;
                }
                Some('0') => {
                    pad0 = true;
                    p += 1;
                }
                Some('+') => {
                    plus = true;
                    p += 1;
                }
                Some(' ') => {
                    space_sign = true;
                    p += 1;
                }
                Some('#') => {
                    p += 1;
                }
                _ => break,
            }
        }
        let mut width: Option<usize> = None;
        while let Some(d) = fchars.get(p) {
            if d.is_ascii_digit() {
                width = Some(width.unwrap_or(0) * 10 + (*d as usize - '0' as usize));
                p += 1;
            } else {
                break;
            }
        }
        let mut prec: Option<usize> = None;
        if fchars.get(p) == Some(&'.') {
            p += 1;
            let mut n = 0usize;
            while let Some(d) = fchars.get(p) {
                if d.is_ascii_digit() {
                    n = n * 10 + (*d as usize - '0' as usize);
                    p += 1;
                } else {
                    break;
                }
            }
            prec = Some(n);
        }
        let letter = match fchars.get(p) {
            Some(l) => *l,
            None => {
                out.push('%');
                break;
            }
        };
        p += 1;
        if letter == '%' {
            out.push('%');
            continue;
        }
        let a = match pos_arg {
            Some(n) => arg(&args, n),
            None => arg(&args, ai),
        };
        if pos_arg.is_none() {
            ai += 1;
        }
        let piece = match letter {
            's' => {
                let s = i.princ_to_string(&a);
                // Precision truncates the printed argument.
                match prec {
                    Some(n) => s.chars().take(n).collect(),
                    None => s,
                }
            }
            'S' => {
                let s = i.print_to_string(&a);
                match prec {
                    Some(n) => s.chars().take(n).collect(),
                    None => s,
                }
            }
            'd' | 'i' => {
                let n = match &a {
                    Value::Int(n) => *n,
                    Value::Float(f) => *f as i128,
                    Value::Marker(m) => m.borrow().position as i128 + 1,
                    _ => return Err(i.wrong_type_mut("integerp", &a)),
                };
                let mut s = n.to_string();
                if n >= 0 {
                    if plus {
                        s = format!("+{}", s);
                    } else if space_sign {
                        s = format!(" {}", s);
                    }
                }
                s
            }
            'c' => {
                let n = match &a {
                    Value::Int(n) => *n,
                    _ => return Err(i.wrong_type_mut("integerp", &a)),
                };
                let ch = char::from_u32((n & 0x3f_ffff) as u32).unwrap_or('\0');
                ch.to_string()
            }
            'o' => {
                let n = int_of(i, &a)?;
                format!("{:o}", n)
            }
            'x' => {
                let n = int_of(i, &a)?;
                format!("{:x}", n)
            }
            'X' => {
                let n = int_of(i, &a)?;
                format!("{:X}", n)
            }
            'e' | 'E' => {
                let f = float_of(i, &a)?;
                format_e(f, prec.unwrap_or(6), letter == 'E')
            }
            'f' => {
                let f = float_of(i, &a)?;
                format!("{:.*}", prec.unwrap_or(6), f)
            }
            'g' => {
                let f = float_of(i, &a)?;
                format_g(f, prec)
            }
            _ => {
                // Unsupported spec — Emacs signals "Invalid format operation".
                return Err(i.error(&format!("Invalid format operation %{}", letter)));
            }
        };
        // apply width/alignment
        let plen = piece.chars().count();
        let padded = match width {
            Some(w) if plen < w => {
                let pad = w - plen;
                if left {
                    format!("{}{}", piece, " ".repeat(pad))
                } else if pad0 && matches!(letter, 'd' | 'i' | 'o' | 'x' | 'X' | 'e' | 'f' | 'g') {
                    // zero-pad after any sign
                    if piece.starts_with('-') || piece.starts_with('+') || piece.starts_with(' ') {
                        let (sign, rest) = piece.split_at(1);
                        format!("{}{}{}", sign, "0".repeat(pad), rest)
                    } else {
                        format!("{}{}", "0".repeat(pad), piece)
                    }
                } else {
                    format!("{}{}", " ".repeat(pad), piece)
                }
            }
            _ => piece,
        };
        out.push_str(&padded);
    }
    Ok(Value::string(out))
}

/// C-printf-style %e: signed exponent with ≥2 digits ("1.000000e+03").
fn format_e(f: f64, prec: usize, upper: bool) -> String {
    let s = format!("{:.*e}", prec, f);
    if let Some(epos) = s.find('e') {
        let (mant, exp) = s.split_at(epos);
        let exp_num: i128 = exp[1..].parse().unwrap_or(0);
        let out = format!(
            "{}e{}{:02}",
            mant,
            if exp_num < 0 { "-" } else { "+" },
            exp_num.abs()
        );
        if upper { out.to_uppercase() } else { out }
    } else {
        s
    }
}

fn int_of(i: &mut Interp, v: &Value) -> Result<i128, super::Flow> {
    match v {
        Value::Int(n) => Ok(*n),
        Value::Float(f) => Ok(*f as i128),
        _ => Err(i.wrong_type_mut("numberp", v)),
    }
}

fn float_of(i: &mut Interp, v: &Value) -> Result<f64, super::Flow> {
    match v {
        Value::Int(n) => Ok(*n as f64),
        Value::Float(f) => Ok(*f),
        _ => Err(i.wrong_type_mut("numberp", v)),
    }
}

fn format_g(f: f64, prec: Option<usize>) -> String {
    let p = prec.unwrap_or(6).max(1);
    if f == 0.0 {
        return "0".to_string();
    }
    let e = f.abs().log10().floor() as i32;
    if e < -4 || e >= p as i32 {
        let s = format!("{:.*e}", p - 1, f);
        // trim trailing zeros in mantissa; exponent gets a sign and ≥2 digits
        if let Some(epos) = s.find('e') {
            let (m, ex) = s.split_at(epos);
            let m2 = m.trim_end_matches('0').trim_end_matches('.');
            let exp: i32 = ex[1..].parse().unwrap_or(0);
            format!("{}e{}{:02}", m2, if exp < 0 { "-" } else { "+" }, exp.abs())
        } else {
            s
        }
    } else {
        let decimals = (p as i32 - 1 - e).max(0) as usize;
        let mut s = format!("{:.*}", decimals, f);
        if s.contains('.') {
            s = s.trim_end_matches('0').trim_end_matches('.').to_string();
        }
        s
    }
}

// ---------- added builtins ----------

fn f_make_string(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let n = want_int(i, &args[0])?.max(0) as usize;
    let c = want_int(i, &args[1])?;
    let ch = char::from_u32(c as u32).unwrap_or('\u{0}');
    let s: String = std::iter::repeat_n(ch, n).collect();
    Ok(Value::string(s))
}

fn f_string_to_list(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let s = want_string(i, &args[0])?;
    let items: Vec<Value> = s.chars().map(|c| Value::Int(c as i128)).collect();
    Ok(Value::list(items))
}

fn f_string_to_vector(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    // Emacs accepts nil (treated as the empty sequence).
    let s = match &args[0] {
        Value::Nil => String::new(),
        v => want_string(i, v)?,
    };
    let items: Vec<Value> = s.chars().map(|c| Value::Int(c as i128)).collect();
    Ok(Value::Vec(std::rc::Rc::new(std::cell::RefCell::new(items))))
}

fn f_string_bytes(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let s = want_string(i, &args[0])?;
    Ok(Value::Int(s.len() as i128))
}

fn f_subst_char_in_string(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let from = want_int(i, &args[0])? as u32;
    let to = want_int(i, &args[1])? as u32;
    let inplace = arg(&args, 3).truthy();
    let from_c = char::from_u32(from).unwrap_or('\u{FFFD}');
    let to_c = char::from_u32(to).unwrap_or('\u{FFFD}');
    match &args[2] {
        Value::Str(s) => {
            if inplace {
                let mut b = s.borrow_mut();
                let replaced: String = b
                    .chars()
                    .map(|c| if c == from_c { to_c } else { c })
                    .collect();
                *b = replaced;
                return Ok(args[2].clone());
            }
            let b = s.borrow();
            let replaced: String = b
                .chars()
                .map(|c| if c == from_c { to_c } else { c })
                .collect();
            Ok(Value::string(replaced))
        }
        other => Err(i.wrong_type_mut("stringp", other)),
    }
}

fn f_compare_strings(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    // (compare-strings STR1 START1 END1 STR2 START2 END2 &optional IGNORE-CASE)
    let s1 = str_or_sym_name(i, &args[0])?;
    let s2 = str_or_sym_name(i, &args[3])?;
    let v1: Vec<char> = s1.chars().collect();
    let v2: Vec<char> = s2.chars().collect();
    let b1 = bound(v1.len(), &args[1], 0)?;
    let e1 = bound(v1.len(), &args[2], v1.len())?;
    let b2 = bound(v2.len(), &args[4], 0)?;
    let e2 = bound(v2.len(), &args[5], v2.len())?;
    let fold = arg(&args, 6).truthy();
    let a: String = v1[b1..e1].iter().collect();
    let b: String = v2[b2..e2].iter().collect();
    let (a, b) = if fold {
        (a.to_lowercase(), b.to_lowercase())
    } else {
        (a, b)
    };
    // Find first differing position.
    let ac: Vec<char> = a.chars().collect();
    let bc: Vec<char> = b.chars().collect();
    let n = ac.len().min(bc.len());
    for k in 0..n {
        if ac[k] != bc[k] {
            let pos = k as i128 + 1;
            return Ok(Value::Int(if ac[k] < bc[k] { -pos } else { pos }));
        }
    }
    if ac.len() == bc.len() {
        return Ok(Value::t());
    }
    let pos = n as i128 + 1;
    Ok(Value::Int(if ac.len() < bc.len() { -pos } else { pos }))
}

fn bound(len: usize, v: &Value, default: usize) -> Result<usize, Flow> {
    match v {
        Value::Nil => Ok(default),
        Value::Int(n) => Ok((*n).max(0).min(len as i128) as usize),
        _ => Ok(default),
    }
}

fn str_or_sym_name(i: &mut Interp, v: &Value) -> Result<String, Flow> {
    match v {
        Value::Str(s) => Ok(s.borrow().clone()),
        Value::Sym(id) => Ok(i.symbol_name(*id)),
        _ => Err(i.wrong_type_mut("char-or-string-p", v)),
    }
}

fn f_char_width(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let _ = i;
    let c = match &args[0] {
        Value::Int(n) => *n as u32,
        _ => 0,
    };
    let ch = char::from_u32(c).unwrap_or('\0');
    // Standard display table: tab→8, newline→0, other controls→2.
    let w = match ch {
        '\t' => 8,
        '\n' => 0,
        c if c.is_control() => 2,
        c => unicode_width::UnicodeWidthChar::width(c).unwrap_or(0),
    };
    Ok(Value::Int(w as i128))
}

fn f_format_spec(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let fmt = want_string(i, &args[0])?;
    let alist = &args[1];
    let mut out = String::new();
    let mut chars = fmt.chars().peekable();
    while let Some(c) = chars.next() {
        if c != '%' {
            out.push(c);
            continue;
        }
        // Optional flags.
        let mut minus = false;
        let mut zero = false;
        let mut width = 0usize;
        let mut prec: Option<usize> = None;
        loop {
            match chars.peek() {
                Some('-') => {
                    minus = true;
                    chars.next();
                }
                Some('0') => {
                    zero = true;
                    chars.next();
                }
                Some(' ') => {
                    chars.next();
                }
                _ => break,
            }
        }
        while let Some(d @ '0'..='9') = chars.peek() {
            width = width * 10 + (*d as usize - '0' as usize);
            chars.next();
        }
        if chars.peek() == Some(&'.') {
            chars.next();
            let mut p = 0usize;
            while let Some(d @ '0'..='9') = chars.peek() {
                p = p * 10 + (*d as usize - '0' as usize);
                chars.next();
            }
            prec = Some(p);
        }
        let spec = match chars.next() {
            Some(s) => s,
            None => break,
        };
        if spec == '%' {
            out.push('%');
            continue;
        }
        // Look up spec char in alist: key is a char (Int).
        let key = Value::Int(spec as i128);
        let mut val = Value::Nil;
        let mut cur = alist.clone();
        loop {
            match cur {
                Value::Nil => break,
                Value::Cons(c2) => {
                    let (elem, next) = {
                        let b = c2.borrow();
                        (b.car.clone(), b.cdr.clone())
                    };
                    if let Value::Cons(pair) = &elem {
                        let (k, v) = {
                            let b = pair.borrow();
                            (b.car.clone(), b.cdr.clone())
                        };
                        if super::eq_values(&k, &key) {
                            val = v;
                            break;
                        }
                    }
                    cur = next;
                }
                _ => break,
            }
        }
        // Render val like princ, honoring width/precision.
        let mut text = match &val {
            Value::Nil => String::new(),
            _ => {
                let mut s = i.princ_to_string(&val);
                if let Some(p) = prec {
                    s = s.chars().take(p).collect();
                }
                s
            }
        };
        if width > 0 && text.chars().count() < width {
            let pad = width - text.chars().count();
            if minus {
                text.extend(std::iter::repeat_n(' ', pad));
            } else {
                let padc = if zero { '0' } else { ' ' };
                text = std::iter::repeat_n(padc, pad).chain(text.chars()).collect();
            }
        }
        out.push_str(&text);
    }
    Ok(Value::string(out))
}

fn f_multibyte_string_p(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let _ = i;
    Ok(Value::from_bool(match &args[0] {
        Value::Str(s) => s.borrow().chars().any(|c| (c as u32) > 0x7f),
        _ => false,
    }))
}

fn f_unibyte_string(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let mut s = String::new();
    for a in &args {
        let n = want_int(i, a)?;
        let c = char::from_u32((n & 0xff) as u32).unwrap_or('\u{FFFD}');
        s.push(c);
    }
    Ok(Value::string(s))
}

fn f_make_char(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    // (make-char CHARSET &optional CODE1 CODE2 CODE3 CODE4)
    // We model all text as Unicode scalars: combine given codes.
    let _cs = i.sym_id(&args[0]).unwrap_or(0);
    let mut code: i128 = 0;
    for a in args.iter().skip(1) {
        if let Value::Int(n) = a {
            code = code * 94 + (*n).max(0);
        }
    }
    // Keep it in range of Unicode scalars.
    if code == 0 {
        code = 32;
    }
    Ok(Value::Int(code & 0x3fffff))
}

fn f_split_char(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let ch = want_int(i, &args[0])?;
    let cs = i.intern(if ch < 0x80 { "ascii" } else { "unicode" });
    Ok(Value::list(vec![Value::Sym(cs), Value::Int(ch)]))
}

fn f_encode_char(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    // (encode-char CH CODING-SYSTEM) — utf-8 identity.
    want_int(i, &args[0])?;
    Ok(args[0].clone())
}

fn f_decode_char(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let code = want_int(i, &args[1])?;
    // GNU returns nil when CODE exceeds the charset's code space.
    let cs_name = match &args[0] {
        Value::Sym(s) => i.symbol_name(*s),
        _ => String::new(),
    };
    let max: i128 = match cs_name.as_str() {
        "ascii" => 127,
        "iso-8859-1" | "latin-iso8859-1" | "eight-bit-graphic" | "eight-bit-control" => 255,
        _ => i128::MAX,
    };
    if code >= 0 && code <= max {
        Ok(args[1].clone())
    } else {
        Ok(Value::Nil)
    }
}

/// Charsets we model, in GNU's `charset-priority-list` order.
pub(crate) const CHARSET_PRIORITY: &[&str] = &[
    "japanese-jisx0208",
    "japanese-jisx0212",
    "japanese-jisx0213.2004-1",
    "ascii",
    "latin-iso8859-1",
    "iso-8859-1",
    "unicode",
    "eight-bit-control",
    "eight-bit-graphic",
];

/// Does GNU charset `name` contain code point `ch`? Approximates the
/// ranges of the charsets we model.
pub(crate) fn charset_contains(name: &str, ch: i128) -> bool {
    let u = ch as u32;
    match name {
        "japanese-jisx0208" => matches!(
            u,
            0x3000..=0x30FF | 0x3400..=0x9FFF | 0xF900..=0xFAFF | 0xFF00..=0xFFEF
        ),
        // JIS X 0212 covers most Latin supplement letters (é, ü, ...).
        "japanese-jisx0212" => matches!(u, 0xA0..=0x24F),
        "japanese-jisx0213.2004-1" => u == 0x20AC,
        "ascii" => u < 0x80,
        "latin-iso8859-1" | "iso-8859-1" => u <= 0xFF,
        "unicode" => true,
        "eight-bit-control" => (0x80..=0x9F).contains(&u),
        "eight-bit-graphic" => (0xA0..=0xFF).contains(&u),
        _ => false,
    }
}

/// Canonical emission order for charset lists — GNU sorts `char-charset`
/// results by internal charset id (ascii first, then the JIS tables).
pub(crate) const CHARSET_ID_ORDER: &[&str] = &[
    "ascii",
    "japanese-jisx0208",
    "japanese-jisx0212",
    "japanese-jisx0213.2004-1",
    "latin-iso8859-1",
    "iso-8859-1",
    "eight-bit-control",
    "eight-bit-graphic",
    "unicode",
];

/// Highest-priority charset containing `ch`, restricted to `allowed`
/// (empty = GNU's full priority list).
pub(crate) fn char_charset_in<'a>(ch: i128, allowed: &'a [&'a str]) -> Option<&'a str> {
    let order: &[&str] = if allowed.is_empty() {
        CHARSET_PRIORITY
    } else {
        allowed
    };
    order
        .iter()
        .copied()
        .find(|name| charset_contains(name, ch))
}

fn f_char_charset(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let ch = want_int(i, &args[0])?;
    let restriction = match args.get(1) {
        None | Some(Value::Nil) => Vec::new(),
        Some(v) => charset_restriction(i, v)?,
    };
    let refs: Vec<&str> = restriction.iter().map(String::as_str).collect();
    match char_charset_in(ch, &refs) {
        Some(name) => Ok(Value::Sym(i.intern(name))),
        None => Ok(Value::Nil),
    }
}

/// Resolve a `char-charset` RESTRICTION arg to charset names. Entries may
/// be charset names or coding-system names (mapped to their charsets).
fn charset_restriction(i: &mut Interp, v: &Value) -> Result<Vec<String>, Flow> {
    let mut out = Vec::new();
    let mut add = |i: &mut Interp, item: &Value| -> Result<(), Flow> {
        let name = match item {
            Value::Sym(s) => i.symbol_name(*s),
            _ => return Err(i.wrong_type_mut("charsetp", item)),
        };
        if CHARSET_PRIORITY.contains(&name.as_str()) || name == "emacs" || name == "eight-bit" {
            out.push(name);
        } else if let Some(cs) = super::misc::coding_known(i, item) {
            // A coding system restricts to its charset list.
            for cs in coding_charsets(&cs) {
                out.push(cs.to_string());
            }
        } else {
            let cs = i.intern("coding-system-error");
            return Err(i.signal_data(cs, vec![item.clone()]));
        }
        Ok(())
    };
    match v {
        Value::Cons(_) => {
            for item in v.list_to_vec().unwrap_or_default() {
                add(i, &item)?;
            }
        }
        _ => add(i, v)?,
    }
    Ok(out)
}

/// Charset list of a coding system, mirroring `coding-system-charset-list`.
fn coding_charsets(name: &str) -> Vec<&'static str> {
    if name.starts_with("utf-8") || name.starts_with("undecided") {
        vec!["unicode"]
    } else if name.starts_with("iso-8859")
        || name.starts_with("iso-latin")
        || name.starts_with("latin")
    {
        vec!["iso-8859-1"]
    } else {
        vec!["ascii"]
    }
}
