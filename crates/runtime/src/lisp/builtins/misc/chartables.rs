//! GNU character-table trie storage, inheritance and range operations.

use crate::lisp::builtins::arg;
use crate::lisp::{
    Interp,
    error::{EvalResult, Flow},
    obarray::sym,
    value::{SymId, Value},
};
use std::{cell::RefCell, collections::HashSet, rc::Rc};

// ---------- char tables (GNU chartab.c trie: 64/16/32/128)

/// Number of characters covered by one slot of each trie level:
/// top-level slots cover 65536, `#^^[1' slots 4096, `#^^[2' slots
/// 128, `#^^[3' leaves one char per slot.
pub(super) const CHARTAB_BITS: [u32; 4] = [16, 12, 7, 0];
/// Slot count of each trie level (the top level is indexed
/// separately — a depth-0 "table" is the char-table's own contents).
pub(super) const CHARTAB_SIZE: [usize; 4] = [64, 16, 32, 128];
/// `MAX_CHAR'.
pub(crate) const CT_MAX_CHAR: u32 = 0x3f_ffff;

/// The contents vector of a char-table: 65 slots where [0] is the
/// ASCII cache slot (a scalar, or the `#^^[3' leaf covering chars
/// 0-127) and [1..=64] are the top-level slots covering 65536
/// characters each.  Sub-tables are Records of the form
/// [sub-char-table DEPTH MIN-CHAR S0 .. SN].
pub(crate) fn char_table_vec(v: &Value) -> Option<Rc<RefCell<Vec<Value>>>> {
    match v {
        Value::Vec(v) => Some(v.clone()),
        Value::Record(r) => {
            let rr = r.borrow();
            match rr.get(2) {
                Some(Value::Vec(v)) => Some(v.clone()),
                _ => None,
            }
        }
        _ => None,
    }
}

pub(crate) fn is_char_table(_i: &Interp, v: &Value) -> bool {
    matches!(v, Value::Record(_))
        && v.record_kind() == Some(crate::lisp::value::RecordKind::CharTable)
}

fn char_table_identity(i: &Interp, v: &Value) -> Option<usize> {
    if !is_char_table(i, v) {
        return None;
    }
    match v {
        Value::Record(r) => Some(Rc::as_ptr(r) as usize),
        _ => None,
    }
}

/// Build a char-table: Record [char-table SUBTYPE CONTENTS65 EXTRAS..]
/// where CONTENTS65 is the 65-slot trie root ([0] ASCII cache,
/// [1..=64] top blocks), every slot = INIT.  GNU's `make_vector'
/// also initializes the defalt slot to INIT.
pub(crate) fn make_ct(i: &mut Interp, subtype: Value, init: Value, extras: Vec<Value>) -> Value {
    let vec = Value::vector(vec![init.clone(); 65]);
    let mut rec = vec![Value::Sym(i.q("char-table")), subtype, vec];
    rec.extend(extras);
    let t = Value::record_with_kind(rec, crate::lisp::value::RecordKind::CharTable);
    if !init.is_nil() {
        i.set_char_table_defalt(&t, init);
    }
    t
}

/// A sub-char-table (`#^^[DEPTH MIN-CHAR S0 .. SN]') is stored as a
/// Record headed by the `sub-char-table' symbol.  Users can fake the
/// tag, but GNU is similarly permissive about stored values.
pub(super) fn is_sub_ct(i: &Interp, v: &Value) -> bool {
    match i.q_soft("sub-char-table") {
        Some(tag) => is_sub_ct_tag(tag, v),
        None => false,
    }
}

/// `is_sub_ct' with a precomputed tag id (no `&Interp' needed).
pub(crate) fn is_sub_ct_tag(tag: SymId, v: &Value) -> bool {
    match v {
        Value::Record(r) => {
            let b = r.borrow();
            matches!(b.first(), Some(Value::Sym(s)) if *s == tag)
                && matches!(b.get(1), Some(Value::Int(d)) if (1..=3).contains(d))
                && matches!(b.get(2), Some(Value::Int(_)))
        }
        _ => false,
    }
}

/// (depth, min_char, record) of a sub-char-table value.
pub(super) fn sub_ct_parts(v: &Value) -> Option<(usize, u32, Rc<RefCell<Vec<Value>>>)> {
    let Value::Record(r) = v else {
        return None;
    };
    let b = r.borrow();
    let d = match b.get(1) {
        Some(Value::Int(n)) => *n as usize,
        _ => return None,
    };
    let m = match b.get(2) {
        Some(Value::Int(n)) => *n as u32,
        _ => return None,
    };
    Some((d, m, r.clone()))
}

pub(super) fn make_sub_ct(i: &mut Interp, depth: usize, min_char: u32, fill: Value) -> Value {
    let mut v = Vec::with_capacity(3 + CHARTAB_SIZE[depth]);
    v.push(Value::Sym(i.q("sub-char-table")));
    v.push(Value::Int(depth as i128));
    v.push(Value::Int(min_char as i128));
    v.extend(std::iter::repeat_n(fill, CHARTAB_SIZE[depth]));
    Value::record(v)
}

/// Raw value stored for C in the 65-slot contents vec CB — no
/// defalt or parent inheritance (GNU's raw `contents' walk).
fn ct_raw(i: &Interp, cb: &[Value], c: u32, uniprop: bool) -> Value {
    ct_raw_tag(i.q_soft("sub-char-table"), cb, c, uniprop)
}

/// GNU `UNIPROP_TABLE_P': purpose `char-code-property-table' with
/// exactly five extra slots.
pub(crate) fn is_uniprop_table(i: &Interp, v: &Value) -> bool {
    match v {
        Value::Record(r) => {
            let rr = r.borrow();
            rr.len() == 8
                && matches!(rr.first(), Some(Value::Sym(s)) if i.symbol_name(*s) == "char-table")
                && matches!(rr.get(1), Some(Value::Sym(s)) if i.symbol_name(*s) == "char-code-property-table")
                && matches!(rr.get(2), Some(Value::Vec(_)))
        }
        _ => false,
    }
}

/// GNU `UNIPROP_COMPRESSED_FORM_P': a string starting with character
/// 1 or 2 encodes 128 per-character values in a depth-2 leaf slot.
fn uniprop_compressed(v: &Value) -> bool {
    match v {
        Value::Str(s) => matches!(
            s.borrow().chars().next().and_then(|c| c.unicode()),
            Some('\u{1}') | Some('\u{2}')
        ),
        _ => false,
    }
}

