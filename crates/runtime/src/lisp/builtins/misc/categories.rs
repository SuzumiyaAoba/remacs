//! GNU category tables, documentation and character membership.

use super::bool_vectors::bool_vec_of;
use super::bool_vectors::is_bool_vector;
use super::bool_vectors::make_bool_vector;
use super::chartables::CHARTAB_BITS;
use super::chartables::CHARTAB_SIZE;
use super::chartables::CT_MAX_CHAR;
use super::chartables::char_table_ref;
use super::chartables::char_table_subtype_of;
use super::chartables::char_table_vec;
use super::chartables::ct_copy;
use super::chartables::ct_effective_runs;
use super::chartables::ct_extra;
use super::chartables::ct_refresh_ascii;
use super::chartables::ct_runs_in;
use super::chartables::ct_set;
use super::chartables::ct_set_extra;
use super::chartables::ct_set_range;
use super::chartables::is_char_table;
use super::chartables::is_sub_ct;
use super::chartables::make_ct;
use super::chartables::make_sub_ct;
use super::chartables::sub_ct_parts;
use crate::lisp::builtins::{arg, want_string};
use crate::lisp::{
    Interp,
    error::{EvalResult, Flow},
    obarray::sym,
    value::Value,
};
use std::{cell::RefCell, rc::Rc};

// ---------- category tables ----------
// Record layout: [char-table, 'category-table, Vec contents,
// Vec docstrings(95), category-set hash].

/// Build a category table value. `standard` populates GNU's ASCII
/// membership and label docstrings.
pub(crate) fn make_category_table_value(i: &mut Interp, standard: bool) -> Value {
    // GNU extra slot 0: docstring vector of 95 (indexed by cat - 32);
    // extra slot 1 starts nil and becomes an `equal' hash-table
    // hash-consing category sets on the first `modify-category-entry'.
    let docs = crate::lisp::Value::vector_ref(vec![Value::Nil; 95]);
    let tag = Value::Sym(i.q("category-table"));
    let t = make_ct(
        i,
        tag,
        Value::Nil,
        vec![Value::Vec(docs.clone()), Value::Nil],
    );
    // GNU init_category_once leaves raw entries nil, with a distinct
    // empty category set as their inherited default.
    let empty = make_bool_vector(i, vec![false; 128]);
    i.set_char_table_defalt(&t, empty);
    if standard {
        // GNU's standard-category-table ASCII defaults.
        const DOCS: &[(usize, &str)] = &[
            (
                32,
                "space for indent\nThis character counts as a space for indentation purposes.",
            ),
            (49, "base vowel\nBase (independent) vowel"),
            (
                50,
                "upper diacritic\nUpper diacritical mark (including upper vowel)",
            ),
            (
                51,
                "lower diacritic\nLower diacritical mark (including lower vowel)",
            ),
            (52, "combining tone\nCombining tone mark"),
            (53, "symbol"),
            (54, "digit"),
            (55, "vowel diacritic\nVowel-modifying diacritical mark"),
            (56, "vowel-signs"),
            (57, "semivowel lower"),
            (
                60,
                "Not at eol\nA character which can't be placed at end of line.",
            ),
            (
                62,
                "Not at bol\nA character which can't be placed at beginning of line.",
            ),
            (
                65,
                "2-byte alnum\nAlphanumeric characters of 2-byte character sets",
            ),
            (
                67,
                "2-byte han\nChinese (Han) characters of 2-byte character sets",
            ),
            (
                71,
                "2-byte Greek\nGreek characters of 2-byte character sets",
            ),
            (
                72,
                "2-byte Hiragana\nJapanese Hiragana characters of 2-byte character sets",
            ),
            (73, "Indian Glyphs"),
            (
                75,
                "2-byte Katakana\nJapanese Katakana characters of 2-byte character sets",
            ),
            (
                76,
                "Strong L2R\nCharacters with \"strong\" left-to-right directionality, i.e.\nwith L, LRE, or LRO Unicode bidi character type.",
            ),
            (
                78,
                "2-byte Korean\nKorean Hangul characters of 2-byte character sets",
            ),
            (
                82,
                "Strong R2L\nCharacters with \"strong\" right-to-left directionality, i.e.\nwith R, AL, RLE, or RLO Unicode bidi character type.",
            ),
            (
                89,
                "2-byte Cyrillic\nCyrillic characters of 2-byte character sets",
            ),
            (
                94,
                "Combining\nCombining diacritic or mark (Unicode General Category M)",
            ),
            (
                97,
                "ASCII\nASCII graphic characters 32-126 (ISO646 IRV:1983[4/0])",
            ),
            (98, "Arabic"),
            (99, "Chinese"),
            (101, "Ethiopic\nEthiopic (Ge'ez)"),
            (103, "Greek"),
            (104, "Korean"),
            (105, "Indian"),
            (106, "Japanese"),
            (107, "Katakana\nJapanese katakana"),
            (111, "Lao"),
            (113, "Tibetan"),
            (114, "Roman\nJapanese roman"),
            (116, "Thai"),
            (118, "Viet\nVietnamese"),
            (119, "Hebrew"),
            (121, "Cyrillic"),
            (
                124,
                "line breakable\nWhile filling, we can break a line at this character.",
            ),
        ];
        {
            let mut d = docs.borrow_mut();
            for &(label, doc) in DOCS {
                d[label - 32] = Value::string(doc);
            }
        }
        for ch in 32u32..=126 {
            let mut bits = vec![false; 128];
            for b in [46usize, 97, 108] {
                bits[b] = true;
            }
            if ch != 32 && ch != 92 && ch != 126 {
                bits[114] = true;
            }
            let c = char::from_u32(ch).unwrap();
            if c.is_ascii_alphabetic() {
                bits[76] = true;
            }
            if c.is_ascii_digit() {
                bits[54] = true;
            }
            let bv = make_bool_vector(i, bits);
            ct_set(i, &t, ch, bv);
        }
    }
    t
}

