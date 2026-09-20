//! Nonlocal control flow: Lisp signals, `throw`, and quit.
//!
//! Rust's `Result` models Emacs's `signal`/`throw`/quit machinery. `eval`
//! returns `Err(Flow)` which unwinds the Rust stack until a matching
//! `condition-case`/`catch` handler (or the top level) catches it.

use super::value::Value;

/// How evaluation unwound.
#[derive(Debug)]
pub enum Flow {
    /// `(signal SYM DATA)` — a Lisp error.
    Signal(Value, Value),
    /// `(throw TAG VALUE)` — caught by a matching `catch`.
    Throw(Value, Value),
    /// C-g quit. Like a signal but `condition-case` can't catch it
    /// (Emacs semantics).
    Quit,
}

/// `eval`/`apply` result.
pub type EvalResult = Result<Value, Flow>;
