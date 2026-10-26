//! Property-list access and mutation shared by evaluator and builtins.

use crate::lisp::value::{SymId, Value};

/// Return the first value for PROP, or nil when absent.
pub fn plist_get(plist: &Value, prop: SymId) -> Value {
    let mut cur = plist.clone();
    let mut n = 0;
    loop {
        match cur {
            Value::Cons(c) => {
                let (car, next) = {
                    let b = c.borrow();
                    (b.car.clone(), b.cdr.clone())
                };
                if let Value::Sym(s) = car {
                    if s == prop {
                        // Value is cadr.
                        if let Value::Cons(c2) = &next {
                            return c2.borrow().car.clone();
                        }
                        return Value::Nil;
                    }
                }
                // advance two
                match next {
                    Value::Cons(c2) => {
                        let n2 = {
                            let b = c2.borrow();
                            b.cdr.clone()
                        };
                        cur = n2;
                    }
                    _ => return Value::Nil,
                }
                n += 1;
                if n > 10000 {
                    return Value::Nil;
                }
            }
            _ => return Value::Nil,
        }
    }
}

/// Presence-aware variant of `plist_get': `Some(v)' iff the key
/// exists, even when its value is nil (GNU `lookup_char_property'
/// returns the slot value directly when found).
pub fn plist_lookup(plist: &Value, prop: SymId) -> Option<Value> {
    let mut cur = plist.clone();
    let mut n = 0;
    loop {
        match cur {
            Value::Cons(c) => {
                let (car, next) = {
                    let b = c.borrow();
                    (b.car.clone(), b.cdr.clone())
                };
                if let Value::Sym(s) = car {
                    if s == prop {
                        if let Value::Cons(c2) = &next {
                            return Some(c2.borrow().car.clone());
                        }
                        return Some(Value::Nil);
                    }
                }
                match next {
                    Value::Cons(c2) => {
                        let n2 = {
                            let b = c2.borrow();
                            b.cdr.clone()
                        };
                        cur = n2;
                    }
                    _ => return None,
                }
                n += 1;
                if n > 10000 {
                    return None;
                }
            }
            _ => return None,
        }
    }
}

/// `get`/`put` accept ANY Lisp object as PROPNAME (eq-compared) —
/// GNU's symbol plist isn't restricted to symbol keys.
pub fn plist_get_v(plist: &Value, prop: &Value) -> Value {
    let mut cur = plist.clone();
    let mut n = 0;
    loop {
        match cur {
            Value::Cons(c) => {
                let (car, next) = {
                    let b = c.borrow();
                    (b.car.clone(), b.cdr.clone())
                };
                if crate::lisp::builtins::eq_values(&car, prop) {
                    if let Value::Cons(c2) = &next {
                        return c2.borrow().car.clone();
                    }
                    return Value::Nil;
                }
                match next {
                    Value::Cons(c2) => {
                        let n2 = {
                            let b = c2.borrow();
                            b.cdr.clone()
                        };
                        cur = n2;
                    }
                    _ => return Value::Nil,
                }
                n += 1;
                if n > 10000 {
                    return Value::Nil;
                }
            }
            _ => return Value::Nil,
        }
    }
}

/// `plist_lookup' with an arbitrary (eq-compared) key — overlay
/// plists aren't restricted to symbol keys either.
pub fn plist_lookup_v(plist: &Value, prop: &Value) -> Option<Value> {
    let mut cur = plist.clone();
    let mut n = 0;
    loop {
        match cur {
            Value::Cons(c) => {
                let (car, next) = {
                    let b = c.borrow();
                    (b.car.clone(), b.cdr.clone())
                };
                if crate::lisp::builtins::eq_values(&car, prop) {
                    if let Value::Cons(c2) = &next {
                        return Some(c2.borrow().car.clone());
                    }
                    return Some(Value::Nil);
                }
                match next {
                    Value::Cons(c2) => {
                        let n2 = {
                            let b = c2.borrow();
                            b.cdr.clone()
                        };
                        cur = n2;
                    }
                    _ => return None,
                }
                n += 1;
                if n > 10000 {
                    return None;
                }
            }
            _ => return None,
        }
    }
}

pub fn plist_put_v(plist: &Value, prop: &Value, val: Value) -> Value {
    let mut cur = plist.clone();
    let mut last_pair_end: Option<crate::lisp::value::ConsRef> = None;
    loop {
        match cur {
            Value::Cons(c) => {
                let (car, next) = {
                    let b = c.borrow();
                    (b.car.clone(), b.cdr.clone())
                };
                if crate::lisp::builtins::eq_values(&car, prop) {
                    if let Value::Cons(c2) = &next {
                        c2.borrow_mut().car = val;
                        return plist.clone();
                    }
                    return plist.clone();
                }
                match next {
                    Value::Cons(c2) => {
                        let n2 = {
                            let b = c2.borrow();
                            b.cdr.clone()
                        };
                        last_pair_end = Some(c2);
                        cur = n2;
                    }
                    _ => break,
                }
            }
            _ => break,
        }
    }
    match last_pair_end {
        Some(cell) => {
            cell.borrow_mut().cdr = Value::cons(prop.clone(), Value::cons(val, Value::Nil));
            plist.clone()
        }
        None => Value::cons(prop.clone(), Value::cons(val, Value::Nil)),
    }
}

/// `overlay-put`-style plist write: existing prop updates in place;
/// a NEW prop conses onto the FRONT (GNU Foverlay_put, buffer.c).
pub fn plist_put_front(plist: &Value, prop: &Value, val: Value) -> Value {
    let mut cur = plist.clone();
    while let Value::Cons(c) = cur {
        let (car, next) = {
            let b = c.borrow();
            (b.car.clone(), b.cdr.clone())
        };
        if crate::lisp::builtins::eq_values(&car, prop) {
            if let Value::Cons(vc) = &next {
                vc.borrow_mut().car = val;
            }
            return plist.clone();
        }
        if let Value::Cons(pair) = next {
            cur = pair.borrow().cdr.clone();
        } else {
            break;
        }
    }
    Value::cons(prop.clone(), Value::cons(val, plist.clone()))
}

/// `put` on a plist value — returns a new plist with prop set.
/// Mutates in place when possible (Emacs mutates the plist).
pub fn plist_put(plist: &Value, prop: SymId, val: Value) -> Value {
    let mut cur = plist.clone();
    // Tracks the last cons cell reached while walking pairs — the
    // value cell of the final pair — so new props append at the end.
    let mut last_pair_end: Option<crate::lisp::value::ConsRef> = None;
    loop {
        match cur {
            Value::Cons(c) => {
                let (car, next) = {
                    let b = c.borrow();
                    (b.car.clone(), b.cdr.clone())
                };
                if let Value::Sym(s) = car {
                    if s == prop {
                        if let Value::Cons(c2) = &next {
                            c2.borrow_mut().car = val;
                            return plist.clone();
                        }
                        return plist.clone();
                    }
                }
                match next {
                    Value::Cons(c2) => {
                        let n2 = {
                            let b = c2.borrow();
                            b.cdr.clone()
                        };
                        last_pair_end = Some(c2);
                        cur = n2;
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
            cell.borrow_mut().cdr = Value::cons(Value::Sym(prop), Value::cons(val, Value::Nil));
            plist.clone()
        }
        None => Value::cons(Value::Sym(prop), Value::cons(val, Value::Nil)),
    }
}