fn is_category_table(i: &Interp, v: &Value) -> bool {
    is_char_table(i, v)
        && matches!(char_table_subtype_of(v), Value::Sym(s) if i.symbol_name(s) == "category-table")
}

fn want_category_table(i: &mut Interp, v: &Value) -> Result<Rc<RefCell<Vec<Value>>>, Flow> {
    match v {
        Value::Nil => Ok(
            match i
                .current_buffer_ref()
                .and_then(|b| b.borrow().category_table.clone())
            {
                Some(t) => match t {
                    Value::Record(r) => r,
                    _ => return Err(i.wrong_type_mut("category-table-p", &Value::Nil)),
                },
                None => match i.standard_category_table() {
                    Value::Record(r) => r,
                    _ => unreachable!(),
                },
            },
        ),
        Value::Record(r) if is_category_table(i, v) => Ok(r.clone()),
        other => Err(i.wrong_type_mut("category-table-p", other)),
    }
}

/// The docstring vec of a category table (record slot 3).
fn cat_docs(v: &Rc<RefCell<Vec<Value>>>) -> Rc<RefCell<Vec<Value>>> {
    let rr = v.borrow();
    match rr.get(3) {
        Some(Value::Vec(d)) => d.clone(),
        _ => crate::lisp::Value::vector_ref(vec![Value::Nil; 95]),
    }
}

pub(super) fn f_make_category_table(i: &mut Interp, _a: crate::lisp::Argv) -> EvalResult {
    let table = make_category_table_value(i, false);
    let contents = char_table_vec(&table).unwrap();
    // Unlike the standard table, GNU fills each top-level block with
    // its own mutable set.  Its ASCII cache remains nil initially.
    for slot in 1..=64 {
        contents.borrow_mut()[slot] = make_bool_vector(i, vec![false; 128]);
    }
    Ok(table)
}

pub(super) fn f_category_table_p(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    Ok(Value::from_bool(is_category_table(i, &a[0])))
}

pub(super) fn f_standard_category_table(i: &mut Interp, _a: crate::lisp::Argv) -> EvalResult {
    Ok(i.standard_category_table())
}

pub(super) fn f_category_table(i: &mut Interp, _a: crate::lisp::Argv) -> EvalResult {
    let v = i
        .current_buffer_ref()
        .and_then(|b| b.borrow().category_table.clone())
        .unwrap_or_else(|| i.standard_category_table());
    Ok(v)
}

pub(super) fn f_set_category_table(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    if !is_category_table(i, &a[0]) {
        return Err(i.wrong_type_mut("category-table-p", &a[0]));
    }
    if let Some(b) = i.current_buffer_ref() {
        b.borrow_mut().category_table = Some(a[0].clone());
    }
    Ok(a[0].clone())
}

