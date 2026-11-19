//! Per-print identity/numbering state and GNU print-number-table preprocessing.

use crate::lisp::{
    Interp,
    error::{EvalResult, Flow},
    print_text::PrintText,
    text::EmacsText,
    value::{HashKey, Value},
};
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
}

/// Per-print state for `print-circle'.
pub(super) struct CircleCtx {
    /// Objects already seen by the pre-scan.
    pub(super) seen: rustc_hash::FxHashSet<usize>,
    /// ptr -> label number, assigned at second encounter.
    pub(super) labels: rustc_hash::FxHashMap<usize, usize>,
    /// Objects that already printed a pointer-derived circle label.
    pub(super) printed: rustc_hash::FxHashSet<usize>,
    /// Canonical `print-number-table' entries for each object identity. A
    /// custom hash test can make distinct objects share an entry, while
    /// distinct entries may carry the same numeric label.
    pub(super) entry_keys: rustc_hash::FxHashMap<usize, HashKey>,
    /// Number-table entries whose number has already been consumed.
    pub(super) printed_entries: rustc_hash::FxHashSet<HashKey>,
    /// Precomputed string substitutions from a user-defined
    /// `print-number-table' test.  The renderer stays callback-free.
    pub(super) substitutions: rustc_hash::FxHashMap<usize, NtSubst>,
    /// First occurrence order, used to commit number-table values after
    /// rendering without changing the pure renderer's borrowing contract.
    pub(super) printed_order: Vec<(usize, usize, Option<HashKey>)>,
    /// Next label index (GNU's print_number_index starts at 1).
    pub(super) next: usize,
}

/// Print-number-table state prepared before entering the pure renderer.
#[derive(Default)]
pub(super) struct PreparedCircle {
    pub(super) labels: rustc_hash::FxHashMap<usize, usize>,
    pub(super) entry_keys: rustc_hash::FxHashMap<usize, HashKey>,
    pub(super) printed_entries: rustc_hash::FxHashSet<HashKey>,
    pub(super) substitutions: rustc_hash::FxHashMap<usize, NtSubst>,
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
    if circle_active() && i.print_var("print-circle").truthy() {
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
        Value::Str(s) => s.try_borrow().is_ok_and(|s| !s.is_empty()),
        Value::Sym(_) => gensym_circle_key(i, v).is_some(),
        Value::Vec(_) if i.is_overlay(v) => false,
        Value::Vec(r) => r.try_borrow().is_ok_and(|r| !r.is_empty()),
        Value::Record(_) if crate::lisp::builtins::misc::raw_symbol_position(v).is_some() => {
            gensym_circle_key(i, v).is_some()
        }
        // Remacs stores these GNU pseudovectors in the Record variant, but
        // GNU's print_circle_candidate_p only accepts RECORDP among the
        // remaining vectorlike types.  Bool vectors, obarrays, terminals,
        // window configurations, and treesit pseudovectors are unreadable
        // objects rather than Lisp records and must not consume #N labels.
        Value::Record(_)
            if matches!(
                v.record_kind(),
                Some(
                    crate::lisp::value::RecordKind::BoolVector
                        | crate::lisp::value::RecordKind::Obarray
                )
            ) =>
        {
            false
        }
        Value::Record(_) if crate::editor::window_configuration_object(v) => false,
        Value::Record(_) if crate::editor::is_terminal(i, v) => false,
        Value::Record(items)
            if items.try_borrow().is_ok_and(|items| {
                crate::lisp::builtins::treesit::treesit_repr(i, &items).is_some()
            }) =>
        {
            false
        }
        Value::Record(_) | Value::Hash(_) | Value::Lambda(_) => true,
        _ => false,
    }
}

