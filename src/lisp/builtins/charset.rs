//! Charsets, char-code properties, and coding-system helpers.
//!
//! GNU Emacs carries a full Unicode/charset machinery; we keep a compact
//! model that matches the observable Lisp API: charset objects identified
//! by name with plists, char-code property tables (general-category
//! provided for ASCII), and coding-system type resolution.

use std::cell::RefCell;
use std::rc::Rc;

use super::{S, arg, eq_values};
use super::misc::{char_table_vec, coding_known, is_char_table};
use crate::lisp::error::Flow;
use crate::lisp::value::{Subr, Value};
use crate::lisp::{EvalResult, Interp};

fn want_sym(i: &mut Interp, v: &Value) -> Result<u32, Flow> {
    match v {
        Value::Sym(s) => Ok(*s),
        _ => Err(i.wrong_type_mut("symbolp", v)),
    }
}

fn symv(i: &mut Interp, s: &str) -> Value {
    Value::Sym(i.intern(s))
}

/// Names GNU Emacs defines as charsets (subset covering the common ones;
/// `define-charset` can extend the table at runtime).
const BUILTIN_CHARSETS: &[&str] = &[
    "ascii",
    "unicode",
    "emacs",
    "eight-bit",
    "ucs",
    "iso-8859-1",
    "latin-iso8859-1",
    "eight-bit-control",
    "eight-bit-graphic",
    "control-1",
    "mule-unicode-0100-24ff",
    "mule-unicode-2500-33ff",
    "mule-unicode-e000-ffff",
    "big5",
    "cp932",
    "cp932-2-byte",
    "chinese-big5-1",
    "chinese-big5-2",
    "japanese-jisx0208",
    "jisx0201",
    "latin-jisx0201",
    "katakana-jisx0201",
    "katakana-sjis",
];

fn charset_entry<'a>(i: &'a Interp, name: &str) -> Option<&'a (String, Value)> {
    i.charsets.iter().find(|(n, _)| n == name)
}

fn charset_defined(i: &Interp, name: &str) -> bool {
    let name = canonical_charset(i, name);
    BUILTIN_CHARSETS.contains(&name.as_str()) || charset_entry(i, &name).is_some()
}

pub(crate) fn want_charset(i: &mut Interp, v: &Value) -> Result<String, Flow> {
    let name = match v {
        Value::Sym(s) => i.symbol_name(*s),
        _ => return Err(i.wrong_type_mut("charsetp", v)),
    };
    if charset_defined(i, &name) {
        Ok(canonical_charset(i, &name))
    } else {
        Err(i.wrong_type_mut("charsetp", v))
    }
}

fn canonical_charset(i: &Interp, name: &str) -> String {
    if let Some(target) = i.charset_aliases.iter().find(|(a, _)| a == name) {
        return target.1.clone();
    }
    name.to_string()
}

fn ensure_charset(i: &mut Interp, name: &str) {
    let name = canonical_charset(i, name);
    if charset_entry(i, &name).is_none() {
        i.charsets.push((name, Value::Nil));
    }
}

fn f_charsetp(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let name = match &a[0] {
        Value::Sym(s) => i.symbol_name(*s),
        _ => return Ok(Value::Nil),
    };
    Ok(Value::from_bool(charset_defined(i, &name)))
}

fn f_define_charset(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    // (name info-vector &rest props) — keep name + plist.
    let sid = want_sym(i, &a[0])?;
    if !matches!(a[1], Value::Vec(_)) {
        // GNU reports a generic error for malformed INFO, not wrong-type.
        return Err(i.error("Attribute :invalid-code must be specified"));
    }
    let name = i.symbol_name(sid);
    let plist = if a.len() > 2 {
        Value::list(a[2..].to_vec())
    } else {
        Value::Nil
    };
    // GNU requires one of :code-offset, :map, :parents to map code points.
    let has_mapping = a[2..].chunks(2).any(|kv| {
        matches!(kv.first(), Some(Value::Sym(s)) if {
            let n = i.symbol_name(*s);
            n == ":code-offset" || n == ":map" || n == ":parents"
        })
    });
    if !has_mapping {
        return Err(i.error("None of :code-offset, :map, :parents are specified"));
    }
    if let Some(e) = i.charsets.iter_mut().find(|(n, _)| *n == name) {
        e.1 = plist;
    } else {
        i.charsets.push((name, plist));
    }
    Ok(Value::Nil)
}

fn f_charset_plist(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let name = want_charset(i, &a[0])?;
    if let Some(e) = charset_entry(i, &name) {
        if !e.1.is_nil() {
            return Ok(e.1.clone());
        }
    }
    Ok(default_charset_plist(i, &name))
}

/// GNU synthesizes a charset's attribute plist on demand when the user
/// has not replaced it via `set-charset-plist`.
fn default_charset_plist(i: &mut Interp, name: &str) -> Value {
    let kv: Vec<Value> = match name {
        "ascii" => vec![
            symv(i, ":name"),
            symv(i, "ascii"),
            symv(i, ":dimension"),
            Value::Int(1),
            symv(i, ":code-space"),
            Value::Vec(Rc::new(RefCell::new(vec![
                Value::Int(0),
                Value::Int(127),
                Value::Int(0),
                Value::Int(0),
                Value::Int(0),
                Value::Int(0),
                Value::Int(0),
                Value::Int(0),
            ]))),
            symv(i, ":iso-final-char"),
            Value::Int(66),
            symv(i, ":emacs-mule-id"),
            Value::Int(0),
            symv(i, ":ascii-compatible-p"),
            Value::Sym(i.intern("t")),
            symv(i, ":code-offset"),
            Value::Int(0),
            symv(i, ":docstring"),
            Value::string("ASCII (ISO646 IRV)"),
            symv(i, ":short-name"),
            Value::string("ASCII"),
            symv(i, ":long-name"),
            Value::string("ASCII (ISO646 IRV)"),
        ],
        _ => vec![symv(i, ":name"), symv(i, name)],
    };
    Value::list(kv)
}

