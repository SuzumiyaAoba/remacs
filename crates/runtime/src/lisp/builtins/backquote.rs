//! Rust port of GNU `emacs-lisp/backquote.el' expansion machinery.
//!
//! GNU byte-compiles backquote.el, so a user `(fmakunbound 'car)' (or
//! any other cell) cannot break `backquote-process' — its `car' calls
//! are `Bcar' opcodes, not symbol-cell lookups.  remacs interprets the
//! prelude copy instead, which makes every `` ` `` expansion fragile
//! once `car'/`cdr'/`nth'/`nreverse'/`append' are voided and slower
//! at macroexpansion time.  Keeping the recursion in Rust restores
//! the bytecode-level robustness.
//!
//! `backquote'/`\`'/`backquote-list*' are installed as
//! `(macro . #<subr>)' cells by `install_macros' — `macro_expand_call'
//! already applies a macro-marked subr to the raw argument list.

use super::S;
use crate::lisp::Interp;
use crate::lisp::error::{EvalResult, Flow};
use crate::lisp::obarray::sym;
use crate::lisp::value::{Subr, SymId, Value};
use num_bigint::BigInt;
use rustc_hash::FxHashSet;
use std::rc::Rc;

fn car(i: &Interp, v: &Value) -> Result<Value, Flow> {
    match v {
        Value::Cons(c) => c
            .try_borrow()
            .map(|cell| cell.car.clone())
            .map_err(|_| i.error("Cannot inspect a mutably borrowed backquote cons cell")),
        _ => Ok(Value::Nil),
    }
}

fn cdr(i: &Interp, v: &Value) -> Result<Value, Flow> {
    match v {
        Value::Cons(c) => c
            .try_borrow()
            .map(|cell| cell.cdr.clone())
            .map_err(|_| i.error("Cannot inspect a mutably borrowed backquote cons cell")),
        _ => Ok(Value::Nil),
    }
}

fn consp(v: &Value) -> bool {
    matches!(v, Value::Cons(_))
}

/// GNU `listp': NILP or CONSP.
fn listp(v: &Value) -> bool {
    matches!(v, Value::Cons(_) | Value::Nil)
}

fn nth(i: &Interp, v: &Value, n: usize) -> Result<Value, Flow> {
    car(i, &nthcdr(i, v, n)?)
}

fn nthcdr(i: &Interp, v: &Value, n: usize) -> Result<Value, Flow> {
    let mut cur = v.clone();
    for _ in 0..n {
        cur = cdr(i, &cur)?;
    }
    Ok(cur)
}

/// `length' of a proper list; signals `wrong-type-argument listp' on a
/// dotted/improper tail like GNU's Flength.
fn list_length(i: &mut Interp, v: &Value) -> Result<usize, Flow> {
    let mut n = 0usize;
    let mut cur = v.clone();
    let mut seen = FxHashSet::default();
    loop {
        match cur.clone() {
            Value::Cons(c) => {
                if !seen.insert(Rc::as_ptr(&c) as usize) {
                    return Err(crate::lisp::builtins::listfn::err_circular(i, &cur));
                }
                cur = c
                    .try_borrow()
                    .map_err(|_| i.error("Cannot inspect a mutably borrowed backquote form"))?
                    .cdr
                    .clone();
                n += 1;
            }
            Value::Nil => return Ok(n),
            other => return Err(i.wrong_type_mut("listp", &other)),
        }
    }
}

fn is_tag(v: &Value, n: i128) -> bool {
    matches!(v, Value::Int(t) if *t == n)
}

/// `backquote-process' defaults only nil/omitted LEVEL to zero.  Other
/// values are checked lazily when a nested comma/backquote needs arithmetic.
fn bq_level_arg(level: Option<&Value>) -> Value {
    match level {
        None | Some(Value::Nil) => Value::Int(0),
        Some(value) => value.clone(),
    }
}

fn bq_level_nonpositive(i: &mut Interp, level: &Value) -> Result<bool, Flow> {
    match level {
        Value::Int(number) => Ok(*number <= 0),
        Value::BigInt(number) => Ok(number.as_ref() <= &BigInt::from(0)),
        Value::Float(number) => Ok(**number <= 0.0),
        Value::Marker(marker) => {
            let marker = marker.borrow();
            if marker.buffer.is_none() {
                return Err(i.error("Marker does not point anywhere"));
            }
            Ok(marker.position as i128 + 1 <= 0)
        }
        other => Err(i.wrong_type_mut("number-or-marker-p", other)),
    }
}

