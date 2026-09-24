//! Startup Lisp: the subr.el subset remacs loads at interpreter boot.
//! These are real Lisp macros/functions (as in Emacs), not Rust code.

/// Source evaluated once per `Interp::new`.  Kept as a real `.el' file so
/// `find-function'/`find-variable' can navigate to prelude definitions
/// (recorded in `load-history' at startup).
pub const PRELUDE: &str = include_str!("prelude.el");
