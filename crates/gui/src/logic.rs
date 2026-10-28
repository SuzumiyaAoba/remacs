//! Logic thread and channel-backed implementation of the common input protocol.
use remacs_runtime::{
    frontend::{
        Grid, InputEvent, KeyIo, MouseButton, MouseInput, MousePosition, fit_text_to_cells,
        render_grid_with_input_cached,
    },
    lisp::{Interp, Value},
};
use std::{
    io,
    ops::Range,
    rc::Rc,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, Sender, SyncSender},
    },
    time::Duration,
};
use unicode_width::UnicodeWidthChar;

/// Messages the UI thread sends to the editor logic thread.
pub(super) enum GuiEvent {
    Key(i128),
    Text(String),
    ReplaceText { range: Range<usize>, text: String },
    Resize(usize, usize),
    Geometry(GuiGeometry),
    MouseDown {
        button: MouseButton,
        click_count: usize,
        modifiers: i128,
        position: MousePosition,
    },
    MouseUp {
        button: MouseButton,
        click_count: usize,
        modifiers: i128,
        position: MousePosition,
    },
    Scroll {
        delta_lines: f32,
        modifiers: i128,
        position: MousePosition,
    },
}

#[derive(Clone, PartialEq)]
pub(super) struct MonitorGeometry {
    pub display_id: u32,
    pub geometry: [i128; 4],
    pub workarea: [i128; 4],
    pub mm_size: [Option<i128>; 2],
    /// AppKit screen origin used to translate GPUI window bounds.
    pub native_origin: [i128; 2],
}

#[derive(Clone, PartialEq)]
pub(super) struct GuiGeometry {
    pub cols: usize,
    pub rows: usize,
    pub pixel_width: i128,
    pub pixel_height: i128,
    pub char_width: i128,
    pub char_height: i128,
    pub scale_factor: f64,
    pub frame_left: i128,
    pub frame_top: i128,
    pub current_display_id: Option<u32>,
    pub monitors: Vec<MonitorGeometry>,
}

#[derive(Clone, Copy)]
pub(super) struct GeometrySymbols {
    pub pixel_width: remacs_runtime::lisp::value::SymId,
    pub pixel_height: remacs_runtime::lisp::value::SymId,
    pub char_width: remacs_runtime::lisp::value::SymId,
    pub char_height: remacs_runtime::lisp::value::SymId,
    pub grid_cols: remacs_runtime::lisp::value::SymId,
    pub grid_rows: remacs_runtime::lisp::value::SymId,
    pub scale_factor: remacs_runtime::lisp::value::SymId,
    pub frame_left: remacs_runtime::lisp::value::SymId,
    pub frame_top: remacs_runtime::lisp::value::SymId,
    pub monitor_attributes: remacs_runtime::lisp::value::SymId,
    pub geometry: remacs_runtime::lisp::value::SymId,
    pub workarea: remacs_runtime::lisp::value::SymId,
    pub mm_size: remacs_runtime::lisp::value::SymId,
    pub frames: remacs_runtime::lisp::value::SymId,
}

pub(super) enum ClipboardRequest {
    Ready(Arc<AtomicBool>),
    Read(SyncSender<Result<Option<String>, String>>),
    Write(String, Option<SyncSender<Result<(), String>>>),
}

struct GuiClipboardBackend {
    requests: smol::channel::Sender<ClipboardRequest>,
    ready: Arc<AtomicBool>,
}

impl remacs_runtime::frontend::ClipboardBackend for GuiClipboardBackend {
    fn read_text(&self) -> Result<Option<String>, String> {
        if !self.ready.load(Ordering::Acquire) {
            return Ok(None);
        }
        let (reply, response) = mpsc::sync_channel(1);
        self.requests
            .send_blocking(ClipboardRequest::Read(reply))
            .map_err(|_| "GUI clipboard service is unavailable".to_owned())?;
        response
            .recv()
            .map_err(|_| "GUI clipboard service stopped".to_owned())?
    }

