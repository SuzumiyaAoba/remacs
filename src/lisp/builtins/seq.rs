//! Sequence subrs: elt, aref, aset, copy-sequence, mapcar, sort, etc.

use std::cell::RefCell;
use std::rc::Rc;

use super::listfn::err_circular;
use super::{S, arg, eq_values, equal_values, want_int, want_list, want_string};
use crate::lisp::Interp;
use crate::lisp::error::{EvalResult, Flow};
use crate::lisp::obarray::sym;
use crate::lisp::value::{Subr, Value};

pub(crate) static SUBRS: &[Subr] = &[
    S!("elt", 2, 2, f_elt, "Return element of SEQUENCE at index N."),
    S!("aref", 2, 2, f_aref, "Return element of ARRAY at index N."),
    S!("aset", 3, 3, f_aset, "Set element of ARRAY at index N."),
    S!("copy-sequence", 1, 1, f_copy_sequence, "Copy a sequence."),
    S!("copy-seq", 1, 1, f_copy_sequence, "Copy a sequence."),
    S!(
        "mapcar",
        2,
        2,
        f_mapcar,
        "Map FUNCTION over SEQUENCE, collect results."
    ),
    S!(
        "mapc",
        2,
        2,
        f_mapc,
        "Map FUNCTION over SEQUENCE for side effects."
    ),
    S!("mapcan", 2, 2, f_mapcan, "Mapcar + nconc."),
    S!(
        "mapconcat",
        2,
        3,
        f_mapconcat,
        "Map FUNCTION over SEQUENCE, join results."
    ),
    S!(
        "maphash",
        2,
        2,
        f_maphash,
        "Map FUNCTION over hash table entries."
    ),
    S!("sort", many 1, f_sort, "Sort SEQ stably by PREDICATE."),
    S!(
        "string-to-sequence",
        1,
        2,
        f_string_to_sequence,
        "Convert string to list/vector."
    ),
    S!("seq", many 0, f_seq, "Return SEQUENCE unchanged."),
    S!("sequence", many 0, f_seq, "Return SEQUENCE unchanged."),
    S!(
        "append-to-list",
        2,
        2,
        f_append_to_list,
        "Append element to list (list + elt)."
    ),
    S!("fillarray", 2, 2, f_fillarray, "Fill ARRAY with ITEM."),
    S!(
        "make-vector",
        2,
        2,
        f_make_vector,
        "Make a vector of LENGTH with INIT."
    ),
    S!("vector", many 0, f_vector, "Make a vector of the arguments."),
    S!("bool-vector", many 0, f_bool_vector, "Make a bool-vector of the arguments."),
    S!("purecopy", 1, 1, f_purecopy, "Return OBJECT unchanged."),
    S!(
        "nreverse",
        1,
        1,
        f_nreverse_seq,
        "Reverse SEQUENCE destructively (seq version)."
    ),
    S!(
        "clear-vector",
        2,
        2,
        f_clear_vector,
        "Set all elements of VECTOR to nil."
    ),
    S!(
        "seq-concatenate",
        many 1,
        f_seq_concatenate,
        "Concatenate SEQS into TYPE."
    ),
    S!("seq-subseq", 2, 3, f_seq_subseq, "Subsequence of SEQ."),
    S!("seq-take", 2, 2, f_seq_take, "First N elements of SEQ."),
    S!(
        "seq-drop",
        2,
        2,
        f_seq_drop,
        "SEQ without first N elements."
    ),
    S!("seq-elt", 2, 2, f_elt, "seq.el elt."),
    S!("seq-length", 1, 1, f_seq_length, "Length of SEQ."),
    S!(
        "seq-do",
        2,
        2,
        f_seq_do,
        "Apply FUNCTION to each element of SEQ."
    ),
    S!(
        "seq-map",
        2,
        2,
        f_seq_map,
        "Map FUNCTION over SEQ, return list."
    ),
    S!(
        "seq-filter",
        2,
        2,
        f_seq_filter,
        "Elements of SEQ satisfying PRED."
    ),
    S!("seq-contains-p", 2, 3, f_seq_contains_p, "Is ELT in SEQ?"),
    S!("seq-position", 2, 3, f_seq_position, "Index of ELT in SEQ."),
    S!(
        "seq-count",
        2,
        2,
        f_seq_count,
        "Count elements satisfying PRED."
    ),
    S!("seq-reverse", 1, 1, f_seq_reverse, "Reversed copy of SEQ."),
    S!(
        "seq-some",
        2,
        2,
        f_seq_some,
        "First non-nil result of PRED on SEQ."
    ),
    S!(
        "seq-every-p",
        2,
        2,
        f_seq_every_p,
        "t if PRED holds for all of SEQ."
    ),
    S!(
        "seq-find",
        2,
        3,
        f_seq_find,
        "First element of SEQ satisfying PRED."
    ),
    S!(
        "seq-remove",
        2,
        2,
        f_seq_remove,
        "Remove elements satisfying PRED."
    ),
    S!(
        "seq-reduce",
        3,
        3,
        f_seq_reduce,
        "Reduce SEQ with FUNCTION and INIT."
    ),
    S!(
        "seq-take-while",
        2,
        2,
        f_seq_take_while,
        "Take while PRED holds."
    ),
    S!(
        "seq-drop-while",
        2,
        2,
        f_seq_drop_while,
        "Drop while PRED holds."
    ),
    S!("seq-copy", 1, 1, f_copy_sequence, ""),
    S!("seq-into", 2, 2, f_seq_into, "Convert SEQ to TYPE."),
    S!(
        "seq-into-sequence",
        1,
        1,
        f_seq_into_sequence,
        "Return SEQ if it is a sequence."
    ),
    S!("seq-empty-p", 1, 1, f_seq_empty_p, "t if SEQ is empty."),
    S!("seq-first", 1, 1, f_seq_first, "First element of SEQ."),
    S!("seq-rest", 1, 1, f_seq_rest, "SEQ minus first element."),
    S!("seq-last", 1, 1, f_seq_last, "Last element of SEQ."),
    S!("seq-min", 1, 1, f_seq_min, "Smallest element of SEQ."),
    S!("seq-max", 1, 1, f_seq_max, "Largest element of SEQ."),
    S!("seq-uniq", 1, 2, f_seq_uniq, "SEQ with duplicates removed."),
    S!(
        "seq-let",
        raw,
        f_seq_let_raw,
        "seq-let is a macro in lisp/."
    ),
    S!("char-table", many 0, f_vector, ""),
    S!(
        "make-char-table",
        1,
        2,
        f_make_char_table,
        "Make a char-table (approx: vector)."
    ),
];