pub(super) fn f_copy_category_table(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let source = match a.first() {
        None | Some(Value::Nil) => i.standard_category_table(),
        Some(value) => Value::Record(want_category_table(i, value)?),
    };
    let table = ct_copy(i, &source);
    // GNU copies mutable category sets and the documentation vector,
    // but leaves the category-set interning hash shared.
    let default = i.char_table_defalt(&table);
    if !default.is_nil() {
        let default = crate::lisp::builtins::seq::f_copy_sequence(i, vec![default].into())?;
        i.set_char_table_defalt(&table, default);
    }
    let docs = crate::lisp::builtins::seq::f_copy_sequence(i, vec![ct_extra(&table, 0)].into())?;
    ct_set_extra(&table, 0, docs);
    for (from, to, value) in ct_effective_runs(i, &table, true) {
        if !value.is_nil() {
            let copied = crate::lisp::builtins::seq::f_copy_sequence(i, vec![value].into())?;
            ct_set_range(i, &table, from, to, copied);
        }
    }
    Ok(table)
}

pub(super) fn f_define_category(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU `CHECK_CATEGORY': CATEGORY must be a printable-ASCII fixnum
    // (32..=126); anything else is `(categoryp CATEGORY)'.
    let cat = match &a[0] {
        Value::Int(c) if (32..=126).contains(c) => *c,
        _ => return Err(i.wrong_type_mut("categoryp", &a[0])),
    };
    let doc = want_string(i, &a[1])?;
    let t = want_category_table(i, &arg(&a, 2))?;
    let docs = cat_docs(&t);
    if !docs.borrow()[cat as usize - 32].is_nil() {
        return Err(i.signal_data(
            sym::ERROR,
            vec![Value::string(format!(
                "Category ‘{}’ is already defined",
                char::from_u32(cat as u32).unwrap_or('?')
            ))],
        ));
    }
    docs.borrow_mut()[cat as usize - 32] = Value::string(doc);
    Ok(Value::Nil)
}

pub(super) fn f_category_docstring(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let cat = match &a[0] {
        Value::Int(c) if (32..=126).contains(c) => *c,
        _ => return Err(i.wrong_type_mut("categoryp", &a[0])),
    };
    let t = want_category_table(i, &arg(&a, 1))?;
    Ok(cat_docs(&t).borrow()[cat as usize - 32].clone())
}

pub(super) fn f_get_unused_category(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let t = want_category_table(i, &arg(&a, 0))?;
    let docs = cat_docs(&t);
    let dd = docs.borrow();
    for c in 32usize..=126 {
        if dd[c - 32].is_nil() {
            return Ok(Value::Int(c as i128));
        }
    }
    Ok(Value::Nil)
}
/// Extra slot 1 of a category table: GNU's `equal' hash-table
/// hash-consing category sets (created lazily by
/// `modify-category-entry'), so equal sets are `eq'.
fn cat_set_hash(t: &Rc<RefCell<Vec<Value>>>) -> Rc<RefCell<crate::lisp::value::LispHash>> {
    {
        let b = t.borrow();
        if let Some(Value::Hash(h)) = b.get(4) {
            return h.clone();
        }
    }
    let h = crate::lisp::memory::allocate(
        crate::lisp::value::LispHash::new(crate::lisp::value::HashTest::Equal),
        crate::lisp::Value::Hash,
    );
    if t.borrow().len() > 4 {
        t.borrow_mut()[4] = Value::Hash(h.clone());
    }
    h
}

