//! Charsets, char-code properties, and coding-system helpers.
//!
//! GNU Emacs carries a full Unicode/charset machinery; we keep a compact
//! model that matches the observable Lisp API: charset objects identified
//! by name with plists, char-code property tables (general-category
//! provided for ASCII), and coding-system type resolution.

use std::cell::RefCell;
use std::rc::Rc;

use super::misc::{coding_known, is_char_table};
use super::{S, arg, eq_values, misc};
use crate::lisp::error::Flow;
use crate::lisp::value::{Subr, Value, lisp_char_code};
use crate::lisp::{EvalResult, Interp};

fn want_sym(i: &mut Interp, v: &Value) -> Result<u32, Flow> {
    i.sym_id(v).ok_or_else(|| i.wrong_type_mut("symbolp", v))
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

pub(crate) fn charset_defined(i: &Interp, name: &str) -> bool {
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

fn f_charsetp(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let name = match &a[0] {
        Value::Sym(s) => i.symbol_name(*s),
        _ => return Ok(Value::Nil),
    };
    Ok(Value::from_bool(charset_defined(i, &name)))
}

fn f_define_charset(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU mule.el `define-charset' (the defun whose calls land here
    // under our subr-shadowed cell): NAME is CHECK_SYMBOL; PROPS is a
    // plist — a dotted tail signals `plistp' (GNU's plist-put).
    let sid = want_sym(i, &a[0])?;
    let name = i.symbol_name(sid);
    let _ = name;
    let docstring = a.get(1).cloned().unwrap_or(Value::Nil);
    let mut props: Vec<Value> = if a.len() > 2 {
        a[2..].to_vec()
    } else {
        Vec::new()
    };
    if props.len() % 2 == 1 {
        return Err(i.wrong_type_mut("plistp", &Value::list(props.clone())));
    }
    let prop_ix = |i: &Interp, props: &[Value], key: &str| -> Option<usize> {
        props
            .chunks_exact(2)
            .position(|kv| matches!(&kv[0], Value::Sym(s) if i.symbol_name(*s) == key))
            .map(|ix| ix * 2 + 1)
    };
    let attr = |i: &Interp, props: &[Value], key: &str| -> Option<Value> {
        prop_ix(i, props, key).map(|ix| props[ix].clone())
    };
    // :dimension defaults from :code-space (len/2, default 4).
    if attr(i, &props, ":dimension").is_none() {
        let dim = match attr(i, &props, ":code-space") {
            Some(Value::Vec(v)) => (v.borrow().len() / 2) as i128,
            Some(other) => {
                if !matches!(other, Value::Cons(_) | Value::Nil | Value::Str(_)) {
                    return Err(i.wrong_type_mut("sequencep", &other));
                }
                0
            }
            None => 4,
        };
        props.push(symv(i, ":dimension"));
        props.push(Value::Int(dim));
    }
    // :code-space defaults to an 8-vector [0 FF] × dimension.
    if attr(i, &props, ":code-space").is_none() {
        let dim = attr(i, &props, ":dimension")
            .and_then(|v| v.int())
            .unwrap_or(4)
            .clamp(0, 4) as usize;
        let mut cs = Vec::with_capacity(8);
        for d in 0..4 {
            cs.push(Value::Int(0));
            cs.push(Value::Int(if d < dim { 0xFF } else { 0 }));
        }
        props.push(symv(i, ":code-space"));
        props.push(Value::Vec(Rc::new(RefCell::new(cs))));
    }
    // Pad :code-space to 8 elements (GNU vconcats zeros).
    if let Some(ix) = prop_ix(i, &props, ":code-space") {
        if let Value::Vec(v) = &props[ix] {
            let mut vv = v.borrow_mut();
            while vv.len() < 8 {
                vv.push(Value::Int(0));
            }
        }
    }
    // Assemble define-charset-internal's 17-arg call: the fixed attr
    // order plus PLIST = (:name NAME :docstring DOC . PROPS).
    let keys = [
        ":dimension",
        ":code-space",
        ":min-code",
        ":max-code",
        ":iso-final-char",
        ":iso-revision-number",
        ":emacs-mule-id",
        ":ascii-compatible-p",
        ":supplementary-p",
        ":invalid-code",
        ":code-offset",
        ":map",
        ":subset",
        ":superset",
        ":unify-map",
    ];
    let mut argv: Vec<Value> = Vec::with_capacity(17);
    argv.push(Value::Sym(sid));
    for k in keys {
        argv.push(attr(i, &props, k).unwrap_or(Value::Nil));
    }
    let mut plist_items = vec![
        symv(i, ":name"),
        Value::Sym(sid),
        symv(i, ":docstring"),
        docstring,
    ];
    plist_items.extend(props.iter().cloned());
    argv.push(Value::list(plist_items));
    define_charset_internal_impl(i, &argv)
}

fn f_charset_plist(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let name = want_charset(i, &a[0])?;
    Ok(effective_charset_plist(i, &name))
}

/// GNU's synthesized attribute plist for the C builtins
/// (`define_charset_internal' in syms_of_charset): `(:name N
/// :dimension D :code-space V :iso-final-char F :emacs-mule-id M
/// :ascii-compatible-p A :code-offset O)'.  `put-charset-property'
/// values land on top of this at runtime.
fn builtin_charset_plist(i: &mut Interp, name: &str) -> Option<Value> {
    // (dimension, LSB-first [code-space pairs], iso-final, mule-id,
    //  ascii-compat, code-offset)
    let (dim, space, fin, mule, compat, off): (i64, [(i64, i64); 4], i64, i64, bool, i64) =
        match name {
            "ascii" => (1, [(0, 127), (0, 0), (0, 0), (0, 0)], 66, 0, true, 0),
            "iso-8859-1" => (1, [(0, 255), (0, 0), (0, 0), (0, 0)], -1, -1, true, 0),
            "unicode" | "ucs" => (3, [(0, 255), (0, 255), (0, 16), (0, 0)], -1, -1, true, 0),
            "emacs" => (3, [(0, 255), (0, 255), (0, 63), (0, 0)], -1, -1, true, 0),
            "eight-bit" => (
                1,
                [(128, 255), (0, 0), (0, 0), (0, 0)],
                -1,
                -1,
                false,
                0x3FFF80,
            ),
            _ => return None,
        };
    let mut spacev = Vec::with_capacity(8);
    for (lo, hi) in space {
        spacev.push(Value::Int(lo.into()));
        spacev.push(Value::Int(hi.into()));
    }
    let kv = vec![
        symv(i, ":name"),
        symv(i, name),
        symv(i, ":dimension"),
        Value::Int(dim.into()),
        symv(i, ":code-space"),
        Value::Vec(Rc::new(RefCell::new(spacev))),
        symv(i, ":iso-final-char"),
        if fin < 0 {
            Value::Nil
        } else {
            Value::Int(fin.into())
        },
        symv(i, ":emacs-mule-id"),
        if mule < 0 {
            Value::Nil
        } else {
            Value::Int(mule.into())
        },
        symv(i, ":ascii-compatible-p"),
        Value::from_bool(compat),
        symv(i, ":code-offset"),
        Value::Int(off.into()),
    ];
    Some(Value::list(kv))
}

/// The effective plist of CHARSET: GNU's synthesized attribute plist
/// for the C builtins overlaid with every `put-charset-property' value
/// stored later, in the stored list's order.
fn effective_charset_plist(i: &mut Interp, name: &str) -> Value {
    let stored = charset_entry(i, name)
        .map(|e| e.1.clone())
        .unwrap_or(Value::Nil);
    match builtin_charset_plist(i, name) {
        Some(base) => {
            let mut merged = base.list_to_vec().unwrap_or_default();
            let mut cur = stored;
            while let Value::Cons(c) = cur {
                let (k, rest) = {
                    let b = c.borrow();
                    (b.car.clone(), b.cdr.clone())
                };
                let Value::Cons(v) = &rest else { break };
                let (val, rest2) = {
                    let b = v.borrow();
                    (b.car.clone(), b.cdr.clone())
                };
                let mut replaced = false;
                for ix in (0..merged.len()).step_by(2) {
                    if eq_values(&merged[ix], &k) {
                        merged[ix + 1] = val.clone();
                        replaced = true;
                        break;
                    }
                }
                if !replaced {
                    merged.push(k);
                    merged.push(val);
                }
                cur = rest2;
            }
            Value::list(merged)
        }
        None => stored,
    }
}

fn f_set_charset_plist(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let name = want_charset(i, &a[0])?;
    ensure_charset(i, &name);
    if let Some(e) = i.charsets.iter_mut().find(|(n, _)| *n == name) {
        e.1 = a[1].clone();
    }
    invalidate_char_charset_cache(i);
    Ok(Value::Nil)
}

fn f_get_charset_property(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let name = want_charset(i, &a[0])?;
    let prop = want_sym(i, &a[1])?;
    let plist = effective_charset_plist(i, &name);
    Ok(crate::lisp::eval::plist_get(&plist, prop))
}

fn f_put_charset_property(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let name = want_charset(i, &a[0])?;
    let prop = want_sym(i, &a[1])?;
    ensure_charset(i, &name);
    if let Some(e) = i.charsets.iter_mut().find(|(n, _)| *n == name) {
        e.1 = crate::lisp::eval::plist_put(&e.1, prop, a[2].clone());
    }
    invalidate_char_charset_cache(i);
    Ok(a[2].clone())
}

fn f_define_charset_alias(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU Fdefine_charset_alias: CHARSET (arg 2) is validated first via
    // CHECK_CHARSET_GET_ATTR — any non-charset object signals
    // `charsetp'.  ALIAS is a bare hash-table key: Fputhash accepts any
    // object, so non-symbol aliases register nothing and return nil.
    let target = want_charset(i, &a[1])?;
    if let Value::Sym(aid) = &a[0] {
        let alias = i.symbol_name(*aid);
        i.charset_aliases.push((alias, target));
        // GNU Fdefine_charset_alias: `Vcharset_list = Fcons (alias,
        // Vcharset_list)' — aliases prepend just like real charsets
        // (that's how `ucs' enters the list in mule-conf).
        let cl = i.intern("charset-list");
        let cur = i.symbol_value(cl);
        let cell = Value::cons(a[0].clone(), cur);
        let _ = i.set_symbol_default(cl, cell);
    }
    Ok(Value::Nil)
}

fn f_unify_charset(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // (charset &optional unify-map deunify) — GNU charset.c: only
    // OFFSET-method charsets whose :code-offset reaches the
    // emacs-mule area (>= #x110000) can be unified; everything else
    // (ascii, unicode, :map/:subset carriers) gets
    // "Can't unify charset: X".  Unifying installs the charset's
    // :unify-map table (parsed from etc/charsets/*.map) so later
    // `decode-char'/`map-charset-chars' produce real Unicode chars.
    let name = want_charset(i, &a[0])?;
    let deunify = a.get(2).is_some_and(|v| !v.is_nil());
    let offset = charset_entry(i, &name).and_then(|e| {
        let mut cur = e.1.clone();
        loop {
            let (k, v, rest) = match &cur {
                Value::Cons(c) => {
                    let b = c.borrow();
                    match &b.cdr {
                        Value::Cons(c2) => {
                            let b2 = c2.borrow();
                            (b.car.clone(), b2.car.clone(), b2.cdr.clone())
                        }
                        _ => break None,
                    }
                }
                _ => break None,
            };
            if matches!(&k, Value::Sym(s) if i.symbol_name(*s) == ":code-offset") {
                break Some(v);
            }
            cur = rest;
        }
    });
    let unified = !deunify && matches!(offset, Some(v) if v.int().is_some_and(|n| n >= 0x110000));
    if !unified {
        return Err(i.error(format!("Can't unify charset: {name}")));
    }
    let key = format!("charset:{name}");
    if deunify {
        i.charset_maps.borrow_mut().remove(&key);
        invalidate_char_charset_cache(i);
        return Ok(Value::Nil);
    }
    // GNU: UNIFY-MAP defaults to the charset's :unify-map property;
    // a string names etc/charsets/NAME.map, a vector is
    // [CODE-1 CHAR-1 CODE-2 CHAR-2 ...].
    let mapval = match a.get(1) {
        Some(v @ (Value::Str(_) | Value::Vec(_))) => Some(v.clone()),
        _ => None,
    }
    .or_else(|| {
        let plist = charset_entry(i, &name).map(|e| e.1.clone())?;
        let v = charset_prop(i, &plist, ":unify-map");
        (!v.is_nil()).then_some(v)
    });
    match mapval {
        Some(Value::Str(s)) => {
            if let Some(t) = charset_table(i, &s.borrow()) {
                i.charset_maps.borrow_mut().insert(key, t);
            }
        }
        Some(v @ Value::Vec(_)) => {
            i.charset_maps
                .borrow_mut()
                .insert(key, Rc::new(vec_map_table(&v)));
        }
        _ => {}
    }
    invalidate_char_charset_cache(i);
    Ok(Value::Nil)
}

/// GNU's `Vcharset_ordered_list' insertion for a freshly defined
/// charset: non-supplementary charsets go before the first
/// supplementary one, supplementary charsets append at the tail.
/// With the default (-Q) order the names are virtual, so runtime
/// definitions just append in `charsets_ordered'.
fn charset_ordered_insert(i: &mut Interp, name: &str) {
    let supp = charset_prop_of(i, name, ":supplementary-p").truthy();
    let mut st = i.charset_order.borrow_mut();
    if st.0.is_empty() {
        return;
    }
    if supp {
        st.0.push(name.to_string());
        return;
    }
    let pos =
        st.0.iter()
            .position(|n| {
                matches!(
                    n.as_str(),
                    "emacs" | "eight-bit" | "eight-bit-control" | "eight-bit-graphic"
                )
            })
            .unwrap_or(st.0.len());
    st.0.insert(pos, name.to_string());
}

/// `:map' property → table: a string names etc/charsets/NAME.map;
/// a vector is inline [CODE-1 CHAR-1 ...] pairs (cached per charset).
fn map_value_table(i: &Interp, name: &str, v: &Value) -> Option<Rc<CharsetTable>> {
    match v {
        Value::Str(s) => charset_table(i, &s.borrow()),
        Value::Vec(_) => {
            let key = format!("vecmap:{name}");
            let cached = i.charset_maps.borrow().get(&key).cloned();
            Some(match cached {
                Some(t) => t,
                None => {
                    let t = Rc::new(vec_map_table(v));
                    i.charset_maps.borrow_mut().insert(key, t.clone());
                    t
                }
            })
        }
        _ => None,
    }
}

/// `[CODE-1 CHAR-1 CODE-2 CHAR-2 ...]' → CharsetTable of 1-char runs.
fn vec_map_table(v: &Value) -> CharsetTable {
    let mut dec = Vec::new();
    let mut enc = Vec::new();
    if let Value::Vec(vec) = v {
        let vec = vec.borrow();
        for p in vec.chunks(2) {
            if let (Some(c), Some(ch)) = (p[0].int(), p.get(1).and_then(|x| x.int())) {
                dec.push((c as i64, c as i64, ch as i64));
                enc.push((ch as i64, ch as i64, c as i64));
            }
        }
    }
    dec.sort_unstable();
    enc.sort_unstable();
    CharsetTable { dec, enc }
}

fn f_charset_after(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU Fcharset_after: POS is a 1-based buffer position
    // (integer-or-marker-p, default point); out of range → nil.
    let pos: Option<i128> = match a.first() {
        None | Some(Value::Nil) => None,
        Some(Value::Int(n)) => Some(*n),
        Some(Value::Marker(m)) => Some(m.borrow().position as i128 + 1),
        Some(v) => return Err(i.wrong_type_mut("integer-or-marker-p", v)),
    };
    let bid = i.current_buffer;
    let ch = i.buffers.get(bid).and_then(|b| {
        let bb = b.borrow();
        let at = match pos {
            Some(p) if p >= 1 && (p as usize) <= bb.text_len() => (p - 1) as usize,
            Some(_) => return None,
            None => bb.point(),
        };
        bb.text.text().chars().nth(at)
    });
    Ok(match ch {
        None => Value::Nil,
        Some(c) => {
            let code = lisp_char_code(c) as i64;
            match char_charset(i, code, &[]) {
                Some(name) => symv(i, &name),
                None => Value::Nil,
            }
        }
    })
}

fn find_charset_of_chars(i: &mut Interp, chars: impl Iterator<Item = char>) -> Value {
    // GNU picks, for each char, the highest-priority charset containing
    // it (char-charset on the ordered list), then emits the covering
    // set sorted by charset id — ascii first, then the JIS tables.
    let mut names: Vec<String> = Vec::new();
    for c in chars {
        if let Some(cs) = char_charset(i, lisp_char_code(c) as i64, &[]) {
            if !names.contains(&cs) {
                names.push(cs);
            }
        }
    }
    if names.is_empty() {
        // GNU returns nil for an empty range/string.
        return Value::Nil;
    }
    names.sort_by_key(|n| charset_id(i, n));
    Value::list(names.iter().map(|n| symv(i, n)).collect())
}

fn f_find_charset_string(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    match &a[0] {
        Value::Str(s) => {
            let text = s.borrow().clone();
            Ok(find_charset_of_chars(i, text.chars()))
        }
        _ => Err(i.wrong_type_mut("stringp", &a[0])),
    }
}

fn f_find_charset_region(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let bid = i.current_buffer;
    let text = i
        .buffers
        .get(bid)
        .map(|b| b.borrow().text.text())
        .unwrap_or_default();
    // GNU Ffind_charset_region: BEG/END via integer-or-marker-p; nil
    // means the whole buffer; out-of-range or detached markers signal
    // `args-out-of-range (BUFFER BEG END)'.
    let zv1 = text.chars().count() as i128 + 1;
    let mut dec = |v: Option<&Value>| -> Result<Option<i128>, Flow> {
        match v {
            Some(v) if !v.is_nil() => match v {
                Value::Int(n) => Ok(Some(*n)),
                Value::Marker(m) if m.borrow().buffer.is_some() => {
                    Ok(Some(m.borrow().position as i128 + 1))
                }
                Value::Marker(_) => Err(i.error("Marker does not point anywhere")),
                _ => Err(i.wrong_type_mut("integer-or-marker-p", v)),
            },
            _ => Ok(None),
        }
    };
    let lo_opt = dec(a.first())?;
    let hi_opt = dec(a.get(1))?;
    let (lo1, hi1) = (lo_opt.unwrap_or(1), hi_opt.unwrap_or(zv1));
    if lo1 < 1 || lo1 > zv1 || hi1 < 1 || hi1 > zv1 || lo1 > hi1 {
        let buf = Value::Buffer(crate::buffer::primitives::cur(i));
        let a0 = a.first().cloned().unwrap_or(Value::Nil);
        let a1 = a.get(1).cloned().unwrap_or(Value::Nil);
        return Err(crate::editor::err_sym(
            i,
            "args-out-of-range",
            vec![buf, a0, a1],
        ));
    }
    let (lo, hi) = ((lo1 - 1) as usize, (hi1 - 1) as usize);
    let sub: String = text.chars().skip(lo).take(hi.saturating_sub(lo)).collect();
    Ok(find_charset_of_chars(i, sub.chars()))
}

fn f_char_resolve_modifiers(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    match &a[0] {
        // GNU CHECK_FIXNUM (any integer — out-of-range modifiers are
        // folded by char_resolve_modifier_mask), not CHECK_CHARACTER.
        Value::Int(_) => Ok(a[0].clone()),
        _ => Err(i.wrong_type_mut("fixnump", &a[0])),
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

/// GNU's `define-char-code-property': push `(NAME . TABLE-OR-FILE)'
/// onto `char-code-property-alist', replacing an existing entry's cdr.
fn register_prop_alist(i: &mut Interp, name: &str, backing: Value) {
    let entry = prop_alist_entry(i, name);
    if let Value::Cons(c) = entry {
        c.borrow_mut().cdr = backing;
    } else {
        let vid = i.intern("char-code-property-alist");
        let alist = i.symbol_value(vid);
        let cell = Value::cons(Value::Sym(i.intern(name)), backing);
        let _ = i.set_symbol(vid, Value::cons(cell, alist));
    }
}

fn f_define_char_code_property(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let sid = want_sym(i, &a[0])?;
    let name = i.symbol_name(sid);
    match &a[1] {
        v if is_char_table(i, v) => {
            // Keep the char-table as the property's backing store.
            prop_table(i, &name);
            i.char_code_prop_tables.retain(|(n, _)| n != &name);
            i.char_code_prop_tables.push((name.clone(), v.clone()));
            register_prop_alist(i, &name, v.clone());
            Ok(Value::Nil)
        }
        Value::Str(_) => {
            // File-based property table. GNU keeps the file name as the
            // backing; any put/get then signals char-table-p — mirror that
            // by registering the string as the backing table.
            i.char_code_prop_tables.retain(|(n, _)| n != &name);
            i.char_code_prop_tables.push((name.clone(), a[1].clone()));
            register_prop_alist(i, &name, a[1].clone());
            Ok(Value::Nil)
        }
        other => Err(i.error(format!(
            "Not a char-table nor a file name: {}",
            i.print_to_string(other)
        ))),
    }
}

/// Extra slot N of a char-table record (`char-table-extra-slot').
fn ct_extra(tbl: &Value, n: usize) -> Value {
    match tbl {
        Value::Record(r) => r.borrow().get(3 + n).cloned().unwrap_or(Value::Nil),
        _ => Value::Nil,
    }
}

/// GNU `uniprop_decode_value_run_length': an integer raw value is an
/// index into the vector stored in extra slot 4.
pub(crate) fn uniprop_decode_run_length(tbl: &Value, raw: Value) -> Value {
    if let (Value::Vec(vv), Value::Int(n)) = (ct_extra(tbl, 4), &raw) {
        let v = vv.borrow();
        if *n >= 0 && (*n as usize) < v.len() {
            return v[*n as usize].clone();
        }
    }
    raw
}

/// Decode the word-list compressed string S (first char is NUL) into
/// the 128 entries GNU's `unidata-word-list-compress' packed.  Each
/// stream char < 3 is a control code (0 = fresh list, 1 = new
/// DIFF-HEAD, 2 = reuse DIFF-HEAD); chars >= 3 index WORD_TABLE —
/// word symbols (name tables) or compatibility tags/char codes
/// (decomposition).  NAME_MODE returns symbol-name strings like
/// `unidata-get-name'; otherwise entries keep symbols and raw ints
/// like `unidata-get-decomposition'.
fn unidata_word_list_decode(
    i: &mut Interp,
    s: &str,
    word_table: &[Value],
    name_mode: bool,
) -> Vec<Value> {
    let chars: Vec<char> = s.chars().collect();
    let mut vec = vec![Value::Nil; 128];
    let mut idx = 0usize;
    let (mut j, mut diff_head) = (1usize, 0usize);
    let mut word: Vec<Value> = Vec::new();
    let mut tail: Vec<Value> = Vec::new();
    let mut last: Vec<Value> = Vec::new();
    while j < chars.len() && idx < 128 {
        let c = chars[j] as u32;
        if c < 3 {
            if !word.is_empty() || !tail.is_empty() {
                word.extend(tail.iter().cloned());
                vec[idx] = Value::list(word.clone());
                last = word.clone();
                word.clear();
            }
            tail.clear();
            j += 1;
            idx += 1;
            if c > 0 {
                if c == 1 && j < chars.len() {
                    diff_head = chars[j] as usize;
                    j += 1;
                }
                let t = diff_head % 16;
                tail = last.get(t.min(last.len())..).unwrap_or(&[]).to_vec();
                for e in last.iter().take(diff_head / 16) {
                    word.push(e.clone());
                }
            }
        } else {
            let w = (c - 3) as usize;
            let e = match word_table.get(w) {
                Some(v) => {
                    if name_mode {
                        match v {
                            Value::Sym(sid) => {
                                Value::Str(Rc::new(RefCell::new(i.symbol_name(*sid))))
                            }
                            other => other.clone(),
                        }
                    } else {
                        v.clone()
                    }
                }
                None => {
                    if name_mode {
                        Value::Str(Rc::new(RefCell::new(
                            char::from_u32(c).unwrap_or('\0').to_string(),
                        )))
                    } else {
                        Value::Int(c as i128)
                    }
                }
            };
            word.push(e);
            j += 1;
        }
    }
    if idx < 128 && (!word.is_empty() || !tail.is_empty()) {
        word.extend(tail.iter().cloned());
        vec[idx] = Value::list(word);
    }
    vec
}

/// Write each decoded entry of a 128-char block back into TABLE —
/// GNU's decoders memoize with `(aset table (+ first-char i) ...)'.
fn unidata_memoize(i: &mut Interp, tbl: &Value, first: u32, vec: &[Value]) {
    for (k, v) in vec.iter().enumerate().take(128) {
        misc::ct_set(i, tbl, first + k as u32, v.clone());
    }
}

/// GNU `unidata-get-decomposition': word-list block decode plus the
/// Hangul-syllable formula; nil decodes to (list CHAR).
fn unidata_get_decomposition(i: &mut Interp, ch: u32, raw: &Value, tbl: &Value) -> Value {
    match raw {
        Value::Nil => Value::list(vec![Value::Int(ch as i128)]),
        Value::Cons(_) => raw.clone(),
        Value::Str(s) => {
            if s.borrow().chars().next() != Some('\0') {
                return raw.clone();
            }
            let first = (ch >> 7) << 7;
            let word_table = match ct_extra(tbl, 4) {
                Value::Vec(v) => v.borrow().clone(),
                _ => Vec::new(),
            };
            let vec = unidata_word_list_decode(i, &s.borrow(), &word_table, false);
            unidata_memoize(i, tbl, first, &vec);
            match vec[(ch - first) as usize].clone() {
                Value::Nil => Value::list(vec![Value::Int(ch as i128)]),
                v => v,
            }
        }
        Value::Int(0) if (0xAC00..=0xD7A3).contains(&ch) => {
            let c = ch - 0xAC00;
            let l = 0x1100 + c / 588;
            let v = 0x1161 + (c % 588) / 28;
            let t = 0x11A7 + c % 28;
            if t == 0x11A7 {
                Value::list(vec![Value::Int(l as i128), Value::Int(v as i128)])
            } else {
                let lv = 0xAC00 + (c / 28) * 28;
                Value::list(vec![Value::Int(lv as i128), Value::Int(t as i128)])
            }
        }
        _ => Value::Nil,
    }
}

/// GNU `unidata-get-name': word-list decode joined into a NAME
/// string (a `-' element joins without spaces); a positive integer
/// raw value indexes the block-name symbol table in extras[4][1]
/// (Hangul syllables decompose into Jamo names).
fn unidata_get_name(i: &mut Interp, ch: u32, raw: &Value, tbl: &Value) -> Value {
    match raw {
        Value::Str(s) => {
            if s.borrow().chars().next() != Some('\0') {
                return raw.clone();
            }
            let first = (ch >> 7) << 7;
            let word_table = match ct_extra(tbl, 4) {
                Value::Vec(v) => match v.borrow().first() {
                    Some(Value::Vec(w)) => w.borrow().clone(),
                    _ => Vec::new(),
                },
                _ => Vec::new(),
            };
            let vec = unidata_word_list_decode(i, &s.borrow(), &word_table, true);
            let mut out = Value::Nil;
            for (k, ent) in vec.iter().enumerate() {
                let name = match ent {
                    Value::Nil => Value::Nil,
                    l => {
                        let mut parts: Vec<String> = Vec::new();
                        // GNU `unidata-get-name': " " before each word,
                        // except before "-" and before the word after "-"
                        // (its `cddr' walks past the word after a dash).
                        let mut after_dash = false;
                        for (n, e) in l.list_to_vec().unwrap_or_default().iter().enumerate() {
                            let w = match e {
                                Value::Str(w) => w.borrow().clone(),
                                Value::Sym(sid) => i.symbol_name(*sid),
                                Value::Int(c) => char::from_u32(*c as u32)
                                    .map(|c| c.to_string())
                                    .unwrap_or_default(),
                                _ => String::new(),
                            };
                            let dash = w == "-";
                            if n > 0 && !dash && !after_dash {
                                parts.push(" ".to_string());
                            }
                            after_dash = dash;
                            parts.push(w);
                        }
                        Value::Str(Rc::new(RefCell::new(parts.concat())))
                    }
                };
                misc::ct_set(i, tbl, first + k as u32, name.clone());
                if first + k as u32 == ch {
                    out = name;
                }
            }
            out
        }
        Value::Int(n) if *n > 0 => {
            let sym_table = match ct_extra(tbl, 4) {
                Value::Vec(v) => match v.borrow().get(1) {
                    Some(Value::Vec(w)) => w.borrow().clone(),
                    _ => Vec::new(),
                },
                _ => Vec::new(),
            };
            let name = match sym_table.get((*n - 1) as usize) {
                Some(Value::Sym(sid)) => i.symbol_name(*sid),
                _ => return Value::Nil,
            };
            match name.as_str() {
                "HANGUL SYLLABLE" => {
                    let jamo = match ct_extra(tbl, 4) {
                        Value::Vec(v) => match v.borrow().get(2) {
                            Some(Value::Vec(j)) => j.borrow().clone(),
                            _ => Vec::new(),
                        },
                        _ => Vec::new(),
                    };
                    let c = ch - 0xAC00;
                    let (l, v, t) = ((c / 588) as usize, ((c % 588) / 28) as usize, c % 28);
                    let jamo_name = |part: usize, k: usize| -> String {
                        match jamo.get(part) {
                            Some(Value::Vec(jv)) => match jv.borrow().get(k) {
                                Some(Value::Sym(sid)) => i.symbol_name(*sid),
                                _ => String::new(),
                            },
                            _ => String::new(),
                        }
                    };
                    let tail = if t == 0 {
                        String::new()
                    } else {
                        jamo_name(2, t as usize - 1)
                    };
                    Value::Str(Rc::new(RefCell::new(format!(
                        "HANGUL SYLLABLE {}{}{}",
                        jamo_name(0, l),
                        jamo_name(1, v),
                        tail
                    ))))
                }
                "CJK IDEOGRAPH"
                | "TANGUT IDEOGRAPH"
                | "CJK COMPATIBILITY IDEOGRAPH"
                | "HIGH SURROGATE"
                | "LOW SURROGATE" => {
                    Value::Str(Rc::new(RefCell::new(format!("{}-{:04X}", name, ch))))
                }
                "VARIATION SELECTOR" => Value::Str(Rc::new(RefCell::new(format!(
                    "{}-{}",
                    name,
                    ch.wrapping_sub(0xE0100).wrapping_add(17)
                )))),
                _ => Value::Nil,
            }
        }
        _ => Value::Nil,
    }
}

/// GNU's `(if (functionp X1) (funcall X1 CH RAW TABLE) ...)' in
/// `get-char-code-property': the uni-*.el tables store a compiled
/// `unidata-get-*' Lisp decoder in extra slot 1 — implemented
/// natively here keyed on the property name in extra slot 0 —
/// while fixnum extra slot 1 selects a C decoder (0 = run-length
/// through the extra-slot-4 value vector).
pub(crate) fn uniprop_lookup(i: &mut Interp, tbl: &Value, ch: u32, raw: Value) -> Value {
    match ct_extra(tbl, 1) {
        Value::Int(0) => uniprop_decode_run_length(tbl, raw),
        Value::Int(_) | Value::Nil => raw,
        _ => match ct_extra(tbl, 0) {
            Value::Sym(p) if i.symbol_name(p) == "decomposition" => {
                unidata_get_decomposition(i, ch, &raw, tbl)
            }
            Value::Sym(p)
                if matches!(
                    i.symbol_name(p).as_str(),
                    "name" | "old-name" | "iso-10646-comment"
                ) =>
            {
                unidata_get_name(i, ch, &raw, tbl)
            }
            _ => raw,
        },
    }
}

fn get_char_prop(i: &mut Interp, ch: i64, prop: &str) -> Value {
    if let Some(e) = i.char_code_props.iter().find(|(n, _)| n == prop) {
        if let Some((_, v)) = e.1.iter().find(|(c, _)| *c == ch) {
            return v.clone();
        }
    }
    // GNU `get-char-code-property': materialize the property's table
    // (lazily loading its backing file), then apply the table's own
    // decoder — a Lisp function in extra slot 1, or the fixnum C
    // decoder index (run-length through extra slot 4).
    let tbl = unicode_prop_table(i, prop);
    if is_char_table(i, &tbl) {
        let raw = misc::char_table_ref(i, &tbl, ch.max(0) as usize);
        return uniprop_lookup(i, &tbl, ch.max(0) as u32, raw);
    }
    builtin_char_prop(i, ch, prop)
}

/// GNU charprop.c lazily loads the file backing a char-code property —
/// `(load FILE nil t)' — and the uni-*.el file re-registers the property
/// with a real char-table.
fn load_file_backed_prop(i: &mut Interp, prop: &str) {
    let file = i
        .char_code_prop_tables
        .iter()
        .find(|(n, _)| n == prop)
        .and_then(|(_, t)| match t {
            Value::Str(s) => Some(s.borrow().clone()),
            _ => None,
        })
        .or_else(|| {
            let e = prop_alist_entry(i, prop);
            if let Value::Cons(c) = &e {
                match c.borrow().cdr.clone() {
                    Value::Str(s) => Some(s.borrow().clone()),
                    _ => None,
                }
            } else {
                None
            }
        });
    if let Some(file) = file {
        // GNU uniprop_table loads the table file with NOMESSAGE = t.
        let _ = crate::lisp::load::load_library_opts_msg(i, &file, false, false, true, true);
    }
}

/// `char-code-property-alist' is the authoritative registry — GNU's
/// `define-char-code-property' is a mule-cmds defun that updates it
/// directly, so a lazily loaded uni-*.el file can register its table
/// without the Rust cache seeing the define call.  Mirror a table
/// found there back into `char_code_prop_tables'.
fn sync_prop_table(i: &mut Interp, prop: &str) {
    if let Value::Cons(c) = prop_alist_entry(i, prop) {
        let v = c.borrow().cdr.clone();
        if is_char_table(i, &v) {
            let stale = i
                .char_code_prop_tables
                .iter()
                .find(|(n, _)| n == prop)
                .map(|(_, t)| t.clone());
            if stale.as_ref().map_or(true, |t| !eq_values(t, &v)) {
                i.char_code_prop_tables.retain(|(n, _)| n != prop);
                i.char_code_prop_tables.push((prop.to_string(), v));
            }
        }
    }
}

/// GNU signals `wrong-type-argument (char-table-p FILE)` when a property
/// registered with a file name is used for lookup or storage.
fn check_prop_backing(i: &mut Interp, prop: &str) -> Result<(), Flow> {
    load_file_backed_prop(i, prop);
    sync_prop_table(i, prop);
    let bad = i
        .char_code_prop_tables
        .iter()
        .find(|(n, _)| n == prop)
        .filter(|(_, tbl)| matches!(tbl, Value::Str(_)))
        .map(|(_, tbl)| tbl.clone())
        .or_else(|| match prop_alist_entry(i, prop) {
            Value::Cons(c) => match c.borrow().cdr.clone() {
                Value::Str(s) => Some(Value::Str(s)),
                _ => None,
            },
            _ => None,
        });
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
            let cls = super::bidi_table::BIDI_CLASS_TABLE
                .binary_search_by(|&(lo, hi, _)| {
                    if u < lo {
                        std::cmp::Ordering::Greater
                    } else if u > hi {
                        std::cmp::Ordering::Less
                    } else {
                        std::cmp::Ordering::Equal
                    }
                })
                .map(|idx| super::bidi_table::BIDI_CLASS_TABLE[idx].2)
                .ok();
            match cls {
                Some(cls) => symv(i, cls),
                None => return Value::Nil,
            }
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

fn f_get_char_code_property(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let ch = match &a[0] {
        Value::Int(n) => *n as i64,
        _ => return Err(i.wrong_type_mut("characterp", &a[0])),
    };
    let pid2 = want_sym(i, &a[1])?;
    let prop = i.symbol_name(pid2);
    check_prop_backing(i, &prop)?;
    Ok(get_char_prop(i, ch, &prop))
}

/// GNU `put-unicode-property-internal' without the arg checking:
/// a fixnum in extra slot 2 selects a C encoder — 0 requires a
/// character (or nil), 1 looks VALUE up in the extra-slot-4 vector
/// and stores its index; the result lands via `char_table_set'.
fn put_unicode_internal(i: &mut Interp, tbl: &Value, ch: u32, val: Value) -> EvalResult {
    let val = match ct_extra(tbl, 2) {
        Value::Int(0) => {
            if !val.is_nil() && !matches!(val, Value::Int(_)) {
                return Err(i.wrong_type_mut("integerp", &val));
            }
            val
        }
        Value::Int(1) => {
            let mut enc = Value::Nil;
            if let Value::Vec(vv) = ct_extra(tbl, 4) {
                for (k, e) in vv.borrow().iter().enumerate() {
                    if eq_values(e, &val) {
                        enc = Value::Int(k as i128);
                        break;
                    }
                }
            }
            if enc.is_nil() {
                return Err(i.wrong_type_mut("Unicode property value", &val));
            }
            enc
        }
        _ => val,
    };
    misc::ct_set(i, tbl, ch, val);
    Ok(Value::Nil)
}

fn f_put_char_code_property(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let ch = match &a[0] {
        Value::Int(n) => *n,
        _ => return Err(i.wrong_type_mut("characterp", &a[0])),
    };
    let pid2 = want_sym(i, &a[1])?;
    let prop = i.symbol_name(pid2);
    check_prop_backing(i, &prop)?;
    // Validate the original integer before narrowing it for native storage.
    // Otherwise negative or oversized codes can alias a valid table entry.
    if !(0..=misc::CT_MAX_CHAR as i128).contains(&ch) {
        return Err(i.wrong_type_mut("characterp", &a[0]));
    }
    let ch = ch as i64;
    let tbl = unicode_prop_table(i, &prop);
    if is_char_table(i, &tbl) {
        match ct_extra(&tbl, 2) {
            Value::Int(_) | Value::Nil => {
                put_unicode_internal(i, &tbl, ch.max(0) as u32, a[2].clone())?;
            }
            _ => {
                // GNU `(funcall X2 CH VAL TABLE)' — a `unidata-put-*'
                // Lisp function: decode the current block first (so
                // its memoization can't clobber VAL), then `aset'.
                let raw = misc::char_table_ref(i, &tbl, ch.max(0) as usize);
                if let Value::Str(s) = &raw {
                    if s.borrow().chars().next() == Some('\0') {
                        uniprop_lookup(i, &tbl, ch.max(0) as u32, raw);
                    }
                }
                misc::ct_set(i, &tbl, ch.max(0) as u32, a[2].clone());
            }
        }
        return Ok(a[2].clone());
    }
    // Non-table property: GNU stores into `char-code-property-table'
    // plists; our overlay keeps the same get-side semantics.
    let store = prop_table(i, &prop);
    if let Some(e) = store.iter_mut().find(|(c, _)| *c == ch) {
        e.1 = a[2].clone();
    } else {
        store.push((ch, a[2].clone()));
    }
    Ok(a[2].clone())
}

fn f_char_code_property_description(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU: non-symbol
    // args (and unknown props/values) → nil, no checks at all.
    let (Some(pid2), Some(vid)) = (i.sym_id(&a[0]), i.sym_id(&a[1])) else {
        return Ok(Value::Nil);
    };
    let prop = i.symbol_name(pid2);
    let val = i.symbol_name(vid);
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

fn f_make_translation_table(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU mule.el: each arg is a list of (FROM . TO) pairs.  A prior
    // mapping TO -> TO-ALT makes FROM also map to TO-ALT, and chars
    // that mapped to FROM are updated to map to TO (tracked by
    // REVLIST, an alist (TO . (FROM ...))).
    let tag = symv(i, "translation-table");
    let t = misc::make_ct(i, tag, Value::Nil, vec![Value::Nil; 2]);
    let mut revlist: Vec<(i128, Vec<i128>)> = vec![];
    for arg in a.iter() {
        let mut elts = Vec::new();
        arg.each_car(|v| elts.push(v.clone()));
        for elt in elts {
            let (from, to) = match &elt {
                Value::Cons(c) => {
                    let b = c.borrow();
                    (b.car.clone(), b.cdr.clone())
                }
                _ => continue,
            };
            let (Value::Int(f), Value::Int(t0)) = (&from, &to) else {
                return Err(i.wrong_type_mut("fixnump", &from));
            };
            let (f, mut to_c) = (*f, *t0);
            // (if (setq to-alt (aref table to)) (setq to to-alt))
            if let Value::Int(ta) = misc::char_table_ref(i, &t, to_c as usize) {
                to_c = ta;
            }
            misc::ct_set(i, &t, f as u32, Value::Int(to_c));
            // Chars previously mapped to FROM now map to TO.
            let mut rev_from: Vec<i128> = vec![];
            if let Some(pos) = revlist.iter().position(|(k, _)| *k == f) {
                let (_, list) = revlist.remove(pos);
                for e in &list {
                    misc::ct_set(i, &t, *e as u32, Value::Int(to_c));
                }
                rev_from = list;
            }
            match revlist.iter_mut().find(|(k, _)| *k == to_c) {
                Some((_, l)) => {
                    let mut nl = vec![f];
                    nl.append(l);
                    nl.extend(rev_from);
                    *l = nl;
                }
                None => {
                    let mut l = vec![f];
                    l.extend(rev_from);
                    revlist.push((to_c, l));
                }
            }
        }
    }
    // GNU sets extra slot 1 to 1 (max-lookup).
    if let Value::Record(r) = &t {
        r.borrow_mut()[4] = Value::Int(1);
    }
    Ok(t)
}

fn f_define_translation_table(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU mule.el: a `translation-table' char-table arg is used
    // directly; other args go to `make-translation-table'.  Registers
    // (SYMBOL . TABLE) in `translation-table-vector' and sets the
    // `translation-table'/`translation-table-id' properties.
    let id = want_sym(i, &a[0])?;
    let table = match a.get(1) {
        Some(v)
            if misc::is_char_table(i, v)
                && matches!(v, Value::Record(r)
                if matches!(r.borrow().get(1), Some(Value::Sym(s)) if i.symbol_name(*s) == "translation-table")) =>
        {
            v.clone()
        }
        _ => f_make_translation_table(i, a[1..].to_vec().into())?,
    };
    let ttv = i.intern("translation-table-vector");
    let mut vec = match i.symbol_value(ttv) {
        Value::Vec(v) => v.borrow().clone(),
        _ => Vec::new(),
    };
    if vec.is_empty() {
        vec = vec![Value::Nil; 16];
    }
    let mut tid = 0usize;
    loop {
        if tid >= vec.len() {
            let n = vec.len();
            vec.extend(std::iter::repeat(Value::Nil).take(n));
        }
        let use_slot = match &vec[tid] {
            Value::Nil => true,
            Value::Cons(c) => matches!(&c.borrow().car, Value::Sym(s) if *s == id),
            _ => false,
        };
        if use_slot {
            break;
        }
        tid += 1;
    }
    vec[tid] = Value::cons(Value::Sym(id), table.clone());
    let _ = i.set_symbol(
        ttv,
        Value::Vec(std::rc::Rc::new(std::cell::RefCell::new(vec))),
    );
    let prop = i.intern("translation-table");
    i.put_prop(id, prop, table);
    let tidp = i.intern("translation-table-id");
    i.put_prop(id, tidp, Value::Int(tid as i128));
    Ok(Value::Int(tid as i128))
}

fn f_make_translation_table_from_vector(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
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
    let tag = symv(i, "translation-table");
    let t = misc::make_ct(i, tag, Value::Nil, vec![]);
    for (k, v) in slots.iter().enumerate() {
        if !v.is_nil() {
            misc::ct_set(i, &t, k as u32, v.clone());
        }
    }
    Ok(t)
}

fn f_set_translation_table(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let _ = &a;
    let _ = i;
    Ok(Value::Nil)
}

// ---------- coding systems ----------

pub(crate) fn f_coding_system_type(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let name = match &a[0] {
        Value::Sym(s) => i.symbol_name(*s),
        // GNU treats nil as `no-conversion' here.
        Value::Nil => "no-conversion".to_string(),
        _ => return Err(i.wrong_type_mut("symbolp", &a[0])),
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

/// A charset's attribute plist entry as i64 (`:iso-final-char',
/// `:emacs-mule-id', `:code-offset'), consulting builtin attrs too.
fn charset_prop_of(i: &mut Interp, name: &str, key: &str) -> Value {
    let plist = effective_charset_plist(i, name);
    match i.intern_soft(key) {
        Some(id) => crate::lisp::eval::plist_get(&plist, id),
        None => Value::Nil,
    }
}

/// GNU `Viso_2022_charset_list': charsets with an iso-final-char, in
/// ordered-list order.
fn iso_2022_charsets(i: &mut Interp) -> Vec<String> {
    charsets_ordered(i)
        .into_iter()
        .filter(|n| !charset_prop_of(i, n, ":iso-final-char").is_nil())
        .collect()
}

/// GNU `Vemacs_mule_charset_list': charsets with an emacs-mule-id, in
/// ordered-list order.
fn emacs_mule_charsets(i: &mut Interp) -> Vec<String> {
    charsets_ordered(i)
        .into_iter()
        .filter(|n| !charset_prop_of(i, n, ":emacs-mule-id").is_nil())
        .collect()
}

/// The key used for CODING-SYSTEM's entry in `remacs-coding-system-plists':
/// EOL variants and runtime aliases share the base system's plist.
fn coding_plist_key(i: &Interp, name: &str) -> String {
    fn strip(n: &str) -> &str {
        n.strip_suffix("-unix")
            .or_else(|| n.strip_suffix("-dos"))
            .or_else(|| n.strip_suffix("-mac"))
            .unwrap_or(n)
    }
    let mut cur = strip(name).to_string();
    // Alias chain (static group head or define-coding-system-alias target).
    for _ in 0..16 {
        if let Some(&(_, group)) = misc::CODING_ALIASES
            .iter()
            .find(|&&(n, grp)| n == cur || grp.iter().any(|m| *m == cur))
        {
            let g = group[0].to_string();
            if g == cur {
                break;
            }
            cur = g;
            continue;
        }
        match i.extra_coding_aliases.iter().find(|(x, _)| *x == cur) {
            Some((_, t)) => cur = strip(t).to_string(),
            None => break,
        }
    }
    cur
}

/// (charset-list attr, coding-type attr) from the coding system's
/// plist in `remacs-coding-system-plists'.
fn coding_charset_attrs(i: &mut Interp, v: &Value) -> Option<(Value, Value)> {
    let table_id = i.intern("remacs-coding-system-plists");
    let table = i.symbol_value(table_id);
    let name = coding_known(i, v)?;
    let key = coding_plist_key(i, &name);
    let keysym = i.intern(&key);
    let mut cur = table;
    while let Value::Cons(c) = cur {
        let (car, cdr) = {
            let b = c.borrow();
            (b.car.clone(), b.cdr.clone())
        };
        if let Value::Cons(pair) = &car {
            let (k, pl) = {
                let b = pair.borrow();
                (b.car.clone(), b.cdr.clone())
            };
            if matches!(&k, Value::Sym(s) if *s == keysym) {
                let cl = crate::lisp::eval::plist_get(&pl, i.intern(":charset-list"));
                let ty = crate::lisp::eval::plist_get(&pl, i.intern(":coding-type"));
                return Some((cl, ty));
            }
        }
        cur = cdr;
    }
    None
}

/// GNU `CODING_ATTR_CHARSET_LIST': the coding system's raw
/// `:charset-list' attribute.
pub(crate) fn coding_charset_list_attr(i: &mut Interp, v: &Value) -> Value {
    coding_charset_attrs(i, v)
        .map(|(cl, _)| cl)
        .unwrap_or(Value::Nil)
}

/// GNU `coding_system_charset_list' as a Lisp list of charset IDs:
/// `emacs-mule' coding systems use Vemacs_mule_charset_list; iso-2022
/// systems with full support use Viso_2022_charset_list; the rest use
/// the raw :charset-list attribute.
pub(crate) fn coding_system_charset_names(i: &mut Interp, v: &Value) -> Value {
    let Some((attr, ty)) = coding_charset_attrs(i, v) else {
        return Value::Nil;
    };
    let type_is = |want: &str| matches!(&ty, Value::Sym(s) if i.symbol_name(*s) == want);
    let attr_is = |want: &str| matches!(&attr, Value::Sym(s) if i.symbol_name(*s) == want);
    if type_is("emacs-mule") {
        let names = emacs_mule_charsets(i);
        Value::list(names.iter().map(|x| symv(i, x)).collect())
    } else if type_is("iso-2022") && attr_is("iso-2022") {
        let names = iso_2022_charsets(i);
        Value::list(names.iter().map(|x| symv(i, x)).collect())
    } else {
        attr
    }
}

fn f_coding_system_charset_list(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    match &a[0] {
        Value::Sym(_) => {}
        v => return Err(i.wrong_type_mut("symbolp", v)),
    }
    if coding_known(i, &a[0]).is_none() {
        // GNU signals coding-system-error on an undefined coding system.
        let cs = i.intern("coding-system-error");
        return Err(i.signal_data(cs, vec![a[0].clone()]));
    }
    // The Lisp function returns the raw :charset-list attribute — a
    // list of charsets, or the `iso-2022'/`emacs-mule' marker symbols.
    Ok(coding_charset_list_attr(i, &a[0]))
}

/// GNU `CODING_ATTR_CATEGORY'/`CODING_ATTR_BASE_NAME' for a known
/// coding system: (category-symbol, base-name) resolved through the
/// alias chain and `remacs-coding-system-plists'.
fn coding_category_member(i: &mut Interp, v: &Value) -> Option<(String, String)> {
    let table_id = i.intern("remacs-coding-system-plists");
    let table = i.symbol_value(table_id);
    let name = coding_known(i, v)?;
    let key = coding_plist_key(i, &name);
    let keysym = i.intern(&key);
    let mut cur = table;
    while let Value::Cons(c) = cur {
        let (car, cdr) = {
            let b = c.borrow();
            (b.car.clone(), b.cdr.clone())
        };
        if let Value::Cons(pair) = &car {
            let (k, pl) = {
                let b = pair.borrow();
                (b.car.clone(), b.cdr.clone())
            };
            if matches!(&k, Value::Sym(s) if *s == keysym) {
                let cat = crate::lisp::eval::plist_get(&pl, i.intern(":category"));
                let base = crate::lisp::eval::plist_get(&pl, i.intern(":name"));
                let cat = match cat {
                    Value::Sym(s) => i.symbol_name(s).to_string(),
                    _ => return None,
                };
                let base = match base {
                    Value::Sym(s) => i.symbol_name(s).to_string(),
                    _ => key.clone(),
                };
                return Some((cat, base));
            }
        }
        cur = cdr;
    }
    None
}

fn f_set_coding_system_priority(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU
    // Fset_coding_system_priority is CATEGORY-based: each argument must
    // name a coding system (symbolp then coding-system-error), gets
    // deduplicated by category (first wins), becomes that category's
    // member (replacing the previous one), and the touched categories move
    // to the front of the priority order — member names never move.
    for v in a.iter() {
        want_sym(i, v)?;
        if coding_known(i, v).is_none() {
            let cs = i.intern("coding-system-error");
            return Err(i.signal_data(cs, vec![v.clone()]));
        }
    }
    let mut seen: Vec<String> = Vec::new();
    let mut moved: Vec<Value> = Vec::new();
    for v in a.iter() {
        let Some((cat, base)) = coding_category_member(i, v) else {
            continue;
        };
        if seen.iter().any(|c| *c == cat) {
            continue;
        }
        seen.push(cat);
        moved.push(symv(i, &base));
    }
    if !moved.is_empty() {
        let var = i.intern("remacs-coding-system-priorities");
        let mut rest = Vec::new();
        let mut cur = i.symbol_value(var);
        while let Value::Cons(c) = cur {
            let (car, cdr) = {
                let b = c.borrow();
                (b.car.clone(), b.cdr.clone())
            };
            // Drop members moved to the front AND members whose category
            // was reassigned (the category's slot takes the new base name,
            // so the old member disappears — assigning iso-2022-jp-2004
            // evicts iso-2022-jp).
            if moved.iter().any(|m| eq_values(m, &car)) {
                cur = cdr;
                continue;
            }
            if let Some((ccat, _)) = coding_category_member(i, &car) {
                if seen.iter().any(|c| *c == ccat) {
                    cur = cdr;
                    continue;
                }
            }
            rest.push(car);
            cur = cdr;
        }
        moved.extend(rest);
        let _ = i.set_symbol(var, Value::list(moved));
    }
    Ok(Value::Nil)
}

fn f_set_keyboard_coding_system_internal(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU: decode_live_terminal(TERMINAL), CHECK_SYMBOL, nil means
    // `no-conversion', anything else must be a coding system.  The
    // name is stored verbatim — GNU's `setup_coding_system' resolves
    // the given coding system itself (no `-unix' munging here; the
    // Lisp wrapper's `coding-system-change-eol-conversion' does that).
    if let Some(t) = a.get(1) {
        match t {
            Value::Nil | Value::Frame(_) => {}
            other => return Err(i.wrong_type_mut("terminal-live-p", other)),
        }
    }
    let _ = want_sym(i, &a[0])?;
    if !a[0].is_nil() && super::misc::coding_known(i, &a[0]).is_none() {
        let e = i.intern("coding-system-error");
        return Err(i.signal_data(e, vec![a[0].clone()]));
    }
    let sym = if a[0].is_nil() {
        Value::Sym(i.intern("no-conversion"))
    } else {
        a[0].clone()
    };
    i.keyboard_coding = sym.clone();
    // `keyboard-coding-system' is ALSO a defcustom that the mule.el
    // wrapper setqs; keep it in sync so the var and the subr agree.
    let kc = i.intern("keyboard-coding-system");
    if i.bound_p(kc) {
        let _ = i.set_symbol(kc, sym.clone());
    }
    Ok(Value::Nil)
}

fn f_find_coding_systems_region_internal(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU Ffind_coding_systems_region_internal: a STRING START returns t
    // for ASCII/unibyte strings without looking at END; otherwise START
    // and END go through fix_position — non-integer/marker args signal
    // wrong-type-argument integer-or-marker-p BEFORE the bounds check.
    if let Value::Str(s) = &a[0] {
        let s = s.borrow();
        // Unibyte or pure-ASCII: every coding system can encode it → t.
        if s.is_ascii() {
            return Ok(Value::t());
        }
        // Multibyte non-ASCII: any-codable approximation (we don't track
        // per-coding charset coverage here).
        return Ok(Value::t());
    }
    let s = crate::lisp::builtins::want_int_or_mark(i, &a[0])?;
    let e = crate::lisp::builtins::want_int_or_mark(i, &a[1])?;
    let zv = i
        .buffers
        .get(i.current_buffer)
        .map(|b| b.borrow().text_len() as i128 + 1)
        .unwrap_or(1);
    // GNU: !(BEG <= s && s <= e && e <= Z) → args-out-of-range.
    if !(1 <= s && s <= e && e <= zv) {
        let sid = i.intern("args-out-of-range");
        return Err(i.signal(sid, Value::list(vec![a[0].clone(), a[1].clone()])));
    }
    // GNU returns t for regions any coding system can encode.
    Ok(Value::t())
}

// ---------- charset ids / priority / unicode property tables ----------

/// GNU's built-in charset ids (init order in charset.c); user-defined
/// charsets get ids >= 40 by registration order.
/// GNU's charset ids: the five C builtins get 0..4 (init order in
/// syms_of_charset); every `define-charset-internal' name takes the
/// next id in definition order.
fn charset_id(i: &Interp, name: &str) -> i128 {
    match name {
        "ascii" => return 0,
        "iso-8859-1" => return 1,
        "unicode" | "ucs" => return 2,
        "emacs" => return 3,
        "eight-bit" => return 4,
        _ => {}
    }
    i.charsets
        .iter()
        .filter(|(n, _)| {
            !matches!(
                n.as_str(),
                "ascii" | "iso-8859-1" | "unicode" | "ucs" | "emacs" | "eight-bit"
            )
        })
        .position(|(n, _)| n == name)
        .map(|p| 5 + p as i128)
        .unwrap_or(-1)
}

fn f_charset_id_internal(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU: nil/missing arg or unknown name → wrong-type-argument charsetp.
    let name = want_charset(i, &arg(&a, 0))?;
    Ok(Value::Int(charset_id(i, &name)))
}

/// GNU 31's `(charset-priority-list)' order at -Q.
pub(crate) const GNU_CHARSET_PRIORITY: &[&str] = &[
    "japanese-jisx0208",
    "japanese-jisx0212",
    "latin-jisx0201",
    "katakana-jisx0201",
    "japanese-jisx0213.2004-1",
    "japanese-jisx0213-1",
    "japanese-jisx0213-2",
    "japanese-jisx0208-1978",
    "ascii",
    "chinese-gb2312",
    "korean-ksc5601",
    "latin-iso8859-1",
    "greek-iso8859-7",
    "iso-8859-1",
    "unicode",
    "control-1",
    "iso-8859-2",
    "latin-iso8859-2",
    "iso-8859-3",
    "latin-iso8859-3",
    "iso-8859-4",
    "latin-iso8859-4",
    "iso-8859-5",
    "cyrillic-iso8859-5",
    "iso-8859-6",
    "arabic-iso8859-6",
    "iso-8859-7",
    "iso-8859-8",
    "hebrew-iso8859-8",
    "iso-8859-9",
    "latin-iso8859-9",
    "iso-8859-10",
    "latin-iso8859-10",
    "iso-8859-11",
    "thai-iso8859-11",
    "iso-8859-13",
    "latin-iso8859-13",
    "iso-8859-14",
    "latin-iso8859-14",
    "iso-8859-15",
    "latin-iso8859-15",
    "iso-8859-16",
    "latin-iso8859-16",
    "thai-tis620",
    "tis620-2533",
    "jisx0201",
    "chinese-gbk",
    "chinese-cns11643-1",
    "chinese-cns11643-2",
    "chinese-cns11643-3",
    "chinese-cns11643-4",
    "chinese-cns11643-5",
    "chinese-cns11643-6",
    "chinese-cns11643-7",
    "big5",
    "cp932",
    "big5-hkscs",
    "cp949",
    "viscii",
    "vscii",
    "vscii-2",
    "koi8-r",
    "alternativnyj",
    "cp866",
    "koi8-u",
    "koi8-t",
    "georgian-ps",
    "georgian-academy",
    "windows-1250",
    "windows-1251",
    "windows-1252",
    "windows-1253",
    "windows-1254",
    "windows-1255",
    "windows-1256",
    "windows-1257",
    "windows-1258",
    "next",
    "cp1125",
    "cp437",
    "cp720",
    "cp737",
    "cp775",
    "cp851",
    "cp852",
    "cp855",
    "cp857",
    "cp858",
    "cp860",
    "cp861",
    "cp862",
    "cp863",
    "cp864",
    "cp865",
    "cp869",
    "cp874",
    "unicode-bmp",
    "unicode-smp",
    "unicode-sip",
    "unicode-ssp",
    "mac-roman",
    "ebcdic-us",
    "ebcdic-uk",
    "ibm038",
    "ibm256",
    "ibm273",
    "ibm274",
    "ibm275",
    "ibm277",
    "ibm278",
    "ibm280",
    "ibm281",
    "ibm284",
    "ibm285",
    "ibm290",
    "ibm297",
    "ibm1047",
    "hp-roman8",
    "adobe-standard-encoding",
    "symbol",
    "ibm850",
    "mik",
    "ptcp154",
    "gb18030",
    "chinese-cns11643-15",
    "emacs",
    "eight-bit",
    "eight-bit-control",
    "eight-bit-graphic",
    "chinese-big5-1",
    "chinese-big5-2",
    "japanese-jisx0213-a",
    "katakana-sjis",
    "cp932-2-byte",
    "cp949-2-byte",
    "chinese-sisheng",
    "ipa",
    "vietnamese-viscii-lower",
    "vietnamese-viscii-upper",
    "arabic-digit",
    "arabic-1-column",
    "arabic-2-column",
    "lao",
    "mule-lao",
    "indian-is13194",
    "devanagari-cdac",
    "sanskrit-cdac",
    "bengali-cdac",
    "tamil-cdac",
    "telugu-cdac",
    "assamese-cdac",
    "oriya-cdac",
    "kannada-cdac",
    "malayalam-cdac",
    "gujarati-cdac",
    "punjabi-cdac",
    "devanagari-akruti",
    "bengali-akruti",
    "punjabi-akruti",
    "gujarati-akruti",
    "oriya-akruti",
    "tamil-akruti",
    "telugu-akruti",
    "kannada-akruti",
    "malayalam-akruti",
    "indian-glyph",
    "indian-1-column",
    "indian-2-column",
    "tibetan",
    "tibetan-1-column",
    "mule-unicode-2500-33ff",
    "mule-unicode-e000-ffff",
    "mule-unicode-0100-24ff",
    "ethiopic",
    "gb18030-2-byte",
    "gb18030-4-byte-bmp",
    "gb18030-4-byte-smp",
    "gb18030-4-byte-ext-1",
    "gb18030-4-byte-ext-2",
];

/// GNU `Vcharset_ordered_list' as names: the fixed -Q order plus any
/// runtime-defined charsets, or the list rebuilt by
/// `set-charset-priority'/`define-charset-internal'.
fn charsets_ordered(i: &Interp) -> Vec<String> {
    let st = i.charset_order.borrow();
    let mut names: Vec<String> = if st.0.is_empty() {
        GNU_CHARSET_PRIORITY.iter().map(|n| n.to_string()).collect()
    } else {
        st.0.clone()
    };
    for (n, _) in &i.charsets {
        if !names.contains(n) {
            names.push(n.clone());
        }
    }
    names
}

/// Index of `Vcharset_non_preferred_head' within `charsets_ordered'.
/// At -Q the first 16 entries are preferred (jisx0208..control-1);
/// `set-charset-priority' resets the boundary to its arg count.
fn charset_non_preferred_idx(i: &Interp) -> usize {
    let st = i.charset_order.borrow();
    if st.0.is_empty() { 16 } else { st.1 }
}

fn invalidate_char_charset_cache(i: &Interp) {
    i.char_charset_cache.borrow_mut().clear();
}

/// GNU `Fmake_char': pack the given per-dimension position codes into
/// a packed code-point (MSB-first bytes), then decode it through the
/// charset.  Missing codes default to the charset's minimum for that
/// dimension (0 if the charset is ASCII-compatible); every supplied
/// code must be a fixnum < 256 (`wholenump' otherwise,
/// args-out-of-range when ≥256).  Charsets with an iso-final-char
/// mask each byte to 7 bits; an in-range packed code that decodes to
/// nothing signals "Invalid code(s)".
pub(crate) fn make_char(i: &mut Interp, name: &str, codes: &[Value]) -> Result<i64, Flow> {
    let plist = effective_charset_plist(i, name);
    let dims: Vec<(i64, i64)> = charset_dims(i, &plist).unwrap_or_default();
    let dim = match prop_int(&charset_prop(i, &plist, ":dimension")) {
        Some(d) if d >= 1 => d as usize,
        _ => dims.len().max(1),
    };
    let iso_final = prop_int(&charset_prop(i, &plist, ":iso-final-char")).unwrap_or(-1);
    let compat = charset_prop(i, &plist, ":ascii-compatible-p").truthy();
    let mut min_code = 0i64;
    for k in 0..dim {
        let lo = dims.get(k).map(|d| d.0).unwrap_or(0);
        min_code = (min_code << 8) | lo;
    }
    if let Some(v) = prop_int(&charset_prop(i, &plist, ":min-code")) {
        min_code = v;
    }
    // GNU: each supplied code goes into byte position (dim - j)
    // (code1 = most significant); a missing code defaults to that
    // dimension's code-space low bound.
    let mut code: i64 = 0;
    if codes.first().is_none_or(|v| v.is_nil()) {
        code = if compat { 0 } else { min_code };
    } else {
        for j in 1..=dim {
            let v = codes.get(j - 1);
            let d = match v {
                None | Some(Value::Nil) => {
                    // Default: low bound of the dim at byte (dim - j),
                    // i.e. dims[j-1] in MSB-first order.
                    dims.get(j - 1).map(|d| d.0).unwrap_or(0)
                }
                Some(other) => match other {
                    Value::Int(n) if *n >= 0 => {
                        if *n >= 0x100 {
                            let sym = i.intern("args-out-of-range");
                            return Err(i.signal_data(sym, vec![Value::Int(0xFF), other.clone()]));
                        }
                        *n as i64
                    }
                    _ => return Err(i.wrong_type_mut("wholenump", other)),
                },
            };
            code = (code << 8) | d;
        }
    }
    if iso_final >= 0 {
        code &= 0x7F7F7F7F;
    }
    match charset_decode(i, name, code) {
        Some(c) => Ok(c),
        None => Err(i.error("Invalid code(s)")),
    }
}

/// GNU `cons_to_unsigned' for `decode-char': an integer or integral
/// float fits a u64; a cons packs (HIGH . LOW) or a two-element list
/// (HIGH LOW) into a >16-bit code-point.
pub(crate) fn cons_to_u64(v: &Value) -> Option<u64> {
    match v {
        Value::Int(n) if *n >= 0 => Some(*n as u64),
        Value::Float(f) if f.fract() == 0.0 && **f >= 0.0 && **f < (u64::MAX as f64) => {
            Some(**f as u64)
        }
        Value::Cons(c) => {
            let (hi, rest) = {
                let b = c.borrow();
                (b.car.clone(), b.cdr.clone())
            };
            let hi = match hi {
                Value::Int(n) if n >= 0 => n as u64,
                _ => return None,
            };
            match &rest {
                Value::Cons(r) => {
                    let (mid, tail) = {
                        let b = r.borrow();
                        (b.car.clone(), b.cdr.clone())
                    };
                    match (&mid, &tail) {
                        (Value::Int(m), Value::Int(lo))
                            if *m >= 0 && *m < (1 << 24) && *lo >= 0 && *lo < (1 << 16) =>
                        {
                            if hi > u64::MAX >> 40 {
                                None
                            } else {
                                Some((hi << 40) | ((*m as u64) << 16) | *lo as u64)
                            }
                        }
                        _ => {
                            let lo = match &mid {
                                Value::Int(n) if *n >= 0 && *n < (1 << 16) => *n as u64,
                                _ => return None,
                            };
                            Some((hi << 16) | lo)
                        }
                    }
                }
                Value::Int(lo) if *lo >= 0 && *lo < (1 << 16) => Some((hi << 16) | *lo as u64),
                _ => None,
            }
        }
        _ => None,
    }
}

/// A charset's `:dimension' (or code-space/2) — the number of packed
/// bytes in its position codes.
pub(crate) fn charset_dimension_of(i: &mut Interp, name: &str) -> usize {
    let plist = effective_charset_plist(i, name);
    match prop_int(&charset_prop(i, &plist, ":dimension")) {
        Some(d) if d >= 1 => d as usize,
        _ => charset_dims(i, &plist).map(|d| d.len()).unwrap_or(1).max(1),
    }
}

/// GNU `char_charset': the first charset in ORDER (empty = the ordered
/// list) whose ENCODE_CHAR succeeds; restricted lists may yield no
/// winner, the default order falls back to `unicode' at the
/// non-preferred boundary (for Unicode chars), then `emacs' or
/// `eight-bit' for internal codes above Unicode.
pub(crate) fn char_charset(i: &Interp, ch: i64, order: &[String]) -> Option<String> {
    if !(0..=0x3FFFFF).contains(&ch) {
        return None;
    }
    if order.is_empty() {
        // GNU's CHAR_CHARSET macro: ASCII chars are hardwired to
        // charset_ascii — the ordered-list walk never sees them.
        if ch < 128 {
            return Some("ascii".to_string());
        }
        let ordered = charsets_ordered(i);
        let boundary = charset_non_preferred_idx(i);
        let default_order = i.charset_order.borrow().0.is_empty();
        if default_order {
            if let Some(hit) = i.char_charset_cache.borrow().get(&ch) {
                return Some(hit.clone());
            }
        }
        let mut winner = None;
        for (ix, name) in ordered.iter().enumerate() {
            if charset_encode(i, name, ch).is_some() {
                winner = Some(name.clone());
                break;
            }
            if ix + 1 == boundary && ch <= 0x10FFFF {
                winner = Some("unicode".to_string());
                break;
            }
        }
        let winner = winner.unwrap_or_else(|| {
            if ch <= 0x3FFF7F {
                "emacs".to_string()
            } else {
                "eight-bit".to_string()
            }
        });
        if default_order {
            i.char_charset_cache.borrow_mut().insert(ch, winner.clone());
        }
        return Some(winner);
    }
    order
        .iter()
        .find(|name| charset_encode(i, name, ch).is_some())
        .cloned()
}

fn f_charset_priority_list(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU Fcharset_priority_list: a non-nil HIGHEST arg
    // returns just the HIGHEST-PRIORITY-CHARSET itself (a symbol), not a
    // list — regardless of what the arg names.
    let names = charsets_ordered(i);
    if !arg(&a, 0).is_nil() {
        return Ok(symv(i, &names[0]));
    }
    Ok(Value::list(names.iter().map(|n| symv(i, n)).collect()))
}

fn f_sort_charsets(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU sorts by position in `Vcharset_ordered_list'.
    let items = super::want_list(i, &a[0])?;
    let order = charsets_ordered(i);
    let mut keyed: Vec<(usize, Value)> = Vec::with_capacity(items.len());
    for v in items.into_iter() {
        let pos = match &v {
            Value::Sym(s) => {
                let n = canonical_charset(i, &i.symbol_name(*s));
                order.iter().position(|o| *o == n).unwrap_or(usize::MAX)
            }
            _ => usize::MAX,
        };
        keyed.push((pos, v));
    }
    keyed.sort_by_key(|(p, _)| *p);
    Ok(Value::list(keyed.into_iter().map(|(_, v)| v).collect()))
}

fn f_set_charset_priority(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU: each arg must be a charset; args present in the ordered
    // list move to the head in argument order; the rest of the old
    // list (which becomes the non-preferred head) follows.
    let mut new_order: Vec<String> = Vec::with_capacity(a.len());
    let mut old = charsets_ordered(i);
    for v in a.iter() {
        // GNU drops an arg already moved to the new head (Fdelq +
        // memq): duplicate arguments produce a single entry.
        let n = want_charset(i, v)?;
        if let Some(pos) = old.iter().position(|x| *x == n) {
            old.remove(pos);
            new_order.push(n);
        }
    }
    let boundary = new_order.len();
    new_order.append(&mut old);
    *i.charset_order.borrow_mut() = (new_order, boundary);
    invalidate_char_charset_cache(i);
    Ok(Value::Nil)
}

/// GNU `check_iso_charset_parameter': fixnum DIMENSION (1..3) and
/// CHARS (94→false / 96→true), CHARACTERP FINAL-CHAR in `0'..`~'.
/// Returns the chars-96 flag used as the table index.
fn iso_check(i: &mut Interp, a: &[Value]) -> Result<(i64, bool, i64), Flow> {
    let dim = crate::lisp::builtins::want_fixnum(i, &a[0])? as i64;
    let chars = crate::lisp::builtins::want_fixnum(i, &a[1])? as i64;
    let fc = match &a[2] {
        Value::Int(n) if (0..=0x3f_ffff).contains(n) => *n as i64,
        other => return Err(i.wrong_type_mut("characterp", other)),
    };
    if !(1..=3).contains(&dim) {
        return Err(i.error(format!("Invalid DIMENSION {dim}, it should be 1, 2, or 3")));
    }
    let chars96 = match chars {
        94 => false,
        96 => true,
        _ => return Err(i.error(format!("Invalid CHARS {chars}, it should be 94 or 96"))),
    };
    if !(48..=126).contains(&fc) {
        let c = char::from_u32(fc as u32).unwrap_or('\u{fffd}');
        return Err(i.error_q(
            "Invalid FINAL-CHAR `%s', it should be `0'..`~'",
            &[&c.to_string()],
        ));
    }
    Ok((dim, chars96, fc))
}

/// The `chars' flag of a charset: GNU `iso_chars_96' is
/// `code_space[2] == 96' — the character count of the LAST dimension
/// (`:code-space' is LSB-first pairs).  `dims' here is MSB-first, so
/// the last dim's span is the flag.
fn iso_chars_96(i: &mut Interp, plist: &Value) -> bool {
    if let Some(dims) = charset_dims(i, plist) {
        if let Some(&(lo, hi)) = dims.last() {
            return hi - lo + 1 == 96;
        }
    }
    false
}

/// `iso-charset' + `set-iso-charset' lookup: every charset whose
/// `:iso-final-char' matches FC, `:dimension' matches DIM, and whose
/// chars flag matches — last definition wins; explicit
/// `iso_charset_overrides' entries win over all.
fn iso_charset_lookup(i: &mut Interp, dim: i64, chars96: bool, fc: i64) -> Option<String> {
    let mut found: Option<String> = None;
    for name in charsets_ordered(i) {
        let plist = effective_charset_plist(i, &name);
        let fin = prop_int(&charset_prop(i, &plist, ":iso-final-char"));
        if fin != Some(fc) {
            continue;
        }
        let pdim = prop_int(&charset_prop(i, &plist, ":dimension"))
            .or_else(|| charset_dims(i, &plist).map(|d| d.len() as i64));
        if pdim != Some(dim) || iso_chars_96(i, &plist) != chars96 {
            continue;
        }
        found = Some(name);
    }
    if let Some((_, _, _, n)) = i
        .iso_charset_overrides
        .iter()
        .rev()
        .find(|(d, c, f, _)| *d == dim && *c == chars96 && *f == fc)
    {
        found = Some(n.clone());
    }
    found
}

fn f_iso_charset(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU `Fiso_charset': (DIMENSION
    // CHARS FINAL-CHAR) indexes iso_charset_table, NOT a coding system.
    let (dim, chars96, fc) = iso_check(i, &a)?;
    match iso_charset_lookup(i, dim, chars96, fc) {
        Some(name) => Ok(symv(i, &name)),
        None => Ok(Value::Nil),
    }
}

fn f_get_unused_iso_final_char(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU `Fget_unused_iso_final_char': first `0'..`?' whose slot is empty.
    let (dim, chars96, _) = iso_check(i, &[a[0].clone(), a[1].clone(), Value::Int(48)])?;
    for fc in 48i64..=63 {
        if iso_charset_lookup(i, dim, chars96, fc).is_none() {
            return Ok(Value::Int(fc as i128));
        }
    }
    Ok(Value::Nil)
}

// NB: GNU's Fset_iso_charset exists in charset.c but is NOT bound in
// this 31.1 -Q build — remacs mirrors the unbound state.

fn f_map_charset_chars(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // (map-charset-chars FUNCTION CHARSET &optional ARG FROM TO)
    // GNU calls FUNCTION once per run of contiguous Emacs characters
    // with `(FROM . TO)' and ARG.  FROM/TO bound the *charset code
    // points* visited (big-endian packed per the :code-space dims),
    // clamped to the charset's min/max code like GNU's
    // Fmap_charset_chars.
    let name = want_charset(i, &a[1])?;
    // Resolve FUNCTION's symbol cell once: per-range `apply' would
    // redo the alias/`indirect_function' walk thousands of times
    // inside characters.el's category seeding.
    let function = match &a[0] {
        Value::Sym(_) => {
            let f = i.indirect_function_value(&a[0]);
            if matches!(f, Value::Sym(s) if s == crate::lisp::sym::UNBOUND) {
                return Err(i.signal_data(crate::lisp::sym::VOID_FUNCTION, vec![a[0].clone()]));
            }
            f
        }
        other => other.clone(),
    };
    let arg = a.get(2).cloned().unwrap_or(Value::Nil);
    let mut read_bound = |v: Option<&Value>| -> Result<Option<i64>, Flow> {
        match v {
            None | Some(Value::Nil) => Ok(None),
            Some(Value::Int(n)) if *n >= 0 => Ok(Some(*n as i64)),
            Some(v) => Err(i.wrong_type_mut("wholenump", v)),
        }
    };
    let from_req = read_bound(a.get(3))?;
    let to_req = read_bound(a.get(4))?;
    let plist = charset_entry(i, &name)
        .map(|e| e.1.clone())
        .unwrap_or(Value::Nil);
    let dims = charset_dims(i, &plist).unwrap_or_else(|| vec![(0, 127)]);
    let pack = |sel: usize| -> i64 {
        dims.iter()
            .fold(0i64, |c, d| (c << 8) | if sel == 0 { d.0 } else { d.1 })
    };
    // GNU Fmap_charset_chars clamps the arguments to min/max code.
    let from = from_req.unwrap_or_else(|| pack(0)).max(pack(0));
    let to = to_req.unwrap_or_else(|| pack(1)).min(pack(1));
    map_cc(i, &name, from, to, &function, &arg, 0)
}

/// GNU `map_charset_chars': recurse through :subset/:superset
/// composition and, at a leaf charset, walk the encoder (ascending
/// *characters*) emitting maximal contiguous-char runs — or, for
/// code-offset charsets, emit the single (OFFSET+IDX . OFFSET+IDX)
/// range.  FROM/TO of None mean the charset's min/max code.
fn map_cc(
    i: &mut Interp,
    name: &str,
    from: i64,
    to: i64,
    function: &Value,
    arg: &Value,
    depth: u8,
) -> EvalResult {
    if depth > 8 {
        return Ok(Value::Nil);
    }
    let name = canonical_charset(i, name);
    let plist = charset_entry(i, &name)
        .map(|e| e.1.clone())
        .unwrap_or(Value::Nil);
    let dims = charset_dims(i, &plist).unwrap_or_else(|| vec![(0, 127)]);
    let pack = |sel: usize| -> i64 {
        dims.iter()
            .fold(0i64, |c, d| (c << 8) | if sel == 0 { d.0 } else { d.1 })
    };
    // GNU does not re-clamp from/to on internal recursion — they are
    // used as-is for the `partial' test (own min/max) and the emit.
    let min_code = pack(0);
    let max_code = pack(1);
    let partial = from > min_code || to < max_code;

    // :subset — adjust the window into parent code space and recurse.
    if let Some((parent, pmin, pmax, off)) = subset_spec(i, &plist) {
        let f = (from - off).max(pmin);
        let t = (to - off).min(pmax);
        return map_cc(i, &parent, f, t, function, arg, depth + 1);
    }
    // :superset — one recursion per parent with offset subtraction;
    // GNU clamps each window to the parent's min/max code.
    let supers = superset_list(i, &plist);
    if !supers.is_empty() {
        for (pname, off) in supers {
            let mut this_from = if from > off { from - off } else { 0 };
            let mut this_to = if to > off { to - off } else { 0 };
            let pplist = charset_entry(i, &pname)
                .map(|e| e.1.clone())
                .unwrap_or(Value::Nil);
            if let Some(pd) = charset_dims(i, &pplist) {
                let ppack = |sel: usize| -> i64 {
                    pd.iter()
                        .fold(0i64, |c, d| (c << 8) | if sel == 0 { d.0 } else { d.1 })
                };
                this_from = this_from.max(ppack(0));
                this_to = this_to.min(ppack(1));
            }
            map_cc(i, &pname, this_from, this_to, function, arg, depth + 1)?;
        }
        return Ok(Value::Nil);
    }

    let unified = i
        .charset_maps
        .borrow()
        .get(&format!("charset:{name}"))
        .cloned();
    let off = prop_int(&charset_prop(i, &plist, ":code-offset"));
    let table = unified
        .clone()
        .or_else(|| map_value_table(i, &name, &charset_prop(i, &plist, ":map")));
    // Fast path: characters.el calls `map-charset-chars' with
    // #'modify-category-entry ~4k times per charset.  When FUNCTION
    // resolved to that subr and ARG is a valid category char, write
    // through a pre-validated `MceFast' (one table/defalt check for
    // the whole walk, shared old→new cache).  Any deviation — advice,
    // alias, non-integer ARG, explicit-table callers — falls back to
    // `apply', preserving GNU's semantics and error behavior.
    let mut mce_fast = crate::lisp::builtins::misc::mce_fast_for(i, function, arg);
    let mut emit = |i: &mut Interp, lo: i64, hi: i64| -> EvalResult {
        if let Some(mf) = &mut mce_fast {
            mf.emit(i, lo, hi);
            return Ok(Value::Nil);
        }
        let range = Value::cons(Value::Int(lo as i128), Value::Int(hi as i128));
        i.apply(function, vec![range, arg.clone()])
    };
    if let Some(t) = &table {
        // Encoder walk: chars ascending, maximal runs of contiguous
        // encodable chars (filtered by code ∈ [from,to] when partial).
        // Each enc entry (LO,HI,BASE) maps chars LO..=HI to codes
        // BASE..=BASE+(HI-LO); the in-window subrange is arithmetic.
        let mut run_start: Option<i64> = None;
        let mut last: i64 = -2;
        for &(lo_c, hi_c, base) in &t.enc {
            let (lo, hi) = if partial {
                let lo_cut = (from - base).max(0);
                let hi_cut = (to - base).min(hi_c - lo_c);
                if hi_cut < lo_cut {
                    continue;
                }
                (lo_c + lo_cut, lo_c + hi_cut)
            } else {
                (lo_c, hi_c)
            };
            if lo == last + 1 {
                last = hi;
            } else {
                if let Some(s) = run_start.take() {
                    emit(i, s, last)?;
                }
                run_start = Some(lo);
                last = hi;
            }
        }
        if let Some(s) = run_start.take() {
            emit(i, s, last)?;
        }
    }
    // GNU emits the code-offset range for OFFSET-method charsets —
    // always for a unified charset (mapped chars above plus the
    // private-char window), or as the sole output otherwise.
    if let Some(o) = off {
        if table.is_none() || unified.is_some() {
            // GNU code_linear_p: `dimension' drops most-significant
            // zero-width dims; linear when 1 dim remains or every dim
            // below the top one spans 256.  For linear charsets the
            // index is `code - min_code' unconditionally (no per-dim
            // range check — degenerate ranges like (144 . 127) still
            // get emitted).
            let sig = {
                let s = dims.iter().position(|d| d.1 > 0).unwrap_or(dims.len());
                if s >= dims.len() {
                    &dims[..]
                } else {
                    &dims[s..]
                }
            };
            let linear = sig.len() == 1 || sig[1..].iter().all(|d| d.1 - d.0 == 255);
            let fi = if linear {
                Some(from - min_code)
            } else {
                linear_index(&dims, from)
            };
            let ti = if linear {
                Some(to - min_code)
            } else {
                linear_index(&dims, to)
            };
            if let (Some(fi), Some(ti)) = (fi, ti) {
                emit(i, o + fi, o + ti)?;
            }
        }
    }
    if table.is_none() && off.is_none() {
        // Charset with no map/offset plist data (internal charsets
        // like `unicode'): enumerate the code space in code order,
        // grouping contiguous decoded characters.
        let los: Vec<i64> = dims.iter().map(|d| d.0).collect();
        let his: Vec<i64> = dims.iter().map(|d| d.1).collect();
        let mut digits = los.clone();
        let mut run_start: Option<i64> = None;
        let mut last_char: i64 = -2;
        loop {
            let code = digits.iter().fold(0i64, |c, &d| (c << 8) | d);
            let mut ok = false;
            if code >= from && code <= to {
                if let Some(c) = charset_decode(i, &name, code) {
                    ok = true;
                    if c != last_char + 1 {
                        if let Some(s) = run_start.take() {
                            emit(i, s, last_char)?;
                        }
                        run_start = Some(c);
                    }
                    last_char = c;
                }
            }
            if !ok {
                if let Some(s) = run_start.take() {
                    emit(i, s, last_char)?;
                }
                last_char = -2;
            }
            let mut d = dims.len();
            loop {
                if d == 0 {
                    if let Some(s) = run_start.take() {
                        emit(i, s, last_char)?;
                    }
                    return Ok(Value::Nil);
                }
                d -= 1;
                if digits[d] < his[d] {
                    digits[d] += 1;
                    break;
                }
                digits[d] = los[d];
            }
        }
    }
    Ok(Value::Nil)
}

fn f_declare_equiv_charset(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU Fdeclare_equiv_charset (DIMENSION CHARS FINAL-CHAR CHARSET):
    // the charset arg is validated first, then check_iso_charset_parameter.
    let _ = want_charset(i, &a[3])?;
    match &a[0] {
        Value::Int(d) => {
            if !(1..=3).contains(d) {
                return Err(i.error(format!("Invalid DIMENSION {}, it should be 1, 2, or 3", d)));
            }
        }
        other => return Err(i.wrong_type_mut("fixnump", other)),
    }
    match &a[1] {
        Value::Int(c) => {
            if *c != 94 && *c != 96 {
                return Err(i.error(format!("Invalid CHARS {}, it should be 94 or 96", c)));
            }
        }
        other => return Err(i.wrong_type_mut("fixnump", other)),
    }
    match &a[2] {
        Value::Int(f) => {
            if !('0' as i128..='~' as i128).contains(f) {
                return Err(i.error(format!(
                    "Invalid FINAL-CHAR `{}', it should be `0'..`~'",
                    char::from_u32(*f as u32).unwrap_or('\u{fffd}')
                )));
            }
        }
        other => return Err(i.wrong_type_mut("characterp", other)),
    }
    Ok(Value::Nil)
}

fn f_clear_charset_maps(_i: &mut Interp, _a: crate::lisp::Argv) -> EvalResult {
    Ok(Value::Nil)
}

/// Entry `(prop . TABLE)' in `char-code-property-alist', or nil.
fn prop_alist_entry(i: &mut Interp, prop: &str) -> Value {
    let vid = i.intern("char-code-property-alist");
    let alist = i.symbol_value(vid);
    for e in alist.list_to_vec().unwrap_or_default() {
        if let Value::Cons(c) = &e {
            let (k, _) = {
                let cc = c.borrow();
                (cc.car.clone(), cc.cdr.clone())
            };
            if let Value::Sym(s) = &k {
                if i.symbol_name(*s) == prop {
                    return e;
                }
            }
        }
    }
    Value::Nil
}

/// Materialize PROP's table: GNU loads the backing file, which
/// re-registers the property with a real char-table.  When the file
/// is absent or does not self-register, return the file-name string —
/// GNU's `uniprop_table' likewise leaves the alist cdr alone so a
/// later attempt can retry the load.
fn install_prop_table(i: &mut Interp, prop: &str) -> Value {
    load_file_backed_prop(i, prop);
    sync_prop_table(i, prop);
    let found = i
        .char_code_prop_tables
        .iter()
        .find(|(n, _)| n == prop)
        .map(|(_, t)| t.clone());
    if let Some(t) = found {
        if is_char_table(i, &t) {
            let entry = prop_alist_entry(i, prop);
            if let Value::Cons(c) = entry {
                c.borrow_mut().cdr = t.clone();
            }
        }
        return t;
    }
    match prop_alist_entry(i, prop) {
        Value::Cons(c) => c.borrow().cdr.clone(),
        _ => Value::Nil,
    }
}

/// char-table for PROP — GNU's `Funicode_property_table_internal':
/// consult `char-code-property-alist' and return nil for unknown
/// properties; a string cdr names a data file that loads into a table.
pub(crate) fn unicode_prop_table(i: &mut Interp, prop: &str) -> Value {
    let dbg = i.dbg_flags & 0x100000 != 0;
    if let Some((_, t)) = i.char_code_prop_tables.iter().find(|(n, _)| n == prop) {
        return match t {
            t if is_char_table(i, t) => {
                if dbg {
                    eprintln!("[uniprop] {prop}: cache hit table");
                }
                t.clone()
            }
            // File-backed registration: GNU loads the file, which builds
            // the table via `define-char-code-property'/`put-unicode-
            // property-internal'; we materialize the (empty) table.
            Value::Str(_) => {
                if dbg {
                    eprintln!("[uniprop] {prop}: cache Str -> install");
                }
                let v = install_prop_table(i, prop);
                if dbg {
                    let tn = match &v {
                        Value::Nil => "nil",
                        Value::Str(_) => "str",
                        Value::Cons(_) => "cons",
                        Value::Vec(_) => "vec",
                        Value::Sym(_) => "sym",
                        _ => "other",
                    };
                    eprintln!(
                        "[uniprop] {prop}: install -> {tn} char-table={}",
                        is_char_table(i, &v)
                    );
                }
                v
            }
            _ => t.clone(),
        };
    }
    if dbg {
        eprintln!("[uniprop] {prop}: no cache entry");
    }
    match prop_alist_entry(i, prop) {
        Value::Cons(c) => {
            let cdr = c.borrow().cdr.clone();
            if dbg {
                eprintln!("[uniprop] {prop}: alist cdr hit");
            }
            match cdr {
                v if is_char_table(i, &v) => {
                    i.char_code_prop_tables.push((prop.to_string(), v.clone()));
                    v
                }
                Value::Str(_) => install_prop_table(i, prop),
                _ => cdr,
            }
        }
        _ => Value::Nil,
    }
}

fn f_unicode_property_table_internal(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU: non-symbol PROP → nil (chartab.c returns Qnil, no check).
    let Some(pid) = i.sym_id(&a[0]) else {
        return Ok(Value::Nil);
    };
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

fn f_get_unicode_property_internal(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    if !is_char_table(i, &a[0]) {
        return Err(i.wrong_type_mut("char-table-p", &a[0]));
    }
    let ch = match &a[1] {
        Value::Int(n) => *n as i64,
        _ => return Err(i.wrong_type_mut("characterp", &a[1])),
    };
    // GNU `get_unicode_property': CHAR_TABLE_REF (which lazily
    // uncompresses leaf strings) plus the C decoder selected by a
    // fixnum in extra slot 1.  A Lisp decoder there is NOT applied
    // here — only `get-char-code-property' funcalls it.
    let raw = misc::char_table_ref(i, &a[0], ch.max(0) as usize);
    let val = match ct_extra(&a[0], 1) {
        Value::Int(0) => uniprop_decode_run_length(&a[0], raw),
        _ => raw,
    };
    if val.is_nil() {
        if let Some(prop) = unicode_table_prop(i, &a[0]) {
            // An absent value shows the built-in default, matching
            // GNU where every property slot is populated.
            return Ok(builtin_char_prop(i, ch, &prop));
        }
    }
    Ok(val)
}

fn f_put_unicode_property_internal(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    if !is_char_table(i, &a[0]) {
        return Err(i.wrong_type_mut("char-table-p", &a[0]));
    }
    let ch = match &a[1] {
        Value::Int(n) if (0..=misc::CT_MAX_CHAR as i128).contains(n) => *n as u32,
        _ => return Err(i.wrong_type_mut("characterp", &a[1])),
    };
    put_unicode_internal(i, &a[0], ch, a[2].clone())
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
        2,
        f_find_charset_string,
        "Charsets needed for STRING."
    ),
    S!(
        "find-charset-region",
        2,
        3,
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
        3,
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
        // GNU mule.el: (symbol &rest args) → (1 . many).
        many 1,
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
        2,
        f_set_keyboard_coding_system_internal,
        "Set keyboard coding system."
    ),
    S!(
        "find-coding-systems-region-internal",
        2,
        3,
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
    if code < 0 {
        return None;
    }
    let u = code as u32;
    match name {
        "ascii" => (0..=0x7F).contains(&code).then_some(code),
        // GNU's eight-bit charsets are code-offset charsets onto the
        // internal raw-byte range 0x3FFF80..=0x3FFFFF.
        "eight-bit" => (0x80..=0xFF).contains(&code).then(|| 0x3FFF00 + code),
        "eight-bit-control" => (0x80..=0x9F).contains(&code).then(|| 0x3FFF00 + code),
        "eight-bit-graphic" => (0xA0..=0xFF).contains(&code).then(|| 0x3FFF00 + code),
        // GNU `iso-8859-1': code-space [0,255], code-offset 0.
        "iso-8859-1" => (0..=0xFF).contains(&code).then_some(code),
        // GNU `control-1': code-space [128,159], code-offset 128.
        "control-1" => (0x80..=0x9F).contains(&code).then_some(code),
        "unicode" | "ucs" => (code <= 0x10FFFF).then_some(code),
        // GNU `emacs' charset covers all internal chars except the
        // eight-bit raw-byte range (max char = MAX_5_BYTE_CHAR).
        "emacs" => (code <= 0x3FFF7F).then_some(code),
        "big5" => big5_decode(u),
        "japanese-jisx0208" => jisx0208_decode(u),
        // `japanese-jisx0212', `japanese-jisx0208-1978', `jisx0201',
        // subset/superset charsets and the map-file charsets resolve
        // through the generic plist path below.
        _ => None,
    }
}

/// `decode-char'/`map-charset-chars' entry point: hardcoded tables for
/// the CJK staples, then the generic plist-driven resolution
/// (`:map'/`:unify-map' tables, `:subset'/`:superset' composition,
/// `:code-offset' linear indexing) like GNU `DECODE_CHAR'.
pub(crate) fn charset_decode(i: &Interp, name: &str, code: i64) -> Option<i64> {
    charset_decode_at(i, name, code, 0)
}

fn charset_decode_at(i: &Interp, name: &str, code: i64, depth: u8) -> Option<i64> {
    if depth > 8 {
        return None;
    }
    let name = canonical_charset(i, name);
    // The five C builtins carry their decode semantics (incl. the
    // max-code bound, e.g. emacs' MAX_5_BYTE_CHAR) in
    // `decode_charset_code'; their synthesized attribute plists lack
    // min/max-code, so the plist path must not widen them.
    if matches!(
        name.as_str(),
        "ascii" | "iso-8859-1" | "unicode" | "ucs" | "emacs" | "eight-bit"
    ) {
        return decode_charset_code(&name, code);
    }
    decode_charset_code(&name, code).or_else(|| charset_decode_plist(i, &name, code, depth))
}

/// A charset map table parsed from `etc/charsets/*.map': each entry
/// (LO,HI,BASE) maps codes LO..=HI to chars BASE..=BASE+(HI-LO).
/// `dec' is sorted by LO for decode lookup, `enc' by char BASE for
/// encode lookup.
pub(crate) struct CharsetTable {
    dec: Vec<(i64, i64, i64)>,
    enc: Vec<(i64, i64, i64)>,
}

fn table_lookup(tab: &[(i64, i64, i64)], x: i64) -> Option<i64> {
    let mut lo = 0usize;
    let mut hi = tab.len();
    while lo < hi {
        let mid = (lo + hi) / 2;
        let (l, h, b) = tab[mid];
        if x < l {
            hi = mid;
        } else if x > h {
            lo = mid + 1;
        } else {
            return Some(b + (x - l));
        }
    }
    None
}

fn parse_charset_map(text: &str) -> CharsetTable {
    let hex = |s: &str| -> Option<i64> {
        let s = s
            .trim()
            .strip_prefix("0x")
            .or_else(|| s.trim().strip_prefix("0X"))?;
        i64::from_str_radix(s, 16).ok()
    };
    let mut dec = Vec::new();
    let mut enc = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let mut it = line.split_whitespace();
        let (Some(a), Some(b)) = (it.next(), it.next()) else {
            continue;
        };
        let (lo, hi) = match a.split_once('-') {
            Some((l, h)) => (hex(l), hex(h)),
            None => (hex(a), hex(a)),
        };
        let (Some(lo), Some(hi), Some(base)) = (lo, hi, hex(b)) else {
            continue;
        };
        dec.push((lo, hi, base));
        enc.push((base, base + (hi - lo), lo));
    }
    dec.sort_unstable();
    enc.sort_unstable();
    CharsetTable { dec, enc }
}

/// Candidate dirs for `etc/charsets': next to every `lisp' dir that
/// `builtin_dirs' would use (repo checkout or install prefix), plus
/// the compile-time manifest dir so the dev binary finds the maps
/// regardless of cwd.
fn charset_map_dirs() -> Vec<std::path::PathBuf> {
    let mut v: Vec<std::path::PathBuf> = crate::lisp::load::builtin_dirs()
        .iter()
        .map(|lispdir| {
            std::path::Path::new(lispdir)
                .parent()
                .unwrap_or_else(|| std::path::Path::new("."))
                .join("etc/charsets")
        })
        .collect();
    v.push(std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("etc/charsets"));
    v
}

/// Load (and cache) `etc/charsets/NAME.map'.  A missing file caches an
/// empty table so we don't rescan per character.
fn charset_table(i: &Interp, mapname: &str) -> Option<Rc<CharsetTable>> {
    if let Some(t) = i.charset_maps.borrow().get(mapname) {
        return if t.dec.is_empty() {
            None
        } else {
            Some(t.clone())
        };
    }
    for d in charset_map_dirs() {
        let p = d.join(format!("{mapname}.map"));
        if let Ok(text) = std::fs::read_to_string(&p) {
            let t = Rc::new(parse_charset_map(&text));
            i.charset_maps
                .borrow_mut()
                .insert(mapname.to_string(), t.clone());
            return Some(t);
        }
    }
    i.charset_maps.borrow_mut().insert(
        mapname.to_string(),
        Rc::new(CharsetTable {
            dec: Vec::new(),
            enc: Vec::new(),
        }),
    );
    None
}

fn charset_prop(i: &Interp, plist: &Value, key: &str) -> Value {
    match i.intern_soft(key) {
        Some(id) => crate::lisp::eval::plist_get(plist, id),
        None => Value::Nil,
    }
}

fn prop_int(v: &Value) -> Option<i64> {
    v.int().map(|n| n as i64)
}

/// `:code-space' vector → per-dimension (lo,hi) pairs.  GNU lists the
/// LEAST significant byte's dimension first (`code_space[0..2]' is the
/// low byte); we return dims most-significant-first for packed codes.
fn charset_dims(i: &Interp, plist: &Value) -> Option<Vec<(i64, i64)>> {
    match charset_prop(i, plist, ":code-space") {
        Value::Vec(v) => {
            let v = v.borrow();
            let mut out: Vec<(i64, i64)> = v
                .chunks(2)
                .filter_map(|p| match (p[0].int(), p[1].int()) {
                    (Some(lo), Some(hi)) => Some((lo as i64, hi as i64)),
                    _ => None,
                })
                .collect();
            out.reverse();
            (!out.is_empty()).then_some(out)
        }
        _ => None,
    }
}

/// GNU `emacs_mule_char' index: position of packed CODE within the
/// multi-dimensional code space (big-endian digits, row-major).
fn linear_index(dims: &[(i64, i64)], code: i64) -> Option<i64> {
    let nd = dims.len();
    let mut idx = 0i64;
    for (k, &(lo, hi)) in dims.iter().enumerate() {
        let shift = 8 * (nd - 1 - k);
        let d = (code >> shift) & 0xff;
        if d < lo || d > hi {
            return None;
        }
        idx = idx * (hi - lo + 1) + (d - lo);
    }
    Some(idx)
}

/// Inverse of `linear_index'.
fn unlinear_index(dims: &[(i64, i64)], mut idx: i64) -> Option<i64> {
    let nd = dims.len();
    let mut digits = vec![0i64; nd];
    for k in (0..nd).rev() {
        let (lo, hi) = dims[k];
        let span = hi - lo + 1;
        let d = lo + idx % span;
        idx /= span;
        digits[k] = d;
    }
    if idx != 0 {
        return None;
    }
    let mut code = 0i64;
    for &d in &digits {
        code = (code << 8) | d;
    }
    Some(code)
}

/// `(PARENT MIN-CODE MAX-CODE OFFSET)' from a `:subset' property.
fn subset_spec(i: &Interp, plist: &Value) -> Option<(String, i64, i64, i64)> {
    let spec = charset_prop(i, plist, ":subset");
    let items = spec.list_to_vec().ok()?;
    if items.len() != 4 {
        return None;
    }
    let parent = match &items[0] {
        Value::Sym(s) => i.symbol_name(*s),
        _ => return None,
    };
    Some((
        parent,
        prop_int(&items[1])?,
        prop_int(&items[2])?,
        prop_int(&items[3])?,
    ))
}

/// `(P1 P2 (P3 . OFF) ...)' from a `:superset' property — each entry
/// is a bare parent symbol (offset 0) or a (PARENT . OFFSET) cons.
fn superset_list(i: &Interp, plist: &Value) -> Vec<(String, i64)> {
    let spec = charset_prop(i, plist, ":superset");
    let Ok(items) = spec.list_to_vec() else {
        return Vec::new();
    };
    items
        .iter()
        .filter_map(|e| match e {
            Value::Sym(s) => Some((i.symbol_name(*s), 0i64)),
            Value::Cons(c) => {
                let b = c.borrow();
                match (&b.car, &b.cdr) {
                    (Value::Sym(s), off) => Some((i.symbol_name(*s), prop_int(off)?)),
                    _ => None,
                }
            }
            _ => None,
        })
        .collect()
}

/// Generic plist-driven `DECODE_CHAR': installed unify map → `:map'
/// table → `:subset'/`:superset' composition → `:code-offset'.
fn charset_decode_plist(i: &Interp, name: &str, code: i64, depth: u8) -> Option<i64> {
    if depth > 8 || code < 0 {
        return None;
    }
    let plist = charset_entry(i, name)
        .map(|e| e.1.clone())
        .unwrap_or(Value::Nil);
    // GNU DECODE_CHAR: reject codes outside [MIN-CODE, MAX-CODE] first;
    // these default to the packed :code-space extremes and can be
    // narrowed by :min-code/:max-code.
    if let Some(dims) = charset_dims(i, &plist) {
        let (mut min, mut max) = (0i64, 0i64);
        for &(lo, hi) in &dims {
            min = (min << 8) | lo;
            max = (max << 8) | hi;
        }
        if let Some(v) = prop_int(&charset_prop(i, &plist, ":min-code")) {
            min = v;
        }
        if let Some(v) = prop_int(&charset_prop(i, &plist, ":max-code")) {
            max = v;
        }
        if code < min || code > max {
            return None;
        }
    }
    // Map installed by `unify-charset' wins over :code-offset (GNU
    // semantics: a unified charset decodes via its map, unmapped
    // in-space codes fall back to the private char).
    let unified = i
        .charset_maps
        .borrow()
        .get(&format!("charset:{name}"))
        .cloned();
    if let Some(t) = unified {
        if let Some(c) = table_lookup(&t.dec, code) {
            return Some(c);
        }
    } else if let Some(t) = map_value_table(i, &name, &charset_prop(i, &plist, ":map")) {
        if let Some(c) = table_lookup(&t.dec, code) {
            return Some(c);
        }
        // Map-method charset, unmapped code → nil (no offset
        // fallback in GNU for non-unified map charsets).
        if prop_int(&charset_prop(i, &plist, ":code-offset")).is_none() {
            return None;
        }
    }
    if let Some((parent, min, max, off)) = subset_spec(i, &plist) {
        let pc = code - off;
        if (min..=max).contains(&pc) {
            return charset_decode_at(i, &parent, pc, depth + 1);
        }
        return None;
    }
    let supers = superset_list(i, &plist);
    for (pname, off) in &supers {
        if let Some(c) = charset_decode_at(i, pname, code - off, depth + 1) {
            return Some(c);
        }
    }
    if !supers.is_empty() {
        return None;
    }
    if let Some(off) = prop_int(&charset_prop(i, &plist, ":code-offset")) {
        let dims = charset_dims(i, &plist)?;
        return linear_index(&dims, code).map(|idx| off + idx);
    }
    None
}

/// Mirror of `charset_decode_plist' for `ENCODE_CHAR'.
fn charset_encode_plist(i: &Interp, name: &str, ch: i64, depth: u8) -> Option<i64> {
    if depth > 8 {
        return None;
    }
    let plist = charset_entry(i, name)
        .map(|e| e.1.clone())
        .unwrap_or(Value::Nil);
    // GNU ENCODE_CHAR order: deunify (unified offset charsets map the
    // char through the installed unify map) → subset → superset →
    // map/offset leaf.
    let unified = i
        .charset_maps
        .borrow()
        .get(&format!("charset:{name}"))
        .cloned();
    if let Some(t) = unified {
        if let Some(c) = table_lookup(&t.enc, ch) {
            return Some(c);
        }
    }
    if let Some((parent, min, max, off)) = subset_spec(i, &plist) {
        if let Some(pc) = charset_encode_at(i, &parent, ch, depth + 1) {
            if (min..=max).contains(&pc) {
                return Some(pc + off);
            }
        }
        return None;
    }
    let supers = superset_list(i, &plist);
    for (pname, off) in &supers {
        if let Some(c) = charset_encode_at(i, pname, ch, depth + 1) {
            return Some(c + off);
        }
    }
    if !supers.is_empty() {
        return None;
    }
    if let Some(off) = prop_int(&charset_prop(i, &plist, ":code-offset")) {
        // GNU OFFSET method: CH must lie in [MIN_CHAR, MAX_CHAR] =
        // OFFSET + index(MIN-CODE) .. OFFSET + index(MAX-CODE), then
        // the index maps back to a code point.
        let dims = charset_dims(i, &plist)?;
        let (mut min_code, mut max_code) = (0i64, 0i64);
        for &(lo, hi) in &dims {
            min_code = (min_code << 8) | lo;
            max_code = (max_code << 8) | hi;
        }
        if let Some(v) = prop_int(&charset_prop(i, &plist, ":min-code")) {
            min_code = v;
        }
        if let Some(v) = prop_int(&charset_prop(i, &plist, ":max-code")) {
            max_code = v;
        }
        let lo_idx = linear_index(&dims, min_code).unwrap_or(0);
        let hi_idx = linear_index(&dims, max_code).unwrap_or(i64::MAX);
        let idx = ch - off;
        if idx < lo_idx || idx > hi_idx {
            return None;
        }
        return unlinear_index(&dims, idx);
    }
    if let Some(t) = map_value_table(i, &name, &charset_prop(i, &plist, ":map")) {
        return table_lookup(&t.enc, ch);
    }
    None
}

/// `encode-char CH CHARSET` semantics: nil when CH has no code in CHARSET.
pub(crate) fn encode_charset_code(name: &str, ch: i64) -> Option<i64> {
    if !(0..0x400000).contains(&ch) {
        return None;
    }
    let u = ch as u32;
    match name {
        "ascii" => (ch < 0x80).then_some(ch),
        "eight-bit" => (0x3FFF80..=0x3FFFFF).contains(&ch).then(|| ch - 0x3FFF00),
        "eight-bit-control" => (0x3FFF80..=0x3FFFBF).contains(&ch).then(|| ch - 0x3FFF00),
        "eight-bit-graphic" => (0x3FFFC0..=0x3FFFFF).contains(&ch).then(|| ch - 0x3FFF00),
        "iso-8859-1" => (ch < 0x100).then_some(ch),
        "control-1" => (0x80..=0x9F).contains(&ch).then_some(ch),
        "unicode" | "ucs" => (ch <= 0x10FFFF).then_some(ch),
        "emacs" => (ch <= 0x3FFF7F).then_some(ch),
        "big5" => big5_encode(u),
        "japanese-jisx0208" => jisx0208_encode(u),
        _ => None,
    }
}

/// `encode-char' entry point: hardcoded tables, then the plist-driven
/// resolution (`charset_decode's mirror image).
pub(crate) fn charset_encode(i: &Interp, name: &str, ch: i64) -> Option<i64> {
    charset_encode_at(i, name, ch, 0)
}

fn charset_encode_at(i: &Interp, name: &str, ch: i64, depth: u8) -> Option<i64> {
    if depth > 8 {
        return None;
    }
    let name = canonical_charset(i, name);
    // Same builtin gate as `charset_decode_at': the C builtins' max_char
    // bound lives in `encode_charset_code'; the plist path has no such
    // bound and would wrongly accept codes above it.
    if matches!(
        name.as_str(),
        "ascii" | "iso-8859-1" | "unicode" | "ucs" | "emacs" | "eight-bit"
    ) {
        return encode_charset_code(&name, ch);
    }
    encode_charset_code(&name, ch).or_else(|| charset_encode_plist(i, &name, ch, depth))
}

/// GNU `big5' charset: code-space [64-254][161-254], code-offset
/// 1245184.  In GNU's packing, dim2 (the 94-slot dimension) is the file
/// first byte 0xA1-0xFE and dim1 (191 slots) the second byte 0x40-0xFE.
/// Unmapped in-space codes decode to private chars at code-offset+index.
const BIG5_PRIV_BASE: i64 = 1245184;

fn big5_decode(code: u32) -> Option<i64> {
    use crate::lisp::cjk_tables as cjk;
    let b1 = code >> 8;
    let b2 = code & 0xff;
    if !(0xa1..=0xfe).contains(&b1) || !(0x40..=0xfe).contains(&b2) {
        return None;
    }
    if let Some(u) = tbl_decode(cjk::BIG5_DECODE, code) {
        return Some(u as i64);
    }
    Some(BIG5_PRIV_BASE + (b1 as i64 - 0xa1) * 191 + (b2 as i64 - 0x40))
}

fn big5_encode(ch: u32) -> Option<i64> {
    use crate::lisp::cjk_tables as cjk;
    if let Some(c) = tbl_encode(cjk::BIG5_ENCODE, ch) {
        return Some(c);
    }
    let idx = ch as i64 - BIG5_PRIV_BASE;
    if !(0..94 * 191).contains(&idx) {
        return None;
    }
    Some(((0xa1 + idx / 191) << 8) | (0x40 + idx % 191))
}

/// GNU `japanese-jisx0208': code-space [33-126][33-126], code-offset
/// 1310720.  Unmapped in-space codes decode to private chars.
const JISX0208_PRIV_BASE: i64 = 1310720;

fn jisx0208_decode(code: u32) -> Option<i64> {
    use crate::lisp::cjk_tables as cjk;
    let j1 = code >> 8;
    let j2 = code & 0xff;
    if !(0x21..=0x7e).contains(&j1) || !(0x21..=0x7e).contains(&j2) {
        return None;
    }
    if let Some(u) = tbl_decode(cjk::JISX0208_DECODE, code) {
        return Some(u as i64);
    }
    Some(JISX0208_PRIV_BASE + (j1 as i64 - 0x21) * 94 + (j2 as i64 - 0x21))
}

fn jisx0208_encode(ch: u32) -> Option<i64> {
    use crate::lisp::cjk_tables as cjk;
    if let Some(c) = tbl_encode(cjk::JISX0208_ENCODE, ch) {
        return Some(c);
    }
    let idx = ch as i64 - JISX0208_PRIV_BASE;
    if !(0..94 * 94).contains(&idx) {
        return None;
    }
    Some(((0x21 + idx / 94) << 8) | (0x21 + idx % 94))
}

/// GNU `japanese-jisx0213.2004-1' / `japanese-jisx0213-2': code-space
/// [33-126][33-126], code-offsets 1359872 / 1376256.  GNU's
/// decode-sjis-char uses the shift_jis-2004 charset list, so sjis kanji
/// decodes through JISX0213.2004-1 rather than JISX0208.
const JISX0213_1_PRIV_BASE: i64 = 1359872;
const JISX0213_2_PRIV_BASE: i64 = 1376256;

fn jisx0213_decode(code: u32, base: i64, t: &[(u32, u32)]) -> Option<i64> {
    let j1 = code >> 8;
    let j2 = code & 0xff;
    if !(0x21..=0x7e).contains(&j1) || !(0x21..=0x7e).contains(&j2) {
        return None;
    }
    if let Some(u) = tbl_decode(t, code) {
        return Some(u as i64);
    }
    Some(base + (j1 as i64 - 0x21) * 94 + (j2 as i64 - 0x21))
}

fn jisx0213_encode(ch: u32, base: i64, t: &[(u32, u32)]) -> Option<i64> {
    if let Some(c) = tbl_encode(t, ch) {
        return Some(c);
    }
    let idx = ch as i64 - base;
    if !(0..94 * 94).contains(&idx) {
        return None;
    }
    Some(((0x21 + idx / 94) << 8) | (0x21 + idx % 94))
}

fn jisx0213_1_decode(code: u32) -> Option<i64> {
    jisx0213_decode(
        code,
        JISX0213_1_PRIV_BASE,
        crate::lisp::cjk_tables::JISX0213_1_DECODE,
    )
}

fn jisx0213_1_encode(ch: u32) -> Option<i64> {
    jisx0213_encode(
        ch,
        JISX0213_1_PRIV_BASE,
        crate::lisp::cjk_tables::JISX0213_1_ENCODE,
    )
}

fn jisx0213_2_encode(ch: u32) -> Option<i64> {
    jisx0213_encode(
        ch,
        JISX0213_2_PRIV_BASE,
        crate::lisp::cjk_tables::JISX0213_2_ENCODE,
    )
}

/// coding.h SJIS_TO_JIS: shift_jis pair -> jisx0208 code.
pub(crate) fn sjis_to_jis(code: i64) -> (i64, i64) {
    let s1 = code >> 8;
    let s2 = code & 0xff;
    if s2 >= 0x9f {
        (s1 * 2 - if s1 >= 0xe0 { 0x160 } else { 0xe0 }, s2 - 0x7e)
    } else {
        (
            s1 * 2 - if s1 >= 0xe0 { 0x161 } else { 0xe1 },
            s2 - if s2 >= 0x7f { 0x20 } else { 0x1f },
        )
    }
}

/// coding.h JIS_TO_SJIS: jisx0208 code -> shift_jis pair.
fn jis_to_sjis(code: i64) -> i64 {
    let j1 = code >> 8;
    let j2 = code & 0xff;
    let (s1, s2) = if j1 & 1 != 0 {
        (
            j1 / 2 + if j1 < 0x5f { 0x71 } else { 0xb1 },
            j2 + if j2 >= 0x60 { 0x20 } else { 0x1f },
        )
    } else {
        (j1 / 2 + if j1 < 0x5f { 0x70 } else { 0xb0 }, j2 + 0x7e)
    };
    (s1 << 8) | s2
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

fn f_decode_big5_char(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let code = want_wholenum_c(i, &a[0])?;
    if code < 0x80 {
        return Ok(Value::Int(code.into()));
    }
    // GNU checks the low byte masked to 7 bits (coding.c Fdecode_big5_char):
    // valid iff b1 in [0xA1,0xFE] and (code & 0x7F) in [0x40,0x7E], which
    // admits full byte2 in [0x40,0x7E] u [0xC0,0xFE].
    let b1 = code >> 8;
    let b2m = code & 0x7f;
    if !(0xa1..=0xfe).contains(&b1) || !(0x40..=0x7e).contains(&b2m) {
        return Err(invalid_code(i, code));
    }
    match big5_decode(code as u32) {
        Some(c) => Ok(Value::Int(c.into())),
        None => Err(invalid_code(i, code)),
    }
}

fn f_encode_big5_char(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let ch = want_char_c(i, &a[0])?;
    if ch < 0x80 {
        return Ok(Value::Int(ch.into()));
    }
    match big5_encode(ch as u32) {
        Some(c) => Ok(Value::Int(c.into())),
        None => Err(i.error(format!("Can't encode by Big5 encoding: {ch}"))),
    }
}

fn f_decode_sjis_char(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let code = want_wholenum_c(i, &a[0])?;
    if code < 0x80 {
        return Ok(Value::Int(code.into()));
    }
    // Halfwidth kana: GNU takes 0xA0..0xDE through charset jisx0201 at
    // code-0x80; 0xA0 lands outside the charset's space and fails.
    if (0xa0..0xdf).contains(&code) {
        if code == 0xa0 {
            return Err(invalid_code(i, code));
        }
        return Ok(Value::Int((code + 0xfec0).into()));
    }
    let c1 = code >> 8;
    let c2 = code & 0xff;
    if (!(0x81..=0x9f).contains(&c1) && !(0xe0..=0xef).contains(&c1))
        || !(0x40..=0xfc).contains(&c2)
        || c2 == 0x7f
    {
        return Err(invalid_code(i, code));
    }
    let (j1, j2) = sjis_to_jis(code);
    match jisx0213_1_decode(((j1 << 8) | j2) as u32) {
        Some(c) => Ok(Value::Int(c.into())),
        None => Err(invalid_code(i, code)),
    }
}

fn f_encode_sjis_char(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let ch = want_char_c(i, &a[0])?;
    if ch < 0x80 {
        return Ok(Value::Int(ch.into()));
    }
    // GNU runs halfwidth kana through JIS_TO_SJIS on their jisx0201 code,
    // which produces 0x70NN values that are not real shift_jis.
    if (0xff61..=0xff9f).contains(&ch) {
        return Ok(Value::Int(jis_to_sjis(ch - 0xff40).into()));
    }
    // GNU's Vsjis_coding_system ends as shift_jis-2004: charsets
    // (ascii katakana-jisx0201 jisx0213.2004-1 jisx0213-2).  Plane-2
    // chars get plain JIS_TO_SJIS, producing nonstandard codes as GNU.
    let jis = jisx0213_1_encode(ch as u32).or_else(|| jisx0213_2_encode(ch as u32));
    match jis {
        Some(jis) => Ok(Value::Int(jis_to_sjis(jis).into())),
        None => Err(i.error(format!("Can't encode by shift_jis encoding: {ch}"))),
    }
}

fn f_define_charset_internal(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU charset.c Fdefine_charset_internal: exactly charset_arg_max
    // (17) args, and the in-function check conses NAME . NARGS (a
    // dotted pair, unlike the dispatcher's list).
    if a.len() != 17 {
        let wna = i.intern("wrong-number-of-arguments");
        let who = symv(i, "define-charset-internal");
        return Err(i.signal(wna, Value::cons(who, Value::Int(a.len() as i128))));
    }
    define_charset_internal_impl(i, &a)
}

/// GNU `Faref' on args[2]-style inputs: vectors and strings index;
/// anything else signals `arrayp', out-of-bounds signals
/// (args-out-of-range ARRAY INDEX).
fn dci_aref(i: &mut Interp, v: &Value, idx: usize) -> Result<Value, Flow> {
    match v {
        Value::Vec(vec) => {
            let vv = vec.borrow();
            match vv.get(idx) {
                Some(x) => Ok(x.clone()),
                None => {
                    drop(vv);
                    let s = i.intern("args-out-of-range");
                    Err(i.signal_data(s, vec![v.clone(), Value::Int(idx as i128)]))
                }
            }
        }
        Value::Str(s) => {
            let sv = s.borrow();
            match sv.chars().nth(idx) {
                Some(c) => Ok(Value::Int(c as i128)),
                None => {
                    drop(sv);
                    let s2 = i.intern("args-out-of-range");
                    Err(i.signal_data(s2, vec![v.clone(), Value::Int(idx as i128)]))
                }
            }
        }
        other => Err(i.wrong_type_mut("arrayp", other)),
    }
}

/// GNU `check_integer_range': non-integers signal `integerp';
/// out-of-range signals (args-out-of-range X LO HI).
fn check_int_range(i: &mut Interp, v: &Value, lo: i128, hi: i128) -> Result<i128, Flow> {
    match v {
        Value::Int(n) => {
            if *n < lo || *n > hi {
                let s = i.intern("args-out-of-range");
                Err(i.signal_data(s, vec![v.clone(), Value::Int(lo), Value::Int(hi)]))
            } else {
                Ok(*n)
            }
        }
        other => Err(i.wrong_type_mut("integerp", other)),
    }
}

/// GNU data.c `cons_to_unsigned': integer, integral float, or the
/// legacy (HI MID . LO) cons packing; anything else or out of range
/// signals the plain `error' below.
fn cons_to_unsigned(v: &Value) -> Option<u128> {
    const UINT_MAX: u128 = u32::MAX as u128;
    match v {
        Value::Int(n) if *n >= 0 && *n <= UINT_MAX as i128 => Some(*n as u128),
        Value::Float(f) => {
            let d = **f;
            if d >= 0.0 && d < 1.0 + UINT_MAX as f64 && d.fract() == 0.0 {
                Some(d as u128)
            } else {
                None
            }
        }
        Value::Cons(c) => {
            let (car, cdr) = {
                let b = c.borrow();
                (b.car.clone(), b.cdr.clone())
            };
            let top = match &car {
                Value::Int(n) if *n >= 0 && (*n as u128) <= (u128::MAX >> 40) => *n as u128,
                _ => return None,
            };
            match &cdr {
                Value::Cons(r) => {
                    let (mid, low) = {
                        let rb = r.borrow();
                        (rb.car.clone(), rb.cdr.clone())
                    };
                    match (&mid, &low) {
                        (Value::Int(m), Value::Int(l))
                            if *m >= 0 && *m < (1 << 24) && *l >= 0 && *l < (1 << 16) =>
                        {
                            let val = (top << 40) | ((*m as u128) << 16) | (*l as u128);
                            if val <= UINT_MAX { Some(val) } else { None }
                        }
                        _ => None,
                    }
                }
                Value::Int(l) if *l >= 0 && *l < (1 << 16) => {
                    let val = (top << 16) | (*l as u128);
                    if val <= UINT_MAX { Some(val) } else { None }
                }
                _ => None,
            }
        }
        _ => None,
    }
}

/// Shared by `define-charset-internal' and the `define-charset' Lisp
/// wrapper's call: GNU's Fdefine_charset_internal validation order.
fn define_charset_internal_impl(i: &mut Interp, a: &[Value]) -> EvalResult {
    // NAME — CHECK_SYMBOL.
    let sid = want_sym(i, &a[0])?;
    // CODE-SPACE — Faref pairs (MIN MAX) × 4 dims; each byte is
    // check_integer_range 0..255.
    let mut mins = [0i128; 4];
    let mut maxs = [0i128; 4];
    let mut dim_auto = 0usize;
    for d in 0..4 {
        let minv = dci_aref(i, &a[2], d * 2)?;
        let maxv = dci_aref(i, &a[2], d * 2 + 1)?;
        let mn = check_int_range(i, &minv, 0, 255)?;
        let mx = check_int_range(i, &maxv, mn, 255)?;
        mins[d] = mn;
        maxs[d] = mx;
        if mx > 0 {
            dim_auto = d + 1;
        }
    }
    // DIMENSION — nil or integer 1..4.
    if !a[1].is_nil() {
        check_int_range(i, &a[1], 1, 4)?;
    }
    let min_code: u128 = (mins[0] as u128)
        | ((mins[1] as u128) << 8)
        | ((mins[2] as u128) << 16)
        | ((mins[3] as u128) << 24);
    let max_code: u128 = (maxs[0] as u128)
        | ((maxs[1] as u128) << 8)
        | ((maxs[2] as u128) << 16)
        | ((maxs[3] as u128) << 24);
    // MIN-CODE / MAX-CODE — nil or cons_to_unsigned within the space.
    // MIN-CODE / MAX-CODE — nil or cons_to_unsigned within the space;
    // GNU applies them (they override the code-space extremes).
    let mut min_code = min_code;
    let mut max_code = max_code;
    for (arg, is_min) in [(&a[3], true), (&a[4], false)] {
        if !arg.is_nil() {
            let code = cons_to_unsigned(arg).ok_or_else(|| {
                i.error("Not an in-range integer, integral float, or cons of integers")
            })?;
            if code < min_code || code > max_code {
                // GNU's reversed args_out_of_range_3 order:
                // (MIN-CODE MAX-CODE VAL).
                let s = i.intern("args-out-of-range");
                return Err(i.signal_data(
                    s,
                    vec![
                        Value::Int(min_code as i128),
                        Value::Int(max_code as i128),
                        arg.clone(),
                    ],
                ));
            }
            if is_min {
                min_code = code;
            } else {
                max_code = code;
            }
        }
    }
    // INVALID-CODE — nil defaults to 0 (min>0) or max+1; max ==
    // UINT_MAX with no attribute → "Attribute :invalid-code must be
    // specified".  Non-nil must be a valid code integer.
    if a[10].is_nil() {
        if min_code == 0 && max_code == u32::MAX as u128 {
            return Err(i.error("Attribute :invalid-code must be specified"));
        }
    } else if cons_to_unsigned(&a[10]).is_none() {
        return Err(i.error("Not an in-range integer, integral float, or cons of integers"));
    }
    // ISO-FINAL-CHAR — nil or fixnum 48..127.
    if !a[5].is_nil() {
        match &a[5] {
            Value::Int(fc) if (48..=127).contains(fc) => {}
            Value::Int(fc) => return Err(i.error(format!("Invalid iso-final-char: {}", fc))),
            other => return Err(i.wrong_type_mut("fixnump", other)),
        }
    }
    // ISO-REVISION-NUMBER — nil or integer -1..63.
    if !a[6].is_nil() {
        check_int_range(i, &a[6], -1, 63)?;
    }
    // EMACS-MULE-ID — nil or fixnat not in 1..=128 and < 256.
    if !a[7].is_nil() {
        match &a[7] {
            Value::Int(mid) if *mid >= 0 => {
                if (*mid > 0 && *mid <= 128) || *mid >= 256 {
                    return Err(i.error(format!("Invalid emacs-mule-id: {}", mid)));
                }
            }
            other => return Err(i.wrong_type_mut("wholenump", other)),
        }
    }
    // Method: CODE-OFFSET > MAP > SUBSET > SUPERSET > error.
    if !a[11].is_nil() {
        // CHECK_CHARACTER: integer in the character range.
        match &a[11] {
            Value::Int(n) if (0..=crate::lisp::builtins::misc::CT_MAX_CHAR as i128).contains(n) => {
                // GNU charset.c: i = CODE_POINT_TO_INDEX(max_code);
                // the check is `MAX_CHAR - offset < i'.  For linear
                // charsets i = max_code - min_code; otherwise
                // Σ ((byte_d - min_d) * Π ranges of lower dims)
                // minus char_index_offset (from :min-code, else 0).
                let idx: u128 = {
                    let linear = dim_auto == 1
                        || (maxs[0] - mins[0] == 255
                            && (dim_auto == 2
                                || (maxs[1] - mins[1] == 255
                                    && (dim_auto == 3 || maxs[2] - mins[2] == 255))));
                    if linear {
                        max_code - min_code
                    } else {
                        // Decompose the effective max_code into bytes;
                        // weight each by the product of the lower
                        // dimensions' ranges (GNU nchars[]).
                        let mut idx = 0u128;
                        let mut span = 1u128;
                        for d in 0..4usize {
                            let byte = ((max_code >> (8 * d)) & 0xFF) as u128;
                            let min = mins[d] as u128;
                            let max = maxs[d] as u128;
                            idx += byte.saturating_sub(min) * span;
                            span *= max - min + 1;
                        }
                        idx
                    }
                };
                // GNU stores `i' as `int': a saturated 0xFFFFFFFF
                // index reads back as -1, so `MAX_CHAR - offset < i'
                // fails and the charset registers anyway.  Mirror the
                // truncation — a full [0 255]*4 :code-space is what
                // GNU's default :dimension-4 charsets look like.
                let idx_i = idx as u32 as i32 as i128;
                let off = *n;
                if (crate::lisp::builtins::misc::CT_MAX_CHAR as i128 - off) < idx_i {
                    return Err(i.error(format!(
                        "Unsupported max char: {} + {}d > MAX_CHAR ({})",
                        idx_i,
                        off,
                        crate::lisp::builtins::misc::CT_MAX_CHAR
                    )));
                }
            }
            other => return Err(i.wrong_type_mut("characterp", other)),
        }
    } else if !a[12].is_nil() {
        // MAP: a string names a charset-map file loaded lazily by
        // `load-charset'; GNU validates nothing here (mule-conf's
        // dumped :map "8859-2" must not touch the filesystem).
    } else if !a[13].is_nil() {
        // SUBSET — (PARENT MIN-CODE MAX-CODE OFFSET): car must name a
        // charset, the next two are wholenump, offset a fixnum.
        let items = a[13].list_to_vec().unwrap_or_default();
        let parent = items.first().cloned().unwrap_or(Value::Nil);
        let _ = want_charset(i, &parent)?;
        for idx in [1usize, 2] {
            if let Some(v) = items.get(idx) {
                match v {
                    Value::Int(n) if *n >= 0 => {}
                    other => return Err(i.wrong_type_mut("wholenump", other)),
                }
            }
        }
        if let Some(v) = items.get(3) {
            match v {
                Value::Int(_) => {}
                other => return Err(i.wrong_type_mut("fixnump", other)),
            }
        }
    } else if !a[14].is_nil() {
        // SUPERSET — Fcopy_sequence first (sequencep), then each elt is
        // a charset symbol or (CHARSET . INT-OFFSET) cons.
        let mut cur = a[14].clone();
        match &cur {
            Value::Cons(_) | Value::Nil | Value::Vec(_) | Value::Str(_) => {}
            other => return Err(i.wrong_type_mut("sequencep", other)),
        }
        loop {
            match cur {
                Value::Cons(c) => {
                    let (elt, rest) = {
                        let b = c.borrow();
                        (b.car.clone(), b.cdr.clone())
                    };
                    match &elt {
                        Value::Cons(cc) => {
                            let (car_part, cdr_part) = {
                                let b = cc.borrow();
                                (b.car.clone(), b.cdr.clone())
                            };
                            let _ = want_charset(i, &car_part)?;
                            let _ =
                                check_int_range(i, &cdr_part, i32::MIN as i128, i32::MAX as i128)?;
                        }
                        _ => {
                            let _ = want_charset(i, &elt)?;
                        }
                    }
                    cur = rest;
                }
                Value::Nil => break,
                other => return Err(i.wrong_type_mut("listp", &other)),
            }
        }
    } else {
        return Err(i.error("None of :code-offset, :map, :parents are specified"));
    }
    // UNIFY-MAP — nil, string, or vector.
    if !a[15].is_nil() && !matches!(a[15], Value::Str(_)) && !matches!(a[15], Value::Vec(_)) {
        return Err(i.wrong_type_mut("vectorp", &a[15]));
    }
    // PLIST — CHECK_LIST.
    if !matches!(a[16], Value::Cons(_) | Value::Nil) {
        return Err(i.wrong_type_mut("listp", &a[16]));
    }
    let plist = a[16].clone();
    let supp = a[9].truthy();
    let name = i.symbol_name(sid);
    if let Some(e) = i.charsets.iter_mut().find(|(n, _)| *n == name) {
        if e.1.is_nil() && !plist.is_nil() {
            e.1 = plist;
        }
    } else {
        i.charsets.push((name.clone(), plist));
        // GNU Fdefine_charset_internal: `Vcharset_list = Fcons (name,
        // Vcharset_list)' — newest definitions prepend; the seed in
        // eval.rs holds the builtins oldest-first at the tail.
        {
            let cl = i.intern("charset-list");
            let cur = i.symbol_value(cl);
            let cell = Value::cons(Value::Sym(sid), cur);
            let _ = i.set_symbol_default(cl, cell);
        }
        if supp {
            let mut st = i.charset_order.borrow_mut();
            if !st.0.is_empty() {
                st.0.push(name.clone());
            }
        } else {
            charset_ordered_insert(i, &name);
        }
    }
    invalidate_char_charset_cache(i);
    Ok(Value::Nil)
}

fn f_define_coding_system_internal(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // (define-coding-system-internal NAME MNEMONIC CODING-TYPE CHARSET-LIST
    //  ASCII-COMPATIBLE-P DECODE-TT ENCODE-TT POST-READ PRE-WRITE
    //  DEFAULT-CHAR FOR-UNIBYTE PLIST EOL-TYPE . SPEC-ATTRS) — register
    //  the name so `coding-system-p` sees it.
    let sid = want_sym(i, &a[0])?;
    // GNU coding.c: MNEMONIC is CHECK_STRING_OR_CHAR first — nil or
    // non-char non-string signals characterp.
    match &a[1] {
        Value::Int(_) | Value::Str(_) => {}
        other => return Err(i.wrong_type_mut("characterp", other)),
    }
    // Then CODING-TYPE must name one of the builtin types.
    const CS_TYPES: &[&str] = &[
        "charset",
        "utf-8",
        "utf-16",
        "iso-2022",
        "emacs-mule",
        "ccl",
        "raw-text",
        "undecided",
        "shift-jis",
        "big5",
        "utf-7",
        "ebcdic",
    ];
    if let Value::Sym(s) = &a[2] {
        let n = i.symbol_name(*s);
        if !CS_TYPES.contains(&n.as_str()) {
            return Err(i.error(format!("Invalid coding system type: {}", n)));
        }
    }
    let name = i.symbol_name(sid);
    if !i.extra_coding_systems.iter().any(|n| *n == name) {
        i.extra_coding_systems.push(name.clone());
    }
    // GNU: define-coding-system-internal `add-to-coding-system-list's
    // the name, and — when EOL-TYPE is a 3-vector — registers the three
    // subsidiary coding systems too (parents land before their
    // subsidiaries since equal mnemonics insert after equal keys).
    crate::lisp::builtins::misc::add_to_coding_system_list(i, &name)?;
    // GNU coding.c Fdefine_coding_system_internal pushes
    // `(SYMBOL-NAME . nil)' onto `coding-system-alist' when absent
    // (subsidiary eol names get the same treatment below).
    let alist_sym = i.intern("coding-system-alist");
    let push_alist = |i: &mut Interp, name: &str| -> Result<(), Flow> {
        let cur = i.symbol_value(alist_sym).list_to_vec().unwrap_or_default();
        let present = cur.iter().any(|v| match v {
            Value::Cons(c) => matches!(&c.borrow().car, Value::Str(s) if *s.borrow() == *name),
            _ => false,
        });
        if !present {
            let mut items = vec![Value::cons(Value::string(name), Value::Nil)];
            items.extend(cur);
            i.set_symbol(alist_sym, Value::list(items))?;
        }
        Ok(())
    };
    push_alist(i, &name)?;
    if let Some(eol) = a.get(12) {
        if let Value::Vec(v) = eol {
            let subs: Vec<String> = v
                .borrow()
                .iter()
                .filter_map(|x| match x {
                    Value::Sym(s) => Some(i.symbol_name(*s)),
                    _ => None,
                })
                .collect();
            if subs.len() == 3 {
                // GNU var order is parent, MAC, DOS, UNIX — the vector
                // order is (UNIX DOS MAC), so register in reverse.
                for sub in subs.iter().rev() {
                    if !i.extra_coding_systems.iter().any(|n| n == sub) {
                        i.extra_coding_systems.push(sub.clone());
                    }
                    crate::lisp::builtins::misc::add_to_coding_system_list(i, sub)?;
                    push_alist(i, sub)?;
                }
            }
        }
    }
    Ok(Value::Nil)
}
