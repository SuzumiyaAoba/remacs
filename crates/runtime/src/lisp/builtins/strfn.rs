//! String subrs: concat, substring, comparison, case, format, etc.

use super::{S, arg, misc, want_int, want_string};
use crate::lisp::Interp;
use crate::lisp::error::{EvalResult, Flow};
use crate::lisp::obarray::sym;
use crate::lisp::value::{
    StrRef, Subr, SymId, Value, eight_bit_byte, lisp_char, lisp_char_code,
};
use std::cell::Ref;
use std::rc::Rc;
use num_traits::{FromPrimitive, Signed, ToPrimitive, Zero};

pub(crate) static SUBRS: &[Subr] = &[
    S!("string", many 0, f_string, "Concatenate characters into a string."),
    S!("concat", many 0, f_concat, "Concatenate sequences into a string."),
    S!(
        "ngettext",
        3,
        3,
        f_ngettext,
        "Plural-aware MSGID/MSGID-PLURAL selector for N."
    ),
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
        f_substring_no_properties,
        "Substring without text props."
    ),
    // GNU: string-equal/string-lessp are primitives accepting strings or
    // symbols; string=, string<, string>, string-greaterp are Lisp-level
    // definitions and string<=/string>= do not exist.
    S!(
        "string-equal",
        2,
        2,
        f_string_equal,
        "t if two strings or symbols have identical contents."
    ),
    S!(
        "string-lessp",
        2,
        2,
        f_string_lessp,
        "t if S1 is less than S2 lexicographically."
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
        "int-to-string",
        1,
        1,
        f_number_to_string,
        "Return INTEGER as a string."
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
    S!("format-message", many 1, f_format_message, "Format with quoting conventions."),
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
    S!("string-reverse", 1, 1, f_string_reverse, "Reverse STRING."),
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
        6,
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
        3,
        f_string_lines,
        "Split STRING on newlines."
    ),
    S!("upcase-initials-region", 2, 3, f_upcase_initials_region, ""),
    S!(
        "string-to-multibyte",
        1,
        1,
        f_string_to_multibyte,
        "Return STRING unchanged."
    ),
    S!(
        "string-to-unibyte",
        1,
        1,
        f_string_to_unibyte,
        "Return STRING unchanged."
    ),
    S!(
        "string-as-unibyte",
        1,
        1,
        f_string_as_unibyte,
        "Return STRING unchanged."
    ),
    S!(
        "string-as-multibyte",
        1,
        1,
        f_string_as_multibyte,
        "Return STRING unchanged."
    ),
    S!(
        "string-make-unibyte",
        1,
        1,
        f_string_make_unibyte,
        "Return STRING with each char truncated to its low byte."
    ),
    S!(
        "string-make-multibyte",
        1,
        1,
        f_string_make_multibyte,
        "Convert STRING to multibyte, preserving ASCII and multibyte inputs."
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
        4,
        f_string_collate_equalp,
        "Collation equality (simple)."
    ),
    S!(
        "string-collate-lessp",
        2,
        4,
        f_string_collate_lessp,
        "Collation lessp (simple)."
    ),
    S!(
        "char-equal",
        2,
        2,
        f_char_equal,
        "t if two chars are equal (case-fold-aware)."
    ),
    S!(
        "multibyte-char-to-unibyte",
        1,
        1,
        f_multibyte_char_to_unibyte,
        "Convert CH to unibyte, or -1."
    ),
    S!(
        "unibyte-char-to-multibyte",
        1,
        1,
        f_unibyte_char_to_multibyte,
        "Convert unibyte BYTE to a char."
    ),
    S!(
        "string-truncate-left",
        2,
        2,
        f_string_truncate_left,
        "Truncate STRING to LENGTH chars with a leading ellipsis."
    ),
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
        "Split STRING into a list of strings representing separate glyphs."
    ),
    S!("sxhash-equal", 1, 1, f_sxhash, "Hash of OBJECT."),
    S!("sxhash", 1, 1, f_sxhash, "Hash of OBJECT."),
    S!("sxhash-eq", 1, 1, f_sxhash_eq, "Hash of OBJECT."),
    S!("sxhash-eql", 1, 1, f_sxhash_eql, "Hash of OBJECT."),
    S!(
        "sxhash-equal-including-properties",
        1,
        1,
        f_sxhash_including_props,
        ""
    ),
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
        3,
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
        1,
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

/// GNU: ASCII/Latin-1 chars (0..=0xFF) are their own unibyte byte,
/// eight-bit chars (0x3FFF80..) map to the raw byte; other valid chars → -1.
/// Values outside Emacs's full character range signal `characterp'.
fn f_multibyte_char_to_unibyte(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    let c = match &args[0] {
        Value::Int(n) if (0..=0x3FFFFF).contains(n) => *n,
        v => return Err(i.wrong_type_mut("characterp", v)),
    };
    Ok(Value::Int(if (0..=0xFF).contains(&c) {
        c
    } else if (0x3FFF80..=0x3FFFFF).contains(&c) {
        c - 0x3FFF00
    } else {
        -1
    }))
}

/// GNU: bytes 0..=127 stay; 128..=255 become eight-bit chars
/// (0x3FFF00 + byte); out of range → error.
fn f_unibyte_char_to_multibyte(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    let b = match &args[0] {
        Value::Int(n) if (0..=0x3FFFFF).contains(n) => *n,
        v => return Err(i.wrong_type_mut("characterp", v)),
    };
    match b {
        0..=127 => Ok(Value::Int(b)),
        128..=255 => Ok(Value::Int(0x3FFF00 + b)),
        _ => Err(i.error(format!("Not a unibyte character: {b}"))),
    }
}

/// GNU string-truncate-left: when STRING is longer than LENGTH,
/// return "..." plus the last max(LENGTH-3, 1) characters.
fn f_string_truncate_left(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    let s = want_string(i, &args[0])?;
    let length = want_int(i, &args[1])?;
    let chars: Vec<char> = s.chars().collect();
    if chars.len() as i128 <= length {
        return Ok(args[0].clone());
    }
    let keep = (length - 3).max(1).min(chars.len() as i128) as usize;
    let tail: String = chars[chars.len() - keep..].iter().collect();
    Ok(Value::string(format!("...{tail}")))
}

fn f_string(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    // GNU's `string' builds a MULTIBYTE string — bytes 0x80-0xFF are
    // stored as eight-bit chars (codes 0x3FFF80-0x3FFFFF, PUA proxies
    // internally); `multibyte-string-p' is t on the result.
    let mut out = String::new();
    for a in args.iter() {
        match a {
            Value::Int(n) if (0..=0x3fffff).contains(n) => {
                out.push(
                    lisp_char(*n as u32)
                        .unwrap_or_else(|| crate::lisp::value::mule_seq_char(*n as u32)),
                );
            }
            _ => return Err(i.wrong_type_mut("characterp", a)),
        }
    }
    let v = Value::string(out);
    if let Value::Str(r) = &v {
        i.mark_multibyte(r);
    }
    Ok(v)
}

/// Concatenate a sequence value's characters into `out`.
pub(crate) fn concat_seq(i: &mut Interp, v: &Value, out: &mut String) -> Result<(), super::Flow> {
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
                    Value::Int(n) => match concat_char(n) {
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
                    Value::Int(n) => match concat_char(*n) {
                        Some(c) => out.push(c),
                        None => return Err(i.wrong_type_mut("characterp", item)),
                    },
                    _ => return Err(i.wrong_type_mut("characterp", item)),
                }
            }
            Ok(())
        }
        // Bool-vectors are sequences of t/nil bits; concatenating one
        // hits the non-char element error like any other sequence.
        Value::Record(_) if super::misc::is_bool_vector(i, v) => {
            match super::misc::bool_vec_of(i, v)?.first() {
                Some(bit) => Err(i.wrong_type_mut("characterp", &Value::from_bool(*bit))),
                None => Ok(()),
            }
        }
        _ => Err(i.wrong_type_mut("sequencep", v)),
    }
}

fn concat_char(code: i128) -> Option<char> {
    let code = u32::try_from(code).ok()?;
    if code > 0x3f_ffff {
        return None;
    }
    Some(lisp_char(code).unwrap_or_else(|| crate::lisp::value::mule_seq_char(code)))
}

/// GNU storage rule fallback for *unmarked* strings: multibyte iff
/// the contents hold a non-ASCII char that is not an eight-bit
/// (raw-byte) proxy.  0x80-0xFF raw bytes count as unibyte.
fn content_is_multibyte(text: &str) -> bool {
    text.chars().any(|c| {
        let u = c as u32;
        // Non-ASCII means multibyte — but eight-bit byte proxies
        // (0xF0080+, raw-byte markers, print sentinels) are raw
        // bytes, which GNU counts as unibyte content.
        u > 0x7F
            && !(0x3FFF80..=0x3FFFFF).contains(&u)
            && crate::lisp::value::eight_bit_byte(c).is_none()
            && crate::lisp::value::raw_byte_marker(c).is_none()
            && crate::lisp::value::print_eight_bit_byte(c).is_none()
            && crate::lisp::value::write_char_eight_bit_byte(c).is_none()
    })
}

pub(crate) fn f_concat(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    let mut out = String::new();
    let mut ivs: Vec<(usize, usize, Vec<Value>)> = Vec::new();
    let mut saw_props = false;
    for a in args.iter() {
        // GNU concat carries each source string's text properties.
        let off = out.chars().count();
        if let Value::Str(s) = a {
            if i.has_str_props(s) {
                saw_props = true;
                for (s0, e0, pl) in i.str_props(s) {
                    // GNU's concat copies props through
                    // copy_text_properties, reversing plist order.
                    ivs.push((
                        s0 + off,
                        e0 + off,
                        crate::buffer::primitives::plist_pairs_rev(pl),
                    ));
                }
            }
        }
        concat_seq(i, a, &mut out)?;
    }
    // GNU `concat_internal': the result is multibyte iff some string
    // arg is multibyte, or some sequence element is a char that is
    // neither ASCII nor an eight-bit byte char (list/vector elements
    // were already validated as characters by concat_seq).
    let mut any_multi = false;
    let promotes = |n: i128| n >= 0x80 && !(0x3FFF80..=0x3FFFFF).contains(&n);
    for a in args.iter() {
        match a {
            Value::Str(s) => {
                if !i.is_unibyte_str(s)
                    && (i.is_multibyte_str(s) || content_is_multibyte(&s.borrow()))
                {
                    any_multi = true;
                }
            }
            Value::Cons(_) => {
                for item in super::want_list(i, a)? {
                    if let Value::Int(n) = item {
                        if promotes(n) {
                            any_multi = true;
                        }
                    }
                }
            }
            Value::Vec(v) => {
                for item in v.borrow().iter() {
                    if let Value::Int(n) = item {
                        if promotes(*n) {
                            any_multi = true;
                        }
                    }
                }
            }
            _ => {}
        }
    }
    let ns = crate::lisp::Value::string_ref(out);
    if any_multi {
        i.mark_multibyte(&ns);
    } else {
        i.mark_unibyte(&ns);
    }
    if saw_props {
        i.set_str_props(&ns, ivs);
    }
    Ok(Value::Str(ns))
}

fn f_ngettext(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    // (ngettext MSGID MSGID-PLURAL N) — English rule: N != 1 → plural.
    let sing = want_string(i, &args[0])?;
    let plural = want_string(i, &args[1])?;
    let n = want_int(i, &args[2])?;
    Ok(Value::string(if n == 1 { sing } else { plural }))
}

fn f_vconcat(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    let mut out = Vec::new();
    for sequence in args.iter() {
        match sequence {
            // GNU concat_to_vector accepts CLOSUREP as a slot sequence.
            Value::Lambda(function) => out.extend(misc::lambda_slot_values(function)),
            // Preserve raw bytes in unibyte strings, original Lisp codes
            // in multibyte strings, and the shared elements of sequences.
            _ => out.extend(super::seq::seq_to_vec(i, sequence)?),
        }
    }
    Ok(if out.is_empty() {
        i.zero_vector.clone()
    } else {
        Value::vector(out)
    })
}

pub(crate) fn f_substring(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    substring_impl(i, args, true)
}

fn f_substring_no_properties(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    // GNU's native-compiled copy checks `stringp' (no arrays/symbols).
    if !matches!(&args[0], Value::Str(_)) {
        return Err(i.wrong_type_mut("stringp", &args[0]));
    }
    substring_impl(i, args, false)
}

fn substring_impl(i: &mut Interp, args: crate::lisp::Argv, copy_props: bool) -> EvalResult {
    // Emacs: substring works on vectors too (returns a new vector).
    let is_vec = matches!(&args[0], Value::Vec(_));
    let chars: Vec<Value> = match &args[0] {
        Value::Str(s) => s
            .borrow()
            .chars()
            .map(|c| Value::Int(lisp_char_code(c)))
            .collect(),
        Value::Vec(v) => v.borrow().clone(),
        other => return Err(i.wrong_type_mut("arrayp", other)),
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
            vec![
                args[0].clone(),
                args.get(1).cloned().unwrap_or(Value::Int(0)),
                args.get(2).cloned().unwrap_or(Value::Nil),
            ],
        ));
    }
    let slice: Vec<Value> = chars[f as usize..t as usize].to_vec();
    if is_vec {
        Ok(Value::vector(slice))
    } else {
        let ns = crate::lisp::Value::string_ref(
            slice
                .iter()
                .filter_map(|v| match v {
                    Value::Int(n) => Some(
                        lisp_char(*n as u32)
                            .unwrap_or_else(|| crate::lisp::value::mule_seq_char(*n as u32)),
                    ),
                    _ => None,
                })
                .collect::<String>(),
        );
        // Substrings preserve the source representation even when the
        // slice contains only ASCII or raw encoded bytes. mule-cmds.el's
        // encode-coding-char relies on this for its byte-string result.
        if let Value::Str(src) = &args[0] {
            if i.is_unibyte_str(src) {
                i.mark_unibyte(&ns);
            } else if i.is_multibyte_str(src) {
                i.mark_multibyte(&ns);
            }
        }
        // GNU substring copies the source's text properties on the
        // sliced range (shifted to 0-based on the result).
        if copy_props {
            if let Value::Str(src) = &args[0] {
                if i.has_str_props(src) {
                    let (f0, t0) = (f as usize, t as usize);
                    let ivs: Vec<(usize, usize, Vec<Value>)> = i
                        .str_props(src)
                        .iter()
                        .filter_map(|(a, b, pl)| {
                            let lo = (*a).max(f0);
                            let hi = (*b).min(t0);
                            // GNU's substring copies properties via
                            // copy_text_properties, reversing plist order.
                            (lo < hi).then(|| {
                                (
                                    lo - f0,
                                    hi - f0,
                                    crate::buffer::primitives::plist_pairs_rev(pl),
                                )
                            })
                        })
                        .collect();
                    i.set_str_props(&ns, ivs);
                }
            }
        }
        Ok(Value::Str(ns))
    }
}

// GNU string-equal/string-lessp accept strings or symbols (symbols compare
// by name); other types signal wrong-type-argument stringp.
fn str_or_sym_arg(i: &mut Interp, v: &Value) -> Result<String, Flow> {
    match v {
        Value::Str(s) => Ok(s.borrow().clone()),
        // nil is the symbol nil in GNU; symbols compare by name.
        Value::Nil => Ok("nil".to_string()),
        Value::Sym(id) => Ok(i.symbol_name(*id)),
        other => Err(i.wrong_type_mut("stringp", other)),
    }
}
fn f_string_equal(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    let a = str_or_sym_arg(i, &args[0])?;
    let b = str_or_sym_arg(i, &args[1])?;
    Ok(Value::from_bool(a == b))
}
/// Keep the actual string object so ordering sees its byte kind and a
/// symbol name's current contents, rather than its internment key.
fn ordering_string(i: &mut Interp, value: &Value) -> Result<StrRef, Flow> {
    match value {
        Value::Str(s) => Ok(s.clone()),
        Value::Nil | Value::Sym(_) => {
            let Value::Str(name) = super::data::f_symbol_name(i, vec![value.clone()].into())?
            else {
                unreachable!()
            };
            Ok(name)
        }
        other => Err(i.wrong_type_mut("stringp", other)),
    }
}

fn f_string_lessp(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    let a = ordering_string(i, &args[0])?;
    let b = ordering_string(i, &args[1])?;
    let a_unibyte = i.is_unibyte_str(&a);
    let b_unibyte = i.is_unibyte_str(&b);
    // GNU string_cmp compares unibyte bytes as 0..255 and multibyte
    // characters by their original Lisp codes, including byte8/Mule.
    let code = |c: char, unibyte: bool| {
        if unibyte {
            eight_bit_byte(c)
                .map(i128::from)
                .unwrap_or(c as i128 & 0xff)
        } else {
            lisp_char_code(c)
        }
    };
    let a = a.borrow();
    let b = b.borrow();
    Ok(Value::from_bool(
        a.chars()
            .map(|c| code(c, a_unibyte))
            .cmp(b.chars().map(|c| code(c, b_unibyte)))
            == std::cmp::Ordering::Less,
    ))
}