pub(super) fn f_modify_category_entry(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // CHAR may be a char or a (MIN . MAX) cons.  GNU checks `consp'
    // only after the FIXNATP fast path: any other type is
    // `(consp CHAR)', while ints and cons elements out of character
    // range are `(characterp ...)'.
    let _tt = (i.dbg_flags & 0x400 != 0).then(std::time::Instant::now);
    let (lo, hi) = match &a[0] {
        Value::Int(c) if (0..=CT_MAX_CHAR as i128).contains(c) => (*c, *c),
        Value::Int(_) => return Err(i.wrong_type_mut("characterp", &a[0])),
        Value::Cons(c) => {
            let (car, cdr) = {
                let r = c.borrow();
                (r.car.clone(), r.cdr.clone())
            };
            let l = match &car {
                Value::Int(n) if (0..=CT_MAX_CHAR as i128).contains(n) => *n,
                _ => return Err(i.wrong_type_mut("characterp", &car)),
            };
            let h = match &cdr {
                Value::Int(n) if (0..=CT_MAX_CHAR as i128).contains(n) => *n,
                _ => return Err(i.wrong_type_mut("characterp", &cdr)),
            };
            (l, h)
        }
        other => return Err(i.wrong_type_mut("consp", other)),
    };
    let cat = match &a[1] {
        Value::Int(c) if (32..=126).contains(c) => *c,
        _ => return Err(i.wrong_type_mut("categoryp", &a[1])),
    };
    let _tc = _tt.is_some().then(std::time::Instant::now);
    let reset = !arg(&a, 3).is_nil();
    let t = want_category_table(i, &arg(&a, 2))?;
    if cat_docs(&t).borrow()[cat as usize - 32].is_nil() {
        return Err(i.signal_data(
            sym::ERROR,
            vec![Value::string(format!(
                "Undefined category: {}",
                char::from_u32(cat as u32).unwrap_or('?')
            ))],
        ));
    }
    if let Some(tc) = _tc {
        eprintln!("[mce-prolog] {}us", tc.elapsed().as_micros());
    }
    if lo > hi {
        // GNU: a reversed range is a silent no-op.
        return Ok(Value::Nil);
    }
    // Fast path: the common `(modify-category-entry RANGE CAT)' form
    // from characters.el / map-charset-chars — parentless table, plain
    // scalar defalt.  Fuse the old-value walk and the write into one
    // descent: each covered slot's new set derives from its old
    // category-set via a per-object cache (`mce_new_set'), so no
    // intermediate run list or per-run trie re-descent is needed.
    if arg(&a, 2).is_nil() {
        let defalt = i.char_table_defalt(&Value::Record(t.clone()));
        let no_parent = i.char_table_parent(&Value::Record(t.clone())).is_nil();
        if no_parent && !is_sub_ct(i, &defalt) {
            let _tf = _tt.is_some().then(std::time::Instant::now);
            let mut mce_cache: rustc_hash::FxHashMap<usize, Value> = Default::default();
            mce_fused(
                i,
                &t,
                lo as u32,
                hi as u32,
                cat as usize,
                reset,
                &defalt,
                &mut mce_cache,
            );
            if let Some(tf) = _tf {
                eprintln!("[mce-fused] {}us", tf.elapsed().as_micros());
            }
            return Ok(Value::Nil);
        }
    }
    let table = Value::Record(t.clone());
    // Group the range by runs of equal old category-set: one
    // bool-vector per run shared over the whole run.  Run
    // boundaries come from `ct_runs_in' — leaf blocks of the
    // char-table trie — instead of a per-character `char_table_ref'
    // walk (multi-GB worth of lookups on CJK-wide categories).
    // Adjacent runs carrying the SAME `old' set yield the same
    // `set' — reuse it without re-hashing, and merge the writes
    // into one `ct_set_range' covering the whole span.
    let mut pending: Option<(u32, u32, Value)> = None; // (start,end,set)
    // Cross-run cache keyed on the old object's identity (Rc pointer;
    // nil/other immediates share one slot): identical `old' values
    // recur non-adjacently too, so a single last-entry cache misses
    // on alternating patterns.
    let mut old_cache: rustc_hash::FxHashMap<usize, Value> = Default::default();
    let _t0 = (i.dbg_flags & 0x400 != 0).then(std::time::Instant::now);
    let _runs_v = ct_runs_in(i, &table, lo as u32, hi as u32);
    if let Some(t0) = _t0 {
        eprintln!(
            "[mce] runs_in {} runs {}us",
            _runs_v.len(),
            t0.elapsed().as_micros()
        );
    }
    let _post_t = (i.dbg_flags & 0x400 != 0).then(std::time::Instant::now);
    for (c, e, old) in _runs_v {
        // New set = old bits with bit CAT = !RESET (an unset cell
        // starts from all-zero — GNU stores that empty set, never
        // leaves nil behind), then hash-consed through extra slot 1
        // so equal sets share one bool-vector object.
        let key = match &old {
            Value::Record(r) => std::rc::Rc::as_ptr(r) as usize,
            Value::Vec(v) => std::rc::Rc::as_ptr(v) as usize,
            Value::Cons(v) => std::rc::Rc::as_ptr(v) as usize,
            _ => usize::MAX,
        };
        let set = match old_cache.get(&key) {
            Some(s) => s.clone(),
            None => {
                let mut bits = vec![false; 128];
                if is_bool_vector(i, &old) {
                    if let Ok(old_bits) = bool_vec_of(i, &old) {
                        for (k, v) in old_bits.iter().enumerate() {
                            if k < 128 {
                                bits[k] = *v;
                            }
                        }
                    }
                }
                bits[cat as usize] = !reset;
                let newv = make_bool_vector(i, bits);
                let hk = crate::lisp::builtins::hashfn::hash_key_for(
                    i,
                    &newv,
                    crate::lisp::value::HashTest::Equal,
                );
                let h = cat_set_hash(&t);
                let s = {
                    let mut hm = h.borrow_mut();
                    match hm.map.get(&hk) {
                        Some(v) => v.clone(),
                        None => {
                            hm.map.insert(hk.clone(), newv.clone());
                            hm.put_key(hk, newv.clone());
                            newv.clone()
                        }
                    }
                };
                old_cache.insert(key, s.clone());
                s
            }
        };
        match pending {
            Some((pc, pe, ref ps)) if crate::lisp::builtins::eq_values(ps, &set) && pe + 1 == c => {
                pending = Some((pc, e, set));
            }
            Some((pc, pe, ps)) => {
                if pe == pc {
                    ct_set(i, &table, pc, ps);
                } else {
                    ct_set_range(i, &table, pc, pe, ps);
                }
                pending = Some((c, e, set));
            }
            None => pending = Some((c, e, set)),
        }
    }
    if let Some(t0) = _post_t {
        eprintln!("[mce] body {}us", t0.elapsed().as_micros());
    }
    if let Some((pc, pe, ps)) = pending {
        if pe == pc {
            ct_set(i, &table, pc, ps);
        } else {
            ct_set_range(i, &table, pc, pe, ps);
        }
    }
    if let Some(tt) = _tt {
        eprintln!("[mce-total] {}us", tt.elapsed().as_micros());
    }
    Ok(Value::Nil)
}

