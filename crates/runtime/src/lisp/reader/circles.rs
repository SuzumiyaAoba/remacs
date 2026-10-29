//! Repair numbered-object placeholders without revisiting cyclic object graphs.

use crate::lisp::{Interp, error::Flow, value::Value};
use rustc_hash::FxHashSet as HashSet;
use std::rc::Rc;

fn placeholder_eq(value: &Value, placeholder: &Value) -> bool {
    matches!((value, placeholder), (Value::Cons(value), Value::Cons(placeholder))
        if Rc::ptr_eq(value, placeholder))
}

/// Replace placeholder cons references inside `v` with `target`.
/// Track visited conses/vectors/records since they may already be cyclic.
pub(super) fn patch_in(
    interp: &mut Interp,
    v: &Value,
    placeholder: &Value,
    target: &Value,
    seen: &mut HashSet<usize>,
    changed: &mut HashSet<usize>,
) -> Result<bool, Flow> {
    match v {
        Value::Cons(c) => {
            let ptr = Rc::as_ptr(c) as usize;
            if !seen.insert(ptr) {
                return Ok(changed.contains(&ptr));
            }
            let (car, cdr) = {
                let b = c.borrow();
                (b.car.clone(), b.cdr.clone())
            };
            let (car, car_changed) = patch_value(interp, car, placeholder, target, seen, changed)?;
            let (cdr, cdr_changed) = patch_value(interp, cdr, placeholder, target, seen, changed)?;
            let did_change = car_changed || cdr_changed;
            if did_change {
                let mut cell = c.borrow_mut();
                cell.car = car;
                cell.cdr = cdr;
                changed.insert(ptr);
            }
            Ok(did_change)
        }
        Value::Vec(vec) | Value::Record(vec) => {
            let ptr = Rc::as_ptr(vec) as usize;
            if !seen.insert(ptr) {
                return Ok(changed.contains(&ptr));
            }
            let mut items: Vec<Value> = vec.borrow().clone();
            let mut did_change = false;
            for item in &mut items {
                let (patched, item_changed) =
                    patch_value(interp, item.clone(), placeholder, target, seen, changed)?;
                *item = patched;
                did_change |= item_changed;
            }
            if did_change {
                *vec.borrow_mut() = items;
                changed.insert(ptr);
            }
            Ok(did_change)
        }
        Value::Hash(hash) => {
            let ptr = Rc::as_ptr(hash) as usize;
            if !seen.insert(ptr) {
                return Ok(changed.contains(&ptr));
            }
            let entries: Vec<_> = {
                let table = hash.borrow();
                table
                    .keys
                    .iter()
                    .filter_map(|(key, original)| {
                        table
                            .map
                            .get(key)
                            .map(|value| (key.clone(), original.clone(), value.clone()))
                    })
                    .collect()
            };
            let mut patched = Vec::with_capacity(entries.len());
            let mut keys_changed = false;
            let mut did_change = false;
            for (old_key, original, value) in entries {
                let (original, key_changed) =
                    patch_value(interp, original, placeholder, target, seen, changed)?;
                let (value, value_changed) =
                    patch_value(interp, value, placeholder, target, seen, changed)?;
                keys_changed |= key_changed;
                did_change |= key_changed || value_changed;
                patched.push((old_key, original, value, key_changed));
            }

            if keys_changed {
                // A key's object graph may have changed while replacing a
                // reader placeholder. Rebuild the normalized keys as well
                // as the visible key slots; otherwise `eq' lookups keep
                // pointing at the temporary cons, and `equal' keys can
                // retain a hash for their pre-patched contents.
                let mut entries = Vec::with_capacity(patched.len());
                for (old_key, original, value, key_changed) in patched {
                    let key = if key_changed {
                        crate::lisp::builtins::hashfn::table_key(interp, hash, &original)?
                    } else {
                        old_key
                    };
                    entries.push((key, original, value));
                }
                let mut table = hash.borrow_mut();
                table.map.clear();
                table.keys.clear();
                table.collision_order.clear();
                table.free_slots.clear();
                table.positioned_keys = 0;
                for (key, original, value) in entries {
                    table.map.insert(key.clone(), value);
                    table.put_key(key, original);
                }
            } else if did_change {
                // A value-only fixup does not affect the key's hash.
                let mut table = hash.borrow_mut();
                for (key, _, value, _) in patched {
                    table.map.insert(key, value);
                }
            }
            if did_change {
                changed.insert(ptr);
            }
            Ok(did_change)
        }
        _ => Ok(false),
    }
}

fn patch_value(
    interp: &mut Interp,
    value: Value,
    placeholder: &Value,
    target: &Value,
    seen: &mut HashSet<usize>,
    changed: &mut HashSet<usize>,
) -> Result<(Value, bool), Flow> {
    if placeholder_eq(&value, placeholder) {
        Ok((target.clone(), true))
    } else {
        let did_change = patch_in(interp, &value, placeholder, target, seen, changed)?;
        Ok((value, did_change))
    }
}

#[cfg(test)]
mod tests {
    use crate::lisp::Interp;

    #[test]
    fn hash_table_circle_fixups_preserve_keys_and_values() {
        let mut interp = Interp::new();
        for form in [
            r##"(let ((read-circle t))
                  (let ((table (read "#1=#s(hash-table size 1 test eq data (self #1#))")))
                    (eq table (gethash 'self table))))"##,
            r##"(let ((read-circle t))
                  (let ((table (read "#1=#s(hash-table size 1 test eq data (#1# #1#))")))
                    (eq table (gethash table table))))"##,
        ] {
            assert!(
                !interp.eval_str(form).unwrap().is_nil(),
                "hash-table circular reference was not restored: {form}"
            );
        }
    }
}
