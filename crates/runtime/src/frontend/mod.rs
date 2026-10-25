//! Frontend-independent rendering, input protocol, and editor command loop.
//!
//! Non-blocking design: the loop always `poll`s input with a timeout,
//! renders the frame, and executes commands. Long Lisp operations can
//! interleave with repaints because `sit-for`/`sleep-for` hand control
//! back to this loop rather than blocking inside the evaluator.

use std::io;
use std::time::Duration;

use crate::lisp::Interp;

/// Frontend-agnostic key/render interface used by the command loop
/// and the nested minibuffer/isearch loops. The crossterm `Terminal`
/// implements it directly; the gpui front-end implements it over a
/// channel to the UI thread.
pub trait KeyIo {
    /// Poll for a key event, waiting at most `dur`. Returns an Emacs
    /// key code (char + modifier bits).
    fn poll_key(&mut self, dur: Duration) -> io::Result<Option<i128>>;
    /// Push a key back onto the pending queue.
    fn unread(&mut self, k: i128);
    /// Repaint the whole frame.
    fn render(&mut self, i: &Interp) -> io::Result<()>;
    /// Show TEXT in the echo area (minibuffer prompt/status).
    fn draw_echo(&mut self, text: &str) -> io::Result<()>;
    /// Current frame size in text cells.
    fn size(&self) -> (usize, usize);
}

mod command;
mod render;
pub(crate) use command::minibuf_dispatch_key;
pub use command::{
    batch_command_error, dispatch_key_core, nonblocking_sleep, replay_macro_events,
    run_batch_editor, run_editor_with,
};
pub use render::{CellColor, CellStyle, Grid, render_grid};
