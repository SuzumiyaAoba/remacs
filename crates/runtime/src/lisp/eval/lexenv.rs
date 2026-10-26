//! Persistent lexical environments and shared mutable binding cells.
//!
//! Lookup stops at the first match; malformed tails are checked only when
//! reached. Keep these rules shared by eval, closures, and explicit environments.

use super::Interp;
use crate::lisp::{
    error::Flow,
    obarray::sym,
    value::{SymId, Value},
};
use std::{cell::RefCell, rc::Rc};

/// A lexical environment head. Its alist spine is persistent while each
/// `(symbol . value)' cell is shared. Capturing the head preserves lexical
/// scope and lets closure optimization retain the original mutable cells.
pub struct LexFrame {
    pub raw: RefCell<Value>,
    /// File-load bookkeeping for scoped bare defvar declarations.
    pub declared: RefCell<Vec<SymId>>,
}

pub fn lexframe_bind(frame: &LexFrame, sym: SymId, value: Value) {
    let symbol = if sym == sym::NIL {
        Value::Nil
    } else {
        Value::Sym(sym)
    };
    let binding = Value::cons(symbol, value);
    let tail = frame.raw.borrow().clone();
    *frame.raw.borrow_mut() = Value::cons(binding, tail);
}

/// The actual GNU interpreter environment, including shared binding cells.
pub fn lexenv_as_value(env: &LexEnv) -> Value {
    env.as_ref()
        .map(|frame| frame.raw.borrow().clone())
        .unwrap_or(Value::Nil)
}

pub fn lexenv_extend(parent: &LexEnv) -> Rc<LexFrame> {
    Rc::new(LexFrame {
        raw: RefCell::new(lexenv_as_value(parent)),
        declared: RefCell::new(Vec::new()),
    })
}

pub type LexEnv = Option<Rc<LexFrame>>;

/// An invalid tail is an error only when a lexical search reaches it.
pub(crate) struct LexEnvError {
    env: Value,
    circular: bool,
}

impl LexEnvError {
    fn in_env(mut self, env: &LexEnv) -> Self {
        self.env = lexenv_as_value(env);
        self
    }

    pub(crate) fn into_flow(self, i: &mut Interp) -> Flow {
        if self.circular {
            i.signal_data(sym::CIRCULAR_LIST, vec![self.env])
        } else {
            i.wrong_type_mut("listp", &self.env)
        }
    }
}

/// GNU's assq/memq searches preserve the first match and do not examine
/// the remaining tail after a match.  Keep the original alist in errors.
fn lexenv_find(
    raw: &Value,
    matches: impl Fn(&Value) -> bool,
) -> Result<Option<Value>, LexEnvError> {
    let mut cur = raw.clone();
    let mut slow = raw.clone();
    let mut advance_slow = false;
    loop {
        match cur {
            Value::Nil => return Ok(None),
            Value::Cons(c) => {
                let (item, next) = {
                    let c = c.borrow();
                    (c.car.clone(), c.cdr.clone())
                };
                if matches(&item) {
                    return Ok(Some(item));
                }
                cur = next;
            }
            _ => {
                return Err(LexEnvError {
                    env: raw.clone(),
                    circular: false,
                });
            }
        }
        // As in Value::list_to_vec, advance the slow cursor every
        // other step. Explicit environment lookup needs no allocation.
        if advance_slow {
            if let Value::Cons(c) = slow {
                slow = c.borrow().cdr.clone();
            }
            if let (Value::Cons(a), Value::Cons(b)) = (&slow, &cur) {
                if Rc::ptr_eq(a, b) {
                    return Err(LexEnvError {
                        env: raw.clone(),
                        circular: true,
                    });
                }
            }
        }
        advance_slow = !advance_slow;
    }
}

fn lexenv_assq(
    raw: &Value,
    sym: SymId,
) -> Result<Option<crate::lisp::value::ConsRef>, LexEnvError> {
    let key = if sym == sym::NIL {
        Value::Nil
    } else {
        Value::Sym(sym)
    };
    Ok(lexenv_find(
        raw,
        |v| matches!(v, Value::Cons(c) if crate::lisp::builtins::eq_values(&c.borrow().car, &key)),
    )?
    .and_then(|v| {
        if let Value::Cons(c) = v {
            Some(c)
        } else {
            None
        }
    }))
}

pub(crate) fn lexenv_lookup_checked(
    env: &LexEnv,
    sym: SymId,
) -> Result<Option<Value>, LexEnvError> {
    let Some(frame) = env else { return Ok(None) };
    let raw = frame.raw.borrow().clone();
    Ok(lexenv_assq(&raw, sym)
        .map_err(|e| e.in_env(env))?
        .map(|cell| cell.borrow().cdr.clone()))
}

pub fn lexenv_lookup(env: &LexEnv, sym: SymId) -> Option<Value> {
    lexenv_lookup_checked(env, sym).ok().flatten()
}

pub(crate) fn lexenv_set(env: &LexEnv, sym: SymId, value: Value) -> Result<bool, LexEnvError> {
    let Some(frame) = env else { return Ok(false) };
    let raw = frame.raw.borrow().clone();
    if let Some(cell) = lexenv_assq(&raw, sym).map_err(|e| e.in_env(env))? {
        cell.borrow_mut().cdr = value;
        return Ok(true);
    }
    Ok(false)
}

/// Keep explicit environments flat and shared, with first-match lookup.
pub(crate) fn lexenv_from_value(value: &Value) -> LexEnv {
    if value.is_nil() {
        None
    } else {
        Some(Rc::new(LexFrame {
            raw: RefCell::new(value.clone()),
            declared: RefCell::new(Vec::new()),
        }))
    }
}

/// GNU's `Fmemq(var, Vinternal_interpreter_environment)': true when SYM
/// is declared dynamically scoped by a bare `defvar' anywhere in ENV.
/// Scoped declarations only affect binding decisions — lookups go
/// through `lexenv_lookup'/the dynamic cell like GNU's `assq'.
pub(crate) fn lexenv_declared(env: &LexEnv, sym: SymId) -> Result<bool, LexEnvError> {
    let Some(frame) = env else { return Ok(false) };
    let key = if sym == sym::NIL {
        Value::Nil
    } else {
        Value::Sym(sym)
    };
    let raw = frame.raw.borrow().clone();
    Ok(
        lexenv_find(&raw, |value| crate::lisp::builtins::eq_values(value, &key))
            .map_err(|e| e.in_env(env))?
            .is_some(),
    )
}

pub fn lexenv_declare(env: &LexEnv, sym: SymId) {
    if let Some(frame) = env {
        frame.declared.borrow_mut().push(sym);
        let symbol = if sym == sym::NIL {
            Value::Nil
        } else {
            Value::Sym(sym)
        };
        let tail = frame.raw.borrow().clone();
        *frame.raw.borrow_mut() = Value::cons(symbol, tail);
    }
}

pub fn lexenv_root() -> LexEnv {
    lexenv_from_value(&Value::list(vec![Value::Sym(sym::T)]))
}