/// Element IDX of a sequence, or nil when out of range or not a
/// sequence — GNU seq.el's `seq--elt-safe' used by `seq-let'.
fn seq_elt_safe(i: &mut Interp, seqv: &Value, idx: usize) -> Value {
    match seqv {
        Value::Nil => Value::Nil,
        Value::Cons(_) => match super::listfn::nthcdr_strict(i, seqv, idx) {
            Ok(Value::Cons(c)) => c.borrow().car.clone(),
            _ => Value::Nil,
        },
        Value::Str(s) => s
            .borrow()
            .chars()
            .nth(idx)
            .map(|c| Value::Int(crate::lisp::value::lisp_char_code(c)))
            .unwrap_or(Value::Nil),
        Value::Vec(v) => v.borrow().get(idx).cloned().unwrap_or(Value::Nil),
        _ => Value::Nil,
    }
}

/// Bind one `seq-let' pattern element: a symbol binds directly, a
/// nested list destructures recursively, nil just skips a position.
fn seq_let_bind_one(
    i: &mut Interp,
    pat: &Value,
    v: &Value,
    nbind: &mut usize,
) -> Result<(), super::Flow> {
    match pat {
        Value::Nil => Ok(()),
        Value::Sym(s) => {
            i.specbind(*s, v.clone())?;
            *nbind += 1;
            Ok(())
        }
        Value::Cons(_) => seq_let_bind(i, pat, v, nbind),
        other => Err(i.wrong_type_mut("symbolp", other)),
    }
}

/// `(seq-let ARGS SEQ ...)' destructuring — mirrors seq.el's
/// seq--make-bindings: positional elements bind via `seq--elt-safe',
/// `&rest' binds `seq-drop', `&optional' is a no-op marker.
fn seq_let_bind(
    i: &mut Interp,
    spec: &Value,
    seqv: &Value,
    nbind: &mut usize,
) -> Result<(), super::Flow> {
    match spec {
        Value::Nil => return Ok(()),
        Value::Sym(s) => {
            i.specbind(*s, seqv.clone())?;
            *nbind += 1;
            return Ok(());
        }
        Value::Cons(_) => {}
        other => return Err(i.wrong_type_mut("listp", other)),
    }
    let elems = want_list(i, spec)?;
    let amp_rest = i.intern("&rest");
    let amp_opt = i.intern("&optional");
    let (mut idx, mut pos) = (0usize, 0usize);
    while pos < elems.len() {
        let el = elems[pos].clone();
        if i.sym_id(&el) == Some(amp_rest) {
            let Some(name) = elems.get(pos + 1) else {
                return Err(i.error("seq-let: &rest without variable"));
            };
            let v = f_seq_drop(i, vec![seqv.clone(), Value::Int(idx as i128)])?;
            return seq_let_bind_one(i, name, &v, nbind);
        }
        if i.sym_id(&el) == Some(amp_opt) {
            pos += 1;
            continue;
        }
        let v = seq_elt_safe(i, seqv, idx);
        seq_let_bind_one(i, &el, &v, nbind)?;
        idx += 1;
        pos += 1;
    }
    Ok(())
}

/// `(seq-let ARGS SEQ BODY...)': evaluate SEQ, destructure-bind ARGS
/// against its elements, then run BODY (dynamic binding like `let').
fn f_seq_let_raw(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let all = arg(&args, 0);
    let forms = match all.list_to_vec() {
        Ok(v) => v,
        Err(_) => return Err(i.wrong_type_mut("listp", &all)),
    };
    if forms.len() < 2 {
        let s = Value::Sym(i.intern("seq-let"));
        return Err(i.wrong_number_of_args(&s, forms.len() as i128));
    }
    let seqv = i.eval(&forms[1])?;
    let mut nbind = 0usize;
    let r = seq_let_bind(i, &forms[0], &seqv, &mut nbind).and_then(|()| i.eval_body(&forms[2..]));
    let _ = i.unbind(nbind);
    r
}

/// Convert sequence to Vec<Value> of its elements.
pub(crate) fn seq_to_vec(i: &mut Interp, v: &Value) -> Result<Vec<Value>, super::Flow> {
    match v {
        Value::Nil => Ok(Vec::new()),
        Value::Cons(_) => want_list(i, v),
        Value::Str(s) => Ok(s
            .borrow()
            .chars()
            .map(|c| Value::Int(crate::lisp::value::lisp_char_code(c)))
            .collect()),
        Value::Vec(vec) => Ok(vec.borrow().clone()),
        other => Err(i.wrong_type_mut("sequencep", other)),
    }
}

/// Rebuild a sequence of the same type from elements.
fn seq_from_like(_i: &mut Interp, like: &Value, items: Vec<Value>) -> Value {
    match like {
        Value::Str(_) => {
            let mut s = String::new();
            for v in items {
                if let Value::Int(n) = v {
                    if let Some(c) = char::from_u32(n as u32) {
                        s.push(c);
                    }
                }
            }
            Value::string(s)
        }
        Value::Vec(_) => Value::Vec(std::rc::Rc::new(std::cell::RefCell::new(items))),
        _ => Value::list(items),
    }
    .clone()
}