    fn write_text(&self, text: String) -> Result<(), String> {
        let ready = self.ready.load(Ordering::Acquire);
        let (reply, response) = mpsc::sync_channel(1);
        self.requests
            .send_blocking(ClipboardRequest::Write(text, ready.then_some(reply)))
            .map_err(|_| "GUI clipboard service is unavailable".to_owned())?;
        if !ready {
            return Ok(());
        }
        response
            .recv()
            .map_err(|_| "GUI clipboard service stopped".to_owned())?
    }
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
    pub(super) geometry: Option<GuiGeometry>,
    pub(super) geometry_symbols: Option<GeometrySymbols>,
    pub(super) mouse_down: Option<(MouseButton, usize, i128, MousePosition)>,
    pub(super) scroll_remainder: f32,
}

fn echo_char_width(character: char, column: usize) -> usize {
    match character {
        '\n' | '\r' => 1,
        '\t' => 8 - column % 8,
        _ => UnicodeWidthChar::width(character).unwrap_or(1),
    }
}

fn echo_input_cursor(text: &str, cursor_utf16: usize) -> Option<(usize, usize)> {
    let mut utf16 = 0usize;
    let mut characters = 0usize;
    let mut columns = 0usize;
    for character in text.chars() {
        if utf16 == cursor_utf16 {
            return Some((characters, columns));
        }
        let next = utf16.saturating_add(character.len_utf16());
        if next > cursor_utf16 {
            return None;
        }
        columns = columns.saturating_add(echo_char_width(character, columns));
        characters += 1;
        utf16 = next;
    }
    (utf16 == cursor_utf16).then_some((characters, columns))
}

fn fit_echo_text_at_cursor(
    text: &str,
    width: usize,
    cursor_character: usize,
    cursor_column: usize,
) -> (String, usize, usize, usize, usize) {
    if width == 0 {
        return (String::new(), 0, 0, 0, 0);
    }
    let scroll_start = cursor_column.saturating_sub(width);
    let scroll_end = scroll_start.saturating_add(width);
    let mut rendered = String::new();
    let mut source_column = 0usize;
    let mut rendered_columns = 0usize;
    let mut cursor_index = None;
    for (index, raw_character) in text.chars().enumerate() {
        if index == cursor_character {
            cursor_index = Some(rendered.len());
        }
        let character = match raw_character {
            '\n' | '\r' => ' ',
            other => other,
        };
        let char_width = echo_char_width(raw_character, source_column);
        let char_end = source_column.saturating_add(char_width);
        let visible_start = source_column.max(scroll_start);
        let visible_end = char_end.min(scroll_end);
        if char_width == 0 {
            if (scroll_start..=scroll_end).contains(&source_column) {
                rendered.push(character);
            }
        } else if visible_end > visible_start {
            let visible_width = visible_end - visible_start;
            if raw_character == '\t' || visible_start != source_column || visible_end != char_end {
                rendered.push_str(&" ".repeat(visible_width));
            } else {
                rendered.push(character);
            }
            rendered_columns = rendered_columns.saturating_add(visible_width);
        }
        source_column = char_end;
    }
    if cursor_character >= text.chars().count() {
        cursor_index = Some(rendered.len());
    }
    let cursor_column = cursor_column
        .saturating_sub(scroll_start)
        .min(width.saturating_sub(1));
    let cursor_index = cursor_index.unwrap_or(rendered.len());
    let visible_bytes = rendered.len();
    rendered.push_str(&" ".repeat(width.saturating_sub(rendered_columns)));
    (
        rendered,
        rendered_columns,
        visible_bytes,
        cursor_column,
        cursor_index.min(visible_bytes),
    )
}

