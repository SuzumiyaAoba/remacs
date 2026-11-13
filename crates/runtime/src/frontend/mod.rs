//! Frontend-independent rendering, input protocol, and editor command loop.
//!
//! Non-blocking design: the loop polls input with a timeout, redraws
//! after input or runtime activity, and executes commands. Front-ends
//! can request periodic idle renders when they need them. Long Lisp
//! operations can interleave with repaints because `sit-for`/`sleep-for`
//! hand control back to this loop rather than blocking inside the evaluator.

use std::io;
use std::ops::Range;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use crate::lisp::{Interp, value::SymId};

/// Clipboard metadata exposed to Lisp selection queries.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ClipboardInfo {
    /// Whether the host clipboard currently advertises any data types.
    pub has_data: bool,
    /// Lisp-facing target names for the host clipboard's known data types.
    pub targets: Vec<String>,
}

/// Platform clipboard access used by window-system selection primitives.
pub trait ClipboardBackend {
    fn read_text(&self) -> Result<Option<String>, String>;
    fn write_text(&self, text: String) -> Result<(), String>;

    /// Return native clipboard metadata when the host exposes it.
    fn clipboard_info(&self) -> Result<Option<ClipboardInfo>, String> {
        Ok(None)
    }

    /// Return the host clipboard generation when the platform exposes one.
    /// Implementations use this to distinguish identical data from a new owner.
    fn clipboard_change_count(&self) -> Result<Option<u64>, String> {
        Ok(None)
    }

    /// Write clipboard text and return the generation produced by that write.
    /// GUI backends should perform both operations on their UI thread.
    fn write_text_with_change_count(&self, text: String) -> Result<Option<u64>, String> {
        self.write_text(text)?;
        self.clipboard_change_count()
    }

    /// Clear the clipboard only when it still has the generation we own.
    fn clear_text_if_change_count(&self, expected: u64) -> Result<(), String> {
        if self.clipboard_change_count()? == Some(expected) {
            self.write_text(String::new())?;
        }
        Ok(())
    }

    /// Whether the host has a PRIMARY selection separate from its clipboard.
    fn supports_primary_selection(&self) -> bool {
        false
    }

    fn read_primary_text(&self) -> Result<Option<String>, String> {
        Err("PRIMARY selection is unavailable".to_owned())
    }

    fn write_primary_text(&self, _text: String) -> Result<(), String> {
        Err("PRIMARY selection is unavailable".to_owned())
    }
}

/// Host bell used by graphical front-ends for audible and visible feedback.
pub trait BellBackend {
    fn ring_bell(&self, frame_id: Option<usize>, visible: bool);
}

/// Native About panel used by graphical front-ends.
pub trait AboutPanelBackend {
    fn show(&self, frame_id: usize, version: &str) -> Result<(), String>;
}

/// A selectable choice or nonselectable text in a graphical dialog.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DialogItem {
    Button(String),
    Text(String),
    /// Boundary between the left and right dialog groups.
    Separator,
}

/// Layout origin used by a popup menu's pixel coordinates.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PopupMenuAnchor {
    Frame,
    Window(usize),
    TextArea(usize),
}

/// A popup menu's target frame, layout origin, and pixel offsets.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PopupMenuPosition {
    pub frame_id: usize,
    pub anchor: PopupMenuAnchor,
    pub from_event: bool,
    pub x: i128,
    pub y: i128,
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

    /// Choose with a Question (false) or Information (true) dialog header.
    /// The default forwards to the existing chooser for older backends.
    fn choose_with_header(
        &self,
        frame_id: usize,
        message: &str,
        detail: Option<&str>,
        buttons: &[String],
        _header: bool,
    ) -> Result<Option<usize>, String> {
        self.choose(frame_id, message, detail, buttons)
    }

    /// Choose from structured items. Returned indices count only Button items.
    /// Older backends receive the selectable labels through their existing API.
    fn choose_with_items(
        &self,
        frame_id: usize,
        message: &str,
        detail: Option<&str>,
        items: &[DialogItem],
        header: bool,
    ) -> Result<Option<usize>, String> {
        let buttons: Vec<_> = items
            .iter()
            .filter_map(|item| match item {
                DialogItem::Button(label) => Some(label.clone()),
                DialogItem::Text(_) | DialogItem::Separator => None,
            })
            .collect();
        if buttons.is_empty() {
            return Ok(None);
        }
        self.choose_with_header(frame_id, message, detail, &buttons, header)
    }
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