fn bq_level_adjust(i: &mut Interp, level: &Value, delta: i8) -> Result<Value, Flow> {
    let delta = i128::from(delta);
    match level {
        Value::Int(number) => Ok(Value::from_bigint(BigInt::from(*number) + delta)),
        Value::BigInt(number) => Ok(Value::from_bigint(number.as_ref() + delta)),
        Value::Float(number) => Ok(Value::float(**number + delta as f64)),
        Value::Marker(marker) => {
            let marker = marker.borrow();
            if marker.buffer.is_none() {
                return Err(i.error("Marker does not point anywhere"));
            }
            let position = marker.position as i128 + 1;
            Ok(Value::from_bigint(BigInt::from(position) + delta))
        }
        other => Err(i.wrong_type_mut("number-or-marker-p", other)),
    }
}

/// The configured marker symbols — GNU reads the `backquote-*-symbol'
/// variables each expansion, so callers may rebind them.
struct BqSyms {
    backquote: SymId,
    unquote: SymId,
    splice: SymId,
}

type BqActive = FxHashSet<(u8, usize)>;
const BQ_CONS: u8 = 0;
const BQ_VECTOR: u8 = 1;

fn bq_syms(i: &mut Interp) -> BqSyms {
    BqSyms {
        backquote: i.q("backquote-backquote-symbol"),
        unquote: i.q("backquote-unquote-symbol"),
        splice: i.q("backquote-splice-symbol"),
    }
}

/// GNU compares against the current variable value with `eq'.  Keep the
/// lookup lazy so atoms do not read these variables, and so bindings changed
/// during expansion take effect at the same comparison point.
fn bq_marker_eq(i: &mut Interp, value: &Value, marker_variable: SymId) -> Result<bool, Flow> {
    let marker = i.symbol_value(marker_variable);
    if matches!(&marker, Value::Sym(id) if *id == sym::UNBOUND) {
        return Err(i.signal_data(sym::VOID_VARIABLE, vec![Value::Sym(marker_variable)]));
    }
    Ok(crate::lisp::builtins::eq_values_with_interp(
        i, value, &marker,
    ))
}

/// `(error "...: %S" s)' — GNU formats the offending form with prin1.
fn bq_error(i: &mut Interp, msg_prefix: &str, s: &Value) -> Flow {
    let rendered = i.prin1_to_string(s);
    i.error(format!("{msg_prefix}{rendered}"))
}

fn quote_sym(i: &mut Interp) -> Value {
    let id = i.q("quote");
    i.sym(id)
}

fn named_sym(i: &mut Interp, name: &str) -> Value {
    let id = i.q(name);
    i.sym(id)
}

/// Port of `backquote-process' (backquote.el).  Returns the
/// `(TAG . FORM)' pair: 0 constant, 1 form evaluates to the
/// structure, 2 form produces a list to splice.
fn bq_process(i: &mut Interp, s: &Value, level: &Value, syms: &BqSyms) -> Result<Value, Flow> {
    bq_process_inner(i, s, level, syms, &mut FxHashSet::default())
}