fn f_set_charset_plist(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let name = want_charset(i, &a[0])?;
    ensure_charset(i, &name);
    if let Some(e) = i.charsets.iter_mut().find(|(n, _)| *n == name) {
        e.1 = a[1].clone();
    }
    Ok(Value::Nil)
}

fn f_get_charset_property(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let name = want_charset(i, &a[0])?;
    let prop = want_sym(i, &a[1])?;
    let plist = charset_entry(i, &name)
        .map(|e| e.1.clone())
        .unwrap_or(Value::Nil);
    Ok(crate::lisp::eval::plist_get(&plist, prop))
}

fn f_put_charset_property(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let name = want_charset(i, &a[0])?;
    let prop = want_sym(i, &a[1])?;
    ensure_charset(i, &name);
    if let Some(e) = i.charsets.iter_mut().find(|(n, _)| *n == name) {
        e.1 = crate::lisp::eval::plist_put(&e.1, prop, a[2].clone());
    }
    Ok(a[2].clone())
}

fn f_define_charset_alias(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let aid = want_sym(i, &a[0])?;
    let tid = want_sym(i, &a[1])?;
    let alias = i.symbol_name(aid);
    let target = i.symbol_name(tid);
    i.charset_aliases.push((alias, target));
    Ok(Value::Nil)
}

fn f_unify_charset(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    // (charset &optional unify-map deunify-map) — GNU signals
    // "Can't unify charset: X" when the charset cannot be unified
    // (ucs, or any charset lacking a :map/:code-space map).
    let name = want_charset(i, &a[0])?;
    // GNU signals "Can't unify charset: X" for every charset we model —
    // none of them carries a unification map.
    Err(i.error(format!("Can't unify charset: {name}")))
}

fn f_charset_after(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let pos = match a.first() {
        None | Some(Value::Nil) => None,
        Some(v) => v.int().map(|n| n as usize),
    };
    let bid = i.current_buffer;
    let ch = i.buffers.get(bid).and_then(|b| {
        let bb = b.borrow();
        // bb.point() is the 0-based offset; POS is 1-based like GNU.
        let at = pos.unwrap_or(bb.point() + 1).saturating_sub(1);
        bb.text.text().chars().nth(at)
    });
    Ok(match ch {
        None => Value::Nil,
        Some(c) if (c as u32) < 0x80 => symv(i, "ascii"),
        Some(_) => symv(i, "unicode"),
    })
}

fn find_charset_of_chars(i: &mut Interp, chars: impl Iterator<Item = char>) -> Value {
    // GNU picks, for each char, the highest-priority charset containing
    // it (charset-priority-list order), then emits the covering set sorted
    // by internal charset id — ascii first, then the JIS tables.
    let mut names: Vec<&'static str> = Vec::new();
    for c in chars {
        if let Some(cs) = super::strfn::char_charset_in(c as i128, &[]) {
            if !names.contains(&cs) {
                names.push(cs);
            }
        }
    }
    if names.is_empty() {
        // GNU returns nil for an empty range/string.
        return Value::Nil;
    }
    names.sort_by_key(|n| {
        super::strfn::CHARSET_ID_ORDER
            .iter()
            .position(|o| o == n)
            .unwrap_or(usize::MAX)
    });
    Value::list(names.into_iter().map(|n| symv(i, n)).collect())
}

fn f_find_charset_string(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    match &a[0] {
        Value::Str(s) => {
            let text = s.borrow().clone();
            Ok(find_charset_of_chars(i, text.chars()))
        }
        _ => Err(i.wrong_type_mut("stringp", &a[0])),
    }
}

fn f_find_charset_region(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let bid = i.current_buffer;
    let text = i
        .buffers
        .get(bid)
        .map(|b| b.borrow().text.text())
        .unwrap_or_default();
    let (lo, hi) = match (
        a.first().and_then(|v| v.int()),
        a.get(1).and_then(|v| v.int()),
    ) {
        (Some(s), Some(e)) => ((s - 1).max(0) as usize, (e - 1).max(0) as usize),
        _ => (0, text.chars().count()),
    };
    let sub: String = text.chars().skip(lo).take(hi.saturating_sub(lo)).collect();
    Ok(find_charset_of_chars(i, sub.chars()))
}

fn f_char_resolve_modifiers(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    match &a[0] {
        // GNU returns CHAR with resolvable modifiers folded into the
        // character code; unresolvable ones (like meta on a letter)
        // stay, so the common cases are identity.
        Value::Int(_) => Ok(a[0].clone()),
        _ => Err(i.wrong_type_mut("characterp", &a[0])),
    }
}

// ---------- char-code properties ----------

fn prop_table<'a>(i: &'a mut Interp, name: &str) -> &'a mut Vec<(i64, Value)> {
    if i.char_code_props.iter().all(|(n, _)| n != name) {
        i.char_code_props.push((name.to_string(), Vec::new()));
    }
    &mut i
        .char_code_props
        .iter_mut()
        .find(|(n, _)| n == name)
        .unwrap()
        .1
}

