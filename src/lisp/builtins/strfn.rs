//! String subrs: concat, substring, comparison, case, format, etc.

use super::{S, arg, want_int, want_string};
use crate::lisp::Interp;
use crate::lisp::error::{EvalResult, Flow};
use crate::lisp::obarray::sym;
use crate::lisp::value::{Subr, Value, eight_bit_byte, lisp_char, lisp_char_code};

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
        f_substring,
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
        2,
        f_string_lines,
        "Split STRING on newlines."
    ),
    S!(
        "upcase-initials-region",
        2,
        2,
        f_upcase_initials_region,
        ""
    ),
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
        f_string_to_unibyte,
        "Return STRING unchanged."
    ),
    S!(
        "string-make-multibyte",
        1,
        1,
        f_string_to_multibyte,
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
        f_string_collate_equalp,
        "Collation equality (simple)."
    ),
    S!(
        "string-collate-lessp",
        2,
        3,
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

fn f_char_identity(_i: &mut Interp, args: Vec<Value>) -> EvalResult {
    Ok(args[0].clone())
}

fn f_string(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let mut out = String::new();
    for a in &args {
        match a {
            Value::Int(n) => match lisp_char(*n as u32) {
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
                    Value::Int(n) => match lisp_char(n as u32) {
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
                    Value::Int(n) => match lisp_char(*n as u32) {
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
    let mut ivs: Vec<(usize, usize, Vec<Value>)> = Vec::new();
    let mut saw_props = false;
    for a in &args {
        // GNU concat carries each source string's text properties.
        let off = out.chars().count();
        if let Value::Str(s) = a {
            if i.has_str_props(s) {
                saw_props = true;
                for (s0, e0, pl) in i.str_props(s) {
                    // GNU's concat copies props through
                    // copy_text_properties, reversing plist order.
                    ivs.push((s0 + off, e0 + off, crate::buffer::primitives::plist_pairs_rev(pl)));
                }
            }
        }
        concat_seq(i, a, &mut out)?;
    }
    let ns = std::rc::Rc::new(std::cell::RefCell::new(out));
    if saw_props {
        i.set_str_props(&ns, ivs);
    }
    Ok(Value::Str(ns))
}

fn f_ngettext(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    // (ngettext MSGID MSGID-PLURAL N) — English rule: N != 1 → plural.
    let sing = want_string(i, &args[0])?;
    let plural = want_string(i, &args[1])?;
    let n = want_int(i, &args[2])?;
    Ok(Value::string(if n == 1 { sing } else { plural }))
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
                    out.push(Value::Int(lisp_char_code(c)));
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
        Value::Str(s) => s.borrow().chars().map(|c| Value::Int(lisp_char_code(c))).collect(),
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
        Ok(Value::Vec(std::rc::Rc::new(std::cell::RefCell::new(slice))))
    } else {
        let ns = std::rc::Rc::new(std::cell::RefCell::new(
            slice
                .iter()
                .filter_map(|v| match v {
                    Value::Int(n) => lisp_char(*n as u32),
                    _ => None,
                })
                .collect::<String>(),
        ));
        // GNU substring copies the source's text properties on the
        // sliced range (shifted to 0-based on the result).
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
                        (lo < hi)
                            .then(|| (lo - f0, hi - f0, crate::buffer::primitives::plist_pairs_rev(pl)))
                    })
                    .collect();
                i.set_str_props(&ns, ivs);
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
fn f_string_equal(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let a = str_or_sym_arg(i, &args[0])?;
    let b = str_or_sym_arg(i, &args[1])?;
    Ok(Value::from_bool(a == b))
}
fn f_string_lessp(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let a = str_or_sym_arg(i, &args[0])?;
    let b = str_or_sym_arg(i, &args[1])?;
    Ok(Value::from_bool(a < b))
}
fn f_string_collate_equalp(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let a = want_string(i, &args[0])?;
    let b = want_string(i, &args[1])?;
    Ok(Value::from_bool(a == b))
}
fn f_string_collate_lessp(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let a = want_string(i, &args[0])?;
    let b = want_string(i, &args[1])?;
    Ok(Value::from_bool(a < b))
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
fn ct_case(i: &Interp, table: &Value, c: u32) -> u32 {
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

/// Single-char titlecase: TITLE_OVERRIDES first, then the upcase map.
fn title_char(i: &Interp, up: &Value, c: u32) -> u32 {
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

fn casify(i: &mut Interp, arg: &Value, op: CaseOp) -> EvalResult {
    let down = i.current_case_table();
    let up = case_extra(&down, 0).unwrap_or_else(|| down.clone());
    match arg {
        Value::Int(n) if (0..=0x3f_ffff).contains(n) => {
            let cp = *n as u32;
            let r = match op {
                CaseOp::Up => ct_case(i, &up, cp),
                CaseOp::Down => ct_case(i, &down, cp),
                _ => title_char(i, &up, cp),
            };
            Ok(Value::Int(r as i128))
        }
        Value::Str(s) => {
            let src = s.borrow().clone();
            Ok(Value::string(case_str(i, &src, op, &down, &up)))
        }
        other => Err(i.wrong_type_mut("char-or-string-p", other)),
    }
}

/// Casify a source string per GNU casefiddle (shared by the string
/// functions, the *-region commands, and the *-word commands).
pub(crate) fn case_str(
    i: &mut Interp,
    src: &str,
    op: CaseOp,
    down: &Value,
    up: &Value,
) -> String {
    let chars: Vec<char> = src.chars().collect();
    let syn = crate::editor::syntax_table_entries(i);
    let wordp = |c: char| crate::editor::syntax_entry_code(syn.as_ref(), c) == b'w';
    let mut out = String::new();
    let mut in_word = false;
    for (idx, &c) in chars.iter().enumerate() {
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
                        if let Some(sp) =
                            special_lookup(crate::lisp::ctdata::SPECIAL_TITLE, c)
                        {
                            out.push_str(sp);
                        } else {
                            out.push(char::from_u32(title_char(i, up, cp)).unwrap_or(c));
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
    }
    out
}

/// `upcase-region', `downcase-region', `capitalize-region',
/// `upcase-initials-region': recase the region in place via the
/// buffer's case table, returning nil.
fn casify_region(i: &mut Interp, args: &[Value], op: CaseOp) -> EvalResult {
    crate::lisp::builtins::misc::check_region_positions(i, args)?;
    let Some(b) = i.buffers.get(i.current_buffer) else {
        return Ok(Value::Nil);
    };
    let (Value::Int(s), Value::Int(e)) = (&args[0], &args[1]) else {
        return Ok(Value::Nil);
    };
    let down = i.current_case_table();
    let up = case_extra(&down, 0).unwrap_or_else(|| down.clone());
    let (start, end) = ((*s - 1) as usize, (*e - 1) as usize);
    let old = b.borrow().text.substring(start, end);
    let new = case_str(i, &old, op, &down, &up);
    if new != old {
        let mut bb = b.borrow_mut();
        bb.delete_region(start, end);
        bb.insert_at(start, &new);
    }
    Ok(Value::Nil)
}

fn f_upcase_initials_region(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    casify_region(i, &args, CaseOp::CapUp)
}

fn f_upcase(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    casify(i, &args[0], CaseOp::Up)
}
fn f_downcase(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    casify(i, &args[0], CaseOp::Down)
}
fn f_capitalize(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    casify(i, &args[0], CaseOp::Cap)
}
fn f_upcase_initials(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    casify(i, &args[0], CaseOp::CapUp)
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
        Value::Float(f) => Ok(Value::string(crate::lisp::print::format_float(**f))),
        other => Err(i.wrong_type_mut("numberp", other)),
    }
}
fn f_string_to_char(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let s = want_string(i, &args[0])?;
    Ok(Value::Int(s.chars().next().map(lisp_char_code).unwrap_or(0)))
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
            Value::Int(n) => lisp_char(*n as u32),
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
        let sym = i.intern("wrong-length-argument");
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
    while pos + flen <= schars.len() {
        if schars[pos..pos + flen] == fchars[..] {
            out.push_str(&to);
            pos += flen;
            changed = true;
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
    let ns = std::rc::Rc::new(std::cell::RefCell::new(out));
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
fn f_string_chop_newline(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let s = want_string(i, &args[0])?;
    Ok(Value::string(s.trim_end_matches('\n').to_string()))
}
/// UAX #11 "East Asian Ambiguous" ranges (the classic wcwidth table).
/// GNU's `char-width-table' counts these as 2 columns in a
/// wide-ambiguous context (this NS build's `char-width' returns 2 for
/// e.g. é … α §).
const EAW_AMBIGUOUS: &[(u32, u32)] = &[
    (0x00a1, 0x00a1), (0x00a4, 0x00a4), (0x00a7, 0x00a8),
    (0x00aa, 0x00aa), (0x00ad, 0x00ad), (0x00ae, 0x00b1),
    (0x00b2, 0x00b5),
    (0x00b6, 0x00b6), (0x00b8, 0x00ba), (0x00bc, 0x00bf),
    (0x00c6, 0x00c6),
    (0x00d0, 0x00d0), (0x00d7, 0x00d8), (0x00de, 0x00e1),
    (0x00e6, 0x00e6), (0x00e8, 0x00ea), (0x00ec, 0x00ed),
    (0x00f0, 0x00f0), (0x00f2, 0x00f3), (0x00f7, 0x00fa),
    (0x00fc, 0x00fc), (0x00fe, 0x00fe), (0x0101, 0x0101),
    (0x0111, 0x0111), (0x0113, 0x0113), (0x011b, 0x011b),
    (0x0126, 0x0127), (0x012b, 0x012b), (0x0131, 0x0133),
    (0x0138, 0x0138), (0x013f, 0x0142), (0x0144, 0x0144),
    (0x0148, 0x014b), (0x014d, 0x014d), (0x0152, 0x0153),
    (0x0166, 0x0167), (0x016b, 0x016b), (0x01ce, 0x01ce),
    (0x01d0, 0x01d0), (0x01d2, 0x01d2), (0x01d4, 0x01d4),
    (0x01d6, 0x01d6), (0x01d8, 0x01d8), (0x01da, 0x01da),
    (0x01dc, 0x01dc), (0x0251, 0x0251), (0x0261, 0x0261),
    (0x02c4, 0x02c4), (0x02c7, 0x02c7), (0x02c9, 0x02cb),
    (0x02cd, 0x02d0), (0x02d8, 0x02db), (0x02dd, 0x02dd),
    (0x02df, 0x02df), (0x0300, 0x036f), (0x0391, 0x03a1),
    (0x03a3, 0x03a9),
    (0x03b1, 0x03c1), (0x03c3, 0x03cb), (0x0401, 0x0401),
    (0x0403, 0x040f), (0x0410, 0x044f), (0x0451, 0x0451),
    (0x045c, 0x045f), (0x2010, 0x2010), (0x2013, 0x2016),
    (0x2018, 0x2019), (0x201c, 0x201d), (0x2020, 0x2022),
    (0x2024, 0x2027), (0x2030, 0x2030), (0x2032, 0x2033),
    (0x2035, 0x2035), (0x203b, 0x203b), (0x203e, 0x203e),
    (0x2074, 0x2074), (0x207f, 0x207f), (0x2081, 0x2084),
    (0x20ac, 0x20ac), (0x2103, 0x2103), (0x2105, 0x2105),
    (0x2109, 0x2109), (0x2113, 0x2113), (0x2116, 0x2116),
    (0x2121, 0x2122), (0x2126, 0x2126), (0x212b, 0x212b),
    (0x2153, 0x2154), (0x215b, 0x215e), (0x2160, 0x216b),
    (0x2170, 0x2179), (0x2190, 0x2199), (0x21b8, 0x21b9),
    (0x21d2, 0x21d2), (0x21d4, 0x21d4), (0x21e7, 0x21e7),
    (0x2200, 0x2200), (0x2202, 0x2203), (0x2207, 0x2208),
    (0x220b, 0x220b), (0x220f, 0x220f), (0x2211, 0x2211),
    (0x2215, 0x2215), (0x221a, 0x221a), (0x221d, 0x2220),
    (0x2223, 0x2223), (0x2225, 0x2225), (0x2227, 0x222c),
    (0x222e, 0x222e), (0x2234, 0x2237), (0x223c, 0x223d),
    (0x2248, 0x2248), (0x224c, 0x224c), (0x2252, 0x2252),
    (0x2260, 0x2261), (0x2264, 0x2267), (0x226a, 0x226b),
    (0x226e, 0x226f), (0x2282, 0x2283), (0x2286, 0x2287),
    (0x2295, 0x2295), (0x2299, 0x2299), (0x22a5, 0x22a5),
    (0x22bf, 0x22bf), (0x2312, 0x2312), (0x2460, 0x24e9),
    (0x24eb, 0x254b), (0x2550, 0x2573), (0x2580, 0x258f),
    (0x2592, 0x2595), (0x25a0, 0x25a1), (0x25a3, 0x25a9),
    (0x25b2, 0x25b3), (0x25b6, 0x25b7), (0x25bc, 0x25bd),
    (0x25c0, 0x25c1), (0x25c6, 0x25c8), (0x25cb, 0x25cb),
    (0x25ce, 0x25d1), (0x25e2, 0x25e5), (0x25ef, 0x25ef),
    (0x2605, 0x2606), (0x2609, 0x2609), (0x260e, 0x260f),
    (0x2614, 0x2615), (0x261c, 0x261c), (0x261e, 0x261e),
    (0x2640, 0x2640), (0x2642, 0x2642), (0x2660, 0x2661),
    (0x2663, 0x2665), (0x2667, 0x266a), (0x266c, 0x266d),
    (0x266f, 0x266f), (0x269e, 0x269f), (0x26bf, 0x26bf),
    (0x26c6, 0x26cd), (0x26cf, 0x26d3), (0x26d5, 0x26e1),
    (0x26e3, 0x26e3), (0x26e8, 0x26e9), (0x26eb, 0x26f1),
    (0x26f4, 0x26f4), (0x26f6, 0x26f9), (0x26fb, 0x26fc),
    (0x26fe, 0x26ff), (0x273d, 0x273d), (0x2776, 0x277f),
    (0x2b56, 0x2b59), (0x3248, 0x324f), (0xe000, 0xf8ff),
    (0xfe00, 0xfe0f), (0xfffd, 0xfffd), (0x1f18e, 0x1f18e),
    (0x1f191, 0x1f19a), (0xe0100, 0xe01ef),
    (0xf0000, 0xffffd), (0x100000, 0x10fffd),
];

/// Display column width of one character under GNU's `strwidth'
/// rules: each character contributes its char-width-table width —
/// tab is 8 flat (no tab-stop tracking), newline counts 0, other
/// controls and DEL count 2, C1 controls count 4 (octal escapes), and
/// the rest use the char-width table — approximated by Unicode width
/// with East Asian Ambiguous characters counting 2 columns, matching
/// this GNU build's `char-width-table' contents.
fn gnu_char_width(c: char) -> usize {
    use unicode_width::UnicodeWidthChar;
    let n = c as u32;
    if c == '\t' {
        8
    } else if c == '\n' {
        0
    } else if n < 0x20 || n == 0x7f {
        2
    } else if (0x80..0xa0).contains(&n) {
        4
    } else if EAW_AMBIGUOUS
        .binary_search_by(|&(lo, hi)| {
            if n < lo {
                std::cmp::Ordering::Greater
            } else if n > hi {
                std::cmp::Ordering::Less
            } else {
                std::cmp::Ordering::Equal
            }
        })
        .is_ok()
    {
        2
    } else {
        UnicodeWidthChar::width(c).unwrap_or(0)
    }
}

fn f_string_width(i: &mut Interp, args: Vec<Value>) -> EvalResult {
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
    let mut w = 0usize;
    for c in &chars[(from.max(0) as usize).min(chars.len())..t.min(chars.len())] {
        w += gnu_char_width(*c);
    }
    Ok(Value::Int(w as i128))
}
/// `truncate-string-to-width' — GNU signature:
/// (truncate-string-to-width STRING WIDTH &optional START-COLUMN
/// PADDING ELLIPSIS).  Cuts STRING so its display width doesn't exceed
/// WIDTH; when the string is actually truncated and ELLIPSIS is
/// non-nil, an ellipsis is appended, reserving its own width first
/// (ELLIPSIS of t means `truncate-string-ellipsis', defaulting to "…"
/// which GNU's char-width table counts as 2 columns).  PADDING non-nil
/// pads the result out to WIDTH — PADDING of a character pads with
/// that character, otherwise with spaces.  START-COLUMN is the
/// starting display column (affects tab expansion and leaves room).
fn f_truncate_string_to_width(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let s = want_string(i, &args[0])?;
    let width = want_int(i, &args[1])?.max(0) as usize;
    let start_col = match args.get(2) {
        Some(Value::Int(n)) => (*n).max(0) as usize,
        Some(other) if !other.is_nil() => {
            return Err(i.wrong_type_mut("integerp", other))
        }
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
            let eid = i.intern("truncate-string-ellipsis");
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
                w += gnu_char_width(c);
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
        col += gnu_char_width(chars[idx]);
        idx += 1;
    }
    // Scan the tail: if it fits within WIDTH, return it whole (an
    // ellipsis is only used when the string is actually truncated).
    let mut fits = true;
    {
        let mut c = col;
        for &ch in &chars[idx..] {
            let cw = gnu_char_width(ch);
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
        let cw = gnu_char_width(c);
        if col + cw > budget {
            truncated = true;
            break;
        }
        out.push(c);
        col += cw;
    }
    if truncated {
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
            }
            Ok(args[0].clone())
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
    Ok(Value::Int(lisp_char_code(chars[idx as usize])))
}

// ---------- format ----------

/// `format` — printf-style with elisp conventions:
/// `%s` princ, `%S` prin1, `%d`/`%o`/`%x`/`%X`/`%e`/`%f`/`%g` numeric,
/// `%c` char, `%%` literal. Supports `%Nd`, `%-Ns`, `%0Nd`, `%.Nf`.
fn f_format(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let fmt = want_string(i, &args[0])?;
    format_impl(i, &fmt, &args)
}

fn f_format_message(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let fmt = want_string(i, &args[0])?;
    let fmt = super::evalfn::translate_message_quotes(&fmt);
    format_impl(i, &fmt, &args)
}

fn format_impl(i: &mut Interp, fmt: &str, args: &[Value]) -> EvalResult {
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
        let aidx = match pos_arg {
            Some(n) => n,
            None => ai,
        };
        if pos_arg.is_none() {
            ai += 1;
        }
        // GNU signals (error "Not enough arguments for format string")
        // when a spec has no corresponding argument.
        let Some(a) = args.get(aidx).cloned() else {
            return Err(i.error("Not enough arguments for format string"));
        };
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
                    Value::Float(f) => **f as i128,
                    Value::Marker(m) => m.borrow().position as i128 + 1,
                    _ => return Err(fmt_type_err(i)),
                };
                // C printf precision: minimum digit count, zero-padded
                // (%.0d of 0 prints nothing); the sign precedes the pad.
                let mut s = match prec {
                    Some(pr) => {
                        if pr == 0 && n == 0 {
                            String::new()
                        } else {
                            let neg = n < 0;
                            let digits = n.unsigned_abs().to_string();
                            if digits.len() < pr {
                                format!(
                                    "{}{}{}",
                                    if neg { "-" } else { "" },
                                    "0".repeat(pr - digits.len()),
                                    digits
                                )
                            } else {
                                format!("{}{}", if neg { "-" } else { "" }, digits)
                            }
                        }
                    }
                    None => n.to_string(),
                };
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
                    _ => return Err(fmt_type_err(i)),
                };
                let ch = char::from_u32((n & 0x3f_ffff) as u32).unwrap_or('\0');
                ch.to_string()
            }
            'o' => {
                let n = int_of(i, &a)?;
                let s = prec_pad_int(format!("{:o}", n), prec);
                if alt && !s.starts_with('0') {
                    format!("0{}", s)
                } else {
                    s
                }
            }
            'x' => {
                let n = int_of(i, &a)?;
                let s = prec_pad_int(format!("{:x}", n), prec);
                if alt && n != 0 {
                    format!("0x{}", s)
                } else {
                    s
                }
            }
            'X' => {
                let n = int_of(i, &a)?;
                let s = prec_pad_int(format!("{:X}", n), prec);
                if alt && n != 0 {
                    format!("0X{}", s)
                } else {
                    s
                }
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
    i.error("Format specifier doesn\u{2019}t match argument type")
}

fn int_of(i: &mut Interp, v: &Value) -> Result<i128, super::Flow> {
    match v {
        Value::Int(n) => Ok(*n),
        Value::Float(f) => Ok(**f as i128),
        _ => Err(fmt_type_err(i)),
    }
}

fn float_of(i: &mut Interp, v: &Value) -> Result<f64, super::Flow> {
    match v {
        Value::Int(n) => Ok(*n as f64),
        Value::Float(f) => Ok(**f),
        _ => Err(fmt_type_err(i)),
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
    let items: Vec<Value> = s.chars().map(|c| Value::Int(lisp_char_code(c))).collect();
    Ok(Value::list(items))
}

fn f_string_to_vector(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    // Emacs accepts nil (treated as the empty sequence).
    let s = match &args[0] {
        Value::Nil => String::new(),
        v => want_string(i, v)?,
    };
    let items: Vec<Value> = s.chars().map(|c| Value::Int(lisp_char_code(c))).collect();
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
    let c = match &args[0] {
        Value::Int(n) => *n as u32,
        other => return Err(i.wrong_type_mut("characterp", other)),
    };
    let ch = char::from_u32(c).unwrap_or('\0');
    Ok(Value::Int(gnu_char_width(ch) as i128))
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

/// `string-to-multibyte' — in a unibyte string, each byte-value char
/// ≥0x80 becomes an eight-bit char (0x3FFF80 + byte − 0x80); a
/// multibyte string is returned unchanged.
fn f_string_to_multibyte(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let s = want_string(i, &args[0])?;
    let unibyte_in = match &args[0] {
        Value::Str(r) => {
            i.is_unibyte_str(r) || (!i.is_multibyte_str(r) && !r.borrow().chars().any(|c| (c as u32) > 0xFF))
        }
        _ => false,
    };
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
fn f_string_to_unibyte(i: &mut Interp, args: Vec<Value>) -> EvalResult {
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
            return Err(i.error(format!(
                "Cannot convert character at index {ix} to unibyte"
            )));
        }
    }
    let v = Value::string(out);
    if let Value::Str(r) = &v {
        i.mark_unibyte(r);
    }
    Ok(v)
}

/// `string-as-unibyte' — a unibyte string is returned unchanged; a
/// multibyte string's chars are re-encoded as their UTF-8 bytes.
fn f_string_as_unibyte(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let s = want_string(i, &args[0])?;
    let multibyte = match &args[0] {
        Value::Str(r) => {
            i.is_multibyte_str(r) || (!i.is_unibyte_str(r) && r.borrow().chars().any(|c| (c as u32) > 0xFF))
        }
        _ => false,
    };
    let out = if multibyte {
        let mut o = String::new();
        for c in s.chars() {
            if let Some(b) = eight_bit_byte(c) {
                o.push(b as char);
            } else {
                let mut buf = [0u8; 4];
                o.extend(c.encode_utf8(&mut buf).bytes().map(|b| b as char));
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
fn f_string_as_multibyte(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let s = want_string(i, &args[0])?;
    let multibyte_in = match &args[0] {
        Value::Str(r) => {
            i.is_multibyte_str(r)
                || (!i.is_unibyte_str(r)
                    && r.borrow().chars().any(|c| (c as u32) > 0xFF))
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

fn f_multibyte_string_p(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    Ok(Value::from_bool(match &args[0] {
        Value::Str(s) => {
            if i.is_unibyte_str(s) {
                false
            } else if i.is_multibyte_str(s) {
                true
            } else {
                // Unmarked strings are unibyte when all chars are
                // byte-representable (GNU's storage rule).
                s.borrow().chars().any(|c| (c as u32) > 0xFF)
            }
        }
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
    let v = Value::string(s);
    if let Value::Str(r) = &v {
        i.mark_unibyte(r);
    }
    Ok(v)
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
    // (encode-char CH CHARSET)
    let ch = match &args[0] {
        Value::Int(n) if (0..0x400000).contains(n) => *n as i64,
        _ => return Err(i.wrong_type_mut("characterp", &args[0])),
    };
    let name = super::charset::want_charset(i, &args[1])?;
    match super::charset::encode_charset_code(&name, ch) {
        Some(c) => Ok(Value::Int(c.into())),
        None => Ok(Value::Nil),
    }
}

fn f_decode_char(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    // (decode-char CHARSET CODE) — nil when CODE is outside CHARSET's space.
    let name = super::charset::want_charset(i, &args[0])?;
    let code = want_int(i, &args[1])?;
    match super::charset::decode_charset_code(&name, code as i64) {
        Some(c) => Ok(Value::Int(c.into())),
        None => Ok(Value::Nil),
    }
}

/// Charsets we model, in GNU's `charset-priority-list` order.
pub(crate) const CHARSET_PRIORITY: &[&str] =
    crate::lisp::builtins::charset::GNU_CHARSET_PRIORITY;

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
        "japanese-jisx0212" => in_tab(ENC_EUC_JP)
            && ENC_EUC_JP
                .binary_search_by_key(&u, |&(x, _)| x)
                .map(|ix| (ENC_EUC_JP[ix].1 >> 56) >= 3)
                .unwrap_or(false),
        "chinese-gb2312" => in_tab(ENC_GB2312),
        "big5" | "chinese-big5-1" | "chinese-big5-2" => in_tab(ENC_BIG5),
        "koi8" | "koi8-r" => in_tab(ENC_KOI8_R),
        "cyrillic-iso8859-5" => {
            u == 0x401 || (0x410..=0x44F).contains(&u) || u == 0x451
        }
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

fn f_char_charset(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let ch = match &args[0] {
        Value::Int(n) => *n,
        other => return Err(i.wrong_type_mut("characterp", other)),
    };
    if !(0..=0x3FFFFF).contains(&ch) {
        return Err(i.wrong_type_mut("characterp", &args[0]));
    }
    let restriction = match args.get(1) {
        None | Some(Value::Nil) => Vec::new(),
        Some(v) => charset_restriction(i, v)?,
    };
    if restriction.is_empty() {
        let name = char_charset_of(ch).unwrap_or("unicode");
        return Ok(Value::Sym(i.intern(name)));
    }
    let refs: Vec<&str> = restriction.iter().map(String::as_str).collect();
    match char_charset_in(ch, &refs) {
        Some(name) => Ok(Value::Sym(i.intern(name))),
        None => Ok(Value::Nil),
    }
}

/// Resolve a `char-charset` RESTRICTION arg to charset names. GNU: a
/// list restricts to its member charsets (each must satisfy
/// `charsetp'); any other non-nil value names a coding system whose
/// charset list applies (unknown → `coding-system-error').
fn charset_restriction(i: &mut Interp, v: &Value) -> Result<Vec<String>, Flow> {
    let mut out = Vec::new();
    match v {
        Value::Cons(_) => {
            for item in v.list_to_vec().unwrap_or_default() {
                let name = match &item {
                    Value::Sym(s) => i.symbol_name(*s),
                    _ => return Err(i.wrong_type_mut("charsetp", &item)),
                };
                if CHARSET_PRIORITY.contains(&name.as_str())
                    || crate::lisp::builtins::charset::GNU_CHARSET_PRIORITY
                        .contains(&name.as_str())
                    || name == "emacs"
                    || name == "eight-bit"
                {
                    out.push(name);
                } else {
                    return Err(i.wrong_type_mut("charsetp", &item));
                }
            }
        }
        _ => {
            if let Some(cs) = super::misc::coding_known(i, v) {
                for cs in coding_charsets(&cs) {
                    out.push(cs.to_string());
                }
            } else {
                let cs = i.intern("coding-system-error");
                return Err(i.signal_data(cs, vec![v.clone()]));
            }
        }
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