/// GNU `uniprop_table_uncompress': expand a compressed leaf string in
/// slot SLOT of VEC into a real depth-3 sub-table covering MIN3..MIN3+127,
/// storing it back into VEC.  Format 1 is a plain sequence (0 = nil);
/// format 2 is run-length (a char >= 128 repeats the previous value).
fn uniprop_uncompress(tag: SymId, vec: &Rc<RefCell<Vec<Value>>>, slot: usize, min3: u32) -> Value {
    let val = vec.borrow().get(slot).cloned().unwrap_or(Value::Nil);
    let mut rec = Vec::with_capacity(3 + 128);
    rec.push(Value::Sym(tag));
    rec.push(Value::Int(3));
    rec.push(Value::Int(min3 as i128));
    rec.extend(std::iter::repeat_n(Value::Nil, 128));
    if let Value::Str(s) = &val {
        let chars: Vec<crate::lisp::value::EmacsChar> = s.borrow().chars().collect();
        match chars.first().and_then(|c| c.unicode()) {
            Some('\u{1}') => {
                let mut k = chars.get(1).map(|c| (*c).code() as usize).unwrap_or(0);
                for c in chars.iter().skip(2) {
                    if k >= 128 {
                        break;
                    }
                    rec[3 + k] = if *c == '\0' {
                        Value::Nil
                    } else {
                        Value::Int((*c).code() as i128)
                    };
                    k += 1;
                }
            }
            Some('\u{2}') => {
                let (mut k, mut j) = (0usize, 1usize);
                while j < chars.len() && k < 128 {
                    let v = (chars[j]).code() as u32;
                    j += 1;
                    let mut count = 1usize;
                    if j < chars.len() && (chars[j]).code() as u32 >= 128 {
                        count = (chars[j]).code() as usize - 128;
                        j += 1;
                    }
                    for _ in 0..count {
                        if k >= 128 {
                            break;
                        }
                        rec[3 + k] = Value::Int(v as i128);
                        k += 1;
                    }
                }
            }
            _ => {}
        }
    }
    let sub = Value::record(rec);
    if let Some(s) = vec.borrow_mut().get_mut(slot) {
        *s = sub.clone();
    }
    sub
}

/// `ct_raw' with the `sub-char-table' tag id (None = no sub-tables
/// can exist, so every slot is scalar).  Usable from `Syn' snapshots
/// and other `&Interp'-free contexts.  UNIPROP mirrors GNU's
/// `sub_char_table_ref' uniprop flag: compressed leaf strings are
/// expanded in place.
pub(crate) fn ct_raw_tag(tag: Option<SymId>, cb: &[Value], c: u32, uniprop: bool) -> Value {
    let is_sub = |v: &Value| tag.is_some_and(|t| is_sub_ct_tag(t, v));
    if c > CT_MAX_CHAR {
        return Value::Nil;
    }
    if c < 128 {
        return match cb.first() {
            Some(s) if is_sub(s) => sub_ct_parts(s)
                .and_then(|(_, _, r)| r.borrow().get(3 + c as usize).cloned())
                .unwrap_or(Value::Nil),
            Some(v) => v.clone(),
            None => Value::Nil,
        };
    }
    let mut cur = cb
        .get(1 + (c >> 16) as usize)
        .cloned()
        .unwrap_or(Value::Nil);
    loop {
        match cur {
            v if is_sub(&v) => {
                let Some((d, m, r)) = sub_ct_parts(&v) else {
                    return Value::Nil;
                };
                let idx = (c.saturating_sub(m) >> CHARTAB_BITS[d]) as usize;
                let next = match r.borrow().get(3 + idx) {
                    Some(x) => x.clone(),
                    None => return Value::Nil,
                };
                cur = if uniprop && uniprop_compressed(&next) {
                    match tag {
                        Some(t) => uniprop_uncompress(t, &r, 3 + idx, m + 128 * idx as u32),
                        None => next,
                    }
                } else {
                    next
                };
            }
            v => return v,
        }
    }
}

/// `char_table_ascii': the value of chars 0-127, derived by walking
/// contents[0]'s 0-index chain (scalar, or the `#^^[3' leaf).
fn ct_ascii(i: &Interp, cb: &[Value]) -> Value {
    let s1 = cb.get(1).cloned().unwrap_or(Value::Nil);
    if !is_sub_ct(i, &s1) {
        return s1;
    }
    let s2 = sub_ct_parts(&s1)
        .and_then(|(_, _, r)| r.borrow().get(3).cloned())
        .unwrap_or(Value::Nil);
    if !is_sub_ct(i, &s2) {
        return s2;
    }
    sub_ct_parts(&s2)
        .and_then(|(_, _, r)| r.borrow().get(3).cloned())
        .unwrap_or(Value::Nil)
}

/// Store `ct_ascii' into the contents vector's ASCII cache slot.
pub(crate) fn ct_refresh_ascii(i: &Interp, contents: &Rc<RefCell<Vec<Value>>>) {
    let v = {
        let cb = contents.borrow();
        ct_ascii(i, &cb)
    };
    contents.borrow_mut()[0] = v;
}

/// GNU `sub_char_table_set': write VAL for C inside sub-table SUB,
/// lazily materializing deeper levels.  UNIPROP mirrors GNU: a
/// compressed leaf string expands before the write descends.
fn sub_ct_set(i: &mut Interp, sub: &Value, c: u32, val: Value, uniprop: bool) {
    let Some((depth, min, r)) = sub_ct_parts(sub) else {
        return;
    };
    let idx = (c.saturating_sub(min) >> CHARTAB_BITS[depth]) as usize;
    if depth == 3 {
        r.borrow_mut()[3 + idx] = val;
        return;
    }
    let child = r.borrow().get(3 + idx).cloned().unwrap_or(Value::Nil);
    let child = if is_sub_ct(i, &child) {
        child
    } else if uniprop && uniprop_compressed(&child) {
        match i.q_soft("sub-char-table") {
            Some(t) => uniprop_uncompress(t, &r, 3 + idx, min + (idx as u32) * 128),
            None => child,
        }
    } else {
        let n = make_sub_ct(
            i,
            depth + 1,
            min + (idx as u32) * (1 << CHARTAB_BITS[depth]),
            child,
        );
        r.borrow_mut()[3 + idx] = n.clone();
        n
    };
    sub_ct_set(i, &child, c, val, uniprop);
}