/// Pre-resolved `modify-category-entry' fast call: the standard
/// category table + CAT + RESET validated once, then every (lo..=hi)
/// range writes through `mce_fused' with a shared old→new cache.
/// `mce_fast_for' returns None for anything needing the full
/// `f_modify_category_entry' semantics (explicit TABLE arg, parented
/// table, invalid CAT) so the caller falls back to `apply'.
pub(crate) struct MceFast {
    t: std::rc::Rc<RefCell<Vec<Value>>>,
    cat: usize,
    defalt: Value,
    cache: rustc_hash::FxHashMap<usize, Value>,
}

pub(crate) fn mce_fast_for(i: &mut Interp, function: &Value, arg: &Value) -> Option<MceFast> {
    let Value::Subr(s) = function else {
        return None;
    };
    if s.name != "modify-category-entry" {
        return None;
    }
    let Value::Int(c) = arg else {
        return None;
    };
    if !(32..=126).contains(c) {
        return None; // let the real call raise `categoryp'
    }
    let t = want_category_table(i, &Value::Nil).ok()?;
    if cat_docs(&t).borrow()[*c as usize - 32].is_nil() {
        return None; // undefined category must still signal
    }
    let tv = Value::Record(t.clone());
    if !i.char_table_parent(&tv).is_nil() {
        return None;
    }
    let defalt = i.char_table_defalt(&tv);
    if is_sub_ct(i, &defalt) {
        return None;
    }
    Some(MceFast {
        t,
        cat: *c as usize,
        defalt,
        cache: Default::default(),
    })
}

impl MceFast {
    /// `(modify-category-entry (LO . HI) CAT)' — RESET always nil here
    /// since `map-charset-chars' passes exactly two arguments.
    pub(crate) fn emit(&mut self, i: &mut Interp, lo: i64, hi: i64) {
        if lo > hi {
            return;
        }
        mce_fused(
            i,
            &self.t,
            lo as u32,
            hi as u32,
            self.cat,
            false,
            &self.defalt,
            &mut self.cache,
        );
    }
}

