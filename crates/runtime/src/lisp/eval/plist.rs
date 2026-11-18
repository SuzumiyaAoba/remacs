//! Property-list access and mutation shared by evaluator and builtins.

use crate::lisp::value::{SymId, Value};
use crate::lisp::{Interp, error::Flow, obarray::sym};
use std::rc::Rc;

fn same_cons(left: &Value, right: &Value) -> bool {
    matches!((left, right), (Value::Cons(a), Value::Cons(b)) if Rc::ptr_eq(a, b))
}

/// Constant-space Brent detector over plist key cells. Advance after moving
/// from one key/value pair to the next pair.
struct PlistCycle {
    anchor: Value,
    power: usize,
    steps: usize,
}

impl PlistCycle {
    fn new(plist: &Value) -> Self {
        Self {
            anchor: plist.clone(),
            power: 1,
            steps: 0,
        }
    }

    fn advance(&mut self, tail: &Value) -> bool {
        self.steps = self.steps.saturating_add(1);
        if same_cons(tail, &self.anchor) {
            return true;
        }
        if self.steps == self.power {
            self.anchor = tail.clone();
            self.power = self.power.saturating_mul(2);
            self.steps = 0;
        }
        false
    }
}

/// Search a plist pair-by-pair without an arbitrary length limit. Keep
/// malformed/circular lists finite while allowing valid plists of any size.
#[derive(Clone, Copy)]
enum PlistReadError {
    Borrowed,
}

fn plist_find(
    plist: &Value,
    mut matches: impl FnMut(&Value) -> bool,
) -> Result<Option<Value>, PlistReadError> {
    let mut cur = plist.clone();
    let mut cycle = PlistCycle::new(plist);
    loop {
        let Value::Cons(key_cell) = cur else {
            return Ok(None);
        };
        let (key, next) = {
            let cell = key_cell
                .try_borrow()
                .map_err(|_| PlistReadError::Borrowed)?;
            (cell.car.clone(), cell.cdr.clone())
        };
        if matches(&key) {
            let value = match next {
                Value::Cons(value_cell) => value_cell
                    .try_borrow()
                    .map_err(|_| PlistReadError::Borrowed)?
                    .car
                    .clone(),
                _ => Value::Nil,
            };
            return Ok(Some(value));
        }
        let Value::Cons(value_cell) = next else {
            return Ok(None);
        };
        cur = value_cell
            .try_borrow()
            .map_err(|_| PlistReadError::Borrowed)?
            .cdr
            .clone();
        if cycle.advance(&cur) {
            return Ok(None);
        }
    }
}

/// Return the first value for PROP, or nil when absent.
pub fn plist_get(plist: &Value, prop: SymId) -> Value {
    plist_find(plist, |key| match key {
        Value::Sym(symbol) => *symbol == prop,
        Value::Nil => prop == sym::NIL,
        _ => false,
    })
    .ok()
    .flatten()
    .unwrap_or(Value::Nil)
}

/// Presence-aware variant of `plist_get': `Some(v)' iff the key
/// exists, even when its value is nil (GNU `lookup_char_property'
/// returns the slot value directly when found).
pub fn plist_lookup(plist: &Value, prop: SymId) -> Option<Value> {
    plist_find(plist, |key| match key {
        Value::Sym(symbol) => *symbol == prop,
        Value::Nil => prop == sym::NIL,
        _ => false,
    })
    .ok()
    .flatten()
}

/// `get`/`put` accept ANY Lisp object as PROPNAME (eq-compared) —
/// GNU's symbol plist isn't restricted to symbol keys.
pub fn plist_get_v(plist: &Value, prop: &Value) -> Value {
    plist_find(plist, |key| crate::lisp::builtins::eq_values(key, prop))
        .ok()
        .flatten()
        .unwrap_or(Value::Nil)
}

/// `plist_lookup' with an arbitrary (eq-compared) key — overlay
/// plists aren't restricted to symbol keys either.
pub fn plist_lookup_v(plist: &Value, prop: &Value) -> Option<Value> {
    plist_find(plist, |key| crate::lisp::builtins::eq_values(key, prop))
        .ok()
        .flatten()
}

/// Checked `get' lookup for Lisp entry points. An outstanding Rust mutable
/// borrow cannot be represented in Lisp, so report it as an ordinary error
/// instead of panicking in `RefCell::borrow'.
pub(crate) fn plist_get_v_checked(
    i: &Interp,
    plist: &Value,
    prop: &Value,
) -> Result<Value, Flow> {
    plist_find(plist, |key| crate::lisp::builtins::eq_values(key, prop))
        .map(|value| value.unwrap_or(Value::Nil))
        .map_err(|_| i.error("Cannot inspect a mutably borrowed property list"))
}

