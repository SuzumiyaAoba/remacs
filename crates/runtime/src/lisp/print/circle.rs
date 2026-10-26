//! Per-print identity/numbering state and GNU print-number-table preprocessing.

use crate::lisp::{Interp, text::EmacsText, value::Value};
use std::cell::RefCell;

thread_local! {
    /// GNU print.c's `being_printed' stack (used when print-circle
    /// is nil): every object printing at depth D sits in slot D, and
    /// an object that reoccurs while still on the stack prints as
    /// `#N' where N is its stack index.  Only containers can be
    /// ancestors, so only they are tracked.
    pub(super) static BEING_PRINTED: RefCell<Vec<usize>> = const { RefCell::new(Vec::new()) };
    /// `print-circle' state for the current top-level print call:
    /// objects seen by the pre-scan, labels numbered at each shared
    /// object's second encounter (GNU's `print_number_index' order),
    /// and which labeled objects have already printed.
    pub(super) static CIRCLE: RefCell<Option<CircleCtx>> = const { RefCell::new(None) };
    /// `print-unreadable-function' support (GNU print.c
    /// `print_unreadable_flag'): every `#<...>'-style emit records its
    /// character span and object here so the `&mut' caller can post-process
    /// with a Lisp callback.
    pub(super) static UNREADABLE_HITS: RefCell<Vec<(usize, usize, Value)>> =
        const { RefCell::new(Vec::new()) };
}

/// Per-print state for `print-circle'.
pub(super) struct CircleCtx {
    /// Objects already seen by the pre-scan.
    pub(super) seen: rustc_hash::FxHashSet<usize>,
    /// ptr -> label number, assigned at second encounter.
    pub(super) labels: rustc_hash::FxHashMap<usize, usize>,
    /// Objects whose `#N=' form has already printed.
    pub(super) printed: rustc_hash::FxHashSet<usize>,
    /// Next label index (GNU's print_number_index starts at 1).
    pub(super) next: usize,
}

/// Identity key for objects that can appear in the print stack —
/// containers by `eq' identity (Rc pointer), like GNU's BASE_EQ.
pub(super) fn print_stack_key(v: &Value) -> Option<usize> {
    match v {
        Value::Cons(r) => Some(std::rc::Rc::as_ptr(r) as usize),
        Value::Str(r) => Some(std::rc::Rc::as_ptr(r) as usize),
        Value::Vec(r) => Some(std::rc::Rc::as_ptr(r) as usize),
        Value::Record(r) => Some(std::rc::Rc::as_ptr(r) as usize),
        Value::Hash(r) => Some(std::rc::Rc::as_ptr(r) as usize),
        // Lambdas print their `[ARGS BODY ENV ...]' slots — a closure
        // whose env captures a `let' binding referencing the closure
        // itself (gv-ref's setter: `(ref . (getter . SETTER))')
        // recurses forever without the being-printed check.
        Value::Lambda(r) => Some(std::rc::Rc::as_ptr(r) as usize),
        _ => None,
    }
}

/// `print-circle' key for an uninterned symbol.  GNU print.c numbers
/// every non-atom reachable object in `print_preprocess' — including
/// symbols when `print-gensym' is on — so a shared `#:test' prints
/// `#1=#:test' … `#1#'.  SymIds live in a different namespace than
/// heap pointers: fold them into key space by setting the high bit.
pub(super) fn gensym_circle_key(i: &Interp, v: &Value) -> Option<usize> {
    // Cheap shape check first — runs for every printed symbol.
    let id = i.sym_id(v)?;
    if !i.obarray.symbol(id).uninterned || !i.print_gensym() {
        return None;
    }
    Some(id as usize | (1usize << (usize::BITS - 1)))
}

pub(super) fn print_identity_key(i: &Interp, value: &Value) -> Option<usize> {
    if circle_active() {
        gensym_circle_key(i, value).or_else(|| print_stack_key(value))
    } else if crate::lisp::builtins::misc::raw_symbol_position(value).is_some() {
        print_stack_key(value)
    } else {
        print_stack_key(value).or_else(|| gensym_circle_key(i, value))
    }
}

