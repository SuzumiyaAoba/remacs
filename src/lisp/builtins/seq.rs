//! Sequence subrs: elt, aref, aset, copy-sequence, mapcar, sort, etc.

use super::{S, arg, eq_values, equal_values, want_fixnum, want_int, want_list};
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
        // GNU seq--make-pcase-patterns recurses into any seq spec —
        // nested lists AND vectors destructure positionally.
        Value::Cons(_) | Value::Vec(_) => seq_let_bind(i, pat, v, nbind),
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
        Value::Cons(_) | Value::Vec(_) => {}
        other => return Err(i.wrong_type_mut("sequencep", other)),
    }
    // Vector specs destructure just like lists (GNU maps them to
    // `seq' pcase patterns).
    let elems = match spec {
        Value::Cons(_) => want_list(i, spec)?,
        Value::Vec(v) => v.borrow().clone(),
        _ => unreachable!(),
    };
    let amp_rest = i.intern("&rest");
    let amp_opt = i.intern("&optional");
    let (mut idx, mut pos) = (0usize, 0usize);
    while pos < elems.len() {
        let el = elems[pos].clone();
        if i.sym_id(&el) == Some(amp_rest) {
            let Some(name) = elems.get(pos + 1) else {
                return Err(i.error("seq-let: &rest without variable"));
            };
            let v = f_seq_drop(i, vec![seqv.clone(), Value::Int(idx as i128)].into())?;
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
fn f_seq_let_raw(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
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
        Value::Str(s) => {
            let unibyte = i.is_unibyte_str(s);
            Ok(s.borrow()
                .chars()
                .map(|c| {
                    // Unibyte strings iterate their raw byte values.
                    if unibyte {
                        Value::Int(
                            crate::lisp::value::eight_bit_byte(c)
                                .map(|b| b as i128)
                                .unwrap_or((c as u32) as i128 & 0xFF),
                        )
                    } else {
                        Value::Int(crate::lisp::value::lisp_char_code(c))
                    }
                })
                .collect())
        }
        Value::Vec(vec) => Ok(vec.borrow().clone()),
        // Bool-vectors are bit sequences; GNU flattens them to
        // t/nil elements (append, vconcat, & co.).
        Value::Record(_) if super::misc::is_bool_vector(i, v) => {
            Ok(super::misc::bool_vec_of(i, v)?
                .into_iter()
                .map(Value::from_bool)
                .collect())
        }
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
        Value::Vec(_) => Value::vector(items),
        _ => Value::list(items),
    }
    .clone()
}

fn f_elt(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    // GNU Felt: cons/nil go through `nthcdr'; every other
    // argument must satisfy CHECK_ARRAY(sequence, sequencep)
    // before `aref's fixnum check on N runs.
    match &args[0] {
        Value::Nil | Value::Cons(_) => {
            super::listfn::f_nth(i, vec![args[1].clone(), args[0].clone()].into())
        }
        Value::Str(s) => {
            let n = want_fixnum(i, &args[1])?;
            let chars: Vec<char> = s.borrow().chars().collect();
            if n < 0 || n as usize >= chars.len() {
                return Err(i.signal_data(
                    sym::ARGS_OUT_OF_RANGE,
                    vec![args[0].clone(), args[1].clone()],
                ));
            }
            let c = chars[n as usize];
            // GNU: elements of a unibyte string are the raw byte
            // values (eight-bit proxies read back as 0x80-0xFF).
            let code = if i.is_unibyte_str(s) {
                crate::lisp::value::eight_bit_byte(c)
                    .map(|b| b as i128)
                    .unwrap_or((c as u32) as i128 & 0xFF)
            } else {
                crate::lisp::value::lisp_char_code(c)
            };
            Ok(Value::Int(code))
        }
        Value::Vec(v) => {
            let n = want_fixnum(i, &args[1])?;
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
            let n = want_fixnum(i, &args[1])?;
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
            let n = want_fixnum(i, &args[1])?;
            // GNU `char_table_ref': content slot → defalt → parent
            // chain.  The index is a character: out-of-range fixnums
            // are `(characterp N)', not `args-out-of-range'.
            if !(0..=0x3FFFFF).contains(&n) {
                return Err(i.wrong_type_mut("characterp", &args[1]));
            }
            Ok(super::misc::char_table_ref(i, &args[0], n as usize))
        }
        // Plain records are sequences for `elt' (GNU CHECK_ARRAY admits
        // them, then `aref' handles the index check itself).
        Value::Record(_) => f_aref(i, args),
        other => Err(i.wrong_type_mut("sequencep", other)),
    }
}

fn f_aref(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    // GNU Faref runs CHECK_FIXNUM on IDX before the
    // per-type dispatch on ARRAY — even a non-array argument
    // reports `(fixnump IDX)' when IDX isn't a fixnum.
    want_fixnum(i, &args[1])?;
    match &args[0] {
        Value::Str(_) | Value::Vec(_) => {}
        Value::Record(r) => {
            if super::misc::as_sympos(&args[0]).is_some() {
                return Err(i.wrong_type_mut("arrayp", &args[0]));
            }
            if !super::misc::is_bool_vector(i, &args[0]) && !super::misc::is_char_table(i, &args[0])
            {
                let n = want_fixnum(i, &args[1])?;
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
        Value::Lambda(l) => {
            // GNU `aref' reads the elements of closures and byte-code
            // objects even though `arrayp' is nil for both — the
            // [args body env nil docstring iform] view (truncated).
            let items = super::misc::lambda_slot_values(l);
            let n = want_fixnum(i, &args[1])?;
            if n < 0 || n as usize >= items.len() {
                return Err(i.signal_data(
                    sym::ARGS_OUT_OF_RANGE,
                    vec![args[0].clone(), args[1].clone()],
                ));
            }
            return Ok(items[n as usize].clone());
        }
        other => return Err(i.wrong_type_mut("arrayp", other)),
    }
    f_elt(i, args)
}

fn f_aset(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    // GNU `Faset' checks the fixnum index before dispatching on ARRAY.
    // Unlike aref, it rejects closure and byte-code function objects.
    let n = want_fixnum(i, &args[1])?;
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
            // GNU checks the index bounds before NEWCHAR's characterp —
            // `(aset "a" 1 nil)' signals args-out-of-range, not wta.
            let mut chars: Vec<char> = s.borrow().chars().collect();
            if n < 0 || n as usize >= chars.len() {
                return Err(i.signal_data(
                    sym::ARGS_OUT_OF_RANGE,
                    vec![args[0].clone(), args[1].clone()],
                ));
            }
            let nch = match &args[2] {
                Value::Int(x) => *x,
                _ => return Err(i.wrong_type_mut("characterp", &args[2])),
            };
            let c = crate::lisp::value::lisp_char(nch as u32)
                .ok_or_else(|| i.wrong_type_mut("characterp", &args[2]))?;
            let unibyte = i.is_unibyte_str(s);
            let old = chars[n as usize];
            if unibyte {
                // GNU: a unibyte string only stores byte values; a
                // 0x80-0xFF byte is the eight-bit char, held as its
                // PUA proxy internally.
                if !(0..=0xFF).contains(&nch) {
                    return Err(i.error("Attempt to store non-byte value into unibyte string"));
                }
                chars[n as usize] = if nch >= 0x80 {
                    char::from_u32(crate::lisp::value::EIGHT_BIT_BASE + nch as u32)
                        .unwrap_or('\u{fffd}')
                } else {
                    c
                };
            } else if nch >= 0x80 || (old as u32) >= 0x80 {
                // GNU multibyte rule: ASCII -> ASCII is always fine;
                // otherwise the new char must encode to the same byte
                // length as the old one.
                let byte_len = |ch: char, code: i128| -> usize {
                    if (0x3FFF80..=0x3FFFFF).contains(&code) {
                        2 // eight-bit codes are 2 bytes internally
                    } else {
                        ch.len_utf8()
                    }
                };
                let (nl, ol) = (
                    byte_len(c, nch),
                    byte_len(old, crate::lisp::value::lisp_char_code(old)),
                );
                if nl != ol {
                    return Err(i.error(if nch < 0x80 {
                        "Attempt to replace non-ASCII char in multibyte string"
                    } else {
                        "Attempt to store non-ASCII char into multibyte string"
                    }));
                }
                chars[n as usize] = c;
            } else {
                chars[n as usize] = c;
            }
            *s.borrow_mut() = chars.into_iter().collect();
            i.note_string_data_change();
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
            // Char-table: (aset CT CHAR VALUE) → `char_table_set';
            // the index must be a character (`characterp').
            if !(0..=super::misc::CT_MAX_CHAR as i128).contains(&n) {
                return Err(i.wrong_type_mut("characterp", &args[1]));
            }
            super::misc::ct_set(i, &args[0], n as u32, args[2].clone());
            Ok(args[2].clone())
        }
        Value::Record(r) => {
            // `symbols-with-pos-enabled' objects are not arrays.
            if super::misc::as_sympos(&args[0]).is_some() {
                return Err(i.wrong_type_mut("arrayp", &args[0]));
            }
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

pub(crate) fn f_copy_sequence(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    match &args[0] {
        Value::Cons(_) => {
            let items = super::listfn::copy_list_items(i, &args[0])?;
            Ok(Value::list(items))
        }
        Value::Str(s) => {
            // A real copy: new identity, own copy of prop intervals.
            let ns = crate::lisp::Value::string_ref(s.borrow().clone());
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
        Value::Nil => Ok(Value::Nil),
        // GNU's `copy_sequence' allocates a fresh vector with the same
        // elements; sharing the backing store would let `aset' mutate
        // the original (CC Mode's `c-find-assignment-for-mode' relies
        // on the copy for its source-position backup).
        Value::Vec(v) => Ok(Value::vector(v.borrow().clone())),
        // Hash tables aren't sequences in GNU (`sequencep' is nil):
        // `copy-sequence' signals `wrong-type-argument sequencep'.
        // Char-tables copy through `copy_char_table' (deep trie copy,
        // shared slot objects); other Records shallow-copy the vec.
        Value::Record(_) if super::misc::is_bool_vector(i, &args[0]) => {
            // Bool-vectors: copy the bit storage too — sharing it would
            // let `aset' on the copy mutate the original.
            let Value::Record(r) = &args[0] else {
                unreachable!()
            };
            let mut rec = r.borrow().clone();
            let bits = match rec.get(1) {
                Some(Value::Vec(b)) => Some(b.borrow().clone()),
                _ => None,
            };
            if let Some(bits) = bits {
                rec[1] = Value::vector(bits);
            }
            Ok(Value::record(rec))
        }
        // Char-tables copy through `copy_char_table' (deep trie copy,
        // shared slot objects); other Records shallow-copy the vec.
        Value::Record(_) if super::misc::is_char_table(i, &args[0]) => {
            Ok(super::misc::ct_copy(i, &args[0]))
        }
        Value::Record(r) => Ok(Value::record(r.borrow().clone())),
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
            let unibyte = i.is_unibyte_str(s);
            let chars: Vec<char> = s.borrow().chars().collect();
            for c in chars {
                // Unibyte strings yield their raw byte values (GNU
                // iterate-bytes), not the 0x3FFFxx char code.
                let code = if unibyte {
                    crate::lisp::value::eight_bit_byte(c)
                        .map(|b| b as i128)
                        .unwrap_or((c as u32) as i128 & 0xFF)
                } else {
                    crate::lisp::value::lisp_char_code(c)
                };
                f(i, &Value::Int(code))?;
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
        // GNU SEQUENCEP includes bool-vectors; map bits to t/nil.
        Value::Record(_) if super::misc::is_bool_vector(i, seq) => {
            let bits = super::misc::bool_vec_of(i, seq)?;
            for b in bits {
                f(i, &Value::from_bool(b))?;
            }
            Ok(())
        }
        other => Err(i.wrong_type_mut("sequencep", other)),
    }
}

fn f_mapcar(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    let fun = args[0].clone();
    let mut out = Vec::new();
    map_seq(i, &fun, &args[1], |i, v| {
        let r = i.apply(&fun, vec![v.clone()])?;
        out.push(r);
        Ok(())
    })?;
    Ok(Value::list(out))
}

fn f_mapc(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    let fun = args[0].clone();
    map_seq(i, &fun, &args[1], |i, v| {
        i.apply(&fun, vec![v.clone()])?;
        Ok(())
    })?;
    Ok(args[1].clone())
}

fn f_mapcan(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    // (mapcan FUNCTION SEQUENCE &rest SEQUENCES) — apply FUNCTION to
    // successive element tuples, then `nconc' the result lists.  GNU
    // splices the function's own list objects into the answer (no
    // copying), so the returned structure shares with them.
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
    let mut out = Value::Nil;
    let mut tail: Option<crate::lisp::value::ConsRef> = None;
    for (k, r) in results.iter().enumerate() {
        // The final nconc argument, including nil or an atom, replaces
        // the accumulated tail verbatim.  Earlier dotted tails drop out.
        if k + 1 == results.len() {
            match &tail {
                Some(t) => t.borrow_mut().cdr = r.clone(),
                None => out = r.clone(),
            }
            break;
        }
        if r.is_nil() {
            continue;
        }
        if !matches!(r, Value::Cons(_)) {
            return Err(i.wrong_type_mut("consp", r));
        }
        match &tail {
            Some(t) => t.borrow_mut().cdr = r.clone(),
            None => out = r.clone(),
        }
        let mut cur = r.clone();
        let mut seen = std::collections::HashSet::new();
        while let Value::Cons(c) = cur {
            if !seen.insert(std::rc::Rc::as_ptr(&c) as usize) {
                return Err(i.error("circular list in mapcan"));
            }
            let next = c.borrow().cdr.clone();
            tail = Some(c);
            cur = next;
        }
    }
    Ok(out)
}

fn f_mapconcat(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    let fun = args[0].clone();
    let sep = args.get(2).cloned().unwrap_or(Value::Nil);
    let mut parts = Vec::new();
    map_seq(i, &fun, &args[1], |i, v| {
        let r = i.apply(&fun, vec![v.clone()])?;
        if !parts.is_empty() {
            parts.push(sep.clone());
        }
        parts.push(r);
        Ok(())
    })?;
    // GNU passes the interleaved results and separators to concat.
    // This also preserves separator properties and multibyte status.
    super::strfn::f_concat(i, parts.into())
}

fn f_maphash(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
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

fn f_sort(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    // GNU fns.c
    // Fsort: nargs==2 → old-style (LESSP, in-place); EVEN count (incl.
    // trailing dangling keyword) → (error "Invalid argument list");
    // otherwise keyword pairs where a non-:key/:lessp/:reverse/:in-place
    // key → signal_error ("Invalid keyword argument", KEY).
    let rest = &args[1..];
    let (keyf, lessp, reverse, in_place) = if rest.len() == 1 {
        (Value::Nil, rest[0].clone(), false, true)
    } else if args.len() % 2 == 0 {
        return Err(i.error("Invalid argument list"));
    } else {
        let mut keyf = Value::Nil;
        let mut lessp = Value::Nil;
        let mut reverse = false;
        let mut in_place = false;
        let mut it = rest.iter();
        while let Some(k) = it.next() {
            let v = it
                .next()
                .cloned()
                .ok_or_else(|| i.error("Invalid argument list"))?;
            let kn = match k {
                Value::Sym(s) => i.obarray.name(*s).to_string(),
                _ => {
                    // signal_error ("Invalid keyword argument", K) →
                    // (error "Invalid keyword argument" K).
                    return Err(i.signal_data(
                        sym::ERROR,
                        vec![Value::string("Invalid keyword argument"), k.clone()],
                    ));
                }
            };
            match kn.as_str() {
                ":key" => keyf = v,
                ":lessp" => lessp = v,
                ":reverse" => reverse = v.truthy(),
                ":in-place" => in_place = v.truthy(),
                _ => {
                    return Err(i.signal_data(
                        sym::ERROR,
                        vec![Value::string("Invalid keyword argument"), k.clone()],
                    ));
                }
            }
        }
        (keyf, lessp, reverse, in_place)
    };
    match &args[0] {
        Value::Cons(_) => {
            // This is a sorting work array, not a managed Lisp vector.
            let items = std::rc::Rc::new(std::cell::RefCell::new(want_list(i, &args[0])?));
            super::sort::tim_sort(i, items.clone(), &lessp, &keyf, reverse)?;
            if !in_place {
                return Ok(Value::list(items.borrow().clone()));
            }
            // GNU converts lists to temporary vectors, copying values
            // back into the original conses only after a successful sort.
            let mut tail = args[0].clone();
            for v in items.borrow().iter() {
                if let Value::Cons(c) = tail {
                    c.borrow_mut().car = v.clone();
                    tail = c.borrow().cdr.clone();
                }
            }
            Ok(args[0].clone())
        }
        Value::Vec(v) => {
            let items = if in_place {
                v.clone()
            } else {
                crate::lisp::Value::vector_ref(v.borrow().clone())
            };
            super::sort::tim_sort(i, items.clone(), &lessp, &keyf, reverse)?;
            Ok(Value::Vec(items))
        }
        Value::Nil => Ok(Value::Nil),
        other => Err(i.wrong_type_mut("list-or-vector-p", other)),
    }
}

fn f_string_to_sequence(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    let items = seq_to_vec(i, &args[0])?;
    let ty = arg(&args, 1);
    match i.sym_id(&ty) {
        Some(s) if s == i.intern("list") || s == 0 => Ok(Value::list(items)),
        Some(s) if s == i.intern("vector") => Ok(Value::vector(items)),
        Some(s) if s == i.intern("string") => Ok(args[0].clone()),
        _ => Ok(Value::list(items)),
    }
}

fn f_seq(_i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    Ok(args.first().cloned().unwrap_or(Value::Nil))
}

fn f_append_to_list(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    let mut items = want_list(i, &args[0])?;
    items.push(args[1].clone());
    Ok(Value::list(items))
}

fn f_fillarray(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
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
            let item = match &args[1] {
                Value::Int(n) if (0..=0x3fffff).contains(n) => *n as u32,
                _ => return Err(i.wrong_type_mut("characterp", &args[1])),
            };
            let multibyte = super::strfn::string_multibyte_p(i, &args[0]);
            let mut st = s.borrow_mut();
            let n = st.chars().count();
            if n == 0 {
                return Ok(args[0].clone());
            }
            let character = if multibyte {
                crate::lisp::value::lisp_char(item)
                    .unwrap_or_else(|| crate::lisp::value::mule_seq_char(item))
            } else {
                let byte = item & 0xff;
                crate::lisp::value::lisp_char(if byte < 128 { byte } else { 0x3fff00 + byte })
                    .unwrap()
            };
            if multibyte {
                // GNU compares total Emacs storage bytes, not Rust UTF-8
                // proxy widths or the width of each original character.
                let width = |c| match crate::lisp::value::lisp_char_code(c) {
                    0..=0x7f => 1usize,
                    0x80..=0x7ff | 0x3fff80..=0x3fffff => 2,
                    0x800..=0xffff => 3,
                    0x10000..=0x1fffff => 4,
                    _ => 5,
                };
                let bytes: usize = st.chars().map(width).sum();
                if n.checked_mul(width(character)) != Some(bytes) {
                    return Err(i.error("Attempt to change byte length of a string"));
                }
            }
            *st = std::iter::repeat_n(character, n).collect();
            drop(st);
            i.note_string_data_change();
            // Preserve inferred ASCII-unibyte strings even when their new
            // contents consist entirely of raw-eight-bit proxy characters.
            if multibyte {
                i.mark_multibyte(s);
            } else {
                i.mark_unibyte(s);
            }
            Ok(args[0].clone())
        }
        other => Err(i.wrong_type_mut("arrayp", other)),
    }
}

fn f_make_vector(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    // GNU `Fmake_vector' CHECK_FIXNAT's LENGTH → `wholenump'.
    let n = match &args[0] {
        Value::Int(n) if *n >= 0 => *n as usize,
        _ => return Err(i.wrong_type_mut("wholenump", &args[0])),
    };
    if n > 0x2000_0000 {
        return Err(i.error("Memory exhausted--use C-x s then exit and restart Emacs"));
    }
    if n == 0 {
        // GNU: allocate_clear_vector(0) returns the shared zero_vector.
        return Ok(i.zero_vector.clone());
    }
    Ok(Value::vector(vec![args[1].clone(); n]))
}

fn f_vector(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    // GNU: `zero_vector' is shared — `(vector)' and `[]' are eq.
    if args.is_empty() {
        return Ok(i.zero_vector.clone());
    }
    Ok(Value::vector(args.as_slice().to_vec()))
}

fn f_bool_vector(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    Ok(super::misc::make_bool_vector(
        i,
        args.iter().map(|v| !v.is_nil()).collect(),
    ))
}

fn f_purecopy(_i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    Ok(args[0].clone())
}

pub(crate) fn f_nreverse_seq(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    match &args[0] {
        Value::Cons(_) => {
            // GNU reverses in place, detecting a cycle when the next
            // link returns to the original head. The error therefore
            // carries the already-mutated (usually one-element) head.
            let mut prev = Value::Nil;
            let mut cur = args[0].clone();
            loop {
                match &cur {
                    Value::Cons(c) => {
                        let next = c.borrow().cdr.clone();
                        if eq_values(&next, &args[0]) {
                            return Err(i.signal_data(sym::CIRCULAR_LIST, vec![args[0].clone()]));
                        }
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
        Value::Record(_) if crate::lisp::builtins::misc::is_bool_vector(i, &args[0]) => {
            // GNU Fnreverse reverses a bool-vector in place.
            if let Value::Record(r) = &args[0] {
                if let Some(Value::Vec(bv)) = r.borrow().get(1).cloned() {
                    bv.borrow_mut().reverse();
                }
            }
            Ok(args[0].clone())
        }
        Value::Vec(v) => {
            v.borrow_mut().reverse();
            Ok(args[0].clone())
        }
        // GNU delegates string nreverse directly to reverse: copy the
        // characters and storage kind, without retaining text properties.
        Value::Str(_) => super::listfn::f_reverse(i, args),
        Value::Nil => Ok(Value::Nil),
        other => Err(i.wrong_type_mut("arrayp", other)),
    }
}

fn f_clear_vector(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
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

fn f_seq_concatenate(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
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
        Value::vector(items)
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

fn f_seq_subseq(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
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

fn f_seq_take(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    let items = seq_to_vec(i, &args[0])?;
    let n = want_int(i, &args[1])?.max(0) as usize;
    Ok(seq_from_like(
        i,
        &args[0],
        items.into_iter().take(n).collect(),
    ))
}
fn f_seq_drop(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    let items = seq_to_vec(i, &args[0])?;
    let n = want_int(i, &args[1])?.max(0) as usize;
    Ok(seq_from_like(
        i,
        &args[0],
        items.into_iter().skip(n).collect(),
    ))
}
fn f_seq_length(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    Ok(Value::Int(seq_to_vec(i, &args[0])?.len() as i128))
}
fn f_seq_do(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    let fun = args[0].clone();
    map_seq(i, &fun, &args[1], |i, v| {
        i.apply(&fun, vec![v.clone()])?;
        Ok(())
    })?;
    // GNU returns the sequence itself.
    Ok(args[1].clone())
}
fn f_seq_map(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    let fun = args[0].clone();
    let mut out = Vec::new();
    map_seq(i, &fun, &args[1], |i, v| {
        out.push(i.apply(&fun, vec![v.clone()])?);
        Ok(())
    })?;
    Ok(Value::list(out))
}
fn f_seq_filter(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
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
fn f_seq_contains_p(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    let items = seq_to_vec(i, &args[0])?;
    for x in items {
        let r = match args.get(2) {
            Some(f) if f.truthy() => i.apply(f, vec![x.clone(), args[1].clone()])?,
            _ => {
                if equal_values(i, &x, &args[1])? {
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
fn f_seq_position(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    let items = seq_to_vec(i, &args[0])?;
    for (k, x) in items.iter().enumerate() {
        let eq = match args.get(2) {
            Some(f) if f.truthy() => i.apply(f, vec![x.clone(), args[1].clone()])?.truthy(),
            _ => equal_values(i, x, &args[1])?,
        };
        if eq {
            return Ok(Value::Int(k as i128));
        }
    }
    Ok(Value::Nil)
}
fn f_seq_count(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
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
fn f_seq_reverse(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    let items = seq_to_vec(i, &args[0])?;
    Ok(seq_from_like(
        i,
        &args[0],
        items.into_iter().rev().collect(),
    ))
}
fn f_seq_some(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
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
fn f_seq_every_p(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    let pred = args[0].clone();
    let items = seq_to_vec(i, &args[1])?;
    for v in items {
        if i.apply(&pred, vec![v])?.is_nil() {
            return Ok(Value::Nil);
        }
    }
    Ok(Value::t())
}
fn f_seq_find(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    let pred = args[0].clone();
    let items = seq_to_vec(i, &args[1])?;
    for v in items {
        if i.apply(&pred, vec![v.clone()])?.truthy() {
            return Ok(v);
        }
    }
    Ok(args.get(2).cloned().unwrap_or(Value::Nil))
}
fn f_seq_remove(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
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
fn f_seq_reduce(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    let fun = args[0].clone();
    let items = seq_to_vec(i, &args[1])?;
    let mut acc = args[2].clone();
    for v in items {
        acc = i.apply(&fun, vec![acc, v])?;
    }
    Ok(acc)
}
fn f_seq_take_while(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
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
fn f_seq_drop_while(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
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
fn f_seq_into(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    let items = seq_to_vec(i, &args[0])?;
    let list_id = i.intern("list");
    let vec_id = i.intern("vector");
    let str_id = i.intern("string");
    let ty = i.sym_id(&args[1]).unwrap_or(0);
    Ok(if ty == list_id {
        Value::list(items)
    } else if ty == vec_id {
        Value::vector(items)
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
fn f_seq_into_sequence(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    match &args[0] {
        Value::Nil | Value::Cons(_) | Value::Str(_) | Value::Vec(_) => Ok(args[0].clone()),
        other => {
            let msg = i.prin1_to_string(other);
            Err(i.error(&format!("Cannot convert {} into a sequence", msg)))
        }
    }
}
fn f_seq_empty_p(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    Ok(Value::from_bool(seq_to_vec(i, &args[0])?.is_empty()))
}
fn f_seq_first(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    let items = seq_to_vec(i, &args[0])?;
    Ok(items.into_iter().next().unwrap_or(Value::Nil))
}
fn f_seq_rest(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    let items = seq_to_vec(i, &args[0])?;
    Ok(seq_from_like(
        i,
        &args[0],
        items.into_iter().skip(1).collect(),
    ))
}
fn f_seq_last(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    let items = seq_to_vec(i, &args[0])?;
    Ok(items.into_iter().last().unwrap_or(Value::Nil))
}
fn f_seq_min(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    // GNU's seq-min applies `min' across the elements, so an empty
    // sequence fails with wrong-number-of-arguments and any
    // non-number element is a type error.
    let items = seq_to_vec(i, &args[0])?;
    let f = Value::Sym(i.intern("min"));
    i.apply(&f, items)
}
fn f_seq_max(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    let items = seq_to_vec(i, &args[0])?;
    let f = Value::Sym(i.intern("max"));
    i.apply(&f, items)
}
fn f_seq_uniq(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    let items = seq_to_vec(i, &args[0])?;
    let testfn = arg(&args, 1);
    let mut out: Vec<Value> = Vec::new();
    'outer: for v in items {
        for u in &out {
            let dup = if testfn.truthy() {
                i.apply(&testfn, vec![u.clone(), v.clone()])?.truthy()
            } else {
                equal_values(i, u, &v)?
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
fn f_make_char_table(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    // Char-table = #s(char-table SUBTYPE [65 slots] EXTRA...) — a
    // Record so `char-table-p'/`char-table-subtype' are exact.  The
    // contents vec is the GNU trie root: [0] is the ASCII cache slot,
    // [1..=64] the top-level 65536-char blocks, all initialized to
    // INIT (GNU `make_vector' fills every slot incl. the defalt).
    // GNU sizes the extras from the subtype's `char-table-extra-slots'
    // property (e.g. disp-table.el puts 18 on `display-table').
    let subtype = arg(&args, 0);
    let init = arg(&args, 1);
    let vec = Value::vector(vec![init.clone(); 65]);
    let prop = i.intern("char-table-extra-slots");
    // GNU `CHECK_FIXNAT': a non-nil property must be a natural number
    // — `(wholenump PROP)', not silently zero slots.
    let n = match crate::lisp::builtins::data::f_get(
        i,
        vec![subtype.clone(), Value::Sym(prop)].into(),
    )? {
        Value::Nil => 0,
        Value::Int(n) if n >= 0 => (n as usize).min(1 << 22),
        ref other => return Err(i.wrong_type_mut("wholenump", other)),
    };
    let mut rec = vec![Value::Sym(i.intern("char-table")), subtype, vec];
    rec.resize(3 + n, Value::Nil);
    let t = Value::record(rec);
    if !init.is_nil() {
        i.set_char_table_defalt(&t, init);
    }
    Ok(t)
}

/// `f_make_char_table' factored for non-call sites (eval.rs seeds
/// `keyboard-translate-table' before Lisp runs).
pub(crate) fn make_char_table_raw(i: &mut Interp, subtype: Value, init: Value) -> Value {
    let vec = Value::vector(vec![init.clone(); 65]);
    let prop = i.intern("char-table-extra-slots");
    let n =
        match crate::lisp::builtins::data::f_get(i, vec![subtype.clone(), Value::Sym(prop)].into())
        {
            Ok(Value::Int(n)) if n >= 0 => (n as usize).min(1 << 22),
            _ => 0,
        };
    let mut rec = vec![Value::Sym(i.intern("char-table")), subtype, vec];
    rec.resize(3 + n, Value::Nil);
    let t = Value::record(rec);
    if !init.is_nil() {
        i.set_char_table_defalt(&t, init);
    }
    t
}
