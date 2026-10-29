//! Frontend-independent rendering, input protocol, and editor command loop.
//!
//! Non-blocking design: the loop polls input with a timeout, redraws
//! after input or runtime activity, and executes commands. Front-ends
//! can request periodic idle renders when they need them. Long Lisp
//! operations can interleave with repaints because `sit-for`/`sleep-for`
//! hand control back to this loop rather than blocking inside the evaluator.

use std::io;
use std::ops::Range;
use std::time::Duration;

use crate::lisp::Interp;

/// Platform clipboard access used by window-system selection primitives.
pub trait ClipboardBackend {
    fn read_text(&self) -> Result<Option<String>, String>;
    fn write_text(&self, text: String) -> Result<(), String>;
}

/// Host bell used by graphical front-ends for audible and visible feedback.
pub trait BellBackend {
    fn ring_bell(&self, frame_id: Option<usize>, visible: bool);
}

/// Native About panel used by graphical front-ends.
pub trait AboutPanelBackend {
    fn show(&self, frame_id: usize, version: &str) -> Result<(), String>;
}

/// Host dialog backend used by interactive graphical front-ends. The logic
/// thread blocks until the UI returns the selected button index.
pub trait DialogBackend {
    fn choose(
        &self,
        frame_id: usize,
        message: &str,
        detail: Option<&str>,
        buttons: &[String],
    ) -> Result<Option<usize>, String>;
}

/// Host file panel used by graphical file-name primitives.
pub trait FileDialogBackend {
    fn read_file_name(
        &self,
        prompt: &str,
        directory: Option<&str>,
        must_match: bool,
        initial: Option<&str>,
        directory_only: bool,
    ) -> Result<Option<String>, String>;
}

/// Host font chooser used by graphical font-selection primitives. The
/// implementation may block the logic thread until the user selects or
/// dismisses the native chooser.
pub trait FontSelectionBackend {
    fn select_font(&self, frame_id: usize, current_font: &str) -> Result<Option<String>, String>;
}

/// Host native window bridge used by GUI frame primitives.
pub trait FrameWindowBackend {
    fn set_visible(&self, frame_id: usize, visible: bool) -> Result<(), String>;
    fn raise(&self, frame_id: usize) -> Result<(), String>;
    fn lower(&self, frame_id: usize) -> Result<(), String>;
    /// Move the pointer to pixel coordinates relative to FRAME's native area.
    fn warp_pointer_in_frame(
        &self,
        frame_id: usize,
        x: i128,
        y: i128,
    ) -> Result<(), String>;
    /// Move the pointer to pixel coordinates relative to FRAME's display.
    fn warp_pointer_absolute(
        &self,
        frame_id: usize,
        x: i128,
        y: i128,
    ) -> Result<(), String>;
}

/// Host color panel used by native GUI faces.
pub trait ColorPanelBackend {
    fn show(&self, frame_id: usize) -> Result<(), String>;
    fn show_character_palette(&self, frame_id: usize) -> Result<(), String>;
}

/// Operation requested for an outgoing drag-and-drop session.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DragAction {
    /// Copy the dragged data at the destination.
    Copy,
    /// Move the dragged data to the destination.
    Move,
    /// Create a link to the dragged data at the destination.
    Link,
    /// The drop target chose an unspecified action.
    Private,
}

/// Host operating-system bridge for outgoing GUI drags.
pub trait DragBackend {
    /// Begin dragging TEXT from FRAME and wait for the destination's action.
    fn begin_text_drag(
        &self,
        frame_id: usize,
        text: &str,
        action: DragAction,
    ) -> Result<Option<DragAction>, String>;

    /// Begin dragging file paths from FRAME and wait for the destination's action.
    fn begin_file_drag(
        &self,
        frame_id: usize,
        paths: &[String],
        action: DragAction,
    ) -> Result<Option<DragAction>, String>;
}