fn f_define_char_code_property(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let sid = want_sym(i, &a[0])?;
    let name = i.symbol_name(sid);
    match &a[1] {
        v if char_table_vec(v).is_some() => {
            // Keep the char-table as the property's backing store.
            prop_table(i, &name);
            i.char_code_prop_tables.retain(|(n, _)| n != &name);
            i.char_code_prop_tables.push((name, v.clone()));
            Ok(Value::Nil)
        }
        Value::Str(_) => {
            // File-based property table. GNU keeps the file name as the
            // backing; any put/get then signals char-table-p — mirror that
            // by registering the string as the backing table.
            i.char_code_prop_tables.retain(|(n, _)| n != &name);
            i.char_code_prop_tables.push((name, a[1].clone()));
            Ok(Value::Nil)
        }
        other => Err(i.error(format!(
            "Not a char-table nor a file name: {}",
            i.print_to_string(other)
        ))),
    }
}

/// ASCII subset of Unicode general-category, matching GNU's table.
fn ascii_general_category(c: u32) -> Option<&'static str> {
    let s = match c {
        _ if c > 0xFF => return None,
        0x00..=0x1F | 0x7F => "Cc",
        32 => "Zs",
        48..=57 => "Nd",
        65..=90 => "Lu",
        97..=122 => "Ll",
        40 | 91 | 123 => "Ps",
        41 | 93 | 125 => "Pe",
        45 => "Pd",
        95 => "Pc",
        94 | 96 => "Sk",
        36 => "Sc",
        43 | 60 | 61 | 62 | 124 | 126 => "Sm",
        0x80..=0x9F => "Cc",
        _ if c < 0x80 => "Po",
        _ => return None,
    };
    Some(s)
}

fn get_char_prop(i: &mut Interp, ch: i64, prop: &str) -> Value {
    if prop == "general-category" {
        if let Some(cat) = ascii_general_category(ch as u32) {
            return symv(i, cat);
        }
        return Value::Nil;
    }
    if let Some(e) = i.char_code_props.iter().find(|(n, _)| n == prop) {
        if let Some((_, v)) = e.1.iter().find(|(c, _)| *c == ch) {
            return v.clone();
        }
    }
    // Check a char-table backing store.
    if let Some((_, tbl)) = i.char_code_prop_tables.iter().find(|(n, _)| n == prop) {
        if let Some(v) = char_table_vec(tbl) {
            let vv = v.borrow();
            if let Some(val) = vv.get(ch.max(0) as usize) {
                if !val.is_nil() {
                    return val.clone();
                }
            }
        }
    }
    builtin_char_prop(i, ch, prop)
}

/// GNU signals `wrong-type-argument (char-table-p FILE)` when a property
/// registered with a file name is used for lookup or storage.
fn check_prop_backing(i: &mut Interp, prop: &str) -> Result<(), Flow> {
    let bad = i
        .char_code_prop_tables
        .iter()
        .find(|(n, _)| n == prop)
        .filter(|(_, tbl)| matches!(tbl, Value::Str(_)))
        .map(|(_, tbl)| tbl.clone());
    if let Some(tbl) = bad {
        return Err(i.wrong_type_mut("char-table-p", &tbl));
    }
    Ok(())
}

/// GNU's built-in property defaults (subset covering ASCII).
fn builtin_char_prop(i: &mut Interp, ch: i64, prop: &str) -> Value {
    let u = ch as u32;
    let c = u8::try_from(u).ok();
    match prop {
        "bidi-class" => {
            let cls = match c {
                Some(b'0'..=b'9') => "EN",
                Some(b'A'..=b'Z' | b'a'..=b'z') => "L",
                Some(b' ' | b'\t') => "WS",
                Some(b'\n') => "B",
                Some(_) => "ON",
                None => return Value::Nil,
            };
            symv(i, cls)
        }
        "decimal-digit-value" | "numeric-value" => match c {
            Some(d @ b'0'..=b'9') => Value::Int((d - b'0') as i128),
            _ => Value::Nil,
        },
        "mirrored" => {
            let m = match c {
                Some(b'(' | b'[' | b'{' | b'<' | b')' | b']' | b'}' | b'>') => "Y",
                Some(_) => "N",
                None => return Value::Nil,
            };
            symv(i, m)
        }
        _ => Value::Nil,
    }
}

fn f_get_char_code_property(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let ch = match &a[0] {
        Value::Int(n) => *n as i64,
        _ => return Err(i.wrong_type_mut("characterp", &a[0])),
    };
    let pid2 = want_sym(i, &a[1])?;
    let prop = i.symbol_name(pid2);
    check_prop_backing(i, &prop)?;
    Ok(get_char_prop(i, ch, &prop))
}

fn f_put_char_code_property(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let ch = match &a[0] {
        Value::Int(n) => *n as i64,
        _ => return Err(i.wrong_type_mut("characterp", &a[0])),
    };
    let pid2 = want_sym(i, &a[1])?;
    let prop = i.symbol_name(pid2);
    check_prop_backing(i, &prop)?;
    let tbl = prop_table(i, &prop);
    if let Some(e) = tbl.iter_mut().find(|(c, _)| *c == ch) {
        e.1 = a[2].clone();
    } else {
        tbl.push((ch, a[2].clone()));
    }
    Ok(a[2].clone())
}