/// GNU `char_table_set': write VAL for C, splitting the path and
/// refreshing the ASCII cache as needed.
pub(crate) fn ct_set(i: &mut Interp, table: &Value, c: u32, val: Value) {
    let Some(contents) = char_table_vec(table) else {
        return;
    };
    if c > CT_MAX_CHAR {
        return;
    }
    if c < 128 {
        let hit = {
            let cb = contents.borrow();
            cb.first().map(|v| is_sub_ct(i, v)).unwrap_or(false)
        };
        if hit {
            if let Some(a) = contents.borrow().first().cloned() {
                if let Some((_, _, r)) = sub_ct_parts(&a) {
                    r.borrow_mut()[3 + c as usize] = val;
                    return;
                }
            }
        }
    }
    let ti = 1 + (c >> 16) as usize;
    let sub = contents.borrow().get(ti).cloned().unwrap_or(Value::Nil);
    let sub = if is_sub_ct(i, &sub) {
        sub
    } else {
        let n = make_sub_ct(i, 1, (c >> 16) << 16, sub);
        contents.borrow_mut()[ti] = n.clone();
        n
    };
    sub_ct_set(i, &sub, c, val, is_uniprop_table(i, table));
    if c < 128 {
        ct_refresh_ascii(i, &contents);
    }
}

/// GNU `sub_char_table_set_range': block-optimized range write —
/// fully covered slots are overwritten with the scalar (collapsing
/// any sub-table), partial slots are materialized and descended.
/// UNIPROP mirrors GNU: compressed leaf strings expand first.
fn sub_ct_set_range(i: &mut Interp, sub: &Value, from: u32, to: u32, val: &Value, uniprop: bool) {
    let Some((depth, min, r)) = sub_ct_parts(sub) else {
        return;
    };
    let chars = 1u32 << CHARTAB_BITS[depth];
    let lim = CHARTAB_SIZE[depth];
    let from = from.max(min);
    let mut idx = (from.saturating_sub(min) >> CHARTAB_BITS[depth]) as usize;
    let mut c = min + chars * idx as u32;
    while idx < lim && c <= to {
        if from <= c && c + chars - 1 <= to {
            r.borrow_mut()[3 + idx] = val.clone();
        } else {
            let child = r.borrow()[3 + idx].clone();
            let child = if is_sub_ct(i, &child) {
                child
            } else if uniprop && uniprop_compressed(&child) {
                match i.q_soft("sub-char-table") {
                    Some(t) => uniprop_uncompress(t, &r, 3 + idx, min + (idx as u32) * 128),
                    None => child,
                }
            } else {
                let n = make_sub_ct(i, depth + 1, c, child);
                r.borrow_mut()[3 + idx] = n.clone();
                n
            };
            sub_ct_set_range(i, &child, from, to, val, uniprop);
        }
        idx += 1;
        c += chars;
    }
}

/// GNU `char_table_set_range': range write with whole-block
/// collapsing (FROM and TO inclusive).
pub(crate) fn ct_set_range(i: &mut Interp, table: &Value, from: u32, to: u32, val: Value) {
    if from == to {
        ct_set(i, table, from, val);
        return;
    }
    let Some(contents) = char_table_vec(table) else {
        return;
    };
    let to = to.min(CT_MAX_CHAR);
    let uniprop = is_uniprop_table(i, table);
    let mut ti = (from >> 16) as usize;
    let lim = (to >> 16) as usize;
    while ti <= lim {
        let c = (ti as u32) << 16;
        if c > to {
            break;
        }
        if from <= c && c + 65535 <= to {
            contents.borrow_mut()[1 + ti] = val.clone();
        } else {
            let sub = contents.borrow()[1 + ti].clone();
            let sub = if is_sub_ct(i, &sub) {
                sub
            } else {
                let n = make_sub_ct(i, 1, c, sub);
                contents.borrow_mut()[1 + ti] = n.clone();
                n
            };
            sub_ct_set_range(i, &sub, from, to, &val, uniprop);
        }
        ti += 1;
    }
    if from < 128 {
        ct_refresh_ascii(i, &contents);
    }
}

/// GNU's `char-width-table' char-table: subtype nil, nil defalt, and
/// a parent holding all the width data — replayed from
/// CHAR_WIDTH_RANGES (extracted from Emacs 31's effective `aref'
/// values).
pub(super) fn f_remacs_char_width_table(i: &mut Interp, _a: crate::lisp::Argv) -> EvalResult {
    // GNU's `char-width-table' is an empty char-table whose parent
    // holds all the width data (charset.c `syms_of_charset_for_lisp').
    let parent = make_ct(i, Value::Nil, Value::Nil, vec![]);
    for &(s, e, w) in crate::lisp::ctdata::CHAR_WIDTH_RANGES {
        ct_set_range(i, &parent, s, e, Value::Int(w as i128));
    }
    let t = make_ct(i, Value::Nil, Value::Nil, vec![]);
    i.set_char_table_parent(&t, parent);
    Ok(t)
}

/// Deep copy of a sub-char-table value (`copy_sub_char_table').
fn sub_ct_copy(i: &mut Interp, v: &Value) -> Value {
    let Some((depth, min, r)) = sub_ct_parts(v) else {
        return v.clone();
    };
    let n = CHARTAB_SIZE[depth];
    let mut rec = Vec::with_capacity(3 + n);
    rec.push(Value::Sym(i.q("sub-char-table")));
    rec.push(Value::Int(depth as i128));
    rec.push(Value::Int(min as i128));
    {
        let b = r.borrow();
        for slot in b.iter().skip(3).take(n) {
            rec.push(if is_sub_ct(i, slot) {
                sub_ct_copy(i, slot)
            } else {
                slot.clone()
            });
        }
    }
    Value::record(rec)
}

