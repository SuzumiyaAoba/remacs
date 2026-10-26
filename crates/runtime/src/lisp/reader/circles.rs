//! Repair numbered-object placeholders without revisiting cyclic object graphs.

use crate::lisp::value::Value;
use rustc_hash::FxHashSet as HashSet;
use std::rc::Rc;

fn placeholder_eq(value: &Value, placeholder: &Value) -> bool {
    matches!((value, placeholder), (Value::Cons(value), Value::Cons(placeholder))
        if Rc::ptr_eq(value, placeholder))
}

/// Replace placeholder cons references inside `v` with `target`.
/// Track visited conses/vectors/records since they may already be cyclic.
pub(super) fn patch_in(v: &Value, placeholder: &Value, target: &Value, seen: &mut HashSet<usize>) {
    match v {
        Value::Cons(c) => {
            let ptr = Rc::as_ptr(c) as usize;
            if !seen.insert(ptr) {
                return;
            }
            let (car, cdr) = {
                let b = c.borrow();
                (b.car.clone(), b.cdr.clone())
            };
            if placeholder_eq(&car, placeholder) {
                c.borrow_mut().car = target.clone();
            } else {
                patch_in(&car, placeholder, target, seen);
            }
            if placeholder_eq(&cdr, placeholder) {
                c.borrow_mut().cdr = target.clone();
            } else {
                patch_in(&cdr, placeholder, target, seen);
            }
        }
        Value::Vec(vec) => {
            let ptr = Rc::as_ptr(vec) as usize;
            if !seen.insert(ptr) {
                return;
            }
            let items: Vec<Value> = vec.borrow().clone();
            let mut touched = false;
            let items: Vec<Value> = items
                .into_iter()
                .map(|it| {
                    if placeholder_eq(&it, placeholder) {
                        touched = true;
                        target.clone()
                    } else {
                        patch_in(&it, placeholder, target, seen);
                        it
                    }
                })
                .collect();
            if touched {
                *vec.borrow_mut() = items;
            }
        }
        Value::Record(rec) => {
            let ptr = Rc::as_ptr(rec) as usize;
            if !seen.insert(ptr) {
                return;
            }
            let items: Vec<Value> = rec.borrow().clone();
            let mut touched = false;
            let items: Vec<Value> = items
                .into_iter()
                .map(|it| {
                    if placeholder_eq(&it, placeholder) {
                        touched = true;
                        target.clone()
                    } else {
                        patch_in(&it, placeholder, target, seen);
                        it
                    }
                })
                .collect();
            if touched {
                *rec.borrow_mut() = items;
            }
        }
        _ => {}
    }
}