/// Push direct Lisp objects the renderer visits for `v', in reverse print
/// order. The circle and number-table preprocessors must walk the same graph
/// as the renderer or cycles inside closures receive no label.
fn push_print_children(i: &Interp, value: &Value, stack: &mut Vec<Value>) {
    match value {
        Value::Cons(cell) => {
            let (car, cdr) = {
                let Ok(cell) = cell.try_borrow() else {
                    return;
                };
                (cell.car.clone(), cell.cdr.clone())
            };
            stack.push(cdr);
            stack.push(car);
        }
        Value::Str(string) => {
            // String properties are part of the printed graph even when the
            // printer later hides some of them.
            for (_, _, plist) in i.str_props(string).iter().rev() {
                stack.extend(plist.iter().rev().cloned());
            }
        }
        Value::Vec(items) | Value::Record(items) => {
            if let Ok(items) = items.try_borrow() {
                stack.extend(items.iter().rev().cloned());
            }
        }
        Value::Hash(hash) => {
            let Ok(hash) = hash.try_borrow() else {
                return;
            };
            // `print_hash' visits each stored key then its value in key order.
            let entries = hash.keys.iter().collect::<Vec<_>>();
            for (key, original) in entries.into_iter().rev() {
                if let Some(value) = hash.map.get(key) {
                    stack.push(value.clone());
                    stack.push(original.clone());
                }
            }
        }
        Value::Lambda(lambda) => {
            // Named compiled functions print as unreadable subr names without
            // visiting their slots.
            if lambda.compiled && i.lambda_name(lambda).is_some() {
                return;
            }
            if let Some(items) = &lambda.bc_items {
                if let Ok(items) = items.try_borrow() {
                    stack.extend(items.iter().rev().cloned());
                }
            } else {
                // Interpreted functions print arglist, body, then lexical
                // environment. The body list itself is freshly assembled by
                // the renderer, so only its existing forms enter the graph.
                stack.push(crate::lisp::eval::lexenv_as_value(&lambda.env));
                if let Some(body) = &lambda.body_value {
                    stack.push(body.clone());
                } else {
                    stack.extend(lambda.body.iter().rev().cloned());
                }
                if let Some(arglist) = &lambda.arglist {
                    stack.push(arglist.clone());
                }
            }
        }
        _ => {}
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
        push_print_children(i, &x, &mut stack);
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
        let entry_key = ctx.entry_keys.get(&k).cloned();
        let first = if let Some(key) = &entry_key {
            ctx.printed_entries.insert(key.clone())
        } else {
            ctx.printed.insert(k)
        };
        if first {
            ctx.printed_order.push((k, n, entry_key));
        }
        Some((n, first))
    })
}