/// Remove every eq-matching property from a plist, keeping the remaining
/// key/value order and terminating if user mutation made the plist circular.
pub fn plist_remove_v(plist: &Value, prop: &Value) -> Value {
    let mut cur = plist.clone();
    let mut cycle = PlistCycle::new(plist);
    let mut kept = Vec::new();
    loop {
        let Value::Cons(key_cell) = cur else {
            break;
        };
        let (key, rest) = {
            let Ok(cell) = key_cell.try_borrow() else {
                return plist.clone();
            };
            (cell.car.clone(), cell.cdr.clone())
        };
        let (value, next) = match &rest {
            Value::Cons(value_cell) => {
                let Ok(cell) = value_cell.try_borrow() else {
                    return plist.clone();
                };
                (cell.car.clone(), cell.cdr.clone())
            }
            _ => (Value::Nil, Value::Nil),
        };
        if !crate::lisp::builtins::eq_values(&key, prop) {
            kept.push(key);
            kept.push(value);
        }
        let Value::Cons(_) = rest else {
            break;
        };
        cur = next;
        if cycle.advance(&cur) {
            break;
        }
    }
    Value::list(kept)
}

#[derive(Clone, Copy)]
enum PlistPutError {
    Circular,
    Malformed,
    Borrowed,
}

fn plist_put_v_inner(plist: &Value, prop: &Value, val: Value) -> Result<Value, PlistPutError> {
    let mut cur = plist.clone();
    let mut last_pair_end: Option<crate::lisp::value::ConsRef> = None;
    let mut cycle = PlistCycle::new(plist);
    loop {
        match &cur {
            Value::Cons(c) => {
                let (car, next) = {
                    let b = c
                        .try_borrow()
                        .map_err(|_| PlistPutError::Borrowed)?;
                    (b.car.clone(), b.cdr.clone())
                };
                if crate::lisp::builtins::eq_values(&car, prop) {
                    if let Value::Cons(c2) = &next {
                        c2.try_borrow_mut()
                            .map_err(|_| PlistPutError::Borrowed)?
                            .car = val;
                        return Ok(plist.clone());
                    }
                    // A key without its value cell is malformed even when
                    // it matches PROP. GNU validates the pair before its
                    // in-place update; a dotted tail after a complete pair
                    // is still left untouched when that pair matches.
                    return Err(PlistPutError::Malformed);
                }
                match next {
                    Value::Cons(c2) => {
                        let n2 = {
                            let b = c2
                                .try_borrow()
                                .map_err(|_| PlistPutError::Borrowed)?;
                            b.cdr.clone()
                        };
                        last_pair_end = Some(c2.clone());
                        cur = n2;
                        if cycle.advance(&cur) {
                            return Err(PlistPutError::Circular);
                        }
                    }
                    // A matching key with no value cell follows GNU Fput's
                    // in-place search path and is a no-op above. If no key
                    // matched, an odd or dotted tail is invalid before a
                    // new property can be appended.
                    Value::Nil => return Err(PlistPutError::Malformed),
                    _ => return Err(PlistPutError::Malformed),
                }
            }
            Value::Nil => break,
            _ => return Err(PlistPutError::Malformed),
        }
    }
    match last_pair_end {
        Some(cell) => {
            cell.try_borrow_mut()
                .map_err(|_| PlistPutError::Borrowed)?
                .cdr = Value::cons(prop.clone(), Value::cons(val, Value::Nil));
            Ok(plist.clone())
        }
        None => Ok(Value::cons(prop.clone(), Value::cons(val, Value::Nil))),
    }
}

/// `put' plist update for existing public callers. On circular or malformed
/// plists there is no valid append point, so preserve the old graph and
/// prepend the new pair.
pub fn plist_put_v(plist: &Value, prop: &Value, val: Value) -> Value {
    let fallback = Value::cons(prop.clone(), Value::cons(val.clone(), plist.clone()));
    plist_put_v_inner(plist, prop, val).unwrap_or(fallback)
}

/// Checked variant used by Lisp primitives whose GNU behavior rejects an
/// invalid tail or signals on a circular plist instead of silently choosing
/// a new insertion point.
pub(crate) fn plist_put_v_checked(
    i: &mut Interp,
    plist: &Value,
    prop: &Value,
    val: Value,
) -> Result<Value, Flow> {
    plist_put_v_inner(plist, prop, val).map_err(|error| match error {
        PlistPutError::Circular => i.signal_data(sym::CIRCULAR_LIST, vec![plist.clone()]),
        PlistPutError::Malformed => i.wrong_type_mut("plistp", plist),
        PlistPutError::Borrowed => i.error("Cannot modify a mutably borrowed property list"),
    })
}