fn f_elt(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let n = want_int(i, &args[1])?;
    match &args[0] {
        Value::Nil => {
            if n == 0 {
                Ok(Value::Nil)
            } else {
                Ok(Value::Nil)
            }
        }
        Value::Cons(_) => {
            // Emacs: elt on a list is car(nthcdr(N, list)) — negative
            // clamps to 0; an improper tail is a wrong-type error.
            let tail = super::listfn::nthcdr_strict(i, &args[0], n.max(0) as usize)?;
            match tail {
                Value::Cons(c) => Ok(c.borrow().car.clone()),
                Value::Nil => Ok(Value::Nil),
                ref other => Err(i.wrong_type_mut("listp", other)),
            }
        }
        Value::Str(s) => {
            let chars: Vec<char> = s.borrow().chars().collect();
            if n < 0 || n as usize >= chars.len() {
                return Err(i.signal_data(
                    sym::ARGS_OUT_OF_RANGE,
                    vec![args[0].clone(), args[1].clone()],
                ));
            }
            Ok(Value::Int(crate::lisp::value::lisp_char_code(
                chars[n as usize],
            )))
        }
        Value::Vec(v) => {
            let items = v.borrow();
            if n < 0 || n as usize >= items.len() {
                return Err(i.signal_data(
                    sym::ARGS_OUT_OF_RANGE,
                    vec![args[0].clone(), args[1].clone()],
                ));
            }
            Ok(items[n as usize].clone())
        }
        Value::Record(_) if super::misc::is_bool_vector(i, &args[0]) => {
            let bits = super::misc::bool_vec_of(i, &args[0])?;
            if n < 0 || n as usize >= bits.len() {
                return Err(i.signal_data(
                    sym::ARGS_OUT_OF_RANGE,
                    vec![args[0].clone(), args[1].clone()],
                ));
            }
            Ok(Value::from_bool(bits[n as usize]))
        }
        Value::Record(_) if super::misc::is_char_table(i, &args[0]) => {
            // GNU `char_table_ref': content slot → defalt → parent
            // chain; any valid character index is readable.
            if n < 0 || n > 0x3FFFFF {
                return Err(i.signal_data(
                    sym::ARGS_OUT_OF_RANGE,
                    vec![args[0].clone(), args[1].clone()],
                ));
            }
            Ok(super::misc::char_table_ref(i, &args[0], n as usize))
        }
        other => Err(i.wrong_type_mut("sequencep", other)),
    }
}

fn f_aref(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    // aref works on arrays including plain records (unlike elt, which
    // rejects them — matching GNU).  Lists/nil are not arrays.
    match &args[0] {
        Value::Str(_) | Value::Vec(_) => {}
        Value::Record(r) => {
            if !super::misc::is_bool_vector(i, &args[0]) && !super::misc::is_char_table(i, &args[0])
            {
                let n = want_int(i, &args[1])?;
                let items = r.borrow();
                if n < 0 || n as usize >= items.len() {
                    return Err(i.signal_data(
                        sym::ARGS_OUT_OF_RANGE,
                        vec![args[0].clone(), args[1].clone()],
                    ));
                }
                return Ok(items[n as usize].clone());
            }
        }
        other => return Err(i.wrong_type_mut("arrayp", other)),
    }
    f_elt(i, args)
}

fn f_aset(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let n = want_int(i, &args[1])?;
    match &args[0] {
        Value::Vec(v) => {
            let mut items = v.borrow_mut();
            if n < 0 || n as usize >= items.len() {
                return Err(i.signal_data(
                    sym::ARGS_OUT_OF_RANGE,
                    vec![args[0].clone(), args[1].clone()],
                ));
            }
            items[n as usize] = args[2].clone();
            Ok(args[2].clone())
        }
        Value::Str(s) => {
            let nch = match &args[2] {
                Value::Int(x) => *x,
                _ => return Err(i.wrong_type_mut("characterp", &args[2])),
            };
            let c = crate::lisp::value::lisp_char(nch as u32)
                .ok_or_else(|| i.wrong_type_mut("characterp", &args[2]))?;
            let mut chars: Vec<char> = s.borrow().chars().collect();
            if n < 0 || n as usize >= chars.len() {
                return Err(i.signal_data(
                    sym::ARGS_OUT_OF_RANGE,
                    vec![args[0].clone(), args[1].clone()],
                ));
            }
            chars[n as usize] = c;
            *s.borrow_mut() = chars.into_iter().collect();
            Ok(args[2].clone())
        }
        Value::Record(r) if super::misc::is_bool_vector(i, &args[0]) => {
            let rr = r.borrow();
            let Value::Vec(bits) = rr[1].clone() else {
                return Err(i.wrong_type_mut("bool-vector-p", &args[0]));
            };
            drop(rr);
            let mut bits = bits.borrow_mut();
            if n < 0 || n as usize >= bits.len() {
                return Err(i.signal_data(
                    sym::ARGS_OUT_OF_RANGE,
                    vec![args[0].clone(), args[1].clone()],
                ));
            }
            bits[n as usize] = Value::Int(if args[2].is_nil() { 0 } else { 1 });
            Ok(args[2].clone())
        }
        Value::Record(_) if super::misc::is_char_table(i, &args[0]) => {
            // Char-table: (aset CT CHAR VALUE) → `char_table_set'.
            if n < 0 || n > super::misc::CT_MAX_CHAR as i128 {
                return Err(i.signal_data(
                    sym::ARGS_OUT_OF_RANGE,
                    vec![args[0].clone(), args[1].clone()],
                ));
            }
            super::misc::ct_set(i, &args[0], n as u32, args[2].clone());
            Ok(args[2].clone())
        }
        Value::Record(r) => {
            // Ordinary records: GNU allows writing any slot (even the
            // tag at index 0).
            let mut items = r.borrow_mut();
            if n < 0 || n as usize >= items.len() {
                return Err(i.signal_data(
                    sym::ARGS_OUT_OF_RANGE,
                    vec![args[0].clone(), args[1].clone()],
                ));
            }
            items[n as usize] = args[2].clone();
            Ok(args[2].clone())
        }
        other => Err(i.wrong_type_mut("arrayp", other)),
    }
}