/// `copy_char_table': deep copy of TABLE's contents (recursive
/// sub-tables), defalt, parent, purpose and extra slots.  The ASCII
/// slot is re-derived from the copied trie.
pub(crate) fn ct_copy(i: &mut Interp, table: &Value) -> Value {
    let Value::Record(r) = table else {
        return table.clone();
    };
    let (subtype, contents, extras) = {
        let b = r.borrow();
        let subtype = b.get(1).cloned().unwrap_or(Value::Nil);
        let contents = match b.get(2) {
            Some(Value::Vec(v)) => Some(v.clone()),
            _ => None,
        };
        let extras: Vec<Value> = b.iter().skip(3).cloned().collect();
        (subtype, contents, extras)
    };
    let Some(contents) = contents else {
        return table.clone();
    };
    let copied: Vec<Value> = {
        let b = contents.borrow();
        b.iter()
            .skip(1)
            .map(|v| {
                if is_sub_ct(i, v) {
                    sub_ct_copy(i, v)
                } else {
                    v.clone()
                }
            })
            .collect()
    };
    let mut nv = Vec::with_capacity(65);
    nv.push(Value::Nil);
    nv.extend(copied);
    let contents_rc = crate::lisp::Value::vector_ref(nv);
    ct_refresh_ascii(i, &contents_rc);
    let mut rec = vec![
        Value::Sym(i.q("char-table")),
        subtype,
        Value::Vec(contents_rc),
    ];
    rec.extend(extras);
    let t = Value::record_with_kind(rec, crate::lisp::value::RecordKind::CharTable);
    let defalt = i.char_table_defalt(table);
    if !defalt.is_nil() {
        i.set_char_table_defalt(&t, defalt);
    }
    let parent = i.char_table_parent(table);
    if !parent.is_nil() {
        i.set_char_table_parent(&t, parent);
    }
    t
}

/// Replay GNU-extracted `(R LO HI VAL)'/`(L OFF VALS)' ops (see
/// `ctdata.rs') into TABLE — reproduces GNU's exact trie structure
/// and contents.  Range ops first is not required: every slot GNU
/// shows gets its own op.
pub(crate) fn ct_replay(i: &mut Interp, table: &Value, ops_src: &str) {
    if i.dbg_flags & 0x10000 != 0 {
        eprintln!("[ct_replay] start");
    }
    let mut r = crate::lisp::reader::Reader::new(i, ops_src);
    let ops = match r.read() {
        Ok(Some(v)) => v,
        _ => return,
    };
    if i.dbg_flags & 0x10000 != 0 {
        eprintln!("[ct_replay] read done");
    }
    let mut n = 0usize;
    ops.each_car(|op| {
        n += 1;
        if i.dbg_flags & 0x10000 != 0 {
            eprintln!("[ct_replay] op {}", n);
        }
        let items = op.list_to_vec().unwrap_or_default();
        let tag = match items.first() {
            Some(Value::Sym(s)) => i.symbol_name(*s).to_string(),
            _ => return,
        };
        match tag.as_str() {
            "R" => {
                if let (Some(Value::Int(lo)), Some(Value::Int(hi)), Some(v)) =
                    (items.get(1), items.get(2), items.get(3))
                {
                    ct_set_range(i, table, *lo as u32, *hi as u32, v.clone());
                }
            }
            "L" => {
                if let (Some(Value::Int(off)), Some(vals)) = (items.get(1), items.get(2)) {
                    if let Some(vs) = vals.list_to_vec().ok() {
                        for (k, v) in vs.iter().enumerate() {
                            ct_set(i, table, *off as u32 + k as u32, v.clone());
                        }
                    }
                }
            }
            _ => {}
        }
    });
}

/// `optimize_sub_char_table': collapse a sub-table whose slots all
/// compare equal (default `equal'; `eq' optimized).  Children are
/// optimized bottom-up first.
fn ct_same(
    i: &mut Interp,
    a: &Value,
    b: &Value,
    test: &Value,
    eq_test: bool,
) -> Result<bool, Flow> {
    if eq_test {
        Ok(crate::lisp::builtins::eq_values(a, b))
    } else if test.is_nil() {
        crate::lisp::builtins::equal_values(i, a, b)
    } else {
        // A/B are values: quote them so `call_function's
        // unevaluated-arg protocol doesn't re-eval a symbol like
        // `cdr' as a variable (void-variable).
        let q = |v: &Value| Value::list(vec![Value::Sym(sym::QUOTE), v.clone()]);
        i.call_function(test, &Value::list(vec![q(a), q(b)]), None)
            .map(|v| !v.is_nil())
    }
}

fn sub_ct_optimize(i: &mut Interp, sub: &Value, test: &Value) -> EvalResult {
    let Some((depth, _, r)) = sub_ct_parts(sub) else {
        return Ok(sub.clone());
    };
    let n = CHARTAB_SIZE[depth];
    let eq_test = matches!(test, Value::Sym(s) if i.symbol_name(*s) == "eq");
    let test_fn = test.clone();
    let mut first = {
        let b = r.borrow();
        b.get(3).cloned().unwrap_or(Value::Nil)
    };
    if is_sub_ct(i, &first) {
        first = sub_ct_optimize(i, &first, &test_fn)?;
        r.borrow_mut()[3] = first.clone();
    }
    let mut optimizable = !is_sub_ct(i, &first);
    for k in 1..n {
        let mut this = {
            let b = r.borrow();
            b.get(3 + k).cloned().unwrap_or(Value::Nil)
        };
        if is_sub_ct(i, &this) {
            this = sub_ct_optimize(i, &this, &test_fn)?;
            r.borrow_mut()[3 + k] = this.clone();
        }
        if optimizable && !ct_same(i, &this, &first, &test_fn, eq_test)? {
            optimizable = false;
        }
    }
    Ok(if optimizable { first } else { sub.clone() })
}

/// `Foptimize_char_table' inner: optimize each top-level slot and
/// refresh the ASCII cache.
pub(super) fn ct_optimize(i: &mut Interp, table: &Value, test: &Value) -> Result<(), Flow> {
    let Some(contents) = char_table_vec(table) else {
        return Ok(());
    };
    for ti in 1..=64usize {
        let sub = contents.borrow()[ti].clone();
        if is_sub_ct(i, &sub) {
            let newv = sub_ct_optimize(i, &sub, test)?;
            contents.borrow_mut()[ti] = newv;
        }
    }
    ct_refresh_ascii(i, &contents);
    Ok(())
}

/// GNU `char_table_ascii' for a uniprop table: a compressed string in
/// the ASCII cache slot (chars 0-127) expands into a depth-3 leaf.
fn ct_uncompress_ascii(i: &Interp, v: &Value, vec: &Rc<RefCell<Vec<Value>>>) {
    let Some(tag) = i.q_soft("sub-char-table") else {
        return;
    };
    let cur = vec.borrow().first().cloned().unwrap_or(Value::Nil);
    if is_uniprop_table(i, v) && uniprop_compressed(&cur) {
        uniprop_uncompress(tag, vec, 0, 0);
    }
}

