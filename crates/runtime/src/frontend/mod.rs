//! Frontend-independent rendering, input protocol, and editor command loop.
//!
//! Non-blocking design: the loop always `poll`s input with a timeout,
//! renders the frame, and executes commands. Long Lisp operations can
//! interleave with repaints because `sit-for`/`sleep-for` hand control
//! back to this loop rather than blocking inside the evaluator.

use std::io;
use std::ops::Range;
use std::time::Duration;

use crate::lisp::Interp;

/// Input from a front-end: a key event or text committed by the OS.
pub enum InputEvent {
    Key(i128),
    Text(String),
    ReplaceText { range: Range<usize>, text: String },
}

/// Frontend-agnostic key/render interface used by the command loop
/// and the nested minibuffer/isearch loops. The crossterm `Terminal`
/// implements it directly; the gpui front-end implements it over a
/// channel to the UI thread.
pub trait KeyIo {
    /// Poll for a key event, waiting at most `dur`. Returns an Emacs
    /// key code (char + modifier bits).
    fn poll_key(&mut self, dur: Duration) -> io::Result<Option<i128>>;
    /// Poll for input while preserving committed text as text.
    /// Key-only front-ends inherit this implementation.
    fn poll_input(&mut self, dur: Duration) -> io::Result<Option<InputEvent>> {
        self.poll_key(dur).map(|key| key.map(InputEvent::Key))
    }
    /// Push a key back onto the pending queue.
    fn unread(&mut self, k: i128);
    /// Repaint the whole frame.
    fn render(&mut self, i: &Interp) -> io::Result<()>;
    /// Show TEXT in the echo area (minibuffer prompt/status).
    fn draw_echo(&mut self, text: &str) -> io::Result<()>;
    /// Show minibuffer text while retaining front-end style information.
    /// Key-only front-ends inherit the plain echo-area implementation.
    fn draw_echo_with_state(&mut self, _i: &Interp, text: &str) -> io::Result<()> {
        self.draw_echo(text)
    }
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
pub use render::{
    CellColor, CellStyle, Grid, GridInput, GridInputRow, GridInputSelection, fit_text_to_cells,
    render_grid, render_grid_with_input_cached,
};