/// `overlay-put`-style plist write: existing prop updates in place;
/// a NEW prop conses onto the FRONT (GNU Foverlay_put, buffer.c).
pub fn plist_put_front(plist: &Value, prop: &Value, val: Value) -> Value {
    let fallback = Value::cons(prop.clone(), Value::cons(val.clone(), plist.clone()));
    let mut cur = plist.clone();
    let mut cycle = PlistCycle::new(plist);
    while let Value::Cons(c) = cur {
        let (car, next) = match c.try_borrow() {
            Ok(b) => (b.car.clone(), b.cdr.clone()),
            Err(_) => return fallback,
        };
        if crate::lisp::builtins::eq_values(&car, prop) {
            if let Value::Cons(vc) = &next {
                match vc.try_borrow_mut() {
                    Ok(mut cell) => cell.car = val,
                    Err(_) => return fallback,
                }
            }
            return plist.clone();
        }
        if let Value::Cons(pair) = next {
            cur = match pair.try_borrow() {
                Ok(cell) => cell.cdr.clone(),
                Err(_) => return fallback,
            };
            if cycle.advance(&cur) {
                break;
            }
        } else {
            break;
        }
    }
    fallback
}

/// `put` on a plist value — returns a new plist with prop set.
/// Mutates in place when possible (Emacs mutates the plist).
pub fn plist_put(plist: &Value, prop: SymId, val: Value) -> Value {
    let fallback = Value::cons(Value::Sym(prop), Value::cons(val.clone(), plist.clone()));
    let mut cur = plist.clone();
    // Tracks the last cons cell reached while walking pairs — the
    // value cell of the final pair — so new props append at the end.
    let mut last_pair_end: Option<crate::lisp::value::ConsRef> = None;
    let mut cycle = PlistCycle::new(plist);
    loop {
        match &cur {
            Value::Cons(c) => {
                let (car, next) = {
                    let Ok(b) = c.try_borrow() else {
                        return fallback;
                    };
                    (b.car.clone(), b.cdr.clone())
                };
                if matches!(&car, Value::Sym(symbol) if *symbol == prop)
                    || (car.is_nil() && prop == sym::NIL)
                {
                    if let Value::Cons(c2) = &next {
                        let Ok(mut cell) = c2.try_borrow_mut() else {
                            return fallback;
                        };
                        cell.car = val;
                    }
                    return plist.clone();
                }
                match next {
                    Value::Cons(c2) => {
                        let Ok(b) = c2.try_borrow() else {
                            return fallback;
                        };
                        let n2 = b.cdr.clone();
                        last_pair_end = Some(c2.clone());
                        cur = n2;
                        if cycle.advance(&cur) {
                            // A circular plist has no end to append to.
                            // Preserve its graph and put the missing
                            // property at the front.
                            return fallback;
                        }
                    }
                    _ => break,
                }
            }
            _ => break,
        }
    }
    // Not found: append (prop val) at the END of the plist,
    // matching Emacs's put/plist-put ordering.
    match last_pair_end {
        Some(cell) => {
            match cell.try_borrow_mut() {
                Ok(mut cell) => {
                    cell.cdr = Value::cons(Value::Sym(prop), Value::cons(val, Value::Nil));
                    plist.clone()
                }
                Err(_) => fallback,
            }
        }
        None => Value::cons(Value::Sym(prop), Value::cons(val, Value::Nil)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_lookup_variants_find_properties_after_ten_thousand_pairs() {
        std::thread::Builder::new()
            .stack_size(128 * 1024 * 1024)
            .spawn(|| {
                let mut interp = crate::lisp::Interp::vacant();
                interp.obarray = crate::lisp::obarray::Obarray::new();
                let filler = interp.intern("plist-long-filler");
                let target = interp.intern("plist-long-target");
                let symbol = interp.intern("plist-long-owner");
                let mut items = Vec::with_capacity(20_004);
                for value in 0..10_001 {
                    items.push(Value::Sym(filler));
                    items.push(Value::Int(value));
                }
                items.push(Value::Sym(target));
                items.push(Value::Int(42));
                let plist = Value::list(items);

                assert!(matches!(plist_get(&plist, target), Value::Int(42)));
                assert!(matches!(plist_lookup(&plist, target), Some(Value::Int(42))));
                assert!(matches!(
                    plist_get_v(&plist, &Value::Sym(target)),
                    Value::Int(42)
                ));
                assert!(matches!(
                    plist_lookup_v(&plist, &Value::Sym(target)),
                    Some(Value::Int(42))
                ));
                interp.obarray.symbol_mut(symbol).plist = plist;
                assert!(matches!(interp.get_prop(symbol, target), Value::Int(42)));
                assert!(matches!(
                    crate::lisp::builtins::data::f_get(
                        &mut interp,
                        vec![Value::Sym(symbol), Value::Sym(target)].into(),
                    )
                    .unwrap(),
                    Value::Int(42)
                ));

                let nil_owner = interp.intern("plist-nil-property-owner");
                interp.obarray.symbol_mut(nil_owner).plist =
                    Value::list(vec![Value::Nil, Value::Int(7)]);
                assert!(matches!(
                    interp.get_prop(nil_owner, sym::NIL),
                    Value::Int(7)
                ));
            })
            .unwrap()
            .join()
            .unwrap();
    }

    #[test]
    fn lookup_variants_stop_at_circular_lists_and_keep_present_nil() {
        let prop = Value::Sym(3);
        let plist = Value::cons(Value::Sym(2), Value::Nil);
        if let Value::Cons(cell) = &plist {
            cell.borrow_mut().cdr = plist.clone();
        }
        assert!(plist_get(&plist, 3).is_nil());
        assert!(plist_lookup(&plist, 3).is_none());
        assert!(plist_get_v(&plist, &prop).is_nil());
        assert!(plist_lookup_v(&plist, &prop).is_none());

        let present_nil = Value::list(vec![prop.clone(), Value::Nil]);
        assert!(plist_get(&present_nil, 3).is_nil());
        assert!(matches!(plist_lookup(&present_nil, 3), Some(Value::Nil)));
        assert!(plist_get_v(&present_nil, &prop).is_nil());
        assert!(matches!(
            plist_lookup_v(&present_nil, &prop),
            Some(Value::Nil)
        ));

        let nil_property = Value::list(vec![Value::Nil, Value::Int(7)]);
        assert!(matches!(plist_get(&nil_property, sym::NIL), Value::Int(7)));
        assert!(matches!(
            plist_lookup(&nil_property, sym::NIL),
            Some(Value::Int(7))
        ));
    }

    #[test]
    fn symbol_nil_property_updates_a_canonical_nil_key_in_place() {
        let plist = Value::list(vec![Value::Nil, Value::Int(1)]);
        let updated = plist_put(&plist, sym::NIL, Value::Int(2));
        assert!(crate::lisp::builtins::eq_values(&plist, &updated));
        assert!(matches!(plist_get(&updated, sym::NIL), Value::Int(2)));
    }

    #[test]
    fn infallible_put_helpers_terminate_on_circular_plists() {
        let mut interp = crate::lisp::Interp::vacant();
        interp.obarray = crate::lisp::obarray::Obarray::new();
        let property = Value::Sym(3);
        let internal_property = interp.intern("plist-cycle-internal-put");
        for length in 1..=8 {
            let circular = Value::list(vec![Value::Sym(2); length]);
            let mut cur = circular.clone();
            let mut last = None;
            while let Value::Cons(cell) = cur {
                cur = cell.borrow().cdr.clone();
                last = Some(cell);
            }
            let last = last.unwrap();
            last.borrow_mut().cdr = circular.clone();

            assert!(plist_get_v(&circular, &property).is_nil());
            let checked = plist_put_v_checked(&mut interp, &circular, &property, Value::Int(42));
            assert!(matches!(
                checked,
                Err(Flow::Signal(ref signal)) if signal.is_symbol(sym::CIRCULAR_LIST)
            ));
            let public = plist_put_v(&circular, &property, Value::Int(42));
            assert!(matches!(plist_get_v(&public, &property), Value::Int(42)));
            let appended = plist_put(&circular, 3, Value::Int(42));
            assert!(matches!(plist_get(&appended, 3), Value::Int(42)));
            let front = plist_put_front(&circular, &Value::Sym(4), Value::Int(7));
            assert!(matches!(plist_get_v(&front, &Value::Sym(4)), Value::Int(7)));

            let owner = interp.intern(&format!("plist-cycle-owner-{length}"));
            interp.obarray.symbol_mut(owner).plist = circular.clone();
            interp.put_prop(owner, internal_property, Value::Int(19));
            assert!(matches!(
                interp.get_prop(owner, internal_property),
                Value::Int(19)
            ));

            // Break the cycle so dropping the test graph does not leak its Rc chain.
            last.borrow_mut().cdr = Value::Nil;
        }
    }

    #[test]
    fn plist_put_v_signals_on_circular_lists() {
        let mut interp = crate::lisp::Interp::vacant();
        interp.obarray = crate::lisp::obarray::Obarray::new();
        let filler = interp.intern("plist-cycle-filler");
        let property = interp.intern("plist-cycle-target");
        let key = Value::cons(Value::Sym(filler), Value::Nil);
        let value = Value::cons(Value::Int(1), key.clone());
        if let Value::Cons(cell) = &key {
            cell.borrow_mut().cdr = value;
        }
        let result = plist_put_v_checked(&mut interp, &key, &Value::Sym(property), Value::Int(42));
        assert!(matches!(
        result,
        Err(Flow::Signal(ref signal)) if signal.is_symbol(sym::CIRCULAR_LIST)
            ));
    }
}