/// Raw contents of TABLE for char C — no defalt/parent inheritance.
pub(crate) fn char_table_raw(i: &Interp, v: &Value, idx: usize) -> Value {
    match char_table_vec(v) {
        Some(vec) => {
            if idx < 128 {
                ct_uncompress_ascii(i, v, &vec);
            }
            let b = vec.borrow();
            // Flat legacy tables (bare vectors / old-style 256-vec
            // contents) index directly.
            if b.len() == 256 {
                return b.get(idx).cloned().unwrap_or(Value::Nil);
            }
            ct_raw(i, &b, idx as u32, is_uniprop_table(i, v))
        }
        None => Value::Nil,
    }
}

/// `char_table_ref_simple' top-level step: raw value for C, falling
/// back to this table's own defalt (never the parent — GNU's
/// cons-range `char-table-range' semantics).
pub(super) fn ct_ref_defalt(i: &Interp, table: &Value, idx: usize) -> Value {
    let raw = char_table_raw(i, table, idx);
    if !raw.is_nil() {
        return raw;
    }
    i.char_table_defalt(table)
}

/// GNU `char_table_ref': raw content slot → this table's defalt →
/// parent chain (defalt checked at each level before following the
/// parent link).  Used by `aref'/`elt' and syntax lookups; NOT by
/// `char-table-range' with a cons range (which skips the parent).
/// Read-only access to a char-table chain without retaining an Interp
/// borrow.  Regexp matching uses this while its buffer is borrowed.
pub(crate) struct CharTableSnapshot {
    tag: Option<SymId>,
    levels: Vec<(Rc<RefCell<Vec<Value>>>, Value, bool)>,
}

impl CharTableSnapshot {
    pub(crate) fn new(i: &Interp, table: &Value) -> Option<Self> {
        if !is_char_table(i, table) {
            return None;
        }
        let mut levels = Vec::new();
        let mut seen = HashSet::new();
        let mut current = table.clone();
        loop {
            let Some(identity) = char_table_identity(i, &current) else {
                break;
            };
            if !seen.insert(identity) {
                break;
            }
            let contents = char_table_vec(&current)?;
            ct_uncompress_ascii(i, &current, &contents);
            levels.push((
                contents,
                i.char_table_defalt(&current),
                is_uniprop_table(i, &current),
            ));
            current = i.char_table_parent(&current);
            if current.is_nil() {
                break;
            }
        }
        Some(Self {
            tag: i.q_soft("sub-char-table"),
            levels,
        })
    }

    pub(crate) fn get(&self, code: u32) -> Value {
        for (contents, default, uniprop) in &self.levels {
            let contents = contents.borrow();
            let value = if contents.len() == 256 {
                contents.get(code as usize).cloned().unwrap_or(Value::Nil)
            } else {
                ct_raw_tag(self.tag, &contents, code, *uniprop)
            };
            if !value.is_nil() {
                return value;
            }
            if !default.is_nil() {
                return default.clone();
            }
        }
        Value::Nil
    }
}

pub(crate) fn char_table_ref(i: &Interp, table: &Value, idx: usize) -> Value {
    let mut cur = table.clone();
    let mut seen = HashSet::new();
    loop {
        let Some(identity) = char_table_identity(i, &cur) else {
            return Value::Nil;
        };
        if !seen.insert(identity) {
            return Value::Nil;
        }
        let val = match char_table_vec(&cur) {
            Some(v) => {
                if idx < 128 {
                    ct_uncompress_ascii(i, &cur, &v);
                }
                let b = v.borrow();
                if b.len() == 256 {
                    b.get(idx).cloned().unwrap_or(Value::Nil)
                } else {
                    ct_raw(i, &b, idx as u32, is_uniprop_table(i, &cur))
                }
            }
            None => return Value::Nil,
        };
        if !val.is_nil() {
            return val;
        }
        let defalt = i.char_table_defalt(&cur);
        if !defalt.is_nil() {
            return defalt;
        }
        let parent = i.char_table_parent(&cur);
        if parent.is_nil() {
            return Value::Nil;
        }
        cur = parent;
    }
    Value::Nil
}

pub(super) fn f_char_table_range(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU `Fchar_table_range': nil RANGE reads the table's own
    // default; a character RANGE resolves via `char_table_ref'
    // (raw → defalt → parent); a cons (FROM . TO) returns the value
    // for FROM via `char_table_ref_and_range' (raw → defalt, no
    // parent).  Other ranges are an error.
    if !is_char_table(i, &a[0]) {
        return Err(i.wrong_type_mut("char-table-p", &a[0]));
    }
    match &a[1] {
        Value::Nil => Ok(i.char_table_defalt(&a[0])),
        Value::Int(n) => {
            // GNU: an out-of-character-range fixnum falls through to
            // the generic "Invalid RANGE" error, not `characterp'.
            if !(0..=CT_MAX_CHAR as i128).contains(n) {
                return Err(i.error_q::<str>("Invalid RANGE argument to `char-table-range'", &[]));
            }
            Ok(char_table_ref(i, &a[0], *n as usize))
        }
        Value::Cons(c) => {
            let (from, to) = {
                let cc = c.borrow();
                (cc.car.clone(), cc.cdr.clone())
            };
            let from = match &from {
                Value::Int(f) if (0..=CT_MAX_CHAR as i128).contains(f) => *f as usize,
                _ => return Err(i.wrong_type_mut("characterp", &from)),
            };
            match &to {
                Value::Int(t) if (0..=CT_MAX_CHAR as i128).contains(t) => {}
                _ => return Err(i.wrong_type_mut("characterp", &to)),
            }
            Ok(ct_ref_defalt(i, &a[0], from))
        }
        _ => Err(i.error_q::<str>("Invalid RANGE argument to `char-table-range'", &[])),
    }
}

pub(super) fn f_char_table_parent(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    if !is_char_table(i, &a[0]) {
        return Err(i.wrong_type_mut("char-table-p", &a[0]));
    }
    Ok(i.char_table_parent(&a[0]))
}

pub(super) fn f_set_char_table_parent(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    if !is_char_table(i, &a[0]) {
        return Err(i.wrong_type_mut("char-table-p", &a[0]));
    }
    if !a[1].is_nil() && !is_char_table(i, &a[1]) {
        return Err(i.wrong_type_mut("char-table-p", &a[1]));
    }
    // GNU: "Attempt to make a chartable be its own parent".
    let mut temp = a[1].clone();
    let mut seen = HashSet::new();
    loop {
        if temp.is_nil() {
            break;
        }
        if crate::lisp::builtins::eq_values(&temp, &a[0]) {
            return Err(i.error("Attempt to make a chartable be its own parent"));
        }
        let Some(identity) = char_table_identity(i, &temp) else {
            break;
        };
        if !seen.insert(identity) {
            return Err(i.error("Attempt to make a chartable be its own parent"));
        }
        temp = i.char_table_parent(&temp);
    }
    i.set_char_table_parent(&a[0], a[1].clone());
    Ok(a[1].clone())
}