/// Font-family names available to a graphical front-end. Implementations
/// may query the native text system on its owning UI thread.
pub trait FontFamilyListBackend {
    fn list_families(&self, frame_id: usize) -> Result<Vec<String>, String>;
}

/// Requested native visibility of a graphical frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FrameVisibility {
    Visible,
    Hidden,
    Iconified,
}

/// Host native window bridge used by GUI frame primitives.
pub trait FrameWindowBackend {
    /// Make FRAME available to a synchronous native operation. A GUI can
    /// publish its current frame state and wait until the window is created.
    fn prepare_frame(&self, _interp: &Interp, _frame_id: usize) -> Result<(), String> {
        Ok(())
    }
    fn set_visible(&self, frame_id: usize, visible: bool) -> Result<(), String>;
    fn set_visibility(&self, frame_id: usize, visibility: FrameVisibility) -> Result<(), String> {
        self.set_visible(frame_id, visibility == FrameVisibility::Visible)
    }
    fn raise(&self, frame_id: usize) -> Result<(), String>;
    fn lower(&self, frame_id: usize) -> Result<(), String>;
    /// Return native frame IDs from topmost to bottommost.
    fn frame_list_z_order(&self) -> Result<Vec<usize>, String> {
        Err("native frame stacking order is unavailable".to_owned())
    }
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

/// A quit request raised by a front-end thread while the interpreter is busy.
///
/// The pending count is consumed by `Interp::maybe_quit`. Requests consumed
/// there correspond to key events that are still in the front-end queue, so
/// `KeyIo` suppresses those events when it eventually receives them. Requests
/// whose key events reach `KeyIo` first are cleared there and keep their normal
/// command-loop meaning.
#[derive(Debug, Default)]
pub struct QuitRequest {
    pending: AtomicUsize,
    consumed: AtomicUsize,
}

impl QuitRequest {
    /// Record a quit key received by the front-end.
    pub fn request(&self) {
        let _ = self
            .pending
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |n| {
                Some(n.saturating_add(1))
            });
    }

    /// Transfer all pending host requests to the interpreter and arrange for
    /// their queued key events to be discarded later.
    pub(crate) fn take_for_eval(&self) -> bool {
        let count = self.pending.swap(0, Ordering::AcqRel);
        if count == 0 {
            return false;
        }
        let _ = self
            .consumed
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |n| {
                Some(n.saturating_add(count))
            });
        true
    }

    /// Return true when this key event belongs to a quit already delivered to
    /// the interpreter. Otherwise clear its not-yet-delivered request.
    pub fn discard_queued_key(&self) -> bool {
        if decrement_if_positive(&self.consumed) {
            true
        } else {
            decrement_if_positive(&self.pending);
            false
        }
    }
}

