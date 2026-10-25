//! Compatibility facade for the runtime and optional frontends.
pub use remacs_cli as cli;
pub use remacs_cli::startup;
#[cfg(feature = "gui")]
pub use remacs_gui as gui;
pub use remacs_runtime::*;

/// Backward-compatible names for frontend-independent APIs and terminal I/O.
pub mod term {
    pub use remacs_cli::terminal::{Terminal, run_editor};
    pub use remacs_runtime::frontend::*;
}