fn f_copy_sequence(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    match &args[0] {
        Value::Cons(_) => {
            let items = want_list(i, &args[0])?;
            Ok(Value::list(items))
        }
        Value::Str(s) => {
            // A real copy: new identity, own copy of prop intervals.
            let ns = std::rc::Rc::new(std::cell::RefCell::new(s.borrow().clone()));
            if i.has_str_props(s) {
                let ivs = i.str_props(s).to_vec();
                i.set_str_props(&ns, ivs);
            }
            if i.is_unibyte_str(s) {
                i.mark_unibyte(&ns);
            }
            if i.is_multibyte_str(s) {
                i.mark_multibyte(&ns);
            }
            Ok(Value::Str(ns))
        }
        Value::Nil | Value::Vec(_) | Value::Hash(_) => Ok(args[0].clone()),
        // Char-tables copy through `copy_char_table' (deep trie copy,
        // shared slot objects); other Records shallow-copy the vec.
        Value::Record(_) if super::misc::is_char_table(i, &args[0]) => {
            Ok(super::misc::ct_copy(i, &args[0]))
        }
        Value::Record(r) => Ok(Value::Record(std::rc::Rc::new(std::cell::RefCell::new(
            r.borrow().clone(),
        )))),
        other => Err(i.wrong_type_mut("sequencep", other)),
    }
}

fn map_seq(
    i: &mut Interp,
    _fun: &Value,
    seq: &Value,
    mut f: impl FnMut(&mut Interp, &Value) -> Result<(), super::Flow>,
) -> Result<(), super::Flow> {
    match seq {
        Value::Nil => Ok(()),
        Value::Cons(_) => {
            let items = want_list(i, seq)?;
            for v in items {
                f(i, &v)?;
            }
            Ok(())
        }
        Value::Str(s) => {
            let chars: Vec<char> = s.borrow().chars().collect();
            for c in chars {
                f(i, &Value::Int(crate::lisp::value::lisp_char_code(c)))?;
            }
            Ok(())
        }
        Value::Vec(v) => {
            let items = v.borrow().clone();
            for x in items {
                f(i, &x)?;
            }
            Ok(())
        }
        other => Err(i.wrong_type_mut("sequencep", other)),
    }
}

fn f_mapcar(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let fun = args[0].clone();
    let mut out = Vec::new();
    map_seq(i, &fun, &args[1], |i, v| {
        let r = i.apply(&fun, vec![v.clone()])?;
        out.push(r);
        Ok(())
    })?;
    Ok(Value::list(out))
}

fn f_mapc(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let fun = args[0].clone();
    map_seq(i, &fun, &args[1], |i, v| {
        i.apply(&fun, vec![v.clone()])?;
        Ok(())
    })?;
    Ok(args[1].clone())
}

fn f_mapcan(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    // (mapcan FUNCTION SEQUENCE &rest SEQUENCES) — apply FUNCTION to
    // successive element tuples, nconc the resulting lists.
    let fun = args[0].clone();
    let seqs: Vec<Vec<Value>> = args[1..]
        .iter()
        .map(|s| seq_to_vec(i, s))
        .collect::<Result<_, _>>()?;
    let n = seqs.iter().map(|s| s.len()).min().unwrap_or(0);
    let mut results = Vec::with_capacity(n);
    for k in 0..n {
        let argv: Vec<Value> = seqs.iter().map(|s| s[k].clone()).collect();
        results.push(i.apply(&fun, argv)?);
    }
    // nconc semantics: every result but the last must be a list;
    // a non-list final result becomes the dotted tail.
    let mut out = Vec::new();
    let last_idx = results.len().saturating_sub(1);
    for (idx, r) in results.iter().enumerate() {
        match r {
            Value::Nil => {}
            Value::Cons(_) => out.extend(want_list(i, r)?),
            other if idx == last_idx => {
                let mut res = other.clone();
                for v in out.into_iter().rev() {
                    res = Value::cons(v, res);
                }
                return Ok(res);
            }
            other => return Err(i.wrong_type_mut("listp", other)),
        }
    }
    Ok(Value::list(out))
}

fn f_mapconcat(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let fun = args[0].clone();
    let mut parts = Vec::new();
    map_seq(i, &fun, &args[1], |i, v| {
        let r = i.apply(&fun, vec![v.clone()])?;
        parts.push(r);
        Ok(())
    })?;
    let sep = args
        .get(2)
        .map(|v| want_string(i, v))
        .transpose()?
        .unwrap_or_default();
    // GNU's mapconcat concatenates the mapped strings directly, so
    // text properties of the parts are carried into the result.
    let mut out = String::new();
    let mut ivs: Vec<(usize, usize, Vec<Value>)> = Vec::new();
    let mut saw_props = false;
    for (k, p) in parts.iter().enumerate() {
        if k > 0 {
            out.push_str(&sep);
        }
        let off = out.chars().count();
        if let Value::Str(s) = p {
            if i.has_str_props(s) {
                saw_props = true;
                for (s0, e0, pl) in i.str_props(s) {
                    ivs.push((
                        s0 + off,
                        e0 + off,
                        crate::buffer::primitives::plist_pairs_rev(pl),
                    ));
                }
            }
            out.push_str(&s.borrow());
        } else {
            out.push_str(&i.princ_to_string(p));
        }
    }
    let ns = std::rc::Rc::new(std::cell::RefCell::new(out));
    if saw_props {
        i.set_str_props(&ns, ivs);
    }
    Ok(Value::Str(ns))
}