impl GeometrySymbols {
    fn intern(i: &mut Interp) -> GeometrySymbols {
        GeometrySymbols {
            pixel_width: i.intern("pixel-width"),
            pixel_height: i.intern("pixel-height"),
            char_width: i.intern("char-width"),
            char_height: i.intern("char-height"),
            grid_cols: i.intern("grid-cols"),
            grid_rows: i.intern("grid-rows"),
            scale_factor: i.intern("scale-factor"),
            frame_left: i.intern("left"),
            frame_top: i.intern("top"),
            monitor_attributes: i.intern("display-monitor-attributes-list"),
            geometry: i.intern("geometry"),
            workarea: i.intern("workarea"),
            mm_size: i.intern("mm-size"),
            frames: i.intern("frames"),
        }
    }
}

impl ChanIo {
    fn mouse_input(
        &mut self,
        button: MouseButton,
        click_count: usize,
        modifiers: i128,
        end: MousePosition,
    ) -> MouseInput {
        let down = self
            .mouse_down
            .take()
            .filter(|(down_button, _, _, _)| *down_button == button);
        let (start, down_click_count, down_modifiers) = down
            .map(|(_, click_count, modifiers, position)| (position, click_count, modifiers))
            .unwrap_or((end, click_count, 0));
        MouseInput {
            button,
            click_count: click_count.max(down_click_count).max(1),
            modifiers: modifiers | down_modifiers,
            start,
            end,
        }
    }

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