fn f_char_code_property_description(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let pid2 = want_sym(i, &a[0])?;
    let vid = want_sym(i, &a[1])?;
    let prop = i.symbol_name(pid2);
    let val = i.symbol_name(vid);
    check_prop_backing(i, &prop)?;
    if prop == "general-category" {
        let desc = match val.as_str() {
            "Lu" => "Letter, Uppercase",
            "Ll" => "Letter, Lowercase",
            "Lt" => "Letter, Titlecase",
            "Lm" => "Letter, Modifier",
            "Lo" => "Letter, Other",
            "Mn" => "Mark, Nonspacing",
            "Mc" => "Mark, Spacing Combining",
            "Me" => "Mark, Enclosing",
            "Nd" => "Number, Decimal Digit",
            "Nl" => "Number, Letter",
            "No" => "Number, Other",
            "Pc" => "Punctuation, Connector",
            "Pd" => "Punctuation, Dash",
            "Ps" => "Punctuation, Open",
            "Pe" => "Punctuation, Close",
            "Pi" => "Punctuation, Initial quote",
            "Pf" => "Punctuation, Final quote",
            "Po" => "Punctuation, Other",
            "Sm" => "Symbol, Math",
            "Sc" => "Symbol, Currency",
            "Sk" => "Symbol, Modifier",
            "So" => "Symbol, Other",
            "Zs" => "Separator, Space",
            "Zl" => "Separator, Line",
            "Zp" => "Separator, Paragraph",
            "Cc" => "Other, Control",
            "Cf" => "Other, Format",
            "Cs" => "Other, Surrogate",
            "Co" => "Other, Private Use",
            "Cn" => "Other, Not Assigned",
            _ => return Ok(Value::Nil),
        };
        return Ok(Value::string(desc));
    }
    if prop == "bidi-class" {
        let desc = match val.as_str() {
            "L" => "Left-to-Right",
            "R" => "Right-to-Left",
            "AL" => "Right-to-Left Arabic",
            "EN" => "European Number",
            "ES" => "European Number Separator",
            "ET" => "European Number Terminator",
            "AN" => "Arabic Number",
            "CS" => "Common Number Separator",
            "NSM" => "Non-Spacing Mark",
            "BN" => "Boundary Neutral",
            "B" => "Paragraph Separator",
            "S" => "Segment Separator",
            "WS" => "Whitespace",
            "ON" => "Other Neutrals",
            "LRE" => "Left-to-Right Embedding",
            "LRO" => "Left-to-Right Override",
            "RLE" => "Right-to-Left Embedding",
            "RLO" => "Right-to-Left Override",
            "PDF" => "Pop Directional Format",
            "LRI" => "Left-to-Right Isolate",
            "RLI" => "Right-to-Left Isolate",
            "FSI" => "First Strong Isolate",
            "PDI" => "Pop Directional Isolate",
            _ => return Ok(Value::Nil),
        };
        return Ok(Value::string(desc));
    }
    Ok(Value::Nil)
}

// ---------- translation tables ----------

fn f_make_translation_table(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    // (make-translation-table &optional arg1 arg2) — char-table-like record.
    let tag = symv(i, "char-table");
    let subtype = symv(i, "translation-table");
    let mut slots = vec![Value::Nil; 256];
    for arg in &a {
        arg.each_car(|pair| {
            if let Value::Cons(c) = pair {
                let (k, v) = {
                    let b = c.borrow();
                    (b.car.clone(), b.cdr.clone())
                };
                if let (Value::Int(k), Value::Int(vv)) = (k, v) {
                    if (0..256).contains(&k) {
                        slots[k as usize] = Value::Int(vv);
                    }
                }
            }
        });
    }
    Ok(Value::Record(Rc::new(RefCell::new(vec![
        tag,
        subtype,
        Value::Vec(Rc::new(RefCell::new(slots))),
    ]))))
}

fn f_define_translation_table(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    // (define-translation-table SYMBOL &rest args) — store table on SYMBOL's
    // 'translation-table property.
    let id = want_sym(i, &a[0])?;
    let tbl = f_make_translation_table(i, a[1..].to_vec())?;
    let prop = i.intern("translation-table");
    i.put_prop(id, prop, tbl);
    Ok(Value::Nil)
}

fn f_make_translation_table_from_vector(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    // (make-translation-table-from-vector VEC) — VEC is a 256-element array
    // mapping unibyte codes to multibyte chars.
    let slots: Vec<Value> = match &a[0] {
        Value::Vec(v) => {
            let v = v.borrow();
            if v.len() != 256 {
                let s = i.intern("args-out-of-range");
                return Err(i.signal_data(s, vec![a[0].clone(), Value::Int(v.len() as i128)]));
            }
            v.clone()
        }
        _ => return Err(i.wrong_type_mut("vectorp", &a[0])),
    };
    let tag = symv(i, "char-table");
    let subtype = symv(i, "translation-table");
    Ok(Value::Record(Rc::new(RefCell::new(vec![
        tag,
        subtype,
        Value::Vec(Rc::new(RefCell::new(slots))),
    ]))))
}

fn f_set_translation_table(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let _ = &a;
    let _ = i;
    Ok(Value::Nil)
}

// ---------- coding systems ----------

fn f_coding_system_type(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let name = match &a[0] {
        Value::Sym(s) => i.symbol_name(*s),
        Value::Nil => "undecided".to_string(),
        _ => return Err(i.wrong_type_mut("coding-system-p", &a[0])),
    };
    let base = name
        .strip_suffix("-unix")
        .or_else(|| name.strip_suffix("-dos"))
        .or_else(|| name.strip_suffix("-mac"))
        .unwrap_or(&name);
    // GNU signals `coding-system-error` for undefined coding systems.
    if !matches!(&a[0], Value::Nil) && coding_known(i, &a[0]).is_none() {
        let s = i.intern("coding-system-error");
        return Err(i.signal_data(s, vec![a[0].clone()]));
    }
    // GNU's coding-system types: charset is the default for the
    // single-byte/dos codepage families; iso-2022 covers euc/cjk escapes.
    let ty = match base {
        "no-conversion" | "no-conversion-multibyte" | "raw-text" | "binary" => "raw-text",
        "undecided" | "prefer-utf-8" => "undecided",
        "emacs-mule" => "emacs-mule",
        s if s.starts_with("utf-16") => "utf-16",
        s if s.starts_with("utf-8")
            || s.starts_with("utf-7")
            || s == "mule-utf-8"
            || s == "hz"
            || s == "chinese-hz" =>
        {
            "utf-8"
        }
        s if s.starts_with("sjis")
            || s.starts_with("shift-jis")
            || s.starts_with("shift_jis")
            || s.starts_with("japanese-shift") =>
        {
            "shift-jis"
        }
        s if s.contains("big5") => "big5",
        s if s.starts_with("iso-2022")
            || s.starts_with("euc-")
            || s.starts_with("eucjp")
            || s == "gb2312"
            || s == "cn-gb-2312"
            || s == "hz-gb-2312"
            || s.starts_with("compound-text")
            || s.starts_with("ctext")
            || s.starts_with("x-ctext")
            || s == "old-jis"
            || s == "junet"
            || s == "iso-safe"
            || s == "chinese-iso-7bit"
            || s == "chinese-iso-8bit"
            || s.starts_with("japanese-iso-")
            || s.starts_with("korean-iso-") =>
        {
            "iso-2022"
        }
        _ => "charset",
    };
    Ok(symv(i, ty))
}