/// Walk one backquote object while tracking only the active recursion path.
/// Shared, acyclic subobjects remain valid and are expanded at each use.
fn bq_process_inner(
    i: &mut Interp,
    s: &Value,
    level: &Value,
    syms: &BqSyms,
    active: &mut BqActive,
) -> Result<Value, Flow> {
    match s {
        Value::Vec(v) => {
            let key = (BQ_VECTOR, Rc::as_ptr(v) as usize);
            if !active.insert(key) {
                return Err(crate::lisp::builtins::listfn::err_circular(i, s));
            }
            let result = (|| {
                // `(append s ())' — vector to list, then re-run.
                let elems: Vec<Value> = v
                    .try_borrow()
                    .map_err(|_| i.error("Cannot inspect a mutably borrowed backquote vector"))?
                    .clone();
                let n = bq_process_inner(i, &Value::list(elems), level, syms, active)?;
                if is_tag(&car(i, &n)?, 0) {
                    return Ok(Value::cons(Value::Int(0), s.clone()));
                }
                let list_id = i.q("list");
                let append_id = i.q("append");
                let form = cdr(i, &n)?;
                // n = (tag . form): `(nth 1 n)' is `car form', `(nthcdr 2 n)'
                // is `cdr form' — GNU relies on the same pair indexing.
                let result = if !listp(&form) {
                    Value::list(vec![named_sym(i, "vconcat"), form])
                } else if i.sym_is(&car(i, &form)?, list_id) {
                    Value::cons(named_sym(i, "vector"), cdr(i, &form)?)
                } else if i.sym_is(&car(i, &form)?, append_id) {
                    Value::cons(named_sym(i, "vconcat"), cdr(i, &form)?)
                } else {
                    // (list 'apply '(function vector) form)
                    let q = Value::list(vec![named_sym(i, "function"), named_sym(i, "vector")]);
                    Value::list(vec![named_sym(i, "apply"), q, form])
                };
                Ok(Value::cons(Value::Int(1), result))
            })();
            active.remove(&key);
            result
        }
        Value::Cons(_) => {
            let start = match s {
                Value::Cons(cell) => (BQ_CONS, Rc::as_ptr(cell) as usize),
                _ => unreachable!(),
            };
            if !active.insert(start) {
                return Err(crate::lisp::builtins::listfn::err_circular(i, s));
            }
            let mut entered = vec![start];
            let result = (|| {
                let head = car(i, s)?;
                if bq_marker_eq(i, &head, syms.unquote)? {
                    if bq_level_nonpositive(i, level)? {
                        if list_length(i, s)? > 2 {
                            return Err(bq_error(i, "Multiple args to , are not supported: ", s));
                        }
                        let qid = i.q("quote");
                        let inner = nth(i, s, 1)?;
                        let tag = if i.sym_is(&car(i, &inner)?, qid) {
                            0
                        } else {
                            1
                        };
                        return Ok(Value::cons(Value::Int(tag), inner));
                    }
                    let next_level = bq_level_adjust(i, level, -1)?;
                    return bq_delay_process_inner(i, s, &next_level, syms, active);
                }
                if bq_marker_eq(i, &head, syms.splice)? {
                    if bq_level_nonpositive(i, level)? {
                        if list_length(i, s)? > 2 {
                            return Err(bq_error(i, "Multiple args to ,@ are not supported: ", s));
                        }
                        return Ok(Value::cons(Value::Int(2), nth(i, s, 1)?));
                    }
                    let next_level = bq_level_adjust(i, level, -1)?;
                    return bq_delay_process_inner(i, s, &next_level, syms, active);
                }
                if bq_marker_eq(i, &head, syms.backquote)? {
                    let next_level = bq_level_adjust(i, level, 1)?;
                    return bq_delay_process_inner(i, s, &next_level, syms, active);
                }

                // List level: scan items, collecting FIRSTLIST/LIST/LISTS.
                // The active path catches cdr cycles and recursive car
                // references without rejecting shared acyclic subobjects.
                // Accumulators stay newest-first like GNU's cons chains.
                let mut firstlist: Vec<Value> = Vec::new();
                let mut list: Vec<Value> = Vec::new();
                let mut lists: Vec<Value> = Vec::new();
                let mut rest = s.clone();
                let mut first_cell = true;
                while consp(&rest) {
                    let rcar = car(i, &rest)?;
                    if !first_cell
                        && (bq_marker_eq(i, &rcar, syms.unquote)?
                            || bq_marker_eq(i, &rcar, syms.backquote)?)
                    {
                        break;
                    }
                    if !first_cell {
                        let Value::Cons(cell) = &rest else {
                            unreachable!()
                        };
                        let key = (BQ_CONS, Rc::as_ptr(cell) as usize);
                        if !active.insert(key) {
                            return Err(crate::lisp::builtins::listfn::err_circular(i, &rest));
                        }
                        entered.push(key);
                    }
                    first_cell = false;
                    let item = bq_process_inner(i, &rcar, level, syms, active)?;
                    if is_tag(&car(i, &item)?, 2) {
                        if lists.is_empty() {
                            firstlist = std::mem::take(&mut list);
                        }
                        if !list.is_empty() {
                            let grouped = bq_listify(
                                i,
                                &Value::list(std::mem::take(&mut list)),
                                &Value::cons(Value::Int(0), Value::Nil),
                                syms,
                            )?;
                            lists.insert(0, grouped);
                        }
                        lists.insert(0, cdr(i, &item)?);
                    } else {
                        list.insert(0, item);
                    }
                    rest = cdr(i, &rest)?;
                }
                // Tail: `rest' is nil, a dotted atom, or a stopped comma/
                // backquote form. The stopped special form is left out of
                // `entered' so its recursive visit owns that path entry.
                if !rest.is_nil() || !list.is_empty() {
                    let tail_pair = bq_process_inner(i, &rest, level, syms, active)?;
                    let grouped = bq_listify(i, &Value::list(list), &tail_pair, syms)?;
                    lists.insert(0, grouped);
                }
                let first_is_splice = match lists.first() {
                    Some(Value::Cons(c)) => {
                        let head = car(i, &Value::Cons(c.clone()))?;
                        bq_marker_eq(i, &head, syms.splice)?
                    }
                    _ => false,
                };
                let expression = if lists.len() > 1 || first_is_splice {
                    let mut items = vec![named_sym(i, "append")];
                    items.extend(lists.iter().rev().cloned());
                    Value::list(items)
                } else {
                    lists.into_iter().next().unwrap_or(Value::Nil)
                };
                let expression = if !firstlist.is_empty() {
                    bq_listify(
                        i,
                        &Value::list(firstlist),
                        &Value::cons(Value::Int(1), expression),
                        syms,
                    )?
                } else {
                    expression
                };
                let qid = i.q("quote");
                let tag = if i.sym_is(&car(i, &expression)?, qid) {
                    0
                } else {
                    1
                };
                Ok(Value::cons(Value::Int(tag), expression))
            })();
            for key in entered {
                active.remove(&key);
            }
            result
        }
        _ => {
            // Atom: `(null s)', `t' and non-symbols pass raw; other
            // symbols get `(quote s)'.  Symbol-with-position objects
            // (`symbols-with-pos-enabled' records and `symbol-with-pos'
            // PVECs) satisfy `symbolp' in GNU — without the quote their
            // bare return is a *variable reference*, and `listify's
            // `eval' would signal `void-variable' (ds11: `` `(x (f))'
            // inside a file read by `read-positioning-symbols').
            let body = match s {
                Value::Nil => s.clone(),
                Value::Sym(id) if *id == sym::T => s.clone(),
                Value::Sym(_) => Value::list(vec![quote_sym(i), s.clone()]),
                v if crate::lisp::builtins::misc::any_sympos_parts(i, v).is_some() => {
                    Value::list(vec![quote_sym(i), v.clone()])
                }
                _ => s.clone(),
            };
            Ok(Value::cons(Value::Int(0), body))
        }
    }
}