/// New category set for `cat' derived from the slot's old value OLD:
/// copy bits, set CAT = !RESET, hash-cons through the table's extra
/// slot so equal sets share one bool-vector (same rule as
/// `f_modify_category_entry').  Results are cached by the old
/// object's identity in CACHE — identical sets recur heavily across
/// a call (nil leaves, alternating bool-vectors).
fn mce_new_set(
    i: &mut Interp,
    t: &Rc<RefCell<Vec<Value>>>,
    old: &Value,
    cat: usize,
    reset: bool,
    cache: &mut rustc_hash::FxHashMap<usize, Value>,
) -> Value {
    let key = match old {
        Value::Record(r) => std::rc::Rc::as_ptr(r) as usize,
        Value::Vec(v) => std::rc::Rc::as_ptr(v) as usize,
        Value::Cons(v) => std::rc::Rc::as_ptr(v) as usize,
        _ => usize::MAX,
    };
    if let Some(s) = cache.get(&key) {
        return s.clone();
    }
    let mut bits = vec![false; 128];
    if is_bool_vector(i, old) {
        if let Ok(old_bits) = bool_vec_of(i, old) {
            for (k, v) in old_bits.iter().enumerate() {
                if k < 128 {
                    bits[k] = *v;
                }
            }
        }
    }
    bits[cat] = !reset;
    let newv = make_bool_vector(i, bits);
    let hk =
        crate::lisp::builtins::hashfn::hash_key_for(i, &newv, crate::lisp::value::HashTest::Equal);
    let h = cat_set_hash(t);
    let s = {
        let mut hm = h.borrow_mut();
        match hm.map.get(&hk) {
            Some(v) => v.clone(),
            None => {
                hm.map.insert(hk.clone(), newv.clone());
                hm.put_key(hk, newv.clone());
                newv.clone()
            }
        }
    };
    cache.insert(key, s.clone());
    s
}

/// Recursive worker for `mce_fused': write the transformed category
/// set into every slot of SUB (a sub-char-table Value) intersecting
/// [FROM,TO].  Fully covered scalar slots collapse to the new value;
/// partially covered scalars materialize a deeper sub-table first —
/// GNU `sub_char_table_set_range' shape, except the written value is
/// computed per slot rather than constant.
/// Recursive worker for `mce_fused'.  Transforms every slot of SUB
/// intersecting [FROM,TO].  Returns Some(V) when the whole sub-table
/// (not just the covered span) resolves to the single scalar V — the
/// caller then collapses SUB's slot to V, matching GNU's
/// `sub_char_table_set_range' block overwrite.  A partially covered
/// child always makes the parent non-uniform (uncovered content
/// stays mixed), so it yields None.
/// Effective old value of a whole slot block when the subtree below V
/// is *uniform* — nil slots read as DEFALT (GNU `char_table_ref' chain
/// stops at this table's defalt for `mce_fused' purposes).  Returns
/// None for mixed content: the caller must descend slot by slot.
/// Purely read-only — lets covered blocks fold without materializing
/// the (usually nil) leaf chain first.
fn mce_fold_value(i: &Interp, v: &Value, defalt: &Value) -> Option<Value> {
    if !is_sub_ct(i, v) {
        return Some(if v.is_nil() {
            defalt.clone()
        } else {
            v.clone()
        });
    }
    let (depth, _min, r) = sub_ct_parts(v)?;
    let b = r.borrow();
    let mut uni: Option<Value> = None;
    for k in 0..CHARTAB_SIZE[depth] {
        let child = b.get(3 + k).cloned().unwrap_or(Value::Nil);
        let vv = mce_fold_value(i, &child, defalt)?;
        match &uni {
            Some(u) if crate::lisp::builtins::eq_values(u, &vv) => {}
            Some(_) => return None,
            None => uni = Some(vv),
        }
    }
    uni
}