fn f_coding_system_charset_list(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let name = match &a[0] {
        Value::Sym(s) => i.symbol_name(*s),
        _ => return Err(i.wrong_type_mut("coding-system-p", &a[0])),
    };
    if coding_known(i, &a[0]).is_none() {
        // GNU signals coding-system-error on an undefined coding system.
        let cs = i.intern("coding-system-error");
        return Err(i.signal_data(cs, vec![a[0].clone()]));
    }
    let list: Vec<&str> = if name.starts_with("utf-8") || name.starts_with("undecided") {
        vec!["unicode"]
    } else if name.starts_with("iso-8859")
        || name.starts_with("iso-latin")
        || name.starts_with("latin")
    {
        vec!["iso-8859-1"]
    } else {
        vec!["ascii"]
    };
    Ok(Value::list(list.into_iter().map(|n| symv(i, n)).collect()))
}

fn f_set_coding_system_priority(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let _ = &a;
    let _ = i;
    Ok(Value::Nil)
}

fn f_set_keyboard_coding_system_internal(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    // GNU: validates the coding system but the variable keeps its
    // terminal default in batch.
    let _ = want_sym(i, &a[0])?;
    Ok(Value::Nil)
}

fn f_find_coding_systems_region_internal(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    // GNU validates START/END against the buffer first.
    let s = a[0].int().unwrap_or(1).max(1);
    let e = a[1].int().unwrap_or(s).max(1);
    let max = i
        .buffers
        .get(i.current_buffer)
        .map(|b| b.borrow().text_len() as i128 + 1)
        .unwrap_or(1);
    if s > max || e > max || s > e {
        let sid = i.intern("args-out-of-range");
        return Err(i.signal(sid, Value::list(vec![a[0].clone(), a[1].clone()])));
    }
    // GNU returns t for regions any coding system can encode.
    Ok(Value::t())
}

// ---------- charset ids / priority / unicode property tables ----------

/// GNU's built-in charset ids (init order in charset.c); user-defined
/// charsets get ids >= 40 by registration order.
fn charset_id(i: &Interp, name: &str) -> i128 {
    match name {
        "ascii" => 0,
        "iso-8859-1" | "latin-iso8859-1" => 1,
        "unicode" | "ucs" => 2,
        "emacs" => 3,
        "eight-bit-control" => 4,
        "eight-bit-graphic" => 5,
        "eight-bit" => 6,
        "control-1" => 7,
        _ => i
            .charsets
            .iter()
            .position(|(n, _)| n == name)
            .map(|p| 40 + p as i128)
            .unwrap_or(-1),
    }
}

fn f_charset_id_internal(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    // GNU: nil/missing arg or unknown name → wrong-type-argument charsetp.
    let name = want_charset(i, &arg(&a, 0))?;
    Ok(Value::Int(charset_id(i, &name)))
}

fn f_charset_priority_list(i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    // Ordered by descending priority: unicode first, like GNU.
    let mut names: Vec<String> = vec!["unicode".to_string()];
    names.extend(
        BUILTIN_CHARSETS
            .iter()
            .filter(|n| **n != "unicode")
            .map(|n| n.to_string()),
    );
    for (n, _) in &i.charsets {
        if !names.contains(n) {
            names.push(n.clone());
        }
    }
    Ok(Value::list(
        names.iter().map(|n| symv(i, n)).collect(),
    ))
}

fn f_sort_charsets(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    // GNU sorts by ascending charset id.
    let mut items = super::want_list(i, &a[0])?;
    let mut keyed: Vec<(i128, Value)> = Vec::with_capacity(items.len());
    for v in items.drain(..) {
        let id = match &v {
            Value::Sym(s) => {
                let n = i.symbol_name(*s);
                if charset_defined(i, &n) {
                    charset_id(i, &canonical_charset(i, &n))
                } else {
                    i128::MAX
                }
            }
            _ => i128::MAX,
        };
        keyed.push((id, v));
    }
    keyed.sort_by_key(|(id, _)| *id);
    Ok(Value::list(keyed.into_iter().map(|(_, v)| v).collect()))
}

fn f_set_charset_priority(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    // GNU validates each arg is a charset.
    for v in &a {
        let _ = want_charset(i, v)?;
    }
    Ok(Value::Nil)
}

fn f_get_unused_iso_final_char(i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    Err(i.error("No unused ISO final char available"))
}

fn f_iso_charset(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    // (iso-charset CODING-SYSTEM DIMENSION FINAL-CHAR)
    if coding_known(i, &a[0]).is_none() {
        return Err(i.wrong_type_mut("coding-system-p", &a[0]));
    }
    Ok(Value::Nil)
}