/// Port of `backquote-delay-process'.
fn bq_delay_process_inner(
    i: &mut Interp,
    s: &Value,
    level: &Value,
    syms: &BqSyms,
    active: &mut BqActive,
) -> Result<Value, Flow> {
    let head_item = Value::cons(Value::Int(0), Value::list(vec![quote_sym(i), car(i, s)?]));
    let tail = bq_process_inner(i, &cdr(i, s)?, level, syms, active)?;
    let exp = bq_listify(i, &Value::list(vec![head_item]), &tail, syms)?;
    let qid = i.q("quote");
    let tag = if i.sym_is(&car(i, &exp)?, qid) { 0 } else { 1 };
    Ok(Value::cons(Value::Int(tag), exp))
}

fn bq_delay_process(
    i: &mut Interp,
    s: &Value,
    level: &Value,
    syms: &BqSyms,
) -> Result<Value, Flow> {
    bq_delay_process_inner(i, s, level, syms, &mut FxHashSet::default())
}

/// Port of `backquote-listify'.  `list' is a proper list of
/// `(TAG . FORM)' pairs; `old_tail' is such a pair for the tail.
fn bq_listify(
    i: &mut Interp,
    list: &Value,
    old_tail: &Value,
    syms: &BqSyms,
) -> Result<Value, Flow> {
    let mut heads: Vec<Value> = Vec::new(); // newest first (GNU conses)
    let mut tail = cdr(i, old_tail)?;
    let mut old_tail_live = old_tail.clone();
    if is_tag(&car(i, old_tail)?, 0) {
        tail = i.eval(&tail)?;
        old_tail_live = Value::Nil;
    }
    let mut list_tail = list.clone();
    while consp(&list_tail) {
        let item = car(i, &list_tail)?;
        list_tail = cdr(i, &list_tail)?;
        if !heads.is_empty() || !old_tail_live.is_nil() || !is_tag(&car(i, &item)?, 0) {
            heads.insert(0, cdr(i, &item)?);
        } else {
            if std::env::var_os("REMACS_BQ_DBG").is_some() {
                eprintln!("BQ listify eval: {}", i.prin1_to_string(&cdr(i, &item)?));
            }
            let evaled = i.eval(&cdr(i, &item)?)?;
            tail = Value::cons(evaled, tail);
        }
    }
    if !tail.is_nil() {
        if old_tail_live.is_nil() {
            tail = Value::list(vec![quote_sym(i), tail]);
        }
        if heads.is_empty() {
            return Ok(tail);
        }
        // `(cdr heads)' non-nil => >1 heads => list*; else a single
        // head that is a splice form also uses list*.
        let first_is_splice = match heads.first() {
            Some(Value::Cons(h)) => {
                let head = car(i, &Value::Cons(h.clone()))?;
                bq_marker_eq(i, &head, syms.splice)?
            }
            _ => false,
        };
        let name = if heads.len() > 1 || first_is_splice {
            "backquote-list*"
        } else {
            "cons"
        };
        let mut items = heads;
        items.push(tail);
        Ok(Value::cons(named_sym(i, name), Value::list(items)))
    } else {
        Ok(Value::cons(named_sym(i, "list"), Value::list(heads)))
    }
}