/// Position symbols use their bare symbol's identity when the dynamic
/// symbol view is enabled. When disabled they are opaque records and do
/// not participate in circle numbering.
pub(super) fn print_object_key(i: &Interp, v: &Value) -> Option<usize> {
    if crate::lisp::builtins::misc::any_sympos_parts(i, v).is_some() {
        return crate::lisp::builtins::misc::active_sympos_parts(i, v)
            .and_then(|(sym, _)| gensym_circle_key(i, &Value::Sym(sym)));
    }
    print_stack_key(v).or_else(|| gensym_circle_key(i, v))
}

/// GNU `print_circle_candidate_p' (print.c): only these object kinds
/// participate in `print-circle' numbering — notably EMPTY strings and
/// EMPTY vectors are excluded, which keeps literal `""'s from
/// sprouting `#N=' labels.
pub(super) fn print_circle_candidate(i: &Interp, v: &Value) -> bool {
    if crate::lisp::builtins::misc::any_sympos_parts(i, v).is_some() {
        return print_object_key(i, v).is_some();
    }
    match v {
        Value::Cons(_) => true,
        Value::Str(s) => !s.borrow().is_empty(),
        Value::Sym(_) => gensym_circle_key(i, v).is_some(),
        Value::Vec(r) => !r.borrow().is_empty(),
        Value::Record(_) if crate::lisp::builtins::misc::raw_symbol_position(v).is_some() => {
            gensym_circle_key(i, v).is_some()
        }
        Value::Record(_) | Value::Hash(_) | Value::Lambda(_) => true,
        _ => false,
    }
}

/// GNU's `print_preprocess' pass: depth-first walk of the object
/// graph (car before cdr, like GNU's recursion), numbering each
/// shared/cyclic object when it is encountered a second time.
/// Already-seen objects are not descended into, so cycles terminate.
pub(super) fn count_print_refs(i: &Interp, v: &Value, ctx: &mut CircleCtx) {
    let mut stack = vec![v.clone()];
    while let Some(x) = stack.pop() {
        // GNU's print_circle_candidate_p gate: empty strings/vectors
        // never participate in print-circle numbering.
        if !print_circle_candidate(i, &x) {
            continue;
        }
        let Some(k) = gensym_circle_key(i, &x).or_else(|| print_stack_key(&x)) else {
            continue;
        };
        if !ctx.seen.insert(k) {
            // Second encounter: GNU assigns `print_number_index'
            // here, not at print time.
            if !ctx.labels.contains_key(&k) {
                ctx.labels.insert(k, ctx.next);
                ctx.next += 1;
            }
            continue;
        }
        match &x {
            Value::Cons(c) => {
                let (car, cdr) = {
                    let b = c.borrow();
                    (b.car.clone(), b.cdr.clone())
                };
                stack.push(cdr);
                stack.push(car);
            }
            Value::Vec(r) | Value::Record(r) => {
                stack.extend(r.borrow().iter().rev().cloned());
            }
            Value::Hash(h) => {
                let hh = h.borrow();
                for (_, orig) in hh.keys.iter() {
                    stack.push(orig.clone());
                }
                stack.extend(hh.map.values().cloned());
            }
            _ => {}
        }
    }
}

/// Is `print-circle' handling active for the current print?
pub(super) fn circle_active() -> bool {
    CIRCLE.with(|c| c.borrow().is_some())
}

/// In `print-circle' mode: if K's object was numbered by the
/// pre-scan, return Some((label, true-if-first-occurrence)).
/// None for singly referenced objects.
pub(super) fn circle_label(k: usize) -> Option<(usize, bool)> {
    CIRCLE.with(|c| {
        let mut binding = c.borrow_mut();
        let ctx = binding.as_mut()?;
        let n = ctx.labels.get(&k).copied()?;
        Some((n, ctx.printed.insert(k)))
    })
}