fn f_map_charset_chars(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    // (map-charset-chars FUNCTION CHARSET &optional ARG FROM TO)
    let _ = want_charset(i, &a[1])?;
    for v in a.iter().skip(3) {
        if !v.is_nil() {
            match v {
                Value::Int(n) if *n >= 0 => {}
                _ => return Err(i.wrong_type_mut("wholenump", v)),
            }
        }
    }
    // Our charsets carry no per-char ranges to map over.
    Ok(Value::Nil)
}

fn f_declare_equiv_charset(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let _ = want_charset(i, &a[0])?;
    // GNU checks the FINAL-CHAR arg as fixnum, then as charset.
    match &a[3] {
        Value::Int(_) => {}
        other => return Err(i.wrong_type_mut("fixnump", other)),
    }
    let _ = want_charset(i, &a[3])?;
    Ok(Value::Nil)
}

fn f_clear_charset_maps(_i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    Ok(Value::Nil)
}

/// char-table for PROP, created lazily like GNU's on-demand tables.
fn unicode_prop_table(i: &mut Interp, prop: &str) -> Value {
    if let Some((_, t)) = i.char_code_prop_tables.iter().find(|(n, _)| n == prop) {
        return t.clone();
    }
    let vec = Value::Vec(Rc::new(RefCell::new(vec![Value::Nil; 256])));
    let t = Value::Record(Rc::new(RefCell::new(vec![
        Value::Sym(i.intern("char-table")),
        symv(i, "char-code-property-table"),
        vec,
    ])));
    i.char_code_prop_tables.push((prop.to_string(), t.clone()));
    t
}

fn f_unicode_property_table_internal(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let pid = want_sym(i, &a[0])?;
    let prop = i.symbol_name(pid);
    Ok(unicode_prop_table(i, &prop))
}

/// Property name a table was registered under (identity match).
fn unicode_table_prop(i: &Interp, tbl: &Value) -> Option<String> {
    i.char_code_prop_tables
        .iter()
        .find(|(_, t)| eq_values(t, tbl))
        .map(|(n, _)| n.clone())
}

fn f_get_unicode_property_internal(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    if !is_char_table(i, &a[0]) {
        return Err(i.wrong_type_mut("char-table-p", &a[0]));
    }
    let ch = match &a[1] {
        Value::Int(n) => *n as i64,
        _ => return Err(i.wrong_type_mut("characterp", &a[1])),
    };
    if let Some(prop) = unicode_table_prop(i, &a[0]) {
        // Route through get_char_prop so built-in defaults (e.g. ASCII
        // general-category) still show through unset slots.
        return Ok(get_char_prop(i, ch, &prop));
    }
    let v = char_table_vec(&a[0]).unwrap();
    Ok(v.borrow().get(ch.max(0) as usize).cloned().unwrap_or(Value::Nil))
}

fn f_put_unicode_property_internal(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    if !is_char_table(i, &a[0]) {
        return Err(i.wrong_type_mut("char-table-p", &a[0]));
    }
    let ch = match &a[1] {
        Value::Int(n) if *n >= 0 => *n as usize,
        _ => return Err(i.wrong_type_mut("characterp", &a[1])),
    };
    let v = char_table_vec(&a[0]).unwrap();
    let mut vv = v.borrow_mut();
    if ch >= vv.len() {
        vv.resize(ch + 1, Value::Nil);
    }
    vv[ch] = a[2].clone();
    Ok(Value::Nil)
}