// ---------- subrs ----------

/// `backquote-process' as a subr: (S &optional LEVEL).
fn f_backquote_process(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let level = bq_level_arg(a.get(1));
    let syms = bq_syms(i);
    bq_process(i, &a[0], &level, &syms)
}

// ---------- GNU DEFUN entry checks (backquote.c) ----------

/// GNU CHECK_LIST: proper-list check; the first non-nil non-cons cdr
/// is reported with `wrong-type-argument listp'.
fn bq_check_list(i: &mut Interp, v: &Value) -> Result<(), Flow> {
    let mut cur = v.clone();
    let mut seen = FxHashSet::default();
    loop {
        match cur.clone() {
            Value::Nil => return Ok(()),
            Value::Cons(c) => {
                if !seen.insert(Rc::as_ptr(&c) as usize) {
                    return Err(crate::lisp::builtins::listfn::err_circular(i, &cur));
                }
                let next = c
                    .try_borrow()
                    .map_err(|_| i.error("Cannot inspect a mutably borrowed backquote list"))?
                    .cdr
                    .clone();
                cur = next;
            }
            other => return Err(i.wrong_type_mut("listp", &other)),
        }
    }
}

/// GNU CHECK_LIST_CAR: every element of `list` must be a cons pair.
/// Its cdr is the backquote FORM and may be dotted.
fn bq_check_list_car(i: &mut Interp, list: &Value) -> Result<(), Flow> {
    let mut cur = list.clone();
    let mut seen = FxHashSet::default();
    loop {
        match &cur {
            Value::Nil => return Ok(()),
            Value::Cons(c) => {
                if !seen.insert(Rc::as_ptr(c) as usize) {
                    return Err(crate::lisp::builtins::listfn::err_circular(i, &cur));
                }
                let (car_v, cdr_v) = {
                    let b = c
                        .try_borrow()
                        .map_err(|_| i.error("Cannot inspect a mutably borrowed backquote list"))?;
                    (b.car.clone(), b.cdr.clone())
                };
                if !matches!(car_v, Value::Cons(_)) {
                    return Err(i.wrong_type_mut("listp", &car_v));
                }
                cur = cdr_v;
            }
            other => return Err(i.wrong_type_mut("listp", other)),
        }
    }
}

/// GNU CHECK_NUMBER_OR_MARKER.
fn bq_check_num_or_mark(i: &mut Interp, v: &Value) -> Result<(), Flow> {
    match v {
        Value::Int(_) | Value::BigInt(_) | Value::Float(_) => Ok(()),
        Value::Marker(marker) => {
            if marker.borrow().buffer.is_none() {
                Err(i.error("Marker does not point anywhere"))
            } else {
                Ok(())
            }
        }
        other => Err(i.wrong_type_mut("number-or-marker-p", other)),
    }
}

