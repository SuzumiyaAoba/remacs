//! Logic thread and channel-backed implementation of the common input protocol.
use remacs_runtime::{
    frontend::{Grid, InputEvent, KeyIo, fit_text_to_cells, render_grid_with_input_cached},
    lisp::Interp,
};
use std::{
    io,
    ops::Range,
    sync::mpsc::{self, Receiver, Sender},
    time::Duration,
};

/// Messages the UI thread sends to the editor logic thread.
pub(super) enum GuiEvent {
    Key(i128),
    Text(String),
    ReplaceText { range: Range<usize>, text: String },
    Resize(usize, usize),
}

/// `KeyIo` implementation backed by channels; runs on the logic
/// thread inside `run_editor_with`.
pub(super) struct ChanIo {
    pub(super) events: Receiver<GuiEvent>,
    pub(super) grids: smol::channel::Sender<Grid>,
    /// Keys pushed back via `unread` (unread-command-events).
    pub(super) pending: Vec<i128>,
    pub(super) cols: usize,
    pub(super) rows: usize,
    /// Last grid sent, so `draw_echo` can patch just the echo row.
    pub(super) last: Option<Grid>,
    /// A resize is not represented by an input event, but still requires
    /// a fresh grid on the next command-loop iteration.
    pub(super) render_requested: bool,
}

impl ChanIo {
    pub(super) fn send_grid(&mut self, grid: Grid) -> io::Result<()> {
        if !grid.blink_cursor && self.last.as_ref().is_some_and(|last| last == &grid) {
            return Ok(());
        }
        // Latest-wins: force_send displaces the oldest queued frame
        // when the bounded channel is full.
        let g = grid.clone();
        self.last = Some(grid);
        self.grids
            .force_send(g)
            .map_err(|_| io::Error::new(io::ErrorKind::BrokenPipe, "gui window closed"))?;
        Ok(())
    }

    fn echo_grid(&self, mut grid: Grid, text: &str, reset_styles: bool) -> Grid {
        let (shown, columns, visible_bytes) = fit_text_to_cells(text, self.cols);
        let echo_row = self.rows.saturating_sub(1);
        let selected_cursor = grid.cursors.iter().find(|cursor| cursor.selected).copied();
        grid.cursors.retain(|cursor| !cursor.selected);
        if let Some(last) = grid.rows.last_mut() {
            *last = shown;
        }
        if reset_styles {
            if let Some(last) = grid.attrs.last_mut() {
                last.iter_mut()
                    .for_each(|c| *c = remacs_runtime::frontend::CellStyle::default());
            }
        }
        if let Some(last) = grid.char_attrs.last_mut() {
            let char_count = grid.rows.last().map_or(0, |row| row.chars().count());
            if reset_styles {
                *last = vec![remacs_runtime::frontend::CellStyle::default(); char_count];
            } else {
                last.resize(char_count, remacs_runtime::frontend::CellStyle::default());
                last.truncate(char_count);
            }
        }
        let cx = columns.min(self.cols.saturating_sub(1));
        grid.cursor = Some((cx, echo_row));
        grid.cursor_index = Some(visible_bytes);
        if let Some(mut cursor) = selected_cursor {
            cursor.row = echo_row;
            cursor.column = cx;
            cursor.index = visible_bytes;
            cursor.after_index = None;
            grid.cursors.push(cursor);
        }
        grid
    }
}

impl KeyIo for ChanIo {
    fn poll_input(&mut self, dur: Duration) -> io::Result<Option<InputEvent>> {
        if let Some(k) = self.pending.pop() {
            return Ok(Some(InputEvent::Key(k)));
        }
        match self.events.recv_timeout(dur) {
            Ok(GuiEvent::Key(k)) => Ok(Some(InputEvent::Key(k))),
            Ok(GuiEvent::Text(text)) => Ok(Some(InputEvent::Text(text))),
            Ok(GuiEvent::ReplaceText { range, text }) => {
                Ok(Some(InputEvent::ReplaceText { range, text }))
            }
            Ok(GuiEvent::Resize(c, r)) => {
                self.cols = c.max(10);
                self.rows = r.max(3);
                self.render_requested = true;
                Ok(None)
            }
            Err(mpsc::RecvTimeoutError::Timeout) => Ok(None),
            Err(mpsc::RecvTimeoutError::Disconnected) => Err(io::Error::new(
                io::ErrorKind::BrokenPipe,
                "gui window closed",
            )),
        }
    }

    fn poll_key(&mut self, dur: Duration) -> io::Result<Option<i128>> {
        if let Some(k) = self.pending.pop() {
            return Ok(Some(k));
        }
        match self.events.recv_timeout(dur) {
            Ok(GuiEvent::Key(k)) => Ok(Some(k)),
            Ok(GuiEvent::Text(text)) => {
                let mut chars = text.chars().map(|ch| ch as i128);
                let first = chars.next();
                self.pending.extend(chars.rev());
                Ok(first)
            }
            Ok(GuiEvent::ReplaceText { text, .. }) => {
                let mut chars = text.chars().map(|ch| ch as i128);
                let first = chars.next();
                self.pending.extend(chars.rev());
                Ok(first)
            }
            Ok(GuiEvent::Resize(c, r)) => {
                self.cols = c.max(10);
                self.rows = r.max(3);
                self.render_requested = true;
                Ok(None)
            }
            Err(mpsc::RecvTimeoutError::Timeout) => Ok(None),
            Err(mpsc::RecvTimeoutError::Disconnected) => Err(io::Error::new(
                io::ErrorKind::BrokenPipe,
                "gui window closed",
            )),
        }
    }