pub(super) fn f_set_char_table_range(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU `Fset_char_table-range': RANGE t sets the ASCII slot and
    // all 64 top-level slots (defalt untouched); nil sets the defalt;
    // a char splits the trie path; a cons does a block-optimized
    // range write.  Returns VALUE.
    if !is_char_table(i, &a[0]) {
        return Err(i.wrong_type_mut("char-table-p", &a[0]));
    }
    let val = a[2].clone();
    match &a[1] {
        Value::Sym(s) if i.symbol_name(*s) == "t" => {
            if let Some(v) = char_table_vec(&a[0]) {
                let mut vv = v.borrow_mut();
                for slot in vv.iter_mut() {
                    *slot = val.clone();
                }
            }
        }
        Value::Nil => {
            i.set_char_table_defalt(&a[0], val.clone());
        }
        Value::Int(n) => {
            if !(0..=CT_MAX_CHAR as i128).contains(n) {
                return Err(
                    i.error_q::<str>("Invalid RANGE argument to `set-char-table-range'", &[])
                );
            }
            ct_set(i, &a[0], *n as u32, val.clone());
        }
        Value::Cons(c) => {
            let (from, to) = {
                let cc = c.borrow();
                (cc.car.clone(), cc.cdr.clone())
            };
            let from = match &from {
                Value::Int(f) if (0..=CT_MAX_CHAR as i128).contains(f) => *f as u32,
                _ => return Err(i.wrong_type_mut("characterp", &from)),
            };
            let to = match &to {
                Value::Int(t) if (0..=CT_MAX_CHAR as i128).contains(t) => *t as u32,
                _ => return Err(i.wrong_type_mut("characterp", &to)),
            };
            // GNU: a reversed (FROM > TO) cons silently sets nothing.
            if from <= to {
                ct_set_range(i, &a[0], from, to, val.clone());
            }
        }
        _ => return Err(i.error_q::<str>("Invalid RANGE argument to `set-char-table-range'", &[])),
    }
    Ok(val)
}

/// Walk the raw trie in char order, producing (FROM TO VAL) spans —
/// every character is covered by exactly one span (scalar slots
/// report their whole block).  Used by `map-char-table' and the
/// keymap iterators.  UNIPROP mirrors GNU's `map_sub_char_table':
/// compressed leaf strings expand into their 128 slots in place.
fn ct_walk(
    i: &Interp,
    v: &Value,
    from: u32,
    to: u32,
    uniprop: bool,
    out: &mut Vec<(u32, u32, Value)>,
) {
    if is_sub_ct(i, v) {
        if let Some((depth, min, r)) = sub_ct_parts(v) {
            for k in 0..CHARTAB_SIZE[depth] {
                let lo = min + (k as u32) * (1 << CHARTAB_BITS[depth]);
                let hi = lo + (1 << CHARTAB_BITS[depth]) - 1;
                let child = {
                    let b = r.borrow();
                    b.get(3 + k).cloned().unwrap_or(Value::Nil)
                };
                let child = if uniprop && uniprop_compressed(&child) {
                    match i.q_soft("sub-char-table") {
                        Some(t) => uniprop_uncompress(t, &r, 3 + k, lo),
                        None => child,
                    }
                } else {
                    child
                };
                ct_walk(i, &child, lo, hi.min(to), uniprop, out);
            }
        }
        return;
    }
    out.push((from, to, v.clone()));
}

/// `ct_walk' over the whole table — 64 top slots of 65536 chars.
fn ct_spans(i: &Interp, table: &Value, uniprop: bool) -> Vec<(u32, u32, Value)> {
    let mut out = Vec::new();
    if let Some(contents) = char_table_vec(table) {
        let b = contents.borrow();
        for k in 0..64usize {
            let lo = (k as u32) << 16;
            let slot = b.get(1 + k).cloned().unwrap_or(Value::Nil);
            ct_walk(i, &slot, lo, lo + 65535, uniprop, &mut out);
        }
    }
    out
}

/// Effective-value runs clipped to `[LO, HI]`: descend only into
/// sub-tables overlapping the range, so untouched space costs
/// nothing per character.  Each emitted leaf is resolved through
/// `char_table_ref' — raw → own defalt → parent chain.  Adjacent
/// runs with `eq' values merge via `merge_runs'.  Used by
/// `modify-category-entry' to find run boundaries in O(leaves)
/// instead of O(characters).
pub(super) fn ct_runs_in(i: &Interp, table: &Value, lo: u32, hi: u32) -> Vec<(u32, u32, Value)> {
    fn walk(
        i: &Interp,
        v: &Value,
        from: u32,
        to: u32,
        lo: u32,
        hi: u32,
        table: &Value,
        out: &mut Vec<(u32, u32, Value)>,
    ) {
        if from > hi || to < lo {
            return;
        }
        if is_sub_ct(i, v) {
            if let Some((depth, min, r)) = sub_ct_parts(v) {
                let chars = 1u32 << CHARTAB_BITS[depth];
                for k in 0..CHARTAB_SIZE[depth] {
                    let c = min + (k as u32) * chars;
                    if c > hi || c + chars - 1 < lo {
                        continue;
                    }
                    let child = {
                        let b = r.borrow();
                        b.get(3 + k).cloned().unwrap_or(Value::Nil)
                    };
                    walk(i, &child, c, c + chars - 1, lo, hi, table, out);
                }
            }
            return;
        }
        let f = from.max(lo);
        let t = to.min(hi);
        // A scalar slot's value is uniform over its whole block.
        // For a nil leaf the effective value comes from this
        // table's defalt first (uniform for the span); only when
        // the defalt is also nil do we follow the parent — whose
        // runs may vary inside the span, so they are spliced in
        // with a clipped recursive walk.
        if !v.is_nil() {
            out.push((f, t, v.clone()));
            return;
        }
        let defalt = i.char_table_defalt(table);
        if !defalt.is_nil() {
            out.push((f, t, defalt));
            return;
        }
        let parent = i.char_table_parent(table);
        if parent.is_nil() {
            out.push((f, t, Value::Nil));
        } else {
            out.extend(ct_runs_in(i, &parent, f, t));
        }
    }
    let mut out = Vec::new();
    if let Some(contents) = char_table_vec(table) {
        let b = contents.borrow();
        for k in 0..64usize {
            let base = (k as u32) << 16;
            if base > hi || base + 65535 < lo {
                continue;
            }
            let slot = b.get(1 + k).cloned().unwrap_or(Value::Nil);
            walk(i, &slot, base, base + 65535, lo, hi, table, &mut out);
        }
    }
    merge_runs(out)
}