/// `backquote-listify' subr: (LIST OLD-TAIL).  GNU entry checks:
/// CHECK_LIST on LIST, CHECK_LIST_CAR on its elements, old_tail must
/// be nil-or-cons (`listp' error — GNU CHECK_CONS uses Qlistp), and
/// CHECK_NUMBER_OR_CAR: a cons old_tail's car (or a non-cons old_tail
/// itself) must satisfy number-or-marker-p.
fn f_backquote_listify(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU's listify reads the TAIL first: `(backquote-listify "a" "b")'
    // signals listp "b", not listp "a".  Syms count as lists here.
    match &a[1] {
        Value::Cons(_) | Value::Nil => {}
        other => return Err(i.wrong_type_mut("listp", other)),
    }
    bq_check_list(i, &a[0])?;
    bq_check_list_car(i, &a[0])?;
    match &a[1] {
        Value::Cons(c) => {
            let car_v = c
                .try_borrow()
                .map_err(|_| i.error("Cannot inspect a mutably borrowed backquote cons cell"))?
                .car
                .clone();
            bq_check_num_or_mark(i, &car_v)?;
        }
        Value::Nil => bq_check_num_or_mark(i, &Value::Nil)?,
        other => return Err(i.wrong_type_mut("listp", other)),
    }
    let syms = bq_syms(i);
    bq_listify(i, &a[0], &a[1], &syms)
}

/// `backquote-delay-process' subr: (S LEVEL).  GNU CHECK_LISTs S.
fn f_backquote_delay_process(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    bq_check_list(i, &a[0])?;
    let syms = bq_syms(i);
    let level = bq_level_arg(a.get(1));
    bq_delay_process(i, &a[0], &level, &syms)
}

/// `backquote-list*-function' (the defun): `(list* FIRST LIST...)'.
fn f_backquote_liststar_function(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let _ = i;
    if a.len() <= 1 {
        return Ok(a.first().cloned().unwrap_or(Value::Nil));
    }
    // Last arg is the tail; fold earlier args onto it right-to-left.
    let mut out = a.last().cloned().unwrap_or(Value::Nil);
    for v in a[..a.len() - 1].iter().rev() {
        out = Value::cons(v.clone(), out);
    }
    Ok(out)
}

/// `backquote' macro body: `(cdr (backquote-process structure))'.
/// Installed as `(macro . #<subr>)' so expansion never consults a
/// user-voidable function cell.
fn f_backquote_macro(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    if std::env::var_os("REMACS_BQ_DBG").is_some() {
        eprintln!("BQ macro arg: {}", i.prin1_to_string(&a[0]));
    }
    let syms = bq_syms(i);
    let level = Value::Int(0);
    let pair = bq_process(i, &a[0], &level, &syms)?;
    cdr(i, &pair)
}

/// `backquote-list*-macro' expansion (backquote.el): folds the whole
/// spine into a `cons' chain — `(backquote-list* A B C)' expands to
/// `(cons A (cons B C))'.
fn f_backquote_liststar_macro(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU: (setq list (nreverse (cons first list))) → args reversed;
    // `first' := the last arg, `list' := the rest reversed (oldest last).
    let mut rev: Vec<Value> = a.iter().cloned().collect();
    rev.reverse();
    let first = rev.remove(0);
    if rev.is_empty() {
        return Ok(first);
    }
    let cons_sym = named_sym(i, "cons");
    let mut newlist = Value::list(vec![cons_sym.clone(), rev.remove(0), first]);
    for v in rev {
        newlist = Value::list(vec![cons_sym.clone(), v, newlist]);
    }
    Ok(newlist)
}

static BQ_MACRO: Subr = S!(
    "backquote",
    1,
    1,
    f_backquote_macro,
    "Argument STRUCTURE describes a template to build.\n\nThe whole structure acts as if it were quoted except for certain\nplaces where expressions are evaluated and inserted or spliced in.\n\nFor example:\n\nb              => (ba bb bc)\t\t; assume b has this value\n`(a b c)       => (a b c)\t\t; backquote acts like quote\n`(a ,b c)      => (a (ba bb bc) c)\t; insert the value of b\n`(a ,@b c)     => (a ba bb bc c)\t; splice in the value of b\n\nVectors work just like lists.  Nested backquotes are permitted.\n\nNote that some macros, such as `pcase', use this symbol for other\npurposes."
);
static BQ_LISTSTAR_MACRO: Subr = S!(
    "backquote-list*",
    many 1,
    f_backquote_liststar_macro,
    "Like `list' but the last argument is the tail of the new list.\n\nFor example (backquote-list* 'a 'b 'c) => (a b . c)"
);