pub(crate) static SUBRS: &[Subr] = &[
    S!("charsetp", 1, 1, f_charsetp, "t if OBJECT names a charset."),
    S!("define-charset", many 2, f_define_charset, "Define a new charset."),
    S!(
        "charset-plist",
        1,
        1,
        f_charset_plist,
        "Property list of CHARSET."
    ),
    S!(
        "set-charset-plist",
        2,
        2,
        f_set_charset_plist,
        "Set charset plist."
    ),
    S!(
        "get-charset-property",
        2,
        2,
        f_get_charset_property,
        "Get PROP of CHARSET."
    ),
    S!(
        "put-charset-property",
        3,
        3,
        f_put_charset_property,
        "Set PROP of CHARSET."
    ),
    S!(
        "define-charset-alias",
        2,
        2,
        f_define_charset_alias,
        "Make ALIAS refer to CHARSET."
    ),
    S!(
        "unify-charset",
        1,
        3,
        f_unify_charset,
        "Unify CHARSET per MAP."
    ),
    S!(
        "charset-after",
        0,
        1,
        f_charset_after,
        "Charset of char after POS."
    ),
    S!(
        "find-charset-string",
        1,
        1,
        f_find_charset_string,
        "Charsets needed for STRING."
    ),
    S!(
        "find-charset-region",
        0,
        2,
        f_find_charset_region,
        "Charsets needed for region."
    ),
    S!(
        "char-resolve-modifiers",
        1,
        1,
        f_char_resolve_modifiers,
        "Base char of CHAR with modifiers."
    ),
    S!(
        "define-char-code-property",
        2,
        2,
        f_define_char_code_property,
        "Define a char-code property."
    ),
    S!(
        "get-char-code-property",
        2,
        2,
        f_get_char_code_property,
        "Property PROP of CHAR."
    ),
    S!(
        "put-char-code-property",
        3,
        3,
        f_put_char_code_property,
        "Set PROP of CHAR to VALUE."
    ),
    S!(
        "char-code-property-description",
        2,
        2,
        f_char_code_property_description,
        "Description of property value."
    ),
    S!(
        "make-translation-table",
        0,
        2,
        f_make_translation_table,
        "Make a translation table."
    ),
    S!(
        "define-translation-table",
        1,
        3,
        f_define_translation_table,
        "Define SYMBOL as translation table."
    ),
    S!(
        "make-translation-table-from-vector",
        1,
        1,
        f_make_translation_table_from_vector,
        "Make a translation table from a 256-vector."
    ),
    S!(
        "set-translation-table",
        1,
        3,
        f_set_translation_table,
        "Set current translation table."
    ),
    S!(
        "coding-system-type",
        1,
        1,
        f_coding_system_type,
        "Base type of CODING-SYSTEM."
    ),
    S!(
        "coding-system-charset-list",
        1,
        1,
        f_coding_system_charset_list,
        "Charsets of CODING-SYSTEM."
    ),
    S!("set-coding-system-priority", many 0, f_set_coding_system_priority, "Set coding system priority."),
    S!(
        "set-keyboard-coding-system-internal",
        1,
        1,
        f_set_keyboard_coding_system_internal,
        "Set keyboard coding system."
    ),
    S!(
        "find-coding-systems-region-internal",
        2,
        2,
        f_find_coding_systems_region_internal,
        "Coding systems covering region."
    ),
    S!(
        "charset-id-internal",
        0,
        1,
        f_charset_id_internal,
        "Internal: id of CHARSET."
    ),
    S!(
        "charset-priority-list",
        0,
        1,
        f_charset_priority_list,
        "Charsets in priority order."
    ),
    S!(
        "sort-charsets",
        1,
        1,
        f_sort_charsets,
        "Sort CHARSETS by id."
    ),
    S!(
        "set-charset-priority",
        many 1,
        f_set_charset_priority,
        "Set charset priority order."
    ),
    S!(
        "get-unused-iso-final-char",
        2,
        2,
        f_get_unused_iso_final_char,
        "Internal: unused ISO final char."
    ),
    S!(
        "iso-charset",
        3,
        3,
        f_iso_charset,
        "Internal: ISO charset for CODING-SYSTEM."
    ),
    S!(
        "map-charset-chars",
        2,
        5,
        f_map_charset_chars,
        "Call FUNCTION over CHARSET's ranges."
    ),
    S!(
        "declare-equiv-charset",
        4,
        4,
        f_declare_equiv_charset,
        "Declare equivalent charset."
    ),
    S!(
        "clear-charset-maps",
        0,
        0,
        f_clear_charset_maps,
        "Clear internal charset maps."
    ),
    S!(
        "unicode-property-table-internal",
        1,
        1,
        f_unicode_property_table_internal,
        "Char-table for unicode PROP."
    ),
    S!(
        "get-unicode-property-internal",
        2,
        2,
        f_get_unicode_property_internal,
        "PROP-TABLE value at CHAR."
    ),
    S!(
        "put-unicode-property-internal",
        3,
        3,
        f_put_unicode_property_internal,
        "Set PROP-TABLE value at CHAR."
    ),
    S!(
        "decode-big5-char",
        1,
        1,
        f_decode_big5_char,
        "Decode Big5 CODE to a character."
    ),
    S!(
        "encode-big5-char",
        1,
        1,
        f_encode_big5_char,
        "Encode CH to a Big5 code."
    ),
    S!(
        "decode-sjis-char",
        1,
        1,
        f_decode_sjis_char,
        "Decode Shift-JIS CODE to a character."
    ),
    S!(
        "encode-sjis-char",
        1,
        1,
        f_encode_sjis_char,
        "Encode CH to a Shift-JIS code."
    ),
    S!(
        "define-charset-internal",
        many 17,
        f_define_charset_internal,
        "Internal: define charset from attributes."
    ),
    S!(
        "define-coding-system-internal",
        many 13,
        f_define_coding_system_internal,
        "Internal: define coding system from attributes."
    ),
];

// ---------- CJK coders ----------

fn tbl_decode(t: &[(u32, u32)], code: u32) -> Option<i64> {
    t.binary_search_by_key(&code, |p| p.0)
        .ok()
        .map(|ix| t[ix].1 as i64)
}

fn tbl_encode(t: &[(u32, u32)], ucs: u32) -> Option<i64> {
    t.binary_search_by_key(&ucs, |p| p.0)
        .ok()
        .map(|ix| t[ix].1 as i64)
}

/// `decode-char CHARSET CODE` semantics: nil when CODE is outside the
/// charset's space or unmapped. Used by `f_decode_char` in strfn.rs.
pub(crate) fn decode_charset_code(name: &str, code: i64) -> Option<i64> {
    use crate::lisp::cjk_tables as cjk;
    if code < 0 {
        return None;
    }
    let u = code as u32;
    match name {
        "ascii" => (code < 0x80).then_some(code),
        "eight-bit" => (0x80..=0xff).contains(&code).then(|| 0x3fff00 + code),
        "iso-8859-1" | "latin-iso8859-1" | "eight-bit-graphic" | "eight-bit-control" => {
            (code < 0x100).then_some(code)
        }
        "unicode" | "ucs" => (code < 0x110000).then_some(code),
        "emacs" => (code < 0x400000).then_some(code),
        "big5" => Some(if code < 0x80 {
            code
        } else {
            return tbl_decode(cjk::BIG5_DECODE, u);
        }),
        "cp932" | "cp932-2-byte" => Some(if code < 0x80 {
            code
        } else if (0xa1..=0xdf).contains(&code) {
            code + 0xfec0
        } else {
            return tbl_decode(cjk::SJIS_DECODE, u);
        }),
        "katakana-sjis" => (0xa1..=0xdf).contains(&code).then(|| code + 0xfec0),
        "japanese-jisx0208" => tbl_decode(cjk::JISX0208_DECODE, u),
        "jisx0201" | "katakana-jisx0201" | "latin-jisx0201" => {
            tbl_decode(cjk::JISX0201_DECODE, u)
        }
        "chinese-big5-1" => tbl_decode(cjk::BIG5_1_DECODE, u),
        "chinese-big5-2" => tbl_decode(cjk::BIG5_2_DECODE, u),
        // Defined charsets we don't model: pass the code through (ASCII-safe).
        _ => Some(code),
    }
}