fn f_maphash(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let fun = args[0].clone();
    match &args[1] {
        Value::Hash(h) => {
            let pairs: Vec<(Value, Value)> = {
                let hh = h.borrow();
                hh.keys
                    .iter()
                    .map(|(hk, k)| {
                        let val = hh.map.get(hk).cloned().unwrap_or(Value::Nil);
                        (k.clone(), val)
                    })
                    .collect()
            };
            for (k, v) in pairs {
                i.apply(&fun, vec![k, v])?;
            }
            Ok(Value::Nil)
        }
        other => Err(i.wrong_type_mut("hash-table-p", other)),
    }
}

fn f_sort(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    // GNU sort.el: (sort SEQ [PRED] &key KEY LESSP REVERSE IN-PLACE).
    // Old-style call has PRED as arg 2; keyword call starts with a
    // keyword symbol.  The two forms cannot be mixed.
    let rest = &args[1..];
    let kw_style =
        matches!(rest.first(), Some(Value::Sym(s)) if i.obarray.name(*s).starts_with(':'));
    let (keyf, lessp, reverse, in_place) = if kw_style {
        let mut keyf = Value::Nil;
        let mut lessp = Value::Nil;
        let mut reverse = false;
        let mut in_place = false;
        let mut it = rest.iter();
        while let Some(k) = it.next() {
            let kn = match k {
                Value::Sym(s) => i.obarray.name(*s).to_string(),
                _ => return Err(i.error("Invalid argument list")),
            };
            let v = it
                .next()
                .cloned()
                .ok_or_else(|| i.error("Invalid argument list"))?;
            match kn.as_str() {
                ":key" => keyf = v,
                ":lessp" => lessp = v,
                ":reverse" => reverse = v.truthy(),
                ":in-place" => in_place = v.truthy(),
                _ => return Err(i.error("Invalid argument list")),
            }
        }
        (keyf, lessp, reverse, in_place)
    } else {
        if rest.len() > 1 {
            return Err(i.error("Invalid argument list"));
        }
        // Old-style call: vectors sort in place (historic behavior);
        // keyword calls default :in-place to nil (sorted copy).
        (
            Value::Nil,
            rest.first().cloned().unwrap_or(Value::Nil),
            false,
            true,
        )
    };
    // Default comparator is `value<' (GNU sorts any comparable
    // values — numbers, strings, symbols — without a predicate).
    let lessp = match lessp {
        Value::Nil => Value::Sym(i.intern("value<")),
        f => f,
    };
    let spec = SortSpec {
        keyf,
        lessp,
        reverse,
    };
    match &args[0] {
        Value::Cons(_) => {
            let mut items = want_list(i, &args[0])?;
            merge_sort(i, &mut items, &spec)?;
            // GNU sort_list writes sorted values back into the
            // argument's own conses — the result is `eq' to the input.
            let mut tail = args[0].clone();
            for v in items {
                if let Value::Cons(c) = tail {
                    c.borrow_mut().car = v;
                    tail = c.borrow().cdr.clone();
                }
            }
            Ok(args[0].clone())
        }
        Value::Vec(v) => {
            let mut items = v.borrow().clone();
            merge_sort(i, &mut items, &spec)?;
            if in_place {
                *v.borrow_mut() = items;
                Ok(args[0].clone())
            } else {
                Ok(Value::Vec(Rc::new(RefCell::new(items))))
            }
        }
        Value::Nil => Ok(Value::Nil),
        other => Err(i.wrong_type_mut("list-or-vector-p", other)),
    }
}

struct SortSpec {
    keyf: Value,
    lessp: Value,
    reverse: bool,
}

impl SortSpec {
    /// Emacs `sort` uses (pred a b) = "a < b"; :key applies to each
    /// element, :reverse flips the comparison (keeping stability).
    fn less(&self, i: &mut Interp, a: &Value, b: &Value) -> Result<bool, super::Flow> {
        let ka = if self.keyf.truthy() {
            i.apply(&self.keyf, vec![a.clone()])?
        } else {
            a.clone()
        };
        let kb = if self.keyf.truthy() {
            i.apply(&self.keyf, vec![b.clone()])?
        } else {
            b.clone()
        };
        let r = if self.reverse {
            i.apply(&self.lessp, vec![kb, ka])?
        } else {
            i.apply(&self.lessp, vec![ka, kb])?
        };
        Ok(r.truthy())
    }
}

/// Stable merge sort (same order guarantees as Emacs `sort`).
fn merge_sort(i: &mut Interp, items: &mut Vec<Value>, spec: &SortSpec) -> Result<(), super::Flow> {
    if items.len() < 2 {
        return Ok(());
    }
    let mid = items.len() / 2;
    let mut left = items[..mid].to_vec();
    let mut right = items[mid..].to_vec();
    merge_sort(i, &mut left, spec)?;
    merge_sort(i, &mut right, spec)?;
    let (mut a, mut b, mut k) = (0usize, 0usize, 0usize);
    while a < left.len() && b < right.len() {
        // pred(right[b], left[a])? Emacs `sort` uses (pred a b) = "a < b"
        let r = spec.less(i, &right[b], &left[a])?;
        if r {
            items[k] = right[b].clone();
            b += 1;
        } else {
            items[k] = left[a].clone();
            a += 1;
        }
        k += 1;
    }
    while a < left.len() {
        items[k] = left[a].clone();
        a += 1;
        k += 1;
    }
    while b < right.len() {
        items[k] = right[b].clone();
        b += 1;
        k += 1;
    }
    Ok(())
}

fn f_string_to_sequence(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let items = seq_to_vec(i, &args[0])?;
    let ty = arg(&args, 1);
    match i.sym_id(&ty) {
        Some(s) if s == i.intern("list") || s == 0 => Ok(Value::list(items)),
        Some(s) if s == i.intern("vector") => {
            Ok(Value::Vec(std::rc::Rc::new(std::cell::RefCell::new(items))))
        }
        Some(s) if s == i.intern("string") => Ok(args[0].clone()),
        _ => Ok(Value::list(items)),
    }
}

fn f_seq(_i: &mut Interp, args: Vec<Value>) -> EvalResult {
    Ok(args.into_iter().next().unwrap_or(Value::Nil))
}