pub(super) fn dump_macro_subrs() -> [&'static Subr; 2] {
    [&BQ_MACRO, &BQ_LISTSTAR_MACRO]
}

pub(crate) static SUBRS: &[Subr] = &[
    S!(
        "backquote-process",
        1,
        2,
        f_backquote_process,
        "Process a backquote body (internal)"
    ),
    S!(
        "backquote-listify",
        2,
        2,
        f_backquote_listify,
        "Combine backquote pieces (internal)"
    ),
    S!(
        "backquote-delay-process",
        2,
        2,
        f_backquote_delay_process,
        "Nested backquote piece (internal)"
    ),
    S!(
        "backquote-list*-function",
        many 1,
        f_backquote_liststar_function,
        "list* with tail (internal)"
    ),
];

/// Re-point the backquote machinery at the Rust subrs and bind
/// `backquote'/`\`'/`backquote-list*' to `(macro . #<subr>)' cells.
/// Called at the end of `Interp::new' — after every dumped library
/// load — so the prelude's Lisp copies can't re-shadow them later
/// and the expander survives `(fmakunbound 'car)'-style voiding
/// exactly like GNU's bytecompiled backquote.el does.
pub fn install_macros(i: &mut Interp) {
    // These are literal symbol references in GNU's byte-compiled source.
    // Cache them before user code can unintern them; later lookups must keep
    // using the original cells rather than interning a same-name replacement.
    for name in [
        "backquote-backquote-symbol",
        "backquote-unquote-symbol",
        "backquote-splice-symbol",
        "quote",
        "vconcat",
        "vector",
        "function",
        "apply",
        "append",
        "backquote-list*",
        "cons",
        "list",
    ] {
        i.q(name);
    }
    for s in SUBRS {
        let id = i.q(s.name);
        i.fset(id, Value::Subr(s));
    }
    let macro_sym = Value::Sym(sym::MACRO);
    for (name, subr) in [
        ("backquote", &BQ_MACRO),
        ("`", &BQ_MACRO),
        ("backquote-list*", &BQ_LISTSTAR_MACRO),
    ] {
        let id = i.q(name);
        i.fset(id, Value::cons(macro_sym.clone(), Value::Subr(subr)));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lisp::{error::Flow, obarray::Obarray};
    use std::{cell::RefCell, panic::AssertUnwindSafe};

    fn assert_borrowed_backquote_value_errors(value: &Value, borrow: impl FnOnce()) {
        let mut interp = Interp::vacant();
        interp.obarray = Obarray::new();
        let markers = [
            ("backquote-backquote-symbol", "backquote"),
            ("backquote-unquote-symbol", "unquote"),
            ("backquote-splice-symbol", "splice"),
        ];
        for (variable, value) in markers {
            let variable = interp.intern(variable);
            let value = interp.intern(value);
            interp.set_symbol(variable, Value::Sym(value)).unwrap();
        }
        let syms = bq_syms(&mut interp);
        let level = Value::Int(0);
        let result = std::panic::catch_unwind(AssertUnwindSafe(|| {
            bq_process(&mut interp, value, &level, &syms)
        }));
        borrow();
        assert!(matches!(
            result,
            Ok(Err(Flow::Signal(ref signal))) if signal.is_symbol(sym::ERROR)
        ));
    }

    #[test]
    fn backquote_reports_mutably_borrowed_cons_and_vector_without_panicking() {
        let cons = Value::cons(Value::Sym(sym::T), Value::Nil);
        let Value::Cons(cell) = &cons else {
            unreachable!()
        };
        let cons_borrow = cell.borrow_mut();
        assert_borrowed_backquote_value_errors(&cons, || drop(cons_borrow));

        let vector = Value::Vec(Rc::new(RefCell::new(vec![Value::Nil])));
        let Value::Vec(items) = &vector else {
            unreachable!()
        };
        let vector_borrow = items.borrow_mut();
        assert_borrowed_backquote_value_errors(&vector, || drop(vector_borrow));
    }
}