fn f_string_collate_equalp(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    // On this build GNU takes the !__STDC_ISO_10646__ branch: a plain
    // Fstring_equal — symbols compare by name, LOCALE and IGNORE-CASE
    // are ignored entirely.
    let a = str_or_sym_arg(i, &args[0])?;
    let b = str_or_sym_arg(i, &args[1])?;
    Ok(Value::from_bool(a == b))
}
fn f_string_collate_lessp(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    // Match GNU's NS fallback, which ignores LOCALE and IGNORE-CASE.
    f_string_lessp(i, args)
}
fn f_string_compare(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
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
// --- GNU casefiddle.c casing ------------------------------------------
// Case ops run through the current buffer's case table: contents is the
// downcase map, extra slot 0 the upcase map.  Titlecase (capitalize /
// upcase-initials first char) is GNU's hidden extras[3] table — the
// upcase map plus TITLE_OVERRIDES (digraphs -> title form, Georgian
// Mk -> identity, etc.).  String ops additionally apply the uniprop
// special-* expansions (ß -> "SS") and the final-sigma rule.

#[derive(Clone, Copy, PartialEq)]
pub(crate) enum CaseOp {
    Up,
    Down,
    Cap,
    CapUp,
}

/// Extra slot N of a case table (record slots 3+).
pub(crate) fn case_extra(ct: &Value, slot: usize) -> Option<Value> {
    match ct {
        Value::Record(r) => r.borrow().get(3 + slot).cloned(),
        _ => None,
    }
}

/// `CHAR_TABLE_REF' casing map: Int result -> that char, else unchanged.
pub(crate) fn ct_case(i: &Interp, table: &Value, c: u32) -> u32 {
    match crate::lisp::builtins::misc::char_table_ref(i, table, c as usize) {
        Value::Int(n) if (0..=0x3f_ffff).contains(&n) => n as u32,
        _ => c,
    }
}

fn special_lookup(table: &[(u32, &'static str)], c: char) -> Option<&'static str> {
    table
        .binary_search_by_key(&(c as u32), |(k, _)| *k)
        .ok()
        .map(|idx| table[idx].1)
}

/// Single-char titlecase at word start: the Unicode `titlecase' property
/// table (GNU casefiddle.c: `titlecase_char_table' = `uniprop_table
/// (Qtitlecase)', independent of the buffer case table), then TITLE_
/// OVERRIDES for the few non-1:1 entries, else the upcase map.
fn title_char(i: &mut Interp, title: &Value, up: &Value, c: u32) -> u32 {
    if let Value::Int(n) = misc::char_table_ref(i, title, c as usize) {
        if (0..=0x3f_ffff).contains(&n) {
            return n as u32;
        }
    }
    crate::lisp::ctdata::TITLE_OVERRIDES
        .binary_search_by_key(&c, |(k, _)| *k)
        .ok()
        .map(|idx| crate::lisp::ctdata::TITLE_OVERRIDES[idx].1)
        .unwrap_or_else(|| ct_case(i, up, c))
}

/// Push the downcased form of C (char code CP); honors the final-sigma
/// rule: capital sigma inside a word, at the word's last word-char
/// position, downcases to U+03C2 rather than U+03C3.
fn push_down(
    i: &Interp,
    down: &Value,
    out: &mut String,
    c: char,
    cp: u32,
    prev_word: bool,
    next_word: bool,
) {
    if let Some(sp) = special_lookup(crate::lisp::ctdata::SPECIAL_LOWER, c) {
        out.push_str(sp);
    } else if cp == 0x3a3 && prev_word && !next_word {
        out.push('\u{3c2}');
    } else {
        out.push(char::from_u32(ct_case(i, down, cp)).unwrap_or(c));
    }
}

pub(crate) fn casify(i: &mut Interp, arg: &Value, op: CaseOp) -> EvalResult {
    let down = i.current_case_table();
    let up = case_extra(&down, 0).unwrap_or_else(|| down.clone());
    // GNU `prepare_casing_context': capitalize ops use the Unicode
    // `titlecase' property table, not the buffer case table.
    let title = match op {
        CaseOp::Cap | CaseOp::CapUp => {
            crate::lisp::builtins::charset::unicode_prop_table(i, "titlecase")
        }
        _ => Value::Nil,
    };
    match arg {
        // GNU casify_object: FIXNATP — only NON-NEGATIVE fixnums take
        // the natnum path (out-of-range naturals are returned
        // unchanged); negative ints, floats, and other non-strings
        // signal `char-or-string-p'.
        Value::Int(n) if *n >= 0 && !(0..=0x3f_ffff).contains(n) => Ok(arg.clone()),
        Value::Int(n) if *n >= 0 => {
            let cp = *n as u32;
            let r = match op {
                CaseOp::Up => ct_case(i, &up, cp),
                CaseOp::Down => ct_case(i, &down, cp),
                _ => title_char(i, &title, &up, cp),
            };
            Ok(Value::Int(r as i128))
        }
        Value::Str(s) => {
            let src = s.borrow().clone();
            if src.is_empty() {
                return Ok(arg.clone());
            }
            if !string_multibyte_p(i, arg) {
                let output = super::seq::f_copy_sequence(i, vec![arg.clone()].into())?;
                let Value::Str(string) = &output else {
                    unreachable!()
                };
                *string.borrow_mut() = case_unibyte_str(i, &src, op, &down, &up, &title);
                i.mark_unibyte(string);
                Ok(output)
            } else {
                // GNU's multibyte branch allocates a fresh string and
                // deliberately carries no text-property intervals.
                let output = Value::string_ref(case_str(i, &src, op, &down, &up, &title));
                i.mark_multibyte(&output);
                Ok(Value::Str(output))
            }
        }
        other => Err(i.wrong_type_mut("char-or-string-p", other)),
    }
}

/// GNU do_casify_unibyte_string applies only single-character mappings.
/// Raw bytes are byte8 characters; ASCII mappings that need multibyte
/// storage fall back to Unicode's ASCII case rather than changing type.
fn case_unibyte_str(
    i: &mut Interp,
    src: &str,
    op: CaseOp,
    down: &Value,
    up: &Value,
    title: &Value,
) -> String {
    let syntax = crate::editor::syntax_table_entries(i);
    let symbols_as_words = i
        .intern_soft("case-symbols-as-words")
        .is_some_and(|id| i.symbol_value(id).truthy());
    let mut in_word = false;
    let mut output = String::new();
    for ch in src.chars() {
        let code = lisp_char_code(ch) as u32;
        let syntax = crate::editor::syntax_entry_code(syntax.as_ref(), ch);
        let word = syntax == b'w' || (symbols_as_words && syntax == b'_');
        let downcase = op == CaseOp::Down || (op == CaseOp::Cap && in_word && word);
        let mut cased = match op {
            CaseOp::Up => ct_case(i, up, code),
            CaseOp::Down => ct_case(i, down, code),
            _ if word && !in_word => title_char(i, title, up, code),
            CaseOp::Cap if word => ct_case(i, down, code),
            _ => code,
        };
        in_word = word;
        if code < 128 && cased >= 128 && !(0x3fff80..=0x3fffff).contains(&cased) {
            let ascii = code as u8;
            cased = if downcase {
                ascii.to_ascii_lowercase()
            } else {
                ascii.to_ascii_uppercase()
            } as u32;
        }
        output.push(char::from(cased as u8));
    }
    output
}
/// Casify a source string per GNU casefiddle (shared by the string
/// functions, the *-region commands, and the *-word commands).
/// `title' is the Unicode `titlecase' property table fetched by the
/// caller for Cap/CapUp ops (nil otherwise).
pub(crate) fn case_str(
    i: &mut Interp,
    src: &str,
    op: CaseOp,
    down: &Value,
    up: &Value,
    title: &Value,
) -> String {
    case_str_impl(i, src, op, down, up, title, None)
}

pub(crate) fn case_str_with_widths(
    i: &mut Interp,
    src: &str,
    op: CaseOp,
    down: &Value,
    up: &Value,
    title: &Value,
) -> (String, Vec<usize>) {
    let mut widths = Vec::new();
    let output = case_str_impl(i, src, op, down, up, title, Some(&mut widths));
    (output, widths)
}

fn case_str_impl(
    i: &mut Interp,
    src: &str,
    op: CaseOp,
    down: &Value,
    up: &Value,
    title: &Value,
    mut widths: Option<&mut Vec<usize>>,
) -> String {
    let chars: Vec<char> = src.chars().collect();
    let syn = crate::editor::syntax_table_entries(i);
    let symbols_as_words = i
        .intern_soft("case-symbols-as-words")
        .is_some_and(|id| i.symbol_value(id).truthy());
    let wordp = |c: char| {
        let syntax = crate::editor::syntax_entry_code(syn.as_ref(), c);
        syntax == b'w' || (symbols_as_words && syntax == b'_')
    };
    let mut out = String::new();
    let mut in_word = false;
    for (idx, &c) in chars.iter().enumerate() {
        let before = out.len();
        let cp = c as u32;
        match op {
            CaseOp::Up => {
                if let Some(sp) = special_lookup(crate::lisp::ctdata::SPECIAL_UPPER, c) {
                    out.push_str(sp);
                } else {
                    out.push(char::from_u32(ct_case(i, up, cp)).unwrap_or(c));
                }
            }
            CaseOp::Down => {
                let prev_word = idx > 0 && wordp(chars[idx - 1]);
                let next_word = idx + 1 < chars.len() && wordp(chars[idx + 1]);
                push_down(i, down, &mut out, c, cp, prev_word, next_word);
            }
            _ => {
                if wordp(c) {
                    if !in_word {
                        if let Some(sp) = special_lookup(crate::lisp::ctdata::SPECIAL_TITLE, c) {
                            out.push_str(sp);
                        } else {
                            out.push(char::from_u32(title_char(i, title, up, cp)).unwrap_or(c));
                        }
                        in_word = true;
                    } else if op == CaseOp::Cap {
                        let next_word = idx + 1 < chars.len() && wordp(chars[idx + 1]);
                        push_down(i, down, &mut out, c, cp, true, next_word);
                    } else {
                        out.push(c);
                    }
                } else {
                    in_word = false;
                    out.push(c);
                }
            }
        }
        if let Some(widths) = widths.as_mut() {
            widths.push(out[before..].chars().count());
        }
    }
    out
}

/// GNU replace_range_2 with MARKERS=nil: overwrite characters while
/// leaving marker/overlay character positions intact. Only point and ZV
/// move on expansion. Caller owns change hooks and the composite tick.
pub(crate) fn apply_case_text(
    i: &mut Interp,
    buffer: &crate::lisp::value::BufferRef,
    start: usize,
    old: &str,
    new: &str,
    widths: &[usize],
) {
    let old_len = old.chars().count();
    let new_len = new.chars().count();
    let old_props = buffer
        .borrow()
        .text
        .props
        .borrow()
        .slice(start, start + old_len);
    let undo = buffer.borrow_mut().record_change(start, old, new_len);
    let mut output = new.chars();
    let mut pos = start;
    for width in widths {
        if *width == 1 {
            buffer.borrow().text.set_char_at(pos, output.next().unwrap());
            pos += 1;
            continue;
        }
        let mapped: Vec<_> = output.by_ref().take(*width).collect();
        debug_assert!(!mapped.is_empty());
        if *width > 1 {
            let extra = width - 1;
            let inherited = crate::buffer::primitives::case_left_properties(i, buffer, pos);
            let mut buffer = buffer.borrow_mut();
            let split = buffer
                .text
                .props
                .borrow()
                .intervals()
                .iter()
                .any(|(a, b, _)| *a < pos && pos < *b);
            let prefix: String = mapped[..extra].iter().collect();
            buffer.text.insert(pos, &prefix);
            buffer.zv += extra;
            if buffer.point() > pos {
                let point = buffer.point() + extra;
                buffer.set_point(point);
            }
            let mut props = buffer.text.props_write();
            props.graft(pos, &[(0, extra, inherited)], false);
            props.finish_inherited_insert(pos, extra, split);
        }
        let buffer = buffer.borrow();
        buffer
            .text
            .set_char_at(pos + width - 1, *mapped.last().unwrap());
        pos += width;
    }
    if let Some(undo) = undo {
        i.set_str_props(&undo, old_props);
        let multi = i.q("enable-multibyte-characters");
        if buffer.borrow().locals.get(&multi).is_none_or(Value::truthy) {
            i.mark_multibyte(&undo);
        } else {
            i.mark_unibyte(&undo);
        }
    }
}

fn f_upcase_initials_region(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    crate::editor::region_op(i, &args, CaseOp::CapUp)
}
fn f_upcase(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    casify(i, &args[0], CaseOp::Up)
}
fn f_downcase(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    casify(i, &args[0], CaseOp::Down)
}
fn f_capitalize(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    casify(i, &args[0], CaseOp::Cap)
}
fn f_upcase_initials(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    casify(i, &args[0], CaseOp::CapUp)
}

fn f_string_to_number(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    let s = want_string(i, &args[0])?;
    let base = match args.get(1) {
        None | Some(Value::Nil) => 10,
        Some(v) => crate::lisp::builtins::want_fixnum(i, v)?,
    };
    // GNU validates BASE before skipping leading whitespace or returning
    // zero for an empty input.
    if !(2..=16).contains(&base) {
        let sym = i.q("args-out-of-range");
        return Err(i.signal_data(sym, vec![Value::Int(base)]));
    }
    // GNU Fstring_to_number skips only spaces and tabs (a leading
    // newline or other whitespace stops the parse -> 0).
    let t = s.trim_start_matches([' ', '\t']);
    if t.is_empty() {
        return Ok(Value::Int(0));
    }

    if base != 10 {
        let neg = t.starts_with('-');
        let digits = if matches!(t.chars().next(), Some('+' | '-')) {
            &t[1..]
        } else {
            t
        };
        // Parse the longest valid digit prefix for the base.
        let take: String = digits
            .chars()
            .take_while(|c| c.to_digit(base as u32).is_some())
            .collect();
        let v = num_bigint::BigInt::parse_bytes(take.as_bytes(), base as u32)
            .unwrap_or_default();
        return Ok(Value::from_bigint(if neg { -v } else { v }));
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
    // A decimal point may precede an exponent even without fractional
    // digits (`1.e2', `1.e+INF').  With no exponent, bare `1.' still
    // ends at the integer prefix, so only advance `end' for a fraction.
    if cs.get(p) == Some(&'.') {
        let frac = scan_digits(&cs, p + 1);
        if int_digits > 0 || frac > 0 {
            p += 1 + frac;
        }
        if frac > 0 {
            end = p;
            float = true;
        }
    }
    if int_digits == 0 && !float {
        return String::new();
    }
    // Exponent: e[+-]?digits — only consumed when digits follow.
    // GNU's `e+NaN'/`e+INF' literal tail is part of the number too.
    if matches!(cs.get(p), Some('e') | Some('E')) {
        let mut q = p + 1;
        if matches!(cs.get(q), Some('+') | Some('-')) {
            q += 1;
        }
        let ed = scan_digits(&cs, q);
        if ed > 0 {
            end = q + ed;
        } else if cs.get(p + 1) == Some(&'+')
            && (cs.get(q..q + 3) == Some(&['N', 'a', 'N'][..])
                || cs.get(q..q + 3) == Some(&['I', 'N', 'F'][..]))
        {
            end = q + 3;
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
fn f_number_to_string(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    match &args[0] {
        Value::Int(n) => Ok(Value::string(n.to_string())),
        Value::BigInt(n) => Ok(Value::string(n.to_string())),
        Value::Float(f) => Ok(Value::string(i.format_float_output(**f))),
        other => Err(i.wrong_type_mut("numberp", other)),
    }
}
fn f_string_to_char(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    let Value::Str(s) = &args[0] else {
        return Err(i.wrong_type_mut("stringp", &args[0]));
    };
    let Some(c) = s.borrow().chars().next() else {
        return Ok(Value::Int(0));
    };
    // GNU Fstring_to_char on a unibyte string returns the raw byte
    // value (0-255), same as `elt' — eight-bit proxy chars map back.
    let code = if i.is_unibyte_str(s) {
        crate::lisp::value::eight_bit_byte(c)
            .map(|b| b as i128)
            .unwrap_or(c as i128 & 0xFF)
    } else {
        lisp_char_code(c)
    };
    Ok(Value::Int(code))
}
fn f_char_to_string(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    match &args[0] {
        // GNU CHECK_CHARACTER: any fixnum 0..#x3FFFFF is a char;
        // eight-bit codes map to PUA proxies like the rest of strfn.
        Value::Int(n) if (0..=0x3f_ffff).contains(n) => {
            let code = *n as u32;
            let c = lisp_char(code).unwrap_or_else(|| crate::lisp::value::mule_seq_char(code));
            let string = Value::string_ref(c.to_string());
            if code >= 128 {
                i.mark_multibyte(&string);
            }
            Ok(Value::Str(string))
        }
        other => Err(i.wrong_type_mut("characterp", other)),
    }
}
/// `string-trim-left/right` use regexps (subr-x): strip ONE match of
/// TRIM at the edge. TRIM nil or absent defaults to whitespace.
/// GNU: subr-x defsubst — (concat "\`" TRIM) requires TRIM to be a
/// *sequence*; non-string sequences feed `concat' char conversion.
fn trim_re(i: &mut Interp, args: &[Value], idx: usize) -> Result<String, Flow> {
    match args.get(idx) {
        None | Some(Value::Nil) => Ok("[ \t\n\r]+".into()),
        Some(Value::Str(s)) => Ok(s.borrow().clone()),
        Some(other @ (Value::Cons(_) | Value::Vec(_))) => {
            let elems = super::seq::seq_to_vec(i, other)?;
            let mut out = String::new();
            for e in &elems {
                match e {
                    Value::Int(n) if (0..=0x3f_ffff).contains(n) => {
                        let code = *n as u32;
                        out.push(lisp_char(code).unwrap_or_else(|| {
                            crate::lisp::value::mule_seq_char(code)
                        }));
                    }
                    _ => return Err(i.wrong_type_mut("characterp", e)),
                }
            }
            Ok(out)
        }
        Some(other) => Err(i.wrong_type_mut("sequencep", other)),
    }
}

fn f_string_trim(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    // GNU subr-x.el `string-trim' evaluates the regexp args first
    // (let ((trim-left ...) (trim-right ...)) ...), so TRIM-LEFT's
    // `sequencep' check fires before STRING's `stringp'.
    let left = trim_re(i, &args, 1)?;
    let right = trim_re(i, &args, 2)?;
    let s = want_string(i, &args[0])?;
    let after_left = trim_left_re(i, &s, &left)?;
    Ok(Value::string(trim_right_re(i, &after_left, &right)?))
}
fn f_string_trim_left(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    let re = trim_re(i, &args, 1)?;
    let s = want_string(i, &args[0])?;
    Ok(Value::string(trim_left_re(i, &s, &re)?))
}
fn f_string_trim_right(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    let re = trim_re(i, &args, 1)?;
    let s = want_string(i, &args[0])?;
    Ok(Value::string(trim_right_re(i, &s, &re)?))
}

fn trim_left_re(i: &mut Interp, s: &str, trim: &str) -> Result<String, Flow> {
    // GNU: (string-match (concat "\\`\\(?:" regexp "\\)") string) —
    // a single match at the start is stripped.
    let invalid_regexp = i.q("invalid-regexp");
    let re = crate::lisp::regexp::compile(&format!("\\`\\(?:{}\\)", trim))
        .map_err(|error| i.signal_data(invalid_regexp, vec![Value::string(error.0)]))?;
    let syn = crate::editor::re_syntax(i);
    let chars: Vec<char> = s.chars().collect();
    match crate::lisp::regexp::looking_at(
        &re,
        &chars,
        0,
        &syn,
        &crate::editor::re_category(i),
        crate::lisp::regexp::NO_POINT,
        false,
    ) {
        Some(regs) => {
            let end = regs.get(1).copied().flatten().unwrap_or(0);
            Ok(chars[end..].iter().collect())
        }
        None => Ok(s.to_string()),
    }
}

fn trim_right_re(i: &mut Interp, s: &str, trim: &str) -> Result<String, Flow> {
    // GNU: (string-match (concat "\\(?:" regexp "\\)\\'") string).
    let invalid_regexp = i.q("invalid-regexp");
    let re = crate::lisp::regexp::compile(&format!("\\(?:{}\\)\\'", trim))
        .map_err(|error| i.signal_data(invalid_regexp, vec![Value::string(error.0)]))?;
    let syn = crate::editor::re_syntax(i);
    let chars: Vec<char> = s.chars().collect();
    match crate::lisp::regexp::search(
        &re,
        &chars,
        0,
        &syn,
        &crate::editor::re_category(i),
        crate::lisp::regexp::NO_POINT,
    ) {
        Some((ms, _me)) => Ok(chars[..ms].iter().collect()),
        None => Ok(s.to_string()),
    }
}
fn f_string_pad(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    let s = want_string(i, &args[0])?;
    // GNU CHECK_FIXNAT: negative LENGTH signals `wholenump'.
    let len = match &args[1] {
        Value::Int(n) if *n >= 0 => *n as usize,
        Value::Int(_) => return Err(i.wrong_type_mut("wholenump", &args[1])),
        other => return Err(i.wrong_type_mut("fixnump", other)),
    };
    // GNU: PADDING is a single character; a string signals.
    let pad_char = match args.get(2) {
        None | Some(Value::Nil) => ' ',
        Some(Value::Int(n)) => match lisp_char(*n as u32) {
            Some(c) => c,
            None => return Err(i.wrong_type_mut("characterp", &args[2])),
        },
        Some(other) => return Err(i.wrong_type_mut("characterp", other)),
    };
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

/// `string-reverse' — GNU fns.c subr: character-reverse the string.
fn f_string_reverse(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    let s = want_string(i, &args[0])?;
    Ok(Value::string(s.chars().rev().collect::<String>()))
}
fn f_string_join(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    let items = super::seq::seq_to_vec(i, &args[0])?;
    // GNU's string-join is `(mapconcat #'identity STRINGS SEPARATOR)':
    // elements and SEPARATOR are concat'ed, so they may be any char
    // sequence, and SEPARATOR is only coerced when a join happens.
    let mut sep: Option<String> = None;
    let mut parts: Vec<String> = Vec::with_capacity(items.len());
    for item in &items {
        let mut p = String::new();
        concat_seq(i, item, &mut p)?;
        parts.push(p);
    }
    let mut out = String::new();
    for (k, p) in parts.iter().enumerate() {
        if k > 0 {
            if sep.is_none() {
                let mut s = String::new();
                match args.get(1) {
                    None | Some(Value::Nil) => {}
                    Some(v) => concat_seq(i, v, &mut s)?,
                }
                sep = Some(s);
            }
            out.push_str(sep.as_deref().unwrap_or_default());
        }
        out.push_str(p);
    }
    Ok(Value::string(out))
}
fn f_split_string(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    // GNU subr.el keeps the next search position separate from the
    // current field's start.  Advancing only the former after empty
    // matches guarantees progress without dropping field characters.
    let source = args[0].clone();
    let keep_empty = args.get(1).is_some_and(Value::truthy)
        && !args.get(2).is_some_and(Value::truthy);
    let length_fn = i.q("length");
    let length = i.apply(&Value::Sym(length_fn), vec![source.clone()])?;
    let len = want_int(i, &length)? as usize;
    let trim = match args.get(3) {
        None | Some(Value::Nil) => None,
        Some(v) => {
            let mut s = String::new();
            concat_seq(i, v, &mut s)?;
            Some((
                Value::string(format!("\\`\\(?:{}\\)", s)),
                Value::string(format!("\\(?:{}\\)\\'", s)),
            ))
        }
    };
    let sep = match args.get(1) {
        Some(v) if v.truthy() => v.clone(),
        _ => {
            let id = i.q("split-string-default-separators");
            i.eval(&Value::Sym(id))?
        }
    };
    let mut parts = Vec::new();
    let (mut next, mut start) = (0usize, 0usize);
    loop {
        let found = split_string_match(i, &sep, &source, next, false)?;
        let Some(sep_start) = found.int() else {
            break;
        };
        let sep_start = sep_start as usize;
        let sep_end = split_match_end(i)?;
        if keep_empty || start < sep_start {
            let item = f_substring(
                i,
                vec![
                    source.clone(),
                    Value::Int(start as i128),
                    Value::Int(sep_start as i128),
                ].into(),
            )?;
            let item = split_trim_item(i, item, &trim)?;
            if keep_empty || matches!(&item, Value::Str(s) if !s.borrow().is_empty()) {
                parts.push(item);
            }
        }
        next = (next + 1).max(sep_end);
        start = sep_end;
        if start >= len {
            break;
        }
    }
    // GNU returns the original object when no separator was found.
    let item = if start == 0 {
        source
    } else {
        f_substring(i, vec![source, Value::Int(start as i128)].into())?
    };
    let item = split_trim_item(i, item, &trim)?;
    if keep_empty || matches!(&item, Value::Str(s) if !s.borrow().is_empty()) {
        parts.push(item);
    }
    Ok(Value::list(parts))
}

fn split_string_match(
    i: &mut Interp,
    regexp: &Value,
    string: &Value,
    start: usize,
    preserve_data: bool,
) -> EvalResult {
    let id = i.intern(if preserve_data {
        "string-match-p"
    } else {
        "string-match"
    });
    i.apply(
        &Value::Sym(id),
        vec![regexp.clone(), string.clone(), Value::Int(start as i128)],
    )
}

fn split_match_end(i: &mut Interp) -> Result<usize, Flow> {
    let id = i.q("match-end");
    let end = i.apply(&Value::Sym(id), vec![Value::Int(0)])?;
    Ok(want_int(i, &end)? as usize)
}

fn split_trim_item(i: &mut Interp, item: Value, trim: &Option<(Value, Value)>) -> EvalResult {
    let Some((left, right)) = trim else {
        return Ok(item);
    };
    let beg = if split_string_match(i, left, &item, 0, false)?.truthy() {
        split_match_end(i)?
    } else {
        0
    };
    let end = split_string_match(i, right, &item, beg, true)?;
    let len = match &item {
        Value::Str(s) => s.borrow().chars().count(),
        _ => 0,
    };
    let end = end.int().map(|n| n as usize).unwrap_or(len);
    if beg > 0 || end < len {
        f_substring(
            i,
            vec![item, Value::Int(beg as i128), Value::Int(end as i128)].into(),
        )
    } else {
        Ok(item)
    }
}
fn f_string_replace(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    // GNU subr.el: `from' is checked via `equal', `in' by the
    // `string-search' loop, and `to' only reaches `concat' when a match
    // occurred — so the arg-check order is FROM, IN, then TO (and only
    // on a hit).
    let from = want_string(i, &args[0])?;
    let s = want_string(i, &args[2])?;
    if from.is_empty() {
        let sym = i.q("wrong-length-argument");
        return Err(i.signal_data(sym, vec![Value::Int(0)]));
    }
    // GNU's string-replace keeps the text properties of the
    // non-replaced runs (verbatim, like copy-sequence); the
    // replacement text itself gets no properties.
    let schars: Vec<char> = s.chars().collect();
    let fchars: Vec<char> = from.chars().collect();
    let flen = fchars.len();
    let mut out = String::new();
    // Kept source runs as (src_beg, src_end, dst_beg), all char indices.
    let mut runs: Vec<(usize, usize, usize)> = Vec::new();
    let mut pos = 0usize;
    let mut changed = false;
    // `to' is converted lazily (only on the first hit) and through
    // `concat' sequence semantics: strings verbatim, vectors/lists of
    // characters joined, anything else wrong-type-argument sequencep
    // (elements that aren't characters → integerp).
    let mut to: Option<String> = None;
    let to_of = |i: &mut Interp, v: &Value| -> Result<String, Flow> {
        let mut out = String::new();
        concat_seq(i, v, &mut out)?;
        Ok(out)
    };
    while pos + flen <= schars.len() {
        if schars[pos..pos + flen] == fchars[..] {
            if to.is_none() {
                to = Some(to_of(i, &args[1])?);
            }
            out.push_str(to.as_deref().unwrap());
            changed = true;
            pos += flen;
        } else {
            let dst = out.chars().count();
            out.push(schars[pos]);
            if let Some(last) = runs.last_mut() {
                if last.1 == pos {
                    last.1 = pos + 1;
                    pos += 1;
                    continue;
                }
            }
            runs.push((pos, pos + 1, dst));
            pos += 1;
        }
    }
    while pos < schars.len() {
        let dst = out.chars().count();
        out.push(schars[pos]);
        if let Some(last) = runs.last_mut() {
            if last.1 == pos {
                last.1 = pos + 1;
                pos += 1;
                continue;
            }
        }
        runs.push((pos, pos + 1, dst));
        pos += 1;
    }
    if !changed {
        // GNU returns the original object when nothing matched.
        return Ok(args[2].clone());
    }
    let ns = crate::lisp::Value::string_ref(out);
    if let Value::Str(src) = &args[2] {
        if i.has_str_props(src) {
            let mut ivs: Vec<(usize, usize, Vec<Value>)> = Vec::new();
            for (a, b, pl) in i.str_props(src) {
                for &(s0, s1, d0) in &runs {
                    let lo = (*a).max(s0);
                    let hi = (*b).min(s1);
                    if lo < hi {
                        ivs.push((d0 + (lo - s0), d0 + (hi - s0), pl.clone()));
                    }
                }
            }
            i.set_str_props(&ns, ivs);
        }
    }
    Ok(Value::Str(ns))
}
fn f_string_chop_newline(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    let s = want_string(i, &args[0])?;
    // GNU removes only the final newline, not a run of them.
    Ok(Value::string(
        s.strip_suffix('\n').unwrap_or(&s).to_string(),
    ))
}
/// GNU's sanitized buffer-local TAB width.
fn display_tab_width(i: &Interp) -> usize {
    // GNU buffer.h sanitize_tab_width: any non-fixnum or value outside
    // 1..=1000 falls back to 8, including zero, floats and bignums.
    match i.q_soft("tab-width").map(|id| i.symbol_value(id)) {
        Some(Value::Int(width)) if (1..=1000).contains(&width) => width as usize,
        _ => 8,
    }
}

/// Display column width under GNU strwidth rules: TAB contributes the
/// sanitized tab-width (without tab-stop tracking), and newline is zero.
fn gnu_char_width(i: &Interp, c: char) -> usize {
    if c == '\t' {
        display_tab_width(i)
    } else {
        gnu_char_code_width(i, lisp_char_code(c) as u32)
    }
}

fn gnu_char_code_width(i: &Interp, code: u32) -> usize {
    // CHARACTER_WIDTH handles ASCII directly. For other characters,
    // consult the live table using the Lisp code rather than a Rust
    // proxy scalar (surrogates, high Mule codes, and eight-bit chars).
    if code < 0x80 {
        return match code {
            10 => 0,
            0x20..=0x7E => 1,
            _ => 2,
        };
    }
    if let Some(id) = i.q_soft("char-width-table") {
        if let Value::Int(width) = misc::char_table_ref(i, &i.symbol_value(id), code as usize) {
            if width >= 0 {
                return width as usize;
            }
        }
    }
    // Sparse bootstrap tables may not yet have a default or all ranges.
    if (0x80..=0x9F).contains(&code) || (0x3FFF80..=0x3FFFFF).contains(&code) {
        4
    } else if code > 0x10FFFF {
        1
    } else {
        char::from_u32(code)
            .map(|c| crate::buffer::primitives::char_width(c) as usize)
            .unwrap_or(1)
    }
}

fn f_string_width(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    let s = match &args[0] {
        Value::Str(s) => s.borrow().clone(),
        other => return Err(i.wrong_type_mut("stringp", other)),
    };
    // GNU: FROM/TO are positions with nil → 0/length, negatives
    // count from the end, and bounds violations signal
    // args-out-of-range carrying the original values.
    let from = args.get(1).cloned().unwrap_or(Value::Nil);
    let to = args.get(2).cloned().unwrap_or(Value::Nil);
    let chars: Vec<char> = s.chars().collect();
    let (f, t) = validate_subarray(i, &args[0], &from, &to, chars.len())?;
    let mut w = 0usize;
    for c in &chars[f..t] {
        w += gnu_char_width(i, *c);
    }
    Ok(Value::Int(w as i128))
}
/// `truncate-string-to-width' — GNU signature:
/// (truncate-string-to-width STRING WIDTH &optional START-COLUMN
/// PADDING ELLIPSIS).  Cuts STRING so its display width doesn't exceed
/// WIDTH; when the string is actually truncated and ELLIPSIS is
/// non-nil, an ellipsis is appended, reserving its own width first
/// (ELLIPSIS of t means `truncate-string-ellipsis', defaulting to "…"
/// measured using the current char-width table).  PADDING non-nil
/// pads the result out to WIDTH — PADDING of a character pads with
/// that character, otherwise with spaces.  START-COLUMN is the
/// starting display column (affects tab expansion and leaves room).
fn f_truncate_string_to_width(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    let s = want_string(i, &args[0])?;
    let width = want_int(i, &args[1])?.max(0) as usize;
    let start_col = match args.get(2) {
        Some(Value::Int(n)) => (*n).max(0) as usize,
        Some(other) if !other.is_nil() => return Err(i.wrong_type_mut("integerp", other)),
        _ => 0,
    };
    let padding = args.get(3).cloned().unwrap_or(Value::Nil);
    let ellipsis_v = args.get(4).cloned().unwrap_or(Value::Nil);

    // Resolve the ellipsis string (GNU: a string is used verbatim;
    // any other non-nil value means `truncate-string-ellipsis',
    // defaulting to "…").
    let ellipsis = match &ellipsis_v {
        Value::Nil => None,
        Value::Str(e) => Some(e.borrow().clone()),
        _ => {
            let eid = i.q("truncate-string-ellipsis");
            match i.symbol_value(eid) {
                Value::Str(e) => Some(e.borrow().clone()),
                _ => Some("…".to_string()),
            }
        }
    };
    let ell_w: usize = ellipsis
        .as_deref()
        .map(|e| {
            let mut w = 0;
            for c in e.chars() {
                w += gnu_char_width(i, c);
            }
            w
        })
        .unwrap_or(0);

    // GNU: skip over the characters lying before START-COLUMN (their
    // widths still accumulate, so they consume part of WIDTH).
    let chars: Vec<char> = s.chars().collect();
    let mut col = 0usize;
    let mut idx = 0usize;
    while idx < chars.len() && col < start_col {
        col += gnu_char_width(i, chars[idx]);
        idx += 1;
    }
    // Scan the tail to decide whether to reserve ellipsis width.
    let mut fits = true;
    {
        let mut c = col;
        for &ch in &chars[idx..] {
            let cw = gnu_char_width(i, ch);
            if c + cw > width {
                fits = false;
                break;
            }
            c += cw;
        }
    }
    let budget = if fits || ellipsis.is_none() {
        width
    } else {
        // Reserve room for the ellipsis.
        width.saturating_sub(ell_w)
    };
    let mut out = String::new();
    let mut truncated = false;
    for &c in &chars[idx..] {
        let cw = gnu_char_width(i, c);
        if col >= budget || col + cw > budget {
            truncated = true;
            break;
        }
        out.push(c);
        col += cw;
    }
    if truncated && !fits {
        if let Some(e) = &ellipsis {
            out.push_str(e);
            col += ell_w;
        }
    }
    // PADDING: non-nil pads out to WIDTH with the padding char (or
    // spaces when PADDING isn't a character).
    if padding.truthy() && col < width {
        let pad = match &padding {
            Value::Int(n) => char::from_u32(*n as u32).unwrap_or(' '),
            _ => ' ',
        };
        for _ in col..width {
            out.push(pad);
        }
    }
    Ok(Value::string(out))
}
/// `string-fill' — GNU's subr-x definition wraps `fill-region': the
/// string is broken into words and whitespace runs (space, tab,
/// newline only); each inter-word run becomes a single space or a
/// newline so that filled lines stay within WIDTH columns. A run
/// containing two or more newlines is a paragraph separator and is
/// kept verbatim from its first newline on (leading whitespace before
/// it is dropped). A leading run is kept verbatim, a trailing run is
/// dropped unless it's a separator. Columns are display columns:
/// tab advances to the next multiple of 8 and newline resets.
fn f_string_fill(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    let s = want_string(i, &args[0])?;
    let width = want_int(i, &args[1])?;
    let chars: Vec<char> = s.chars().collect();
    let n = chars.len();
    let is_ws = |c: char| c == ' ' || c == '\t' || c == '\n';
    let adv = |col: usize, c: char| -> usize {
        if c == '\n' {
            0
        } else if c == '\t' {
            col + (8 - col % 8)
        } else {
            col + gnu_char_width(i, c)
        }
    };
    let word_width = |mut pos: usize| -> usize {
        let mut w = 0usize;
        while pos < n && !is_ws(chars[pos]) {
            w = adv(w, chars[pos]);
            pos += 1;
        }
        w
    };
    let mut out = String::new();
    let mut col = 0usize;
    let mut pos = 0usize;
    // Leading whitespace run: verbatim.
    while pos < n && is_ws(chars[pos]) {
        col = adv(col, chars[pos]);
        out.push(chars[pos]);
        pos += 1;
    }
    while pos < n {
        // Word run.
        while pos < n && !is_ws(chars[pos]) {
            col = adv(col, chars[pos]);
            out.push(chars[pos]);
            pos += 1;
        }
        if pos >= n {
            break;
        }
        // Whitespace gap.
        let gstart = pos;
        while pos < n && is_ws(chars[pos]) {
            pos += 1;
        }
        let gap = &chars[gstart..pos];
        let first_nl = gap.iter().position(|&c| c == '\n');
        let nls = gap.iter().filter(|&&c| c == '\n').count();
        if nls >= 2 || pos >= n {
            // Paragraph separator (or trailing run): verbatim from the
            // first newline on; a trailing run without two newlines is
            // dropped entirely.
            if let Some(f) = first_nl.filter(|_| nls >= 2) {
                for &c in &gap[f..] {
                    col = adv(col, c);
                    out.push(c);
                }
            }
        } else if col + 1 + word_width(pos) <= width.max(0) as usize {
            out.push(' ');
            col += 1;
        } else {
            out.push('\n');
            col = 0;
        }
    }
    Ok(Value::string(out))
}
fn f_string_lines(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    // GNU: nil splits to nil.
    if args[0].is_nil() {
        return Ok(Value::Nil);
    }
    // `(length STRING)' evals first — atoms signal `sequencep';
    // conses/vectors pass length then fail `stringp' at string-search.
    let s = match &args[0] {
        Value::Str(s) => s.borrow().clone(),
        Value::Cons(_) | Value::Vec(_) => {
            return Err(i.wrong_type_mut("stringp", &args[0]));
        }
        other => return Err(i.wrong_type_mut("sequencep", other)),
    };
    let omit = arg(&args, 1).truthy();
    let keep = arg(&args, 2).truthy();
    // (string-lines STRING &optional OMIT-EMPTY KEEP-NEWLINES) —
    // GNU's subr.el loop walks `string-search' hits, so a trailing
    // newline never yields a final empty element, and a `""' input
    // splits to `(""' (nil when OMIT-EMPTY).
    if s.is_empty() {
        return Ok(if omit {
            Value::Nil
        } else {
            Value::list(vec![Value::string("")])
        });
    }
    let bytes = s.as_bytes();
    let mut lines: Vec<&str> = Vec::new();
    let mut start = 0usize;
    while start < bytes.len() {
        match bytes[start..].iter().position(|b| *b == b'\n') {
            Some(off) => {
                let nl = start + off;
                if !omit || start != nl {
                    let end = if keep { nl + 1 } else { nl };
                    let line = &s[start..end];
                    if !(keep && omit && line == "\n") {
                        lines.push(line);
                    }
                }
                start = nl + 1;
            }
            None => {
                lines.push(&s[start..]);
                break;
            }
        }
    }
    Ok(Value::list(lines.into_iter().map(Value::string).collect()))
}
fn f_string_equal_ignore_case(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    let a = want_string(i, &args[0])?;
    let b = want_string(i, &args[1])?;
    Ok(Value::from_bool(a.to_lowercase() == b.to_lowercase()))
}
fn f_char_equal(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    // GNU CHECK_CHARACTER: int in [0, 0x3FFFFF]; anything else is
    // `wrong-type-argument characterp'.
    let mut check = |v: &Value| -> Result<i128, Flow> {
        match v {
            Value::Int(n) if (0..=0x3FFFFF).contains(n) => Ok(*n),
            v => Err(i.wrong_type_mut("characterp", v)),
        }
    };
    let a = check(&args[0])?;
    let b = check(&args[1])?;
    // Emacs consults `case-fold-search' (default t in -Q).
    let fold = i
        .symbol_value(i.q_soft("case-fold-search").unwrap_or(0))
        .truthy();
    if a == b {
        return Ok(Value::t());
    }
    if !fold {
        return Ok(Value::Nil);
    }
    let multibyte = i
        .intern_soft("enable-multibyte-characters")
        .map(|id| i.symbol_value(id).truthy())
        .unwrap_or(true);
    let char_code = |code: i128| {
        if !multibyte && (128..=255).contains(&code) {
            0x3fff00 + code as u32
        } else {
            code as u32
        }
    };
    let down = i.current_case_table();
    Ok(Value::from_bool(
        ct_case(i, &down, char_code(a)) == ct_case(i, &down, char_code(b)),
    ))
}

fn f_char_or_string_p(_i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    Ok(Value::from_bool(match &args[0] {
        // GNU CHARACTERP: valid character code only (0..=0x3FFFFF).
        Value::Int(n) => (0..=0x3FFFFF).contains(n),
        Value::Str(_) => true,
        _ => false,
    }))
}
fn f_string_search(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    let needle = want_string(i, &args[0])?;
    let hay = want_string(i, &args[1])?;
    let start = match args.get(2) {
        Some(v) if !v.is_nil() => super::want_fixnum(i, v)?,
        _ => 0,
    };
    let hchars: Vec<char> = hay.chars().collect();
    // Validate in character positions before either an empty-needle
    // return or conversion to usize, as GNU's Fstring_search does.
    if !(0..=hchars.len() as i128).contains(&start) {
        let sym = i.q("args-out-of-range");
        return Err(i.signal_data(sym, vec![Value::Int(start)]));
    }
    let nchars: Vec<char> = needle.chars().collect();
    if nchars.is_empty() {
        return Ok(Value::Int(start));
    }
    let mut idx = start as usize;
    while idx + nchars.len() <= hchars.len() {
        if hchars[idx..idx + nchars.len()] == nchars[..] {
            return Ok(Value::Int(idx as i128));
        }
        idx += 1;
    }
    Ok(Value::Nil)
}
fn f_string_version_lessp(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    // GNU calls gnulib's filenvercmp; symbols compare by name.
    macro_rules! name_of {
        ($v:expr) => {
            match $v {
                Value::Str(s) => s.borrow().clone(),
                Value::Sym(s) => i.symbol_name(*s).to_string(),
                _ => return Err(i.wrong_type_mut("stringp", $v)),
            }
        };
    }
    let a = name_of!(&args[0]);
    let b = name_of!(&args[1]);
    Ok(Value::from_bool(
        filevercmp(
            &a.chars().collect::<Vec<_>>(),
            &b.chars().collect::<Vec<_>>(),
        ) == std::cmp::Ordering::Less,
    ))
}

/// gnulib `verrevcmp' (dpkg order): non-digit runs compare via
/// `order' (~=-2 < end=-1 < digits=0 < letters=byte < others=byte+256);
/// digit runs compare numerically (leading zeros skipped; first
/// differing digit wins; the longer run wins a prefix tie).  Emacs's
/// `string-version-lessp' calls `filenvercmp', which wraps this with
/// a file-suffix split that only matters for `x.y' names.
fn verrevcmp(s1: &[char], s2: &[char]) -> std::cmp::Ordering {
    use std::cmp::Ordering;
    fn order(s: &[char], pos: usize) -> i64 {
        if pos >= s.len() {
            return -1;
        }
        let c = s[pos];
        if c.is_ascii_digit() {
            0
        } else if c.is_ascii_alphabetic() {
            c as i64
        } else if c == '~' {
            -2
        } else {
            (c as i64) + 256 // UCHAR_MAX+1
        }
    }
    let (mut p1, mut p2) = (0usize, 0usize);
    while p1 < s1.len() || p2 < s2.len() {
        let mut first_diff: i64 = 0;
        while (p1 < s1.len() && !s1[p1].is_ascii_digit())
            || (p2 < s2.len() && !s2[p2].is_ascii_digit())
        {
            let d = order(s1, p1) - order(s2, p2);
            if d != 0 {
                return d.cmp(&0);
            }
            p1 += 1;
            p2 += 1;
        }
        while p1 < s1.len() && s1[p1] == '0' {
            p1 += 1;
        }
        while p2 < s2.len() && s2[p2] == '0' {
            p2 += 1;
        }
        while p1 < s1.len() && p2 < s2.len() && s1[p1].is_ascii_digit() && s2[p2].is_ascii_digit() {
            if first_diff == 0 {
                first_diff = s1[p1] as i64 - s2[p2] as i64;
            }
            p1 += 1;
            p2 += 1;
        }
        if p1 < s1.len() && s1[p1].is_ascii_digit() {
            return Ordering::Greater;
        }
        if p2 < s2.len() && s2[p2].is_ascii_digit() {
            return Ordering::Less;
        }
        if first_diff != 0 {
            return first_diff.cmp(&0);
        }
    }
    Ordering::Equal
}

fn filevercmp(s1: &[char], s2: &[char]) -> std::cmp::Ordering {
    verrevcmp(s1, s2)
}
fn f_string_distance(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    want_string(i, &args[0])?;
    want_string(i, &args[1])?;
    let units = |v: &Value| -> Vec<i128> {
        let Value::Str(s) = v else { unreachable!() };
        if arg(&args, 2).truthy() {
            gnu_sbytes(i, s).into_iter().map(i128::from).collect()
        } else {
            s.borrow()
                .chars()
                .map(|c| {
                    if i.is_unibyte_str(s) {
                        eight_bit_byte(c).map(i128::from).unwrap_or(c as i128)
                    } else {
                        lisp_char_code(c)
                    }
                })
                .collect()
        }
    };
    let a = units(&args[0]);
    let b = units(&args[1]);
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
fn f_string_glyph_split(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    let length = Value::Sym(i.q("length"));
    let len = i.apply(&length, vec![args[0].clone()])?.int().unwrap_or(0);
    let mut pieces = Vec::new();
    let mut start = 0;
    while start < len {
        let comp = misc::f_find_composition_internal(
            i,
            vec![
                Value::Int(start),
                Value::Int(start + 1),
                args[0].clone(),
                Value::Nil,
            ]
            .into(),
        )?;
        let end = comp
            .list_to_vec()
            .ok()
            .and_then(|range| range.get(1).and_then(Value::int))
            .unwrap_or(start + 1);
        pieces.push(f_substring(
            i,
            vec![args[0].clone(), Value::Int(start), Value::Int(end)].into(),
        )?);
        start = end;
    }
    Ok(Value::list(pieces))
}

// GNU sxhash (src/fns.c): EMACS_UINT is 64-bit, fixnums are 62-bit.
// sxhash_combine (x, y) = (x << 4) + (x >> 60) + y   (wrapping)
// SXHASH_REDUCE (x)     = (x ^ (x >> 2)) & (2^62 - 1)
// The reduced value is stored via make_ufixnum, so bit 61 set reads
// as a negative fixnum.
const SXHASH_MAX_DEPTH: usize = 3;
const SXHASH_MAX_LEN: usize = 7;

// GNU fixnums span -2^61 .. 2^61-1. Wider integers hash as bignums.
const SXHASH_FIXNUM_MIN: i128 = -(1i128 << 61);
const SXHASH_FIXNUM_MAX: i128 = (1i128 << 61) - 1;

// Pseudovector header bits from GNU lisp.h, folded into ASIZE when
// sxhash_vector hashes a pseudovector (char-tables, records).
// PSEUDOVECTOR_FLAG = PTRDIFF_MAX - PTRDIFF_MAX/2 (bit 62);
// type sits at PSEUDOVECTOR_AREA_BITS = 24 (size 12b + rest 12b).
const PSEUDOVECTOR_FLAG: u64 = 0x4000_0000_0000_0000;
const PVEC_CHAR_TABLE: u64 = 32;
const PVEC_RECORD: u64 = 34;
const CHAR_TABLE_STD_SLOTS: usize = 68; // GNU CHAR_TABLE_STANDARD_SLOTS

fn sxhash_combine(x: u64, y: u64) -> u64 {
    x.wrapping_shl(4).wrapping_add(x >> 60).wrapping_add(y)
}

fn sxhash_reduce(x: u64) -> i128 {
    let v = ((x ^ (x >> 2)) & 0x3fff_ffff_ffff_ffff) as i128;
    if v >= (1i128 << 61) {
        v - (1i128 << 62)
    } else {
        v
    }
}

/// GNU `hash_char_array': byte-wise hash over the string's byte
/// storage (Emacs internal UTF-8 — same bytes as our UTF-8 storage).
pub(crate) fn sxhash_char_array(bytes: &[u8]) -> u64 {
    let len = bytes.len();
    let mut hash = len as u64;
    let step = (len >> 3).max(8);
    if len >= 8 {
        let mut p = 0usize;
        while p + 8 <= len {
            let c = u64::from_le_bytes(bytes[p..p + 8].try_into().unwrap());
            hash = sxhash_combine(hash, c);
            p += step;
        }
        let c = u64::from_le_bytes(bytes[len - 8..len].try_into().unwrap());
        hash = sxhash_combine(hash, c);
    } else {
        let mut tail: u64 = 0;
        let mut p = 0usize;
        if len >= 4 {
            let c = u32::from_le_bytes(bytes[0..4].try_into().unwrap());
            tail = c as u64;
            p = 4;
        }
        if len - p >= 2 {
            let c = u16::from_le_bytes(bytes[p..p + 2].try_into().unwrap());
            tail = (tail << 16) + c as u64;
            p += 2;
        }
        if p < len {
            tail = (tail << 8) + bytes[p] as u64;
        }
        hash = sxhash_combine(hash, tail);
    }
    hash
}

/// The raw bytes GNU hashes for a string object (SDATA/SBYTES).
/// Unibyte strings hash their byte storage — byte 0xFF is one byte —
/// while multibyte strings hash Emacs' UTF-8 variant, which encodes
/// raw-byte chars (our eight-bit proxies) as BYTE8_STRING
/// (0xC0|((b>>6)&1), 0x80|(b&0x3F)), NOT UTF-8.
pub(crate) fn gnu_sbytes(i: &Interp, s: &StrRef) -> Vec<u8> {
    let text = s.borrow();
    if i.is_unibyte_str(s) {
        return text
            .chars()
            .flat_map(|c| {
                if let Some(b) = eight_bit_byte(c) {
                    vec![b]
                } else if (c as u32) <= 0xFF {
                    vec![c as u8]
                } else {
                    // Degenerate input (multibyte char in a unibyte
                    // string can't occur in GNU); hash its UTF-8 so
                    // the result still differs by content.
                    c.to_string().into_bytes()
                }
            })
            .collect();
    }
    let mut out = Vec::with_capacity(text.len());
    for c in text.chars() {
        if let Some(b) = eight_bit_byte(c) {
            out.push(0xC0 | ((b >> 6) & 0x01));
            out.push(0x80 | (b & 0x3F));
        } else {
            out.extend(super::coding_legacy::internal_encode(
                lisp_char_code(c) as u32
            ));
        }
    }
    out
}

/// GNU `sxhash_bignum' (fns.c): seed with the sign flag, then fold
/// each 64-bit limb of the magnitude.
fn sxhash_bignum(n: i128) -> u64 {
    let mut hash = if n < 0 { 1 } else { 0 };
    let mut rest = n.unsigned_abs();
    while rest > 0 {
        hash = sxhash_combine(hash, (rest & u64::MAX as u128) as u64);
        rest >>= 64;
    }
    hash
}

/// GNU `sxhash_list': hash up to SXHASH_MAX_LEN cars, then the
/// leftover tail (dotted or truncated); depth-capped so circular
/// lists terminate.
fn sxhash_list(i: &Interp, list: &Value, depth: usize) -> u64 {
    let mut hash = 0u64;
    let mut tail = list.clone();
    if depth < SXHASH_MAX_DEPTH {
        let mut n = 0;
        while n < SXHASH_MAX_LEN {
            let Value::Cons(c) = &tail else { break };
            let (car, cdr) = {
                let b = c.borrow();
                (b.car.clone(), b.cdr.clone())
            };
            hash = sxhash_combine(hash, sxhash_obj(i, &car, depth + 1));
            tail = cdr;
            n += 1;
        }
    }
    if !matches!(tail, Value::Nil) {
        hash = sxhash_combine(hash, sxhash_obj(i, &tail, depth + 1));
    }
    hash
}

/// A stand-in for GNU's XHASH (raw pointer >> 2). Session-stable is
/// all we need; GNU's value is the object's address.
fn sxhash_xhash_ptr<T>(rc: &Rc<T>) -> u64 {
    (Rc::as_ptr(rc) as usize as u64) >> 2
}

/// GNU `sxhash_bool_vector': seed with the bit count, fold up to
/// SXHASH_MAX_LEN packed u64 words (little-endian bit order, same
/// packing as GNU's bool_vector_data).
fn sxhash_bool_vector(i: &Interp, v: &Value) -> u64 {
    let bits = match v {
        Value::Record(r) => {
            let rr = r.borrow();
            match rr.get(1) {
                Some(Value::Vec(bv)) => bv
                    .borrow()
                    .iter()
                    .map(|x| matches!(x, Value::Int(n) if *n != 0))
                    .collect::<Vec<bool>>(),
                _ => Vec::new(),
            }
        }
        _ => Vec::new(),
    };
    let _ = i;
    let mut hash = bits.len() as u64;
    for (w, chunk) in bits.chunks(64).enumerate() {
        if w >= SXHASH_MAX_LEN {
            break;
        }
        let mut word = 0u64;
        for (k, bit) in chunk.iter().enumerate() {
            if *bit {
                word |= 1u64 << k;
            }
        }
        hash = sxhash_combine(hash, word);
    }
    hash
}

/// GNU hashes markers as
/// `sxhash_combine((intptr_t) marker->buffer, marker->bytepos)'; a
/// buffer-less marker is combine(0, 0) → 0.
fn sxhash_marker(i: &Interp, m: &crate::lisp::value::Marker) -> u64 {
    let buf_addr: u64 = match m.buffer {
        Some(bid) => i
            .buffers
            .get(bid)
            .map(|b| Rc::as_ptr(&b) as usize as u64)
            .unwrap_or(0),
        None => 0,
    };
    // Our positions are 0-based chars; GNU's bytepos is 1-based.
    let bytepos = if m.buffer.is_some() {
        m.position as u64 + 1
    } else {
        0
    };
    sxhash_combine(buf_addr, bytepos)
}

/// GNU hashes overlays (PVEC_OVERLAY) by start, end, then plist —
/// not elementwise. Remacs overlays are `[overlay BID IDX]' handles;
/// GNU positions are the itree bounds (0-based start, 1-based end
/// bytepos), or -1 each once detached.
fn sxhash_overlay(i: &Interp, vv: &[Value], depth: usize) -> Option<u64> {
    let (Value::Sym(tag), Value::Int(bid), Value::Int(idx)) = (vv.first()?, vv.get(1)?, vv.get(2)?)
    else {
        return None;
    };
    if Some(*tag) != i.q_soft("overlay") {
        return None;
    }
    let ov = i
        .buffers
        .get(*bid as usize)
        .and_then(|b| b.borrow().overlays.get(*idx as usize).cloned());
    let (start, end, plist) = match ov.as_ref().and_then(|o| o.buffer.map(|_| o)) {
        Some(o) => (o.start as u64, o.end as u64 + 1, o.plist.clone()),
        // Detached/deleted overlay: GNU's itree bounds are -1 each.
        None => (
            u64::MAX,
            u64::MAX,
            ov.map(|o| o.plist).unwrap_or(Value::Nil),
        ),
    };
    let mut hash = start;
    hash = sxhash_combine(hash, end);
    hash = sxhash_combine(hash, sxhash_obj(i, &plist, depth));
    Some(hash)
}

/// GNU `sxhash_vector' for a PVEC_CHAR_TABLE: seed with ASIZE
/// (pseudovector flag | type | slot count) and hash the first
/// SXHASH_MAX_LEN Lisp slots in layout order — defalt, parent,
/// purpose, ascii, then contents[].  A remacs char-table is a Record
/// [char-table SUBTYPE CONTENTS65 EXTRAS..] whose defalt and parent
/// live in side-tables.
fn sxhash_char_table(i: &Interp, v: &Value, r: Ref<'_, Vec<Value>>, depth: usize) -> u64 {
    let extras = r.len().saturating_sub(3);
    let mut hash =
        PSEUDOVECTOR_FLAG | (PVEC_CHAR_TABLE << 24) | (CHAR_TABLE_STD_SLOTS + extras) as u64;
    let defalt = i.char_table_defalt(v);
    let parent = i.char_table_parent(v);
    let purpose = r.get(1).cloned().unwrap_or(Value::Nil);
    let contents: Vec<Value> = match r.get(2) {
        Some(Value::Vec(c)) => c.borrow().clone(),
        _ => Vec::new(),
    };
    let slots = [defalt, parent, purpose]
        .into_iter()
        .chain(contents.into_iter());
    for slot in slots.take(SXHASH_MAX_LEN) {
        hash = sxhash_combine(hash, sxhash_obj(i, &slot, depth + 1));
    }
    hash
}

/// GNU `lispsym' builtin (DEFSYM) symbols in slot order: line K names
/// the symbol at lispsym index K, so its `sxhash' is SXHASH_REDUCE
/// (12 * K).  Empty lines are slots not interned at -Q.  Extracted from
/// GNU Emacs 31.1 -Q by inverting `(sxhash SYM)' over the symbols whose
/// hash is stable across runs (dumped/heap symbols hash by ASLR'd
/// address, so only lispsym members are stable).
const GNU_BUILTIN_SYMBOL_NAMES: &str = "nil
t

error
lambda
AEAD-ciphers
Automatic GC
CLIPBOARD
:adstyle
:advertised-binding
:align-to
:anchor
:animate-buffer
:animate-position
:animate-tardiness
:antialias
:array-type
:ascent
:ascii-compatible-p
:avgwidth
:background
:base-uri
:bold
:box
:buffer
:button
:bytepos
:bytesize
:category
:certificate
:certificate-id
:certificate-security-level
:certificates
:cipher
:cipher-aead-capable
:cipher-blocksize
:cipher-id
:cipher-ivsize
:cipher-keysize
:cipher-tagsize
:coding
:col
:color
:color-adjustment
:color-symbols
:combining-capability
:command
:complete-negotiation
:compression
:connection-type
:conversion
:crlfiles
:crop
:css
:data
:debug-on-exit
:decode-translation-table
:default-char
:destination
:device
:diffie-hellman-prime-bits
:digest-algorithm-id
:digest-algorithm-length
:distant-foreground
:documentation
:dpi
:emergency
:enable
:encode-translation-table
:encrypt-then-mac
:eq?
:equal
:error
:eval
:expired
:extend
:false
:false-object
:family
:file
:file-handler
:filter
:filtered
:flags
:flip
:flowcontrol
:font
:font-entity
:fontset
:foreground
:format
:foundry
:function
:height
:help
:heuristic-mask
:host
:hostname
:ignore-defface
:image
:in-place
:index
:inherit
:insecure
:invalid
:invalid-ocsp-status
:inverse-video
:issuer
:issuer-unique-id
:italic
:key
:key-exchange
:key-sequence
:keylist
:keys
:label
:lang
:lessp
ClientHello Padding
:line
:line-width
:loader
:local
:log
:loglevel
:mac
:mac-algorithm-id
:mac-algorithm-keysize
:mac-algorithm-length
:mac-algorithm-noncesize
:map
:margin
:mask
:match
:match?
:matrix
:max-height
:max-width
:min-prime-bits
:minspace
:missing-ocsp-status
:mnemonic
:mouse
:name
:no-host-match
:noquery
:not-activated
:not-ca
:nowait
:null
:null-object
:object-type
:ordinary
:otf
:overline
:parity
:pass
:pem
:plist
:+
:pointer
:port
:pos
:position
:post-read-conversion
:pre-write-conversion
:pred
:pred?
:priority
:process
:propertize
:protocol
:pt-height
:pt-width
:public-key-algorithm
:public-key-id
:public-key-id-sha256
:purecopy
:purpose-mismatch
:?
:radio
:registry
:rehash-size
:rehash-threshold
:relative-height
:relative-width
:relief
:remote
:reverse
:revocation-data-issued-in-future
:revocation-data-superseded
:revoked
:rotation
:rtl
:safe-renegotiation
:scalable
:scale
:script
:self-signed
:sentinel
:serial-number
:server
:service
:signature-algorithm
:signature-failure
:signer-constraints-failure
:size
:slant
:spacing
:speed
:*
:stderr
:stipple
:stop
:stopbits
:strike-through
:style
:subject
:subject-unique-id
:success
:summary
:test
:tls-parameters
:toggle
:transform-smoothing
:trustfiles
:type
:underline
:unknown-ca
:use-external-socket
:user-spec
:valid-from
:valid-to
:verify-error
:verify-flags
:version
:vert-only
:visible
:volume
:warnings
:weakness
:weight
:width
:window
:wrap
Discarded Samples
FILE_NAME
L2R
PRIMARY
R2L
SECONDARY
TARGETS
TEXT
UTF8_STRING
Unknown error
XdndActionCopy
XdndActionLink
XdndActionMove
XdndActionPrivate
above
above-handle
above-suspended
access-file
activate
activate-front
activate-input-method
activate-mark-hook
activate-menubar-hook
1+
add-name-to-file
add-to-history
after-change-functions
after-delete-frame-functions
after-handle
after-insert-file-set-buffer-file-coding-system
after-insert-file-set-coding
after-pdump-load-hook
after-string
alist
all
alloc
alpha
alpha-background
alt
and
&optional
&rest
android
anonymous
append
apply
aref
args
args-out-of-range
arith-error
array
arrayp
arrow
ascii
ascii-0
ascii-character
aset
ask-user-about-lock
assq-delete-all
assume
attrib
auto
auto-composed
auto-fill-chars
auto-hscroll-mode
auto-lower
auto-raise
auto-save
auto-save-coding
auto-save-hook
autoload
autosaved
background-color
background-mode
`
bar
bare-symbol-p
before-change-functions
before-handle
before-string
beginning-of-buffer
below
below-handle
big
big5
bignum
binary
bitmap-spec-p
black
bmp
bold
book
bool-vector
bool-vector-p
boolean
booleanp
border
border-color
border-width
borders-respect-alpha-background
both
both-horiz
bottom
bottom-divider
bottom-divider-width
bottom-edge
bottom-left-corner
bottom-right-corner
boundary
bounds
box
buffer
buffer-access-fontify-functions
buffer-file-coding-system
buffer-file-name
buffer-file-number
buffer-list
buffer-list-update-hook
buffer-name-history
buffer-or-string-p
buffer-position
buffer-predicate
buffer-read-only
buffer-save-without-query
buffer-stale-function
buffer-undo-list
buffered
bufferp
buffers
buried-buffer-list
byte-code-function
byte-code-function-p
byte-code-meter
byte-run--unescaped-character-literals-warning
c
call
call-process
call-process-region
callref
cancel-timer
canonical-combining-class
car
car-less-than-car
case-fold-search
case-symbols-as-words
case-table
case-table-p
catcher
category
category-table
category-table-p
categoryp
categorysetp
ccl
ccl-program-idx
cclp
cdr
ceiling
center
ch
change_frame_size
change-major-mode-hook
char-code-property-table
char-from-name
char-or-string-p
char-script-table
char-table
char-table-extra-slots
char-table-p
characterp
charset
charsetp
child-frame-border
child-frame-border-width
choice
ciphers
circle
circular-list
clone-indirect-buffer-hook
clone-of
cloned-from
close-tab
closed
cmajflt
cminflt
cocoa
code-conversion-map
code-conversion-map-id
codeset
coding
coding-system
coding-system-define-form
coding-system-error
coding-system-for-write
coding-system-history
coding-system-p
color
columns
comm
,
,@
command-debug-status
command-error-default-function
command-execute
command-history
command-line-processed
command-modes
commandp
comment
comment-end-can-be-escaped
comp-libgccjit-reproducer
comp-maybe-gc-or-quit
comp-mvar
comp-sanitizer-error
comp-subr-trampoline-install
completing-read-function
completion-ignore-case
composition
concat
cond-jump
cond-jump-narg-leq
condensed
condition-case
condition-variable
condition-variable-p
config-changed-event
configuration
connect
cons
conses
consp
continuation
control
copy-directory
copy-file
count
create
critical
crop
cstime
ctime
current
current-input-method
current-key-remap-sequence
current-line
current-load-list
current-minibuffer-command
cursor
cursor-color
cursor-in-echo-area
cursor-type
curve
custom-delayed-init-variables
custom-variable-history
custom-variable-p
cutime
cw
cycle-sort-function
cyclic-function-indirection
cyclic-variable-indirection
d
d-default
d-ephemeral
dark
dashes
data
datagram
days
dbus-event
deactivate-mark
debug
debug-early
debug-early--handler
debug-early--muted
debug-on-next-call
debugger
debugger-may-continue
decomposed-characters
dedicated
defalias-fset-function
default
default-directory
default-keyboard-coding-system
default-terminal-coding-system
define-charset-internal
define-coding-system-internal
defun
defvaralias
delay
delayed-warnings-hook
delete
delete-auto-save-file-if-necessary
delete-before
delete-by-moving-to-trash
delete-directory
delete-file
delete-file-internal
delete-frame
delete-frame-functions
delete-terminal-functions
delete-window
digests
dir-ok
direct-call
direct-callref
direct-color
directory-file-name
directory-files
directory-files-and-attributes
disable-eval
disabled
display
display-buffer
display-fill-column-indicator
display-fill-column-indicator-character
display-fill-column-indicator-column
display-line-numbers
display-line-numbers-disable
display-line-numbers-offset
display-line-numbers-widen
display-line-numbers-width
display-monitors-changed-functions
display-table
display-type
display-warning
do-after-load-evaluation
domain-error
dont-clear-message
dos
dots
double-line
down
drag-internal-border
drag-n-drop
drag-source
drag-with-header-line
drag-with-mode-line
drag-with-tab-line
dragging
dropping
dump-emacs-portable--sort-predicate
dump-emacs-portable--sort-predicate-copied
dump-file-name
dumped-with-pdumper
echo-area-clear-hook
echo-keystrokes
edge-detection
egid
eight-bit
elt
em
emacs
emacs-mule
emboss
emoji
empty-box
empty-line
enable-recursive-minibuffers
encode-time
encoded
end-of-buffer
end-of-file
end-scroll
end-session
entry
eq
eql
equal
error-conditions
error-message
escape-glyph
etime
euid
eval
eval-buffer-list
eval-minibuffer
evaporate
even
event-kind
event-symbol-element-mask
event-symbol-elements
excessive-lisp-nesting
excessive-variable-binding
excl
exec-suffixes
exit
expand-abbrev
expand-file-name
expanded
explicit
explicit-name
expose-hidden-buffer
extend
extension-data
external-border-size
external-debugging-output
extra
extra-bold
extra-light
f0
f10
face
face-alias
face-no-inherit
face-remapping-alist
face-set-after-frame-default
failed
false
fboundp
features
fetch-handler
field
file
file-accessible-directory-p
file-acl
file-already-exists
file-attributes
file-attributes-lessp
file-date-error
file-directory-p
file-error
file-executable-p
file-exists-p
file-locked-p
file-missing
file-modes
file-name-all-completions
file-name-as-directory
file-name-case-insensitive-p
file-name-completion
file-name-directory
file-name-handler-alist
file-name-history
file-name-nondirectory
file-newer-than-file-p
file-notify
file-notify-error
file-offset
file-readable-p
file-regular-p
file-remote-p
file-selinux-context
file-symlink-p
file-system-info
file-truename
file-writable-p
filenamep
fill-column-indicator
finalizer
first-change-hook
fixnum
fixnum-or-symbol-with-pos-p
fixnump
flat-button
float
float-output-format
floatp
floats
floor
focus-in
focus-out
font
font-backend
font-driver-superseded-by
font-entity
font-extra-type
font-lock-face
font-object
font-parameter
font-spec
fontification-functions
fontified
fontset
fontset-info
fontset-startup
fontsize
force
foreground-color
format-annotate-function
format-decode
format-prompt
fraction
frame
frame--z-order-lessp
frame-edges
frame-id
frame-initial-p
frame-live-p
frame-monitor-attributes
frame-set-background-mode
frame-title-format
frame-windows-min-size
framep
frames
fringe
from--tty-menu-p
front-sticky
full
fullboth
fullheight
fullscreen
fullwidth
funcall
funcall-interactively
function
function-documentation
function-history
function-key
functionp
fundamental-mode
gc-cons-percentage
gc-cons-threshold
gccjit
gdiplus
gdk-pixbuf
geometry
get-buffer-window-list

get-file-buffer

get-mru-frame
get-mru-window
get-scratch-buffer-create
gif
gio
glib
glyphless-char
glyphless-char-display
gnustep
gnutls
gnutls3
gnutls-anon
gnutls-code
gnutls-e-again
gnutls-e-interrupted
gnutls-e-invalid-session
gnutls-e-not-ready-for-handshake
GNUTLS_PKCS_NULL_PASSWORD
GNUTLS_PKCS_PBES1_DES_MD5
GNUTLS_PKCS_PBES2_3DES
GNUTLS_PKCS_PBES2_AES_128
GNUTLS_PKCS_PBES2_AES_192
GNUTLS_PKCS_PBES2_AES_256
GNUTLS_PKCS_PBES2_DES
GNUTLS_PKCS_PBES2_GOST_CPA
GNUTLS_PKCS_PBES2_GOST_CPB
GNUTLS_PKCS_PBES2_GOST_CPC
GNUTLS_PKCS_PBES2_GOST_CPD
GNUTLS_PKCS_PBES2_GOST_TC26Z
GNUTLS_PKCS_PKCS12_3DES
GNUTLS_PKCS_PKCS12_ARCFOUR
GNUTLS_PKCS_PKCS12_RC2_40
GNUTLS_PKCS_PLAIN
gnutls-symmetric-cipher
gnutls-digest-algorithm
gnutls-mac-algorithm
gnutls-x509pki
gobject
grave
gray-scale
group
grow-only
gs-load-image
gui_figure_window_size
gui-set-selection
haiku
hand
handle
handle-select-window
handle-shift-selection
handle-switch-frame
has-error
hash-table
hash-table-p
hash-table-test
haskell
hbar
hdrag
header-line
header-line-active
header-line-format
header-line-inactive
heap
heavy
heic
height
height-only
help--append-keystrokes-help
help--describe-map-tree
help-echo
help-echo-inhibit-substitution
help-form-show
help-key-binding
helper_sanitizer_assert
helper_save_restriction
helper_unbind_n
helper_unwind_protect
heuristic
hex-code
hide
hollow
hollow-small
horizontal-handle
horizontal-scroll-bar
horizontal-scroll-bars
hourglass
hw
hyper
icon
icon-left
icon-name
icon-title-format
icon-top
icon-type
iconify-frame
iconify-top-level
identity
if
if-regular
ignore-self-insert
ignored
image
image-format-suffixes
imagemagick
inc-args
informational
inhibit
inhibit-changing-match-data
inhibit-debugger
inhibit-double-buffering
inhibit-eval-during-redisplay
inhibit-file-name-operation
inhibit-free-realized-faces
inhibit-menubar-update
inhibit-modification-hooks
inhibit-point-motion-hooks
inhibit-quit
inhibit-read-only
inhibit-redisplay
inhibited-interaction
initial-major-mode
inner-edges
input-method-exit-on-first-char
input-method-use-echo-area
insert-behind-hooks
insert-file-contents
insert-in-front-hooks
inserted-chars
insufficient-source
intangible
integer
integer-or-marker-p
integerp
interactive
interactive-args
interactive-form
interactive-p
internal--syntax-propertize
internal-auto-fill
internal-border
internal-border-width
internal-complete-buffer
internal-default-interrupt-process
internal-default-process-filter
internal-default-process-sentinel
internal-default-signal-process
internal-echo-keystrokes-prefix

internal-macroexpand-for-load
internal-timer-start-idle
internal-when-entered-debugger
interpreted-function
interrupt-process-functions
interrupted
intervals
invalid-arity
invalid-function
invalid-read-syntax
invalid-regexp
invalid-source
invisible
ipv4
ipv6
iso10646-1
iso8859-1
iso-2022
iso-8859-1
italic
iv-auto
ja
jka-compr-load-suffixes
jpeg
json-end-of-file
json-error
json-escape-sequence-error
json-invalid-surrogate-error
json-number-out-of-range-error
json-object-too-deep
json-out-of-memory
json-parse-error
json-trailing-content
json-utf8-decode-error
json-value-p
jump
kbd-macro-termination-hook
keep-ratio
key
key-and-value
key-or-value
key-parse
key-valid-p
keymap
keymap-canonicalize
keymapp
kill-buffer--possibly-save
kill-buffer-hook
kill-buffer-query-functions
kill-emacs
kill-emacs-hook
kill-forward-chars
ko
language-grammar-version-mismatch
language-change
laplace
last-arrow-position
last-arrow-string
last-nonmenu-event
late
latin
left
left-edge
left-fringe
left-fringe-help
left-margin
left-only
left-to-right
leftmost
let
let*
lexical-binding
libgif-version
libjpeg-version
libpng-version
light
line
line-height
line-number
line-number-current-line
line-number-major-tick
line-number-minor-tick
line-prefix
line-spacing
link
lisp-directory
list
list-or-vector-p
list-system-processes
listen
listp
little
live
load
load-file-name
load-force-doc-strings
load-in-progress
load-time
load-true-file-name
local
local-map
lock-file
long
long-line-optimizations-in-command-hooks
long-line-optimizations-in-fontification-functions
losing-value
lowercase
lread--unescaped-character-literals
m
mac
mac-ct
macro
macroexp--dynvars
macs
majflt
make-cursor-line-fully-visible
make-directory
make-directory-internal
make-frame-visible
make-initial-minibuffer-frame
make-invisible
make-lock-file-name
make-process
make-symbolic-link
make-window-start-visible
makunbound
malformed-keyword-arg-list
many
map-keymap-sorted
margin
mark-for-redisplay
mark-inactive
marker
markerp
maximized
md5
medium
memory-buffer-too-small
memory-info
menu
menu-bar
menu-bar-external
menu-bar-lines
menu-bar-size
menu-bar-update-hook
menu-enable
menu-item
message
messages-buffer-mode
meta
metadata
min-height
min-width
minflt
minibuffer
minibuffer-completing-file-name
minibuffer-completion-table
minibuffer-default
minibuffer-exit
minibuffer-exit-hook
minibuffer-follows-selected-frame
minibuffer-history
minibuffer-inactive-mode
minibuffer-mode
minibuffer-prompt
minibuffer-quit
minibuffer-quit-recursive-edit
minibuffer-setup-hook
-
missing
missing-module-init-function
mm-size
mode-class
mode-line
mode-line-active
mode-line-default-help-echo
mode-line-elt-no
mode-line-format
mode-line-inactive
modeline
modification-hooks
modifier-cache
modifier-value
module-function
module-function-p
module-init-failed
module-load-failed
module-not-gpl-compatible
module-open-failed
module-out-of-memory
monitors-changed
months
mouse
mouse-click
mouse-color
mouse-face
mouse-fixup-help-message
mouse-leave-buffer-hook
mouse-movement
mouse-wheel-frame
move-frame
move-toolbar
mru
mutex
mutexp
name
named
native--compile-async
native-comp-compiler-options
native-comp-debug
native-comp-driver-options
native-comp-function
native-comp-speed
native-comp-unit
native-comp-warning-on-missing-source
native-compiler
native-compiler-error
native-edges
native-ice
native-image
native-lisp-file-inconsistent
native-lisp-load-failed
native-lisp-wrong-reloc
natnump
negate
network
nhdrag
nice
no-accept-focus
no-catch
no-conversion
no-focus-on-map
no-other-frame
no-other-window
no-record
no-self-insert
no-special-glyphs
nobreak-hyphen
nobreak-space
noelisp
non-ascii
non-key-event
non-retained
none
normal
not
not-found
now
ns
ns-appearance
ns-drag-operation-copy
ns-drag-operation-generic
ns-drag-operation-link
ns-handle-drag-motion
ns-in-echo-area
ns-parse-geometry
ns-put-working-text
ns-transparent-titlebar
ns-unput-working-text
nsm-verify-connection
nth
null
number-or-marker-p
numberp
numeric
obarray
obarray-cache
obarrayp
object
oblique
oclosure-interactive-form
odd
only
open
open-network-stream
opentype
operations
or
outdated
outer-border-width
outer-edges
outer-position
outer-size
outer-window-id

overflow-error
overlay
overlay-arrow
overlay-arrow-bitmap
overlay-arrow-string
overlayp
override-redirect
overriding-local-map
overriding-plist-environment
overriding-terminal-local-map
overwrite-mode
overwrite-mode-binary
p
paper
parent-frame
parent-id
pbm
pc
pcpu
permanent-local
permanent-local-hook
permission-denied
pgrp
pgtk
phi
pinch
pipe
pipe-process-p
play-sound-functions
plist
plistp
+
pmem
png
point-entered
point-left
pointer
polling-period
poly
pop-handler
position
post-command-hook
post-gc-hook
post-select-region-hook
post-self-insert-hook
post-wake
postscript
ppid
pre-command-hook
pre-sleep
preedit-text
pressed-button
pri
primitive-function
princ

print-escape-multibyte
print-escape-nonascii
print-symbols-bare
print-unreadable-function
priority
process
process-attributes
processp
progn
propertize
protected-field
provide
pseudo-color
pty
purecopy
push-handler
push-window-buffer-onto-prev
quit
quit-restore
quit-restore-prev
quote
raise
range
range-error
ratio
raw-text
read
read-char
read-file-name
read-minibuffer
read-number
read-only
read-symbol-shorthands
real
real-this-command
rear-nonsticky
record
record_unwind_current_buffer
record_unwind_protect_excursion
record-window-buffer
recordp
rect
recursion-error
redisplay_internal (C function)
relative
released-button
remap
remote-file-error
rename
rename-auto-save-file
rename-file
replace-buffer-in-windows
require
reset
resume-tty-functions
retained
return
reverse-italic
revoke
right
right-divider
right-divider-width
right-edge
right-fringe
right-fringe-help
right-margin
right-to-left
rightmost
risky-local-variable
rotate
rotate90
rss
run
run-at-time
run-hook-query-error-with-timeout
run-hook-with-args
safe
safe-magic
save-excursion
save-session
scale
scale-factor
scan-error
scratch
screen-gamma
scroll-bar
scroll-bar-background
scroll-bar-foreground
scroll-bar-height
scroll-bar-movement
scroll-bar-width
scroll-command
scroll-down
scroll-up
search-failed
select-frame-set-input-focus
select-window
selection-request
self-insert-command
semi-bold
semi-light
seqpacket
sequencep
serial
sess
set
set-args-to-local
set-buffer-multibyte
set-default
set-file-acl
set-file-modes
set-file-selinux-context
set-file-times
set_internal
set-par-to-local
set-rest-args-to-local
set-visited-file-modtime
set_window_configuration
setcar
setcdr
setimm
setq
setting-constant
sha1
sha224
sha256
sha384
sha3-224
sha3-256
sha3-384
sha3-512
sha512
shaded
shift-jis
shlwapi
signal
signal-process-functions
sigusr2
silently
singularity-error
size
size-and-position
skip-taskbar
sleep-event
slice
sound
source
space
space-width
special-form
special-lowercase
special-titlecase
special-uppercase
sqlite
sqlite3
sqlite-error
sqlite-locked-error
sqlitep
standard-input
standard-output
start
start-process
state
static-gray
stderr
stdin
stdout
sticky
stime
stop
straight
string
string-bytes
string-lessp
string-replace
stringp
strings
1-
sub-char-table
subfeatures
subr
subr-native-elisp
subrp
substitute-command-keys
substitute-env-in-file-name
substitute-in-file-name
super
suppress-keymap
suspend-hook
suspend-resume-hook
suspend-tty-functions
svg
sw
switch-frame
symbol
symbol-error
symbol-with-pos
symbol-with-pos-p
symbolp
symbols
symbols-with-pos-enabled
syntax-ppss
syntax-ppss-flush-cache
syntax-table
syntax-table-p
tab-bar
tab-bar-lines
tab-bar-size
tab-line
tab-line-active
tab-line-format
tab-line-inactive
target-idx
temp-buffer-setup-hook
temp-buffer-show-hook
terminal
terminal_frame
terminal-live-p
test
text
text-conversion
text-image-horiz
text-pixels
text-property-default-nonsticky
text-read-only
thcount
thin
thin-space
thread
thread-buffer-killed
thread-event
threadp
tiff
time
timer-event-handler
tip_frame
title
title-bar-size
titlecase
tool-bar
tool-bar-external
tool-bar-lines
tool-bar-position
tool-bar-size
toolkit-theme-changed
toolkit-theme-set-functions
tooltip
top
top-bottom
top-edge
top-left-corner
top-level
top-only
top-right-corner
touch-end
touchscreen
touchscreen-begin
touchscreen-end
touchscreen-update
tpgid
trailing-whitespace
translation-table
translation-table-id
trapping-constant
tree-sitter
tree-sitter--library-abi
treesit-buffer_changed
treesit-buffer-too-large
treesit-compiled-query
treesit-compiled-query-p
treesit-error
treesit-invalid-predicate
treesit-language-remap-alist
treesit-load-language-error
treesit-node
treesit-node-buffer-killed
treesit-node-outdated
treesit-node-p
treesit-parse-error
treesit-parser
treesit-parser-deleted
treesit-parser-p
treesit-pattern-expand
treesit-predicate-not-found
treesit-query-error
treesit-query-p
treesit-range-invalid
treesit-thing-symbol
true-color
truncation
ttname
tty
tty-color-alist
tty-color-by-index
tty-color-desc
tty-color-mode
tty-color-standard-values
tty-defined-color-alist
tty-menu-disabled-face
tty-menu-enabled-face
tty-menu-exit
tty-menu-ignore
tty-menu-mouse-movement
tty-menu-navigation-map
tty-menu-next-item
tty-menu-next-menu
tty-menu-open-use-tmm
tty-menu-prev-item
tty-menu-prev-menu
tty-menu-select
tty-mode-reset-strings
tty-mode-set-strings
tty-non-selected-cursor
tty-select-active-regions
tty-set-up-initial-frame-faces
tty-type
type-mismatch
ultra-bold
ultra-heavy
ultra-light
undecided
undecorated
undefined
undeleted
underflow-error
underline-minimum-offset
undo-auto--add-boundary
undo-auto--last-boundary-cause
undo-auto--this-command-amalgamating
undo-auto--undoable-change
undo-auto--undoably-changed-buffers
undo-auto-amalgamate
unevalled
unhandled-file-name-directory
unicode
unicode-bmp
unicode-string-p
uniquify--rename-buffer-advice
unix
unlet
unlock-file
unreachable
unspecified
unsplittable
up
uppercase
url
us-ascii
use-frame-synchronization
use-region-p
user
user-emacs-directory
user-error
user-position
user-ptr
user-ptrp
user-search-failed
user-size
userlock--ask-user-about-supersession-threat
userlock--handle-unlock-error
utf-16
utf-16le
utf-8
utf-8-emacs
utf-8-string-p
utf-8-unix
utime
value
value<
variable-documentation
vdrag
vector
vector-or-char-table-p
vector-slots
vectorp
vectors
verify-visited-file-modtime
vertical-border
vertical-line
vertical-scroll-bar
vertical-scroll-bars
visibility
visible
visual
void-function
void-variable
vsize
w32
wait-for-wm
wall
watchers
wave
weakness
webp
webpdemux
when
wholenump
widget-type
width
width-only
window
window--pixel-to-total
window--resize-mini-frame
window--resize-root-window
window--resize-root-window-vertically
window-buffer-change-functions
window-configuration
window-configuration-change-hook
window-configuration-p
window-deletable-p
window-divider
window-divider-first-pixel
window-divider-last-pixel
window-edges
window-id
window-live-p
window-point-insertion-type
window-scroll-functions
window-selection-change-functions
window-size
window-size-change-functions
window-state-change-functions
window-state-change-hook
window-valid-p
windowp
workarea
wrap-prefix
write
write-region
write-region-annotate-functions
wrong-length-argument
wrong-number-of-arguments
wrong-register-subr-call
wrong-type-argument
x
x_create_frame_1
x_create_frame_2
x-frame-parameter
x-hide-tip
x-pre-popup-menu-hook
x-resource-name
x_set_menu_bar_lines
x_set_window_size_1
x-underline-at-descent-line
x-use-underline-position-properties
xbm
xg_frame_set_char_size
xpm
xterm--set-selection
xwidget
xwidget-display-event
xwidget-event
xwidget-view
y-or-n-p
yes-or-no-p
yes-or-no-p-history
z-group
zero-width";
const GNU_BUILTIN_SYMBOL_COUNT: u64 = 1712;

/// GNU `sxhash_obj' for a symbol is XHASH: the raw Lisp_Object
/// (offset from lispsym) >> 2.  Builtin symbols sit at fixed
/// sizeof(struct Lisp_Symbol)=48-byte slots, so hash = 12 * lispsym
/// index — deterministic across sessions, which is why GNU's
/// `sxhash-equal t/car/x` are small stable numbers.  Dynamically
/// interned symbols hash by heap offset in GNU (ASLR-dependent); we
/// assign a stable pseudo-offset past the builtin range.
fn sxhash_sym_hash(i: &Interp, id: SymId) -> u64 {
    let idx = gnu_builtin_symbol_index(&i.symbol_name(id))
        .unwrap_or(GNU_BUILTIN_SYMBOL_COUNT + id as u64);
    12 * idx
}

/// lispsym index of a GNU builtin symbol name, or None when the name
/// is interned dynamically (GNU hashes those by heap address).
fn gnu_builtin_symbol_index(name: &str) -> Option<u64> {
    GNU_BUILTIN_SYMBOL_NAMES
        .split('\n')
        .position(|n| n == name)
        .map(|k| k as u64)
}

fn sxhash_obj(i: &Interp, v: &Value, depth: usize) -> u64 {
    if depth > SXHASH_MAX_DEPTH {
        return 0;
    }
    // `symbols-with-pos-enabled' objects are `equal'/`eql' to their
    // bare symbol — hash the symbol so the invariant holds.
    if let Some((id, _)) = misc::as_sympos(v) {
        return sxhash_sym_hash(i, id);
    }
    match v {
        Value::BigInt(n) => {
            let (sign, limbs) = n.to_u64_digits();
            let mut hash = u64::from(sign == num_bigint::Sign::Minus);
            for limb in limbs { hash = sxhash_combine(hash, limb); }
            hash
        }
        // XUFIXNUM: the fixnum's value bits — the low 62 bits of the
        // two's-complement representation.  Beyond the fixnum range
        // these are GNU bignums, hashed per sxhash_bignum.
        Value::Int(n) => {
            if (SXHASH_FIXNUM_MIN..=SXHASH_FIXNUM_MAX).contains(n) {
                (*n as u64) & 0x3fff_ffff_ffff_ffff
            } else {
                sxhash_bignum(*n)
            }
        }
        // nil is Qnil — builtin symbol index 0 → XHASH 0.
        Value::Nil => 0,
        Value::Sym(id) => sxhash_sym_hash(i, *id),
        Value::Str(s) => sxhash_char_array(&gnu_sbytes(i, s)),
        // GNU sxhash_float folds the double's single EMACS_UINT word.
        Value::Float(f) => sxhash_combine(0, (**f).to_bits()),
        Value::Cons(_) => sxhash_list(i, v, depth),
        Value::Vec(vec) => {
            // Overlay handles `[overlay BID IDX]' hash like GNU's
            // PVEC_OVERLAY (start, end, plist), not elementwise.
            if let Some(h) = sxhash_overlay(i, &vec.borrow(), depth) {
                return h;
            }
            let vec = vec.borrow();
            let mut hash = vec.len() as u64;
            for item in vec.iter().take(SXHASH_MAX_LEN) {
                hash = sxhash_combine(hash, sxhash_obj(i, item, depth + 1));
            }
            hash
        }
        // GNU `sxhash_obj' dispatches pseudovectors: bool-vectors hash
        // their bit words, sub-char-tables hash 42, char-tables and
        // records hash via sxhash_vector seeded with ASIZE.
        Value::Record(r) => {
            if misc::is_bool_vector(i, v) {
                return sxhash_bool_vector(i, v);
            }
            if let Some(tag) = i.q_soft("sub-char-table") {
                if misc::is_sub_ct_tag(tag, v) {
                    return 42;
                }
            }
            let rr = r.borrow();
            if misc::is_char_table(i, v) {
                return sxhash_char_table(i, v, rr, depth);
            }
            let mut hash = PSEUDOVECTOR_FLAG | (PVEC_RECORD << 24) | rr.len() as u64;
            for item in rr.iter().take(SXHASH_MAX_LEN) {
                hash = sxhash_combine(hash, sxhash_obj(i, item, depth + 1));
            }
            hash
        }
        Value::Hash(h) => sxhash_xhash_ptr(h),
        Value::Lambda(l) => {
            // GNU hashes byte-code objects (`#[...]') by contents like
            // vectors, keeping the `equal ⇒ same-hash' invariant that
            // `equal'-test hash tables and constant dedup rely on.
            if let Some(items) = &l.bc_items {
                let items = items.borrow();
                let mut hash = items.len() as u64;
                for item in items.iter().take(SXHASH_MAX_LEN) {
                    hash = sxhash_combine(hash, sxhash_obj(i, item, depth + 1));
                }
                hash
            } else {
                // Interpreted lambdas are `equal' by structure: hash the
                // same fields `equal_values' compares.
                let mut hash = l.is_macro as u64;
                for s in l
                    .required
                    .iter()
                    .chain(l.optional.iter().map(|p| &p.sym))
                    .chain(l.rest.iter())
                {
                    hash = sxhash_combine(hash, sxhash_sym_hash(i, *s));
                }
                for f in l.body.iter().take(SXHASH_MAX_LEN) {
                    hash = sxhash_combine(hash, sxhash_obj(i, f, depth + 1));
                }
                hash
            }
        }
        Value::Buffer(b) => sxhash_xhash_ptr(b),
        Value::Marker(m) => sxhash_marker(i, &m.borrow()),
        Value::Window(w) => sxhash_xhash_ptr(w),
        Value::Frame(f) => sxhash_xhash_ptr(f),
        Value::Process(p) => sxhash_xhash_ptr(p),
        Value::Thread(t) => sxhash_xhash_ptr(t),
        Value::Mutex(m) => sxhash_xhash_ptr(m),
        Value::CondVar(c) => sxhash_xhash_ptr(c),
        Value::Finalizer(f) => sxhash_xhash_ptr(f),
        Value::Subr(s) => (*s as *const crate::lisp::value::Subr as usize as u64) >> 2,
    }
}

/// GNU `sxhash_eq' = XHASH (key) ^ XTYPE (key). The XTYPE tags with
/// LSB tagging are: symbol 0, cons 3, string 4, vectorlike 5, int
/// 2|6 (2 when the value is even), float 7.
fn sxhash_eq(i: &Interp, v: &Value) -> u64 {
    // `symbols-with-pos-enabled' objects ARE the symbol in GNU — hash
    // the bare symbol, ignoring the position.
    if let Some((id, _)) = misc::as_sympos(v) {
        return sxhash_sym_hash(i, id);
    }
    match v {
        // XHASH ^ XTYPE: ints tag 2 (even) or 6 (odd).
        Value::Int(n) => ((*n as u64) & 0x3fff_ffff_ffff_ffff) ^ (2 + 4 * (*n as u64 & 1)),
        // nil is Qnil in GNU: XHASH 0 ^ XTYPE 0.
        Value::Nil => 0,
        Value::Sym(id) => sxhash_sym_hash(i, *id),
        Value::Str(s) => sxhash_xhash_ptr(s) ^ 4,
        Value::Cons(c) => sxhash_xhash_ptr(c) ^ 3,
        Value::Float(f) => sxhash_xhash_ptr(f) ^ 7,
        _ => {
            // Everything else is vectorlike in GNU.
            let _ = i;
            sxhash_obj_ptr5(v)
        }
    }
}

fn sxhash_obj_ptr5(v: &Value) -> u64 {
    let p = match v {
        Value::BigInt(n) => Rc::as_ptr(n) as usize,
        Value::Vec(x) | Value::Record(x) => Rc::as_ptr(x) as usize,
        Value::Hash(x) => Rc::as_ptr(x) as usize,
        Value::Lambda(x) => Rc::as_ptr(x) as usize,
        Value::Buffer(x) => Rc::as_ptr(x) as usize,
        Value::Marker(x) => Rc::as_ptr(x) as usize,
        Value::Window(x) => Rc::as_ptr(x) as usize,
        Value::Frame(x) => Rc::as_ptr(x) as usize,
        Value::Process(x) => Rc::as_ptr(x) as usize,
        Value::Thread(x) => Rc::as_ptr(x) as usize,
        Value::Mutex(x) => Rc::as_ptr(x) as usize,
        Value::CondVar(x) => Rc::as_ptr(x) as usize,
        Value::Finalizer(x) => Rc::as_ptr(x) as usize,
        Value::Subr(s) => *s as *const crate::lisp::value::Subr as usize,
        _ => 0,
    };
    (p as u64 >> 2) ^ 5
}

/// Built-in equal tables use GNU's bounded sxhash as a bucket selector,
/// not as equality: cyclic and truncated structures may collide.
pub(crate) fn equal_hash_code(i: &Interp, value: &Value) -> u32 {
    let hash = sxhash_obj(i, value, 0);
    (hash ^ (hash >> 32)) as u32
}

/// GNU hashfn_user_defined: fixnums are already hash codes; all other
/// Lisp results are hashed with sxhash, then reduced to hash_hash_t.
pub(crate) fn user_hash_code(i: &Interp, value: &Value) -> u32 {
    let hash = match value {
        Value::Int(n) if (-(1i128 << 61)..(1i128 << 61)).contains(n) => {
            (*n as u64) & 0x3fff_ffff_ffff_ffff
        }
        _ => sxhash_obj(i, value, 0),
    };
    (hash ^ (hash >> 32)) as u32
}

fn f_sxhash(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    Ok(Value::Int(sxhash_reduce(sxhash_obj(i, &args[0], 0))))
}

fn f_sxhash_eq(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    Ok(Value::Int(sxhash_reduce(sxhash_eq(i, &args[0]))))
}

fn f_sxhash_eql(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    let h = match &args[0] {
        // FLOATP or BIGNUMP → sxhash, else sxhash_eq.
        Value::Float(_) | Value::BigInt(_) => sxhash_obj(i, &args[0], 0),
        Value::Int(n) if !(SXHASH_FIXNUM_MIN..=SXHASH_FIXNUM_MAX).contains(n) => {
            sxhash_obj(i, &args[0], 0)
        }
        _ => sxhash_eq(i, &args[0]),
    };
    Ok(Value::Int(sxhash_reduce(h)))
}

fn f_sxhash_including_props(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    let mut hash = sxhash_obj(i, &args[0], 0);
    if let Value::Str(s) = &args[0] {
        // GNU folds each interval's (position, length, plist) in.
        for (start, end, plist) in i.str_props(s) {
            hash = sxhash_combine(hash, *start as u64);
            hash = sxhash_combine(hash, (end - start) as u64);
            let pl = Value::list(plist.clone());
            hash = sxhash_combine(hash, sxhash_obj(i, &pl, 0));
        }
    }
    Ok(Value::Int(sxhash_reduce(hash)))
}
fn f_clear_string(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    match &args[0] {
        Value::Str(s) => {
            // GNU clears the logical byte allocation; nonempty strings
            // become unibyte, while the empty string keeps its byte kind.
            let n = string_storage_bytes(i, s);
            i.clear_str_props(s);
            if n != 0 {
                *s.borrow_mut() = "\0".repeat(n);
                i.note_string_data_change();
                i.mark_unibyte(s);
            }
            Ok(Value::Nil)
        }
        other => Err(i.wrong_type_mut("stringp", other)),
    }
}

fn f_store_substring(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    let idx = want_int(i, &args[1])?;
    match &args[0] {
        Value::Str(s) => {
            let mut chars: Vec<char> = s.borrow().chars().collect();
            let rep: Vec<char> = match &args[2] {
                Value::Str(rs) => rs.borrow().chars().collect(),
                Value::Int(n) => vec![char::from_u32(*n as u32).unwrap_or('\0')],
                other => return Err(i.wrong_type_mut("char-or-string-p", other)),
            };
            // GNU stores character by character and signals
            // args-out-of-range at the first invalid index (a partially
            // written string is observable in the error object).
            let len = chars.len() as i128;
            for (k, c) in rep.iter().enumerate() {
                let at = idx + k as i128;
                if at < 0 || at >= len {
                    return Err(i.signal_data(
                        sym::ARGS_OUT_OF_RANGE,
                        vec![args[0].clone(), Value::Int(at)],
                    ));
                }
                chars[at as usize] = *c;
                *s.borrow_mut() = chars.iter().collect();
                i.note_string_data_change();
            }
            Ok(args[0].clone())
        }
        other => Err(i.wrong_type_mut("stringp", other)),
    }
}
fn f_string_aref(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    let chars: Vec<char> = want_string(i, &args[0])?.chars().collect();
    let idx = want_int(i, &args[1])?;
    if idx < 0 || idx as usize >= chars.len() {
        return Err(i.signal_data(
            sym::ARGS_OUT_OF_RANGE,
            vec![args[0].clone(), Value::Int(idx)],
        ));
    }
    Ok(Value::Int(lisp_char_code(chars[idx as usize])))
}

// ---------- format ----------

/// `format` — printf-style with elisp conventions:
/// `%s` princ, `%S` prin1, `%d`/`%o`/`%x`/`%X`/`%e`/`%f`/`%g` numeric,
/// `%c` char, `%%` literal. Supports `%Nd`, `%-Ns`, `%0Nd`, `%.Nf`.
fn f_format(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    let fmt = want_string(i, &args[0])?;
    if !fmt.contains('%') { return Ok(args[0].clone()); }
    let fmt_ivs = match &args[0] {
        Value::Str(s) => i.str_props(s).to_vec(),
        _ => Vec::new(),
    };
    let mut spans = Vec::new();
    let v = format_impl_spans(i, &fmt, &args, &fmt_ivs, &mut spans)?;
    if let Value::Str(s) = &v {
        if !spans.is_empty() {
            i.set_str_props(s, spans);
        }
    }
    Ok(v)
}

pub(crate) fn f_format_message(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    let fmt = want_string(i, &args[0])?;
    let fmt_ivs = match &args[0] {
        Value::Str(s) => i.str_props(s).to_vec(),
        _ => Vec::new(),
    };
    let fmt = super::evalfn::translate_message_quotes(i, &fmt);
    let mut spans = Vec::new();
    let v = format_impl_spans(i, &fmt, &args, &fmt_ivs, &mut spans)?;
    if let Value::Str(s) = &v {
        if !spans.is_empty() {
            i.set_str_props(s, spans);
        }
    }
    Ok(v)
}

fn format_impl_spans(
    i: &mut Interp,
    fmt: &str,
    args: &[Value],
    fmt_ivs: &[(usize, usize, Vec<Value>)],
    spans: &mut Vec<(usize, usize, Vec<Value>)>,
) -> EvalResult {
    // styled_format returns the converted first argument directly for an
    // unpropertized two-character string conversion. Other arguments and
    // the format's storage kind then have no effect on that returned object.
    if !fmt.contains('%') && matches!(&args[0], Value::Str(source) if source.borrow().as_str() == fmt) {
        return Ok(args[0].clone());
    }
    if fmt_ivs.is_empty() && fmt.len() == 2 && fmt.starts_with('%') {
        let conversion = fmt.as_bytes()[1];
        if matches!(conversion, b's' | b'S' | b'c') {
            let Some(argument) = args.get(1) else {
                return Err(i.error("Not enough arguments for format string"));
            };
            if conversion == b's' {
                if matches!(argument, Value::Str(_)) { return Ok(argument.clone()); }
                if i.sym_id(argument).is_some() {
                    return super::data::f_symbol_name(i, vec![argument.clone()].into());
                }
            }
            if conversion == b'c' {
                if matches!(argument, Value::Int(code) if *code >= 128) {
                    return f_char_to_string(i, vec![argument.clone()].into());
                }
            } else {
                let text = if conversion == b'S' { i.print_to_string(argument) }
                           else { i.princ_to_string(argument) };
                let multibyte = content_is_multibyte(&text);
                let value = Value::string(text);
                if let Value::Str(string) = &value {
                    if multibyte { i.mark_multibyte(string); } else { i.mark_unibyte(string); }
                }
                return Ok(value);
            }
        }
    }
    let mut out = String::new();
    let mut multibyte = args.iter().any(|value| string_multibyte_p(i, value));
    let fchars: Vec<char> = fmt.chars().collect();
    // GNU doprnt copies the format string's text properties onto the
    // result via `text_property_list' + `add_text_properties_from_list'
    // AFTER formatting: whole input intervals are remapped onto the
    // emitted text (spec chars are "discarded", their argument's text
    // taking their place), never per-character.  `tr[k]' is the output
    // char index just before format char k; track it only when the
    // format string actually carries intervals.
    let track = !fmt_ivs.is_empty();
    let mut tr: Vec<usize> = Vec::new();
    let mut oc = 0usize; // chars emitted so far
    let emit_lit = |fchars: &[char],
                    k: usize,
                    out: &mut String,
                    track: bool,
                    tr: &mut Vec<usize>,
                    oc: &mut usize| {
        out.push(fchars[k]);
        if track {
            tr.push(*oc);
        }
        *oc += 1;
    };
    // Emit a literal format-string char: record its input→output
    // position mapping (props are remapped wholesale afterwards).
    macro_rules! emit_lit {
        ($k:expr) => {{
            emit_lit(&fchars, $k, &mut out, track, &mut tr, &mut oc);
        }};
    }
    let mut ai = 1usize; // next arg index
    let mut p = 0usize;
    while p < fchars.len() {
        let c = fchars[p];
        if c != '%' {
            emit_lit!(p);
            p += 1;
            continue;
        }
        // The spec starts at this '%' — its format-string props copy
        // onto the emitted (padded) text, like GNU.
        let spec_begin = p;
        p += 1;
        // GNU: (error "Format string ends in middle of format specifier").
        if p >= fchars.len() {
            return Err(i.error("Format string ends in middle of format specifier"));
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
        let mut alt = false;
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
                    alt = true;
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
            // GNU: (error "Format string ends in middle of format specifier").
            None => {
                return Err(i.error("Format string ends in middle of format specifier"));
            }
        };
        p += 1;
        if letter == '%' {
            out.push('%');
            // "%%" consumes two format chars for one emitted char:
            // tr[spec_begin] = oc (before it), tr[spec_begin+1] = oc+1
            // (after it) — matching GNU's discarded-char mapping.
            if track {
                tr.push(oc);
                tr.push(oc + 1);
            }
            oc += 1;
            continue;
        }
        let aidx = match pos_arg {
            Some(n) => n,
            None => ai,
        };
        if pos_arg.is_none() {
            ai += 1;
        }
        // GNU signals (error "Not enough arguments for format string")
        // when a spec has no corresponding argument.
        let Some(mut a) = args.get(aidx).cloned() else {
            return Err(i.error("Not enough arguments for format string"));
        };
        if letter == 's' && i.sym_id(&a).is_some() {
            a = super::data::f_symbol_name(i, vec![a].into())?;
        }
        if letter == 's' && string_multibyte_p(i, &a) { multibyte = true; }
        let piece = match letter {
            's' => {
                // GNU copies string arguments verbatim — the `%s' path
                // is NOT `princ' (which escapes eight-bit bytes).
                let s = match &a {
                    Value::Str(raw) => raw.borrow().clone(),
                    _ => i.princ_to_string(&a),
                };
                truncate_format_string(i, s, prec)
            }
            'S' => {
                let s = i.print_to_string(&a);
                truncate_format_string(i, s, prec)
            }
            'd' | 'i' => {
                let (mut s, add_sign) = match &a {
                    Value::Float(f) => {
                        // GNU formats truncated doubles with %.0f, avoiding
                        // saturation at the host integer boundary.  Integer
                        // conversions deliberately discard negative zero.
                        let f = f.trunc();
                        let f = if f == 0.0 { 0.0 } else { f };
                        let s = format_c_float(i, f, 'f', Some(0), plus, space_sign, false)?;
                        let signed = !s.as_bytes()[0].is_ascii_digit();
                        let digits = s.len() - usize::from(signed);
                        let zeros = prec.unwrap_or(0).saturating_sub(digits);
                        let sign_len = usize::from(s.starts_with(['-', '+', ' ']));
                        let mut s = s;
                        s.insert_str(sign_len, &"0".repeat(zeros));
                        (s, false) // sign flags already applied by snprintf
                    }
                    Value::BigInt(n) => {
                        let digits = n.magnitude().to_str_radix(10);
                        let zeros = prec.unwrap_or(0).saturating_sub(digits.len());
                        (format!("{}{}{}", if n.is_negative() { "-" } else { "" }, "0".repeat(zeros), digits), !n.is_negative())
                    }
                    Value::Int(n) => {
                        let n = *n;
                        // C printf integer precision: minimum digit count,
                        // excluding a sign; %.0d of integer zero is empty.
                        let s = if prec == Some(0) && n == 0 {
                            String::new()
                        } else {
                            let digits = n.unsigned_abs().to_string();
                            let zeros = prec.unwrap_or(0).saturating_sub(digits.len());
                            format!(
                                "{}{}{}",
                                if n < 0 { "-" } else { "" },
                                "0".repeat(zeros),
                                digits
                            )
                        };
                        (s, n >= 0)
                    }
                    _ => return Err(fmt_type_err(i)),
                };
                if add_sign {
                    if plus {
                        s = format!("+{}", s);
                    } else if space_sign {
                        s = format!(" {}", s);
                    }
                }
                s
            }
            'c' => {
                // GNU CHECK_CHARACTER: signals `characterp' outside
                // [0, #x3fffff] (e.g. -1), and on non-integers.
                // 0x110000..=0x3FFF7F can't be a Rust char — they
                // surface as one mule-seq proxy char like GNU's code.
                let n = match &a {
                    Value::Int(n) => *n,
                    _ => return Err(fmt_type_err(i)),
                };
                let ch = if (0x11_0000..=0x3F_FF7F).contains(&n) {
                    crate::lisp::value::mule_seq_char(n as u32)
                } else {
                    match crate::lisp::value::lisp_char(n as u32) {
                        Some(ch) if (0..=0x3f_ffff).contains(&n) => ch,
                        _ => {
                            let pid = i.q_soft("characterp").unwrap_or(0);
                            return Err(i.signal_data(
                                sym::WRONG_TYPE_ARGUMENT,
                                vec![Value::Sym(pid), a.clone()],
                            ));
                        }
                    }
                };
                if n >= 128 { multibyte = true; }
                ch.to_string()
            }
            'o' | 'x' | 'X' | 'b' => {
                let n = int_of(i, &a)?;
                // GNU: the sign precedes any alt-form prefix, and the
                // precision pads the digit run.
                let digits = match letter {
                    'o' => format!("{:o}", n.magnitude()),
                    'x' => format!("{:x}", n.magnitude()),
                    'X' => format!("{:X}", n.magnitude()),
                    _ => format!("{:b}", n.magnitude()),
                };
                let digits = prec_pad_int(digits, prec);
                let mut s = String::new();
                if n.is_negative() {
                    s.push('-');
                }
                match letter {
                    'o' => {
                        if alt && !digits.starts_with('0') {
                            s.push('0');
                        }
                    }
                    _ => {
                        if alt && !n.is_zero() {
                            s.push_str(match letter {
                                'x' => "0x",
                                'X' => "0X",
                                _ => "0b",
                            });
                        }
                    }
                }
                s.push_str(&digits);
                s
            }
            'e' | 'f' | 'g' => {
                format_c_float(i, float_of(i, &a)?, letter, prec, plus, space_sign, alt)?
            }
            _ => {
                // Unsupported spec — Emacs signals "Invalid format operation".
                return Err(i.error(&format!("Invalid format operation %{}", letter)));
            }
        };
        // apply width/alignment — GNU's doprnt pads by DISPLAY width
        // (strwidth), so CJK chars count 2 columns, not 1.
        let plen: usize = piece.chars().map(|ch| format_char_width(i, ch)).sum();
        let piece_chars = piece.chars().count();
        let padded = match width {
            Some(w) if plen < w => {
                let pad = w - plen;
                if left {
                    format!("{}{}", piece, " ".repeat(pad))
                } else if pad0
                    && matches!(letter, 'd' | 'i' | 'o' | 'x' | 'X' | 'b' | 'e' | 'f' | 'g')
                    && !(prec.is_some() && matches!(letter, 'd' | 'i' | 'o' | 'x' | 'X' | 'b'))
                    && !matches!(&a, Value::Float(f) if !f.is_finite())
                {
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
        // GNU copies the argument string's text properties onto the
        // inserted text; right-justified padding stays unpropertized
        // while left-justified padding extends the trailing interval.
        if matches!(letter, 's') {
            if let Value::Str(src) = &a {
                if i.has_str_props(src) {
                    let pad = padded.chars().count() - piece_chars;
                    let base = out.chars().count() + if left { 0 } else { pad };
                    let mut last_end = None;
                    for (s0, e0, pl) in i.str_props(src) {
                        let (a0, b0) = (*s0, (*e0).min(piece_chars));
                        if a0 < b0 {
                            spans.push((
                                base + a0,
                                base + b0,
                                crate::buffer::primitives::plist_pairs_rev(pl),
                            ));
                            last_end = Some(base + b0);
                        }
                    }
                    if left && pad > 0 && last_end == Some(base + piece_chars) {
                        if let Some(iv) = spans.last_mut() {
                            iv.1 += pad;
                        }
                    }
                }
            }
        }
        // Format-string props are remapped after the loop; here just
        // record tr[] entries — every spec char after the first maps to
        // the end of the emitted text (GNU's discarded-char mapping).
        out.push_str(&padded);
        if track {
            for k in spec_begin..p {
                tr.push(if k == spec_begin {
                    oc
                } else {
                    oc + padded.chars().count()
                });
            }
        }
        oc += padded.chars().count();
    }
    // GNU: `add_text_properties_from_list' plputs each interval's pairs
    // in order, so copied plists come out reversed (like the `%s'
    // argument copy's plist_pairs_rev), and whole intervals map at once
    // — adjacent equal-plist chars stay one interval.
    if track {
        for (s0, e0, pl) in fmt_ivs {
            // e0 may reach the end of the format string — tr has one
            // entry per format char, so use the final out length.
            let (a0, b0) = (tr[*s0], if *e0 >= fchars.len() { oc } else { tr[*e0] });
            if a0 < b0 && !pl.is_empty() {
                spans.push((a0, b0, crate::buffer::primitives::plist_pairs_rev(pl)));
            }
        }
    }
    multibyte |= content_is_multibyte(&out);
    let result = Value::string(out);
    if let Value::Str(string) = &result {
        if multibyte { i.mark_multibyte(string); } else { i.mark_unibyte(string); }
    }
    Ok(result)
}

/// GNU styled_format uses snprintf for floating-point conversions,
/// preserving sign/sharp flags and C rounding even around %g boundaries.
/// Width is applied by the caller after conversion, as in GNU.
fn format_c_float(
    i: &Interp,
    value: f64,
    letter: char,
    precision: Option<usize>,
    plus: bool,
    space: bool,
    sharp: bool,
) -> Result<String, Flow> {
    let precision = precision
        .map(libc::c_int::try_from)
        .transpose()
        .map_err(|_| i.error("String overflow"))?
        .unwrap_or(-1);
    let spec = format!(
        "%{}{}{}.*{}\0",
        if plus { "+" } else { "" },
        if space { " " } else { "" },
        if sharp { "#" } else { "" },
        letter,
    );
    let mut output = vec![0u8; 128];
    loop {
        // The format is constructed exclusively from supported conversion
        // letters and flags; the variadic arguments are int and double.
        let len = unsafe {
            libc::snprintf(
                output.as_mut_ptr().cast(),
                output.len(),
                spec.as_ptr().cast(),
                precision,
                value,
            )
        };
        if len < 0 {
            return Err(i.error("String overflow"));
        }
        let len = len as usize;
        if len < output.len() {
            return Ok(String::from_utf8_lossy(&output[..len]).into_owned());
        }
        output
            .try_reserve_exact(len + 1 - output.len())
            .map_err(|_| i.error("String overflow"))?;
        output.resize(len + 1, 0);
    }
}

/// Precision limits %s/%S by display columns, including zero-width
/// characters at the boundary.  GNU treats precision zero specially.
fn truncate_format_string(i: &Interp, mut s: String, precision: Option<usize>) -> String {
    let Some(limit) = precision else { return s };
    if limit == 0 {
        s.clear();
        return s;
    }
    let mut width = 0usize;
    let end = s.char_indices().find_map(|(pos, ch)| {
        let w = format_char_width(i, ch);
        if w > limit - width {
            Some(pos)
        } else {
            width += w;
            None
        }
    });
    if let Some(end) = end {
        s.truncate(end);
    }
    s
}

fn format_char_width(i: &Interp, ch: char) -> usize {
    // GNU CHARACTER_WIDTH handles printable ASCII directly, without
    // consulting char-width-table, even in a multibyte string.
    if (' '..='~').contains(&ch) {
        return 1;
    }
    if ch == '\n' {
        return 0;
    }
    if ch == '\t' {
        return display_tab_width(i);
    }
    if let Some(id) = i.q_soft("char-width-table") {
        if let Value::Int(width) = misc::char_table_ref(
            i,
            &i.symbol_value(id),
            lisp_char_code(ch) as usize,
        ) {
            if width >= 0 {
                return width as usize;
            }
        }
    }
    gnu_char_width(i, ch)
}

/// C printf precision on a pre-formatted integer string: pad with
/// leading zeros to PREC digits; precision 0 of value "0" is "".
fn prec_pad_int(s: String, prec: Option<usize>) -> String {
    match prec {
        Some(0) if s == "0" => String::new(),
        Some(pr) if s.len() < pr => format!("{}{}", "0".repeat(pr - s.len()), s),
        _ => s,
    }
}

fn fmt_type_err(i: &Interp) -> super::Flow {
    // GNU: (error "Format specifier doesn’t match argument type").
    i.error_q("Format specifier doesn't match argument type", &[])
}

fn int_of(i: &Interp, v: &Value) -> Result<num_bigint::BigInt, super::Flow> {
    match v {
        Value::Int(n) => Ok((*n).into()),
        Value::BigInt(n) => Ok((**n).clone()),
        Value::Float(f) if f.is_finite() => Ok(num_bigint::BigInt::from_f64(f.trunc()).unwrap()),
        Value::Float(_) => Err(i.signal_data(
            i.q_soft("overflow-error").unwrap_or(sym::ERROR),
            vec![],
        )),
        _ => Err(fmt_type_err(i)),
    }
}

fn float_of(i: &Interp, v: &Value) -> Result<f64, super::Flow> {
    match v {
        Value::Int(n) => Ok(*n as f64),
        Value::BigInt(n) => Ok(n.to_f64().unwrap_or(if n.is_negative() {
            f64::NEG_INFINITY
        } else {
            f64::INFINITY
        })),
        Value::Float(f) => Ok(**f),
        _ => Err(fmt_type_err(i)),
    }
}

// ---------- added builtins ----------

fn f_make_string(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    // GNU checks LENGTH and INIT before allocating the result.
    let n = match &args[0] {
        Value::Int(n) if args[0].fixnump() && *n >= 0 => *n as usize,
        other => return Err(i.wrong_type_mut("wholenump", other)),
    };
    let code = match &args[1] {
        Value::Int(code) if (0..=0x3f_ffff).contains(code) => *code as u32,
        other => return Err(i.wrong_type_mut("characterp", other)),
    };
    if n > 0x2000_0000 {
        return Err(i.error("Memory exhausted--use C-x s then exit and restart Emacs"));
    }
    let ch = lisp_char(code).unwrap_or_else(|| crate::lisp::value::mule_seq_char(code));
    // Only ASCII with a nil MULTIBYTE argument uses unibyte storage.
    let multibyte = code >= 128 || args.get(2).is_some_and(Value::truthy);
    let mut text = String::with_capacity(n * ch.len_utf8());
    for _ in 0..n {
        text.push(ch);
    }
    let value = Value::string(text);
    if let Value::Str(text) = &value {
        // Even empty and ASCII strings retain an explicit MULTIBYTE kind.
        if multibyte {
            i.mark_multibyte(text);
        } else {
            i.mark_unibyte(text);
        }
    }
    Ok(value)
}

fn f_string_to_list(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    // GNU subr.el: (append STRING nil), including non-string inputs.
    super::listfn::f_append(i, vec![args[0].clone(), Value::Nil].into())
}
fn f_string_to_vector(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    // GNU subr.el: (vconcat STRING), including non-string inputs.
    f_vconcat(i, args)
}

fn string_storage_bytes(i: &Interp, string: &crate::lisp::value::StrRef) -> usize {
    // Unibyte strings hold chars as raw bytes (eight-bit PUA proxies)
    // — GNU's byte count is the char count; multibyte strings count
    // per-char Emacs storage: UTF-8 through 0x1fffff, five bytes for
    // extended Mule codes, and two bytes for eight-bit characters.
    // Count original Lisp codes, not our surrogate/Mule proxy chars.
    if i.is_unibyte_str(string) {
        string.borrow().chars().count()
    } else {
        string
            .borrow()
            .chars()
            .map(|c| match lisp_char_code(c) {
                0..=0x7f => 1,
                0x80..=0x7ff | 0x3fff80..=0x3fffff => 2,
                0x800..=0xffff => 3,
                0x10000..=0x1fffff => 4,
                _ => 5,
            })
            .sum()
    }
}

fn f_string_bytes(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    match &args[0] {
        Value::Str(string) => Ok(Value::Int(string_storage_bytes(i, string) as i128)),
        other => Err(i.wrong_type_mut("stringp", other)),
    }
}

fn f_subst_char_in_string(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    // GNU subr.el semantics:
    //  1. `(if (multibyte-string-p STRING) (> (max F T) 127) (> T 255))'
    //     — `(multibyte-string-p X)' is nil for non-strings, so the
    //     number-or-marker checks on FROMCHAR/TOCHAR run BEFORE any
    //     sequencep/stringp validation of STRING.
    //  2. Resize path: GNU calls `string-replace', whose `string-search'
    //     FIRST CHECKs `(string FROMCHAR)' → characterp, THEN STRING →
    //     stringp.  The replacement itself is char-equality, identical
    //     to the slow path's result here.
    //  3. Slow path (INPLACE or no resize): `(length STRING)' →
    //     sequencep, `copy-sequence' → arrayp on conses; `aset' →
    //     characterp on TOCHAR but only when an element matched.
    let inplace = arg(&args, 3).truthy();
    let s_v = &args[2];
    enum SubstSeq {
        Str(Vec<char>),
        Vec(Vec<Value>),
    }
    let multibyte = match s_v {
        Value::Str(s) => {
            if i.is_unibyte_str(s) {
                false
            } else if i.is_multibyte_str(s) {
                true
            } else {
                content_is_multibyte(&s.borrow())
            }
        }
        _ => false,
    };
    if !inplace {
        let resize = if multibyte {
            // (max from to) needs numbers-or-markers — both checked.
            let f = crate::editor::want_num_or_mark(i, &args[0])?;
            let t = crate::editor::want_num_or_mark(i, &args[1])?;
            f.max(t) > 127
        } else {
            crate::editor::want_num_or_mark(i, &args[1])? > 255
        };
        if resize {
            // `string-replace' → `string-search' validates
            // `(string FROMCHAR)' → characterp before STRING → stringp.
            match &args[0] {
                Value::Int(n) if (0..=0x3f_ffff).contains(n) => {}
                other => return Err(i.wrong_type_mut("characterp", other)),
            }
            if !matches!(s_v, Value::Str(_)) {
                return Err(i.wrong_type_mut("stringp", s_v));
            }
        }
    }
    let seq = match s_v {
        Value::Str(s) => SubstSeq::Str(s.borrow().chars().collect::<Vec<char>>()),
        // GNU `copy-sequence' accepts vectors; `aref'/`aset' work on
        // ints — element equality follows `eq' (fixnum only).
        Value::Vec(v) => SubstSeq::Vec(v.borrow().clone()),
        Value::Nil => SubstSeq::Vec(Vec::new()),
        // `copy-sequence' signals arrayp on conses (lists are sequences
        // but not arrays); bare atoms die earlier at `length' → sequencep.
        Value::Cons(_) => return Err(i.wrong_type_mut("arrayp", s_v)),
        other => return Err(i.wrong_type_mut("sequencep", other)),
    };
    // For multibyte fast path GNU uses `string-replace' + property copy;
    // remacs strings keep no per-char text props here, so the replace
    // result is identical either way.
    let from_c = match &args[0] {
        Value::Int(n) => char::from_u32(*n as u32),
        // Non-number FROMCHAR in the resize path was already checked;
        // on the slow path it's just a never-matching key — `aref'
        // comparisons are `eq', so non-chars simply never match.
        _ => None,
    };
    match seq {
        SubstSeq::Vec(_) if s_v.is_nil() => Ok(Value::Nil),
        SubstSeq::Vec(elems) => {
            // `eq' element compare: fixnum elements match fixnum chars.
            let hit = |e: &Value| match (e, &args[0]) {
                (Value::Int(a), Value::Int(b)) => a == b,
                _ => false,
            };
            let out: Vec<Value> = elems
                .iter()
                .map(|e| if hit(e) { args[1].clone() } else { e.clone() })
                .collect();
            if inplace {
                if let Value::Vec(v) = s_v {
                    *v.borrow_mut() = out;
                    return Ok(args[2].clone());
                }
                unreachable!()
            }
            Ok(Value::vector(out))
        }
        SubstSeq::Str(chars) => {
            let from = from_c.unwrap_or('\u{10FFFF}');
            // GNU `aset' CHECKs TOCHAR (characterp) only when an element
            // actually matched FROMCHAR.
            if chars.iter().any(|&c| c == from) {
                match &args[1] {
                    Value::Int(n) if (0..=0x3f_ffff).contains(n) => {}
                    other => return Err(i.wrong_type_mut("characterp", other)),
                }
            }
            let to_c = match &args[1] {
                Value::Int(n) => char::from_u32(*n as u32).unwrap_or('\u{FFFD}'),
                _ => '\u{FFFD}',
            };
            let replaced: String = chars
                .iter()
                .map(|&c| if c == from { to_c } else { c })
                .collect();
            if inplace {
                if let Value::Str(s) = s_v {
                    *s.borrow_mut() = replaced;
                    i.note_string_data_change();
                    return Ok(args[2].clone());
                }
                unreachable!()
            }
            Ok(Value::string(replaced))
        }
    }
}

fn f_compare_strings(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    // (compare-strings STR1 START1 END1 STR2 START2 END2 &optional IGNORE-CASE)
    // GNU Fcompare_strings: CHECK_STRING on both (symbols rejected),
    // then too-large positive END values are silently clamped before
    // validate_subarray runs.
    let s1 = want_string(i, &args[0])?;
    let s2 = want_string(i, &args[3])?;
    let v1: Vec<char> = s1.chars().collect();
    let v2: Vec<char> = s2.chars().collect();
    let e1v = clamp_end(&args[2], v1.len());
    let e2v = clamp_end(&args[5], v2.len());
    let (b1, e1) = validate_subarray(i, &args[0], &args[1], &e1v, v1.len())?;
    let (b2, e2) = validate_subarray(i, &args[3], &args[4], &e2v, v2.len())?;
    let fold = arg(&args, 6).truthy();
    let ac: Vec<char> = v1[b1..e1].to_vec();
    let bc: Vec<char> = v2[b2..e2].to_vec();
    // GNU upcases differing characters through the buffer's table,
    // without full string expansions (sharp-s must not become S).
    let multibyte = i
        .intern_soft("enable-multibyte-characters")
        .map(|id| i.symbol_value(id).truthy())
        .unwrap_or(true);
    let upcase = |i: &Interp, table: &Value, code: u32| {
        // Fupcase's numeric path treats 0..255 as bytes in a unibyte
        // buffer, even when compare-strings read them from multibyte text.
        let byte_input = !multibyte && code < 256;
        let source = if byte_input && code >= 128 {
            code + 0x3fff00
        } else {
            code
        };
        let cased = ct_case(i, table, source);
        if cased == source {
            code
        } else if byte_input && cased >= 128 {
            if cased >= 0x3fff80 {
                cased - 0x3fff00
            } else {
                cased & 0xff
            }
        } else {
            cased
        }
    };
    let mut up = None;
    for (k, (&a, &b)) in ac.iter().zip(bc.iter()).enumerate() {
        let (mut a, mut b) = (lisp_char_code(a) as u32, lisp_char_code(b) as u32);
        if a != b && fold {
            let table = up.get_or_insert_with(|| {
                let down = i.current_case_table();
                misc::set_case_table_fill(i, &down);
                case_extra(&down, 0).unwrap_or(down)
            });
            a = upcase(i, table, a);
            b = upcase(i, table, b);
        }
        if a != b {
            let pos = k as i128 + 1;
            return Ok(Value::Int(if a < b { -pos } else { pos }));
        }
    }
    if ac.len() == bc.len() {
        return Ok(Value::t());
    }
    let n = ac.len().min(bc.len()) as i128 + 1;
    Ok(Value::Int(if ac.len() < bc.len() { -n } else { n }))
}

/// GNU Fcompare_strings's "For backward compatibility, silently bring
/// too-large positive end values into range" clamp — applied to the
/// raw arg *before* validate_subarray sees it (a fixnum END > LEN is
/// replaced by LEN; non-fixnums and negatives pass through).
fn clamp_end(v: &Value, len: usize) -> Value {
    match v {
        Value::Int(n) if *n > len as i128 => Value::Int(len as i128),
        other => other.clone(),
    }
}

/// GNU `validate_subarray' (fns.c): nil → 0/SIZE, fixnum (negatives
/// count from the end), anything else `wrong-type-argument integerp';
/// then 0 <= from <= to <= SIZE or `args-out-of-range' with the array
/// and the ORIGINAL bounds (after the compare-strings END clamp).
pub(super) fn validate_subarray(
    i: &mut Interp,
    arr: &Value,
    from: &Value,
    to: &Value,
    size: usize,
) -> Result<(usize, usize), Flow> {
    let f = match from {
        Value::Int(n) => {
            if *n < 0 {
                n + size as i128
            } else {
                *n
            }
        }
        Value::Nil => 0,
        other => return Err(i.wrong_type_mut("integerp", other)),
    };
    let t = match to {
        Value::Int(n) => {
            if *n < 0 {
                n + size as i128
            } else {
                *n
            }
        }
        Value::Nil => size as i128,
        other => return Err(i.wrong_type_mut("integerp", other)),
    };
    if !(0 <= f && f <= t && t <= size as i128) {
        let sym = i.q("args-out-of-range");
        return Err(i.signal_data(sym, vec![arr.clone(), from.clone(), to.clone()]));
    }
    Ok((f as usize, t as usize))
}

fn f_char_width(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    // GNU Fchar_width: CHECK_CHARACTER — 0..=MAX_5_BYTE_CHAR.
    let c = match &args[0] {
        Value::Int(n) if (0..=0x3FFFFF).contains(n) => *n,
        other => return Err(i.wrong_type_mut("characterp", other)),
    };
    // GNU `Fchar_width' reads the buffer-local `tab-width' for TAB.
    if c == '\t' as i128 {
        return Ok(Value::Int(display_tab_width(i) as i128));
    }
    let w = gnu_char_code_width(i, c as u32);
    Ok(Value::Int(w as i128))
}
fn f_format_spec(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
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
            // GNU errors on a trailing lone `%'.
            None => return Err(i.error("Invalid format character: `%'")),
        };
        if spec == '%' {
            out.push('%');
            continue;
        }
        // Look up spec char in alist: key is a char (Int). GNU skips
        // non-cons elements (assq semantics) but a non-list tail is a
        // wrong-type-argument; a spec char with no entry is an error.
        let key = Value::Int(spec as i128);
        let mut val = Value::Nil;
        let mut found = false;
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
                            found = true;
                            break;
                        }
                    }
                    cur = next;
                }
                other => return Err(i.wrong_type_mut("listp", &other)),
            }
        }
        if !found {
            return Err(i.error(&format!("Invalid format character: `%{}'", spec)));
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

/// Unlike string-to-multibyte, ASCII input needs no representation change.
/// GNU also preserves its identity and text properties in this case.
fn f_string_make_multibyte(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    let string = want_string(i, &args[0])?;
    if string.is_ascii() {
        Ok(args[0].clone())
    } else {
        f_string_to_multibyte(i, args)
    }
}

/// `string-to-multibyte' — in a unibyte string, each byte-value char
/// ≥0x80 becomes an eight-bit char (0x3FFF80 + byte − 0x80); a
/// multibyte string is returned unchanged.
fn f_string_to_multibyte(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    let s = want_string(i, &args[0])?;
    let unibyte_in = match &args[0] {
        Value::Str(r) => {
            i.is_unibyte_str(r) || (!i.is_multibyte_str(r) && !content_is_multibyte(&r.borrow()))
        }
        _ => false,
    };
    if !unibyte_in {
        return Ok(args[0].clone());
    }
    let out: String = s
        .chars()
        .map(|c| {
            let u = c as u32;
            if unibyte_in && (0x80..=0xFF).contains(&u) {
                lisp_char(u + 0x3FFF00).unwrap()
            } else {
                c
            }
        })
        .collect();
    let v = Value::string(out);
    if let Value::Str(r) = &v {
        i.mark_multibyte(r);
    }
    Ok(v)
}

/// `string-to-unibyte' — a unibyte string is unchanged; in multibyte
/// input, chars <0x80 stay bytes and eight-bit chars return to their
/// byte value; anything else errors.
fn f_string_to_unibyte(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    let s = want_string(i, &args[0])?;
    let unibyte_in = matches!(&args[0], Value::Str(r) if i.is_unibyte_str(r));
    let mut out = String::new();
    for (ix, c) in s.chars().enumerate() {
        let u = c as u32;
        if u < 0x80 || (unibyte_in && u <= 0xFF) {
            out.push(c);
        } else if let Some(b) = eight_bit_byte(c) {
            out.push(b as char);
        } else {
            return Err(i.error(format!("Cannot convert character at index {ix} to unibyte")));
        }
    }
    let v = Value::string(out);
    if let Value::Str(r) = &v {
        i.mark_unibyte(r);
    }
    Ok(v)
}

/// `string-as-unibyte' — a unibyte string is returned unchanged; a
/// multibyte string's chars use GNU's internal encoding (including
/// surrogate and non-Unicode characters), with raw eight-bit chars unwrapped.
pub(crate) fn f_string_as_unibyte(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    let s = want_string(i, &args[0])?;
    let multibyte = match &args[0] {
        Value::Str(r) => {
            i.is_multibyte_str(r) || (!i.is_unibyte_str(r) && content_is_multibyte(&r.borrow()))
        }
        _ => false,
    };
    let out = if multibyte {
        let mut o = String::new();
        for c in s.chars() {
            if let Some(b) = eight_bit_byte(c) {
                o.push(b as char);
            } else {
                o.extend(
                    super::coding_legacy::internal_encode(lisp_char_code(c) as u32)
                        .into_iter()
                        .map(|byte| byte as char),
                );
            }
        }
        o
    } else {
        s
    };
    let v = Value::string(out);
    if let Value::Str(r) = &v {
        i.mark_unibyte(r);
    }
    Ok(v)
}

/// `string-as-multibyte' — decode the string's byte-chars as UTF-8;
/// bytes that can't decode become eight-bit chars.  An already
/// multibyte string is returned unchanged.
fn f_string_as_multibyte(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    let s = want_string(i, &args[0])?;
    let multibyte_in = match &args[0] {
        Value::Str(r) => {
            i.is_multibyte_str(r) || (!i.is_unibyte_str(r) && content_is_multibyte(&r.borrow()))
        }
        _ => false,
    };
    if multibyte_in {
        let v = Value::string(s);
        if let Value::Str(r) = &v {
            i.mark_multibyte(r);
        }
        return Ok(v);
    }
    let bytes: Vec<u8> = s.chars().map(|c| c as u8).collect();
    let mut out = String::new();
    let mut k = 0;
    while k < bytes.len() {
        match std::str::from_utf8(&bytes[k..]) {
            Ok(t) => {
                out.push_str(t);
                break;
            }
            Err(e) => {
                let good = e.valid_up_to();
                out.push_str(unsafe { std::str::from_utf8_unchecked(&bytes[k..k + good]) });
                k += good;
                let bad = e.error_len().unwrap_or(1);
                for _ in 0..bad.min(bytes.len() - k) {
                    out.push(lisp_char(bytes[k] as u32 + 0x3FFF00).unwrap());
                    k += 1;
                }
            }
        }
    }
    let v = Value::string(out);
    if let Value::Str(r) = &v {
        i.mark_multibyte(r);
    }
    Ok(v)
}

/// `string-make-unibyte' — truncate every char to its low-order 8
/// bits and mark the result unibyte (GNU `CHAR_TO_BYTE8': eight-bit
/// chars yield their byte; anything else keeps `c & 0xFF').
fn f_string_make_unibyte(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    let s = want_string(i, &args[0])?;
    let out: String = s
        .chars()
        .map(|c| {
            let b = crate::lisp::value::eight_bit_byte(c)
                .unwrap_or_else(|| (crate::lisp::value::lisp_char_code(c) & 0xFF) as u8);
            if b >= 0x80 {
                char::from_u32(crate::lisp::value::EIGHT_BIT_BASE + b as u32).unwrap()
            } else {
                b as char
            }
        })
        .collect();
    let v = Value::string(out);
    if let Value::Str(r) = &v {
        i.mark_unibyte(r);
    }
    Ok(v)
}

pub(crate) fn string_multibyte_p(i: &Interp, value: &Value) -> bool {
    match value {
        Value::Str(s) => {
            if i.is_unibyte_str(s) {
                false
            } else if i.is_multibyte_str(s) {
                true
            } else {
                // Unmarked strings follow GNU's storage rule.
                content_is_multibyte(&s.borrow())
            }
        }
        _ => false,
    }
}

fn f_multibyte_string_p(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    Ok(Value::from_bool(string_multibyte_p(i, &args[0])))
}

fn f_unibyte_string(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    let mut s = String::new();
    for a in args.iter() {
        let n = want_int(i, a)?;
        if !(0..=255).contains(&n) {
            let sym = i.q("args-out-of-range");
            return Err(i.signal_data(sym, vec![a.clone(), Value::Int(0), Value::Int(255)]));
        }
        let c = char::from_u32(n as u32).unwrap();
        s.push(c);
    }
    let v = Value::string(s);
    if let Value::Str(r) = &v {
        i.mark_unibyte(r);
    }
    Ok(v)
}

fn f_make_char(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    // (make-char CHARSET &optional CODE1 CODE2 CODE3 CODE4)
    // GNU Fmake_char: the codes are positions inside CHARSET's
    // code-space; the mapped char = DECODE_CHAR(charset, packed).
    let name = super::charset::want_charset(i, &args[0])?;
    match super::charset::make_char(i, &name, &args[1..]) {
        Ok(c) => Ok(Value::Int(c.into())),
        Err(f) => Err(f),
    }
}

fn f_split_char(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    // GNU `split_char': (CHARSET CODE1 .. CODEn) where CHARSET is the
    // `char-charset' winner and each CODE is a byte of CH's packed
    // code-point in that charset (dimension bytes, MSB first).
    let ch = match &args[0] {
        Value::Int(n) if (0..=0x3FFFFF).contains(n) => *n as i64,
        other => return Err(i.wrong_type_mut("characterp", other)),
    };
    let name = super::charset::char_charset(i, ch, &[]).unwrap_or_else(|| "emacs".to_string());
    let Some(code) = super::charset::charset_encode(i, &name, ch) else {
        return Ok(Value::Nil);
    };
    let dim = super::charset::charset_dimension_of(i, &name).max(1);
    let mut out = vec![Value::Sym(i.intern(&name))];
    for k in (0..dim).rev() {
        out.push(Value::Int(((code >> (8 * k)) & 0xFF).into()));
    }
    Ok(Value::list(out))
}

fn f_encode_char(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    // GNU Fencode_char: CHECK_CHARSET_GET_ID runs before CHECK_CHARACTER.
    let name = super::charset::want_charset(i, &args[1])?;
    let ch = match &args[0] {
        Value::Int(n) if (0..=0x3FFFFF).contains(n) => *n as i64,
        _ => return Err(i.wrong_type_mut("characterp", &args[0])),
    };
    match super::charset::charset_encode(i, &name, ch) {
        Some(c) => Ok(Value::Int(c.into())),
        None => Ok(Value::Nil),
    }
}

fn f_decode_char(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    // GNU Fdecode_char: CHECK_CHARSET_GET_ID then cons_to_unsigned —
    // an integer, an integral float in range, or a cons pair
    // (HIGH . LOW) packing a >16-bit code-point; anything else signals
    // "Not an in-range integer, integral float, or cons of integers".
    let name = super::charset::want_charset(i, &args[0])?;
    let code = match super::charset::cons_to_u64(&args[1]) {
        Some(c) => c,
        None => return Err(i.error("Not an in-range integer, integral float, or cons of integers")),
    };
    match super::charset::charset_decode(i, &name, code as i64) {
        Some(c) => Ok(Value::Int(c.into())),
        None => Ok(Value::Nil),
    }
}

/// Charsets we model, in GNU's `charset-priority-list` order.
pub(crate) const CHARSET_PRIORITY: &[&str] = crate::lisp::builtins::charset::GNU_CHARSET_PRIORITY;

/// Whether charset NAME contains Unicode scalar CH — driven by the
/// generated `encode-coding-char' tables where a coding system's
/// charset coverage coincides (sjis double-byte ⇔ jisx0208, euc-jp
/// SS2 ⇔ jisx0212, etc.).
pub(crate) fn charset_contains(name: &str, ch: i128) -> bool {
    use super::enc_tables::*;
    let u = ch as u32;
    let in_tab = |t: &[(u32, u64)]| t.binary_search_by_key(&u, |&(x, _)| x).is_ok();
    match name {
        "ascii" | "us-ascii" => u < 0x80,
        // JIS X 0201 Latin differs from ASCII only at ¥ and ‾.
        "latin-jisx0201" => u == 0xA5 || u == 0x203E,
        "katakana-jisx0201" => (0xFF61..=0xFF9F).contains(&u),
        // sjis encodes jisx0208 as double bytes; halfwidth katakana is
        // a separate charset (katakana-jisx0201).
        "japanese-jisx0208" | "japanese-jisx0208-1978" => {
            !(0xFF61..=0xFF9F).contains(&u) && in_tab(ENC_SHIFT_JIS)
        }
        // euc-jp encodes jisx0212 as 0x8F + two bytes.
        "japanese-jisx0212" => {
            in_tab(ENC_EUC_JP)
                && ENC_EUC_JP
                    .binary_search_by_key(&u, |&(x, _)| x)
                    .map(|ix| (ENC_EUC_JP[ix].1 >> 56) >= 3)
                    .unwrap_or(false)
        }
        "chinese-gb2312" => in_tab(ENC_GB2312),
        "big5" | "chinese-big5-1" | "chinese-big5-2" => in_tab(ENC_BIG5),
        "koi8" | "koi8-r" => in_tab(ENC_KOI8_R),
        "cyrillic-iso8859-5" => u == 0x401 || (0x410..=0x44F).contains(&u) || u == 0x451,
        "windows-1251" | "cp1251" => in_tab(ENC_WINDOWS_1251),
        "mac-roman" => in_tab(ENC_MAC_ROMAN),
        "latin-iso8859-1" | "iso-8859-1" => (0xA0..=0xFF).contains(&u),
        "unicode" => true,
        "unicode-bmp" => u <= 0xFFFF,
        "unicode-smp" => (0x10000..=0x1FFFF).contains(&u),
        "unicode-sip" => (0x20000..=0x2FFFF).contains(&u),
        "unicode-ssp" => (0x30000..=0x3FFFF).contains(&u),
        // ISO C1 controls; eight-bit-* charsets hold Emacs-internal
        // raw-byte characters above the Unicode space.
        "control-1" => (0x80..=0x9F).contains(&u),
        "eight-bit-control" => (0x3FFF80..=0x3FFFBF).contains(&u),
        "eight-bit-graphic" => (0x3FFFC0..=0x3FFFFF).contains(&u),
        "eight-bit" => (0x3FFF80..=0x3FFFFF).contains(&u),
        "emacs" => true,
        // Charsets without a coding-table model: a char whose GNU
        // char-charset winner is NAME is at least a member.
        _ => char_charset_of(ch) == Some(name),
    }
}

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

/// GNU's `(char-charset CH)' result — exact below U+30000 (generated
/// table), `unicode' elsewhere.
fn char_charset_of(ch: i128) -> Option<&'static str> {
    if !(0..=0x3FFFFF).contains(&ch) {
        return None;
    }
    let u = ch as u32;
    super::enc_tables::CHAR_CHARSET
        .binary_search_by_key(&u, |&(x, _)| x)
        .ok()
        .map(|ix| super::enc_tables::CHAR_CHARSET[ix].1)
}

fn f_char_charset(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    let ch = match &args[0] {
        Value::Int(n) => *n,
        other => return Err(i.wrong_type_mut("characterp", other)),
    };
    if !(0..=0x3FFFFF).contains(&ch) {
        return Err(i.wrong_type_mut("characterp", &args[0]));
    }
    match args.get(1) {
        None | Some(Value::Nil) => match super::charset::char_charset(i, ch as i64, &[]) {
            Some(name) => Ok(Value::Sym(i.intern(&name))),
            None => Ok(Value::Nil),
        },
        Some(v) => charset_restricted_charset(i, ch as i64, v),
    }
}

/// GNU `Fchar_charset' with a non-nil RESTRICTION: a list restricts
/// to its member charsets in LIST order (each must satisfy
/// `charsetp'); any other non-nil value names a coding system whose
/// `coding_system_charset_list' applies (unknown → coding-system-error).
fn charset_restricted_charset(i: &mut Interp, ch: i64, v: &Value) -> EvalResult {
    match v {
        Value::Cons(_) => {
            for item in v.list_to_vec().unwrap_or_default() {
                let name = match &item {
                    Value::Sym(s) => i.symbol_name(*s),
                    _ => return Err(i.wrong_type_mut("charsetp", &item)),
                };
                if !super::charset::charset_defined(i, &name) {
                    return Err(i.wrong_type_mut("charsetp", &item));
                }
                if super::charset::charset_encode(i, &name, ch).is_some() {
                    return Ok(Value::Sym(i.intern(&name)));
                }
            }
            Ok(Value::Nil)
        }
        _ => {
            // GNU char-charset RESTRICTION: a non-list non-nil must
            // be a coding-system SYMBOL — other atoms signal
            // `symbolp', unknown symbols `coding-system-error'.
            match v {
                Value::Sym(_) => {}
                other => return Err(i.wrong_type_mut("symbolp", other)),
            }
            if super::misc::coding_known(i, v).is_none() {
                let cs = i.q("coding-system-error");
                return Err(i.signal_data(cs, vec![v.clone()]));
            }
            let list = super::charset::coding_system_charset_names(i, v);
            let Ok(names) = list.list_to_vec() else {
                return Ok(Value::Nil);
            };
            for item in names {
                if let Value::Sym(s) = &item {
                    let name = i.symbol_name(*s);
                    if super::charset::charset_encode(i, &name, ch).is_some() {
                        return Ok(item.clone());
                    }
                }
            }
            Ok(Value::Nil)
        }
    }
}
