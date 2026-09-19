//! Sequence subrs: elt, aref, aset, copy-sequence, mapcar, sort, etc.

use super::listfn::nthcdr_of;
use super::{arg, equal_values, want_int, want_list, want_string, S};
use crate::lisp::error::EvalResult;
use crate::lisp::obarray::sym;
use crate::lisp::value::{Subr, Value};
use crate::lisp::Interp;

pub(crate) static SUBRS: &[Subr] = &[
    S!("elt", 2, 2, f_elt, "Return element of SEQUENCE at index N."),
    S!("aref", 2, 2, f_aref, "Return element of ARRAY at index N."),
    S!("aset", 3, 3, f_aset, "Set element of ARRAY at index N."),
    S!("copy-sequence", 1, 1, f_copy_sequence, "Copy a sequence."),
    S!("copy-seq", 1, 1, f_copy_sequence, "Copy a sequence."),
    S!("mapcar", 2, 3, f_mapcar, "Map FUNCTION over SEQUENCE, collect results."),
    S!("mapc", 2, 3, f_mapc, "Map FUNCTION over SEQUENCE for side effects."),
    S!("mapcan", 2, 3, f_mapcan, "Mapcar + nconc."),
    S!("mapconcat", 3, 4, f_mapconcat, "Map FUNCTION over SEQUENCE, join results."),
    S!("maphash", 2, 2, f_maphash, "Map FUNCTION over hash table entries."),
    S!("sort", 2, 2, f_sort, "Sort SEQ destructively by PREDICATE."),
    S!("string-to-sequence", 1, 2, f_string_to_sequence, "Convert string to list/vector."),
    S!("seq", many 0, f_seq, "Return SEQUENCE unchanged."),
    S!("sequence", many 0, f_seq, "Return SEQUENCE unchanged."),
    S!("append-to-list", 2, 2, f_append_to_list, "Append element to list (list + elt)."),
    S!("fillarray", 2, 2, f_fillarray, "Fill ARRAY with ITEM."),
    S!("make-vector", 2, 2, f_make_vector, "Make a vector of LENGTH with INIT."),
    S!("vector", many 0, f_vector, "Make a vector of the arguments."),
    S!("bool-vector", many 0, f_vector, "Make a vector (bool-vec approx)."),
    S!("purecopy", 1, 1, f_purecopy, "Return OBJECT unchanged."),
    S!("nreverse", 1, 1, f_nreverse_seq, "Reverse SEQUENCE destructively (seq version)."),
    S!("clear-vector", 2, 2, f_clear_vector, "Set all elements of VECTOR to nil."),
    S!("seq-concatenate", 3, 3, f_seq_concatenate, "Concatenate SEQS into TYPE."),
    S!("seq-subseq", 2, 3, f_seq_subseq, "Subsequence of SEQ."),
    S!("seq-take", 2, 2, f_seq_take, "First N elements of SEQ."),
    S!("seq-drop", 2, 2, f_seq_drop, "SEQ without first N elements."),
    S!("seq-elt", 2, 2, f_elt, "seq.el elt."),
    S!("seq-length", 1, 1, f_seq_length, "Length of SEQ."),
    S!("seq-do", 2, 2, f_seq_do, "Apply FUNCTION to each element of SEQ."),
    S!("seq-map", 2, 2, f_seq_map, "Map FUNCTION over SEQ, return list."),
    S!("seq-filter", 2, 2, f_seq_filter, "Elements of SEQ satisfying PRED."),
    S!("seq-contains-p", 2, 3, f_seq_contains_p, "Is ELT in SEQ?"),
    S!("seq-position", 2, 3, f_seq_position, "Index of ELT in SEQ."),
    S!("seq-count", 2, 2, f_seq_count, "Count elements satisfying PRED."),
    S!("seq-reverse", 1, 1, f_seq_reverse, "Reversed copy of SEQ."),
    S!("seq-some", 2, 2, f_seq_some, "First non-nil result of PRED on SEQ."),
    S!("seq-every-p", 2, 2, f_seq_every_p, "t if PRED holds for all of SEQ."),
    S!("seq-find", 2, 3, f_seq_find, "First element of SEQ satisfying PRED."),
    S!("seq-remove", 2, 2, f_seq_remove, "Remove elements satisfying PRED."),
    S!("seq-reduce", 3, 3, f_seq_reduce, "Reduce SEQ with FUNCTION and INIT."),
    S!("seq-take-while", 2, 2, f_seq_take_while, "Take while PRED holds."),
    S!("seq-drop-while", 2, 2, f_seq_drop_while, "Drop while PRED holds."),
    S!("seq-copy", 1, 1, f_copy_sequence, ""),
    S!("seq-into", 2, 2, f_seq_into, "Convert SEQ to TYPE."),
    S!("seq-empty-p", 1, 1, f_seq_empty_p, "t if SEQ is empty."),
    S!("seq-first", 1, 1, f_seq_first, "First element of SEQ."),
    S!("seq-rest", 1, 1, f_seq_rest, "SEQ minus first element."),
    S!("seq-last", 1, 1, f_seq_last, "Last element of SEQ."),
    S!("seq-min", 1, 1, f_seq_min, "Smallest element of SEQ."),
    S!("seq-max", 1, 1, f_seq_max, "Largest element of SEQ."),
    S!("seq-uniq", 1, 2, f_seq_uniq, "SEQ with duplicates removed."),
    S!("seq-let", raw, f_seq_let_raw, "seq-let is a macro in lisp/."),
    S!("char-table", many 0, f_vector, ""),
    S!("make-char-table", 1, 3, f_make_char_table, "Make a char-table (approx: vector)."),
];

