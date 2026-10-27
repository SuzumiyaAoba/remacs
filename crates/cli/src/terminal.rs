//! Crossterm session lifecycle and the terminal KeyIo adapter.
use crossterm::{
    cursor,
    event::{self, Event},
    queue,
    style::{Attribute, SetAttribute},
    terminal::{self},
};
use keys::key_event_to_code;
use remacs_runtime::frontend::{self, KeyIo, render_grid};
use remacs_runtime::lisp::Interp;

mod keys;
mod render;
use std::io::{self, Write};
use std::time::Duration;
/// The terminal screen.
pub struct Terminal {
    pub width: usize,
    pub height: usize,
    out: Box<dyn io::Write>,
    /// Pending key events read but not yet consumed (for unread-command-events).
    pending: Vec<i128>,
}

impl Terminal {
    /// Enter raw mode + alternate screen.
    pub fn enter() -> io::Result<Terminal> {
        let (w, h) = terminal::size().unwrap_or((80, 24));
        let w = w.max(10);
        let h = h.max(3);
        terminal::enable_raw_mode()?;
        let mut out = io::stdout();
        queue!(out, terminal::EnterAlternateScreen, cursor::Hide)?;
        out.flush()?;
        Ok(Terminal {
            width: w as usize,
            height: h as usize,
            out: Box::new(out),
            pending: Vec::new(),
        })
    }

    /// Restore the terminal. Always call before exit (RAII via Drop).
    pub fn leave(&mut self) {
        let _ = queue!(
            self.out,
            SetAttribute(Attribute::Reset),
            cursor::Show,
            terminal::LeaveAlternateScreen
        );
        let _ = self.out.flush();
        let _ = terminal::disable_raw_mode();
    }
}

impl KeyIo for Terminal {
    /// Poll for a key event, waiting at most `dur`.
    /// Returns an Emacs key code (char + modifier bits).
    fn poll_key(&mut self, dur: Duration) -> io::Result<Option<i128>> {
        if let Some(k) = self.pending.pop() {
            return Ok(Some(k));
        }
        if !event::poll(dur)? {
            return Ok(None);
        }
        match event::read()? {
            Event::Key(k) => Ok(key_event_to_code(k)),
            Event::Resize(w, h) => {
                self.width = (w as usize).max(10);
                self.height = (h as usize).max(3);
                Ok(None)
            }
            _ => Ok(None),
        }
    }

    /// Push a key back to the pending queue.
    fn unread(&mut self, k: i128) {
        self.pending.push(k);
    }

    /// Draw the frame: each window's buffer, mode lines, minibuffer line.
    fn render(&mut self, i: &Interp) -> io::Result<()> {
        let grid = render_grid(i, self.width, self.height);
        render::draw_grid(&mut self.out, &grid)
    }

    /// Draw the echo-area line with TEXT, cursor at its end.
    fn draw_echo(&mut self, text: &str) -> io::Result<()> {
        render::draw_echo(&mut self.out, self.width, self.height, text)
    }

    fn size(&self) -> (usize, usize) {
        (self.width, self.height)
    }
}

impl Drop for Terminal {
    fn drop(&mut self) {
        self.leave();
    }
}

/// Run the editor until `C-x C-c` (or `kill-emacs`).
pub fn run_editor(i: &mut Interp) -> io::Result<()> {
    frontend::run_editor_with(Terminal::enter()?, i)
}

#[cfg(test)]
mod tests;
