//! The Emacs Lisp subsystem: values, symbols, reader, printer,
//! evaluator, special forms, builtins, regexp engine, loader.

pub mod builtins;
pub(crate) mod bytecode;
// Generated charset map data: the SJIS/JISX0201/BIG5-1/BIG5-2/VISCII
// tables aren't wired to a decoder yet, but stay with the rest of the set.
#[allow(dead_code)]
pub(crate) mod cjk_tables;
pub(crate) mod ctdata;
pub mod dynlib;
mod embedded_cache;
pub mod error;
pub mod eval;
pub mod load;
pub(crate) mod memory;
pub mod obarray;
pub mod prelude;
pub mod print;
pub(crate) mod profiler;
pub mod process;
pub mod reader;
pub mod regexp;
pub mod special;
pub mod value;

pub use builtins::eq_values;
pub use error::{EvalResult, Flow};
pub use eval::{
    ExcursionState, Interp, LexEnv, LexFrame, MatchData, MinibufInput, OutputSink, RestrictionState,
};
pub use obarray::{Obarray, sym};
pub use value::{Argv, Arity, Cons, HashTest, Lambda, LispHash, Marker, Subr, SymId, Value};