/// `encode-char CH CHARSET` semantics: nil when CH has no code in CHARSET.
pub(crate) fn encode_charset_code(name: &str, ch: i64) -> Option<i64> {
    use crate::lisp::cjk_tables as cjk;
    if !(0..0x400000).contains(&ch) {
        return None;
    }
    let u = ch as u32;
    match name {
        "ascii" => (ch < 0x80).then_some(ch),
        "eight-bit" => (0x3fff80..=0x3fffff).contains(&ch).then(|| ch - 0x3fff00),
        "iso-8859-1" | "latin-iso8859-1" | "eight-bit-graphic" | "eight-bit-control" => {
            (ch < 0x100).then_some(ch)
        }
        "unicode" | "ucs" | "emacs" => Some(ch),
        "big5" => Some(if ch < 0x80 {
            ch
        } else {
            return tbl_encode(cjk::BIG5_ENCODE, u);
        }),
        "cp932" | "cp932-2-byte" => Some(if ch < 0x80 {
            ch
        } else if (0xff61..=0xff9f).contains(&ch) {
            ch - 0xfec0
        } else {
            return tbl_encode(cjk::SJIS_ENCODE, u);
        }),
        "katakana-sjis" => (0xff61..=0xff9f).contains(&ch).then(|| ch - 0xfec0),
        "japanese-jisx0208" => tbl_encode(cjk::JISX0208_ENCODE, u),
        "jisx0201" | "katakana-jisx0201" | "latin-jisx0201" => {
            tbl_encode(cjk::JISX0201_ENCODE, u)
        }
        "chinese-big5-1" => tbl_encode(cjk::BIG5_1_ENCODE, u),
        "chinese-big5-2" => tbl_encode(cjk::BIG5_2_ENCODE, u),
        _ => Some(ch),
    }
}

fn want_wholenum_c(i: &mut Interp, v: &Value) -> Result<i64, Flow> {
    match v {
        Value::Int(n) if *n >= 0 => Ok(*n as i64),
        _ => Err(i.wrong_type_mut("wholenump", v)),
    }
}

fn want_char_c(i: &mut Interp, v: &Value) -> Result<i64, Flow> {
    match v {
        Value::Int(n) if (0..0x400000).contains(n) => Ok(*n as i64),
        _ => Err(i.wrong_type_mut("characterp", v)),
    }
}

fn invalid_code(i: &mut Interp, code: i64) -> Flow {
    i.error(format!("Invalid code: {code}"))
}

fn f_decode_big5_char(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    use crate::lisp::cjk_tables as cjk;
    let code = want_wholenum_c(i, &a[0])?;
    if code < 0x80 {
        return Ok(Value::Int(code.into()));
    }
    match tbl_decode(cjk::BIG5_DECODE, code as u32) {
        Some(u) => Ok(Value::Int(u.into())),
        None => Err(invalid_code(i, code)),
    }
}

fn f_encode_big5_char(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    use crate::lisp::cjk_tables as cjk;
    let ch = want_char_c(i, &a[0])?;
    if ch < 0x80 {
        return Ok(Value::Int(ch.into()));
    }
    match tbl_encode(cjk::BIG5_ENCODE, ch as u32) {
        Some(c) => Ok(Value::Int(c.into())),
        None => Err(i.error(format!("Cannot encode character: {ch}"))),
    }
}

fn f_decode_sjis_char(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    use crate::lisp::cjk_tables as cjk;
    let code = want_wholenum_c(i, &a[0])?;
    if code < 0x80 {
        return Ok(Value::Int(code.into()));
    }
    if (0xa1..=0xdf).contains(&code) {
        return Ok(Value::Int((code + 0xfec0).into()));
    }
    match tbl_decode(cjk::SJIS_DECODE, code as u32) {
        Some(u) => Ok(Value::Int(u.into())),
        None => Err(invalid_code(i, code)),
    }
}

fn f_encode_sjis_char(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    use crate::lisp::cjk_tables as cjk;
    let ch = want_char_c(i, &a[0])?;
    if ch < 0x80 {
        return Ok(Value::Int(ch.into()));
    }
    // GNU's `sjis` charset encodes halfwidth kana at 0x709F+offset.
    if (0xff61..=0xff9f).contains(&ch) {
        return Ok(Value::Int((ch - 0xff61 + 0x709f).into()));
    }
    match tbl_encode(cjk::SJIS_ENCODE, ch as u32) {
        Some(c) => Ok(Value::Int(c.into())),
        None => Err(i.error(format!("Cannot encode character: {ch}"))),
    }
}

fn f_define_charset_internal(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    // (define-charset-internal NAME DIMENSION CODE-SPACE MIN-CHAR MAX-CHAR
    //  ISO-FINAL-CHAR ISO-GRAPHIC-PLANE ASCII-COMPATIBLE-P SUPPLEMENT-P
    //  INVALID-CODE CODE-OFFSET MAP SUBSET-PARENTS SUPPLEMENT-CHARSET
    //  UNIFY-MAP UNICODES &rest)
    let sid = want_sym(i, &a[0])?;
    let name = i.symbol_name(sid);
    if charset_entry(i, &name).is_none() {
        i.charsets.push((name, Value::Nil));
    }
    Ok(Value::Nil)
}

fn f_define_coding_system_internal(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    // (define-coding-system-internal NAME MNEMONIC CODING-TYPE CHARSET-LIST
    //  ...13+ attrs) — register the name so `coding-system-p` sees it.
    let sid = want_sym(i, &a[0])?;
    let name = i.symbol_name(sid);
    if !i.extra_coding_systems.iter().any(|n| *n == name) {
        i.extra_coding_systems.push(name);
    }
    Ok(Value::Nil)
}
