//! Logic thread and channel-backed implementation of the common input protocol.
use remacs_runtime::{
    frontend::{
        DragAction, Grid, GridScrollBarOrientation, InputEvent, KeyIo, MouseButton, MouseInput,
        MousePosition, fit_text_to_cells, render_frame_grid_with_mouse_position_cached,
        render_grid_with_mouse_position_cached,
    },
    lisp::{Interp, Value},
};
use std::{
    collections::HashMap,
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

pub(super) const COLOR_DROP_PREFIX: &str = "\u{1f}remacs-color-drop:";

fn same_mouse_face_position(left: Option<MousePosition>, right: Option<MousePosition>) -> bool {
    match (left, right) {
        (None, None) => true,
        (Some(left), Some(right)) => {
            left.buffer_id == right.buffer_id && left.buffer_position == right.buffer_position
        }
        _ => false,
    }
}

/// Messages the UI thread sends to the editor logic thread.
pub(super) enum GuiEvent {
    /// Input emitted by a particular native frame window.
    ForFrame {
        frame_id: usize,
        event: Box<GuiEvent>,
    },
    FrameVisibility { frame_id: usize, visible: bool },
    ColorDrop {
        position: Option<MousePosition>,
        color: String,
        background: bool,
    },
    FrameFocus { frame_id: usize, focused: bool },
    FrameDeleteRequest { frame_id: usize },
    PowerOff,
    Key(i128),
    Text(String),
    MenuCommand(String),
    CommandArgs { command: String, args: Vec<i128> },
    ReplaceText { range: Range<usize>, text: String },
    Drop {
        position: Option<MousePosition>,
        urls: Vec<String>,
        action: String,
    },
    Resize(usize, usize),
    Geometry(GuiGeometry),
    MouseMove {
        frame_id: Option<usize>,
        position: Option<MousePosition>,
    },
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
    ScrollBar {
        window_id: usize,
        orientation: GridScrollBarOrientation,
        offset: usize,
        max_offset: usize,
    },
}

#[derive(Clone)]
pub(super) struct FrameEventSender {
    sender: Sender<GuiEvent>,
    frame_id: Rc<std::cell::Cell<Option<usize>>>,
}

impl FrameEventSender {
    pub(super) fn new(sender: Sender<GuiEvent>, frame_id: Option<usize>) -> Self {
        Self {
            sender,
            frame_id: Rc::new(std::cell::Cell::new(frame_id)),
        }
    }

    pub(super) fn set_frame_id(&self, frame_id: usize) {
        self.frame_id.set(Some(frame_id));
    }

    pub(super) fn frame_id(&self) -> Option<usize> {
        self.frame_id.get()
    }

    pub(super) fn send_visibility_update(
        &self,
        frame_id: usize,
        visible: bool,
    ) -> Result<(), mpsc::SendError<GuiEvent>> {
        self.sender
            .send(GuiEvent::FrameVisibility { frame_id, visible })
    }

    pub(super) fn send_delete_request(
        &self,
        frame_id: usize,
    ) -> Result<(), mpsc::SendError<GuiEvent>> {
        self.sender.send(GuiEvent::FrameDeleteRequest { frame_id })
    }

    pub(super) fn send_focus_update(
        &self,
        frame_id: usize,
        focused: bool,
    ) -> Result<(), mpsc::SendError<GuiEvent>> {
        self.sender.send(GuiEvent::FrameFocus { frame_id, focused })
    }

    pub(super) fn send(&self, event: GuiEvent) -> Result<(), mpsc::SendError<GuiEvent>> {
        let event = match self.frame_id.get() {
            Some(frame_id) => GuiEvent::ForFrame {
                frame_id,
                event: Box::new(event),
            },
            None => event,
        };
        self.sender.send(event)
    }

    pub(super) fn send_mouse_move(
        &self,
        position: Option<MousePosition>,
    ) -> Result<(), mpsc::SendError<GuiEvent>> {
        self.sender.send(GuiEvent::MouseMove {
            frame_id: self.frame_id.get(),
            position,
        })
    }
}

/// A frame's render snapshot, sent from the logic thread to the GPUI thread.
/// It contains no Lisp objects, keeping the `Interp` on its logic thread.
#[derive(Clone)]
pub(super) struct FrameSnapshot {
    pub frame_id: usize,
    pub name: String,
    pub parent_frame: Option<usize>,
    pub modifier_mapping: super::keys::ModifierMapping,
    pub alpha_active: u8,
    pub alpha_inactive: u8,
    pub alpha_background: u8,
    pub width: usize,
    pub height: usize,
    pub left: Option<i128>,
    pub top: Option<i128>,
    pub undecorated: bool,
    pub ns_transparent_titlebar: bool,
    pub ns_appearance: Option<gpui::WindowAppearance>,
    pub fullscreen: Option<String>,
    pub auto_raise: bool,
    pub auto_lower: bool,
    pub z_group: Option<String>,
    pub grid: Grid,
}

#[derive(Clone, Copy)]
pub(super) struct InitialFrameSize {
    pub width: usize,
    pub height: usize,
}

pub(super) enum GuiRequest {
    FrameSnapshots(Vec<FrameSnapshot>),
    FrameVisibility { frame_id: usize, visible: bool },
    FrameOrder { frame_id: usize, raise: bool },
    Bell { frame_id: Option<usize>, visible: bool },
    ColorPanel {
        frame_id: usize,
        reply: SyncSender<Result<(), String>>,
    },
    AboutPanel {
        frame_id: usize,
        version: String,
        reply: SyncSender<Result<(), String>>,
    },
    CharacterPalette {
        frame_id: usize,
        reply: SyncSender<Result<(), String>>,
    },
    HideApp {
        hide: bool,
        reply: SyncSender<Result<(), String>>,
    },
    HideOtherApps(SyncSender<Result<(), String>>),
    Prompt {
        frame_id: usize,
        message: String,
        detail: Option<String>,
        buttons: Vec<String>,
        reply: SyncSender<Result<Option<usize>, String>>,
    },
    FileDialog {
        prompt: String,
        directory: Option<String>,
        must_match: bool,
        initial: Option<String>,
        directory_only: bool,
        reply: SyncSender<Result<Option<String>, String>>,
    },
    BeginTextDrag {
        frame_id: usize,
        text: String,
        action: DragAction,
        started: SyncSender<Result<(), String>>,
        completed: SyncSender<Option<DragAction>>,
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

struct GuiDialogBackend {
    requests: smol::channel::Sender<GuiRequest>,
}

impl remacs_runtime::frontend::DialogBackend for GuiDialogBackend {
    fn choose(
        &self,
        frame_id: usize,
        message: &str,
        detail: Option<&str>,
        buttons: &[String],
    ) -> Result<Option<usize>, String> {
        let (reply, response) = mpsc::sync_channel(1);
        self.requests
            .send_blocking(GuiRequest::Prompt {
                frame_id,
                message: message.to_owned(),
                detail: detail.map(str::to_owned),
                buttons: buttons.to_vec(),
                reply,
            })
            .map_err(|_| "GUI dialog service is unavailable".to_owned())?;
        response
            .recv()
            .map_err(|_| "GUI dialog service stopped".to_owned())?
    }
}

struct GuiFileDialogBackend {
    requests: smol::channel::Sender<GuiRequest>,
}

struct GuiDragBackend {
    requests: smol::channel::Sender<GuiRequest>,
}

impl remacs_runtime::frontend::DragBackend for GuiDragBackend {
    fn begin_text_drag(
        &self,
        frame_id: usize,
        text: &str,
        action: DragAction,
    ) -> Result<Option<DragAction>, String> {
        let (started, start_response) = mpsc::sync_channel(1);
        let (completed, completion_response) = mpsc::sync_channel(1);
        self.requests
            .send_blocking(GuiRequest::BeginTextDrag {
                frame_id,
                text: text.to_owned(),
                action,
                started,
                completed,
            })
            .map_err(|_| "GUI drag service is unavailable".to_owned())?;
        start_response
            .recv()
            .map_err(|_| "GUI drag service stopped".to_owned())??;
        completion_response
            .recv()
            .map_err(|_| "GUI drag service stopped".to_owned())
    }
}

impl remacs_runtime::frontend::FileDialogBackend for GuiFileDialogBackend {
    fn read_file_name(
        &self,
        prompt: &str,
        directory: Option<&str>,
        must_match: bool,
        initial: Option<&str>,
        directory_only: bool,
    ) -> Result<Option<String>, String> {
        let (reply, response) = mpsc::sync_channel(1);
        self.requests
            .send_blocking(GuiRequest::FileDialog {
                prompt: prompt.to_owned(),
                directory: directory.map(str::to_owned),
                must_match,
                initial: initial.map(str::to_owned),
                directory_only,
                reply,
            })
            .map_err(|_| "GUI file dialog service is unavailable".to_owned())?;
        response
            .recv()
            .map_err(|_| "GUI file dialog service stopped".to_owned())?
    }
}

struct GuiFrameVisibilityBackend {
    requests: smol::channel::Sender<GuiRequest>,
}

impl remacs_runtime::frontend::FrameWindowBackend for GuiFrameVisibilityBackend {
    fn set_visible(&self, frame_id: usize, visible: bool) -> Result<(), String> {
        self.requests
            .send_blocking(GuiRequest::FrameVisibility { frame_id, visible })
            .map_err(|_| "GUI window service is unavailable".to_owned())
    }

    fn raise(&self, frame_id: usize) -> Result<(), String> {
        self.requests
            .send_blocking(GuiRequest::FrameOrder {
                frame_id,
                raise: true,
            })
            .map_err(|_| "GUI window service is unavailable".to_owned())
    }

    fn lower(&self, frame_id: usize) -> Result<(), String> {
        self.requests
            .send_blocking(GuiRequest::FrameOrder {
                frame_id,
                raise: false,
            })
            .map_err(|_| "GUI window service is unavailable".to_owned())
    }
}

struct GuiBellBackend {
    requests: smol::channel::Sender<GuiRequest>,
}

impl remacs_runtime::frontend::BellBackend for GuiBellBackend {
    fn ring_bell(&self, frame_id: Option<usize>, visible: bool) {
        let _ = self
            .requests
            .send_blocking(GuiRequest::Bell { frame_id, visible });
    }
}

struct GuiAboutPanelBackend {
    requests: smol::channel::Sender<GuiRequest>,
}

impl remacs_runtime::frontend::AboutPanelBackend for GuiAboutPanelBackend {
    fn show(&self, frame_id: usize, version: &str) -> Result<(), String> {
        let (reply, response) = mpsc::sync_channel(1);
        self.requests
            .send_blocking(GuiRequest::AboutPanel {
                frame_id,
                version: version.to_owned(),
                reply,
            })
            .map_err(|_| "GUI About panel service is unavailable".to_owned())?;
        response
            .recv()
            .map_err(|_| "GUI About panel service stopped".to_owned())?
    }
}

struct GuiColorPanelBackend {
    requests: smol::channel::Sender<GuiRequest>,
}

impl remacs_runtime::frontend::ColorPanelBackend for GuiColorPanelBackend {
    fn show(&self, frame_id: usize) -> Result<(), String> {
        let (reply, response) = mpsc::sync_channel(1);
        self.requests
            .send_blocking(GuiRequest::ColorPanel { frame_id, reply })
            .map_err(|_| "GUI color panel service is unavailable".to_owned())?;
        response
            .recv()
            .map_err(|_| "GUI color panel service stopped".to_owned())?
    }

    fn show_character_palette(&self, frame_id: usize) -> Result<(), String> {
        let (reply, response) = mpsc::sync_channel(1);
        self.requests
            .send_blocking(GuiRequest::CharacterPalette { frame_id, reply })
            .map_err(|_| "GUI character palette service is unavailable".to_owned())?;
        response
            .recv()
            .map_err(|_| "GUI character palette service stopped".to_owned())?
    }
}

struct GuiApplicationVisibilityBackend {
    requests: smol::channel::Sender<GuiRequest>,
}

impl remacs_runtime::frontend::ApplicationVisibilityBackend
    for GuiApplicationVisibilityBackend
{
    fn hide(&self) -> Result<(), String> {
        self.request_hide(true)
    }

    fn activate(&self) -> Result<(), String> {
        self.request_hide(false)
    }

    fn hide_other_apps(&self) -> Result<(), String> {
        let (reply, response) = mpsc::sync_channel(1);
        self.requests
            .send_blocking(GuiRequest::HideOtherApps(reply))
            .map_err(|_| "GUI application service is unavailable".to_owned())?;
        response
            .recv()
            .map_err(|_| "GUI application service stopped".to_owned())?
    }
}

impl GuiApplicationVisibilityBackend {
    fn request_hide(&self, hide: bool) -> Result<(), String> {
        let (reply, response) = mpsc::sync_channel(1);
        self.requests
            .send_blocking(GuiRequest::HideApp { hide, reply })
            .map_err(|_| "GUI application service is unavailable".to_owned())?;
        response
            .recv()
            .map_err(|_| "GUI application service stopped".to_owned())?
    }
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
    pub(super) legacy_grid_output: bool,
    pub(super) gui_requests: Option<smol::channel::Sender<GuiRequest>>,
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
    frame_sizes: HashMap<usize, (usize, usize)>,
    frame_runtime_sizes: HashMap<usize, (usize, usize)>,
    frame_geometries: HashMap<usize, GuiGeometry>,
    frame_grids: HashMap<usize, Grid>,
    latest_snapshots: Vec<FrameSnapshot>,
    active_frame_id: Option<usize>,
    primary_frame_id: Option<usize>,
    pending_frame_selection: Option<usize>,
    pending_frame_visibility: Option<(usize, bool)>,
    pending_frame_focus: Option<(usize, bool)>,
    pending_frame_moved: Option<(usize, i128, i128)>,
    pending_frame_delete: Option<usize>,
    pending_power_off: bool,
    pub(super) geometry_symbols: Option<GeometrySymbols>,
    pub(super) mouse_down: Option<(MouseButton, usize, i128, MousePosition)>,
    mouse_positions: HashMap<usize, MousePosition>,
    mouse_position: Option<MousePosition>,
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

fn frame_parameter_value(
    i: &Interp,
    frame: &remacs_runtime::editor::FrameRef,
    name: &str,
) -> Value {
    let Some(symbol) = i.intern_soft(name) else {
        return Value::Nil;
    };
    remacs_runtime::lisp::eval::plist_get_v(&frame.borrow().params, &Value::Sym(symbol))
}

fn frame_system_is_graphical(i: &Interp, frame: &remacs_runtime::editor::FrameRef) -> bool {
    match frame_parameter_value(i, frame, "window-system") {
        Value::Sym(system) => matches!(
            i.symbol_name(system).as_str(),
            "x" | "w32" | "ns" | "pgtk" | "haiku" | "android" | "gpui"
        ),
        _ => false,
    }
}

fn alpha_percent(value: &Value) -> Option<i128> {
    match value {
        Value::Int(value) => Some(*value),
        _ => None,
    }
}

fn frame_alpha(i: &Interp, frame: &remacs_runtime::editor::FrameRef) -> (u8, u8) {
    let lower_limit = i
        .intern_soft("frame-alpha-lower-limit")
        .filter(|symbol| i.bound_p(*symbol))
        .and_then(|symbol| i.symbol_value(symbol).int())
        .unwrap_or(20)
        .clamp(0, 100);
    let value = frame_parameter_value(i, frame, "alpha");
    let (active, inactive) = match value {
        Value::Int(value) => (Some(value), Some(value)),
        Value::Cons(pair) => {
            let pair = pair.borrow();
            let active = alpha_percent(&pair.car);
            let inactive_value = match &pair.cdr {
                Value::Cons(tail) => tail.borrow().car.clone(),
                value => value.clone(),
            };
            (active, alpha_percent(&inactive_value))
        }
        _ => (None, None),
    };
    let normalize = |value: Option<i128>, fallback: u8| {
        value
            .map(|value| value.clamp(lower_limit, 100) as u8)
            .unwrap_or(fallback)
    };
    let active = normalize(active, 100);
    let inactive = normalize(inactive, active);
    (active, inactive)
}

fn frame_alpha_background(i: &Interp, frame: &remacs_runtime::editor::FrameRef) -> u8 {
    match frame_parameter_value(i, frame, "alpha-background") {
        Value::Int(value) => value.clamp(0, 100) as u8,
        _ => 100,
    }
}

fn frame_ns_appearance(
    i: &Interp,
    frame: &remacs_runtime::editor::FrameRef,
) -> Option<gpui::WindowAppearance> {
    match frame_parameter_value(i, frame, "ns-appearance") {
        Value::Sym(symbol) => match i.symbol_name(symbol).as_str() {
            "dark" => Some(gpui::WindowAppearance::VibrantDark),
            "light" => Some(gpui::WindowAppearance::Light),
            _ => None,
        },
        _ => None,
    }
}

impl ChanIo {
    fn update_mouse_face_position(
        &mut self,
        frame_id: Option<usize>,
        position: Option<MousePosition>,
    ) {
        let changed = if let Some(frame_id) = frame_id {
            let previous = self.mouse_positions.get(&frame_id).copied();
            if same_mouse_face_position(previous, position) {
                false
            } else {
                if let Some(position) = position {
                    self.mouse_positions.insert(frame_id, position);
                } else {
                    self.mouse_positions.remove(&frame_id);
                }
                true
            }
        } else if same_mouse_face_position(self.mouse_position, position) {
            false
        } else {
            self.mouse_position = position;
            true
        };
        self.render_requested |= changed;
    }

    fn receive_event(&mut self, dur: Duration) -> io::Result<Option<GuiEvent>> {
        match self.events.recv_timeout(dur) {
            Ok(event) => Ok(Some(self.unwrap_frame_event(event))),
            Err(mpsc::RecvTimeoutError::Timeout) => Ok(None),
            Err(mpsc::RecvTimeoutError::Disconnected) => Err(io::Error::new(
                io::ErrorKind::BrokenPipe,
                "gui window closed",
            )),
        }
    }

    fn unwrap_frame_event(&mut self, event: GuiEvent) -> GuiEvent {
        match event {
            GuiEvent::ForFrame { frame_id, event } => {
                self.active_frame_id = Some(frame_id);
                self.pending_frame_selection = Some(frame_id);
                *event
            }
            event => event,
        }
    }

    fn update_size(&mut self, frame_id: Option<usize>, cols: usize, rows: usize) {
        let size = (cols.max(10), rows.max(3));
        if let Some(frame_id) = frame_id {
            self.frame_sizes.insert(frame_id, size);
            if Some(frame_id) == self.active_frame_id || Some(frame_id) == self.primary_frame_id {
                (self.cols, self.rows) = size;
            }
        } else {
            (self.cols, self.rows) = size;
        }
        self.render_requested = true;
    }

    fn record_frame_focus(&mut self, frame_id: usize, focused: bool) {
        self.pending_frame_focus = Some((frame_id, focused));
        self.render_requested = true;
        if focused {
            self.active_frame_id = Some(frame_id);
            self.pending_frame_selection = Some(frame_id);
        }
    }

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
        let changed = grid.blink_cursor || self.last.as_ref() != Some(&grid);
        if let Some(frame_id) = self.active_frame_id
            && let Some(snapshot) = self
                .latest_snapshots
                .iter_mut()
                .find(|snapshot| snapshot.frame_id == frame_id)
        {
            snapshot.grid = grid.clone();
            self.frame_grids.insert(frame_id, grid.clone());
            if let Some(updates) = &self.gui_requests {
                updates
                    .send_blocking(GuiRequest::FrameSnapshots(self.latest_snapshots.clone()))
                    .map_err(|_| {
                        io::Error::new(io::ErrorKind::BrokenPipe, "gui window closed")
                    })?;
            }
        }
        self.last = Some(grid.clone());
        if self.legacy_grid_output && changed {
            // Latest-wins: force_send displaces the oldest queued frame
            // when the bounded channel is full.
            self.grids
                .force_send(grid)
                .map_err(|_| io::Error::new(io::ErrorKind::BrokenPipe, "gui window closed"))?;
        }
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
        let Some(event) = self.receive_event(dur)? else {
            return Ok(None);
        };
        match event {
            GuiEvent::ForFrame { .. } => unreachable!("frame events are unwrapped"),
            GuiEvent::FrameDeleteRequest { frame_id } => {
                self.pending_frame_delete = Some(frame_id);
                self.render_requested = true;
                Ok(None)
            }
            GuiEvent::PowerOff => {
                self.pending_power_off = true;
                self.render_requested = true;
                Ok(None)
            }
            GuiEvent::FrameVisibility { frame_id, visible } => {
                self.pending_frame_visibility = Some((frame_id, visible));
                self.render_requested = true;
                Ok(None)
            }
            GuiEvent::ColorDrop {
                position,
                color,
                background,
            } => Ok(Some(InputEvent::ColorDrop {
                position,
                color,
                background,
            })),
            GuiEvent::FrameFocus { frame_id, focused } => {
                self.record_frame_focus(frame_id, focused);
                Ok(None)
            }
            GuiEvent::Key(k) => Ok(Some(InputEvent::Key(k))),
            GuiEvent::Text(text) => Ok(Some(InputEvent::Text(text))),
            GuiEvent::MenuCommand(command) => Ok(Some(InputEvent::Command(command))),
            GuiEvent::CommandArgs { command, args } => {
                Ok(Some(InputEvent::CommandArgs { command, args }))
            }
            GuiEvent::ReplaceText { range, text } => {
                Ok(Some(InputEvent::ReplaceText { range, text }))
            }
            GuiEvent::Drop {
                position,
                urls,
                action,
            } => Ok(Some(InputEvent::Drop {
                position,
                urls,
                action,
            })),
            GuiEvent::Resize(c, r) => {
                self.update_size(self.active_frame_id, c, r);
                Ok(None)
            }
            GuiEvent::Geometry(geometry) => {
                let frame_move = if let Some(frame_id) = self.active_frame_id {
                    let previous = self.frame_geometries.insert(frame_id, geometry.clone());
                    previous
                        .filter(|old| {
                            old.frame_left != geometry.frame_left
                                || old.frame_top != geometry.frame_top
                        })
                        .map(|_| frame_id)
                } else {
                    let previous = self.geometry.replace(geometry.clone());
                    previous
                        .filter(|old| {
                            old.frame_left != geometry.frame_left
                                || old.frame_top != geometry.frame_top
                        })
                        .and(self.active_frame_id)
                };
                self.update_size(self.active_frame_id, geometry.cols, geometry.rows);
                if let Some(frame_id) = frame_move {
                    Ok(Some(InputEvent::FrameMoved {
                        frame_id,
                        left: geometry.frame_left,
                        top: geometry.frame_top,
                    }))
                } else {
                    Ok(None)
                }
            }
            GuiEvent::MouseMove {
                frame_id,
                position,
            } => {
                self.update_mouse_face_position(frame_id, position);
                Ok(None)
            }
            GuiEvent::MouseDown {
                button,
                click_count,
                modifiers,
                position,
            } => {
                self.mouse_down = Some((button, click_count, modifiers, position));
                Ok(Some(InputEvent::MouseDown(MouseInput {
                    button,
                    click_count: click_count.max(1),
                    modifiers,
                    start: position,
                    end: position,
                })))
            }
            GuiEvent::MouseUp {
                button,
                click_count,
                modifiers,
                position,
            } => Ok(Some(InputEvent::Mouse(self.mouse_input(
                button,
                click_count,
                modifiers,
                position,
            )))),
            GuiEvent::Scroll {
                delta_lines,
                modifiers,
                position,
            } => {
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
            GuiEvent::ScrollBar {
                window_id,
                orientation,
                offset,
                max_offset,
            } => Ok(Some(InputEvent::ScrollBar {
                window_id,
                orientation,
                offset,
                max_offset,
            })),
        }
    }

    fn poll_key(&mut self, dur: Duration) -> io::Result<Option<i128>> {
        if let Some(k) = self.pending.pop() {
            return Ok(Some(k));
        }
        let Some(event) = self.receive_event(dur)? else {
            return Ok(None);
        };
        match event {
            GuiEvent::ForFrame { .. } => unreachable!("frame events are unwrapped"),
            GuiEvent::FrameDeleteRequest { frame_id } => {
                self.pending_frame_delete = Some(frame_id);
                self.render_requested = true;
                Ok(None)
            }
            GuiEvent::PowerOff => {
                self.pending_power_off = true;
                self.render_requested = true;
                Ok(None)
            }
            GuiEvent::FrameVisibility { frame_id, visible } => {
                self.pending_frame_visibility = Some((frame_id, visible));
                self.render_requested = true;
                Ok(None)
            }
            GuiEvent::ColorDrop { .. } => Ok(None),
            GuiEvent::FrameFocus { frame_id, focused } => {
                self.record_frame_focus(frame_id, focused);
                Ok(None)
            }
            GuiEvent::Key(k) => Ok(Some(k)),
            GuiEvent::Text(text) => {
                let mut chars = text.chars().map(|ch| ch as i128);
                let first = chars.next();
                self.pending.extend(chars.rev());
                Ok(first)
            }
            GuiEvent::MenuCommand(_) => Ok(None),
            GuiEvent::CommandArgs { .. } => Ok(None),
            GuiEvent::ReplaceText { text, .. } => {
                let mut chars = text.chars().map(|ch| ch as i128);
                let first = chars.next();
                self.pending.extend(chars.rev());
                Ok(first)
            }
            GuiEvent::Drop { .. } => Ok(None),
            GuiEvent::Resize(c, r) => {
                self.update_size(self.active_frame_id, c, r);
                Ok(None)
            }
            GuiEvent::Geometry(geometry) => {
                let moved_frame = if let Some(frame_id) = self.active_frame_id {
                    self.frame_geometries
                        .insert(frame_id, geometry.clone())
                        .filter(|old| {
                            old.frame_left != geometry.frame_left
                                || old.frame_top != geometry.frame_top
                        })
                        .map(|_| frame_id)
                } else {
                    self.geometry
                        .replace(geometry.clone())
                        .filter(|old| {
                            old.frame_left != geometry.frame_left
                                || old.frame_top != geometry.frame_top
                        })
                        .and(self.active_frame_id)
                };
                self.update_size(self.active_frame_id, geometry.cols, geometry.rows);
                if let Some(frame_id) = moved_frame {
                    self.pending_frame_moved = Some((
                        frame_id,
                        geometry.frame_left,
                        geometry.frame_top,
                    ));
                }
                Ok(None)
            }
            GuiEvent::MouseMove {
                frame_id,
                position,
            } => {
                self.update_mouse_face_position(frame_id, position);
                Ok(None)
            }
            GuiEvent::MouseDown {
                button,
                click_count,
                modifiers,
                position,
            } => {
                self.mouse_down = Some((button, click_count, modifiers, position));
                Ok(None)
            }
            GuiEvent::MouseUp { .. } => {
                self.mouse_down = None;
                Ok(None)
            }
            GuiEvent::ScrollBar { .. } => Ok(None),
            GuiEvent::Scroll { .. } => Ok(None),
        }
    }

    fn unread(&mut self, k: i128) {
        self.pending.push(k);
    }

    fn render(&mut self, i: &Interp) -> io::Result<()> {
        if let Some(frame) = i.selected_frame.as_ref() {
            self.active_frame_id = Some(frame.borrow().id);
        }
        if self.primary_frame_id.is_none() {
            self.primary_frame_id = i.frames.first().map(|frame| frame.borrow().id);
        }

        let live_frames: Vec<_> = i
            .frames
            .iter()
            .filter(|frame| !frame.borrow().dead)
            .cloned()
            .collect();
        let live_ids: std::collections::HashSet<_> =
            live_frames.iter().map(|frame| frame.borrow().id).collect();
        self.frame_grids.retain(|id, _| live_ids.contains(id));
        self.frame_runtime_sizes
            .retain(|id, _| live_ids.contains(id));
        self.mouse_positions.retain(|id, _| live_ids.contains(id));
        let all_frames = Value::list(live_frames.iter().cloned().map(Value::Frame).collect());
        let selected_id = i.selected_frame.as_ref().map(|frame| frame.borrow().id);
        let mut selected_grid = None;
        let mut snapshots = Vec::new();

        for frame in &live_frames {
            let (frame_id, frame_width, frame_height) = {
                let frame = frame.borrow();
                (frame.id, frame.width, frame.height)
            };
            let (mut width, mut height) = *self
                .frame_sizes
                .entry(frame_id)
                .or_insert((frame_width, frame_height));
            if (frame_width, frame_height) != (width, height) {
                let graphical =
                    self.gui_requests.is_some() && frame_system_is_graphical(i, frame);
                let last_runtime_size = self
                    .frame_runtime_sizes
                    .get(&frame_id)
                    .copied()
                    .unwrap_or((frame_width, frame_height));
                let runtime_requested_size = graphical
                    && (frame_width, frame_height) != last_runtime_size;
                if runtime_requested_size {
                    // Lisp changed the requested frame size. Keep it as the
                    // target size and let the native window report its actual
                    // cell dimensions through the ordinary resize event.
                    (width, height) = (frame_width, frame_height);
                    self.frame_sizes.insert(frame_id, (width, height));
                }
                remacs_runtime::editor::reflow_frame_windows(frame, width, height);
                if !runtime_requested_size {
                    let mut frame = frame.borrow_mut();
                    frame.width = width;
                    frame.height = height;
                    let mut params = frame.params.clone();
                    if let Some(parameter) = i.intern_soft("width") {
                        params = remacs_runtime::lisp::eval::plist_put_front(
                            &params,
                            &Value::Sym(parameter),
                            Value::Int(width as i128),
                        );
                    }
                    if let Some(parameter) = i.intern_soft("height") {
                        params = remacs_runtime::lisp::eval::plist_put_front(
                            &params,
                            &Value::Sym(parameter),
                            Value::Int(height as i128),
                        );
                    }
                    frame.params = params;
                }
            }
            let runtime_size = {
                let frame = frame.borrow();
                (frame.width, frame.height)
            };
            self.frame_runtime_sizes.insert(frame_id, runtime_size);

            let geometry = self.frame_geometries.get(&frame_id).cloned().or_else(|| {
                (Some(frame_id) == self.primary_frame_id)
                    .then(|| self.geometry.clone())
                    .flatten()
            });
            if let (Some(geometry), Some(keys)) = (geometry, self.geometry_symbols) {
                let mut monitors = Vec::with_capacity(geometry.monitors.len());
                for monitor in &geometry.monitors {
                    let monitor_frames = if geometry
                        .current_display_id
                        .is_none_or(|display_id| display_id == monitor.display_id)
                    {
                        all_frames.clone()
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
                    (keys.scale_factor, Value::float(geometry.scale_factor)),
                    (keys.monitor_attributes, Value::list(monitors)),
                ] {
                    params = remacs_runtime::lisp::eval::plist_put_front(
                        &params,
                        &Value::Sym(key),
                        value,
                    );
                }
                let left_parameter = Value::Sym(keys.frame_left);
                let current_left = frame_parameter_value(i, frame, "left");
                let left = if matches!(&current_left, Value::Int(left) if *left != geometry.frame_left) {
                    current_left
                } else {
                    Value::Int(geometry.frame_left)
                };
                params = remacs_runtime::lisp::eval::plist_put_front(
                    &params,
                    &left_parameter,
                    left,
                );
                let top_parameter = Value::Sym(keys.frame_top);
                let current_top = frame_parameter_value(i, frame, "top");
                let top = if matches!(&current_top, Value::Int(top) if *top != geometry.frame_top) {
                    current_top
                } else {
                    Value::Int(geometry.frame_top)
                };
                params = remacs_runtime::lisp::eval::plist_put_front(
                    &params,
                    &top_parameter,
                    top,
                );
                frame.borrow_mut().params = params;
            }

            if self.gui_requests.is_some() && frame_system_is_graphical(i, frame) {
                let cached = self
                    .frame_grids
                    .get(&frame_id)
                    .and_then(|grid| grid.input.as_ref());
                let mouse_position = self.mouse_positions.get(&frame_id).copied();
                let grid = render_frame_grid_with_mouse_position_cached(
                    i,
                    frame,
                    width,
                    height,
                    cached,
                    mouse_position,
                );
                if Some(frame_id) == selected_id {
                    selected_grid = Some(grid.clone());
                }
                self.frame_grids.insert(frame_id, grid.clone());
                let parent_frame = match frame_parameter_value(i, frame, "parent-frame") {
                    Value::Frame(parent) if !parent.borrow().dead => Some(parent.borrow().id),
                    _ => None,
                };
                let name = frame.borrow().name.borrow().to_string();
                let int_param = |name: &str| frame_parameter_value(i, frame, name).int();
                let (alpha_active, alpha_inactive) = frame_alpha(i, frame);
                let alpha_background = frame_alpha_background(i, frame);
                let fullscreen = match frame_parameter_value(i, frame, "fullscreen") {
                    Value::Sym(symbol) => Some(i.symbol_name(symbol)),
                    _ => None,
                };
                let z_group = match frame_parameter_value(i, frame, "z-group") {
                    Value::Sym(symbol) => Some(i.symbol_name(symbol)),
                    _ => None,
                };
                snapshots.push(FrameSnapshot {
                    frame_id,
                    name,
                    parent_frame,
                    modifier_mapping: super::keys::ModifierMapping::from_interp(i),
                    alpha_active,
                    alpha_inactive,
                    alpha_background,
                    width,
                    height,
                    left: int_param("left"),
                    top: int_param("top"),
                    undecorated: frame_parameter_value(i, frame, "undecorated").truthy(),
                    ns_transparent_titlebar: frame_parameter_value(
                        i,
                        frame,
                        "ns-transparent-titlebar",
                    )
                    .truthy(),
                    ns_appearance: frame_ns_appearance(i, frame),
                    fullscreen,
                    auto_raise: frame_parameter_value(i, frame, "auto-raise").truthy(),
                    auto_lower: frame_parameter_value(i, frame, "auto-lower").truthy(),
                    z_group,
                    grid,
                });
            }
        }

        self.latest_snapshots = snapshots.clone();
        if let Some(updates) = &self.gui_requests {
            updates
                .send_blocking(GuiRequest::FrameSnapshots(snapshots))
                .map_err(|_| {
                io::Error::new(io::ErrorKind::BrokenPipe, "gui window closed")
            })?;
        }

        let grid = match selected_grid {
            Some(grid) => grid,
            None => {
                let cached = self.last.as_ref().and_then(|grid| grid.input.as_ref());
                render_grid_with_mouse_position_cached(
                    i,
                    self.cols,
                    self.rows,
                    cached,
                    self.mouse_position,
                )
            }
        };
        self.send_grid(grid)
    }

    fn render_on_idle(&self) -> bool {
        self.last.as_ref().is_some_and(|grid| grid.blink_cursor)
    }

    fn take_render_request(&mut self) -> bool {
        std::mem::take(&mut self.render_requested)
    }

    fn take_frame_selection(&mut self) -> Option<usize> {
        self.pending_frame_selection.take()
    }

    fn take_frame_moved(&mut self) -> Option<(usize, i128, i128)> {
        self.pending_frame_moved.take()
    }

    fn take_frame_visibility_update(&mut self) -> Option<(usize, bool)> {
        self.pending_frame_visibility.take()
    }

    fn take_frame_focus_update(&mut self) -> Option<(usize, bool)> {
        self.pending_frame_focus.take()
    }

    fn take_frame_delete_request(&mut self) -> Option<usize> {
        self.pending_frame_delete.take()
    }

    fn take_power_off_request(&mut self) -> bool {
        std::mem::take(&mut self.pending_power_off)
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
        let (cols, rows) = self.size();
        let grid = if let Some(frame) = i.selected_frame.as_ref() {
            let frame_id = frame.borrow().id;
            let cached = self
                .frame_grids
                .get(&frame_id)
                .and_then(|grid| grid.input.as_ref());
            render_frame_grid_with_mouse_position_cached(
                i,
                frame,
                cols,
                rows,
                cached,
                self.mouse_positions.get(&frame_id).copied(),
            )
        } else {
            let cached = self.last.as_ref().and_then(|grid| grid.input.as_ref());
            render_grid_with_mouse_position_cached(i, cols, rows, cached, self.mouse_position)
        };
        let grid = self.echo_grid(grid, text, false, true);
        self.send_grid(grid)
    }

    fn size(&self) -> (usize, usize) {
        self.active_frame_id
            .and_then(|frame_id| self.frame_sizes.get(&frame_id).copied())
            .unwrap_or((self.cols, self.rows))
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
    let (events, grids, _gui_requests, _clipboard_requests, _initial_frame_size) =
        start_logic_inner(files, None, Vec::new(), false, true, false)?;
    Ok((events, grids))
}

#[cfg(test)]
pub(super) fn start_logic_with_dump_for_test(
    files: Vec<String>,
    dump_file: Option<String>,
    arguments: Vec<String>,
) -> io::Result<(Sender<GuiEvent>, smol::channel::Receiver<Grid>)> {
    let (events, grids, _gui_requests, _clipboard_requests, _initial_frame_size) =
        start_logic_inner(files, dump_file, arguments, false, true, false)?;
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
    let (events, grids, _gui_requests, clipboard, _initial_frame_size) =
        start_logic_inner(files, dump_file, arguments, true, true, false)?;
    Ok((events, grids, clipboard))
}

pub(super) fn start_logic_with_frames(
    files: Vec<String>,
    dump_file: Option<String>,
    arguments: Vec<String>,
) -> io::Result<(
    Sender<GuiEvent>,
    smol::channel::Receiver<GuiRequest>,
    smol::channel::Receiver<ClipboardRequest>,
    Option<InitialFrameSize>,
)> {
    let (events, grids, gui_requests, clipboard, initial_frame_size_rx) =
        start_logic_inner(files, dump_file, arguments, true, false, true)?;
    drop(grids);
    let initial_frame_size = initial_frame_size_rx
        .recv()
        .map_err(|_| io::Error::other("GUI startup frame size is unavailable"))?;
    Ok((events, gui_requests, clipboard, initial_frame_size))
}

fn start_logic_inner(
    files: Vec<String>,
    dump_file: Option<String>,
    arguments: Vec<String>,
    enable_gui_selections: bool,
    legacy_grid_output: bool,
    enable_gui_requests: bool,
) -> io::Result<(
    Sender<GuiEvent>,
    smol::channel::Receiver<Grid>,
    smol::channel::Receiver<GuiRequest>,
    smol::channel::Receiver<ClipboardRequest>,
    mpsc::Receiver<Option<InitialFrameSize>>,
)> {
    let wait_for_dump = dump_file.is_some();
    let (startup_tx, startup_rx) = mpsc::channel::<Result<(), String>>();
    let (event_tx, event_rx) = mpsc::channel::<GuiEvent>();
    let (grid_tx, grid_rx) = smol::channel::bounded::<Grid>(1);
    let (gui_requests_tx, gui_requests_rx) = smol::channel::unbounded::<GuiRequest>();
    let (clipboard_tx, clipboard_rx) = smol::channel::unbounded::<ClipboardRequest>();
    let (initial_frame_size_tx, initial_frame_size_rx) = mpsc::sync_channel(1);
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
                        let _ = initial_frame_size_tx.send(None);
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
                let _ = initial_frame_size_tx.send(None);
                return;
            }
            if enable_gui_selections {
                configure_gui_window_system(&mut i);
            }
            if enable_gui_requests {
                i.set_bell_backend(Some(Rc::new(GuiBellBackend {
                    requests: gui_requests_tx.clone(),
                })));
                #[cfg(target_os = "macos")]
                i.set_about_panel_backend(Some(Rc::new(GuiAboutPanelBackend {
                    requests: gui_requests_tx.clone(),
                })));
                i.set_dialog_backend(Some(Rc::new(GuiDialogBackend {
                    requests: gui_requests_tx.clone(),
                })));
                i.set_file_dialog_backend(Some(Rc::new(GuiFileDialogBackend {
                    requests: gui_requests_tx.clone(),
                })));
                i.set_frame_window_backend(Some(Rc::new(GuiFrameVisibilityBackend {
                    requests: gui_requests_tx.clone(),
                })));
                i.set_color_panel_backend(Some(Rc::new(GuiColorPanelBackend {
                    requests: gui_requests_tx.clone(),
                })));
                i.set_application_visibility_backend(Some(Rc::new(
                    GuiApplicationVisibilityBackend {
                        requests: gui_requests_tx.clone(),
                    },
                )));
                #[cfg(target_os = "macos")]
                i.set_drag_backend(Some(Rc::new(GuiDragBackend {
                    requests: gui_requests_tx.clone(),
                })));
            }
            let initial_frame_size = i.selected_frame.as_ref().and_then(|frame| {
                let has_size_parameter = [
                    "initial-frame-alist",
                    "default-frame-alist",
                    "frame-initial-frame-alist",
                ]
                .iter()
                .any(|name| {
                    i.intern_soft(name).is_some_and(|symbol| {
                        let alist = i.symbol_value(symbol);
                        frame_alist_sets_size(&i, &alist)
                    })
                });
                has_size_parameter.then(|| {
                    let frame = frame.borrow();
                    InitialFrameSize {
                        width: frame.width,
                        height: frame.height,
                    }
                })
            });
            let _ = initial_frame_size_tx.send(initial_frame_size);
            let _ = startup_tx.send(Ok(()));
            let selected_frame_id = i.selected_frame.as_ref().map(|frame| frame.borrow().id);
            let mut frame_sizes = HashMap::new();
            if let Some(frame_id) = selected_frame_id {
                if let Some(frame) = &i.selected_frame {
                    let frame = frame.borrow();
                    frame_sizes.insert(frame_id, (frame.width, frame.height));
                }
            }
            let io = ChanIo {
                events: event_rx,
                grids: grid_tx,
                legacy_grid_output,
                gui_requests: enable_gui_requests.then_some(gui_requests_tx),
                pending: Vec::new(),
                cols: 80,
                rows: 24,
                last: None,
                render_requested: false,
                geometry: None,
                frame_sizes,
                frame_runtime_sizes: HashMap::new(),
                frame_geometries: HashMap::new(),
                frame_grids: HashMap::new(),
                latest_snapshots: Vec::new(),
                active_frame_id: selected_frame_id,
                primary_frame_id: selected_frame_id,
                pending_frame_selection: None,
                pending_frame_visibility: None,
                pending_frame_focus: None,
                pending_frame_moved: None,
                pending_frame_delete: None,
                pending_power_off: false,
                geometry_symbols: if enable_gui_selections {
                    Some(GeometrySymbols::intern(&mut i))
                } else {
                    None
                },
                mouse_down: None,
                mouse_positions: HashMap::new(),
                mouse_position: None,
                scroll_remainder: 0.0,
            };
            let _ = remacs_runtime::frontend::run_editor_with(io, &mut i);
            // The GUI request sender drops here → the UI pump quits the app.
        })
        .map_err(io::Error::other)?;
    if wait_for_dump {
        startup_rx
            .recv()
            .map_err(io::Error::other)?
            .map_err(io::Error::other)?;
    }
    Ok((
        event_tx,
        grid_rx,
        gui_requests_rx,
        clipboard_rx,
        initial_frame_size_rx,
    ))
}

fn configure_gui_window_system(i: &mut Interp) {
    let window_system = i.intern("window-system");
    let initial_window_system = i.intern("initial-window-system");
    #[cfg(target_os = "macos")]
    let system = "ns";
    #[cfg(not(target_os = "macos"))]
    let system = "gpui";
    let value = remacs_runtime::lisp::Value::Sym(i.intern(system));
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

fn frame_alist_sets_size(i: &Interp, alist: &Value) -> bool {
    let Ok(entries) = alist.list_to_vec() else {
        return false;
    };
    entries.into_iter().any(|entry| {
        let Value::Cons(entry) = entry else {
            return false;
        };
        let (key, value) = {
            let entry = entry.borrow();
            (entry.car.clone(), entry.cdr.clone())
        };
        let Some(key) = i.sym_id(&key) else {
            return false;
        };
        match i.symbol_name(key).as_str() {
            "width" => value.int().is_some_and(|width| width > 0),
            "height" => value.int().is_some_and(|height| height > 1),
            _ => false,
        }
    })
}