/// Host application visibility bridge for NS hide commands.
pub trait ApplicationVisibilityBackend {
    fn hide(&self) -> Result<(), String>;
    fn activate(&self) -> Result<(), String>;
    fn hide_other_apps(&self) -> Result<(), String>;
    fn set_dock_badge(&self, badge: Option<&str>) -> Result<(), String>;
    fn set_dock_progress(&self, progress: Option<f64>) -> Result<(), String>;
    fn request_user_attention(&self, urgency: Option<UserAttentionUrgency>) -> Result<(), String>;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UserAttentionUrgency {
    Informational,
    Critical,
}

/// Result of executing an AppleScript from a native GUI front-end.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AppleScriptResult {
    Nil,
    String(String),
    Integer(i128),
    Other,
}

/// Native AppleScript bridge used by the macOS GUI front-end.
pub trait AppleScriptBackend {
    fn execute(&self, script: &str) -> Result<AppleScriptResult, String>;
}

/// Native macOS Services bridge used by NS frontend primitives.
pub trait NativeServiceBackend {
    fn perform_service(&self, service: &str, send: Option<&str>) -> Result<Option<String>, String>;
}

/// Input from a front-end: key events, committed text, or mouse actions.
pub enum InputEvent {
    Key(i128),
    Text(String),
    /// Execute a command selected from a native GUI menu.
    Command(String),
    /// Execute a GUI action with pre-supplied interactive arguments.
    CommandArgs { command: String, args: Vec<i128> },
    ReplaceText { range: Range<usize>, text: String },
    Drop {
        position: Option<MousePosition>,
        urls: Vec<String>,
        action: String,
    },
    /// Files opened by the operating system, optionally at a requested line.
    OpenFiles(Vec<(String, Option<usize>)>),
    /// A request from a native macOS Services provider.
    NativeService { name: String, argument: String },
    /// Apply a color dragged from the native color panel to the face under the pointer.
    ColorDrop {
        position: Option<MousePosition>,
        color: String,
        background: bool,
    },
    MouseDown(MouseInput),
    Mouse(MouseInput),
    /// Pointer movement while a mouse button is held.
    MouseMovement { position: Option<MousePosition> },
    /// Pointer movement outside a drag, including movement over blank areas.
    MouseMoved {
        frame_id: Option<usize>,
        /// Frame-relative character-cell coordinates for `mouse-position`.
        frame_position: Option<(i128, i128)>,
        /// Frame-relative pixel coordinates for `mouse-pixel-position`.
        frame_pixel_position: Option<(i128, i128)>,
        position: Option<MousePosition>,
    },
    /// A wheel/trackpad event, with line count normalized by the front-end.
    MouseWheel {
        up: bool,
        modifiers: i128,
        position: MousePosition,
        line_count: usize,
    },
    /// A native window manager moved a graphical frame.
    FrameMoved {
        frame_id: usize,
        left: i128,
        top: i128,
    },
    /// Move a window's vertical or horizontal scroll bar to a logical offset.
    ScrollBar {
        window_id: usize,
        orientation: GridScrollBarOrientation,
        offset: usize,
        max_offset: usize,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MouseButton {
    Left,
    Middle,
    Right,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MouseArea {
    ModeLine,
}

/// A text position mapped from the front-end's most recent grid snapshot.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MousePosition {
    pub buffer_id: usize,
    /// Zero-based character position in BUFFER.
    pub buffer_position: usize,
    /// UTF-16 offset used by the native text input APIs.
    pub utf16_offset: usize,
    /// Pixel coordinates relative to the front-end's text area.
    pub x: i128,
    pub y: i128,
    /// Display row and character column.
    pub row: usize,
    pub column: usize,
    /// Target window for non-buffer areas such as mode lines.
    pub window_id: Option<usize>,
    /// Window area under the pointer; nil denotes buffer text.
    pub area: Option<MouseArea>,
    /// The pointer is over the mode-line buffer-identification field.
    pub mode_line_buffer_name: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MouseInput {
    pub button: MouseButton,
    pub click_count: usize,
    /// Emacs keyboard modifier bits for the event.
    pub modifiers: i128,
    pub start: MousePosition,
    pub end: MousePosition,
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
    /// Run the pre-redisplay callback for a specific set of windows.
    fn run_pre_redisplay_for_windows(
        &mut self,
        i: &mut Interp,
        windows: Vec<crate::editor::WindowRef>,
    ) -> Result<(), crate::lisp::error::Flow> {
        crate::editor::winxtra::run_pre_redisplay_function(i, windows)
    }
    /// Run the configured pre-redisplay function for the windows this
    /// front-end will draw. Multi-window front-ends can override this.
    fn run_pre_redisplay_functions(
        &mut self,
        i: &mut Interp,
    ) -> Result<(), crate::lisp::error::Flow> {
        let windows = i
            .selected_frame
            .as_ref()
            .map(crate::editor::changes::frame_windows)
            .unwrap_or_default();
        self.run_pre_redisplay_for_windows(i, windows)
    }
    /// Repaint the whole frame.
    fn render(&mut self, i: &Interp) -> io::Result<()>;
    /// Most recently observed native pointer position, in the display-relative
    /// pixel coordinates of the selected frame, when the frontend has one.
    fn mouse_absolute_pixel_position(&self, _i: &Interp) -> Option<(i128, i128)> {
        None
    }
    /// Whether this front-end needs periodic renders without input or
    /// runtime activity (for example, a terminal cursor animation).
    fn render_on_idle(&self) -> bool {
        true
    }
    /// Consume a front-end event that requires redisplay without being
    /// represented as a key or text input, such as a resize.
    fn take_render_request(&mut self) -> bool {
        false
    }
    /// Frame selected by the most recent native input or focus-in event.
    /// Multi-window front-ends use this to select the corresponding Lisp
    /// frame before handling that event. Single-window front-ends inherit
    /// the default.
    fn take_frame_selection(&mut self) -> Option<usize> {
        None
    }
    /// Native frame position change consumed by a key-only nested loop.
    fn take_frame_moved(&mut self) -> Option<(usize, i128, i128)> {
        None
    }
    /// Visibility change reported by the native window manager.
    fn take_frame_visibility_update(&mut self) -> Option<(usize, bool)> {
        None
    }
    /// Focus change reported by the native window manager.
    fn take_frame_focus_update(&mut self) -> Option<(usize, bool)> {
        None
    }
    /// Frame whose native window received a close request.
    fn take_frame_delete_request(&mut self) -> Option<usize> {
        None
    }
    /// Whether the operating system is asking the editor to save and quit.
    fn take_power_off_request(&mut self) -> bool {
        false
    }
    /// Show TEXT in the echo area (minibuffer prompt/status).
    fn draw_echo(&mut self, text: &str) -> io::Result<()>;
    /// Show minibuffer text while retaining front-end style information.
    /// Key-only front-ends inherit the plain echo-area implementation.
    fn draw_echo_with_state(&mut self, _i: &Interp, text: &str) -> io::Result<()> {
        self.draw_echo(text)
    }
    /// Show an echo-area message while preserving the displayed buffer cursor
    /// unless `cursor-in-echo-area` requests moving it into the message.
    fn draw_echo_message_with_state(&mut self, _i: &Interp, text: &str) -> io::Result<()> {
        self.draw_echo(text)
    }
    /// Current frame size in text cells.
    fn size(&self) -> (usize, usize);
}

mod command;
mod render;
pub(crate) use command::minibuf_dispatch_key;
pub use command::{
    batch_command_error, dispatch_key_core, install_minibuf_reader, nonblocking_sleep,
    replay_macro_events, run_batch_editor, run_editor_with, run_editor_with_shared,
};
pub use render::{
    BoxStyle, BoxStyleKind, CellColor, CellStyle, Grid, GridBottomDivider, GridCursor,
    GridCursorShape, GridHelpEcho, GridImage, GridImageSource, GridInput, GridInputRow,
    GridBidiDirection, GridBidiDisabledRange, GridBidiParagraphRange, GridComposition,
    GridInputSelection, GridMenu, GridPixelwiseSegment, GridStipple, GridPointerShape,
    GridModeLine, GridVScroll,
    GridMenuItem, GridPopupMenuItem, GridScrollBar, GridScrollBarOrientation, GridTabBarItem,
    GridToolBarItem, fit_text_to_cells, render_frame_grid_with_input_cached,
    render_frame_grid_with_mouse_position_cached, render_grid, render_grid_with_input_cached,
    render_grid_with_mouse_position_cached,
};
pub(crate) use render::{position_at_display_cell, position_display_cell};
pub(crate) use render::buffer_setting;
pub(crate) use render::visible_fontification_range;