/// Merge adjacent spans with `eq' values (GNU's range coalescing).
fn merge_runs(spans: Vec<(u32, u32, Value)>) -> Vec<(u32, u32, Value)> {
    let mut out: Vec<(u32, u32, Value)> = Vec::with_capacity(spans.len());
    for (f, t, v) in spans {
        if let Some((_, pt, pv)) = out.last_mut() {
            if crate::lisp::builtins::eq_values(pv, &v) && *pt + 1 == f {
                *pt = t;
                continue;
            }
        }
        out.push((f, t, v));
    }
    out
}

/// Effective (raw → defalt) runs of TABLE; nil-value regions are
/// spliced with the parent's own runs when `with_parent' — GNU's
/// `map_char_table' semantics (the parent's own parent is excluded).
pub(crate) fn ct_effective_runs(
    i: &Interp,
    table: &Value,
    with_parent: bool,
) -> Vec<(u32, u32, Value)> {
    let defalt = i.char_table_defalt(table);
    let spans = ct_spans(i, table, is_uniprop_table(i, table))
        .into_iter()
        .map(|(f, t, v)| (f, t, if v.is_nil() { defalt.clone() } else { v }))
        .collect();
    let runs = merge_runs(spans);
    let parent = i.char_table_parent(table);
    if with_parent && !parent.is_nil() {
        let parent_runs = ct_effective_runs(i, &parent, false);
        let mut out = Vec::new();
        for (f, t, v) in runs {
            if v.is_nil() {
                // Splice parent's own runs clipped to [f, t].
                let mut any = false;
                for (pf, pt, pv) in &parent_runs {
                    let (lo, hi) = ((*pf).max(f), (*pt).min(t));
                    if lo <= hi {
                        out.push((lo, hi, pv.clone()));
                        any = true;
                    }
                }
                if !any {
                    out.push((f, t, Value::Nil));
                }
            } else {
                out.push((f, t, v));
            }
        }
        merge_runs(out)
    } else {
        runs
    }
}

/// Raw non-nil runs of TABLE — for keymap iteration, which treats
/// nil slots as unbound.
pub(crate) fn ct_collect(i: &Interp, table: &Value) -> Vec<(u32, u32, Value)> {
    merge_runs(ct_spans(i, table, false))
        .into_iter()
        .filter(|(_, _, v)| !v.is_nil())
        .collect()
}

pub(super) fn f_map_char_table(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    if !is_char_table(i, &a[1]) {
        return Err(i.wrong_type_mut("char-table-p", &a[1]));
    }
    // GNU's `uniprop_get_decoder': emitted run values are decoded only
    // when extra slot 1 is the fixnum decoder index (0 = run-length
    // through the extra-slot-4 value vector); a Lisp-function slot
    // (word-list tables) leaves the raw value untouched.
    let decode = matches!(&a[1], Value::Record(r) if matches!(r.borrow().get(4), Some(Value::Int(0))))
        && is_uniprop_table(i, &a[1]);
    let runs = ct_effective_runs(i, &a[1], true);
    // GNU's `map_char_table' only calls the function for ranges whose
    // effective value is non-nil — unset regions (and nil stores,
    // which revert to the default) are skipped entirely.  GNU passes
    // ONE shared RANGE cons it mutates between calls (a lone
    // character is passed as a fixnum) — retained keys all end up
    // showing the final mutation; mirror that.
    let range = Value::cons(Value::Int(0), Value::Int(0));
    let mut last: Option<(u32, u32)> = None;
    for (from, to, val) in runs.into_iter().filter(|(_, _, v)| !v.is_nil()) {
        last = Some((from, to));
        // GNU updates the shared range even when this callback receives
        // a fixnum. A previously retained cons must already show the
        // singleton bounds while that callback is running (or exits).
        if let Value::Cons(c) = &range {
            let mut cc = c.borrow_mut();
            cc.car = Value::Int(from as i128);
            cc.cdr = Value::Int(to as i128);
        }
        let key = if from == to {
            Value::Int(from as i128)
        } else {
            range.clone()
        };
        let val = if decode {
            crate::lisp::builtins::charset::uniprop_decode_run_length(&a[1], val)
        } else {
            val
        };
        i.apply(&a[0], vec![key, val])?;
    }
    // GNU leaves its shared RANGE cons at `(FROM . TO)' where FROM is
    // the block start after the last emitted run (i.e. last-to + 1,
    // or the run's own `from' when it reached MAX_CHAR) and TO is
    // MAX_CHAR — a callback that retains the cons sees this state.
    if let Some((from, to)) = last {
        if let Value::Cons(c) = &range {
            let mut cc = c.borrow_mut();
            cc.car = Value::Int(if to < CT_MAX_CHAR { to + 1 } else { from } as i128);
            cc.cdr = Value::Int(CT_MAX_CHAR as i128);
        }
    }
    Ok(Value::Nil)
}

/// Record slot 3+N of a char-table (the extra slots).
pub(crate) fn ct_extra(table: &Value, n: usize) -> Value {
    match table {
        Value::Record(r) => r.borrow().get(3 + n).cloned().unwrap_or(Value::Nil),
        _ => Value::Nil,
    }
}

pub(super) fn ct_set_extra(table: &Value, n: usize, v: Value) {
    if let Value::Record(r) = table {
        if r.borrow().len() > 3 + n {
            r.borrow_mut()[3 + n] = v;
        }
    }
}