    fn unread(&mut self, k: i128) {
        self.pending.push(k);
    }

    fn render(&mut self, i: &Interp) -> io::Result<()> {
        let cached = self.last.as_ref().and_then(|grid| grid.input.as_ref());
        let grid = render_grid_with_input_cached(i, self.cols, self.rows, cached);
        self.send_grid(grid)
    }

    fn render_on_idle(&self) -> bool {
        self.last.as_ref().is_some_and(|grid| grid.blink_cursor)
    }

    fn take_render_request(&mut self) -> bool {
        std::mem::take(&mut self.render_requested)
    }

    fn draw_echo(&mut self, text: &str) -> io::Result<()> {
        let grid = self
            .last
            .clone()
            .unwrap_or_else(|| Grid::blank(self.cols, self.rows));
        let grid = self.echo_grid(grid, text, true);
        self.send_grid(grid)
    }

    fn draw_echo_with_state(&mut self, i: &Interp, text: &str) -> io::Result<()> {
        let cached = self.last.as_ref().and_then(|grid| grid.input.as_ref());
        let grid = render_grid_with_input_cached(i, self.cols, self.rows, cached);
        let grid = self.echo_grid(grid, text, false);
        self.send_grid(grid)
    }

    fn size(&self) -> (usize, usize) {
        (self.cols, self.rows)
    }
}

/// Translate a gpui keystroke to an Emacs event code, mirroring
/// `key_event_to_code` in the terminal front-end.
/// Spawn the logic thread: build an `Interp`, visit `files`, and
/// run the shared command loop against a channel-backed `KeyIo`.
/// Returns the event sender (UI → logic) and the grid receiver
/// (logic → UI). Exposed separately from `run_gui` so tests can
/// drive the whole loop headlessly.
#[cfg(test)]
pub(super) fn start_logic(
    files: Vec<String>,
) -> io::Result<(Sender<GuiEvent>, smol::channel::Receiver<Grid>)> {
    start_logic_with_dump(files, None, Vec::new())
}

fn apply_startup_arguments(
    i: &mut Interp,
    files: &[String],
    arguments: &[String],
) -> remacs_runtime::lisp::EvalResult {
    let arguments = if arguments.is_empty() {
        let mut arguments = Vec::new();
        if !files.is_empty() {
            arguments.push("--".to_owned());
            arguments.extend_from_slice(files);
        }
        arguments
    } else {
        arguments.to_vec()
    };
    remacs_cli::startup::run(i, &arguments);
    Ok(remacs_runtime::lisp::Value::Nil)
}

pub(super) fn start_logic_with_dump(
    files: Vec<String>,
    dump_file: Option<String>,
    arguments: Vec<String>,
) -> io::Result<(Sender<GuiEvent>, smol::channel::Receiver<Grid>)> {
    let wait_for_dump = dump_file.is_some();
    let (startup_tx, startup_rx) = mpsc::channel::<Result<(), String>>();
    let (event_tx, event_rx) = mpsc::channel::<GuiEvent>();
    let (grid_tx, grid_rx) = smol::channel::bounded::<Grid>(1);
    std::thread::Builder::new()
        .name("remacs-logic".into())
        // Eval/macroexpand recursion needs far more than the default
        // thread stack (see main.rs's headless worker).
        .stack_size(512 * 1024 * 1024)
        .spawn(move || {
            let mut i = match dump_file {
                None => Interp::new(),
                Some(path) => match remacs_runtime::lisp::dump::load_with_arguments(
                    std::path::Path::new(&path),
                    Some(&arguments),
                ) {
                    Ok(interpreter) => interpreter,
                    Err(remacs_runtime::lisp::dump::LoadError::Exit(code)) => {
                        remacs_runtime::lifecycle::exit(code)
                    }
                    Err(error) => {
                        let _ = startup_tx.send(Err(error.to_string()));
                        return;
                    }
                },
            };
            if let Err(error) = apply_startup_arguments(&mut i, &files, &arguments) {
                let message = format!("GUI startup: {error:?}");
                eprintln!("remacs: {message}");
                let _ = startup_tx.send(Err(message));
                return;
            }
            let _ = startup_tx.send(Ok(()));
            let io = ChanIo {
                events: event_rx,
                grids: grid_tx,
                pending: Vec::new(),
                cols: 80,
                rows: 24,
                last: None,
                render_requested: false,
            };
            let _ = remacs_runtime::frontend::run_editor_with(io, &mut i);
            // grid_tx drops here → the UI pump below quits the app.
        })
        .map_err(io::Error::other)?;
    if wait_for_dump {
        startup_rx
            .recv()
            .map_err(io::Error::other)?
            .map_err(io::Error::other)?;
    }
    Ok((event_tx, grid_rx))
}
