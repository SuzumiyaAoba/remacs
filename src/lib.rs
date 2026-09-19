//! remacs — an Emacs-compatible editor written in Rust.
//!
//! Architecture: a faithful Emacs *Lisp runtime* (values, reader,
//! evaluator, builtins) layered over a gap-buffer text core, an
//! editor layer (keymaps, command loop, windows, minibuffer), and a
//! terminal front-end that never blocks the UI thread.

pub mod buffer;
pub mod editor;
pub mod lisp;
pub mod term;