/// Does K's object need a `print-circle' label (shared/cyclic)?
pub(super) fn circle_counted(k: usize) -> bool {
    CIRCLE.with(|c| {
        c.borrow()
            .as_ref()
            .map(|ctx| ctx.labels.contains_key(&k))
            .unwrap_or(false)
    })
}
thread_local! {
    /// GNU print.c's `print_number_index': persists across print
    /// calls while `print-continuous-numbering' is non-nil.
    pub(super) static PRINT_NUMBER_INDEX: std::cell::Cell<i64> =
        const { std::cell::Cell::new(0) };
}

/// `print-number-table' outcome for a heap object under print-circle.
pub(super) enum NtSubst {
    /// Emit this text and print the object's contents after it
    /// (GNU's negative-index `#N=' prefix).
    Prefix(String),
    /// Emit this text INSTEAD of the object (a `#N#' back-reference
    /// or a verbatim substitution string).
    Replace(EmacsText),
}

/// `print-number-table' substitution for a heap object (GNU's
/// `Fgethash' in `print_object' under `print-circle'): a string
/// prints verbatim, a negative index prints `#N=' then flips to its
/// positive, a positive index prints `#N#' and consumes the object.
/// The caller uses Some(_) to suppress the ctx labels too — the
/// table is authoritative for objects it contains.
pub(super) fn number_table_subst(i: &Interp, v: &Value) -> Option<NtSubst> {
    if !i.print_var("print-circle").truthy() {
        return None;
    }
    // Without `print-continuous-numbering' GNU resets the table at
    // every top-level print — a user-bound table is ignored.
    if !i.print_var("print-continuous-numbering").truthy() {
        return None;
    }
    let nt = i.print_var("print-number-table");
    let Value::Hash(h) = nt else {
        return None;
    };
    let key = crate::lisp::builtins::hashfn::hash_key_for(i, v, crate::lisp::value::HashTest::Eq);
    let found = h.borrow().map.get(&key).cloned()?;
    match &found {
        Value::Str(s) => Some(NtSubst::Replace(s.borrow().clone())),
        Value::Int(n) => {
            let n = *n;
            if n < 0 {
                // First print of a labeled object: emit the prefix
                // and mark it printed (GNU stores the positive).
                h.borrow_mut().map.insert(key, Value::Int(-n));
                Some(NtSubst::Prefix(format!("#{}=", -n)))
            } else {
                Some(NtSubst::Replace(format!("#{n}#").into()))
            }
        }
        _ => None,
    }
}

impl Interp {
    /// GNU print's direct C assignment resets numbering without notifying
    /// Lisp variable watchers, including when print-circle is nil.
    pub(super) fn reset_print_numbering(&mut self) {
        if self.print_var("print-continuous-numbering").truthy()
            && !self.print_var("print-number-table").is_nil()
        {
            return;
        }
        PRINT_NUMBER_INDEX.with(|index| index.set(0));
        let symbol = self.q("print-number-table");
        if let Some(buffer) = self.current_buffer_ref() {
            if let Some(value) = buffer.borrow_mut().locals.get_mut(&symbol) {
                *value = Value::Nil;
                return;
            }
        }
        self.obarray.symbol_mut(symbol).value = Value::Nil;
    }

    /// When `print-circle' is non-nil, pre-scan V for shared/cyclic
    /// objects and install the per-print state.  Returns the previous
    /// state so callers can restore it (nested print calls).
    pub(super) fn install_circle_ctx(&self, v: &Value) -> Option<CircleCtx> {
        if !self.print_var("print-circle").truthy() {
            return None;
        }
        let mut ctx = CircleCtx {
            seen: rustc_hash::FxHashSet::default(),
            labels: rustc_hash::FxHashMap::default(),
            printed: rustc_hash::FxHashSet::default(),
            next: 1,
        };
        count_print_refs(self, v, &mut ctx);
        CIRCLE.with(|c| c.borrow_mut().replace(ctx))
    }
}