fn mce_sub(
    i: &mut Interp,
    sub: &Value,
    from: u32,
    to: u32,
    cat: usize,
    reset: bool,
    t: &Rc<RefCell<Vec<Value>>>,
    defalt: &Value,
    cache: &mut rustc_hash::FxHashMap<usize, Value>,
) -> Option<Value> {
    let (depth, min, r) = sub_ct_parts(sub)?;
    let chars = 1u32 << CHARTAB_BITS[depth];
    let lim = CHARTAB_SIZE[depth];
    let lo = from.max(min);
    let mut idx = (lo.saturating_sub(min) >> CHARTAB_BITS[depth]) as usize;
    let mut c = min + chars * idx as u32;
    // UNIFORM tracks one foldable value; starts unset and becomes
    // Some(v) once a covered slot resolves — any later covered slot
    // with a different value, or any uncovered child that keeps mixed
    // content, breaks uniformity.
    let mut uniform: Option<Value> = None;
    let mut broke = false;
    if depth == 3 {
        // Leaf level: one slot = one character.  One borrow to read the
        // old values, then one borrow_mut for the whole covered write —
        // the transform cost is a per-object cache lookup (identical
        // `old' objects share one entry), not per-character work.
        let base = c;
        let mut k = idx;
        let mut cc = c;
        let olds: Vec<Value> = {
            let b = r.borrow();
            let mut v: Vec<Value> = Vec::with_capacity(lim - k);
            while k < lim && cc <= to {
                v.push(b.get(3 + k).cloned().unwrap_or(Value::Nil));
                k += 1;
                cc += 1;
            }
            v
        };
        let end = k;
        let mut sets: Vec<Value> = Vec::with_capacity(end - idx);
        for (j, old) in olds.iter().enumerate() {
            if from <= base + j as u32 {
                let old_eff = if old.is_nil() {
                    defalt.clone()
                } else {
                    old.clone()
                };
                sets.push(mce_new_set(i, t, &old_eff, cat, reset, cache));
            } else {
                sets.push(old.clone());
            }
        }
        {
            let mut b = r.borrow_mut();
            for (j, set) in sets.iter().enumerate() {
                if from <= base + j as u32 {
                    b[3 + idx + j] = set.clone();
                    match &uniform {
                        Some(u) if crate::lisp::builtins::eq_values(u, set) => {}
                        Some(_) => broke = true,
                        None => uniform = Some(set.clone()),
                    }
                }
            }
        }
        idx = k;
        c = cc;
    }
    while idx < lim && c <= to {
        let covered = from <= c && c + chars - 1 <= to;
        if !covered {
            // Uncovered content remains inside SUB: the parent can't
            // fold this sub-table to a single scalar.
            if depth < 3 {
                let child = r.borrow()[3 + idx].clone();
                if is_sub_ct(i, &child) {
                    mce_sub(i, &child, from, to, cat, reset, t, defalt, cache);
                } else {
                    let n = make_sub_ct(i, depth + 1, c, child);
                    r.borrow_mut()[3 + idx] = n.clone();
                    mce_sub(i, &n, from, to, cat, reset, t, defalt, cache);
                }
            }
            broke = true;
        } else {
            // Covered slot: try the read-only fold first — a uniform
            // subtree collapses to one scalar write WITHOUT touching
            // (or materializing) its leaves.  Only mixed content
            // descends and writes per-slot.
            let child = r.borrow()[3 + idx].clone();
            let folded = mce_fold_value(i, &child, defalt)
                .map(|old_eff| mce_new_set(i, t, &old_eff, cat, reset, cache));
            match folded {
                Some(v) => {
                    r.borrow_mut()[3 + idx] = v.clone();
                    match &uniform {
                        Some(u) if crate::lisp::builtins::eq_values(u, &v) => {}
                        Some(_) => broke = true,
                        None => uniform = Some(v),
                    }
                }
                None => {
                    let v = if is_sub_ct(i, &child) {
                        mce_sub(i, &child, from, to, cat, reset, t, defalt, cache)
                    } else {
                        let old_eff = if child.is_nil() {
                            defalt.clone()
                        } else {
                            child
                        };
                        Some(mce_new_set(i, t, &old_eff, cat, reset, cache))
                    };
                    match v {
                        Some(v) => {
                            r.borrow_mut()[3 + idx] = v.clone();
                            match &uniform {
                                Some(u) if crate::lisp::builtins::eq_values(u, &v) => {}
                                Some(_) => broke = true,
                                None => uniform = Some(v),
                            }
                        }
                        None => broke = true,
                    }
                }
            }
        }
        idx += 1;
        c += chars;
    }
    // This sub-table folds only if the covered span hit every slot
    // (whole block) AND all resolved to one scalar.
    if broke || idx < lim || lo > min {
        return None;
    }
    uniform
}