/// GNU casetab.c `set_case_table' (the shared body of `set-case-table'
/// and `set-standard-case-table'): derive the UPCASE/CANON/EQV extra
/// slots of TABLE when they are nil.  GNU builds each table with
/// `map_char_table' over the *effective* entries of the source table:
///
///   up:    identity pass, then `shuffle' pass cycling each class
///   canon: canon[range] = down[up[down[c]]]   (FIXNATP elts only)
///   eqv:   identity + shuffle over the canon table's runs
///
/// and finally stashes `canon.extras[2] = eqv' so regexp's
/// set_image_of_range_1 can find it.
pub(crate) fn set_case_table_fill(i: &mut Interp, table: &Value) {
    let case_table_sym = Value::Sym(i.q("case-table"));
    let up = ct_extra(table, 0);
    let canon = ct_extra(table, 1);
    let eqv = ct_extra(table, 2);

    let up = if up.is_nil() {
        let u = make_ct(
            i,
            case_table_sym.clone(),
            Value::Nil,
            vec![Value::Nil, Value::Nil, Value::Nil],
        );
        // set_identity pass over TABLE's runs, then shuffle pass.
        // GNU's callbacks act only on FIXNATP elts — nil-valued runs
        // (and strings/symbols) contribute nothing to either pass.
        let runs: Vec<(u32, u32, u32)> = ct_effective_runs(i, table, true)
            .into_iter()
            .filter_map(|(f, t, v)| match v {
                Value::Int(e) if e >= 0 => Some((f, t, e as u32)),
                _ => None,
            })
            .collect();
        for (from, to, _) in &runs {
            for c in *from..=*to {
                ct_set(i, &u, c, Value::Int(c as i128));
            }
        }
        for (from, to, e) in &runs {
            for c in *from..=*to {
                let tem = char_table_ref(i, &u, *e as usize);
                ct_set(i, &u, *e, Value::Int(c as i128));
                ct_set(i, &u, c, tem.clone());
            }
        }
        ct_set_extra(table, 0, u.clone());
        u
    } else {
        up
    };

    let canon = if canon.is_nil() {
        let cn = make_ct(
            i,
            case_table_sym.clone(),
            Value::Nil,
            vec![Value::Nil, Value::Nil, Value::Nil],
        );
        ct_set_extra(table, 1, cn.clone());
        // set_canon: canon[range] = table[up[elt]].
        for (from, to, elt) in ct_effective_runs(i, table, true) {
            let Value::Int(e) = elt else { continue };
            if e < 0 {
                continue;
            }
            let uv = char_table_ref(i, &up, e as usize);
            let dv = match uv {
                Value::Int(n) if n >= 0 => char_table_ref(i, table, n as usize),
                _ => Value::Nil,
            };
            ct_set_range(i, &cn, from, to, dv);
        }
        cn
    } else {
        canon
    };

    if eqv.is_nil() {
        let eq = make_ct(
            i,
            case_table_sym.clone(),
            Value::Nil,
            vec![Value::Nil, Value::Nil, Value::Nil],
        );
        let runs: Vec<(u32, u32, u32)> = ct_effective_runs(i, &canon, true)
            .into_iter()
            .filter_map(|(f, t, v)| match v {
                Value::Int(e) if e >= 0 => Some((f, t, e as u32)),
                _ => None,
            })
            .collect();
        for (from, to, _) in &runs {
            for c in *from..=*to {
                ct_set(i, &eq, c, Value::Int(c as i128));
            }
        }
        for (from, to, e) in &runs {
            for c in *from..=*to {
                let tem = char_table_ref(i, &eq, *e as usize);
                ct_set(i, &eq, *e, Value::Int(c as i128));
                ct_set(i, &eq, c, tem.clone());
            }
        }
        ct_set_extra(table, 2, eq.clone());
        // canon.extras[2] = eqv (regex's set_image_of_range_1).
        ct_set_extra(&canon, 2, eq);
    }
}

/// GNU `buffer.c:1150' (`init_buffer'): if the standard case table's
/// extras aren't all char-tables, re-run `set_case_table' on it so
/// the extras recompute.  Called wherever GNU would create a buffer.
pub(crate) fn heal_standard_case_table(i: &mut Interp) {
    let Some(st) = i.standard_case_table.clone() else {
        return;
    };
    let ok = (0..3).all(|n| is_char_table(i, &ct_extra(&st, n)));
    if !ok {
        set_case_table_fill(i, &st);
    }
}

/// Record slot 1 of a char-table holds its subtype symbol.
pub(super) fn char_table_subtype_of(v: &Value) -> Value {
    match v {
        Value::Record(r) => r.borrow().get(1).cloned().unwrap_or(Value::Nil),
        _ => Value::Nil,
    }
}

pub(super) fn f_char_table_p(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    Ok(Value::from_bool(is_char_table(i, &a[0])))
}

pub(super) fn f_char_table_subtype(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    if !is_char_table(i, &a[0]) {
        return Err(i.wrong_type_mut("char-table-p", &a[0]));
    }
    Ok(char_table_subtype_of(&a[0]))
}

pub(super) fn f_char_table_extra_slot(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU `Fchar_table_extra_slot': CHECK_CHAR_TABLE, then N is
    // `fixnump' (non-fixnums are `(fixnump N)'; negatives fall to
    // `args-out-of-range').
    if !is_char_table(i, &a[0]) {
        return Err(i.wrong_type_mut("char-table-p", &a[0]));
    }
    let Value::Record(r) = &a[0] else {
        unreachable!()
    };
    let Value::Int(n) = &a[1] else {
        return Err(i.wrong_type_mut("fixnump", &a[1]));
    };
    let slots = r.borrow().len().saturating_sub(3);
    let n = usize::try_from(*n).ok().filter(|u| *u < slots);
    let Some(n) = n else {
        let s = i.q("args-out-of-range");
        return Err(i.signal_data(s, vec![a[0].clone(), a[1].clone()]));
    };
    Ok(r.borrow()[3 + n].clone())
}

pub(super) fn f_set_char_table_extra_slot(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    if !is_char_table(i, &a[0]) {
        return Err(i.wrong_type_mut("char-table-p", &a[0]));
    }
    let Value::Record(r) = &a[0] else {
        unreachable!()
    };
    let Value::Int(n) = &a[1] else {
        return Err(i.wrong_type_mut("fixnump", &a[1]));
    };
    let mut rr = r.borrow_mut();
    let slots = rr.len().saturating_sub(3);
    let n = usize::try_from(*n).ok().filter(|u| *u < slots);
    let Some(n) = n else {
        let s = i.q("args-out-of-range");
        return Err(i.signal_data(s, vec![a[0].clone(), a[1].clone()]));
    };
    rr[3 + n] = a[2].clone();
    Ok(a[2].clone())
}

/// `optimize-char-table' — collapse uniform sub-tables bottom-up
/// (TEST defaults to `equal'; `eq' uses identity).
pub(super) fn f_optimize_char_table(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    if !is_char_table(i, &a[0]) {
        return Err(i.wrong_type_mut("char-table-p", &a[0]));
    }
    let test = arg(&a, 1);
    ct_optimize(i, &a[0].clone(), &test)?;
    Ok(Value::Nil)
}
