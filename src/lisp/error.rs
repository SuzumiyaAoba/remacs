//! Nonlocal control flow: Lisp signals, `throw`, and quit.
//!
//! Rust's `Result` models Emacs's `signal`/`throw`/quit machinery. `eval`
//! returns `Err(Flow)` which unwinds the Rust stack until a matching
//! `condition-case`/`catch` handler (or the top level) catches it.

use super::{Interp, value::Value};

/// One raised condition object, shared by observers, catchers and reporters.
/// GNU caches its matching conditions before invoking handler-bind; changing
/// the object's car/cdr does not select a different condition-case clause.
#[derive(Debug)]
pub struct Signal {
    pub condition: Value,
    pub(crate) conditions: Option<Value>,
    pub(crate) offered: bool,
    pub(crate) out_of_memory: bool,
    pub(crate) handler: Option<SignalHandler>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct SignalHandler {
    pub frame: usize,
    pub clause: usize,
}

/// A live condition-case scope. Interpreted forms group their clauses in one
/// scope; VM instructions and native catchers install a single clause.
pub(crate) struct CaseHandler {
    pub clauses: Vec<Value>,
}

impl CaseHandler {
    pub fn single(conditions: Value) -> Self {
        Self { clauses: vec![conditions] }
    }

    pub fn matching_clause(&self, i: &Interp, signal: &Signal) -> Option<usize> {
        self.clauses.iter().position(|conditions| signal.matches(i, conditions))
    }
}

impl Signal {
    pub fn new(symbol: Value, data: Value) -> Self {
        Self::from_condition(Value::cons(symbol, data))
    }

    pub fn from_condition(condition: Value) -> Self {
        Self {
            condition,
            conditions: None,
            offered: false,
            out_of_memory: false,
            handler: None,
        }
    }

    pub fn symbol(&self) -> Value {
        match &self.condition {
            Value::Cons(cell) => cell.borrow().car.clone(),
            other => other.clone(),
        }
    }

    pub fn data(&self) -> Value {
        match &self.condition {
            Value::Cons(cell) => cell.borrow().cdr.clone(),
            _ => Value::Nil,
        }
    }

    pub fn is_symbol(&self, symbol: super::value::SymId) -> bool {
        matches!(self.symbol(), Value::Sym(id) if id == symbol)
    }

    pub(crate) fn set_data(&mut self, data: Value) {
        if let Value::Cons(cell) = &self.condition {
            cell.borrow_mut().cdr = data;
        }
    }

    pub(crate) fn matches(&self, i: &Interp, handlers: &Value) -> bool {
        match &self.conditions {
            Some(conditions) => i.conditions_match(conditions, handlers),
            None => i.signal_matches(&self.symbol(), handlers),
        }
    }

    pub(crate) fn claims_frame(&self, frame: usize) -> bool {
        self.handler.is_some_and(|handler| handler.frame == frame)
    }

    pub(crate) fn claims_clause(&self, frame: usize, clause: usize) -> bool {
        self.handler == Some(SignalHandler { frame, clause })
    }
}

/// How evaluation unwound.
#[derive(Debug)]
pub enum Flow {
    /// `(signal SYM DATA)` or `(signal CONDITION)` — a shared condition.
    Signal(Signal),
    /// `(throw TAG VALUE)` — caught by a matching `catch`.
    Throw(Value, Value),
    /// A host cancellation, converted to a quit signal at Lisp boundaries.
    Quit,
    /// `(kill-emacs N)' / batch `suspend-emacs': the process is exiting.
    /// Uncatchable from Lisp — unwinds to the batch driver, which exits
    /// with the given status code.
    Exit(i128),
}

impl Flow {
    pub fn is_quit(&self) -> bool {
        matches!(self, Self::Quit)
            || matches!(self, Self::Signal(signal) if signal.is_symbol(super::obarray::sym::QUIT))
    }

    pub fn signal(symbol: Value, data: Value) -> Self {
        Self::Signal(Signal::new(symbol, data))
    }
}

/// `eval`/`apply` result.
pub type EvalResult = Result<Value, Flow>;