/// Fused `modify-category-entry' for a parentless category table:
/// one descent writes [LO,HI] (inclusive) at all levels.
fn mce_fused(
    i: &mut Interp,
    t: &Rc<RefCell<Vec<Value>>>,
    lo: u32,
    hi: u32,
    cat: usize,
    reset: bool,
    defalt: &Value,
    cache: &mut rustc_hash::FxHashMap<usize, Value>,
) {
    // T is the char-table Record; its contents vector is slot 2.
    let contents = match t.borrow().get(2).cloned() {
        Some(Value::Vec(v)) => v,
        _ => return,
    };
    let mut ti = (lo >> 16) as usize;
    let lim = (hi.min(CT_MAX_CHAR) >> 16) as usize;
    while ti <= lim {
        let c = (ti as u32) << 16;
        if c > hi {
            break;
        }
        let slot = {
            let b = contents.borrow();
            b.get(1 + ti).cloned().unwrap_or(Value::Nil)
        };
        let covered = lo <= c && c + 65535 <= hi;
        // Covered 64k block: fold the slot's effective value — uniform
        // content (a nil slot inheriting DEFALT included) collapses to
        // one scalar write without materializing the leaf chain.
        let folded = if covered {
            mce_fold_value(i, &slot, defalt)
        } else {
            None
        };
        if let Some(old_eff) = folded {
            let set = mce_new_set(i, t, &old_eff, cat, reset, cache);
            contents.borrow_mut()[1 + ti] = set;
        } else {
            let sub = if is_sub_ct(i, &slot) {
                slot
            } else {
                let n = make_sub_ct(i, 1, c, slot);
                contents.borrow_mut()[1 + ti] = n.clone();
                n
            };
            if let Some(v) = mce_sub(i, &sub, lo, hi, cat, reset, t, defalt, cache) {
                // Whole 64k block resolved to one scalar — collapse.
                contents.borrow_mut()[1 + ti] = v;
            }
        }
        ti += 1;
    }
    if lo < 128 {
        ct_refresh_ascii(i, &contents);
    }
}

pub(super) fn f_char_category_set(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU `CHECK_CHARACTER' — non-chars are `(characterp CH)'.
    let ch = match &a[0] {
        Value::Int(n) if (0..=CT_MAX_CHAR as i128).contains(n) => *n,
        _ => return Err(i.wrong_type_mut("characterp", &a[0])),
    };
    let t = want_category_table(i, &Value::Nil)?;
    let table = Value::Record(t);
    Ok(match char_table_ref(i, &table, ch as usize) {
        v if !v.is_nil() => v,
        _ => make_bool_vector(i, vec![false; 128]),
    })
}

pub(super) fn f_category_set_mnemonics(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let bits = bool_vec_of_cat(i, &a[0])?;
    let s: String = bits
        .iter()
        .enumerate()
        .filter(|(_, b)| **b)
        .filter_map(|(k, _)| char::from_u32(k as u32))
        .collect();
    Ok(Value::string(s))
}

fn bool_vec_of_cat(i: &mut Interp, v: &Value) -> Result<Vec<bool>, Flow> {
    // GNU `CHECK_CATEGORY_SET': a bool-vector of exactly 128 bits.
    let bits = if is_bool_vector(i, v) {
        bool_vec_of(i, v)?
    } else {
        return Err(i.wrong_type_mut("categorysetp", v));
    };
    if bits.len() != 128 {
        return Err(i.wrong_type_mut("categorysetp", v));
    }
    Ok(bits)
}

pub(super) fn f_make_category_set(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU: STRING must be unibyte ("Multibyte string in
    // `make-category-set'"), and every byte must name a category
    // (printable ASCII 32..126 — `(categoryp BYTE)').
    let Value::Str(sref) = &a[0] else {
        return Err(i.wrong_type_mut("stringp", &a[0]));
    };
    let chars: Vec<crate::lisp::value::EmacsChar> = sref.borrow().chars().collect();
    let multibyte = if i.is_unibyte_str(sref) {
        false
    } else if i.is_multibyte_str(sref) {
        true
    } else {
        // Unmarked strings are multibyte when any char exceeds the
        // byte range (same storage rule as `multibyte-string-p');
        // eight-bit proxies in a still-unmarked string count as
        // bytes, not multibyte chars.
        chars
            .iter()
            .any(|&c| (c).code() as u32 > 0xFF && crate::lisp::value::eight_bit_byte(c).is_none())
    };
    if multibyte {
        return Err(i.error_q::<str>("Multibyte string in `make-category-set'", &[]));
    }
    let mut bits = vec![false; 128];
    for c in chars {
        let k = crate::lisp::value::eight_bit_byte(c)
            .map(|b| b as usize)
            .unwrap_or((c).code() as usize);
        if !(32..=126).contains(&k) {
            return Err(i.wrong_type_mut("categoryp", &Value::Int(k as i128)));
        }
        bits[k] = true;
    }
    Ok(make_bool_vector(i, bits))
}