fn f_append_to_list(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let mut items = want_list(i, &args[0])?;
    items.push(args[1].clone());
    Ok(Value::list(items))
}

fn f_fillarray(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    match &args[0] {
        Value::Vec(v) => {
            for x in v.borrow_mut().iter_mut() {
                *x = args[1].clone();
            }
            Ok(args[0].clone())
        }
        Value::Record(r) if super::misc::is_char_table(i, &args[0]) => {
            // GNU `Ffillarray' on a char-table: the 64 top-level
            // contents slots take ITEM (the ASCII slot is untouched),
            // the defalt and every extra slot become ITEM, and the
            // parent is preserved.
            let item = args[1].clone();
            if let Some(v) = super::misc::char_table_vec(&args[0]) {
                let mut vv = v.borrow_mut();
                for slot in vv.iter_mut().skip(1) {
                    *slot = item.clone();
                }
            }
            i.set_char_table_defalt(&args[0], item.clone());
            for slot in r.borrow_mut().iter_mut().skip(3) {
                *slot = item.clone();
            }
            Ok(args[0].clone())
        }
        Value::Record(r) if super::misc::is_bool_vector(i, &args[0]) => {
            // GNU fills the bit vector with (not (null ITEM)).
            let bit = if args[1].is_nil() { 0 } else { 1 };
            if let Some(Value::Vec(b)) = r.borrow().get(1) {
                for x in b.borrow_mut().iter_mut() {
                    *x = Value::Int(bit);
                }
            }
            Ok(args[0].clone())
        }
        Value::Str(s) => {
            // GNU replaces each character with ITEM.  A unibyte
            // (all-ASCII) string filled with a non-ASCII char takes
            // ITEM's low byte; shrinking a multibyte char signals.
            let item = match &args[1] {
                Value::Int(n) if (0..=0x3fffff).contains(n) => *n as u32,
                _ => return Err(i.wrong_type_mut("characterp", &args[1])),
            };
            let mut st = s.borrow_mut();
            let n = st.chars().count();
            let all_ascii = st.chars().all(|c| (c as u32) < 0x80);
            let new: String = if item < 0x80 {
                if !all_ascii {
                    return Err(i.error("Attempt to change byte length of a string"));
                }
                std::iter::repeat_n(item as u8 as char, n).collect()
            } else if all_ascii {
                let low = char::from_u32(item & 0xff).unwrap_or('\u{fffd}');
                std::iter::repeat_n(low, n).collect()
            } else {
                let ilen = char::from_u32(item).map_or(4, |c| c.len_utf8());
                if !st.chars().all(|c| c.len_utf8() == ilen) {
                    return Err(i.error("Attempt to change byte length of a string"));
                }
                let c = char::from_u32(item).unwrap_or('\u{fffd}');
                std::iter::repeat_n(c, n).collect()
            };
            *st = new;
            drop(st);
            Ok(args[0].clone())
        }
        other => Err(i.wrong_type_mut("arrayp", other)),
    }
}

fn f_make_vector(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let len = want_int(i, &args[0])?;
    if len < 0 {
        let s = i.intern("args-out-of-range");
        return Err(i.signal_data(s, vec![args[0].clone()]));
    }
    let n = len as usize;
    Ok(Value::Vec(std::rc::Rc::new(std::cell::RefCell::new(
        vec![args[1].clone(); n],
    ))))
}

fn f_vector(_i: &mut Interp, args: Vec<Value>) -> EvalResult {
    Ok(Value::Vec(std::rc::Rc::new(std::cell::RefCell::new(args))))
}

fn f_bool_vector(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    Ok(super::misc::make_bool_vector(
        i,
        args.iter().map(|v| !v.is_nil()).collect(),
    ))
}

fn f_purecopy(_i: &mut Interp, args: Vec<Value>) -> EvalResult {
    Ok(args[0].clone())
}

fn f_nreverse_seq(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    match &args[0] {
        Value::Cons(_) => {
            // GNU signals circular-list before mutating; detect the
            // cycle with a tortoise/hare walk first.
            let mut tortoise = args[0].clone();
            let mut hare = args[0].clone();
            let mut guard = 0usize;
            loop {
                guard += 1;
                if guard > 500_000 {
                    return Err(err_circular(i));
                }
                match &hare {
                    Value::Cons(c) => {
                        let n1 = c.borrow().cdr.clone();
                        match &n1 {
                            Value::Cons(c2) => {
                                let n2 = c2.borrow().cdr.clone();
                                hare = n2;
                            }
                            Value::Nil => break,
                            _ => break,
                        }
                        if let Value::Cons(tc) = &tortoise {
                            let tnext = tc.borrow().cdr.clone();
                            tortoise = tnext;
                        }
                        if eq_values(&hare, &tortoise) {
                            return Err(err_circular(i));
                        }
                    }
                    _ => break,
                }
            }
            // delegate to list version semantics
            let mut prev = Value::Nil;
            let mut cur = args[0].clone();
            loop {
                match &cur {
                    Value::Cons(c) => {
                        let next = c.borrow().cdr.clone();
                        c.borrow_mut().cdr = prev;
                        prev = cur;
                        cur = next;
                    }
                    _ => break,
                }
            }
            // Emacs requires a proper list; a dotted tail is an error.
            if !cur.is_nil() {
                return Err(i.wrong_type_mut("listp", &args[0]));
            }
            Ok(prev)
        }
        Value::Vec(v) => {
            v.borrow_mut().reverse();
            Ok(args[0].clone())
        }
        Value::Str(s) => {
            let reversed: String = s.borrow().chars().rev().collect();
            *s.borrow_mut() = reversed;
            Ok(args[0].clone())
        }
        Value::Nil => Ok(Value::Nil),
        other => Err(i.wrong_type_mut("arrayp", other)),
    }
}

