//! Persistent lexical environments and shared mutable binding cells.
//!
//! Lookup stops at the first match; malformed tails are checked only when
//! reached. Keep these rules shared by eval, closures, and explicit environments.

use std::{cell::RefCell, rc::Rc};

use super::Interp;
use crate::lisp::{
    error::Flow,
    obarray::sym,
    value::{SymId, Value},
};

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

pub(crate) fn lexframe_bind_checked(
    frame: &LexFrame,
    sym: SymId,
    value: Value,
) -> Result<(), LexEnvError> {
    let symbol = if sym == sym::NIL {
        Value::Nil
    } else {
        Value::Sym(sym)
    };
    let binding = Value::cons(symbol, value);
    let tail = frame
        .raw
        .try_borrow()
        .map_err(|_| LexEnvError::borrowed(&Value::Nil))?
        .clone();
    *frame
        .raw
        .try_borrow_mut()
        .map_err(|_| LexEnvError::borrowed(&Value::Nil))? = Value::cons(binding, tail);
    Ok(())
}

/// The actual GNU interpreter environment, including shared binding cells.
pub fn lexenv_as_value(env: &LexEnv) -> Value {
    // This compatibility API cannot return a Lisp error, but a borrowed
    // frame must not unwind through printers or Rust embedding callers.
    // Interpreter entry points that need to distinguish the failure use
    // `lexenv_as_value_checked` instead.
    lexenv_as_value_checked(env).unwrap_or(Value::Nil)
}

pub(crate) fn lexenv_as_value_checked(env: &LexEnv) -> Result<Value, LexEnvError> {
    match env {
        Some(frame) => frame
            .raw
            .try_borrow()
            .map(|raw| raw.clone())
            .map_err(|_| LexEnvError::borrowed(&Value::Nil)),
        None => Ok(Value::Nil),
    }
}

pub fn lexenv_extend(parent: &LexEnv) -> Rc<LexFrame> {
    Rc::new(LexFrame {
        raw: RefCell::new(lexenv_as_value(parent)),
        declared: RefCell::new(Vec::new()),
    })
}

pub(crate) fn lexenv_extend_checked(parent: &LexEnv) -> Result<Rc<LexFrame>, LexEnvError> {
    Ok(Rc::new(LexFrame {
        raw: RefCell::new(lexenv_as_value_checked(parent)?),
        declared: RefCell::new(Vec::new()),
    }))
}

pub type LexEnv = Option<Rc<LexFrame>>;

/// An invalid tail is an error only when a lexical search reaches it.
pub(crate) struct LexEnvError {
    env: Value,
    circular: bool,
    borrowed: bool,
}

impl LexEnvError {
    fn borrowed(env: &Value) -> Self {
        Self {
            env: env.clone(),
            circular: false,
            borrowed: true,
        }
    }

    fn in_env(mut self, env: &LexEnv) -> Self {
        if let Some(frame) = env
            && let Ok(raw) = frame.raw.try_borrow()
        {
            self.env = raw.clone();
        }
        self
    }

