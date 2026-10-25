//! Command-line arguments, GNU-compatible startup, and terminal adapter.
pub mod application;
pub mod arguments;
pub mod startup;
#[cfg(feature = "terminal")]
pub mod terminal;
pub use arguments::*;