fn f_clear_vector(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    match &args[0] {
        Value::Vec(v) => {
            for x in v.borrow_mut().iter_mut() {
                *x = args[1].clone();
            }
            Ok(args[0].clone())
        }
        other => Err(i.wrong_type_mut("arrayp", other)),
    }
}

fn f_seq_concatenate(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let ty = i.sym_id(&args[0]).unwrap_or(0);
    let mut items = Vec::new();
    for s in &args[1..] {
        items.extend(seq_to_vec(i, s)?);
    }
    let list_id = i.intern("list");
    let vec_id = i.intern("vector");
    let str_id = i.intern("string");
    Ok(if ty == list_id {
        Value::list(items)
    } else if ty == vec_id {
        Value::Vec(std::rc::Rc::new(std::cell::RefCell::new(items)))
    } else if ty == str_id {
        let mut s = String::new();
        for v in items {
            if let Value::Int(n) = v {
                if let Some(c) = char::from_u32(n as u32) {
                    s.push(c);
                }
            }
        }
        Value::string(s)
    } else {
        let name = match &args[0] {
            Value::Sym(s) => i.obarray.name(*s).to_string(),
            other => format!("{:?}", other),
        };
        return Err(i.error(&format!("Not a sequence type name: {}", name)));
    })
}

fn f_seq_subseq(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let items = seq_to_vec(i, &args[0])?;
    let len = items.len() as i128;
    // GNU seq.el checks indices with number-or-marker-p; nil END = length.
    let num_index = |i: &mut Interp, v: &Value| -> Result<i128, Flow> {
        match v {
            Value::Int(n) => Ok(*n),
            Value::Float(f) => Ok((**f).trunc() as i128),
            other => Err(i.wrong_type_mut("number-or-marker-p", other)),
        }
    };
    let start0 = num_index(i, &args[1])?;
    let end0 = match args.get(2) {
        Some(v) if !v.is_nil() => num_index(i, v)?,
        _ => len,
    }
    .min(len);
    // GNU: negative indices count from the end; anything outside
    // [0,len] or a reversed range is a plain `error'.
    let start = if start0 < 0 { start0 + len } else { start0 };
    let end = if end0 < 0 { end0 + len } else { end0 };
    if start < 0 || end < 0 || start > len || end > len || start > end {
        return Err(i.error(&format!("Bad bounding indices: {}, {}", start0, end0)));
    }
    Ok(seq_from_like(
        i,
        &args[0],
        items[start as usize..end as usize].to_vec(),
    ))
}