    pub(crate) fn into_flow(self, i: &mut Interp) -> Flow {
        if self.borrowed {
            i.error("Cannot access a mutably borrowed lexical environment")
        } else if self.circular {
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
    mut matches: impl FnMut(&Value) -> Result<bool, LexEnvError>,
) -> Result<Option<Value>, LexEnvError> {
    let mut cur = raw.clone();
    let mut slow = raw.clone();
    let mut advance_slow = false;
    loop {
        match cur {
            Value::Nil => return Ok(None),
            Value::Cons(c) => {
                let (item, next) = {
                    let c = c.try_borrow().map_err(|_| LexEnvError::borrowed(raw))?;
                    (c.car.clone(), c.cdr.clone())
                };
                if matches(&item)? {
                    return Ok(Some(item));
                }
                cur = next;
            }
            _ => {
                return Err(LexEnvError {
                    env: raw.clone(),
                    circular: false,
                    borrowed: false,
                });
            }
        }
        // As in Value::list_to_vec, advance the slow cursor every
        // other step. Explicit environment lookup needs no allocation.
        if advance_slow {
            if let Value::Cons(c) = slow {
                slow = c
                    .try_borrow()
                    .map_err(|_| LexEnvError::borrowed(raw))?
                    .cdr
                    .clone();
            }
            if let (Value::Cons(a), Value::Cons(b)) = (&slow, &cur) {
                if Rc::ptr_eq(a, b) {
                    return Err(LexEnvError {
                        env: raw.clone(),
                        circular: true,
                        borrowed: false,
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
    Ok(lexenv_find(raw, |v| match v {
        Value::Cons(c) => c
            .try_borrow()
            .map(|cell| crate::lisp::builtins::eq_values(&cell.car, &key))
            .map_err(|_| LexEnvError::borrowed(raw)),
        _ => Ok(false),
    })?
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
    let raw = frame
        .raw
        .try_borrow()
        .map_err(|_| LexEnvError::borrowed(&Value::Nil))?
        .clone();
    match lexenv_assq(&raw, sym).map_err(|e| e.in_env(env))? {
        Some(cell) => cell
            .try_borrow()
            .map(|cell| Some(cell.cdr.clone()))
            .map_err(|_| LexEnvError::borrowed(&raw)),
        None => Ok(None),
    }
}

pub fn lexenv_lookup(env: &LexEnv, sym: SymId) -> Option<Value> {
    lexenv_lookup_checked(env, sym).ok().flatten()
}

pub(crate) fn lexenv_set(env: &LexEnv, sym: SymId, value: Value) -> Result<bool, LexEnvError> {
    let Some(frame) = env else { return Ok(false) };
    let raw = frame
        .raw
        .try_borrow()
        .map_err(|_| LexEnvError::borrowed(&Value::Nil))?
        .clone();
    if let Some(cell) = lexenv_assq(&raw, sym).map_err(|e| e.in_env(env))? {
        cell.try_borrow_mut()
            .map_err(|_| LexEnvError::borrowed(&raw))?
            .cdr = value;
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
    let raw = frame
        .raw
        .try_borrow()
        .map_err(|_| LexEnvError::borrowed(&Value::Nil))?
        .clone();
    Ok(lexenv_find(&raw, |value| {
        Ok(crate::lisp::builtins::eq_values(value, &key))
    })
    .map_err(|e| e.in_env(env))?
    .is_some())
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

pub(crate) fn lexenv_declare_checked(env: &LexEnv, sym: SymId) -> Result<(), LexEnvError> {
    let Some(frame) = env else { return Ok(()) };
    let symbol = if sym == sym::NIL {
        Value::Nil
    } else {
        Value::Sym(sym)
    };
    let tail = frame
        .raw
        .try_borrow()
        .map_err(|_| LexEnvError::borrowed(&Value::Nil))?
        .clone();
    let mut raw = frame
        .raw
        .try_borrow_mut()
        .map_err(|_| LexEnvError::borrowed(&tail))?;
    let mut declared = frame
        .declared
        .try_borrow_mut()
        .map_err(|_| LexEnvError::borrowed(&tail))?;
    *raw = Value::cons(symbol, tail);
    declared.push(sym);
    Ok(())
}

pub fn lexenv_root() -> LexEnv {
    lexenv_from_value(&Value::list(vec![Value::Sym(sym::T)]))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{panic::AssertUnwindSafe, thread};

    #[test]
    fn lexical_lookup_reports_a_mutably_borrowed_binding_without_panicking() {
        thread::Builder::new()
            .stack_size(64 * 1024 * 1024)
            .spawn(|| {
                let mut interp = Interp::new();
                let symbol = interp.intern("qa-borrowed-lexical-binding");
                let binding = Value::cons(Value::Sym(symbol), Value::Int(42));
                let raw = Value::cons(binding.clone(), Value::Nil);
                interp.lexenv = lexenv_from_value(&raw);
                let Value::Cons(binding_cell) = &binding else {
                    unreachable!();
                };

                let borrow = binding_cell.borrow_mut();
                let result =
                    std::panic::catch_unwind(AssertUnwindSafe(|| interp.eval(&Value::Sym(symbol))));
                drop(borrow);

                let result = result.expect("lexical lookup must not panic on a borrowed binding");
                assert!(result.is_err(), "the unavailable lexical read must signal");

                let frame = lexenv_root().expect("lexical root frame");
                interp.lexenv = Some(frame.clone());
                let borrow = frame.raw.borrow_mut();
                let variable = interp.intern("qa-borrowed-lexenv-let");
                let form = Value::list(vec![
                    Value::Sym(interp.q("let")),
                    Value::list(vec![Value::list(vec![Value::Sym(variable), Value::Int(1)])]),
                    Value::Sym(variable),
                ]);
                let result = std::panic::catch_unwind(AssertUnwindSafe(|| interp.eval(&form)));
                drop(borrow);
                let result =
                    result.expect("nested lexical binding must not panic on a borrowed frame");
                assert!(
                    result.is_err(),
                    "unavailable lexical frame access must signal"
                );

                interp.lexenv = Some(frame.clone());
                let borrow = frame.raw.borrow_mut();
                let result = std::panic::catch_unwind(AssertUnwindSafe(|| {
                    interp.function_from_parts(Value::Nil, Value::Nil, Value::Nil, Value::Nil)
                }));
                drop(borrow);
                let result =
                    result.expect("closure construction must not panic on a borrowed frame");
                assert!(
                    result.is_err(),
                    "unavailable closure environment must signal"
                );
            })
            .unwrap()
            .join()
            .unwrap();
    }

    #[test]
    fn lexical_environment_reads_and_writes_report_borrow_conflicts() {
        let mut interp = Interp::vacant();
        let symbol = interp.intern("qa-borrowed-lexenv-cell");
        let binding = Value::cons(Value::Sym(symbol), Value::Int(42));
        let raw = Value::cons(binding.clone(), Value::Nil);
        let env = lexenv_from_value(&raw);
        let Value::Cons(binding_cell) = &binding else {
            unreachable!();
        };

        let borrow = binding_cell.borrow_mut();
        assert!(lexenv_lookup_checked(&env, symbol).is_err());
        assert!(lexenv_set(&env, symbol, Value::Int(43)).is_err());
        drop(borrow);
        assert!(matches!(
            lexenv_lookup_checked(&env, symbol),
            Ok(Some(Value::Int(42)))
        ));

        let frame = Rc::new(LexFrame {
            raw: RefCell::new(Value::Nil),
            declared: RefCell::new(Vec::new()),
        });
        let borrowed = frame.raw.borrow_mut();
        assert!(lexenv_as_value_checked(&Some(frame.clone())).is_err());
        assert!(lexenv_extend_checked(&Some(frame.clone())).is_err());
        assert!(lexframe_bind_checked(&frame, symbol, Value::Int(1)).is_err());
        assert!(lexenv_declare_checked(&Some(frame.clone()), symbol).is_err());
        drop(borrowed);

        let declared = Value::cons(Value::Sym(symbol), Value::Nil);
        let declared_env = lexenv_from_value(&declared);
        let Value::Cons(declared_cell) = &declared else {
            unreachable!();
        };
        let borrow = declared_cell.borrow_mut();
        assert!(lexenv_declared(&declared_env, symbol).is_err());
        drop(borrow);
        assert!(matches!(lexenv_declared(&declared_env, symbol), Ok(true)));
    }
}