fn decrement_if_positive(counter: &AtomicUsize) -> bool {
    counter
        .fetch_update(Ordering::AcqRel, Ordering::Acquire, |n| n.checked_sub(1))
        .is_ok()
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
    /// Begin dragging the supplied TEXT and file PATHS from FRAME, then wait
    /// for the destination's action.
    fn begin_drag(
        &self,
        frame_id: usize,
        text: Option<&str>,
        paths: &[String],
        action: DragAction,
        follow_tooltip: bool,
        tooltip: Option<&str>,
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

/// Host GUI resource database used by `x-get-resource`.
pub trait XResourceBackend {
    fn get_string_resource(&self, name: &str, class: &str) -> Result<Option<String>, String>;
}

/// Direction of a wheel or trackpad scroll event.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MouseWheelDirection {
    Up,
    Down,
    Left,
    Right,
}

/// Input from a front-end: key events, committed text, or mouse actions.
pub enum InputEvent {
    Key(i128),
    Text(String),
    /// Execute a command selected from a native GUI menu.
    Command(String),
    /// Execute the command object retained by a native menu snapshot.
    NativeMenuCommand(NativeMenuCommandId),
    /// A choice or dismissal of a specific popup, never a command dispatch.
    PopupMenuSelection { menu_id: usize, index: Option<usize> },
    /// Scroll a specific popup without dispatching a buffer wheel command.
    PopupMenuScroll { menu_id: usize, delta_lines: f32 },
    /// Point at an item in one displayed popup level without selecting it.
    PopupMenuHover { menu_id: usize, parent: Option<usize>, index: Option<usize> },
    /// Scroll a specific displayed submenu; None denotes the root level.
    PopupMenuScrollAt { menu_id: usize, parent: Option<usize>, delta_lines: f32 },
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
    /// A native system sleep or wake event.
    SystemSleep { sleeping: bool },
    /// Apply a color dragged from the native color panel to the face under the pointer.
    ColorDrop {
        position: Option<MousePosition>,
        color: String,
        background: bool,
    },
    MouseDown(MouseInput),
    Mouse(MouseInput),
    /// Pointer movement while a mouse button is held.
    MouseMovement {
        position: Option<MousePosition>,
        /// Emacs keyboard modifier bits held while the pointer moved.
        modifiers: i128,
    },
    /// Pointer movement outside a drag, including movement over blank areas.
    MouseMoved {
        frame_id: Option<usize>,
        /// Window whose visible text rectangle contains the pointer, including blank space.
        text_window_id: Option<usize>,
        /// Frame-relative character-cell coordinates for `mouse-position`.
        frame_position: Option<(i128, i128)>,
        /// Frame-relative pixel coordinates for `mouse-pixel-position`.
        frame_pixel_position: Option<(i128, i128)>,
        position: Option<MousePosition>,
        /// Emacs keyboard modifier bits held while the pointer moved.
        modifiers: i128,
    },
    /// A wheel/trackpad event, with direction and available deltas from the front-end.
    MouseWheel {
        direction: MouseWheelDirection,
        modifiers: i128,
        position: MousePosition,
        /// Whole screen lines, or None for a pixel-only movement.
        line_count: Option<usize>,
        /// Pixel delta (x, y) from a precise wheel/trackpad event.
        pixel_delta: Option<(i128, i128)>,
    },
    /// A touchpad gesture ended, distinct from the final wheel delta.
    TouchEnd {
        modifiers: i128,
        position: MousePosition,
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
        /// Mouse click or completed drag associated with this scroll update.
        mouse: Option<MouseInput>,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MouseButton {
    Left,
    Middle,
    Right,
    /// GUI back navigation button, delivered as mouse-8.
    Back,
    /// GUI forward navigation button, delivered as mouse-9.
    Forward,
}

impl MouseButton {
    fn number(self) -> u8 {
        match self {
            Self::Left => 1,
            Self::Middle => 2,
            Self::Right => 3,
            Self::Back => 8,
            Self::Forward => 9,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MouseArea {
    ModeLine,
    HeaderLine,
    TabLine,
    LeftMargin,
    RightMargin,
    VerticalScrollBar,
    HorizontalScrollBar,
    ImageMap(SymId),
}

/// Identifies the Lisp value rendered as a mouse event object. These are
/// stable references into the source buffer snapshot, rather than Lisp values,
/// so a position can cross the GUI thread boundary safely.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MouseObjectSource {
    TextProperty { position: usize, property: SymId },
    OverlayProperty { index: usize, property: SymId },
}

/// Lisp object and character position under a mouse event.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MouseObject {
    Image { source: MouseObjectSource },
    String {
        source: MouseObjectSource,
        position: usize,
    },
}

/// A text position mapped from the front-end's most recent grid snapshot.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MousePosition {
    pub buffer_id: usize,
    /// Zero-based character position in BUFFER.
    pub buffer_position: usize,
    /// UTF-16 offset used by the native text input APIs.
    pub utf16_offset: usize,
    /// Pixel coordinates relative to the target window's text area.
    pub x: i128,
    pub y: i128,
    /// Display row and character column.
    pub row: usize,
    pub column: usize,
    /// Target window for nonselected buffer rows or non-buffer areas such as
    /// mode lines.
    pub window_id: Option<usize>,
    /// Window area under the pointer; nil denotes buffer text.
    pub area: Option<MouseArea>,
    /// Display image or string under the pointer, if any.
    pub object: Option<MouseObject>,
    /// Milliseconds since this front-end's monotonic mouse-event epoch.
    pub timestamp: i128,
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
    /// Poll keyboard and click events in a nested search loop. Front-ends
    /// retain their key-polling policy for other events, such as deferring
    /// operating-system file-open requests until the outer command loop.
    fn poll_key_or_mouse(&mut self, dur: Duration) -> io::Result<Option<InputEvent>> {
        self.poll_key(dur).map(|key| key.map(InputEvent::Key))
    }
    /// A close/shutdown request that must release a nested popup input loop.
    fn popup_should_cancel(&self) -> bool {
        false
    }
    /// Push a key back onto the pending queue.
    fn unread(&mut self, k: i128);
    /// Discard currently pending frontend input without waiting for new input.
    fn discard_input(&mut self) -> io::Result<()> {
        while self.poll_input(Duration::ZERO)?.is_some() {}
        Ok(())
    }
    /// Run the pre-redisplay callback for a specific set of windows.
    fn run_pre_redisplay_for_windows(
        &mut self,
        i: &mut Interp,
        windows: Vec<crate::editor::WindowRef>,
    ) -> Result<(), crate::lisp::error::Flow> {
        crate::editor::winxtra::run_pre_redisplay_function(i, windows.clone())?;
        mode_line::prepare(i, &windows)?;
        render::prepare_mode_line_remote(i, windows)
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
    /// Repaint with mutable runtime access for redisplay forms that evaluate Lisp.
    /// Existing front-ends keep the read-only renderer unless they override this.
    fn render_mut(&mut self, i: &mut Interp) -> io::Result<()> {
        let _condition_cache = render::prepare_display_condition_cache(i);
        self.render(i)
    }
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
    /// Native pixel geometry for the frames currently known to this
    /// front-end. The shared command loop publishes these dimensions before
    /// running frame-size change hooks.
    fn frame_pixel_geometry(&self) -> Vec<(usize, i128, i128)> {
        Vec::new()
    }
}

mod bitmap;
mod fringe;
mod mode_line;
mod command;
mod render;
mod popup;
mod menus;
mod menu_commands;
pub use menu_commands::{NativeMenuCommandId, WeakNativeMenuCommandId};
pub(crate) use menu_commands::NativeMenuCommands;
pub use menus::prepare_native_menus;
pub use popup::{PopupMenu, PopupMenuEntry, PopupMenuNavigation};
pub(crate) use popup::show_popup_menu;
pub(crate) use command::minibuf_dispatch_key;
pub use command::{
    batch_command_error, dispatch_key_core, install_minibuf_reader, nonblocking_sleep,
    replay_macro_events, run_batch_editor, run_editor_with, run_editor_with_shared,
};
pub use render::{
    BoxStyle, BoxStyleKind, CellColor, CellStyle, Grid, GridBottomDivider, GridCursor,
    GridCursorShape, GridFringeBitmap, GridFringeBitmapAlign, GridHelpEcho, GridImage,
    GridImageConversion, GridImageMapArea, GridImageMapShape, GridImageSizeLimit,
    GridImageSlice, GridImageSlicePart, GridImageSlicePixels, GridImageSource, GridInput,
    GridInputPosition,
    GridInputRow,
    GridBidiDirection, GridBidiDisabledRange, GridBidiParagraphRange, GridComposition,
    GridInputSelection, GridMenu, GridPixelwiseSegment, GridStipple, GridPointerShape,
    GridPointerRegion,
    GridModeLine, GridHeaderLine, GridTabLine, GridVScroll, GridWindowTextArea,
    GridMenuItem, GridPopupMenuItem, GridScrollBar, GridScrollBarOrientation, GridTabBarItem,
    GridToolBarItem, fit_text_to_cells, fit_text_to_cells_with_tab_width,
    render_frame_grid_with_input_cached, render_frame_grid_with_mouse_position_cached,
    render_grid, render_grid_mut, render_grid_with_input_cached,
    render_grid_with_mouse_position_cached,
};
pub(crate) use render::{position_at_display_cell, position_display_cell};
pub(crate) use render::window_text_pixel_rows;
pub(crate) use render::visual_line_ranges_for_window;
pub(crate) use render::{buffer_setting, line_number_display_settings};
pub(crate) use render::visible_fontification_range;

pub(crate) fn gui_face_attributes_supported(
    i: &Interp,
    frame: &crate::editor::FrameRef,
    attributes: &crate::lisp::Value,
) -> bool {
    render::gui_face_attributes_supported(i, frame, attributes)
}
    LeftMargin,
    RightMargin,