fn f_seq_take(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let items = seq_to_vec(i, &args[0])?;
    let n = want_int(i, &args[1])?.max(0) as usize;
    Ok(seq_from_like(
        i,
        &args[0],
        items.into_iter().take(n).collect(),
    ))
}
fn f_seq_drop(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let items = seq_to_vec(i, &args[0])?;
    let n = want_int(i, &args[1])?.max(0) as usize;
    Ok(seq_from_like(
        i,
        &args[0],
        items.into_iter().skip(n).collect(),
    ))
}
fn f_seq_length(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    Ok(Value::Int(seq_to_vec(i, &args[0])?.len() as i128))
}
fn f_seq_do(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let fun = args[0].clone();
    map_seq(i, &fun, &args[1], |i, v| {
        i.apply(&fun, vec![v.clone()])?;
        Ok(())
    })?;
    // GNU returns the sequence itself.
    Ok(args[1].clone())
}
fn f_seq_map(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let fun = args[0].clone();
    let mut out = Vec::new();
    map_seq(i, &fun, &args[1], |i, v| {
        out.push(i.apply(&fun, vec![v.clone()])?);
        Ok(())
    })?;
    Ok(Value::list(out))
}
fn f_seq_filter(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let pred = args[0].clone();
    let mut out = Vec::new();
    map_seq(i, &pred, &args[1], |i, v| {
        if i.apply(&pred, vec![v.clone()])?.truthy() {
            out.push(v.clone());
        }
        Ok(())
    })?;
    // GNU seq.el always returns a list here, even for vector/string
    // input.
    Ok(Value::list(out))
}
fn f_seq_contains_p(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let items = seq_to_vec(i, &args[0])?;
    for x in items {
        let r = match args.get(2) {
            Some(f) if f.truthy() => i.apply(f, vec![x.clone(), args[1].clone()])?,
            _ => {
                if equal_values(i, &x, &args[1]) {
                    Value::t()
                } else {
                    Value::Nil
                }
            }
        };
        if r.truthy() {
            return Ok(Value::t());
        }
    }
    Ok(Value::Nil)
}
fn f_seq_position(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let items = seq_to_vec(i, &args[0])?;
    for (k, x) in items.iter().enumerate() {
        let eq = match args.get(2) {
            Some(f) if f.truthy() => i.apply(f, vec![x.clone(), args[1].clone()])?.truthy(),
            _ => equal_values(i, x, &args[1]),
        };
        if eq {
            return Ok(Value::Int(k as i128));
        }
    }
    Ok(Value::Nil)
}
fn f_seq_count(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let pred = args[0].clone();
    let mut n = 0;
    map_seq(i, &pred, &args[1], |i, v| {
        if i.apply(&pred, vec![v.clone()])?.truthy() {
            n += 1;
        }
        Ok(())
    })?;
    Ok(Value::Int(n))
}
fn f_seq_reverse(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let items = seq_to_vec(i, &args[0])?;
    Ok(seq_from_like(
        i,
        &args[0],
        items.into_iter().rev().collect(),
    ))
}
fn f_seq_some(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let pred = args[0].clone();
    let items = seq_to_vec(i, &args[1])?;
    for v in items {
        let r = i.apply(&pred, vec![v])?;
        if r.truthy() {
            return Ok(r);
        }
    }
    Ok(Value::Nil)
}
fn f_seq_every_p(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let pred = args[0].clone();
    let items = seq_to_vec(i, &args[1])?;
    for v in items {
        if i.apply(&pred, vec![v])?.is_nil() {
            return Ok(Value::Nil);
        }
    }
    Ok(Value::t())
}
fn f_seq_find(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let pred = args[0].clone();
    let items = seq_to_vec(i, &args[1])?;
    for v in items {
        if i.apply(&pred, vec![v.clone()])?.truthy() {
            return Ok(v);
        }
    }
    Ok(Value::Nil)
}
fn f_seq_remove(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let pred = args[0].clone();
    let mut out = Vec::new();
    map_seq(i, &pred, &args[1], |i, v| {
        if i.apply(&pred, vec![v.clone()])?.is_nil() {
            out.push(v.clone());
        }
        Ok(())
    })?;
    // GNU seq.el always returns a list here.
    Ok(Value::list(out))
}
fn f_seq_reduce(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let fun = args[0].clone();
    let items = seq_to_vec(i, &args[1])?;
    let mut acc = args[2].clone();
    for v in items {
        acc = i.apply(&fun, vec![acc, v])?;
    }
    Ok(acc)
}
fn f_seq_take_while(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let pred = args[0].clone();
    let items = seq_to_vec(i, &args[1])?;
    let mut out = Vec::new();
    for v in items {
        if i.apply(&pred, vec![v.clone()])?.is_nil() {
            break;
        }
        out.push(v);
    }
    Ok(seq_from_like(i, &args[1], out))
}
fn f_seq_drop_while(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let pred = args[0].clone();
    let items = seq_to_vec(i, &args[1])?;
    let mut k = 0;
    while k < items.len() {
        if i.apply(&pred, vec![items[k].clone()])?.is_nil() {
            break;
        }
        k += 1;
    }
    Ok(seq_from_like(i, &args[1], items[k..].to_vec()))
}
fn f_seq_into(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let items = seq_to_vec(i, &args[0])?;
    let list_id = i.intern("list");
    let vec_id = i.intern("vector");
    let str_id = i.intern("string");
    let ty = i.sym_id(&args[1]).unwrap_or(0);
    Ok(if ty == list_id {
        Value::list(items)
    } else if ty == vec_id {
        Value::Vec(std::rc::Rc::new(std::cell::RefCell::new(items)))
    } else if ty == str_id {
        let mut s = String::new();
        for v in items {
            if let Value::Int(n) = v {
                if let Some(c) = char::from_u32(n as u32) {
                    s.push(c);
                }
            }
        }
        Value::string(s)
    } else {
        let name = match &args[1] {
            Value::Sym(s) => i.obarray.name(*s).to_string(),
            other => format!("{:?}", other),
        };
        return Err(i.error(&format!("Not a sequence type name: {}", name)));
    })
}
fn f_seq_into_sequence(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    match &args[0] {
        Value::Nil | Value::Cons(_) | Value::Str(_) | Value::Vec(_) => Ok(args[0].clone()),
        other => {
            let msg = i.prin1_to_string(other);
            Err(i.error(&format!("Cannot convert {} into a sequence", msg)))
        }
    }
}
fn f_seq_empty_p(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    Ok(Value::from_bool(seq_to_vec(i, &args[0])?.is_empty()))
}
fn f_seq_first(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let items = seq_to_vec(i, &args[0])?;
    Ok(items.into_iter().next().unwrap_or(Value::Nil))
}
fn f_seq_rest(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let items = seq_to_vec(i, &args[0])?;
    Ok(seq_from_like(
        i,
        &args[0],
        items.into_iter().skip(1).collect(),
    ))
}
fn f_seq_last(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let items = seq_to_vec(i, &args[0])?;
    Ok(items.into_iter().last().unwrap_or(Value::Nil))
}
fn f_seq_min(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    // GNU's seq-min applies `min' across the elements, so an empty
    // sequence fails with wrong-number-of-arguments and any
    // non-number element is a type error.
    let items = seq_to_vec(i, &args[0])?;
    let f = Value::Sym(i.intern("min"));
    i.apply(&f, items)
}
fn f_seq_max(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let items = seq_to_vec(i, &args[0])?;
    let f = Value::Sym(i.intern("max"));
    i.apply(&f, items)
}
fn f_seq_uniq(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let items = seq_to_vec(i, &args[0])?;
    let testfn = arg(&args, 1);
    let mut out: Vec<Value> = Vec::new();
    'outer: for v in items {
        for u in &out {
            let dup = if testfn.truthy() {
                i.apply(&testfn, vec![u.clone(), v.clone()])?.truthy()
            } else {
                equal_values(i, u, &v)
            };
            if dup {
                continue 'outer;
            }
        }
        out.push(v);
    }
    // GNU seq.el always returns a list here.
    Ok(Value::list(out))
}
fn f_make_char_table(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    // Char-table = #s(char-table SUBTYPE [65 slots] EXTRA...) — a
    // Record so `char-table-p'/`char-table-subtype' are exact.  The
    // contents vec is the GNU trie root: [0] is the ASCII cache slot,
    // [1..=64] the top-level 65536-char blocks, all initialized to
    // INIT (GNU `make_vector' fills every slot incl. the defalt).
    // GNU sizes the extras from the subtype's `char-table-extra-slots'
    // property (e.g. disp-table.el puts 18 on `display-table').
    let subtype = arg(&args, 0);
    let init = arg(&args, 1);
    let vec = Value::Vec(std::rc::Rc::new(std::cell::RefCell::new(vec![
        init.clone();
        65
    ])));
    let prop = i.intern("char-table-extra-slots");
    let n = match crate::lisp::builtins::data::f_get(i, vec![subtype.clone(), Value::Sym(prop)])? {
        Value::Int(n) if n > 0 => n as usize,
        _ => 0,
    };
    let mut rec = vec![Value::Sym(i.intern("char-table")), subtype, vec];
    rec.resize(3 + n, Value::Nil);
    let t = Value::Record(std::rc::Rc::new(std::cell::RefCell::new(rec)));
    if !init.is_nil() {
        i.set_char_table_defalt(&t, init);
    }
    Ok(t)
}