/// A user-supplied print-number-table can use a custom test. Route its
/// writes through puthash so callbacks and their mutation guard apply.
fn preprocess_put(
    i: &mut Interp,
    h: &crate::lisp::value::HashRef,
    key: crate::lisp::value::HashKey,
    original: &Value,
    value: Value,
) -> Result<(), crate::lisp::error::Flow> {
    if h.borrow().user_test.is_some() {
        crate::lisp::builtins::hashfn::f_puthash(
            i,
            vec![original.clone(), value, Value::Hash(h.clone())].into(),
        )?;
    } else {
        h.borrow_mut().map.insert(key, value);
    }
    Ok(())
}

/// GNU print.c `Fprint_preprocess': when `print-circle' is non-nil,
/// fill `print-number-table' — each `print-circle-candidate' gets t
/// at its first encounter and `-N' (bumping `print_number_index') at
/// the second; the `t' singles are left in (the top-level `print'
/// removes them for its own pass).  No-op when print-circle is nil.
pub fn f_print_preprocess(i: &mut Interp, a: crate::lisp::Argv) -> crate::lisp::error::EvalResult {
    if !i.print_var("print-circle").truthy() {
        return Ok(Value::Nil);
    }
    PRINT_NUMBER_INDEX.with(|c| c.set(0));
    let nt_id = i.q("print-number-table");
    let mut nt = i.symbol_value(nt_id);
    if !matches!(nt, Value::Hash(_)) {
        nt = Value::Hash(crate::lisp::memory::allocate(
            crate::lisp::value::LispHash::new(crate::lisp::value::HashTest::Eq),
            crate::lisp::Value::Hash,
        ))
        .tracked();
        let _ = i.set_symbol(nt_id, nt.clone());
    }
    let Value::Hash(h) = nt else {
        return Ok(Value::Nil);
    };
    let mut stack = vec![a[0].clone()];
    while let Some(x) = stack.pop() {
        if !print_circle_candidate(i, &x) {
            continue;
        }
        let custom = h.borrow().user_test.is_some();
        let key = if custom {
            crate::lisp::builtins::hashfn::table_key(i, &h, &x)?
        } else {
            crate::lisp::builtins::hashfn::hash_key_for(i, &x, crate::lisp::value::HashTest::Eq)
        };
        let cur = h.borrow().map.get(&key).cloned();
        match cur {
            Some(Value::Sym(s)) if s == crate::lisp::sym::T => {
                let n = PRINT_NUMBER_INDEX.with(|c| {
                    c.set(c.get() + 1);
                    c.get()
                });
                preprocess_put(i, &h, key, &x, Value::Int(-(n as i128)))?;
            }
            // Under `print-continuous-numbering' an uninterned symbol
            // always gets a number (GNU's byte-compile-output-docform
            // special).
            None => {
                let gensym = crate::lisp::builtins::misc::active_sympos_parts(i, &x)
                    .map(|(sym, _)| gensym_circle_key(i, &Value::Sym(sym)).is_some())
                    .unwrap_or_else(|| gensym_circle_key(i, &x).is_some());
                if gensym && i.print_var("print-continuous-numbering").truthy() {
                    let n = PRINT_NUMBER_INDEX.with(|c| {
                        c.set(c.get() + 1);
                        c.get()
                    });
                    preprocess_put(i, &h, key, &x, Value::Int(-(n as i128)))?;
                } else {
                    preprocess_put(i, &h, key, &x, Value::Sym(crate::lisp::sym::T))?;
                }
                if crate::lisp::builtins::misc::any_sympos_parts(i, &x).is_some() {
                    continue;
                }
                match &x {
                    Value::Cons(c) => {
                        let (car, cdr) = {
                            let b = c.borrow();
                            (b.car.clone(), b.cdr.clone())
                        };
                        stack.push(cdr);
                        stack.push(car);
                    }
                    Value::Vec(r) | Value::Record(r) => {
                        stack.extend(r.borrow().iter().rev().cloned());
                    }
                    Value::Hash(hh) => {
                        let h2 = hh.borrow();
                        for (_, orig) in h2.keys.iter() {
                            stack.push(orig.clone());
                        }
                        stack.extend(h2.map.values().cloned());
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }
    Ok(Value::Nil)
}
