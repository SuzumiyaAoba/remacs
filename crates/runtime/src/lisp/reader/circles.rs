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
        Value::Str(string) => {
            let ptr = Rc::as_ptr(string) as usize;
            if !seen.insert(ptr) {
                return Ok(changed.contains(&ptr));
            }
            // String text properties live in Interp's side table rather
            // than in the String value.  They are still part of the object
            // graph: a `#N#' in a printed property plist can refer back to
            // the string currently being patched.
            let mut properties = interp.str_props(string).to_vec();
            let mut did_change = false;
            for (_, _, plist) in &mut properties {
                for item in plist {
                    let (patched, item_changed) =
                        patch_value(interp, item.clone(), placeholder, target, seen, changed)?;
                    *item = patched;
                    did_change |= item_changed;
                }
            }
            if did_change {
                interp.set_str_props(string, properties);
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
        Value::Hash(_) => {
            // GNU leaves hash-table data exactly as it was read. In
            // particular, it does not replace self-references in either
            // keys or values after the outer `#N=' definition is resolved.
            // Rewriting either side can also invalidate the stored hash key.
            Ok(false)
        }
        Value::Lambda(lambda) => {
            let ptr = Rc::as_ptr(lambda) as usize;
            if !seen.insert(ptr) {
                return Ok(changed.contains(&ptr));
            }
            // A `#[...]' reader result keeps its raw slots in this vector.
            // In particular, interpreted closures share the body/env conses
            // here with their execution fields, so patching those containers
            // repairs references back to the function itself.
            let Some(slot_vector) = &lambda.bc_items else {
                return Ok(false);
            };
            let mut items = slot_vector
                .try_borrow()
                .map(|items| items.clone())
                .map_err(|_| interp.error("Cannot inspect a mutably borrowed byte-code object"))?;
            let mut did_change = false;
            for item in &mut items {
                let (patched, item_changed) =
                    patch_value(interp, item.clone(), placeholder, target, seen, changed)?;
                if item_changed {
                    *item = patched;
                    did_change = true;
                }
            }
            if did_change {
                *slot_vector
                    .try_borrow_mut()
                    .map_err(|_| {
                        interp.error("Cannot inspect a mutably borrowed byte-code object")
                    })? = items;
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
    fn hash_table_circle_fixups_match_gnu_key_and_value_behavior() {
        std::thread::Builder::new()
            .stack_size(128 * 1024 * 1024)
            .spawn(|| {
                let mut interp = Interp::new();
                for form in [
                    r##"(let ((read-circle t))
                  (let ((table (read "#1=#s(hash-table size 1 test eq data (self #1#))")))
                    (let ((value (gethash 'self table)))
                      (and (consp value) (not (eq value table))
                           (null (car value)) (null (cdr value))))))"##,
                    r##"(let ((read-circle t))
                  (let ((table (read "#1=#s(hash-table size 1 test eq data (#1# #1#))")))
                    (let ((key nil) (value nil))
                      (maphash (lambda (k v) (setq key k value v)) table)
                      (and (consp key) (eq key value) (consp value)
                           (not (eq value table))
                           (eq (gethash table table 'missing) 'missing)
                           (eq (gethash key table) value)))))"##,
                    r##"(let* ((vector (let ((read-circle t))
                                      (read "#1=[#s(hash-table test eq data (self #1#))]")))
                                (table (aref vector 0))
                                (value (gethash 'self table)))
                           (and (consp value) (not (eq value vector))
                                (null (car value)) (null (cdr value))))"##,
                ] {
                    assert!(
                        !interp.eval_str(form).unwrap().is_nil(),
                        "hash-table circular reference was not restored: {form}"
                    );
                }
            })
            .unwrap()
            .join()
            .unwrap();
    }
}