fn f_seq_let_raw(i: &mut Interp, _args: Vec<Value>) -> EvalResult {
    Err(i.error("seq-let is defined as a Lisp macro"))
}

/// Convert sequence to Vec<Value> of its elements.
pub(crate) fn seq_to_vec(i: &mut Interp, v: &Value) -> Result<Vec<Value>, super::Flow> {
    match v {
        Value::Nil => Ok(Vec::new()),
        Value::Cons(_) => want_list(i, v),
        Value::Str(s) => Ok(s.borrow().chars().map(|c| Value::Int(c as i128)).collect()),
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
            // clamps to 0, out-of-range gives nil (never an error).
            let tail = nthcdr_of(&args[0], n.max(0) as usize);
            match tail {
                Value::Cons(c) => Ok(c.borrow().car.clone()),
                _ => Ok(Value::Nil),
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
            Ok(Value::Int(chars[n as usize] as i128))
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
        other => Err(i.wrong_type_mut("sequencep", other)),
    }
}

fn f_aref(i: &mut Interp, args: Vec<Value>) -> EvalResult {
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
            let c = char::from_u32(nch as u32)
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
        other => Err(i.wrong_type_mut("arrayp", other)),
    }
}

fn f_copy_sequence(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    match &args[0] {
        Value::Cons(_) => {
            let items = want_list(i, &args[0])?;
            Ok(Value::list(items))
        }
        Value::Str(_) | Value::Nil | Value::Vec(_) | Value::Hash(_) => Ok(args[0].clone()),
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
                f(i, &Value::Int(c as i128))?;
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
    let mut out = Vec::new();
    for k in 0..n {
        let argv: Vec<Value> = seqs.iter().map(|s| s[k].clone()).collect();
        let r = i.apply(&fun, argv)?;
        if let Value::Cons(_) = &r {
            out.extend(want_list(i, &r)?);
        } else if !r.is_nil() {
            out.push(r);
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
    let sep = args.get(2).map(|v| want_string(i, v)).transpose()?.unwrap_or_default();
    let mut out = String::new();
    for (k, p) in parts.iter().enumerate() {
        if k > 0 {
            out.push_str(&sep);
        }
        match p {
            Value::Str(s) => out.push_str(&s.borrow()),
            other => out.push_str(&i.princ_to_string(other)),
        }
    }
    Ok(Value::string(out))
}

fn f_maphash(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let fun = args[0].clone();
    match &args[1] {
        Value::Hash(h) => {
            let pairs: Vec<(Value, Value)> = {
                let hh = h.borrow();
                hh.keys
                    .values()
                    .map(|k| {
                        let hk = super::hashfn::hash_key_for(i, k, hh.test);
                        let val = hh.map.get(&hk).cloned().unwrap_or(Value::Nil);
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
    let pred = args[1].clone();
    match &args[0] {
        Value::Cons(_) => {
            let mut items = want_list(i, &args[0])?;
            merge_sort(i, &mut items, &pred)?;
            Ok(Value::list(items))
        }
        Value::Vec(v) => {
            let mut items = v.borrow().clone();
            merge_sort(i, &mut items, &pred)?;
            *v.borrow_mut() = items;
            Ok(args[0].clone())
        }
        Value::Nil => Ok(Value::Nil),
        other => Err(i.wrong_type_mut("listp", other)),
    }
}

/// Stable merge sort (same order guarantees as Emacs `sort`).
fn merge_sort(i: &mut Interp, items: &mut Vec<Value>, pred: &Value) -> Result<(), super::Flow> {
    if items.len() < 2 {
        return Ok(());
    }
    let mid = items.len() / 2;
    let mut left = items[..mid].to_vec();
    let mut right = items[mid..].to_vec();
    merge_sort(i, &mut left, pred)?;
    merge_sort(i, &mut right, pred)?;
    let (mut a, mut b, mut k) = (0usize, 0usize, 0usize);
    while a < left.len() && b < right.len() {
        // pred(right[b], left[a])? Emacs `sort` uses (pred a b) = "a < b"
        let r = i.apply(pred, vec![right[b].clone(), left[a].clone()])?;
        if r.truthy() {
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
        Some(s) if s == i.intern("vector") => Ok(Value::Vec(std::rc::Rc::new(
            std::cell::RefCell::new(items),
        ))),
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

fn f_purecopy(_i: &mut Interp, args: Vec<Value>) -> EvalResult {
    Ok(args[0].clone())
}

fn f_nreverse_seq(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    match &args[0] {
        Value::Cons(_) => {
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
        other => Err(i.wrong_type_mut("sequencep", other)),
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
    if let Value::Cons(_) = &args[1] {
        let seqs = want_list(i, &args[1])?;
        for s in seqs {
            items.extend(seq_to_vec(i, &s)?);
        }
    } else {
        items.extend(seq_to_vec(i, &args[1])?);
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
        Value::list(items)
    })
}

fn f_seq_subseq(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let items = seq_to_vec(i, &args[0])?;
    let len = items.len() as i128;
    let start = want_int(i, &args[1])?.max(0).min(len) as usize;
    let end = args.get(2).map(|v| want_int(i, v)).transpose()?.unwrap_or(len);
    let e = if end < 0 { (len + end).max(0) } else { end.min(len) } as usize;
    if start > e {
        return Err(i.signal_data(
            sym::ARGS_OUT_OF_RANGE,
            vec![args[0].clone()],
        ));
    }
    Ok(seq_from_like(i, &args[0], items[start..e].to_vec()))
}

fn f_seq_take(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let items = seq_to_vec(i, &args[0])?;
    let n = want_int(i, &args[1])?.max(0) as usize;
    Ok(seq_from_like(i, &args[0], items.into_iter().take(n).collect()))
}
fn f_seq_drop(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let items = seq_to_vec(i, &args[0])?;
    let n = want_int(i, &args[1])?.max(0) as usize;
    Ok(seq_from_like(i, &args[0], items.into_iter().skip(n).collect()))
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
    Ok(Value::Nil)
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
    Ok(seq_from_like(i, &args[1], out))
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
    Ok(seq_from_like(i, &args[1], out))
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
        return Err(i.wrong_type_mut("symbolp", &args[1]));
    })
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
    let items = seq_to_vec(i, &args[0])?;
    let mut best: Option<f64> = None;
    let mut bestv = Value::Nil;
    for v in items {
        if let Value::Int(n) = v {
            let f = n as f64;
            if best.map(|b| f < b).unwrap_or(true) {
                best = Some(f);
                bestv = v.clone();
            }
        }
    }
    if best.is_none() {
        return Err(i.wrong_type_mut("sequencep", &args[0]));
    }
    Ok(bestv)
}
fn f_seq_max(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let items = seq_to_vec(i, &args[0])?;
    let mut best: Option<f64> = None;
    let mut bestv = Value::Nil;
    for v in items {
        if let Value::Int(n) = v {
            let f = n as f64;
            if best.map(|b| f > b).unwrap_or(true) {
                best = Some(f);
                bestv = v.clone();
            }
        }
    }
    if best.is_none() {
        return Err(i.wrong_type_mut("sequencep", &args[0]));
    }
    Ok(bestv)
}
fn f_seq_uniq(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let items = seq_to_vec(i, &args[0])?;
    let mut out: Vec<Value> = Vec::new();
    'outer: for v in items {
        for u in &out {
            if equal_values(i, u, &v) {
                continue 'outer;
            }
        }
        out.push(v);
    }
    Ok(seq_from_like(i, &args[0], out))
}
fn f_make_char_table(_i: &mut Interp, args: Vec<Value>) -> EvalResult {
    // Approximate char-tables with a 256-element vector.
    let init = arg(&args, 1);
    Ok(Value::Vec(std::rc::Rc::new(std::cell::RefCell::new(
        vec![init; 256],
    ))))
}