pub(super) fn printed_circle_labels() -> Vec<(usize, usize, Option<HashKey>)> {
    CIRCLE.with(|c| {
        c.borrow()
            .as_ref()
            .map(|ctx| ctx.printed_order.clone())
            .unwrap_or_default()
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
#[derive(Clone)]
pub(super) enum NtSubst {
    /// Emit this text and print the object's contents after it
    /// (GNU's negative-index `#N=' prefix).
    Prefix(String, Option<(usize, usize, Option<HashKey>)>),
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
pub(super) fn number_table_subst(
    i: &Interp,
    v: &Value,
    identity: usize,
    output: &crate::lisp::print_text::PrintText,
) -> Result<Option<NtSubst>, Flow> {
    if !i.print_var("print-circle").truthy() {
        return Ok(None);
    }
    // GNU consults print-number-table only for print-circle candidates.
    // In particular, a preexisting table entry cannot replace an empty
    // string or vector, and a custom hash test must not see either object.
    if !print_circle_candidate(i, v) {
        return Ok(None);
    }
    // Without `print-continuous-numbering' GNU resets the table at
    // every top-level print — a user-bound table is ignored.
    if !i.print_var("print-continuous-numbering").truthy() {
        return Ok(None);
    }
    let nt = i.print_var("print-number-table");
    let Value::Hash(h) = nt else {
        return Ok(None);
    };
    let (test, has_user_test) = {
        let table = h
            .try_borrow()
            .map_err(|_| i.error("Cannot inspect a mutably borrowed hash table"))?;
        (table.test, table.user_test.is_some())
    };
    if has_user_test {
        // User hash tests need mutable Lisp callbacks, so preprocessing
        // resolves their entries and the pure renderer consumes this
        // per-object replacement map.
        let Some(identity) = print_identity_key(i, v) else {
            return Ok(None);
        };
        return Ok(CIRCLE.with(|circle| {
            circle
                .borrow()
                .as_ref()?
                .substitutions
                .get(&identity)
                .cloned()
        }));
    }
    let key = crate::lisp::builtins::hashfn::hash_key_for_checked(i, v, test)?;
    let found = h
        .try_borrow()
        .map_err(|_| i.error("Cannot inspect a mutably borrowed hash table"))?
        .map
        .get(&key)
        .cloned();
    let Some(found) = found else {
        return Ok(None);
    };
    let substitution = match &found {
        Value::Str(s) => Some(NtSubst::Replace(
            s.try_borrow()
                .map_err(|_| i.error("Cannot inspect a mutably borrowed string"))?
                .clone(),
        )),
        Value::Int(n) => {
            let n = *n;
            if n < 0 {
                let Some(number) = n.checked_neg().and_then(|n| usize::try_from(n).ok()) else {
                    return Ok(None);
                };
                if output.previously_numbered(&key).is_some() {
                    Some(NtSubst::Replace(format!("#{number}#").into()))
                } else {
                    // The table update follows the complete prefix write. A
                    // Lisp function stream may signal while accepting one of
                    // its characters, in which case GNU leaves the entry -N.
                    Some(NtSubst::Prefix(
                        format!("#{number}="),
                        Some((identity, number, Some(key))),
                    ))
                }
            } else {
                Some(NtSubst::Replace(format!("#{n}#").into()))
            }
        }
        _ => None,
    };
    Ok(substitution)
}

/// Whether LIST's cdr must be rendered as a dotted object because either
/// the pointer-based circle context or `print-number-table' has assigned it
/// a label. Continuous numbering with the built-in table intentionally
/// disables `CIRCLE' to avoid duplicate prefixes, but list traversal still
/// needs to know when flattening a shared cdr would move its `#N=' past the
/// first occurrence.
pub(super) fn print_circle_tail_is_labeled(
    i: &Interp,
    value: &Value,
    output: &PrintText,
) -> Result<bool, Flow> {
    let Some(key) = print_stack_key(value) else {
        return Ok(false);
    };
    if circle_counted(key) {
        return Ok(true);
    }
    Ok(number_table_subst(i, value, key, output)?.is_some())
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
            if let Ok(mut buffer) = buffer.try_borrow_mut() {
                if let Some(value) = buffer.locals.get_mut(&symbol) {
                    *value = Value::Nil;
                    return;
                }
            }
        }
        self.obarray.symbol_mut(symbol).value = Value::Nil;
    }

    /// When `print-circle' is non-nil, pre-scan V for shared/cyclic
    /// objects and install the per-print state.  Returns the previous
    /// state so callers can restore it (nested print calls).
    pub(super) fn install_circle_ctx(
        &self,
        v: &Value,
        prepared: Option<PreparedCircle>,
    ) -> Option<CircleCtx> {
        if !self.print_var("print-circle").truthy() {
            return CIRCLE.with(|circle| circle.borrow_mut().take());
        }
        // Continuous numbering is driven by `print-number-table' across
        // top-level prints; using the per-print pointer table as well would
        // emit duplicate #N= prefixes.
        if self.print_var("print-continuous-numbering").truthy()
            && matches!(self.print_var("print-number-table"), Value::Hash(h) if h.borrow().user_test.is_none())
        {
            return CIRCLE.with(|circle| circle.borrow_mut().take());
        }
        let user_table = self.print_var("print-continuous-numbering").truthy()
            && matches!(
                self.print_var("print-number-table"),
                Value::Hash(h) if h.borrow().user_test.is_some()
            );
        let mut ctx = CircleCtx {
            seen: rustc_hash::FxHashSet::default(),
            labels: rustc_hash::FxHashMap::default(),
            printed: rustc_hash::FxHashSet::default(),
            entry_keys: rustc_hash::FxHashMap::default(),
            printed_entries: rustc_hash::FxHashSet::default(),
            substitutions: rustc_hash::FxHashMap::default(),
            printed_order: Vec::new(),
            next: 1,
        };
        if user_table {
            if let Some(prepared) = prepared {
                ctx.labels = prepared.labels;
                ctx.entry_keys = prepared.entry_keys;
                ctx.printed_entries = prepared.printed_entries;
                ctx.substitutions = prepared.substitutions;
            } else {
                count_print_refs(self, v, &mut ctx);
            }
        } else {
            count_print_refs(self, v, &mut ctx);
        }
        CIRCLE.with(|c| c.borrow_mut().replace(ctx))
    }
}

/// Route print-number-table writes through `puthash' so key iteration,
/// custom tests, and the table's mutation guard stay consistent.
fn preprocess_put(
    i: &mut Interp,
    h: &crate::lisp::value::HashRef,
    original: &Value,
    value: Value,
) -> Result<(), crate::lisp::error::Flow> {
    crate::lisp::builtins::hashfn::f_puthash(
        i,
        vec![original.clone(), value, Value::Hash(h.clone())].into(),
    )?;
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
    let _ = print_preprocess_object(i, &a[0])?;
    Ok(Value::Nil)
}

/// The top-level print path's `print_preprocess' pass. Unlike the Lisp
/// `print--preprocess' subr, this retains the continuous numbering index and
/// removes single-occurrence entries after the scan, as GNU `print' does.
pub(crate) fn preprocess_for_print(
    i: &mut Interp,
    object: &Value,
) -> Result<Option<PreparedCircle>, Flow> {
    if !i.print_var("print-circle").truthy() {
        return Ok(None);
    }
    let object_keys = print_preprocess_object(i, object)?;
    let table_id = i.q("print-number-table");
    let table = i.symbol_value(table_id);
    if let Value::Hash(hash) = table {
        let singles = {
            let table = hash.borrow();
            table
                .keys
                .iter()
                .filter_map(|(key, original)| {
                    matches!(table.map.get(key), Some(Value::Sym(s)) if *s == crate::lisp::sym::T)
                        .then(|| original.clone())
                })
                .collect::<Vec<_>>()
        };
        for key in singles {
            crate::lisp::builtins::hashfn::f_remhash(
                i,
                vec![key, Value::Hash(hash.clone())].into(),
            )?;
        }
        let uses_custom_test =
            i.print_var("print-continuous-numbering").truthy() && hash.borrow().user_test.is_some();
        if uses_custom_test {
            let table = hash.borrow();
            let mut prepared = PreparedCircle::default();
            for (identity, key) in object_keys {
                prepared.entry_keys.insert(identity, key.clone());
                match table.map.get(&key) {
                    Some(Value::Int(number)) if *number < 0 => {
                        prepared.labels.insert(identity, (-*number) as usize);
                    }
                    Some(Value::Int(number)) if *number > 0 => {
                        prepared.labels.insert(identity, *number as usize);
                        prepared.printed_entries.insert(key.clone());
                    }
                    Some(Value::Int(0)) => {
                        prepared
                            .substitutions
                            .insert(identity, NtSubst::Replace("#0#".into()));
                    }
                    Some(Value::Str(string)) => {
                        prepared
                            .substitutions
                            .insert(identity, NtSubst::Replace(string.borrow().clone()));
                    }
                    _ => {}
                }
            }
            return Ok(Some(prepared));
        }
    }
    Ok(None)
}

/// GNU changes a numbered table entry from `-N' to `N' on the first actual
/// occurrence during printing. The renderer records that order in its circle
/// context and commits it here, preserving custom table callbacks.
pub(super) fn finish_print_number_table(
    i: &mut Interp,
    labels: &[(usize, usize, Option<HashKey>)],
) -> EvalResult {
    if labels.is_empty() {
        return Ok(Value::Nil);
    }
    let table_id = i.q("print-number-table");
    let Value::Hash(table) = i.symbol_value(table_id) else {
        return Ok(Value::Nil);
    };
    let entries = {
        let table_ref = table.borrow();
        table_ref
            .keys
            .iter()
            .filter_map(|(key, original)| {
                table_ref
                    .map
                    .get(key)
                    .cloned()
                    .map(|value| (key.clone(), original.clone(), value))
            })
            .collect::<Vec<_>>()
    };
    let mut updates = Vec::new();
    for (_identity, number, entry_key) in labels {
        if let Some((_, original, _)) = entries.iter().find(|(key, _original, value)| {
            entry_key.as_ref().is_none_or(|entry_key| key == entry_key)
                && matches!(value, Value::Int(index) if *index == -(*number as i128))
        }) {
            updates.push((original.clone(), Value::Int(*number as i128)));
        }
    }
    for (key, value) in updates {
        crate::lisp::builtins::hashfn::f_puthash(
            i,
            vec![key, value, Value::Hash(table.clone())].into(),
        )?;
    }
    Ok(Value::Nil)
}

/// Shared diagnostic printers cannot invoke Lisp hash-table callbacks. Keep
/// their built-in hash-table side effects in sync with the mutable printer.
pub(super) fn finish_print_number_table_shared(
    i: &Interp,
    labels: &[(usize, usize, Option<HashKey>)],
) {
    let Some(table_id) = i.q_soft("print-number-table") else {
        return;
    };
    let Value::Hash(table) = i.symbol_value(table_id) else {
        return;
    };
    if table.borrow().user_test.is_some() {
        return;
    }
    for (_, number, entry_key) in labels {
        let mut table = table.borrow_mut();
        let expected = -(*number as i128);
        let key = entry_key.clone().or_else(|| {
            table.map.iter().find_map(|(key, value)| {
                matches!(value, Value::Int(current) if *current == expected).then(|| key.clone())
            })
        });
        if let Some(key) = key
            && matches!(table.map.get(&key), Some(Value::Int(current)) if *current == expected)
        {
            table.map.insert(key, Value::Int(*number as i128));
        }
    }
}

fn print_preprocess_object(
    i: &mut Interp,
    object: &Value,
) -> Result<rustc_hash::FxHashMap<usize, HashKey>, Flow> {
    let nt_id = i.q("print-number-table");
    let mut nt = i.symbol_value(nt_id);
    if !matches!(nt, Value::Hash(_)) {
        nt = Value::Hash(crate::lisp::memory::allocate(
            crate::lisp::value::LispHash::new(crate::lisp::value::HashTest::Eq),
            crate::lisp::Value::Hash,
        ))
        .tracked();
        i.set_symbol_no_watchers(nt_id, nt.clone())?;
    }
    let Value::Hash(h) = nt else {
        return Ok(rustc_hash::FxHashMap::default());
    };
    let mut stack = vec![object.clone()];
    let mut object_keys = rustc_hash::FxHashMap::default();
    while let Some(x) = stack.pop() {
        if !print_circle_candidate(i, &x) {
            continue;
        }
        let identity = print_identity_key(i, &x);
        let key = if identity.is_some() {
            let (test, custom) = {
                let table = h.borrow();
                (table.test, table.user_test.is_some())
            };
            Some(if custom {
                crate::lisp::builtins::hashfn::table_key(i, &h, &x)?
            } else {
                crate::lisp::builtins::hashfn::hash_key_for_checked(i, &x, test)?
            })
        } else {
            None
        };
        if let (Some(identity), Some(key)) = (identity, key.as_ref()) {
            object_keys.insert(identity, key.clone());
        }
        let Some(key) = key else {
            continue;
        };
        let cur = h.borrow().map.get(&key).cloned();
        match cur {
            // GNU treats any symbol-valued entry as the first-visit marker
            // (normally t), then replaces it with a negative label.
            Some(Value::Sym(_)) => {
                let n = PRINT_NUMBER_INDEX.with(|c| {
                    c.set(c.get() + 1);
                    c.get()
                });
                preprocess_put(i, &h, &x, Value::Int(-(n as i128)))?;
            }
            // Under `print-continuous-numbering' an uninterned symbol
            // always gets a number (GNU's byte-compile-output-docform
            // special).
            None | Some(Value::Nil) => {
                let gensym = crate::lisp::builtins::misc::active_sympos_parts(i, &x)
                    .map(|(sym, _)| gensym_circle_key(i, &Value::Sym(sym)).is_some())
                    .unwrap_or_else(|| gensym_circle_key(i, &x).is_some());
                if gensym && i.print_var("print-continuous-numbering").truthy() {
                    let n = PRINT_NUMBER_INDEX.with(|c| {
                        c.set(c.get() + 1);
                        c.get()
                    });
                    preprocess_put(i, &h, &x, Value::Int(-(n as i128)))?;
                } else {
                    preprocess_put(i, &h, &x, Value::Sym(crate::lisp::sym::T))?;
                }
                if crate::lisp::builtins::misc::any_sympos_parts(i, &x).is_some() {
                    continue;
                }
                push_print_children(i, &x, &mut stack);
            }
            _ => {}
        }
    }
    Ok(object_keys)
}

#[cfg(test)]
mod custom_hash_print_tests {
    use crate::lisp::{Interp, Value};

    #[test]
    fn print_circle_labels_a_self_referential_closure_environment() {
        std::thread::Builder::new()
            .stack_size(128 * 1024 * 1024)
            .spawn(|| {
                let mut interp = Interp::new();
                let result = interp
                    .eval_str(
                        "(let ((f (eval '(let ((f nil)) (setq f (lambda () f)) f) t))\
                               (print-circle t)\
                               (print-level nil)\
                               (print-length nil))\
                           (prin1-to-string f))",
                    )
                    .unwrap();
                let Value::Str(output) = result else {
                    panic!("expected a string from prin1-to-string")
                };
                assert_eq!(
                    output.borrow().to_unicode_lossy(),
                    "#1=#[nil (f) ((f . #1#))]"
                );
            })
            .unwrap()
            .join()
            .unwrap();
    }

    #[test]
    fn print_circle_preserves_self_references_in_string_properties() {
        std::thread::Builder::new()
            .stack_size(128 * 1024 * 1024)
            .spawn(|| {
                let mut interp = Interp::new();
                let result = interp
                    .eval_str(
                        "(let ((string (copy-sequence \"x\")))\
                           (put-text-property 0 1 'loop string string)\
                           (let ((print-circle t))\
                             (prin1-to-string string)))",
                    )
                    .unwrap();
                let Value::Str(output) = result else {
                    panic!("expected a string from prin1-to-string")
                };
                assert_eq!(
                    output.borrow().to_unicode_lossy(),
                    "#1=#(\"x\" 0 1 (loop #1#))"
                );
            })
            .unwrap()
            .join()
            .unwrap();
    }

    #[test]
    fn continuous_numbering_uses_the_custom_table_test_for_equal_objects() {
        std::thread::Builder::new()
            .stack_size(64 * 1024 * 1024)
            .spawn(|| {
                let mut interp = Interp::new();
                let result = interp
                    .eval_str(
                        "(progn\
                           (define-hash-table-test 'qa-print-custom-equal\
                                                   #'equal #'sxhash-equal)\
                           (let ((print-circle t)\
                                 (print-continuous-numbering t)\
                                 (print-number-table\
                                  (make-hash-table :test 'qa-print-custom-equal))\
                                 (a (copy-sequence \"x\"))\
                                 (b (copy-sequence \"x\")))\
                             (let ((first (prin1-to-string (vector a b)))\
                                   (second (prin1-to-string (vector a b))))\
                               (list first second\
                                     (hash-table-count print-number-table)\
                                     (gethash a print-number-table)))))",
                    )
                    .unwrap();
                assert_eq!(
                    interp.prin1_to_string(&result),
                    "(\"[#1=\\\"x\\\" #1#]\" \"[#1# #1#]\" 1 1)"
                );
            })
            .unwrap()
            .join()
            .unwrap();
    }

    #[test]
    fn hash_key_and_value_references_use_print_order_for_circle_labels() {
        std::thread::Builder::new()
            .stack_size(64 * 1024 * 1024)
            .spawn(|| {
                let mut interp = Interp::new();
                interp
                    .eval_str("(let ((print-circle t)) (print--preprocess nil))")
                    .unwrap();
                let result = interp
                    .eval_str(
                        "(let* ((a (list 'a))\
                                (b (list 'b))\
                                (table (make-hash-table :test 'eq)))\
                           (puthash a a table)\
                           (puthash b b table)\
                           (list\
                            (let ((print-circle t))\
                              (prin1-to-string table))\
                            (progn\
                              (let ((print-circle t))\
                                (print--preprocess nil))\
                              (let ((print-circle t)\
                                    (print-continuous-numbering t)\
                                    (print-number-table (make-hash-table :test 'eq)))\
                                (prin1-to-string table)))))",
                    )
                    .unwrap();
                assert_eq!(
                    interp.prin1_to_string(&result),
                    "(\"#s(hash-table test eq data (#1=(a) #1# #2=(b) #2#))\" \
                     \"#s(hash-table test eq data (#1=(a) #1# #2=(b) #2#))\")"
                );
            })
            .unwrap()
            .join()
            .unwrap();
    }
}