    fn echo_grid(
        &self,
        mut grid: Grid,
        text: &str,
        reset_styles: bool,
        use_grid_cursor: bool,
    ) -> Grid {
        let echo_row = self.rows.saturating_sub(1);
        let selected_cursor = use_grid_cursor
            .then(|| grid.cursors.iter().find(|cursor| cursor.selected).copied())
            .flatten();
        let fallback_cursor_column = use_grid_cursor
            .then(|| grid.cursor.map(|(column, _)| column))
            .flatten();
        let fallback_cursor_index = use_grid_cursor.then_some(grid.cursor_index).flatten();
        let input_cursor = if use_grid_cursor {
            grid.input.as_ref().and_then(|input| {
                (input.text.as_ref() == text)
                    .then(|| echo_input_cursor(&input.text, input.cursor))
                    .flatten()
            })
        } else {
            None
        };
        let (shown, cursor_column, cursor_index) = match input_cursor {
            Some((cursor_character, cursor_column)) => {
                let (shown, _, _, cursor_column, cursor_index) =
                    fit_echo_text_at_cursor(text, self.cols, cursor_character, cursor_column);
                (shown, cursor_column, cursor_index)
            }
            None => {
                let (shown, columns, visible_bytes) = fit_text_to_cells(text, self.cols);
                let cursor_column = selected_cursor
                    .map(|cursor| cursor.column)
                    .or(fallback_cursor_column)
                    .unwrap_or(columns);
                let cursor_index = selected_cursor
                    .map(|cursor| cursor.index)
                    .or(fallback_cursor_index)
                    .unwrap_or(visible_bytes);
                (shown, cursor_column, cursor_index.min(visible_bytes))
            }
        };
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
        let cx = cursor_column.min(self.cols.saturating_sub(1));
        grid.cursor = Some((cx, echo_row));
        grid.cursor_index = Some(cursor_index);
        if let Some(mut cursor) = selected_cursor {
            cursor.row = echo_row;
            cursor.column = cx;
            cursor.index = cursor_index;
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
            Ok(GuiEvent::Geometry(geometry)) => {
                self.cols = geometry.cols.max(10);
                self.rows = geometry.rows.max(3);
                self.geometry = Some(geometry);
                self.render_requested = true;
                Ok(None)
            }
            Ok(GuiEvent::MouseDown {
                button,
                click_count,
                modifiers,
                position,
            }) => {
                self.mouse_down = Some((button, click_count, modifiers, position));
                Ok(Some(InputEvent::MouseDown(MouseInput {
                    button,
                    click_count: click_count.max(1),
                    modifiers,
                    start: position,
                    end: position,
                })))
            }
            Ok(GuiEvent::MouseUp {
                button,
                click_count,
                modifiers,
                position,
            }) => Ok(Some(InputEvent::Mouse(self.mouse_input(
                button,
                click_count,
                modifiers,
                position,
            )))),
            Ok(GuiEvent::Scroll {
                delta_lines,
                modifiers,
                position,
            }) => {
                if !delta_lines.is_finite() {
                    return Ok(None);
                }
                let total_lines = self.scroll_remainder + delta_lines;
                let whole_lines = total_lines.trunc();
                self.scroll_remainder = total_lines - whole_lines;
                if whole_lines == 0.0 {
                    Ok(None)
                } else {
                    Ok(Some(InputEvent::MouseWheel {
                        up: whole_lines < 0.0,
                        modifiers,
                        position,
                        line_count: whole_lines.abs() as usize,
                    }))
                }
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
            Ok(GuiEvent::Geometry(geometry)) => {
                self.cols = geometry.cols.max(10);
                self.rows = geometry.rows.max(3);
                self.geometry = Some(geometry);
                self.render_requested = true;
                Ok(None)
            }
            Ok(GuiEvent::MouseDown {
                button,
                click_count,
                modifiers,
                position,
            }) => {
                self.mouse_down = Some((button, click_count, modifiers, position));
                Ok(None)
            }
            Ok(GuiEvent::MouseUp { .. }) => {
                self.mouse_down = None;
                Ok(None)
            }
            Ok(GuiEvent::Scroll { .. }) => Ok(None),
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
        if let (Some(geometry), Some(keys)) = (&self.geometry, self.geometry_symbols)
            && let Some(frame) = i.selected_frame.as_ref()
        {
            let frames = Value::list(
                i.frames
                    .iter()
                    .filter(|frame| !frame.borrow().dead)
                    .cloned()
                    .map(Value::Frame)
                    .collect(),
            );
            let mut monitors = Vec::with_capacity(geometry.monitors.len());
            for monitor in &geometry.monitors {
                let monitor_frames = if geometry
                    .current_display_id
                    .is_none_or(|display_id| display_id == monitor.display_id)
                {
                    frames.clone()
                } else {
                    Value::Nil
                };
                let attr = |key, values: [i128; 4]| {
                    let mut fields = Vec::with_capacity(5);
                    fields.push(Value::Sym(key));
                    fields.extend(values.into_iter().map(Value::Int));
                    Value::list(fields)
                };
                monitors.push(Value::list(vec![
                    attr(keys.geometry, monitor.geometry),
                    attr(keys.workarea, monitor.workarea),
                    Value::list(vec![
                        Value::Sym(keys.mm_size),
                        monitor.mm_size[0].map(Value::Int).unwrap_or(Value::Nil),
                        monitor.mm_size[1].map(Value::Int).unwrap_or(Value::Nil),
                    ]),
                    Value::list(vec![Value::Sym(keys.frames), monitor_frames]),
                ]));
            }
            let mut params = frame.borrow().params.clone();
            for (key, value) in [
                (keys.pixel_width, Value::Int(geometry.pixel_width)),
                (keys.pixel_height, Value::Int(geometry.pixel_height)),
                (keys.char_width, Value::Int(geometry.char_width)),
                (keys.char_height, Value::Int(geometry.char_height)),
                (keys.grid_cols, Value::Int(geometry.cols as i128)),
                (keys.grid_rows, Value::Int(geometry.rows as i128)),
                (keys.frame_left, Value::Int(geometry.frame_left)),
                (keys.frame_top, Value::Int(geometry.frame_top)),
                (keys.scale_factor, Value::float(geometry.scale_factor)),
                (keys.monitor_attributes, Value::list(monitors)),
            ] {
                params =
                    remacs_runtime::lisp::eval::plist_put_front(&params, &Value::Sym(key), value);
            }
            frame.borrow_mut().params = params;
        }
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
        let grid = self.echo_grid(grid, text, true, false);
        self.send_grid(grid)
    }

    fn draw_echo_with_state(&mut self, i: &Interp, text: &str) -> io::Result<()> {
        let cached = self.last.as_ref().and_then(|grid| grid.input.as_ref());
        let grid = render_grid_with_input_cached(i, self.cols, self.rows, cached);
        let grid = self.echo_grid(grid, text, false, true);
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
    let (events, grids, _clipboard_requests) =
        start_logic_inner(files, None, Vec::new(), false)?;
    Ok((events, grids))
}

#[cfg(test)]
pub(super) fn start_logic_with_dump_for_test(
    files: Vec<String>,
    dump_file: Option<String>,
    arguments: Vec<String>,
) -> io::Result<(Sender<GuiEvent>, smol::channel::Receiver<Grid>)> {
    let (events, grids, _clipboard_requests) =
        start_logic_inner(files, dump_file, arguments, false)?;
    Ok((events, grids))
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
) -> io::Result<(
    Sender<GuiEvent>,
    smol::channel::Receiver<Grid>,
    smol::channel::Receiver<ClipboardRequest>,
)> {
    start_logic_inner(files, dump_file, arguments, true)
}

fn start_logic_inner(
    files: Vec<String>,
    dump_file: Option<String>,
    arguments: Vec<String>,
    enable_gui_selections: bool,
) -> io::Result<(
    Sender<GuiEvent>,
    smol::channel::Receiver<Grid>,
    smol::channel::Receiver<ClipboardRequest>,
)> {
    let wait_for_dump = dump_file.is_some();
    let (startup_tx, startup_rx) = mpsc::channel::<Result<(), String>>();
    let (event_tx, event_rx) = mpsc::channel::<GuiEvent>();
    let (grid_tx, grid_rx) = smol::channel::bounded::<Grid>(1);
    let (clipboard_tx, clipboard_rx) = smol::channel::unbounded::<ClipboardRequest>();
    let clipboard_ready = Arc::new(AtomicBool::new(false));
    clipboard_tx
        .send_blocking(ClipboardRequest::Ready(clipboard_ready.clone()))
        .map_err(|_| io::Error::new(io::ErrorKind::BrokenPipe, "GUI clipboard channel closed"))?;
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
            if enable_gui_selections && cfg!(target_os = "macos") {
                i.set_clipboard_backend(Some(Rc::new(GuiClipboardBackend {
                    requests: clipboard_tx,
                    ready: clipboard_ready,
                })));
            }
            if let Err(error) = apply_startup_arguments(&mut i, &files, &arguments) {
                let message = format!("GUI startup: {error:?}");
                eprintln!("remacs: {message}");
                let _ = startup_tx.send(Err(message));
                return;
            }
            #[cfg(target_os = "macos")]
            if enable_gui_selections {
                // Startup still uses the shared terminal initializer; set
                // NS identity afterward so generic selection methods dispatch
                // without running native AppKit window initialization.
                configure_ns_window_system(&mut i);
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
                geometry: None,
                geometry_symbols: if enable_gui_selections {
                    Some(GeometrySymbols::intern(&mut i))
                } else {
                    None
                },
                mouse_down: None,
                scroll_remainder: 0.0,
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
    Ok((event_tx, grid_rx, clipboard_rx))
}

#[cfg(target_os = "macos")]
fn configure_ns_window_system(i: &mut Interp) {
    let window_system = i.intern("window-system");
    let initial_window_system = i.intern("initial-window-system");
    let ns = i.intern("ns");
    let value = remacs_runtime::lisp::Value::Sym(ns);
    let _ = i.set_symbol_default(window_system, value.clone());
    let _ = i.set_symbol_default(initial_window_system, value.clone());
    if let Some(frame) = i.selected_frame.clone() {
        let key = remacs_runtime::lisp::Value::Sym(window_system);
        let params = remacs_runtime::lisp::eval::plist_put_front(
            &frame.borrow().params,
            &key,
            value,
        );
        frame.borrow_mut().params = params;
        let update_faces = i.intern("frame-set-background-mode");
        let _ = i.apply(
            &remacs_runtime::lisp::Value::Sym(update_faces),
            vec![remacs_runtime::lisp::Value::Frame(frame)],
        );
    }
}
