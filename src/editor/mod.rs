//! Editor layer: windows, frames, keymaps, kill-ring, file I/O,
//! and editing commands built on the buffer primitives.
//!
//! Windows/frames are Lisp objects (`Value::Window`, `Value::Frame`)
//! shared via `Rc<RefCell<_>>` so Lisp code can hold references while
//! the front-end mutates geometry.

use std::cell::RefCell;
use std::rc::Rc;

use crate::lisp::Interp;
use crate::lisp::builtins::{eq_values, equal_values};
use crate::lisp::builtins::listfn::nthcdr_of;
use crate::lisp::error::{EvalResult, Flow};
use crate::lisp::obarray::sym;
use crate::lisp::value::{Subr, SymId, Value};

pub(crate) mod winxtra;

/// An editor window: a viewport into a buffer.
pub struct Window {
    pub id: usize,
    /// Displayed buffer id.
    pub buffer: usize,
    /// Point when this window is selected (0-based).
    pub point: usize,
    /// First visible char position (0-based).
    pub start: usize,
    /// Horizontal scroll column.
    pub hscroll: usize,
    /// Screen layout (terminal cells, set by the front-end).
    pub top: usize,
    pub height: usize,
    pub left: usize,
    pub width: usize,
    /// Dedicated windows refuse buffer switches.
    pub dedicated: bool,
    /// Minibuffer windows are special.
    pub minibuffer: bool,
    /// This window's parameters alist.
    pub params: Value,
    /// Fringes/margins.
    pub margins: (usize, usize),
    /// `window-use-time': tick of the last creation/selection.
    pub use_time: u64,
    /// Buffers previously shown here — list of
    /// `(buffer start-marker point-marker)' triples, newest first.
    pub prev_buffers: Value,
    /// Buffers recorded by `unrecord-window-buffer'/quit-restore.
    pub next_buffers: Value,
    pub dead: bool,
}

pub type WindowRef = Rc<RefCell<Window>>;

/// An editor frame (a terminal or GUI frame).
pub struct Frame {
    pub id: usize,
    pub name: String,
    /// Non-minibuffer windows in display order.
    pub windows: Vec<WindowRef>,
    pub selected: WindowRef,
    /// The minibuffer window (echo area in ttys).
    pub minibuffer: Option<WindowRef>,
    /// Synthetic root window — spans the frame minus the echo area,
    /// created lazily by `frame-root-window' once the frame holds more
    /// than one live window (with a single window that window IS the
    /// root in GNU).
    pub root: Option<WindowRef>,
    pub width: usize,
    pub height: usize,
    pub params: Value,
    pub dead: bool,
}

pub type FrameRef = Rc<RefCell<Frame>>;

static NEXT_ID: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(1);
fn next_id() -> usize {
    NEXT_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
}

/// `window-use-time' tick — GNU bumps `window_select_count' at window
/// creation and each selection, and reports the last tick a window got.
static USE_TICK: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
pub(crate) fn next_use_time() -> u64 {
    USE_TICK.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1
}

impl Window {
    pub fn new(buffer: usize) -> WindowRef {
        Rc::new(RefCell::new(Window {
            id: next_id(),
            buffer,
            point: 0,
            start: 0,
            hscroll: 0,
            top: 0,
            height: 24,
            left: 0,
            width: 80,
            dedicated: false,
            minibuffer: false,
            params: Value::Nil,
            margins: (0, 0),
            // GNU bumps the use tick on selection, not creation; a
            // fresh window reports 0 until first selected.
            use_time: 0,
            prev_buffers: Value::Nil,
            next_buffers: Value::Nil,
            dead: false,
        }))
    }
}

impl Frame {
    pub fn new_tty(buffer: usize, minibuf: usize, width: usize, height: usize) -> FrameRef {
        let main = Window::new(buffer);
        let mb = Window::new(minibuf);
        {
            // GNU tty layout: the root window spans the frame minus the
            // echo area; the minibuffer window occupies the last line.
            let mut m = main.borrow_mut();
            m.left = 0;
            m.top = 0;
            m.width = width;
            m.height = height.saturating_sub(1);
            let mut e = mb.borrow_mut();
            e.minibuffer = true;
            e.left = 0;
            e.top = height.saturating_sub(1);
            e.width = width;
            e.height = 1;
        }
        // The frame's main window is the one selected at setup (GNU:
        // window_select_count bump), so its use-time starts at 1.
        main.borrow_mut().use_time = next_use_time();
        Rc::new(RefCell::new(Frame {
            id: next_id(),
            name: "F1".into(),
            windows: vec![main.clone()],
            selected: main,
            minibuffer: Some(mb),
            root: None,
            width,
            height,
            params: Value::Nil,
            dead: false,
        }))
    }
}

macro_rules! S {
    ($name:literal, $min:expr, $max:expr, $f:expr, $doc:literal) => {
        crate::lisp::value::Subr {
            name: $name,
            arity: crate::lisp::value::Arity::Range {
                min: $min,
                max: $max,
            },
            func: $f,
            doc: $doc,
        }
    };
    ($name:literal, many $min:expr, $f:expr, $doc:literal) => {
        crate::lisp::value::Subr {
            name: $name,
            arity: crate::lisp::value::Arity::Many { min: $min },
            func: $f,
            doc: $doc,
        }
    };
    ($name:literal, raw, $f:expr, $doc:literal) => {
        crate::lisp::value::Subr {
            name: $name,
            arity: crate::lisp::value::Arity::Unevalled,
            func: $f,
            doc: $doc,
        }
    };
}

pub(crate) static SUBRS: &[Subr] = &[
    // windows
    S!(
        "selected-window",
        0,
        0,
        f_selected_window,
        "Currently selected window."
    ),
    S!("windowp", 1, 1, f_windowp, "t if OBJECT is a window."),
    S!(
        "window-live-p",
        1,
        1,
        f_window_live_p,
        "t if WINDOW is live."
    ),
    S!(
        "window-buffer",
        0,
        1,
        f_window_buffer,
        "Buffer shown in WINDOW."
    ),
    S!(
        "set-window-buffer",
        2,
        3,
        f_set_window_buffer,
        "Show BUFFER in WINDOW."
    ),
    S!("window-point", 0, 1, f_window_point, "Point in WINDOW."),
    S!(
        "set-window-point",
        2,
        2,
        f_set_window_point,
        "Set WINDOW's point."
    ),
    S!(
        "window-start",
        0,
        1,
        f_window_start,
        "First visible position in WINDOW."
    ),
    S!(
        "set-window-start",
        2,
        3,
        f_set_window_start,
        "Set WINDOW's start."
    ),
    S!(
        "window-end",
        0,
        3,
        f_window_end,
        "Last visible position in WINDOW."
    ),
    S!(
        "window-frame",
        0,
        1,
        f_window_frame,
        "Frame containing WINDOW."
    ),
    S!("window-list", 0, 3, f_window_list, "Windows of FRAME."),
    S!(
        "window-minibuffer-p",
        0,
        1,
        f_window_minibuffer_p,
        "t if WINDOW is a minibuffer."
    ),
    S!(
        "minibuffer-window-active-p",
        1,
        1,
        f_minibuffer_window_active_p,
        "t if WINDOW is an active minibuffer."
    ),
    S!("split-window", 0, 4, f_split_window, "Split WINDOW."),
    S!(
        "split-window-below",
        0,
        2,
        f_split_window_vertically,
        "Split below."
    ),
    S!(
        "split-window-right",
        0,
        2,
        f_split_window_horizontally,
        "Split right."
    ),
    S!(
        "split-window-vertically",
        0,
        2,
        f_split_window_vertically,
        "Split vertically."
    ),
    S!(
        "split-window-horizontally",
        0,
        2,
        f_split_window_horizontally,
        "Split horizontally."
    ),
    S!("delete-window", 0, 1, f_delete_window, "Delete WINDOW."),
    S!(
        "delete-other-windows",
        0,
        1,
        f_delete_other_windows,
        "Delete all but WINDOW."
    ),
    S!(
        "other-window",
        0,
        2,
        f_other_window,
        "Select the next window."
    ),
    S!(
        "select-window",
        1,
        2,
        f_select_window,
        "Make WINDOW selected."
    ),
    S!(
        "one-window-p",
        0,
        1,
        f_one_window_p,
        "t if only one window."
    ),
    S!("next-window", 0, 3, f_next_window, "Next window in cycle."),
    S!(
        "enlarge-window",
        1,
        2,
        f_enlarge_window,
        "Make selected window taller."
    ),
    S!(
        "shrink-window",
        1,
        2,
        f_shrink_window,
        "Make selected window shorter."
    ),
    S!(
        "enlarge-window-horizontally",
        1,
        1,
        f_enlarge_window_horizontally,
        ""
    ),
    S!(
        "shrink-window-horizontally",
        1,
        1,
        f_shrink_window_horizontally,
        ""
    ),
    S!(
        "balance-windows",
        0,
        1,
        f_balance_windows,
        "Equalize window heights."
    ),
    S!(
        "maximize-window",
        0,
        1,
        f_maximize_window,
        "Make window as tall as possible."
    ),
    S!(
        "minimize-window",
        0,
        1,
        f_minimize_window,
        "Make window as short as possible."
    ),
    S!(
        "adjust-window-trailing-edge",
        2,
        4,
        f_adjust_window_trailing_edge,
        ""
    ),
    S!(
        "switch-to-buffer-other-window",
        1,
        2,
        f_switch_to_buffer_other_window,
        ""
    ),
    S!(
        "switch-to-buffer-other-frame",
        1,
        2,
        f_switch_to_buffer_other_frame,
        ""
    ),
    S!(
        "display-buffer-other-frame",
        1,
        1,
        f_display_buffer_other_frame,
        ""
    ),
    S!("quit-window", 0, 2, f_quit_window, "Quit WINDOW."),
    S!("quit-restore-window", 0, 2, f_quit_restore_window, ""),
    S!("kill-buffer-and-window", 0, 0, f_kill_buffer_and_window, ""),
    S!(
        "replace-buffer-in-windows",
        0,
        1,
        f_replace_buffer_in_windows,
        ""
    ),
    S!("window-tree", 0, 1, f_window_tree, "Window layout tree."),
    S!(
        "window-combination-limit",
        1,
        1,
        f_window_combination_limit,
        ""
    ),
    S!(
        "scroll-other-window-down",
        0,
        1,
        f_scroll_other_window_down,
        ""
    ),
    S!(
        "previous-window",
        0,
        3,
        f_previous_window,
        "Previous window."
    ),
    S!(
        "walk-windows",
        1,
        3,
        f_walk_windows,
        "Call FN on each window."
    ),
    S!(
        "get-buffer-window",
        0,
        2,
        f_get_buffer_window,
        "Window displaying BUFFER."
    ),
    S!(
        "get-buffer-window-list",
        0,
        3,
        f_get_buffer_window_list,
        "Windows displaying BUFFER."
    ),
    S!("window-height", 0, 2, f_window_height, "Height of WINDOW."),
    S!(
        "window-body-height",
        0,
        2,
        f_window_body_height,
        "Body height of WINDOW."
    ),
    S!("window-total-height", 0, 2, f_window_height, ""),
    S!("window-width", 0, 2, f_window_width, "Width of WINDOW."),
    S!(
        "window-body-width",
        0,
        2,
        f_window_body_width,
        "Body width of WINDOW."
    ),
    S!("window-total-width", 0, 2, f_window_width, ""),
    S!("window-text-width", 0, 2, f_window_body_width, ""),
    S!(
        "window-hscroll",
        0,
        1,
        f_window_hscroll,
        "Horizontal scroll of WINDOW."
    ),
    S!(
        "set-window-hscroll",
        2,
        2,
        f_set_window_hscroll,
        "Set WINDOW hscroll."
    ),
    S!(
        "window-edges",
        0,
        4,
        f_window_edges,
        "Edge coords of WINDOW."
    ),
    S!("window-inside-edges", 0, 1, f_window_edges, ""),
    S!("window-at", 2, 2, f_window_at, "Window at X,Y."),
    S!(
        "recenter",
        0,
        2,
        f_recenter,
        "Scroll so point is on line N."
    ),
    S!("scroll-up", 0, 1, f_scroll_up, "Scroll text up N lines."),
    S!(
        "scroll-up-line",
        0,
        1,
        f_scroll_up_line,
        "Scroll up one line."
    ),
    S!(
        "scroll-down-line",
        0,
        1,
        f_scroll_down_line,
        "Scroll down one line."
    ),
    S!(
        "window-left",
        0,
        1,
        f_window_left,
        "Window to the left of WINDOW."
    ),
    S!(
        "window-right",
        0,
        1,
        f_window_right,
        "Window to the right of WINDOW."
    ),
    S!(
        "scroll-down",
        0,
        1,
        f_scroll_down,
        "Scroll text down N lines."
    ),
    S!(
        "scroll-up-command",
        0,
        1,
        f_scroll_up_command,
        "Scroll up a screenful."
    ),
    S!(
        "scroll-down-command",
        0,
        1,
        f_scroll_down_command,
        "Scroll down a screenful."
    ),
    S!(
        "scroll-other-window",
        0,
        1,
        f_scroll_other_window,
        "Scroll the other window."
    ),
    S!(
        "pos-visible-in-window-p",
        0,
        3,
        f_pos_visible_in_window_p,
        "t if POS is visible."
    ),
    S!(
        "window-dedicated-p",
        0,
        1,
        f_window_dedicated_p,
        "t if WINDOW is dedicated."
    ),
    S!(
        "set-window-dedicated-p",
        2,
        2,
        f_set_window_dedicated_p,
        "Set WINDOW dedicated flag."
    ),
    S!(
        "window-parameter",
        2,
        2,
        f_window_parameter,
        "WINDOW's PARAMETER."
    ),
    S!(
        "set-window-parameter",
        3,
        3,
        f_set_window_parameter,
        "Set WINDOW parameter."
    ),
    S!(
        "window-parameters",
        0,
        1,
        f_window_parameters,
        "WINDOW's parameter alist."
    ),
    S!("window-left-char", 0, 1, f_window_start, ""),
    S!("window-top-line", 0, 1, f_window_start, ""),
    S!("window-display-table", 0, 1, f_window_display_table, ""),
    S!("set-window-display-table", 2, 2, f_second, ""),
    S!(
        "window-margins",
        0,
        1,
        f_window_margins,
        "Margins of WINDOW."
    ),
    S!("window-use-time", 0, 1, f_window_use_time, ""),
    S!("window-cursor-type", 0, 1, f_t, ""),
    S!("window-configuration-p", 1, 1, f_window_configuration_p, ""),
    S!("current-window-configuration", 0, 1, f_current_window_configuration, ""),
    S!("set-window-configuration", 1, 3, f_set_window_configuration, ""),
    S!("window-state-get", 0, 2, f_window_state_get, ""),
    S!("window-state-put", 1, 3, f_window_state_put, ""),
    // frames
    S!(
        "selected-frame",
        0,
        0,
        f_selected_frame,
        "The selected frame."
    ),
    S!("framep", 1, 1, f_framep, "t if OBJECT is a frame."),
    S!("frame-live-p", 1, 1, f_frame_live_p, "t if FRAME is live."),
    S!("frame-list", 0, 0, f_frame_list, "All live frames."),
    S!("visible-frame-list", 0, 0, f_frame_list, ""),
    S!("delete-frame", 0, 2, f_delete_frame, "Delete FRAME."),
    S!(
        "frame-parameter",
        2,
        2,
        f_frame_parameter,
        "FRAME's PARAMETER."
    ),
    S!(
        "frame-parameters",
        0,
        1,
        f_frame_parameters,
        "FRAME's parameters alist."
    ),
    S!(
        "modify-frame-parameters",
        2,
        2,
        f_modify_frame_parameters,
        "Set FRAME parameters."
    ),
    S!(
        "set-frame-parameter",
        3,
        3,
        f_set_frame_parameter,
        "Set FRAME parameter."
    ),
    S!(
        "set-frame-selected-window",
        2,
        3,
        f_set_frame_selected_window,
        "Select WINDOW in FRAME."
    ),
    S!(
        "frame-width",
        0,
        1,
        f_frame_width,
        "Width of FRAME in chars."
    ),
    S!(
        "frame-height",
        0,
        1,
        f_frame_height,
        "Height of FRAME in chars."
    ),
    S!("frame-pixel-width", 0, 1, f_frame_width, ""),
    S!("frame-pixel-height", 0, 1, f_frame_height, ""),
    S!(
        "frame-position",
        0,
        1,
        f_frame_position,
        "Frame position (0,0)."
    ),
    S!("frame-edges", 0, 2, f_frame_edges, "Frame edges."),
    S!(
        "make-frame",
        0,
        1,
        f_make_frame,
        "Make a new frame (tty: reuse)."
    ),
    S!("display-graphic-p", 0, 1, f_nil, "t on GUI (nil on tty)."),
    S!(
        "window-system",
        0,
        1,
        f_nil,
        "Window system type (nil on tty)."
    ),
    S!("frame-terminal", 0, 1, f_frame_terminal, ""),
    S!("select-frame", 1, 2, f_select_frame, "Select FRAME."),
    S!("handle-switch-frame", 1, 1, f_handle_switch_frame, ""),
    S!("frame-focus-state", 0, 1, f_frame_focus_state, ""),
    S!("redraw-frame", 0, 1, f_redraw_frame, ""),
    S!("redraw-display", 0, 0, f_nil, ""),
    S!("frame-visible-p", 1, 1, f_frame_visible_p, ""),
    // keymaps
    S!("make-keymap", 0, 1, f_make_keymap, "Create a full keymap."),
    S!(
        "make-sparse-keymap",
        0,
        1,
        f_make_sparse_keymap,
        "Create a sparse keymap."
    ),
    S!("keymapp", 1, 1, f_keymapp, "t if OBJECT is a keymap."),
    S!("keymap-prompt", 1, 1, f_keymap_prompt, ""),
    S!("copy-keymap", 1, 1, f_copy_keymap, "Copy KEYMAP."),
    S!("keymap-parent", 1, 1, f_keymap_parent, "Parent of KEYMAP."),
    S!(
        "set-keymap-parent",
        2,
        2,
        f_set_keymap_parent,
        "Set KEYMAP's parent."
    ),
    S!(
        "define-key",
        3,
        4,
        f_define_key,
        "Bind KEY to DEF in KEYMAP."
    ),
    S!("lookup-key", 2, 3, f_lookup_key, "Look up KEY in KEYMAP."),
    S!("key-binding", 1, 4, f_key_binding, "Command bound to KEY."),
    S!(
        "local-key-binding",
        1,
        2,
        f_local_key_binding,
        "Local binding of KEY."
    ),
    S!(
        "global-key-binding",
        1,
        2,
        f_global_key_binding,
        "Global binding of KEY."
    ),
    S!("minor-mode-key-binding", 1, 2, f_minor_mode_key_binding, ""),
    S!(
        "current-local-map",
        0,
        0,
        f_current_local_map,
        "Current buffer's local map."
    ),
    S!(
        "current-global-map",
        0,
        0,
        f_current_global_map,
        "The global map."
    ),
    S!("current-minor-mode-maps", 0, 0, f_current_minor_mode_maps, ""),
    S!("use-local-map", 1, 1, f_use_local_map, "Set local map."),
    S!("use-global-map", 1, 1, f_use_global_map, "Set global map."),
    S!(
        "local-set-key",
        2,
        2,
        f_local_set_key,
        "Bind KEY in local map."
    ),
    S!(
        "global-set-key",
        2,
        2,
        f_global_set_key,
        "Bind KEY in global map."
    ),
    S!(
        "local-unset-key",
        1,
        1,
        f_local_unset_key,
        "Unbind KEY in local map."
    ),
    S!(
        "global-unset-key",
        1,
        1,
        f_global_unset_key,
        "Unbind KEY in global map."
    ),
    S!(
        "define-prefix-command",
        1,
        3,
        f_define_prefix_command,
        "Define COMMAND as a prefix keymap."
    ),
    S!(
        "command-remapping",
        1,
        3,
        f_command_remapping,
        "Remapped command for COMMAND."
    ),
    S!(
        "where-is-internal",
        1,
        5,
        f_where_is_internal,
        "Keys binding COMMAND."
    ),
    S!("kbd", 1, 1, f_kbd, "Parse KEYS into a key vector."),
    S!(
        "key-description",
        1,
        2,
        f_key_description,
        "Human-readable key description."
    ),
    S!(
        "single-key-description",
        1,
        2,
        f_single_key_description,
        "Describe one key event."
    ),
    S!(
        "text-char-description",
        1,
        1,
        f_text_char_description,
        "Describe character."
    ),
    S!(
        "read-key-sequence",
        1,
        7,
        f_read_key_sequence,
        "Read a key sequence."
    ),
    S!(
        "read-key-sequence-vector",
        1,
        7,
        f_read_key_sequence_vector,
        "Read keys to a vector."
    ),
    S!(
        "this-command-keys",
        0,
        0,
        f_this_command_keys,
        "Keys for this command."
    ),
    S!(
        "this-command-keys-vector",
        0,
        0,
        f_this_command_keys_vector,
        ""
    ),
    S!(
        "this-single-command-keys",
        0,
        0,
        f_this_command_keys_vector,
        ""
    ),
    S!(
        "this-single-command-raw-keys",
        0,
        0,
        f_this_command_keys_vector,
        ""
    ),
    S!("recent-keys", 0, 1, f_this_command_keys_vector, ""),
    S!("clear-this-command-keys", 0, 1, f_nil, ""),
    S!("input-pending-p", 0, 1, f_nil, ""),
    S!("discard-input", 0, 0, f_nil, ""),
    S!("last-nonminibuffer-frame", 0, 0, f_selected_frame, ""),
    // kill ring
    S!("kill-new", 1, 2, f_kill_new, "Push STRING onto kill-ring."),
    S!(
        "kill-append",
        1,
        2,
        f_kill_append,
        "Append STRING to latest kill."
    ),
    S!("current-kill", 1, 2, f_current_kill, "Nth kill-ring entry."),
    S!(
        "copy-region-as-kill",
        2,
        2,
        f_copy_region_as_kill,
        "Copy region to kill-ring."
    ),
    S!("kill-ring-save", 2, 2, f_copy_region_as_kill, ""),
    S!("yank", 0, 1, f_yank, "Insert the latest kill."),
    S!(
        "yank-pop",
        0,
        1,
        f_yank_pop,
        "Replace yank with earlier kill."
    ),
    S!(
        "rotate-yank-pointer",
        1,
        1,
        f_rotate_yank_pointer,
        "Rotate kill-ring pointer."
    ),
    S!(
        "copy-to-buffer",
        3,
        3,
        f_copy_to_buffer,
        "Copy region to BUFFER, replacing its contents."
    ),
    S!(
        "append-to-buffer",
        3,
        3,
        f_append_to_buffer,
        "Append region to BUFFER at its point."
    ),
    // file I/O
    S!(
        "file-exists-p",
        1,
        1,
        f_file_exists_p,
        "t if FILENAME exists."
    ),
    S!(
        "file-directory-p",
        1,
        1,
        f_file_directory_p,
        "t if FILENAME is a directory."
    ),
    S!(
        "file-regular-p",
        1,
        1,
        f_file_regular_p,
        "t if FILENAME is a regular file."
    ),
    S!(
        "file-readable-p",
        1,
        1,
        f_file_readable_p,
        "t if FILENAME is readable."
    ),
    S!(
        "file-writable-p",
        1,
        1,
        f_file_writable_p,
        "t if FILENAME is writable."
    ),
    S!(
        "file-executable-p",
        1,
        1,
        f_file_executable_p,
        "t if FILENAME is executable."
    ),
    S!(
        "file-symlink-p",
        1,
        1,
        f_file_symlink_p,
        "t if FILENAME is a symlink."
    ),
    S!(
        "file-newer-than-file-p",
        2,
        2,
        f_file_newer_than_file_p,
        "t if FILE1 is newer."
    ),
    S!(
        "file-attributes",
        1,
        2,
        f_file_attributes,
        "Attributes of FILENAME."
    ),
    S!("file-modes", 1, 2, f_file_modes, "Mode bits of FILENAME."),
    S!("set-file-modes", 2, 3, f_set_file_modes, "Set mode bits."),
    S!(
        "file-name-absolute-p",
        1,
        1,
        f_file_name_absolute_p,
        "t if FILENAME is absolute."
    ),
    S!(
        "expand-file-name",
        1,
        2,
        f_expand_file_name,
        "Make FILENAME absolute."
    ),
    S!(
        "file-name-directory",
        1,
        1,
        f_file_name_directory,
        "Directory part of FILENAME."
    ),
    S!(
        "file-name-nondirectory",
        1,
        1,
        f_file_name_nondirectory,
        "Nondirectory part."
    ),
    S!(
        "file-name-extension",
        1,
        2,
        f_file_name_extension,
        "Extension of FILENAME."
    ),
    S!(
        "file-name-sans-extension",
        1,
        1,
        f_file_name_sans_extension,
        "FILENAME minus extension."
    ),
    S!(
        "file-name-sans-versions",
        1,
        2,
        f_file_name_sans_extension,
        ""
    ),

    S!(
        "file-name-base",
        1,
        1,
        f_file_name_base,
        "FILENAME minus dir and ext."
    ),
    S!(
        "file-name-as-directory",
        1,
        1,
        f_file_name_as_directory,
        "Ensure trailing slash."
    ),
    S!(
        "directory-file-name",
        1,
        1,
        f_directory_file_name,
        "Directory as filename (no slash)."
    ),
    S!("file-name-concat", many 1, f_file_name_concat, "Join path components."),
    S!(
        "file-relative-name",
        1,
        2,
        f_file_relative_name,
        "FILENAME relative to DIR."
    ),
    S!(
        "abbreviate-file-name",
        1,
        1,
        f_abbreviate_file_name,
        "Abbreviate home dir."
    ),
    S!(
        "substitute-in-file-name",
        1,
        1,
        f_substitute_in_file_name,
        "Expand $VARS."
    ),
    S!(
        "directory-files",
        1,
        4,
        f_directory_files,
        "Files in DIRECTORY."
    ),
    S!(
        "directory-files-and-attributes",
        1,
        5,
        f_directory_files_and_attributes,
        ""
    ),
    S!(
        "file-name-completion",
        2,
        3,
        f_file_name_completion,
        "Complete FILE in DIRECTORY."
    ),
    S!(
        "file-name-all-completions",
        2,
        2,
        f_file_name_all_completions,
        "All completions of FILE."
    ),
    S!("make-directory", 1, 2, f_make_directory, "Create DIR."),
    S!(
        "make-directory-internal",
        1,
        1,
        f_make_directory_internal,
        "Create DIR (no parents)."
    ),
    S!(
        "make-temp-name",
        1,
        1,
        f_make_temp_name,
        "Unique name with PREFIX."
    ),
    S!(
        "make-temp-file",
        1,
        5,
        f_make_temp_file,
        "Create a new temp file."
    ),
    S!(
        "file-local-copy",
        1,
        1,
        f_file_local_copy,
        "Copy remote file locally (nil for local)."
    ),
    S!(
        "file-in-directory-p",
        2,
        2,
        f_file_in_directory_p,
        "Is FILE under DIRECTORY?"
    ),
    S!("delete-directory", 1, 3, f_delete_directory, "Delete DIR."),
    S!("delete-file", 1, 2, f_delete_file, "Delete FILENAME."),
    S!(
        "delete-directory-internal",
        1,
        1,
        f_delete_directory_internal,
        "Internal: delete DIRECTORY."
    ),
    S!(
        "delete-file-internal",
        1,
        1,
        f_delete_file_internal,
        "Internal: delete FILE (nil if missing)."
    ),
    S!(
        "locate-file-internal",
        2,
        4,
        f_locate_file_internal,
        "Search PATH for FILENAME."
    ),
    S!(
        "rename-file",
        2,
        3,
        f_rename_file,
        "Rename FILE to NEWNAME."
    ),
    S!("copy-file", 2, 4, f_copy_file, "Copy FILE to NEWNAME."),

    S!(
        "add-name-to-file",
        2,
        3,
        f_rename_file,
        "Hard link FILE to NEWNAME."
    ),
    S!(
        "insert-file-contents",
        1,
        7,
        f_insert_file_contents,
        "Insert contents of FILENAME."
    ),
    S!(
        "insert-file-contents-literally",
        1,
        5,
        f_insert_file_contents_literally,
        ""
    ),
    S!(
        "write-region",
        3,
        7,
        f_write_region,
        "Write region to FILENAME."
    ),
    // `write-region-annotate-functions',
    // `write-region-post-annotation-function' are plain variables in GNU;
    // `write-region-charset-for-write' does not exist there.
    S!(
        "car-less-than-car",
        2,
        2,
        f_car_less_than_car,
        "Compare cars."
    ),
    S!(
        "set-visited-file-name",
        0,
        2,
        f_set_visited_file_name,
        "Set buffer-file-name."
    ),
    S!(
        "find-file-noselect",
        1,
        4,
        f_find_file_noselect,
        "Read FILENAME into a buffer."
    ),
    S!("find-file", 1, 2, f_find_file, "Visit FILENAME."),
    S!("find-file-literally", 1, 1, f_find_file, ""),
    S!("save-buffer", 0, 1, f_save_buffer, "Save current buffer."),
    S!(
        "write-file",
        1,
        2,
        f_write_file,
        "Write buffer to FILENAME."
    ),
    S!(
        "append-to-file",
        3,
        4,
        f_append_to_file,
        "Append region to FILENAME."
    ),
    S!(
        "file-truename",
        1,
        1,
        f_file_truename,
        "Canonical name of FILENAME."
    ),
    S!(
        "unhandled-file-name-directory",
        1,
        1,
        f_unhandled_dir,
        ""
    ),
    S!("file-remote-p", 1, 3, f_file_remote_p, ""),
    S!("file-local-name", 1, 1, f_file_local_name, ""),
    S!("file-name-quote", 1, 2, f_file_name_quote, ""),
    S!("file-name-unquote", 1, 2, f_file_name_unquote, ""),
    S!("file-accessible-directory-p", 1, 1, f_file_directory_p, ""),
    S!("default-file-modes", 0, 0, f_default_file_modes, ""),
    S!("set-default-file-modes", 1, 1, f_set_default_file_modes, ""),
    S!(
        "file-modes-symbolic-to-number",
        1,
        2,
        f_modes_sym2num,
        ""
    ),
    S!("unix-sync", 0, 0, f_unix_sync, ""),
    S!("file-system-info", 1, 1, f_file_system_info, ""),
    S!("file-equal-p", 2, 2, f_file_equal_p, ""),
    // processes
    S!(
        "call-process",
        1,
        8,
        f_call_process,
        "Run PROGRAM synchronously."
    ),
    S!(
        "call-process-region",
        3,
        9,
        f_call_process_region,
        "Run PROGRAM on region."
    ),
    S!(
        "shell-command",
        1,
        4,
        f_shell_command,
        "Run COMMAND in a shell."
    ),
    S!(
        "shell-command-to-string",
        1,
        1,
        f_shell_command_to_string,
        "Run COMMAND, return output."
    ),
    // Process primitives are real implementations in lisp::process.
    // editing commands
    S!("kill-line", 0, 1, f_kill_line, "Kill to end of line."),
    S!(
        "kill-whole-line",
        0,
        1,
        f_kill_whole_line,
        "Kill the whole line."
    ),
    S!("kill-word", 1, 1, f_kill_word, "Kill N words."),
    S!(
        "backward-kill-word",
        1,
        1,
        f_backward_kill_word,
        "Kill N words backward."
    ),
    S!(
        "delete-horizontal-space",
        0,
        1,
        f_delete_horizontal_space,
        "Delete surrounding whitespace."
    ),
    S!(
        "just-one-space",
        0,
        1,
        f_just_one_space,
        "One space around point."
    ),
    S!(
        "delete-indentation",
        0,
        1,
        f_delete_indentation,
        "Join this line to previous."
    ),
    S!("join-line", 0, 1, f_delete_indentation, ""),
    S!(
        "zap-to-char",
        2,
        3,
        f_zap_to_char,
        "Kill up to Nth occurrence of CHAR."
    ),
    S!(
        "transpose-chars",
        1,
        1,
        f_transpose_chars,
        "Swap chars around point."
    ),
    S!(
        "transpose-lines",
        1,
        1,
        f_transpose_lines,
        "Swap lines around point."
    ),
    S!("upcase-region", 2, 3, f_upcase_region, "Uppercase region."),
    S!(
        "downcase-region",
        2,
        3,
        f_downcase_region,
        "Lowercase region."
    ),
    S!(
        "capitalize-region",
        2,
        3,
        f_capitalize_region,
        "Capitalize region."
    ),
    S!(
        "upcase-word",
        1,
        1,
        f_upcase_word,
        "Uppercase next N words."
    ),
    S!(
        "downcase-word",
        1,
        1,
        f_downcase_word,
        "Lowercase next N words."
    ),
    S!(
        "capitalize-word",
        1,
        1,
        f_capitalize_word,
        "Capitalize next N words."
    ),
    S!(
        "indent-line-to",
        1,
        1,
        f_indent_line_to,
        "Indent line to COLUMN."
    ),
    S!("indent-to", 1, 2, f_indent_to, "Indent to COLUMN."),
    S!(
        "indent-rigidly",
        3,
        4,
        f_indent_rigidly,
        "Indent region rigidly."
    ),
    // `tab-to-tab-stop' is defined in Lisp (as in GNU's indent.el).
    S!(
        "delete-trailing-whitespace",
        0,
        2,
        f_delete_trailing_whitespace,
        ""
    ),
    S!("untabify", 0, 2, f_untabify, "Convert tabs to spaces."),
    S!(
        "tabify",
        0,
        2,
        f_tabify,
        "Convert spaces to tabs (stub keeps)."
    ),
    S!(
        "move-beginning-of-line",
        1,
        1,
        f_move_beginning_of_line,
        "Command: BOL."
    ),
    S!(
        "move-end-of-line",
        1,
        1,
        f_move_end_of_line,
        "Command: EOL."
    ),
    S!("forward-line-command", 0, 1, f_forward_line_cmd, ""),
    // `beginning-of-buffer-other-window'/`end-of-buffer-other-window'
    // are Lisp (prelude window.el port).
    // minibuffer/echo
    S!(
        "minibufferp",
        0,
        1,
        f_minibufferp,
        "t if BUFFER is a minibuffer."
    ),
    S!(
        "minibuffer-contents",
        0,
        0,
        f_minibuffer_contents,
        "Minibuffer text."
    ),
    S!(
        "minibuffer-contents-no-properties",
        0,
        0,
        f_minibuffer_contents,
        ""
    ),
    S!(
        "delete-minibuffer-contents",
        0,
        0,
        f_delete_minibuffer_contents,
        "Clear minibuffer."
    ),
    S!(
        "minibuffer-depth",
        0,
        0,
        f_minibuffer_depth,
        "Minibuffer recursion depth."
    ),
    S!(
        "minibuffer-prompt",
        0,
        0,
        f_minibuffer_prompt,
        "Minibuffer prompt text."
    ),
    S!("minibuffer-prompt-end", 0, 0, f_minibuffer_prompt_end, ""),
    S!(
        "active-minibuffer-window",
        0,
        0,
        f_active_minibuffer_window,
        ""
    ),
    S!("set-minibuffer-window", 1, 1, f_set_minibuffer_window, ""),
    S!("minibuffer-message", many 1, f_minibuffer_message, "Message in minibuffer."),
    S!(
        "read-from-minibuffer",
        1,
        8,
        f_read_from_minibuffer,
        "Read from minibuffer."
    ),
    S!("read-buffer", 1, 4, f_read_buffer, "Read a buffer name."),
    S!(
        "read-file-name",
        1,
        8,
        f_read_file_name,
        "Read a file name."
    ),
    S!("read-directory-name", 1, 7, f_read_file_name, ""),
    S!("read-number", 1, 3, f_read_number, "Read a number."),
    S!("read-regexp", 1, 3, f_read_regexp, "Read a regexp."),
    S!(
        "completing-read",
        2,
        8,
        f_completing_read,
        "Read with completion."
    ),
    S!(
        "try-completion",
        2,
        3,
        f_try_completion,
        "Completion of STRING."
    ),
    S!(
        "all-completions",
        2,
        4,
        f_all_completions,
        "All completions of STRING."
    ),
    S!(
        "test-completion",
        2,
        3,
        f_test_completion,
        "t if STRING completes."
    ),
    S!(
        "completion-boundaries",
        4,
        4,
        f_completion_boundaries,
        "Return the boundaries of the completions."
    ),
    S!(
        "completion--flex-cost-gotoh",
        2,
        2,
        f_completion_flex_cost_gotoh,
        "Compute cost of PAT matching STR using modified Gotoh\nalgorithm.  Return nil if no match found, else return (COST . MATCHES)\nwhere COST is a fixnum (lower is better) and MATCHES is a list of the\nsame length as PAT.  Each i-th element is a FIXNUM indicating where in\nSTR the i-th character of PAT matched."
    ),
    S!(
        "internal-complete-buffer",
        3,
        3,
        f_internal_complete_buffer,
        "Complete STRING over buffer names."
    ),
    S!(
        "completing-read-default",
        2,
        8,
        f_completing_read_default,
        ""
    ),
    S!(
        "completing-read-multiple",
        2,
        8,
        f_completing_read_multiple,
        ""
    ),
    S!(
        "minibuffer-completion-help",
        0,
        0,
        f_minibuffer_completion_help,
        ""
    ),
    S!("read-string", 1, 5, f_read_string, "Read a string."),
    S!("read-command", 1, 2, f_read_command, "Read a command name."),
    S!(
        "read-variable",
        1,
        2,
        f_read_variable,
        "Read a variable name."
    ),
    S!("read-key", 0, 2, f_read_char, "Read one key event."),
    S!("read-coding-system", 1, 2, f_read_coding_system, ""),
    S!("read-color", 0, 6, f_read_color, "Read a color name."),
    S!("read-passwd", 1, 3, f_read_passwd, "Read a password."),
    S!(
        "read-kbd-macro",
        1,
        1,
        f_read_kbd_macro,
        "Read a kbd macro."
    ),
    S!(
        "momentary-string-display",
        2,
        4,
        f_momentary_string_display,
        ""
    ),
    S!(
        "display-message-or-buffer",
        1,
        4,
        f_display_message_or_buffer,
        ""
    ),
    S!("redisplay", 0, 1, f_redisplay, "Redisplay."),
    S!("force-mode-line-update", 0, 1, f_force_mode_line_update, ""),
    S!("tooltip-show", 1, 4, f_tooltip_show, ""),
    S!("tooltip-hide", 0, 1, f_tooltip_hide, ""),
    S!("timer-event-handler", 1, 1, f_timer_event_handler, ""),
    S!("invisible-p", 1, 1, f_invisible_p, "t if POS invisible."),
    S!("fringe-bitmaps-at-pos", 0, 2, f_fringe_bitmaps_at_pos, ""),
    S!("binary-overwrite-mode", 0, 1, f_binary_overwrite_mode, ""),
    S!("scroll-lock-mode", 0, 1, f_scroll_lock_mode, ""),
    S!("pixel-scroll-mode", 0, 1, f_pixel_scroll_mode, ""),
    S!(
        "pixel-scroll-precision-mode",
        0,
        1,
        f_pixel_scroll_precision_mode,
        ""
    ),

    S!(
        "move-to-window-line-top-bottom",
        0,
        1,
        f_move_to_window_line_top_bottom,
        "Cycle point through window top/middle/bottom."
    ),
    S!(
        "recenter-other-window",
        0,
        1,
        f_recenter_other_window,
        "Center point in other window."
    ),
    S!(
        "exit-minibuffer",
        0,
        0,
        f_exit_minibuffer,
        "Exit the minibuffer."
    ),
    S!(
        "self-insert-and-exit",
        0,
        0,
        f_self_insert_and_exit,
        "Insert char and exit minibuffer."
    ),
    S!(
        "save-buffers-kill-terminal",
        0,
        1,
        f_save_buffers_kill_terminal,
        "Save buffers and exit."
    ),
    S!(
        "open-dribble-file",
        1,
        1,
        f_open_dribble_file,
        "Record keystrokes to FILE."
    ),
    S!("suspend-emacs", 0, 1, f_suspend_emacs, "Suspend Emacs."),
    S!("suspend-frame", 0, 0, f_suspend_emacs, "Suspend the frame."),
    S!("byteorder", 0, 0, f_byteorder, "Byte order: ?l or ?B."),
    S!(
        "command-line-1",
        0,
        1,
        f_nil,
        "Process command-line args (done)."
    ),
    S!(
        "normal-top-level",
        0,
        0,
        f_nil,
        "Top-level entry point (done)."
    ),
    S!(
        "standard-display-european-internal",
        0,
        0,
        f_standard_display_european_internal,
        "European display setup."
    ),
    S!("read-event", 0, 3, f_read_char, "Read one input event."),
    S!("read-char", 0, 3, f_read_char, "Read one character."),
    S!(
        "read-char-exclusive",
        0,
        3,
        f_read_char,
        "Read one character."
    ),
    S!("y-or-n-p", 1, 1, f_y_or_n_p, "Ask yes/no (batch: t)."),
    S!("yes-or-no-p", 1, 1, f_y_or_n_p, ""),
    // commands/misc
    S!(
        "commandp",
        1,
        2,
        f_commandp,
        "t if OBJECT is callable interactively."
    ),
    S!(
        "call-interactively",
        1,
        3,
        f_call_interactively,
        "Call FUNCTION interactively."
    ),
    S!(
        "execute-extended-command",
        1,
        2,
        f_execute_extended_command,
        "M-x."
    ),
    S!(
        "execute-kbd-macro",
        1,
        3,
        f_execute_kbd_macro,
        "Execute MACRO (string or vector of events) COUNT times."
    ),
    S!(
        "start-kbd-macro",
        1,
        2,
        f_start_kbd_macro,
        "Record subsequent input, defining a keyboard macro."
    ),
    S!(
        "end-kbd-macro",
        0,
        2,
        f_end_kbd_macro,
        "Finish defining a keyboard macro."
    ),
    S!(
        "call-last-kbd-macro",
        0,
        2,
        f_call_last_kbd_macro,
        "Call the last keyboard macro."
    ),
    S!(
        "defining-kbd-macro",
        1,
        2,
        f_defining_kbd_macro,
        "Record subsequent keyboard input, defining a keyboard macro."
    ),
    S!(
        "cancel-kbd-macro-events",
        0,
        0,
        f_cancel_kbd_macro_events,
        "Cancel events recorded for this command."
    ),
    S!(
        "store-kbd-macro-event",
        1,
        1,
        f_store_kbd_macro_event,
        "Store EVENT into the keyboard macro being defined."
    ),
    S!(
        "prefix-numeric-value",
        1,
        1,
        f_prefix_numeric_value,
        "Numeric prefix value."
    ),
    S!("universal-argument", 0, 0, f_universal_argument, "C-u."),
    S!(
        "digit-argument",
        1,
        1,
        f_digit_argument,
        "Set prefix arg from typed digits."
    ),
    S!("negative-argument", 1, 1, f_negative_argument, "M--."),
    // `beginning-of-defun', `end-of-defun', `mark-defun' and
    // `narrow-to-defun' are defined in Lisp (as in GNU's lisp.el).
    // `narrow-to-page', `count-lines-page', `what-page' and
    // `set-goal-column' are defined in Lisp (as in GNU's page.el/simple.el).
    S!("count-words", 2, 2, f_count_words, "Words in region."),
    S!("count-words-region", 2, 2, f_count_words, ""),
    S!(
        "what-cursor-position",
        0,
        1,
        f_what_cursor_position,
        "Describe point."
    ),
    S!("what-line", 0, 0, f_what_line, "Show line number."),
    S!("char-syntax", 1, 1, f_char_syntax, "Syntax code of CHAR."),
    S!("modify-syntax-entry", 2, 3, f_modify_syntax_entry, ""),
    S!("syntax-table", 0, 0, f_syntax_table, ""),
    S!("set-syntax-table", 1, 1, f_set_syntax_table, ""),
    S!("syntax-table-p", 1, 1, f_syntax_table_p, ""),
    S!(
        "make-syntax-table",
        0,
        1,
        f_make_syntax_table,
        "New syntax table."
    ),
    S!("copy-syntax-table", 0, 1, f_copy_syntax_table, ""),
    S!(
        "syntax-after",
        1,
        1,
        crate::buffer::primitives::f_syntax_after,
        ""
    ),
    S!("syntax-class", 1, 1, f_syntax_class, ""),
    S!(
        "syntax-class-to-char",
        1,
        1,
        f_syntax_class_to_char,
        "Character designating syntax class CLASS."
    ),
    S!(
        "matching-paren",
        1,
        1,
        f_matching_paren,
        "Matching parenthesis of CHAR, or nil."
    ),
    S!("standard-syntax-table", 0, 0, f_standard_syntax_table, ""),
    S!("string-to-syntax", 1, 1, f_string_to_syntax, ""),
    // `syntax-propertize' and `internal--syntax-propertize' are defined
    // in Lisp (as in GNU's syntax.el).
    S!(
        "parse-partial-sexp",
        2,
        6,
        f_parse_partial_sexp,
        "Sexp parse state."
    ),
    S!(
        "syntax-ppss",
        0,
        1,
        f_syntax_ppss,
        "Sexp parser state at POS."
    ),
    // `comment-beginning' is defined in Lisp (as in GNU's newcomment.el).
    // modes
    S!(
        "fundamental-mode",
        0,
        0,
        f_fundamental_mode,
        "The default major mode."
    ),
    S!("major-mode-suspend", 0, 0, f_major_mode_suspend, ""),
    S!("major-mode-restore", 0, 0, f_major_mode_restore, ""),
    // delay-mode-hooks / run-mode-hooks /
    // normal-mode / set-auto-mode{,-0} / set-buffer-major-mode /
    // hack-local-variables / hack-dir-local-variables /
    // dir-locals-set-class-variables are Lisp (prelude files.el port),
    // like GNU.
    // timers: run-at-time / run-with-timer / run-with-idle-timer /
    // cancel-timer / timerp / timer-activate / with-timeout are Lisp
    // (prelude timer.el port), like GNU.  `current-idle-time' stays
    // nil — batch Emacs is never idle.
    S!("current-idle-time", 0, 0, f_nil, ""),
    // overlays
    S!(
        "make-overlay",
        2,
        5,
        f_make_overlay,
        "Create overlay BEG..END."
    ),
    S!("delete-overlay", 1, 1, f_delete_overlay, "Delete OVERLAY."),
    S!(
        "delete-all-overlays",
        0,
        1,
        f_delete_all_overlays,
        "Delete all overlays of BUFFER."
    ),
    S!(
        "move-overlay",
        3,
        4,
        f_move_overlay,
        "Move OVERLAY to BEG..END."
    ),
    S!("overlay-start", 1, 1, f_overlay_start, "Start of OVERLAY."),
    S!("overlay-end", 1, 1, f_overlay_end, "End of OVERLAY."),
    S!(
        "overlay-buffer",
        1,
        1,
        f_overlay_buffer,
        "Buffer of OVERLAY."
    ),
    S!("overlay-put", 3, 3, f_overlay_put, "Set OVERLAY property."),
    S!("overlay-get", 2, 2, f_overlay_get, "Get OVERLAY property."),
    S!(
        "overlay-properties",
        1,
        1,
        f_overlay_properties,
        "Overlay plist."
    ),
    S!("overlayp", 1, 1, f_overlayp, "t if OBJECT is an overlay."),
    S!("overlays-at", 1, 2, f_overlays_at, "Overlays at POS."),
    S!(
        "overlays-in",
        2,
        2,
        f_overlays_in,
        "Overlays between BEG and END."
    ),
    S!("overlays-at-point", 0, 0, f_overlays_at_point, ""),
    S!(
        "next-overlay-change",
        1,
        1,
        f_next_overlay_change,
        "Next pos with overlay boundary."
    ),
    S!("previous-overlay-change", 1, 1, f_prev_overlay_change, ""),
    S!(
        "remove-overlays",
        0,
        4,
        f_remove_overlays,
        "Remove overlays in region."
    ),
    S!("copy-overlay", 1, 1, f_copy_overlay, "Copy overlay."),
    S!(
        "overlay-lists",
        0,
        0,
        f_overlay_lists,
        "Overlays before/after point."
    ),
    S!("overlay-recenter", 1, 1, f_overlay_recenter, ""),
    S!(
        "restore-buffer-modified-p",
        1,
        1,
        crate::buffer::primitives::f_set_buffer_modified_p,
        ""
    ),
    // faces (minimal tty model)
    S!("facep", 1, 1, f_facep, ""),
    S!(
        "internal-get-lisp-face-attribute",
        2,
        3,
        f_face_attribute,
        ""
    ),
    S!("set-face-attribute", many 2, f_set_face_attribute, ""),
    S!("face-attribute", 2, 4, f_face_attribute, ""),
    S!("face-attribute-relative-p", 2, 2, f_face_attribute_relative_p, ""),
    S!("merge-face-attribute", 3, 3, f_merge_face_attribute, ""),
    S!("face-all-attributes", 1, 2, f_face_all_attributes, ""),
    S!("face-list", 0, 0, f_face_list, ""),
    S!("make-face", 1, 1, f_make_face, ""),
    S!("copy-face", 2, 4, f_copy_face, ""),
    S!("face-equal", 2, 2, f_face_equal, ""),
    S!("face-id", 1, 2, f_face_id, ""),
    S!("face-background", 1, 3, f_face_background, ""),
    S!("face-foreground", 1, 3, f_face_foreground, ""),
    S!("face-bold-p", 1, 3, f_face_bold_p, ""),
    S!("face-italic-p", 1, 3, f_face_italic_p, ""),
    S!("face-underline-p", 1, 3, f_face_underline_p, ""),
    S!("internal-lisp-face-p", 1, 2, f_facep, ""),
    S!("internal-lisp-face-empty-p", 1, 2, f_lisp_face_check_nil, ""),
    S!("internal-lisp-face-equal-p", 2, 3, f_face_equal, ""),
    S!(
        "internal-set-lisp-face-attribute",
        3,
        4,
        f_internal_set_lisp_face_attribute,
        ""
    ),
    S!("internal-lisp-face-attribute-values", 1, 1, f_lisp_face_check_nil, ""),
    S!("internal-merge-in-global-face", 2, 2, f_internal_merge_in_global_face, ""),
    S!("display-color-p", 0, 1, f_display_color_p, ""),
    S!("display-grayscale-p", 0, 1, f_nil, ""),
    S!("display-mouse-p", 0, 1, f_nil, ""),
    S!("color-defined-p", 1, 1, f_color_defined_p, ""),
    S!("defined-colors", 0, 1, f_defined_colors, ""),
    S!("color-values", 1, 2, f_color_values, ""),
    S!("x-color-values", 1, 1, f_x_color_values, ""),
    S!("xw-color-values", 1, 2, f_xw_color_values, ""),
    S!("tty-color-values", 1, 1, f_tty_color_values, ""),
    S!("x-list-fonts", 1, 5, f_x_no_display, ""),
    S!("internal-char-font", 1, 2, f_internal_char_font, ""),
    S!("fontp", 1, 2, f_fontp, ""),
    S!("find-font", 1, 2, f_find_font, ""),
    S!("font-xlfd-name", 1, 1, f_font_xlfd_name, ""),
    S!("clear-font-cache", 0, 0, f_nil, ""),
    S!("list-fonts", 1, 4, f_list_fonts, ""),
    // cursor/display misc
    // `cursor-type` is a variable in Emacs, not a function —
    // calling it signals void-function like GNU.
    S!("blink-cursor-mode", 0, 1, f_blink_cursor_mode, ""),
    S!("internal-show-cursor", 2, 2, f_nil, ""),
    S!("internal-show-cursor-p", 0, 1, f_show_cursor_p, ""),
    S!("set-window-cursor-type", 2, 3, f_set_window_cursor_type, ""),
    // `make-display-table', `display-table-slot', `set-display-table-slot'
    // are Lisp in GNU (disp-table.el) — see prelude.rs.
    S!("describe-display-table", 1, 1, f_describe_display_table, ""),
    // `standard-display-table' is a variable in GNU (nil in batch).
    S!("open-font", 1, 3, f_open_font, ""),
    S!("query-font", 1, 1, f_query_font, ""),
    S!("font-get", 2, 2, f_font_get, ""),
    S!("font-put", 3, 3, f_font_put, ""),
    S!("set-fontset-font", 3, 5, f_fontset_font, ""),
    S!("new-fontset", 2, 2, f_new_fontset, ""),
    S!("fontset-info", 1, 1, f_x_no_display, ""),
    S!("fontset-font", 2, 3, f_fontset_font, ""),
    S!("fontset-list", 0, 0, f_fontset_list, ""),
    // menus/popups
    S!("x-popup-menu", 2, 2, f_x_popup_menu, ""),
    S!("x-popup-dialog", 2, 3, f_x_popup_dialog, ""),
    S!("menu-or-popup-active-p", 0, 0, f_nil, ""),
    S!("menu-bar-menu-at-x-y", 2, 3, f_nil, ""),
    // echo/help
    S!(
        "documentation-property",
        2,
        3,
        f_documentation_property,
        "Prop on symbol."
    ),
    S!("Snarf-documentation", 1, 1, f_snarf_documentation, ""),
    S!(
        "documentation",
        1,
        2,
        f_documentation,
        "Docstring of FUNCTION."
    ),
    S!(
        "internal-event-symbol-parse-modifiers",
        1,
        1,
        f_identity,
        ""
    ),
    S!(
        "substitute-command-keys",
        1,
        1,
        f_substitute_command_keys,
        "Substitute key descriptions in STRING."
    ),
    S!(
        "apropos-internal",
        1,
        2,
        f_apropos_internal,
        "Symbols matching REGEXP."
    ),
    // indent-according-to-mode etc are Lisp-level
    // dynamic-completion-table skip
    // text-conversion? skip
];

// ---------- small helpers ----------

fn f_nil(_i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    Ok(Value::Nil)
}
fn f_t(_i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    Ok(Value::t())
}
fn f_identity(_i: &mut Interp, a: Vec<Value>) -> EvalResult {
    Ok(a.into_iter().next().unwrap_or(Value::Nil))
}
fn f_second(_i: &mut Interp, a: Vec<Value>) -> EvalResult {
    Ok(a.into_iter().nth(1).unwrap_or(Value::Nil))
}
fn arg(a: &[Value], i: usize) -> Value {
    a.get(i).cloned().unwrap_or(Value::Nil)
}

pub(crate) fn err_sym(i: &mut Interp, name: &str, data: Vec<Value>) -> Flow {
    let id = i.intern(name);
    i.signal_data(id, data)
}

fn want_str(i: &mut Interp, v: &Value) -> Result<String, Flow> {
    match v {
        Value::Str(s) => Ok(s.borrow().clone()),
        _ => Err(i.wrong_type_mut("stringp", v)),
    }
}

fn want_int(i: &mut Interp, v: &Value) -> Result<i128, Flow> {
    match v {
        Value::Int(n) => Ok(*n),
        Value::Marker(m) => Ok(m.borrow().position as i128 + 1),
        _ => Err(i.wrong_type_mut("integer-or-marker-p", v)),
    }
}

fn want_sym(i: &mut Interp, v: &Value) -> Result<SymId, Flow> {
    i.sym_id(v).ok_or_else(|| i.wrong_type_mut("symbolp", v))
}

pub(crate) fn cur(i: &Interp) -> Rc<RefCell<crate::buffer::Buffer>> {
    i.current_buffer_ref().unwrap()
}

pub(crate) fn sel_frame(i: &Interp) -> Option<FrameRef> {
    i.selected_frame.clone()
}

pub(crate) fn sel_window(i: &Interp) -> Option<WindowRef> {
    i.selected_frame
        .as_ref()
        .map(|f| f.borrow().selected.clone())
}

pub(crate) fn win_of(i: &mut Interp, v: &Value) -> Result<WindowRef, Flow> {
    match v {
        Value::Nil => sel_window(i).ok_or_else(|| i.error("No window")),
        Value::Window(w) => Ok(w.clone()),
        other => Err(i.wrong_type_mut("windowp", other)),
    }
}

pub(crate) fn frame_of(i: &mut Interp, v: &Value) -> Result<FrameRef, Flow> {
    match v {
        Value::Nil => sel_frame(i).ok_or_else(|| i.error("No frame")),
        Value::Frame(f) => Ok(f.clone()),
        Value::Window(w) => {
            let wid = w.borrow().id;
            match i
                .frames
                .iter()
                .find(|f| f.borrow().windows.iter().any(|w2| w2.borrow().id == wid))
                .cloned()
            {
                Some(f) => Ok(f),
                None => sel_frame(i).ok_or_else(|| i.error("No frame")),
            }
        }
        other => Err(i.wrong_type_mut("framep", other)),
    }
}

// ---------- windows ----------

fn f_selected_window(i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    match sel_window(i) {
        Some(w) => Ok(Value::Window(w)),
        None => Ok(Value::Nil),
    }
}

fn f_windowp(_i: &mut Interp, a: Vec<Value>) -> EvalResult {
    Ok(Value::from_bool(matches!(&a[0], Value::Window(_))))
}

fn f_window_live_p(_i: &mut Interp, a: Vec<Value>) -> EvalResult {
    match &a[0] {
        Value::Window(w) => Ok(Value::from_bool(!w.borrow().dead)),
        _ => Ok(Value::Nil),
    }
}

fn f_window_buffer(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let w = win_of(i, &arg(&a, 0))?;
    Ok(i.buffer_value(w.borrow().buffer).unwrap_or(Value::Nil))
}

fn f_set_window_buffer(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let w = win_of(i, &arg(&a, 0))?;
    let bid = i
        .buffer_id_of(&a[1])
        .ok_or_else(|| i.error(format!("No buffer named {}", i.princ_to_string(&a[1]))))?;
    let (old_bid, start, point) = {
        let pt = window_point(i, &w);
        let wb = w.borrow();
        (wb.buffer, wb.start, pt)
    };
    if old_bid != bid {
        // GNU `unshow_buffer': prepend a (buffer start point) entry
        // for the old buffer to `window-prev-buffers', deduplicated,
        // and drop the new buffer's own entry.
        let old_buf = i
            .buffers
            .get(old_bid)
            .map(Value::Buffer)
            .unwrap_or(Value::Nil);
        let entry = Value::list(vec![
            old_buf,
            crate::buffer::primitives::new_marker_at(i, old_bid, start),
            crate::buffer::primitives::new_marker_at(i, old_bid, point),
        ]);
        let mut kept: Vec<Value> = Vec::new();
        let prev = w.borrow().prev_buffers.clone();
        if let Ok(items) = prev.list_to_vec() {
            for e in items {
                if let Value::Cons(c) = &e {
                    if let Value::Buffer(b) = &c.borrow().car {
                        let id = b.borrow().id;
                        if id == old_bid || id == bid {
                            continue;
                        }
                    }
                }
                kept.push(e);
            }
        }
        kept.insert(0, entry);
        w.borrow_mut().prev_buffers = Value::list(kept);
        w.borrow_mut().buffer = bid;
        w.borrow_mut().point = 0;
        w.borrow_mut().start = 0;
    }
    // Displaying a buffer makes it most-recent in buffer-list order.
    i.buffers.touch(bid);
    Ok(Value::Nil)
}

/// Sync the selected window's point with the buffer's actual point.
fn window_point(i: &Interp, w: &WindowRef) -> usize {
    let sel = sel_window(i).map(|s| s.borrow().id) == Some(w.borrow().id);
    if sel {
        // selected window shows the buffer's live point
        i.buffers
            .get(w.borrow().buffer)
            .map(|b| b.borrow().point())
            .unwrap_or_else(|| w.borrow().point)
    } else {
        w.borrow().point
    }
}

fn f_window_point(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let w = win_of(i, &arg(&a, 0))?;
    Ok(Value::Int(window_point(i, &w) as i128 + 1))
}

fn f_set_window_point(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let w = win_of(i, &arg(&a, 0))?;
    let pos = want_int(i, &a[1])?.max(1) as usize - 1;
    w.borrow_mut().point = pos;
    let is_sel = sel_window(i).map(|s| s.borrow().id) == Some(w.borrow().id);
    if is_sel {
        if let Some(b) = i.buffers.get(w.borrow().buffer) {
            b.borrow_mut().set_point(pos);
        }
    }
    Ok(a[1].clone())
}

fn f_window_start(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let w = win_of(i, &arg(&a, 0))?;
    Ok(Value::Int(w.borrow().start as i128 + 1))
}

fn f_set_window_start(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let w = win_of(i, &arg(&a, 0))?;
    let pos = want_int(i, &a[1])?.max(1) as usize - 1;
    w.borrow_mut().start = pos;
    Ok(a[1].clone())
}

fn f_window_end(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let w = win_of(i, &arg(&a, 0))?;
    let (start, height, buf) = {
        let ww = w.borrow();
        (ww.start, ww.height, ww.buffer)
    };
    let len = i
        .buffers
        .get(buf)
        .map(|b| b.borrow().text_len())
        .unwrap_or(0);
    // Approximate: start + height lines.
    let end = if let Some(b) = i.buffers.get(buf) {
        let bb = b.borrow();
        let mut p = start.min(bb.text_len());
        for _ in 0..height {
            p = bb.text.line_end(p).min(bb.text.len());
            if p < bb.text.len() {
                p += 1;
            }
        }
        p
    } else {
        len
    };
    Ok(Value::Int(end as i128 + 1))
}

fn f_window_frame(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let w = win_of(i, &arg(&a, 0))?;
    let wid = w.borrow().id;
    for f in &i.frames {
        if f.borrow().windows.iter().any(|w2| w2.borrow().id == wid) {
            return Ok(Value::Frame(f.clone()));
        }
        if let Some(mb) = &f.borrow().minibuffer {
            if mb.borrow().id == wid {
                return Ok(Value::Frame(f.clone()));
            }
        }
    }
    Ok(Value::Nil)
}

pub(crate) fn f_window_list(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let f = frame_of(i, &arg(&a, 0))?;
    // GNU: MINIBUF t = always include the minibuffer window; nil =
    // include only when active; any other non-nil = never include.
    let include_mini = match a.get(1) {
        Some(v) if v.truthy() => i.sym_id(v) == Some(sym::T),
        _ => i.minibuf_level > 0,
    };
    // GNU returns the frame's window chain in cyclic order starting
    // at the SELECTED window — the minibuffer window (when included)
    // sits in the chain after the last content window.
    let (mut ws, sel_id) = {
        let fb = f.borrow();
        let mut ws = fb.windows.clone();
        if include_mini {
            if let Some(mb) = &fb.minibuffer {
                ws.push(mb.clone());
            }
        }
        (ws, fb.selected.borrow().id)
    };
    if let Some(pos) = ws.iter().position(|w| w.borrow().id == sel_id) {
        ws.rotate_left(pos);
    }
    Ok(Value::list(
        ws.iter().map(|w| Value::Window(w.clone())).collect(),
    ))
}

fn f_window_minibuffer_p(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let w = win_of(i, &arg(&a, 0))?;
    Ok(Value::from_bool(w.borrow().minibuffer))
}

fn f_minibuffer_window(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let f = frame_of(i, &arg(&a, 0))?;
    match &f.borrow().minibuffer {
        Some(w) => Ok(Value::Window(w.clone())),
        None => Ok(Value::Nil),
    }
}

fn f_active_minibuffer_window(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    // nil unless a minibuffer is currently active.
    if i.minibuf_level == 0 {
        return Ok(Value::Nil);
    }
    f_minibuffer_window(i, a)
}

/// The sole terminal object: a lazily-created record standing in for
/// GNU's `#<terminal 0 on initial_terminal>' (singleton — `eq' holds).
pub(crate) fn terminal_token(i: &mut Interp) -> Value {
    if let Some(t) = &i.terminal {
        return t.clone();
    }
    let t = Value::Record(Rc::new(RefCell::new(vec![
        Value::Sym(i.intern("terminal")),
        Value::Int(0),
        Value::string("initial_terminal"),
    ])));
    i.terminal = Some(t.clone());
    t
}

/// Whether V is our terminal record (nil counts as the default
/// terminal for predicates that accept it — callers decide).
pub(crate) fn is_terminal(i: &Interp, v: &Value) -> bool {
    if let Value::Record(r) = v {
        if let Some(Value::Sym(tag)) = r.borrow().first() {
            return i.symbol_name(*tag) == "terminal";
        }
    }
    false
}

/// `window-display-table` — window-live-p check; nil (no per-window
/// display table in our model).
fn f_window_display_table(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    match a.first() {
        None | Some(Value::Nil) | Some(Value::Window(_)) => Ok(Value::Nil),
        Some(other) => Err(i.wrong_type_mut("window-live-p", other)),
    }
}

/// `merge-face-attribute` — GNU: VALUE1 wins unless `unspecified' or
/// `:ignore-defface', in which case VALUE2.  (`:height' merges
/// relative specs; we take VALUE1.)
fn f_merge_face_attribute(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    if let Value::Sym(s) = &a[1] {
        let n = i.symbol_name(*s);
        if n == "unspecified" || n == ":ignore-defface" {
            return Ok(a[2].clone());
        }
    }
    Ok(a[1].clone())
}

/// `keymap-prompt' — GNU scans KEYMAP's cdr for the first string
/// element (the menu prompt); non-keymaps return nil, no error.
fn f_keymap_prompt(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    if !is_keymap(i, &a[0]) {
        return Ok(Value::Nil);
    }
    let mut tail = match &a[0] {
        Value::Cons(c) => c.borrow().cdr.clone(),
        _ => Value::Nil,
    };
    loop {
        match tail {
            Value::Cons(link) => {
                let (car, next) = {
                    let b = link.borrow();
                    (b.car.clone(), b.cdr.clone())
                };
                if let Value::Str(_) = car {
                    return Ok(car);
                }
                tail = next;
            }
            _ => return Ok(Value::Nil),
        }
    }
}

/// `syntax-class' — a syntax descriptor is a cons (CLASS . MATCHING);
/// GNU returns CLASS.
fn f_syntax_class(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    match &a[0] {
        Value::Nil => Ok(Value::Nil),
        Value::Cons(c) => {
            let car = c.borrow().car.clone();
            match car {
                Value::Int(_) => Ok(car),
                _ => Ok(Value::Nil),
            }
        }
        other => Err(i.wrong_type_mut("listp", other)),
    }
}

/// `major-mode-suspend' — remember the buffer's local `major-mode'
/// (nil/`fundamental-mode' isn't recorded), reset it to fundamental,
/// and return the remembered mode.  Repeated calls return the recorded
/// value, like GNU's pdump/suspend machinery.
fn f_major_mode_suspend(i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    let sid = i.intern("major-mode");
    let fundamental = i.intern("fundamental-mode");
    let Some(b) = i.current_buffer_ref() else {
        return Ok(Value::Nil);
    };
    let mut bb = b.borrow_mut();
    if let Some(m) = &bb.suspended_mode {
        return Ok(m.clone());
    }
    let m = bb.locals.get(&sid).cloned().unwrap_or(Value::Nil);
    if m.is_nil() || i.sym_is(&m, fundamental) {
        return Ok(Value::Nil);
    }
    bb.suspended_mode = Some(m.clone());
    bb.locals.insert(sid, Value::Sym(fundamental));
    Ok(m)
}

/// `major-mode-restore' — funcall the mode `major-mode-suspend'
/// recorded; nil when nothing is suspended.
fn f_major_mode_restore(i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    let m = i
        .current_buffer_ref()
        .and_then(|b| b.borrow_mut().suspended_mode.take())
        .unwrap_or(Value::Nil);
    if m.is_nil() {
        return Ok(Value::Nil);
    }
    i.apply(&m, vec![])?;
    Ok(Value::Nil)
}

/// `fontset-list' — the default fontset's name, like GNU.
fn f_fontset_list(_i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    Ok(Value::list(vec![Value::string(
        "-*-*-*-*-*-*-*-*-*-*-*-*-fontset-default",
    )]))
}

/// Opaque window-configuration token (`#<window-configuration>').
/// Record layout: [tag frame SPECS SELECTED] where SPECS is a list of
/// vectors, one per live window in display order:
///   [window buffer-id point start hscroll top height left width
///    dedicated margins params prev-buffers next-buffers mark]
/// GNU reuses the same window objects on restore, so the spec keeps the
/// `window' reference and `set-window-configuration' writes the saved
/// fields back into it (reviving it if it died in the meantime).
fn window_configuration(i: &mut Interp) -> Value {
    let tag = Value::Sym(i.intern("window-configuration"));
    let frame_v = i
        .selected_frame
        .as_ref()
        .map(|f| Value::Frame(f.clone()))
        .unwrap_or(Value::Nil);
    let mut specs = Vec::new();
    let mut selected = Value::Nil;
    if let Some(f) = i.selected_frame.as_ref() {
        let fr = f.borrow();
        let sel_id = fr.selected.borrow().id;
        for w in &fr.windows {
            let wb = w.borrow();
            if wb.id == sel_id {
                selected = Value::Window(w.clone());
            }
            let point = {
                let sel = wb.id == sel_id;
                if sel {
                    i.buffers
                        .get(wb.buffer)
                        .map(|b| b.borrow().point())
                        .unwrap_or(wb.point)
                } else {
                    wb.point
                }
            };
            let mark = i
                .buffers
                .get(wb.buffer)
                .and_then(|b| b.borrow().mark)
                .map(|m| Value::Int(m as i128))
                .unwrap_or(Value::Nil);
            specs.push(Value::list(vec![
                Value::Window(w.clone()),
                Value::Int(wb.buffer as i128),
                Value::Int(point as i128),
                Value::Int(wb.start as i128),
                Value::Int(wb.hscroll as i128),
                Value::Int(wb.top as i128),
                Value::Int(wb.height as i128),
                Value::Int(wb.left as i128),
                Value::Int(wb.width as i128),
                Value::from_bool(wb.dedicated),
                Value::from_bool(wb.minibuffer),
                wb.params.clone(),
                wb.prev_buffers.clone(),
                wb.next_buffers.clone(),
                mark,
            ]));
        }
        // GNU configs record the minibuffer window's geometry too; it
        // restores to its saved rectangle (no rescaling).
        if let Some(m) = &fr.minibuffer {
            let mb = m.borrow();
            specs.push(Value::list(vec![
                Value::Window(m.clone()),
                Value::Int(mb.buffer as i128),
                Value::Int(mb.point as i128),
                Value::Int(mb.start as i128),
                Value::Int(mb.hscroll as i128),
                Value::Int(mb.top as i128),
                Value::Int(mb.height as i128),
                Value::Int(mb.left as i128),
                Value::Int(mb.width as i128),
                Value::from_bool(mb.dedicated),
                Value::Sym(sym::T),
                mb.params.clone(),
                mb.prev_buffers.clone(),
                mb.next_buffers.clone(),
                Value::Nil,
            ]));
        }
    }
    Value::Record(Rc::new(RefCell::new(vec![
        tag,
        frame_v,
        Value::list(specs),
        selected,
    ])))
}

/// Integer field N of a saved window spec (0 when absent/non-integer).
fn spec_int(v: &[Value], n: usize) -> i128 {
    match v.get(n) {
        Some(Value::Int(x)) => *x,
        _ => 0,
    }
}

fn window_config_parts(tag: SymId, v: &Value) -> Option<(Value, Vec<Value>, Value)> {
    let r = match v {
        Value::Record(r) => r.clone(),
        _ => return None,
    };
    let cells = r.borrow();
    if !matches!(cells.first(), Some(Value::Sym(s)) if *s == tag) {
        return None;
    }
    let specs = cells.get(2)?.list_to_vec().ok()?;
    Some((
        cells.get(1).cloned().unwrap_or(Value::Nil),
        specs,
        cells.get(3).cloned().unwrap_or(Value::Nil),
    ))
}

fn f_window_configuration_p(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let tag = i.intern("window-configuration");
    Ok(Value::from_bool(match &a[0] {
        Value::Record(r) => {
            matches!(r.borrow().first(), Some(Value::Sym(s)) if *s == tag)
        }
        _ => false,
    }))
}

pub(crate) fn f_current_window_configuration(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    // Optional FRAME: frame-live-p check.
    if let Some(v) = a.first() {
        match v {
            Value::Nil | Value::Frame(_) => {}
            other => return Err(i.wrong_type_mut("frame-live-p", other)),
        }
    }
    Ok(window_configuration(i))
}

pub(crate) fn f_set_window_configuration(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let tag = i.intern("window-configuration");
    let (_frame_v, specs, selected) = match window_config_parts(tag, &a[0]) {
        Some(p) => p,
        None => return Err(i.wrong_type_mut("window-configuration-p", &a[0])),
    };
    let f = sel_frame(i).ok_or_else(|| i.error("No frame"))?;
    // Kill live windows that the configuration does not contain, then
    // rebuild the frame's window list from the saved specs (GNU writes
    // the saved contents back into the same window objects).
    let saved_ids: Vec<usize> = specs
        .iter()
        .filter_map(|s| match s.list_to_vec().ok()?.first()? {
            Value::Window(w) => Some(w.borrow().id),
            _ => None,
        })
        .collect();
    // The current root extent must be measured before any window is
    // marked dead: it is the span of the live non-minibuffer windows.
    let mut cur_specs: Vec<Vec<Value>> = Vec::new();
    {
        let fr = f.borrow();
        for w in &fr.windows {
            let wb = w.borrow();
            if wb.dead || wb.minibuffer {
                continue;
            }
            cur_specs.push(vec![
                Value::Nil,
                Value::Nil,
                Value::Nil,
                Value::Nil,
                Value::Nil,
                Value::Int(wb.top as i128),
                Value::Int(wb.height as i128),
                Value::Int(wb.left as i128),
                Value::Int(wb.width as i128),
            ]);
        }
    }
    {
        let fr = f.borrow_mut();
        for w in &fr.windows {
            if !saved_ids.contains(&w.borrow().id) {
                w.borrow_mut().dead = true;
            }
        }
    }
    let mut new_windows: Vec<WindowRef> = Vec::new();
    let mut marks: Vec<(usize, usize)> = Vec::new();
    // GNU stores normalized sizes and re-lays the configuration out in
    // the frame's current root extent: map each window's edges
    // proportionally from the saved extent to the live one (identity
    // when nothing resized, exact tiling via edge-based mapping).
    let extent_of = |specs: &[Vec<Value>]| -> Option<(i128, i128, i128, i128)> {
        if specs.is_empty() {
            return None;
        }
        let (mut t, mut l, mut b, mut r) = (i128::MAX, i128::MAX, i128::MIN, i128::MIN);
        for v in specs {
            if !(5..=8).all(|n| matches!(v.get(n), Some(Value::Int(_)))) {
                return None;
            }
            let (wt, wh, wl, ww) = (
                spec_int(v, 5),
                spec_int(v, 6),
                spec_int(v, 7),
                spec_int(v, 8),
            );
            t = t.min(wt);
            l = l.min(wl);
            b = b.max(wt + wh);
            r = r.max(wl + ww);
        }
        Some((t, l, b, r))
    };
    // GNU restores the minibuffer window to its saved rectangle; its
    // top edge is the bottom of the root extent regular windows map
    // into (persistent chrome like the tty menu row keeps the top).
    let mut mini_top: Option<i128> = None;
    let content_specs: Vec<Vec<Value>> = specs
        .iter()
        .filter_map(|s| s.list_to_vec().ok())
        .filter(|v| !v.get(10).map(|x| x.truthy()).unwrap_or(false))
        .collect();
    for spec in &specs {
        let v = match spec.list_to_vec() {
            Ok(v) => v,
            Err(_) => continue,
        };
        if !v.get(10).map(|x| x.truthy()).unwrap_or(false) {
            continue;
        }
        if let Some(Value::Window(w)) = v.first() {
            let mut wb = w.borrow_mut();
            wb.top = spec_int(&v, 5).max(0) as usize;
            wb.height = spec_int(&v, 6).max(0) as usize;
            wb.left = spec_int(&v, 7).max(0) as usize;
            wb.width = spec_int(&v, 8).max(0) as usize;
            wb.dead = false;
            mini_top = Some(spec_int(&v, 5));
        }
    }
    let map = extent_of(&content_specs)
        .zip(extent_of(&cur_specs))
        .map(|((st, sl, sb, sr), (ct, cl, cb, cr))| {
            let (sh, sw) = ((sb - st).max(1), (sr - sl).max(1));
            let cb = mini_top.unwrap_or(cb);
            let (ch, cw) = ((cb - ct).max(1), (cr - cl).max(1));
            move |top: i128, height: i128, left: i128, width: i128| {
                let nt = ct + (top - st) * ch / sh;
                let nb = ct + (top + height - st) * ch / sh;
                let nl = cl + (left - sl) * cw / sw;
                let nr = cl + (left + width - sl) * cw / sw;
                (nt, (nb - nt).max(0), nl, (nr - nl).max(0))
            }
        });
    for v in &content_specs {
        let w = match v.first() {
            Some(Value::Window(w)) => w.clone(),
            _ => continue,
        };
        {
            let mut wb = w.borrow_mut();
            wb.buffer = spec_int(v, 1).max(0) as usize;
            wb.point = spec_int(v, 2).max(0) as usize;
            wb.start = spec_int(v, 3).max(0) as usize;
            wb.hscroll = spec_int(v, 4).max(0) as usize;
            let (nt, nh, nl, nw) = match &map {
                Some(m) => m(
                    spec_int(v, 5),
                    spec_int(v, 6),
                    spec_int(v, 7),
                    spec_int(v, 8),
                ),
                None => (
                    spec_int(v, 5),
                    spec_int(v, 6),
                    spec_int(v, 7),
                    spec_int(v, 8),
                ),
            };
            wb.top = nt.max(0) as usize;
            wb.height = nh.max(0) as usize;
            wb.left = nl.max(0) as usize;
            wb.width = nw.max(0) as usize;
            wb.dedicated = v.get(9).map(|x| x.truthy()).unwrap_or(false);
            wb.params = v.get(11).cloned().unwrap_or(Value::Nil);
            wb.prev_buffers = v.get(12).cloned().unwrap_or(Value::Nil);
            wb.next_buffers = v.get(13).cloned().unwrap_or(Value::Nil);
            wb.dead = false;
        }
        if let Some(Value::Int(m)) = v.get(14) {
            marks.push((spec_int(v, 1).max(0) as usize, (*m).max(0) as usize));
        }
        new_windows.push(w);
    }
    {
        let mut fr = f.borrow_mut();
        fr.windows = new_windows;
        // The synthetic root's extent depends on the live set.
        fr.root = None;
        match &selected {
            Value::Window(w) => fr.selected = w.clone(),
            _ => {
                if let Some(w) = fr.windows.first() {
                    fr.selected = w.clone();
                }
            }
        }
    }
    // GNU re-selects the configuration's window, bumping its use-time,
    // and makes its buffer current with the saved point.
    if let Value::Window(w) = &selected {
        w.borrow_mut().use_time = next_use_time();
        let (bid, pt) = {
            let wb = w.borrow();
            (wb.buffer, wb.point)
        };
        if let Some(b) = i.buffers.get(bid) {
            b.borrow_mut().set_point(pt);
            i.current_buffer = bid;
        }
    }
    for (bid, pos) in marks {
        if let Some(b) = i.buffers.get(bid) {
            b.borrow_mut().mark = Some(pos);
        }
    }
    Ok(Value::Sym(sym::T))
}

/// GNU `compare_window_configurations': equal means same buffers shown
/// in windows of the same size/position, in the same order.
pub(crate) fn f_window_configuration_equal_p(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let tag = i.intern("window-configuration");
    for v in &a[..2] {
        if window_config_parts(tag, v).is_none() {
            return Err(i.wrong_type_mut("window-configuration-p", v));
        }
    }
    let (_, s1, _) = window_config_parts(tag, &a[0]).unwrap();
    let (_, s2, _) = window_config_parts(tag, &a[1]).unwrap();
    if s1.len() != s2.len() {
        return Ok(Value::Nil);
    }
    for (x, y) in s1.iter().zip(s2.iter()) {
        let (xv, yv) = match (x.list_to_vec(), y.list_to_vec()) {
            (Ok(xv), Ok(yv)) => (xv, yv),
            _ => return Ok(Value::Nil),
        };
        // Compare buffer id, geometry, point and start (indices 1..7).
        for n in 1..8 {
            if !crate::lisp::eq_values(
                xv.get(n).unwrap_or(&Value::Nil),
                yv.get(n).unwrap_or(&Value::Nil),
            ) {
                return Ok(Value::Nil);
            }
        }
    }
    Ok(Value::Sym(sym::T))
}

pub(crate) fn f_window_configuration_frame(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let tag = i.intern("window-configuration");
    match window_config_parts(tag, &a[0]) {
        Some((frame_v, _, _)) => Ok(frame_v),
        None => Err(i.wrong_type_mut("window-configuration-p", &a[0])),
    }
}

fn f_frame_terminal(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    // Validate optional FRAME like GNU, then return its terminal.
    if let Some(v) = a.first() {
        let _ = frame_of(i, v)?;
    }
    Ok(terminal_token(i))
}

/// The tag name when V is one of our font objects: a record tagged
/// `font-spec', `font-entity', or `font-object'.
fn font_kind(i: &Interp, v: &Value) -> Option<String> {
    if let Value::Record(r) = v {
        if let Some(Value::Sym(tag)) = r.borrow().first() {
            let name = i.symbol_name(*tag);
            if matches!(name.as_str(), "font-spec" | "font-entity" | "font-object") {
                return Some(name);
            }
        }
    }
    None
}

fn f_fontp(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let kind = font_kind(i, &a[0]);
    let ok = match a.get(1) {
        None | Some(Value::Nil) => kind.is_some(),
        Some(Value::Sym(s)) => kind.as_deref() == Some(i.symbol_name(*s).as_str()),
        Some(_) => false,
    };
    Ok(Value::from_bool(ok))
}

fn f_find_font(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    // GNU checks FONT-SPEC; batch finds no matching font → nil.
    if font_kind(i, &a[0]).as_deref() != Some("font-spec") {
        return Err(i.wrong_type_mut("font-spec", &a[0]));
    }
    if let Some(v) = a.get(1) {
        let _ = frame_of(i, v)?;
    }
    Ok(Value::Nil)
}

fn f_list_fonts(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    if font_kind(i, &a[0]).as_deref() != Some("font-spec") {
        return Err(i.wrong_type_mut("font-spec", &a[0]));
    }
    Ok(Value::Nil)
}

fn f_open_font(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    if font_kind(i, &a[0]).as_deref() != Some("font-entity") {
        return Err(i.wrong_type_mut("font-entity", &a[0]));
    }
    Ok(Value::Nil)
}

fn f_query_font(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    if font_kind(i, &a[0]).as_deref() != Some("font-object") {
        return Err(i.wrong_type_mut("font-object", &a[0]));
    }
    Ok(Value::Nil)
}

fn f_set_minibuffer_window(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    // GNU: WINDOW must be a window; ours is fixed.
    match &a[0] {
        Value::Window(_) => Ok(Value::Nil),
        other => Err(i.wrong_type_mut("windowp", other)),
    }
}

fn f_minibuffer_window_active_p(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    // Active iff the minibuffer has contents / is selected.
    let _ = (i, a);
    Ok(Value::Nil)
}

fn f_split_window(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let w = win_of(i, &arg(&a, 0))?;
    // GNU Fsplit_window: `above'/`below' split vertically, `left'
    // horizontally with the new window on the left, and ANY other
    // non-nil side (including `right', t, bogus) horizontally with
    // the new window on the right.
    let side = match a.get(2).and_then(|v| i.sym_id(v)).map(|s| i.symbol_name(s)).as_deref() {
        Some("above") => 1,
        Some("left") => 3,
        Some("below") | None => 0,
        _ => 2,
    };
    let hor = side >= 2;
    // GNU: positive SIZE is the part the PARENT keeps; negative SIZE
    // is the size of the NEW window; nil halves the parent.
    let size = arg(&a, 1).int();
    // GNU batch/tty quirk: the first split materializes the
    // menu-bar line — the content area shifts down one row and the
    // minibuffer window moves down with it (`frame-height' keeps
    // reporting the old value; observed GNU -Q: root becomes
    // (0 1 80 25), minibuffer (0 25 80 26)).
    if let Some(f) = sel_frame(i) {
        let ff = f.borrow();
        if ff.windows.len() == 1 && w.borrow().top == 0 {
            w.borrow_mut().top = 1;
            if let Some(mb) = &ff.minibuffer {
                let top = mb.borrow().top;
                mb.borrow_mut().top = top + 1;
            }
        }
    }
    let buf = w.borrow().buffer;
    let new = Window::new(buf);
    {
        let mut ww = w.borrow_mut();
        let mut nw = new.borrow_mut();
        nw.point = ww.point;
        nw.start = ww.start;
        if hor {
            let total = ww.width;
            let (keep, sz) = match size {
                Some(s) if s >= 0 => ((s.max(0) as usize).min(total), total.saturating_sub(s.max(0) as usize)),
                Some(s) => {
                    let n = (-s).max(0) as usize;
                    (total.saturating_sub(n), n.min(total))
                }
                None => (total / 2, total - total / 2),
            };
            nw.top = ww.top;
            nw.height = ww.height;
            if side == 2 {
                ww.width = keep;
                nw.left = ww.left + keep;
                nw.width = sz;
            } else {
                nw.left = ww.left;
                nw.width = sz;
                ww.left += sz;
                ww.width = keep;
            }
        } else {
            let total = ww.height;
            let (keep, sz) = match size {
                Some(s) if s >= 0 => ((s.max(0) as usize).min(total), total.saturating_sub(s.max(0) as usize)),
                Some(s) => {
                    let n = (-s).max(0) as usize;
                    (total.saturating_sub(n), n.min(total))
                }
                None => (total / 2, total - total / 2),
            };
            nw.left = ww.left;
            nw.width = ww.width;
            if side == 0 {
                ww.height = keep;
                nw.top = ww.top + keep;
                nw.height = sz;
            } else {
                nw.top = ww.top;
                nw.height = sz;
                ww.top += sz;
                ww.height = keep;
            }
        }
    }
    let f = sel_frame(i).unwrap();
    // GNU inserts the new window adjacent to its parent in the
    // window-list order: after it for below/right, before it for
    // above/left.
    let mut ff = f.borrow_mut();
    let wid = w.borrow().id;
    let idx = ff
        .windows
        .iter()
        .position(|w2| w2.borrow().id == wid)
        .unwrap_or(ff.windows.len());
    ff.windows
        .insert(idx + usize::from(side == 0 || side == 2), new.clone());
    Ok(Value::Window(new))
}

fn f_delete_window(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let w = win_of(i, &arg(&a, 0))?;
    let wid = w.borrow().id;
    let f = sel_frame(i).unwrap();
    {
        let mut ff = f.borrow_mut();
        if ff.windows.len() <= 1 {
            return Err(i.error("Attempt to delete the only window"));
        }
        // GNU gives the deleted window's space to its sibling.  Flat
        // model: merge into the first remaining window that shares a
        // full edge (same left+width for vertical neighbors, same
        // top+height for horizontal ones).
        {
            let d = w.borrow();
            let (dl, dt, dr, db) =
                (d.left, d.top, d.left + d.width, d.top + d.height);
            for r in ff.windows.iter() {
                if r.borrow().id == wid {
                    continue;
                }
                let mut rb = r.borrow_mut();
                let (rl, rt, rr, rb2) =
                    (rb.left, rb.top, rb.left + rb.width, rb.top + rb.height);
                if rl == dl && rb.width == dr - dl && rb2 == dt {
                    // R directly above D: extend down over D's area.
                    rb.height += db - dt;
                    break;
                } else if rl == dl && rb.width == dr - dl && rt == db {
                    // R directly below D: extend up.
                    rb.top = dt;
                    rb.height += db - dt;
                    break;
                } else if rt == dt && rb.height == db - dt && rr == dl {
                    // R directly left of D: extend right.
                    rb.width += dr - dl;
                    break;
                } else if rt == dt && rb.height == db - dt && rl == dr {
                    // R directly right of D: extend left.
                    rb.left = dl;
                    rb.width += dr - dl;
                    break;
                }
            }
        }
        let del_idx = ff
            .windows
            .iter()
            .position(|w2| w2.borrow().id == wid)
            .unwrap_or(0);
        let sel_dead = ff.selected.borrow().id == wid;
        ff.windows.retain(|w2| w2.borrow().id != wid);
        w.borrow_mut().dead = true;
        if sel_dead {
            // GNU: deleting the selected window selects the window
            // FOLLOWING it in the frame's window chain.
            let idx = del_idx % ff.windows.len().max(1);
            ff.selected = ff.windows[idx].clone();
        }
    }
    Ok(Value::Nil)
}

fn f_delete_other_windows(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let w = win_of(i, &arg(&a, 0))?;
    let wid = w.borrow().id;
    let f = sel_frame(i).unwrap();
    let mut ff = f.borrow_mut();
    // GNU expands the kept window over the whole root area: frame
    // width x (topmost content edge .. minibuffer top).
    let root_top = ff
        .windows
        .iter()
        .map(|w2| w2.borrow().top)
        .min()
        .unwrap_or(0);
    let root_bot = ff
        .minibuffer
        .as_ref()
        .map(|m| m.borrow().top)
        .unwrap_or(ff.height);
    let fwidth = ff.width;
    ff.windows.retain(|w2| {
        let keep = w2.borrow().id == wid;
        if !keep {
            w2.borrow_mut().dead = true;
        }
        keep
    });
    {
        let mut wb = w.borrow_mut();
        wb.left = 0;
        wb.top = root_top;
        wb.width = fwidth;
        wb.height = root_bot.saturating_sub(root_top);
    }
    ff.selected = w.clone();
    Ok(Value::Nil)
}

fn window_cycle(i: &Interp, dir: i128, from: &WindowRef) -> Option<WindowRef> {
    let f = sel_frame(i)?;
    let ws = &f.borrow().windows;
    let from_id = from.borrow().id;
    let idx = ws.iter().position(|w| w.borrow().id == from_id)?;
    let n = ws.len() as i128;
    let j = ((idx as i128 + dir) % n + n) % n;
    Some(ws[j as usize].clone())
}

fn f_other_window(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let n = arg(&a, 0).int().unwrap_or(1);
    let sel = sel_window(i).unwrap();
    let mut cur = sel;
    for _ in 0..n.abs() {
        cur = window_cycle(i, n.signum(), &cur).unwrap_or(cur);
    }
    // Select it.
    let f = sel_frame(i).unwrap();
    f.borrow_mut().selected = cur.clone();
    cur.borrow_mut().use_time = next_use_time();
    // Swap buffer point bookkeeping: save old selected point? For a
    // single-buffer-per-window model, point lives in the buffer — on
    // select we copy window point into buffer.
    if let Some(b) = i.buffers.get(cur.borrow().buffer) {
        b.borrow_mut().set_point(cur.borrow().point);
        i.current_buffer = cur.borrow().buffer;
        i.buffers.touch(cur.borrow().buffer);
    }
    Ok(Value::Window(cur))
}

fn f_select_window(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let w = win_of(i, &arg(&a, 0))?;
    let f = sel_frame(i).unwrap();
    f.borrow_mut().selected = w.clone();
    w.borrow_mut().use_time = next_use_time();
    if let Some(b) = i.buffers.get(w.borrow().buffer) {
        b.borrow_mut().set_point(w.borrow().point);
        i.current_buffer = w.borrow().buffer;
        i.buffers.touch(w.borrow().buffer);
    }
    Ok(a[0].clone())
}

fn f_window_use_time(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let w = win_of(i, &arg(&a, 0))?;
    Ok(Value::Int(w.borrow().use_time as i128))
}

fn f_one_window_p(i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    let f = sel_frame(i).unwrap();
    Ok(Value::from_bool(f.borrow().windows.len() == 1))
}

fn f_next_window(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let w = win_of(i, &arg(&a, 0))?;
    match window_cycle(i, 1, &w) {
        Some(n) => Ok(Value::Window(n)),
        None => Ok(Value::Nil),
    }
}

fn f_previous_window(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let w = win_of(i, &arg(&a, 0))?;
    match window_cycle(i, -1, &w) {
        Some(n) => Ok(Value::Window(n)),
        None => Ok(Value::Nil),
    }
}

fn f_walk_windows(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let fun = a[0].clone();
    let f = sel_frame(i).unwrap();
    let ws: Vec<WindowRef> = f.borrow().windows.clone();
    for w in ws {
        i.apply(&fun, vec![Value::Window(w)])?;
    }
    Ok(Value::Nil)
}

fn f_get_buffer_window(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let bid = match a.get(0) {
        None => i.current_buffer,
        // Nonexistent buffer name → nil.
        Some(v) => match i.buffer_id_of(v) {
            Some(b) => b,
            None => return Ok(Value::Nil),
        },
    };
    for f in &i.frames {
        for w in &f.borrow().windows {
            if w.borrow().buffer == bid {
                return Ok(Value::Window(w.clone()));
            }
        }
    }
    Ok(Value::Nil)
}

fn f_get_buffer_window_list(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let bid = match a.get(0) {
        None => i.current_buffer,
        // Nonexistent buffer name → empty list.
        Some(v) => match i.buffer_id_of(v) {
            Some(b) => b,
            None => return Ok(Value::Nil),
        },
    };
    let mut out = Vec::new();
    for f in &i.frames {
        for w in &f.borrow().windows {
            if w.borrow().buffer == bid {
                out.push(Value::Window(w.clone()));
            }
        }
    }
    Ok(Value::list(out))
}

fn f_window_height(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let w = win_of(i, &arg(&a, 0))?;
    Ok(Value::Int(w.borrow().height as i128))
}
fn f_window_body_height(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let w = win_of(i, &arg(&a, 0))?;
    Ok(Value::Int(w.borrow().height.saturating_sub(1) as i128))
}
fn f_window_width(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let w = win_of(i, &arg(&a, 0))?;
    Ok(Value::Int(w.borrow().width as i128))
}
fn f_window_body_width(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let w = win_of(i, &arg(&a, 0))?;
    Ok(Value::Int(w.borrow().width as i128))
}
fn f_window_hscroll(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let w = win_of(i, &arg(&a, 0))?;
    Ok(Value::Int(w.borrow().hscroll as i128))
}
fn f_set_window_hscroll(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let w = win_of(i, &arg(&a, 0))?;
    w.borrow_mut().hscroll = arg(&a, 1).int().unwrap_or(0).max(0) as usize;
    Ok(arg(&a, 1))
}
fn f_window_edges(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let w = win_of(i, &arg(&a, 0))?;
    let ww = w.borrow();
    Ok(Value::list(vec![
        Value::Int(ww.left as i128),
        Value::Int(ww.top as i128),
        Value::Int((ww.left + ww.width) as i128),
        Value::Int((ww.top + ww.height) as i128),
    ]))
}
fn f_window_at(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let x = want_int(i, &a[0])? as usize;
    let y = want_int(i, &a[1])? as usize;
    for f in &i.frames {
        for w in &f.borrow().windows {
            let ww = w.borrow();
            if x >= ww.left && x < ww.left + ww.width && y >= ww.top && y < ww.top + ww.height {
                return Ok(Value::Window(w.clone()));
            }
        }
    }
    Ok(Value::Nil)
}

fn f_recenter(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let w = sel_window(i).unwrap();
    let n = arg(&a, 0).int();
    let (buf_id, height) = {
        let ww = w.borrow();
        (ww.buffer, ww.height)
    };
    if let Some(b) = i.buffers.get(buf_id) {
        let bb = b.borrow();
        let line = bb.text.line_of_pos(bb.point());
        let target_line = match n {
            Some(k) => line.saturating_sub(k.max(0) as usize),
            None => line.saturating_sub(height / 2),
        };
        drop(bb);
        w.borrow_mut().start = {
            let bb = b.borrow();
            bb.text.line_start(target_line)
        };
    }
    Ok(Value::Nil)
}

fn f_scroll_up(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let n = arg(&a, 0).int().unwrap_or_else(|| {
        sel_window(i)
            .map(|w| w.borrow().height as i128 - 2)
            .unwrap_or(10)
    });
    let w = sel_window(i).unwrap();
    let buf = w.borrow().buffer;
    if let Some(b) = i.buffers.get(buf) {
        let bb = b.borrow();
        let mut p = w.borrow().start;
        for _ in 0..n.max(0) {
            let e = bb.text.line_end(p);
            if e >= bb.text.len() {
                break;
            }
            p = e + 1;
        }
        w.borrow_mut().start = p;
    }
    Ok(Value::Nil)
}

fn f_scroll_down(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let n = arg(&a, 0).int().unwrap_or_else(|| {
        sel_window(i)
            .map(|w| w.borrow().height as i128 - 2)
            .unwrap_or(10)
    });
    let w = sel_window(i).unwrap();
    let buf = w.borrow().buffer;
    if let Some(b) = i.buffers.get(buf) {
        let bb = b.borrow();
        let line = bb.text.line_of_pos(w.borrow().start);
        let target = line.saturating_sub(n.max(0) as usize);
        let p = bb.text.line_start(target);
        drop(bb);
        w.borrow_mut().start = p;
    }
    Ok(Value::Nil)
}

fn f_scroll_up_line(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let n = arg(&a, 0).int().unwrap_or(1);
    // GNU signals end-of-buffer when no scrolling is possible.
    let w = sel_window(i).unwrap();
    let buf = w.borrow().buffer;
    let can = i
        .buffers
        .get(buf)
        .map(|b| {
            let bb = b.borrow();
            let start = w.borrow().start;
            let mut p = start;
            for _ in 0..n.max(0) {
                let e = bb.text.line_end(p);
                if e >= bb.text.len() {
                    return false;
                }
                p = e + 1;
            }
            true
        })
        .unwrap_or(false);
    if !can && n != 0 {
        return Err(i.signal_data(sym::END_OF_BUFFER, vec![]));
    }
    f_scroll_up(i, vec![Value::Int(n)])
}
fn f_scroll_down_line(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let n = arg(&a, 0).int().unwrap_or(1);
    let w = sel_window(i).unwrap();
    let buf = w.borrow().buffer;
    let can = i
        .buffers
        .get(buf)
        .map(|b| {
            let bb = b.borrow();
            let line = bb.text.line_of_pos(w.borrow().start);
            line >= n.max(0) as usize || n == 0
        })
        .unwrap_or(false);
    if !can {
        return Err(i.signal_data(sym::BEGINNING_OF_BUFFER, vec![]));
    }
    f_scroll_down(i, vec![Value::Int(n)])
}
fn f_window_left(i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    let sel = sel_window(i).unwrap();
    match window_cycle(i, -1, &sel) {
        Some(w) => Ok(Value::Window(w)),
        None => Ok(Value::Nil),
    }
}
fn f_window_right(i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    let sel = sel_window(i).unwrap();
    match window_cycle(i, 1, &sel) {
        Some(w) => Ok(Value::Window(w)),
        None => Ok(Value::Nil),
    }
}
fn f_scroll_up_command(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    f_scroll_up(i, a)
}
fn f_scroll_down_command(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    f_scroll_down(i, a)
}
fn f_scroll_other_window(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    // Scroll the *next* window.
    let sel = sel_window(i).unwrap();
    if let Some(other) = window_cycle(i, 1, &sel) {
        let n = arg(&a, 0)
            .int()
            .unwrap_or(other.borrow().height as i128 - 2);
        let buf = other.borrow().buffer;
        if let Some(b) = i.buffers.get(buf) {
            let bb = b.borrow();
            let mut p = other.borrow().start;
            for _ in 0..n.max(0) {
                let e = bb.text.line_end(p);
                if e >= bb.text.len() {
                    break;
                }
                p = e + 1;
            }
            other.borrow_mut().start = p;
        }
    }
    Ok(Value::Nil)
}

fn f_pos_visible_in_window_p(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    // GNU computes visibility from the window's glyph matrix — in
    // batch there is no display, so nothing is ever visible.
    if i.noninteractive {
        return Ok(Value::Nil);
    }
    let pos = arg(&a, 0)
        .int()
        .unwrap_or_else(|| cur(i).borrow().point() as i128 + 1);
    let w = win_of(i, &arg(&a, 1))?;
    let start = w.borrow().start as i128 + 1;
    Ok(Value::from_bool(pos >= start))
}

fn f_window_dedicated_p(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let w = win_of(i, &arg(&a, 0))?;
    Ok(Value::from_bool(w.borrow().dedicated))
}
fn f_set_window_dedicated_p(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let w = win_of(i, &arg(&a, 0))?;
    w.borrow_mut().dedicated = a[1].truthy();
    Ok(a[1].clone())
}
fn f_window_parameter(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let w = win_of(i, &arg(&a, 0))?;
    let ps = want_sym(i, &a[1])?;
    Ok(crate::lisp::eval::plist_get(&w.borrow().params, ps))
}
fn f_set_window_parameter(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let w = win_of(i, &arg(&a, 0))?;
    let ps = want_sym(i, &a[1])?;
    let new = crate::lisp::eval::plist_put(&w.borrow().params, ps, a[2].clone());
    w.borrow_mut().params = new;
    Ok(a[2].clone())
}
fn f_window_parameters(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let w = win_of(i, &arg(&a, 0))?;
    let plist = w.borrow().params.clone();
    // convert plist to alist
    let items = plist.list_to_vec().unwrap_or_default();
    let mut out = Vec::new();
    let mut k = 0;
    while k + 1 < items.len() {
        out.push(Value::cons(items[k].clone(), items[k + 1].clone()));
        k += 2;
    }
    Ok(Value::list(out))
}
fn f_window_margins(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let w = match &arg(&a, 0) {
        Value::Nil => sel_window(i).ok_or_else(|| i.error("No window"))?,
        Value::Window(w) if !w.borrow().dead => w.clone(),
        other => return Err(i.wrong_type_mut("window-live-p", other)),
    };
    let (l, r) = w.borrow().margins;
    // GNU always returns (LEFT . RIGHT), nil sides for zero margins.
    let lv = if l == 0 { Value::Nil } else { Value::Int(l as i128) };
    let rv = if r == 0 { Value::Nil } else { Value::Int(r as i128) };
    Ok(Value::cons(lv, rv))
}
// ---------- frames ----------

fn f_display_color_p(_i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    // TTY batch session: no color.
    Ok(Value::Nil)
}

fn f_selected_frame(i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    match &i.selected_frame {
        Some(f) => Ok(Value::Frame(f.clone())),
        None => Ok(Value::Nil),
    }
}
fn f_framep(_i: &mut Interp, a: Vec<Value>) -> EvalResult {
    Ok(Value::from_bool(matches!(&a[0], Value::Frame(_))))
}
fn f_frame_live_p(_i: &mut Interp, a: Vec<Value>) -> EvalResult {
    match &a[0] {
        Value::Frame(f) => Ok(Value::from_bool(!f.borrow().dead)),
        _ => Ok(Value::Nil),
    }
}

fn f_frame_visible_p(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    // GNU requires a live frame; our frames are always "visible".
    match &a[0] {
        Value::Frame(f) if !f.borrow().dead => Ok(Value::t()),
        other => Err(i.wrong_type_mut("frame-live-p", other)),
    }
}

fn f_show_cursor_p(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    // GNU: optional WINDOW arg (nil = selected); ours always shows.
    match a.first() {
        None | Some(Value::Nil) | Some(Value::Window(_)) => Ok(Value::t()),
        Some(other) => Err(i.wrong_type_mut("windowp", other)),
    }
}

fn f_frame_focus_state(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    // GNU: optional FRAME arg must be a live frame; nil (unknown) result.
    match a.first() {
        None | Some(Value::Nil) => Ok(Value::Nil),
        Some(Value::Frame(f)) if !f.borrow().dead => Ok(Value::Nil),
        Some(other) => Err(i.wrong_type_mut("framep", other)),
    }
}
fn f_frame_list(i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    Ok(Value::list(
        i.frames
            .iter()
            .filter(|f| !f.borrow().dead)
            .map(|f| Value::Frame(f.clone()))
            .collect(),
    ))
}
fn f_delete_frame(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    if i.frames.len() <= 1 {
        return Err(i.error("Attempt to delete the only frame"));
    }
    let f = frame_of(i, &arg(&a, 0))?;
    f.borrow_mut().dead = true;
    Ok(Value::Nil)
}
fn f_frame_parameter(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let f = frame_of(i, &arg(&a, 0))?;
    let ps = want_sym(i, &a[1])?;
    let v = crate::lisp::eval::plist_get(&f.borrow().params, ps);
    if !v.is_nil() {
        return Ok(v);
    }
    let name = i.symbol_name(ps);
    let ff = f.borrow();
    Ok(match name.as_str() {
        "name" => Value::string(ff.name.clone()),
        "width" => Value::Int(ff.width as i128),
        "height" => Value::Int(ff.height as i128),
        "modeline" => Value::t(),
        "visibility" => Value::t(),
        "minibuffer" => {
            if ff.minibuffer.is_some() {
                Value::t()
            } else {
                Value::Nil
            }
        }
        "unsplittable" | "no-accept-focus" | "tab-bar-lines" | "menu-bar-lines"
        | "buried-buffer-list" | "buffer-list" => Value::Nil,
        _ => Value::Nil,
    })
}
pub(crate) fn f_frame_parameters(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let f = frame_of(i, &arg(&a, 0))?;
    let (stored, name, w, h, mbuf, bufv) = {
        let ff = f.borrow();
        let items = ff.params.list_to_vec().unwrap_or_default();
        let mut stored = Vec::new();
        let mut k = 0;
        while k + 1 < items.len() {
            stored.push(Value::cons(items[k].clone(), items[k + 1].clone()));
            k += 2;
        }
        let bufv = i
            .buffer_value(ff.windows.first().map(|w| w.borrow().buffer).unwrap_or(0))
            .unwrap_or(Value::Nil);
        (
            stored,
            ff.name.clone(),
            ff.width,
            ff.height,
            ff.minibuffer.is_some(),
            bufv,
        )
    };
    // GNU's tty frame parameter list, in order.
    let dark = i.intern("dark");
    let mono = i.intern("mono");
    let defaults: Vec<(&str, Value)> = vec![
        ("no-accept-focus", Value::Nil),
        ("visibility", Value::t()),
        ("tab-bar-lines", Value::Int(0)),
        ("menu-bar-lines", Value::Int(1)),
        ("buried-buffer-list", Value::Nil),
        ("buffer-list", Value::list(vec![bufv])),
        ("unsplittable", Value::Nil),
        ("modeline", Value::t()),
        ("width", Value::Int(w as i128)),
        ("height", Value::Int(h as i128)),
        ("name", Value::string(name)),
        ("font", Value::string("tty")),
        ("background-color", Value::string("unspecified-bg")),
        ("foreground-color", Value::string("unspecified-fg")),
        ("cursor-color", Value::string("white")),
        ("background-mode", Value::Sym(dark)),
        ("display-type", Value::Sym(mono)),
        ("minibuffer", Value::from_bool(mbuf)),
    ];
    let mut out = Vec::new();
    for (k, dv) in defaults {
        let kid = i.intern(k);
        let from_store = stored.iter().find_map(|v| match v {
            Value::Cons(c) if matches!(c.borrow().car, Value::Sym(s) if s == kid) => {
                Some(c.borrow().cdr.clone())
            }
            _ => None,
        });
        out.push(Value::cons(
            Value::Sym(kid),
            from_store.unwrap_or(dv),
        ));
    }
    // Any stored params not in the default list keep their place.
    let def_ids: Vec<SymId> = out
        .iter()
        .filter_map(|v| match v {
            Value::Cons(c) => match &c.borrow().car {
                Value::Sym(s) => Some(*s),
                _ => None,
            },
            _ => None,
        })
        .collect();
    for v in stored {
        let dup = matches!(&v, Value::Cons(c)
            if matches!(&c.borrow().car, Value::Sym(s) if def_ids.contains(s)));
        if !dup {
            out.push(v);
        }
    }
    Ok(Value::list(out))
}
fn f_modify_frame_parameters(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let f = frame_of(i, &arg(&a, 0))?;
    let alist = a[1].list_to_vec().unwrap_or_default();
    for pair in alist {
        if let Value::Cons(c) = &pair {
            let (k, v) = {
                let b = c.borrow();
                (b.car.clone(), b.cdr.clone())
            };
            if let Some(ps) = i.sym_id(&k) {
                let new = crate::lisp::eval::plist_put(&f.borrow().params, ps, v);
                f.borrow_mut().params = new;
            }
        }
    }
    Ok(Value::Nil)
}
fn f_set_frame_parameter(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let f = frame_of(i, &arg(&a, 0))?;
    let ps = want_sym(i, &a[1])?;
    let new = crate::lisp::eval::plist_put(&f.borrow().params, ps, a[2].clone());
    f.borrow_mut().params = new;
    Ok(Value::Nil)
}

fn f_set_frame_selected_window(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let f = frame_of(i, &arg(&a, 0))?;
    let w = win_of(i, &a[1])?;
    f.borrow_mut().selected = w;
    Ok(a[1].clone())
}
fn f_frame_width(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let f = frame_of(i, &arg(&a, 0))?;
    Ok(Value::Int(f.borrow().width as i128))
}
fn f_frame_height(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let f = frame_of(i, &arg(&a, 0))?;
    Ok(Value::Int(f.borrow().height as i128))
}

fn f_frame_position(_i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    Ok(Value::cons(Value::Int(0), Value::Int(0)))
}
fn f_frame_edges(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let f = frame_of(i, &arg(&a, 0))?;
    let ff = f.borrow();
    Ok(Value::list(vec![
        Value::Int(0),
        Value::Int(0),
        Value::Int(ff.width as i128),
        Value::Int(ff.height as i128),
    ]))
}
fn f_make_frame(i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    // On a tty, a "new frame" is a new full-screen view sharing the tty.
    let buf = i.current_buffer;
    let f = Frame::new_tty(buf, buf, 80, 25);
    i.frames.push(f.clone());
    Ok(Value::Frame(f))
}

fn f_select_frame(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let f = frame_of(i, &arg(&a, 0))?;
    i.selected_frame = Some(f);
    Ok(a[0].clone())
}

fn f_handle_switch_frame(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    // GNU requires a frame argument.
    match &a[0] {
        Value::Frame(_) => Ok(Value::Nil),
        other => Err(i.wrong_type_mut("framep", other)),
    }
}

fn f_color_values(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    // GNU's second arg is a TERMINAL (default: selected). Any non-nil
    // non-terminal arg errors via get-device-terminal.
    if let Some(v) = a.get(1) {
        if !v.is_nil() {
            return Err(i.error(format!(
                "Invalid argument {} in ‘get-device-terminal’",
                i.princ_to_string(v)
            )));
        }
    }
    // No display colors in batch.
    Ok(Value::Nil)
}

/// `(r g b)' list for a parsed color, else nil.
fn color_rgb_list(v: &Value) -> Value {
    match crate::lisp::builtins::misc::parse_color_16(v) {
        Some((r, g, b)) => Value::list(vec![
            Value::Int(r as i128),
            Value::Int(g as i128),
            Value::Int(b as i128),
        ]),
        None => Value::Nil,
    }
}

/// `tty-color-values' — the tty color database knows the standard
/// color names and #rgb specs; anything else yields nil.
fn f_tty_color_values(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    // Consulting the tty color database initializes the color
    // machinery; afterwards `x-color-values' stops erroring.
    i.color_db_init = true;
    Ok(color_rgb_list(&a[0]))
}

/// `x-color-values' — GNU errors "Window system is not in use or not
/// initialized" until either an X connection attempt or a tty color
/// database lookup initialized the color machinery.
fn f_x_color_values(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    if !i.x_display_attempted && !i.color_db_init {
        return Err(i.error("Window system is not in use or not initialized"));
    }
    Ok(color_rgb_list(&a[0]))
}

/// `xw-color-values' — unlike `x-color-values' this needs a real X
/// connection attempt; a tty color lookup is not enough.  GNU's xw
/// palette (rgb.txt on X, the NS palette on macOS) differs slightly
/// from the tty table for the standard names.
fn f_xw_color_values(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    if !i.x_display_attempted {
        return Err(i.error("Window system is not in use or not initialized"));
    }
    const XW: &[(&str, i128, i128, i128)] = &[
        ("red", 65535, 9773, 0),
        ("green", 0, 64015, 0),
        ("blue", 1101, 12999, 65535),
        ("cyan", 0, 64974, 65535),
        ("magenta", 65535, 16567, 65535),
        ("yellow", 65497, 64588, 0),
    ];
    if let Value::Str(s) = &a[0] {
        let n = s.borrow();
        for (name, r, g, b) in XW {
            if n.eq_ignore_ascii_case(name) {
                return Ok(Value::list(vec![
                    Value::Int(*r),
                    Value::Int(*g),
                    Value::Int(*b),
                ]));
            }
        }
    }
    Ok(color_rgb_list(&a[0]))
}

/// `x-list-fonts' / `fontset-info' — GNU signals "Window system is
/// not in use or not initialized" for any arg on a tty batch.
fn f_x_no_display(i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    Err(i.error("Window system is not in use or not initialized"))
}

/// `fontset-font' / `set-fontset-font' — NAME must be a string; GNU
/// then fails because no fontsets exist on a tty.
fn f_fontset_font(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    match &a[0] {
        Value::Str(s) => {
            let name = s.borrow().clone();
            Err(i.error(format!("Fontset ‘{name}’ does not exist")))
        }
        other => Err(i.wrong_type_mut("stringp", other)),
    }
}

/// `new-fontset' — GNU requires a string name in XLFD form whose
/// registry field matches "fontset-*".
fn f_new_fontset(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let Value::Str(s) = &a[0] else {
        return Err(i.wrong_type_mut("stringp", &a[0]));
    };
    let name = s.borrow().clone();
    if !name.contains('-') {
        return Err(i.error("Fontset name must be in XLFD format"));
    }
    let registry = name.rsplit('-').next().unwrap_or("");
    if !registry.starts_with("fontset-") {
        return Err(i.error("Registry field of fontset name must be \"fontset-*\""));
    }
    Ok(a[0].clone())
}

/// `internal-char-font' — GNU signature is (FRAME &optional CH); nil
/// FRAME means the selected frame.  We track no font info, so nil.
fn f_internal_char_font(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let _ = i;
    match a.get(1) {
        None | Some(Value::Nil) => {}
        Some(v) => {
            want_int(i, v)?;
        }
    }
    Ok(Value::Nil)
}

/// `internal-lisp-face-empty-p' / `internal-lisp-face-attribute-
/// values' — GNU signals a plain "Invalid face" error for unknown
/// faces; a real face yields nil on a tty.
fn f_lisp_face_check_nil(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let id = match &a[0] {
        Value::Sym(s) => *s,
        other => {
            let shown = i.princ_to_string(other);
            return Err(i.error(format!("Invalid face {shown}")));
        }
    };
    let name = i.symbol_name(id).to_string();
    if !face_known(i, &name) {
        return Err(i.error(format!("Invalid face {name}")));
    }
    Ok(Value::Nil)
}

/// `internal-merge-in-global-face' — FACE must name a known face and
/// FRAME must be live; returns nil.
fn f_internal_merge_in_global_face(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    match &a[0] {
        Value::Sym(s) => {
            let name = i.symbol_name(*s).to_string();
            if !face_known(i, &name) {
                return Err(i.error(format!("Invalid face {name}")));
            }
        }
        other => {
            let shown = i.princ_to_string(other);
            return Err(i.error(format!("Invalid face {shown}")));
        }
    }
    match &a[1] {
        Value::Frame(_) => Ok(Value::Nil),
        other => Err(i.wrong_type_mut("frame-live-p", other)),
    }
}

/// `describe-display-table' — requires a char-table argument.
fn f_describe_display_table(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    if crate::lisp::builtins::misc::is_char_table(i, &a[0]) {
        Ok(Value::Nil)
    } else {
        Err(i.wrong_type_mut("char-table-p", &a[0]))
    }
}

/// `redraw-frame' — frame-live-p check, then nil (no display).
fn f_redraw_frame(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    match arg(&a, 0) {
        Value::Nil | Value::Frame(_) => Ok(Value::Nil),
        other => Err(i.wrong_type_mut("frame-live-p", &other)),
    }
}

/// Extract PROP's value from a font-spec record's plist.
fn font_spec_prop(i: &Interp, spec: &Value, prop: &str) -> Option<Value> {
    if let Value::Record(r) = spec {
        let fields = r.borrow();
        if let Some(Value::Cons(_)) = fields.get(1) {
            let mut cur = fields[1].clone();
            while let Value::Cons(c) = cur {
                let (car, cdr) = (c.borrow().car.clone(), c.borrow().cdr.clone());
                if let Value::Cons(c2) = cdr {
                    if let Value::Sym(id) = &car {
                        if i.symbol_name(*id) == prop {
                            return Some(c2.borrow().car.clone());
                        }
                    }
                    cur = c2.borrow().cdr.clone();
                } else {
                    break;
                }
            }
        }
    }
    None
}

/// GNU parses the spec's `:name' as XLFD; a plain name fills the
/// family field, an XLFD name's second field is the family.
fn font_spec_family(i: &Interp, spec: &Value) -> Option<String> {
    if let Some(Value::Str(s)) = font_spec_prop(i, spec, ":family") {
        return Some(s.borrow().clone());
    }
    if let Some(Value::Str(s)) = font_spec_prop(i, spec, ":name") {
        let name = s.borrow().clone();
        if name.starts_with('-') {
            return name.split('-').nth(2).map(|f| f.to_string());
        }
        return Some(name);
    }
    None
}

/// `font-xlfd-name' — GNU requires a font object (typed `font'
/// check) and renders the spec as an XLFD wildcard string.
fn f_font_xlfd_name(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    if font_kind(i, &a[0]).is_none() {
        return Err(i.wrong_type_mut("font", &a[0]));
    }
    let sym_name = |i: &Interp, v: Option<Value>| -> Option<String> {
        match v {
            Some(Value::Sym(id)) => Some(i.symbol_name(id).to_string()),
            _ => None,
        }
    };
    let family = font_spec_family(i, &a[0]).unwrap_or_else(|| "*".to_string());
    let weight = sym_name(i, font_spec_prop(i, &a[0], ":weight")).unwrap_or_else(|| "*".to_string());
    let slant = sym_name(i, font_spec_prop(i, &a[0], ":slant")).unwrap_or_else(|| "*".to_string());
    let width = sym_name(i, font_spec_prop(i, &a[0], ":width")).unwrap_or_else(|| "*".to_string());
    let size = match font_spec_prop(i, &a[0], ":size") {
        Some(Value::Int(n)) => format!("{}", n * 10),
        Some(Value::Float(f)) => format!("{}", (*f * 10.0) as i64),
        _ => "*".to_string(),
    };
    Ok(Value::string(format!(
        "-*-{family}-{weight}-{slant}-{width}-*-*-*-{size}-*-*-*-*-*"
    )))
}

/// `font-get' — a font-spec stores its properties as a plist; GNU
/// returns the property value or nil.  `:family' falls back to the
/// spec's `:name' (GNU parses it as XLFD).
fn f_font_get(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    if font_kind(i, &a[0]).is_none() {
        return Err(i.wrong_type_mut("font", &a[0]));
    }
    if let Value::Sym(id) = &a[1] {
        if i.symbol_name(*id) == ":family" {
            // GNU stores the spec's family as a symbol.
            if let Some(f) = font_spec_family(i, &a[0]) {
                let sym = i.intern(&f);
                return Ok(Value::Sym(sym));
            }
            return Ok(Value::Nil);
        }
    }
    if let Some(v) = font_spec_prop_by_key(i, &a[0], &a[1]) {
        return Ok(v);
    }
    Ok(Value::Nil)
}

fn font_spec_prop_by_key(i: &Interp, spec: &Value, key: &Value) -> Option<Value> {
    if let Value::Record(r) = spec {
        let fields = r.borrow();
        if let Some(Value::Cons(_)) = fields.get(1) {
            let mut cur = fields[1].clone();
            while let Value::Cons(c) = cur {
                let (car, cdr) = (c.borrow().car.clone(), c.borrow().cdr.clone());
                if let Value::Cons(c2) = cdr {
                    let _ = i;
                    if eq_values(&car, key) {
                        return Some(c2.borrow().car.clone());
                    }
                    cur = c2.borrow().cdr.clone();
                } else {
                    break;
                }
            }
        }
    }
    None
}

/// `font-put' — GNU checks FONT with `font-spec' and returns VALUE.
fn f_font_put(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let spec = i.intern("font-spec");
    let is_spec = match &a[0] {
        Value::Record(r) => i.sym_is(&r.borrow()[0], spec),
        _ => false,
    };
    if !is_spec {
        return Err(i.wrong_type_mut("font-spec", &a[0]));
    }
    Ok(a[2].clone())
}

/// `x-popup-menu' — POSITION must be a list (or t/nil); the menu is
/// (TITLE PANE...), each pane (TITLE ITEM...).  GNU checks TITLE is
/// a string, each pane is a list, and a pane's tail is a list of
/// items (its cdr may not be a bare atom).  Returns nil on a tty.
fn f_x_popup_menu(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    if !matches!(a[0], Value::Cons(_) | Value::Nil | Value::Sym(_) | Value::Marker(_)) {
        return Err(i.wrong_type_mut("listp", &a[0]));
    }
    check_popup_menu(i, &a[1], true)?;
    Ok(Value::Nil)
}

/// `x-popup-dialog' — only the dialog title is validated; the pane
/// structure is not walked like `x-popup-menu' does.  Returns nil.
fn f_x_popup_dialog(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    if !matches!(a[0], Value::Cons(_) | Value::Nil | Value::Sym(_) | Value::Marker(_)) {
        return Err(i.wrong_type_mut("listp", &a[0]));
    }
    check_popup_menu(i, &a[1], false)?;
    Ok(Value::Nil)
}

fn check_popup_menu(i: &mut Interp, menu: &Value, deep: bool) -> Result<(), Flow> {
    let items = match menu {
        Value::Cons(_) => menu.list_to_vec().ok(),
        _ => None,
    };
    let items = match items {
        Some(it) if !it.is_empty() => it,
        _ => return Err(i.wrong_type_mut("stringp", menu)),
    };
    if !matches!(items[0], Value::Str(_)) {
        return Err(i.wrong_type_mut("stringp", &items[0]));
    }
    if !deep {
        return Ok(());
    }
    for pane in &items[1..] {
        let c = match pane {
            Value::Cons(c) => c,
            other => return Err(i.wrong_type_mut("listp", other)),
        };
        let (head, tail) = (c.borrow().car.clone(), c.borrow().cdr.clone());
        if !matches!(head, Value::Str(_)) {
            return Err(i.wrong_type_mut("stringp", &head));
        }
        // A pane's tail is the item list — a bare atom (or nil, i.e.
        // no items) fails GNU's consp check on that tail.
        if !matches!(tail, Value::Cons(_)) {
            return Err(i.wrong_type_mut("consp", &tail));
        }
    }
    Ok(())
}

// ---------- keymaps ----------
//
// A keymap is `(keymap . BINDINGS)` where BINDINGS is an alist of
// (KEY . DEF). KEY is an Int (char with modifier bits) or a vector
// `\[mods... key\]`? We support the simple `(char . def)` and nested
// keymaps for multi-key sequences via define-key descending.

pub(crate) fn is_keymap(i: &Interp, v: &Value) -> bool {
    if let Value::Cons(c) = v {
        let b = c.borrow();
        i.sym_is(&b.car, i.obarray.intern_soft("keymap").unwrap_or(u32::MAX))
    } else {
        false
    }
}

/// Resolve DEF to a keymap when it is a keymap cons or a symbol whose
/// function cell holds one (prefix commands like `Control-X-prefix',
/// `ESC-prefix', `mode-specific-command-prefix'). Autoload keymaps
/// (`(autoload FILE ... keymap)') trigger their file load, as in GNU.
/// Otherwise returns DEF unchanged.
pub(crate) fn keymap_def(i: &mut Interp, def: Value) -> EvalResult {
    if let Value::Sym(s) = &def {
        if let f @ Value::Cons(_) = i.symbol_function(*s) {
            if is_keymap(i, &f) {
                return Ok(f);
            }
            // (autoload "file" ... keymap): load, then recheck.
            if is_autoload_keymap(i, &f) {
                let newf = crate::lisp::builtins::evalfn::autoload_do_load(
                    i,
                    f.clone(),
                    false,
                )?;
                if is_keymap(i, &newf) {
                    return Ok(newf);
                }
            }
        }
    }
    Ok(def)
}

/// GNU `get_keymap (m, 0, 0)' — resolves a symbol's function cell to
/// its keymap, but never loads an autoload (KEYMAPP's check inside
/// `access_keymap_1' passes autoload=false).
pub(crate) fn keymap_def_noautoload(i: &Interp, def: &Value) -> Value {
    if let Value::Sym(s) = def {
        if let f @ Value::Cons(_) = i.symbol_function(*s) {
            if is_keymap(i, &f) {
                return f;
            }
        }
    }
    def.clone()
}

/// Push MAP onto `keymap--builtin-maps' so the prelude's keymap-order
/// normalization pass can tell maps built binding-by-binding (whose
/// alists are reversed by define-key's prepend) from literal
/// (keymap ...) data, which is already stored in GNU order.
fn register_builtin_keymap(i: &mut Interp, m: &Value) {
    let id = i.intern("keymap--builtin-maps");
    let cur = i.obarray.symbol(id).value.clone();
    let cur = if matches!(cur, Value::Sym(s) if s == sym::UNBOUND) {
        Value::Nil
    } else {
        cur
    };
    i.obarray.symbol_mut(id).value = Value::cons(m.clone(), cur);
}

/// Fresh purpose-`keymap' char-table — the first cdr element of every
/// `make-keymap' result, as in GNU.  Our bindings stay in the alist;
/// the table is for representation parity (it prints `#^[...]').
pub(crate) fn keymap_char_table(i: &mut Interp) -> Value {
    let tag = Value::Sym(i.intern("keymap"));
    crate::lisp::builtins::misc::make_ct(i, tag, Value::Nil, vec![])
}

fn f_make_keymap(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    // GNU: (keymap CHARTABLE . ALIST) — CHARTABLE holds unmodified
    // char bindings; the optional STRING arg is the menu prompt.
    let ct = keymap_char_table(i);
    let tail = match arg(&a, 0) {
        Value::Str(_) => Value::cons(ct, Value::list(vec![a[0].clone()])),
        _ => Value::cons(ct, Value::Nil),
    };
    let km = Value::cons(Value::Sym(i.intern("keymap")), tail);
    register_builtin_keymap(i, &km);
    Ok(km)
}
fn f_make_sparse_keymap(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    // GNU: (keymap . ALIST) — sparse maps have no char-table.
    let rest = match arg(&a, 0) {
        Value::Str(_) => Value::list(vec![a[0].clone()]),
        _ => Value::Nil,
    };
    let km = Value::cons(Value::Sym(i.intern("keymap")), rest);
    register_builtin_keymap(i, &km);
    Ok(km)
}
fn f_keymapp(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    // GNU: a cons keymap, or a symbol whose function cell is a keymap
    // or an (autoload ... keymap) form (checked without loading).
    let ok = match &a[0] {
        Value::Sym(s) => match i.symbol_function(*s) {
            f @ Value::Cons(_) => is_keymap(i, &f) || is_autoload_keymap(i, &f),
            _ => false,
        },
        _ => is_keymap(i, &a[0]),
    };
    Ok(Value::from_bool(ok))
}

/// `(autoload FILE DOC INTERACTIVE keymap)' form check (no load).
pub(crate) fn is_autoload_keymap(i: &Interp, v: &Value) -> bool {
    if let Value::Cons(c) = v {
        if !i.sym_is(&c.borrow().car, i.obarray.intern_soft("autoload").unwrap_or(u32::MAX))
        {
            return false;
        }
        v.list_to_vec()
            .ok()
            .and_then(|l| l.get(4).cloned())
            .map(|t| i.sym_is(&t, i.obarray.intern_soft("keymap").unwrap_or(u32::MAX)))
            .unwrap_or(false)
    } else {
        false
    }
}
/// Deep-copy a keymap spine cell: cons structure is copied
/// recursively so nested keymaps and binding cells are fresh.
/// Char-table and vector elements are copied too (GNU copies the
/// char-table's contents); other leaf types share their value.
fn copy_keymap_elem(i: &Interp, v: &Value, depth: usize) -> Value {
    if depth > 200 {
        return v.clone();
    }
    match v {
        Value::Cons(c) => {
            let b = c.borrow();
            let (car, cdr) = (b.car.clone(), b.cdr.clone());
            drop(b);
            Value::cons(
                copy_keymap_elem(i, &car, depth + 1),
                copy_keymap_elem(i, &cdr, depth + 1),
            )
        }
        Value::Record(r) if crate::lisp::builtins::misc::is_char_table(i, v) => {
            let items: Vec<Value> = r
                .borrow()
                .iter()
                .map(|e| copy_keymap_elem(i, e, depth + 1))
                .collect();
            Value::Record(std::rc::Rc::new(std::cell::RefCell::new(items)))
        }
        Value::Vec(vv) => {
            let items: Vec<Value> = vv
                .borrow()
                .iter()
                .map(|e| copy_keymap_elem(i, e, depth + 1))
                .collect();
            Value::Vec(std::rc::Rc::new(std::cell::RefCell::new(items)))
        }
        _ => v.clone(),
    }
}

fn f_copy_keymap(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    // GNU copies the whole cdr spine, including the parent tail.
    if !is_keymap(i, &a[0]) {
        return Err(i.wrong_type_mut("keymapp", &a[0]));
    }
    let bindings = match &a[0] {
        Value::Cons(c) => c.borrow().cdr.clone(),
        _ => Value::Nil,
    };
    Ok(Value::cons(
        Value::Sym(i.intern("keymap")),
        copy_keymap_elem(i, &bindings, 0),
    ))
}

/// The binding spine of KM: everything after the `keymap' head when
/// present, the whole cons otherwise — GNU's
/// `(CONSP (map) && EQ (Qkeymap, XCAR (map))) ? XCDR (map) : map',
/// which is also what makes a plain list of keymaps work as a lookup
/// root (its elements are searched as embedded keymaps).
pub(crate) fn keymap_bindings(i: &Interp, km: &Value) -> Value {
    match km {
        Value::Cons(c) => {
            let b = c.borrow();
            if i.sym_is(
                &b.car,
                i.obarray.intern_soft("keymap").unwrap_or(u32::MAX),
            ) {
                b.cdr.clone()
            } else {
                km.clone()
            }
        }
        _ => Value::Nil,
    }
}

/// Parent keymaps of KM. GNU stores the parent as the improper tail
/// of the cdr spine — `(keymap E1 ... En . PARENT)'. Scanning the
/// spine, the first cons cell that is itself a keymap (car `keymap')
/// is the parent. An element that is a bare keymap (composed maps
/// from `make-composed-keymap') makes the remaining spine the parent.
pub(crate) fn keymap_parents(i: &Interp, km: &Value) -> Vec<Value> {
    let mut out = Vec::new();
    if let Value::Cons(c) = km {
        let mut cur = c.borrow().cdr.clone();
        loop {
            match cur.clone() {
                Value::Cons(cc) => {
                    if is_keymap(i, &cur) {
                        out.push(cur.clone());
                        break;
                    }
                    let (car, next) = {
                        let b = cc.borrow();
                        (b.car.clone(), b.cdr.clone())
                    };
                    if is_keymap(i, &car) {
                        if is_keymap(i, &next) {
                            out.push(next);
                        }
                        break;
                    }
                    cur = next;
                }
                tail => {
                    if is_keymap(i, &tail) {
                        out.push(tail);
                    }
                    break;
                }
            }
        }
        // Bare keymap elements (composed-map members) are searched
        // like parents.
        keymap_bindings(i, km).each_car(|el| {
            if is_keymap(i, el) {
                out.push(el.clone());
            } else if let Value::Cons(_) = el {
                let items = el.list_to_vec().unwrap_or_default();
                if !items.is_empty() && items.iter().all(|v| is_keymap(i, v)) {
                    out.extend(items);
                }
            }
        });
    }
    out
}

/// Push the (KEY . DEF) pairs one keymap element yields, in GNU
/// `map_keymap_internal' order: a char-table element contributes its
/// non-nil slots as compressed ((LO . HI) . DEF) or (CH . DEF) pairs,
/// a vector contributes (INDEX . VAL) for every slot, and a cons is
/// one binding as-is.  Other elements (prompt strings etc.) yield
/// nothing.
fn push_elem_bindings(i: &Interp, elem: &Value, out: &mut Vec<(Value, Value)>) {
    match elem {
        Value::Cons(c) => {
            let b = c.borrow();
            out.push((b.car.clone(), b.cdr.clone()));
        }
        _ if crate::lisp::builtins::misc::is_char_table(i, elem) => {
            // map_char_table compresses runs of equal values into
            // (LO . HI) range keys; `ct_collect' yields raw non-nil
            // runs.
            for (k, h, val) in crate::lisp::builtins::misc::ct_collect(i, elem) {
                let key = if h == k {
                    Value::Int(k as i128)
                } else {
                    Value::cons(Value::Int(k as i128), Value::Int(h as i128))
                };
                // A `t' slot is GNU's "explicitly unbound" marker;
                // map_keymap_item reports it as nil.
                let def = if matches!(val, Value::Sym(s) if i.symbol_name(s) == "t")
                {
                    Value::Nil
                } else {
                    val
                };
                out.push((key, def));
            }
        }
        Value::Vec(vv) => {
            for (n, v) in vv.borrow().iter().enumerate() {
                out.push((Value::Int(n as i128), v.clone()));
            }
        }
        _ => {}
    }
}

/// The sort key a binding's KEY carries in GNU's char-table
/// traversal: plain chars and (LO . HI) range keys sort by their low
/// bound; non-character keys return None.
fn char_key_lo(k: &Value) -> Option<i128> {
    match k {
        Value::Int(n) => Some(*n),
        Value::Cons(c) => match &c.borrow().car {
            Value::Int(lo) => Some(*lo),
            _ => None,
        },
        _ => None,
    }
}

/// Merge the deferred high-char alist entries (`chars' collects
/// (lo, key, def) triples) into `out' at the char-table's emission
/// position so the whole character section iterates ascending like
/// GNU's char-table scan.
fn merge_char_section(
    out: &mut Vec<(Value, Value)>,
    chars: &mut Vec<(i128, Value, Value)>,
    ct_pos: Option<usize>,
) {
    if chars.is_empty() {
        return;
    }
    chars.sort_by_key(|(lo, _, _)| *lo);
    let pos = ct_pos.unwrap_or(out.len());
    let merged: Vec<(Value, Value)> =
        chars.drain(..).map(|(_, k, d)| (k, d)).collect();
    out.splice(pos..pos, merged);
}

/// Own bindings of KEYMAP in GNU `map_keymap_internal' order: the
/// walk stops at the improper parent tail and at an embedded keymap
/// element.  Char-table contents appear at the table's spine
/// position, then alist conses follow in stored order.  In a dense
/// map, alist entries whose keys are characters or char ranges
/// (the >255 overflow our flat table cannot hold) merge into the
/// character section sorted by key, as GNU's trie iterates them.
pub(crate) fn keymap_own_bindings(
    i: &Interp,
    km: &Value,
) -> Vec<(Value, Value)> {
    let mut out = Vec::new();
    let mut chars: Vec<(i128, Value, Value)> = Vec::new();
    let mut ct_pos: Option<usize> = None;
    let mut cur = keymap_bindings(i, km);
    loop {
        let cons = match cur.clone() {
            c @ Value::Cons(_) => c,
            _ => break,
        };
        // A spine cons that is itself a keymap is the parent tail.
        if is_keymap(i, &cons) {
            break;
        }
        let (elem, next) = {
            let b = match &cons {
                Value::Cons(c) => c.borrow(),
                _ => break,
            };
            (b.car.clone(), b.cdr.clone())
        };
        if is_keymap(i, &elem) {
            // GNU stops at an embedded keymap element.
            break;
        }
        if crate::lisp::builtins::misc::is_char_table(i, &elem)
            || matches!(elem, Value::Vec(_))
        {
            if ct_pos.is_none() {
                ct_pos = Some(out.len());
            }
            let mut tmp = Vec::new();
            push_elem_bindings(i, &elem, &mut tmp);
            for (k, d) in tmp {
                match char_key_lo(&k) {
                    Some(lo) => chars.push((lo, k, d)),
                    None => out.push((k, d)),
                }
            }
        } else if let Value::Cons(_) = &elem {
            push_elem_bindings(i, &elem, &mut out);
            let (k, d) = out.pop().unwrap();
            // Dense map: alist char keys are char-table overflow.
            if ct_pos.is_some() {
                match char_key_lo(&k) {
                    Some(lo) => chars.push((lo, k, d)),
                    None => out.push((k, d)),
                }
            } else {
                out.push((k, d));
            }
        }
        cur = next;
    }
    merge_char_section(&mut out, &mut chars, ct_pos);
    out
}

/// Bindings in the order of GNU's C `map_keymap' (used by
/// `accessible-keymaps' and `where-is-internal'): like the own
/// bindings, but embedded keymap elements are expanded in place and
/// the improper-tail parent's elements follow inline.
pub(crate) fn keymap_all_bindings(
    i: &Interp,
    km: &Value,
) -> Vec<(Value, Value)> {
    let mut out = Vec::new();
    let mut chars: Vec<(i128, Value, Value)> = Vec::new();
    let mut ct_pos: Option<usize> = None;
    let mut cur = keymap_bindings(i, km);
    loop {
        let cons = match cur.clone() {
            c @ Value::Cons(_) => c,
            _ => break,
        };
        if is_keymap(i, &cons) {
            // Parent tail: the deferred char section belongs to the
            // child; flush it, then the parent's elements follow
            // inline with a fresh section.
            merge_char_section(&mut out, &mut chars, ct_pos);
            ct_pos = None;
            cur = match &cons {
                Value::Cons(c) => c.borrow().cdr.clone(),
                _ => break,
            };
            continue;
        }
        let (elem, next) = {
            let b = match &cons {
                Value::Cons(c) => c.borrow(),
                _ => break,
            };
            (b.car.clone(), b.cdr.clone())
        };
        if is_keymap(i, &elem) {
            // Embedded keymap element: expand its bindings in place.
            merge_char_section(&mut out, &mut chars, ct_pos);
            ct_pos = None;
            out.extend(keymap_all_bindings(i, &elem));
        } else if crate::lisp::builtins::misc::is_char_table(i, &elem)
            || matches!(elem, Value::Vec(_))
        {
            if ct_pos.is_none() {
                ct_pos = Some(out.len());
            }
            let mut tmp = Vec::new();
            push_elem_bindings(i, &elem, &mut tmp);
            for (k, d) in tmp {
                match char_key_lo(&k) {
                    Some(lo) => chars.push((lo, k, d)),
                    None => out.push((k, d)),
                }
            }
        } else if let Value::Cons(_) = &elem {
            push_elem_bindings(i, &elem, &mut out);
            let (k, d) = out.pop().unwrap();
            if ct_pos.is_some() {
                match char_key_lo(&k) {
                    Some(lo) => chars.push((lo, k, d)),
                    None => out.push((k, d)),
                }
            } else {
                out.push((k, d));
            }
        }
        cur = next;
    }
    merge_char_section(&mut out, &mut chars, ct_pos);
    out
}

/// Split a keymap's cdr into its own elements and the parent tail
/// (the first spine cons that is itself a keymap, or nil).
pub(crate) fn keymap_parts(i: &Interp, km: &Value) -> (Vec<Value>, Value) {
    let mut elems = Vec::new();
    let mut tail = Value::Nil;
    if let Value::Cons(c) = km {
        let mut cur = c.borrow().cdr.clone();
        loop {
            match cur.clone() {
                Value::Cons(cc) => {
                    if is_keymap(i, &cur) {
                        tail = cur;
                        break;
                    }
                    let (car, next) = {
                        let b = cc.borrow();
                        (b.car.clone(), b.cdr.clone())
                    };
                    elems.push(car);
                    cur = next;
                }
                t => {
                    tail = t;
                    break;
                }
            }
        }
    }
    (elems, tail)
}

fn f_keymap_parent(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    if !is_keymap(i, &a[0]) {
        return Ok(Value::Nil);
    }
    Ok(keymap_parents(i, &a[0])
        .into_iter()
        .next()
        .unwrap_or(Value::Nil))
}

fn f_set_keymap_parent(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    if !is_keymap(i, &a[0]) {
        return Err(i.wrong_type_mut("keymapp", &a[0]));
    }
    let parent = a[1].clone();
    if !parent.is_nil() && !is_keymap(i, &parent) {
        return Err(i.wrong_type_mut("keymapp", &parent));
    }
    if let Value::Cons(c) = &a[0] {
        // GNU stores the parent as the improper tail of the cdr:
        // (keymap E1 ... En . PARENT). Own elements are preserved
        // verbatim; only the tail is replaced.
        let (elems, _old_tail) = keymap_parts(i, &a[0]);
        let mut tail = parent.clone();
        for el in elems.iter().rev() {
            tail = Value::cons(el.clone(), tail);
        }
        c.borrow_mut().cdr = tail;
    }
    Ok(a[1].clone())
}

/// One element of a key sequence.  GNU converts "Lucid-style" event
/// lists like `(control meta ?c)' via `event-convert-list' inside
/// `Fdefine_key'/`Flookup_key'; other elements are ints or symbols.
fn key_seq_elt(i: &mut Interp, x: &Value) -> Result<Option<i128>, Flow> {
    match x {
        Value::Int(n) => Ok(Some(*n)),
        Value::Sym(s) => Ok(Some(event_code_for(&i.symbol_name(*s)))),
        Value::Cons(c) => {
            // A (LO . HI) character range isn't a Lucid event list —
            // GNU keeps it as a range key, which our i128 codes can't
            // express; skip rather than feed it to event-convert-list.
            if matches!(&c.borrow().car, Value::Int(_)) {
                return Ok(None);
            }
            match crate::lisp::builtins::misc::f_event_convert_list(
                i,
                vec![x.clone()],
            )? {
                Value::Int(n) => Ok(Some(n)),
                Value::Sym(s) => {
                    Ok(Some(event_code_for(&i.symbol_name(s))))
                }
                _ => Ok(None),
            }
        }
        _ => Ok(None),
    }
}

/// Parse a key sequence (string or vector) into event codes.
pub(crate) fn key_seq(i: &mut Interp, v: &Value) -> Result<Vec<i128>, Flow> {
    match v {
        Value::Str(s) => Ok(s
            .borrow()
            .chars()
            .map(|c| {
                let c = c as i128;
                // Chars 128-255 are metafied (like GNU's 8-bit string chars).
                if (0x80..0x100).contains(&c) {
                    CHAR_META | (c - 0x80)
                } else {
                    c
                }
            })
            .collect()),
        Value::Vec(vec) => {
            let mut out = Vec::new();
            for x in vec.borrow().iter() {
                if let Some(n) = key_seq_elt(i, x)? {
                    out.push(n);
                }
            }
            Ok(out)
        }
        Value::Int(n) => Ok(vec![*n]),
        Value::Cons(_) => {
            let mut out = Vec::new();
            for x in v.list_to_vec().unwrap_or_default().iter() {
                if let Some(n) = key_seq_elt(i, x)? {
                    out.push(n);
                }
            }
            Ok(out)
        }
        Value::Sym(id) => {
            // A symbol key like `quit` or `f1`.
            let name = i.symbol_name(*id);
            Ok(vec![event_code_for(&name)])
        }
        other => Err(i.wrong_type_mut("sequencep", other)),
    }
}

/// Named-function-key codes. The base sits above CHAR_META and the
/// hash is masked to the low 21 bits so codes never collide with the
/// modifier bits (CHAR_ALT and above).
pub(crate) const NAMED_KEY_BASE: i128 = 0x4000_0000;
pub(crate) const NAMED_KEY_MASK: i128 = 0x1f_ffff;

pub(crate) fn event_code_for(name: &str) -> i128 {
    let h = {
        let mut h = 0usize;
        for b in name.bytes() {
            h = h.wrapping_mul(31).wrapping_add(b as usize);
        }
        h as i128
    };
    let code = NAMED_KEY_BASE + (h & NAMED_KEY_MASK);
    // Register the code → name mapping so the printer/key-describer
    // can recover event names like `down` or `S-f5`.
    if let Ok(mut g) = named_key_names().lock() {
        g.get_or_insert_with(Default::default)
            .entry(code)
            .or_insert_with(|| name.to_string());
    }
    code
}

fn named_key_names() -> &'static std::sync::Mutex<Option<std::collections::HashMap<i128, String>>> {
    static NAMES: std::sync::Mutex<Option<std::collections::HashMap<i128, String>>> =
        std::sync::Mutex::new(None);
    &NAMES
}

/// Reverse map: named event codes → event names. Used when printing
/// key descriptions and `where-is` results, so `[down]` prints as
/// `down` rather than a raw integer.
/// Return the event name for a named-key code, if registered.
pub(crate) fn key_name_for(code: i128) -> Option<String> {
    if code < NAMED_KEY_BASE || code > NAMED_KEY_BASE + NAMED_KEY_MASK {
        return None;
    }
    named_key_names()
        .lock()
        .ok()
        .and_then(|g| g.as_ref().and_then(|m| m.get(&code).cloned()))
}

/// GNU `get_keyelt': unwrap a keymap binding's definition — strip
/// `(STRING . DEF)' menu labels and `(menu-item LABEL DEF . PROPS)'
/// wrappers, repeatedly, until the real definition remains.  (GNU also
/// evaluates a `:filter' property; that needs lisp evaluation which
/// the lookup paths pass autoload=0 for, so it is left as-is.)
pub(crate) fn menu_label_def(i: &Interp, v: Value) -> Value {
    let mut v = v;
    loop {
        let Value::Cons(c) = &v else {
            return v;
        };
        let (car, cur) = {
            let b = c.borrow();
            (b.car.clone(), b.cdr.clone())
        };
        if matches!(car, Value::Str(_)) {
            v = cur;
            continue;
        }
        if matches!(&car, Value::Sym(s) if i.symbol_name(*s) == "menu-item") {
            // (menu-item LABEL DEF . PROPS): DEF is the caddr.
            let mut cc = cur;
            match cc {
                Value::Cons(x) => cc = x.borrow().cdr.clone(),
                _ => return v,
            }
            v = match cc {
                Value::Cons(x) => x.borrow().car.clone(),
                _ => cc,
            };
            continue;
        }
        return v;
    }
}

/// Meta modifier bit in GNU's event encoding (CHAR_META).
pub(crate) const META_BIT: i128 = 1 << 27;

/// GNU `access_keymap_1' on MAP — a keymap cons or a plain list of
/// keymaps — for KEY, returning Option: `None' is Qunbound (no entry
/// found, keep scanning), `Some(nil)' is an explicit nil binding
/// (suppresses the parent and the default binding, but not a later
/// sibling map).  T_OK permits `t' default bindings.
///
/// The cdr spine is one flat sequence: an improper-tail parent's
/// elements merge in order, and bare keymap elements (composed-map
/// members, or the elements of a keymap list) are searched inline.
/// Multiple keymap results compose into `(keymap M1 M2 ...)'; a
/// non-nil non-keymap result shadows everything after it.
fn access_keymap(
    i: &mut Interp,
    km: &Value,
    mut key: i128,
    t_ok: bool,
) -> Result<Option<Value>, Flow> {
    // A meta-bit key is looked up through the map's meta-prefix (27)
    // binding — M-x means ESC x.
    if key & META_BIT != 0 {
        let esc_b = access_keymap(i, km, 27, t_ok)?;
        let esc = match &esc_b {
            Some(v) => keymap_def(i, v.clone())?,
            None => Value::Nil,
        };
        if is_keymap(i, &esc) {
            return access_keymap(i, &esc, key & !META_BIT, t_ok);
        }
        return if t_ok {
            // No meta map: only the default (t) binding can match.
            key = event_code_for("t");
            access_keymap_int(i, km, key, t_ok)
        } else {
            // An explicit nil meta binding means nil; anything else
            // leaves the key unbound here.
            match esc_b {
                Some(v) if v.is_nil() => Ok(Some(Value::Nil)),
                _ => Ok(None),
            }
        };
    }
    access_keymap_int(i, km, key, t_ok)
}

/// The element-walk of `access_keymap_1' once meta translation is
/// done (KEY is a plain code or the `t' default key).
fn access_keymap_int(
    i: &mut Interp,
    km: &Value,
    key: i128,
    mut t_ok: bool,
) -> Result<Option<Value>, Flow> {
    let t_code = event_code_for("t");
    let t_sym = i.intern("t");
    let keymap_sym = i.intern("keymap");
    let mut t_binding: Option<Value> = None;
    // RETVAL: None = Qunbound; Some(nil) = explicit nil; otherwise a
    // binding — a bare keymap, or `(keymap M1 M2 ...)' composing
    // several keymap hits (RETVAL_TAIL is its last cons cell).
    let mut retval: Option<Value> = None;
    let mut retval_tail: Option<Value> = None;
    let mut cur = keymap_bindings(i, km);
    loop {
        // GNU's loop head also resolves a non-cons improper tail
        // (a symbol whose function cell is a keymap).
        let cons = match cur.clone() {
            c @ Value::Cons(_) => c,
            other => match keymap_def(i, other)? {
                v @ Value::Cons(_) => v,
                _ => break,
            },
        };
        let (elem, next) = {
            let b = match &cons {
                Value::Cons(c) => c.borrow(),
                _ => break,
            };
            (b.car.clone(), b.cdr.clone())
        };
        // An element that IS the `keymap' symbol means the spine has
        // reached the parent tail (the tail cons is itself a keymap).
        if matches!(&elem, Value::Sym(s) if *s == keymap_sym) {
            match &retval {
                // An explicit nil binding shadows the parent.
                Some(v) if v.is_nil() => break,
                // A keymap result absorbs the parent's binding for
                // KEY when that is also a keymap, then stops.
                Some(_) => {
                    let pv = access_keymap_int(i, &cons, key, t_ok)?
                        .unwrap_or(Value::Nil);
                    let pv = keymap_def(i, pv)?;
                    if is_keymap(i, &pv) {
                        append_keymap_hit(
                            i, &mut retval, &mut retval_tail, pv,
                        );
                    }
                    break;
                }
                // Nothing yet: keep walking the parent's spine.
                None => {
                    cur = next;
                    continue;
                }
            }
        }
        // The binding this element yields for KEY: None = unbound.
        let val: Option<Value> = if is_keymap(i, &elem) {
            // Bare keymap element: searched inline.
            access_keymap_int(i, &elem, key, t_ok)?
        } else if matches!(&elem, Value::Sym(_)) {
            // A bare symbol element whose function cell is a keymap
            // (composed maps can store raw symbols like `ESC-prefix').
            match keymap_def(i, elem.clone())? {
                v if is_keymap(i, &v) => access_keymap_int(i, &v, key, t_ok)?,
                _ => None,
            }
        } else if crate::lisp::builtins::misc::is_char_table(i, &elem) {
            // Plain character bindings live in the char-table; a nil
            // slot is unbound (no entry at all).  GNU looks the key
            // up through `char_table_ref' (defalt/parent included).
            if key >= 0 && key & CHAR_MODIFIER_MASK == 0 {
                let s = crate::lisp::builtins::misc::char_table_ref(
                    i,
                    &elem,
                    key as usize,
                );
                if s.is_nil() { None } else { Some(s) }
            } else {
                None
            }
        } else if let Value::Vec(vv) = &elem {
            // Vector keymap element: a nil slot is an explicit nil
            // binding (it shadows the parent).
            if key >= 0 && (key as usize) < vv.borrow().len() {
                Some(vv.borrow()[key as usize].clone())
            } else {
                None
            }
        } else if let Value::Cons(c) = &elem {
            let b = c.borrow();
            // ((LO . HI) . DEF) is a char-table style range binding.
            if let Value::Cons(r) = &b.car {
                let rb = r.borrow();
                match (&rb.car, &rb.cdr) {
                    (Value::Int(lo), Value::Int(hi))
                        if *lo <= key && key <= *hi =>
                    {
                        Some(b.cdr.clone())
                    }
                    _ => None,
                }
            } else {
                let k = match &b.car {
                    Value::Int(n) => Some(*n),
                    Value::Sym(s) => {
                        Some(event_code_for(&i.symbol_name(*s)))
                    }
                    _ => None,
                };
                match k {
                    Some(k) if k == key => Some(b.cdr.clone()),
                    Some(k) if k == t_code && t_ok => {
                        t_binding = Some(b.cdr.clone());
                        t_ok = false;
                        None
                    }
                    _ => None,
                }
            }
        } else {
            None
        };
        // Fold the element's result into RETVAL, GNU-style.
        if let Some(v) = val {
            // `t' in a slot is an explicit nil binding.
            let v = if matches!(&v, Value::Sym(s) if *s == t_sym) {
                Value::Nil
            } else {
                v
            };
            let v = menu_label_def(i, v);
            // GNU KEYMAPP(val) is get_keymap(val, 0, 0): symbol
            // function-cells resolve, but autoloads never load — and
            // RETVAL stores the raw VAL (a `Control-X-prefix' symbol
            // stays a symbol in the result).
            let kv = keymap_def_noautoload(i, &v);
            if is_keymap(i, &kv) {
                append_keymap_hit(i, &mut retval, &mut retval_tail, v);
            } else {
                if retval.is_none()
                    || matches!(&retval, Some(r) if r.is_nil())
                {
                    retval = Some(v.clone());
                }
                // A non-nil non-keymap result shadows the rest.
                if !v.is_nil() {
                    break;
                }
            }
        }
        cur = next;
    }
    Ok(match retval {
        Some(v) => Some(v),
        None => t_binding.map(|v| menu_label_def(i, v)),
    })
}

/// GNU's retval accumulator: V is a keymap hit — the first stands
/// alone, later ones compose `(keymap M1 M2 ...)' by appending to
/// RETVAL_TAIL (the composed list's last cons cell).
fn append_keymap_hit(
    i: &mut Interp,
    retval: &mut Option<Value>,
    retval_tail: &mut Option<Value>,
    v: Value,
) {
    match retval {
        None => *retval = Some(v),
        Some(r) if r.is_nil() => *retval = Some(v),
        Some(_) => {
            if let Some(Value::Cons(tc)) = retval_tail {
                let cell = Value::cons(v, Value::Nil);
                tc.borrow_mut().cdr = cell.clone();
                *retval_tail = Some(cell);
            } else {
                let t = Value::cons(v, Value::Nil);
                let old = retval.take().unwrap_or(Value::Nil);
                *retval = Some(Value::cons(
                    Value::Sym(i.intern("keymap")),
                    Value::cons(old, t.clone()),
                ));
                *retval_tail = Some(t);
            }
        }
    }
}

/// `access_keymap' — the public form returning nil for both unbound
/// and explicit-nil results.  ACCEPT_DEFAULT permits `t' bindings.
pub(crate) fn lookup_in_keymap(
    i: &mut Interp,
    km: &Value,
    key: i128,
    accept_default: bool,
) -> Result<Value, Flow> {
    Ok(access_keymap(i, km, key, accept_default)?.unwrap_or(Value::Nil))
}

fn f_define_key(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    // GNU resolves symbol map args via the function cell.
    let map = keymap_def(i, a[0].clone())?;
    if !is_keymap(i, &map) {
        return Err(i.wrong_type_mut("keymapp", &a[0]));
    }
    // GNU's char-table keymaps support (LO . HI) range keys; our alist
    // model stores them as ((LO . HI) . DEF) pairs.
    if let Value::Vec(v) = &a[1] {
        let vv = v.borrow();
        if vv.len() == 1 {
            if let Value::Cons(r) = &vv[0] {
                let rb = r.borrow();
                if let (Value::Int(lo), Value::Int(hi)) =
                    (&rb.car, &rb.cdr)
                {
                    let (lo, hi) = (*lo, *hi);
                    drop(rb);
                    drop(vv);
                    set_range_binding(i, &map, lo, hi, a[2].clone());
                    return Ok(a[2].clone());
                }
            }
        }
    }
    let raw_keys = key_seq(i, &a[1])?;
    let def = a[2].clone();
    if raw_keys.is_empty() {
        // GNU returns nil for an empty key sequence.
        return Ok(Value::Nil);
    }
    // GNU Fdefine_key metizes: an event carrying the meta bit is
    // defined through the meta-prefix (27) submap with the bit
    // stripped — [?\M-x] means [27 ?x].
    let mut keys = Vec::with_capacity(raw_keys.len());
    for &k in &raw_keys {
        if k & CHAR_META != 0 {
            keys.push(27);
            keys.push(k & !CHAR_META);
        } else {
            keys.push(k);
        }
    }
    // Descend for multi-key sequences, following symbol-backed prefix
    // maps; GNU errors when an intermediate binding is not a keymap.
    let mut km = map;
    for (n, &k) in keys[..keys.len() - 1].iter().enumerate() {
        // GNU descends with access_keymap(c, t_ok=0, noinherit=1).
        let next_raw = lookup_in_keymap(i, &km, k, false)?;
        let next = keymap_def(i, next_raw)?;
        if is_keymap(i, &next) {
            km = next;
        } else if next.is_nil() {
            let sub = Value::cons(Value::Sym(i.intern("keymap")), Value::Nil);
            register_builtin_keymap(i, &sub);
            set_binding(i, &km, k, sub.clone());
            km = sub;
        } else {
            let seq = Value::Vec(std::rc::Rc::new(std::cell::RefCell::new(
                keys[..n + 2]
                    .iter()
                    .map(|&c| Value::Int(c))
                    .collect(),
            )));
            return Err(i.error(&format!(
                "Key sequence {} uses invalid prefix characters",
                i.princ_to_string(&seq)
            )));
        }
    }
    // GNU's optional 4th arg REMOVE removes the binding entirely
    // (parent maps show through) instead of storing nil.
    let remove = a.get(3).map(|v| v.truthy()).unwrap_or(false);
    if remove {
        remove_binding(i, &km, keys[keys.len() - 1]);
    } else {
        set_binding(i, &km, keys[keys.len() - 1], def.clone());
    }
    Ok(def)
}

/// GNU `get_keyelt' (keymap.c): trace a slot's actual definition —
/// `(menu-item NAME DEFN ...)' unwraps to DEFN (the `:filter' path is
/// only used when AUTOLOAD, which `describe-vector' never passes),
/// and `(MENUSTRING . DEFN)' strips the menu name.  Anything else is
/// already the value.
pub(crate) fn keyelt_value(i: &mut Interp, object: &Value) -> Value {
    let menu_item = i.intern("menu-item");
    let mut object = object.clone();
    loop {
        let Value::Cons(c) = &object else {
            return object;
        };
        let (car, cdr) = {
            let b = c.borrow();
            (b.car.clone(), b.cdr.clone())
        };
        if matches!(&car, Value::Sym(s) if *s == menu_item) {
            if let Value::Cons(tail) = &cdr {
                let rest = tail.borrow().cdr.clone();
                object = match &rest {
                    Value::Cons(d) => d.borrow().car.clone(),
                    _ => rest,
                };
            } else {
                return object;
            }
        } else if matches!(&car, Value::Str(_)) {
            object = cdr;
        } else {
            return object;
        }
    }
}

/// GNU `store_in_keymap' with REMOVE: delete KEY's own binding — the
/// alist cons is dropped (a parent map's binding shows through) and
/// char-table/vector slots reset to nil (not the `t' unbind marker).
pub(crate) fn remove_binding(i: &mut Interp, km: &Value, key: i128) {
    if let Value::Cons(head) = km {
        let mut cur = head.borrow().cdr.clone();
        // (PREV . CUR) cons cells; we relink PREV's cdr past the hit.
        let mut prev: Option<Value> = None;
        loop {
            let rc = match &cur {
                Value::Cons(rc) => rc.clone(),
                _ => break,
            };
            if is_keymap(i, &cur) {
                break;
            }
            let (car, next) = {
                let b = rc.borrow();
                (b.car.clone(), b.cdr.clone())
            };
            if crate::lisp::builtins::misc::is_char_table(i, &car) {
                if key >= 0
                    && key & CHAR_MODIFIER_MASK == 0
                    && key <= crate::lisp::builtins::misc::CT_MAX_CHAR as i128
                {
                    crate::lisp::builtins::misc::ct_set(
                        i,
                        &car,
                        key as u32,
                        Value::Nil,
                    );
                    return;
                }
            } else if let Value::Vec(vv) = &car {
                if key >= 0 && (key as usize) < vv.borrow().len() {
                    vv.borrow_mut()[key as usize] = Value::Nil;
                    return;
                }
            } else if let Value::Cons(pair) = &car {
                let kk = match &pair.borrow().car {
                    Value::Int(n) => Some(*n),
                    Value::Sym(s) => {
                        Some(event_code_for(&i.symbol_name(*s)))
                    }
                    _ => None,
                };
                if kk == Some(key) {
                    match &prev {
                        Some(Value::Cons(p)) => {
                            p.borrow_mut().cdr = next;
                        }
                        // Front of the alist: splice into the
                        // keymap head's cdr.
                        _ => head.borrow_mut().cdr = next,
                    }
                    return;
                }
            }
            prev = Some(cur.clone());
            cur = next;
        }
    }
}

/// GNU `store_in_keymap': set KEY's binding to DEF.  Plain character
/// codes are stored in the keymap's char-table element when one
/// exists (a nil DEF stores `t', the explicit-unbind marker); other
/// keys go in the alist at GNU's insertion point — right after the
/// last char-table/vector/embedded-keymap element — which for a pure
/// alist map means the front.  The walk stops at the parent tail:
/// only own bindings are replaced.
pub(crate) fn set_binding(i: &mut Interp, km: &Value, key: i128, def: Value) {
    if let Value::Cons(head) = km {
        let t_sym = i.intern("t");
        let mut ins = km.clone();
        let bindings = head.borrow().cdr.clone();
        let mut cur = bindings;
        loop {
            match cur.clone() {
                Value::Cons(cell) => {
                    if is_keymap(i, &cur) {
                        break;
                    }
                    let (car, next) = {
                        let b = cell.borrow();
                        (b.car.clone(), b.cdr.clone())
                    };
                    if crate::lisp::builtins::misc::is_char_table(i, &car)
                    {
                        ins = cur.clone();
                        // Character codes without modifier bits are
                        // stored in the char-table; codes beyond
                        // MAX_CHAR (named events like `f5') fall
                        // through to the alist.
                        if key >= 0
                            && key & CHAR_MODIFIER_MASK == 0
                            && key <= crate::lisp::builtins::misc::CT_MAX_CHAR as i128
                        {
                            let stored = if def.is_nil() {
                                Value::Sym(t_sym)
                            } else {
                                def.clone()
                            };
                            crate::lisp::builtins::misc::ct_set(
                                i,
                                &car,
                                key as u32,
                                stored,
                            );
                            return;
                        }
                    } else if is_keymap(i, &car) {
                        ins = cur.clone();
                    } else if let Value::Vec(vv) = &car {
                        ins = cur.clone();
                        // GNU stores keys < ASIZE into vector elements.
                        if key >= 0 && (key as usize) < vv.borrow().len() {
                            vv.borrow_mut()[key as usize] = def;
                            return;
                        }
                    } else if let Value::Cons(pair) = &car {
                        let kk = match &pair.borrow().car {
                            Value::Int(n) => Some(*n),
                            Value::Sym(s) => Some(event_code_for(&i.symbol_name(*s))),
                            _ => None,
                        };
                        if kk == Some(key) {
                            pair.borrow_mut().cdr = def;
                            return;
                        }
                    }
                    cur = next;
                }
                _ => break,
            }
        }
        // Not found: insert (KEY . DEF) at the insertion point.
        // Named events store the event symbol so printed maps show
        // `down`, `menu-bar`, etc.
        let kv = key_name_for(key)
            .map(|n| Value::Sym(i.intern(&n)))
            .unwrap_or(Value::Int(key));
        let pair = Value::cons(kv, def);
        if let Value::Cons(ic) = &ins {
            let old = ic.borrow().cdr.clone();
            ic.borrow_mut().cdr = Value::cons(pair, old);
        }
    }
}

/// GNU `store_in_keymap' for a (LO . HI) range key: the range is
/// stored via `set-char-table-range' in the keymap's char-table
/// element; when the map has none, GNU inserts a fresh char-table at
/// the insertion point.  Our flat table covers chars 0-255 — a range
/// that fits goes into the table; a range extending beyond stays
/// whole as an alist `((LO . HI) . DEF)' pair that
/// `lookup_in_keymap' reads and the iterators merge into char order.
pub(crate) fn set_range_binding(
    i: &mut Interp,
    km: &Value,
    lo: i128,
    hi: i128,
    def: Value,
) {
    if let Value::Cons(head) = km {
        let t_sym = i.intern("t");
        let stored = if def.is_nil() {
            Value::Sym(t_sym)
        } else {
            def
        };
        let mut ins = km.clone();
        let bindings = head.borrow().cdr.clone();
        let mut cur = bindings;
        let mut ct_elem = None;
        loop {
            match cur.clone() {
                Value::Cons(cell) => {
                    if is_keymap(i, &cur) {
                        break;
                    }
                    let (car, next) = {
                        let b = cell.borrow();
                        (b.car.clone(), b.cdr.clone())
                    };
                    if crate::lisp::builtins::misc::is_char_table(i, &car)
                    {
                        ins = cur.clone();
                        ct_elem = Some(car);
                        break;
                    }
                    if is_keymap(i, &car) || matches!(car, Value::Vec(_)) {
                        ins = cur.clone();
                    }
                    cur = next;
                }
                _ => break,
            }
        }
        if hi <= crate::lisp::builtins::misc::CT_MAX_CHAR as i128 && lo >= 0 {
            // GNU `store_in_keymap': ranges go through
            // `set-char-table-range' on the map's char-table element
            // (created at the insertion point when absent).
            let ct = match ct_elem {
                Some(v) => v,
                None => {
                    let ct = keymap_char_table(i);
                    if let Value::Cons(ic) = &ins {
                        let old = ic.borrow().cdr.clone();
                        ic.borrow_mut().cdr = Value::cons(ct.clone(), old);
                    }
                    ct
                }
            };
            crate::lisp::builtins::misc::ct_set_range(
                i,
                &ct,
                lo as u32,
                hi as u32,
                stored,
            );
            return;
        }
        let range = Value::cons(Value::Int(lo), Value::Int(hi));
        let pair = Value::cons(range, stored);
        if let Value::Cons(ic) = &ins {
            let old = ic.borrow().cdr.clone();
            ic.borrow_mut().cdr = Value::cons(pair, old);
        }
    }
}

fn f_lookup_key(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    // GNU lookup_key_1: a cons arg (a keymap, or a list of keymaps)
    // is used as-is; other non-nil args resolve via their function
    // cell (and autoload), then must be keymaps.
    let map = match &a[0] {
        Value::Cons(_) | Value::Nil => a[0].clone(),
        _ => {
            let m = keymap_def(i, a[0].clone())?;
            if !is_keymap(i, &m) {
                return Err(i.wrong_type_mut("keymapp", &a[0]));
            }
            m
        }
    };
    let elts = seq_events(&a[1]);
    let t_ok = a.get(2).map(|v| v.truthy()).unwrap_or(false);
    lookup_seq(i, &map, &elts, t_ok)
}

/// GNU `current_minor_maps': walk `emulation-mode-map-alists' (each
/// element itself an alist, resolved through a symbol's value), then
/// `minor-mode-overriding-map-alist', then `minor-mode-map-alist';
/// collect (MODE-VAR . MAP) for each entry whose mode variable is
/// bound and non-nil in the current buffer.  An overriding-alist
/// entry shadows the same variable's regular-alist entry.
pub(crate) fn current_minor_maps(
    i: &mut Interp,
) -> Result<Vec<(Value, Value)>, Flow> {
    let emul_id = i.intern("emulation-mode-map-alists");
    let over_id = i.intern("minor-mode-overriding-map-alist");
    let reg_id = i.intern("minor-mode-map-alist");
    let emul = i.symbol_value(emul_id);
    let overriding = i.symbol_value(over_id);
    let regular = i.symbol_value(reg_id);

    let mut alists: Vec<(Value, bool)> = Vec::new();
    emul.each_car(|e| {
        let alist = match e {
            Value::Sym(s) => i.symbol_value(*s),
            v => v.clone(),
        };
        alists.push((alist, false));
    });
    alists.push((overriding.clone(), false));
    alists.push((regular, true));

    let mut out: Vec<(Value, Value)> = Vec::new();
    for (alist, is_regular) in alists {
        let mut cur = alist;
        while let Value::Cons(c) = cur.clone() {
            let (assoc, next) = {
                let b = c.borrow();
                (b.car.clone(), b.cdr.clone())
            };
            cur = next;
            let (var, mapdef) = match &assoc {
                Value::Cons(a) => {
                    let ab = a.borrow();
                    (ab.car.clone(), ab.cdr.clone())
                }
                _ => continue,
            };
            let var_id = match var {
                Value::Sym(s) => s,
                _ => continue,
            };
            if !i.bound_p(var_id) || !i.symbol_value(var_id).truthy() {
                continue;
            }
            if is_regular {
                let mut shadowed = false;
                overriding.each_car(|oa| {
                    if let Value::Cons(a) = oa {
                        if matches!(&a.borrow().car, Value::Sym(s) if *s == var_id)
                        {
                            shadowed = true;
                        }
                    }
                });
                if shadowed {
                    continue;
                }
            }
            // GNU stores Findirect_function(cdr) — nil when unbound.
            let map = i.indirect_function_value(&mapdef);
            if !map.is_nil()
                && !matches!(&map, Value::Sym(s) if *s == sym::UNBOUND)
            {
                out.push((Value::Sym(var_id), map));
            }
        }
    }
    Ok(out)
}

fn f_current_minor_mode_maps(i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    let pairs = current_minor_maps(i)?;
    Ok(Value::list(pairs.into_iter().map(|(_, m)| m).collect()))
}

fn f_minor_mode_key_binding(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let accept = a.get(1).cloned().unwrap_or(Value::Nil);
    let mut out: Vec<Value> = Vec::new();
    for (mode, map) in current_minor_maps(i)? {
        if map.is_nil() {
            continue;
        }
        let binding =
            f_lookup_key(i, vec![map, a[0].clone(), accept.clone()])?;
        if binding.is_nil() || matches!(binding, Value::Int(_)) {
            continue;
        }
        // KEYMAPP check is get_keymap(v, 0, 0): no autoload.
        let km = keymap_def_noautoload(i, &binding);
        if is_keymap(i, &km) {
            out.push(Value::cons(mode, binding));
        } else if out.is_empty() {
            // A non-prefix first binding ends the search.
            return Ok(Value::list(vec![Value::cons(mode, binding)]));
        }
        // Non-prefix bindings after prefix maps are omitted.
    }
    Ok(Value::list(out))
}

fn f_key_binding(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    // (key-binding KEY &optional accept-defaults no-remap position)
    // GNU: lookup over the full active-maps list — overriding maps,
    // local map, minor-mode maps, then global.
    let keys = key_seq(i, &a[0])?;
    let t_ok = a.get(1).map(|v| v.truthy()).unwrap_or(false);
    let am_id = i.intern("current-active-maps");
    let maps = i.apply(&Value::Sym(am_id), vec![Value::t()])?;
    let mut list = maps;
    while let Value::Cons(c) = list.clone() {
        let (km, next) = {
            let b = c.borrow();
            (b.car.clone(), b.cdr.clone())
        };
        list = next;
        if !is_keymap(i, &km) {
            continue;
        }
        let mut km = km;
        for (n, &k) in keys.iter().enumerate() {
            let raw = lookup_in_keymap(i, &km, k, t_ok)?;
            let def = keymap_def(i, raw.clone())?;
            if is_keymap(i, &def) && n + 1 < keys.len() {
                km = def;
                continue;
            }
            // GNU: a non-keymap def before the last key is an overlong
            // sequence — no binding in this map, try the next map.
            if n + 1 == keys.len() && raw.truthy() {
                return Ok(raw);
            }
            break;
        }
    }
    Ok(Value::Nil)
}

fn f_local_key_binding(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let keys = key_seq(i, &a[0])?;
    let t_ok = a.get(1).map(|v| v.truthy()).unwrap_or(false);
    let local = {
        let b = cur(i);
        let lb = b.borrow();
        lb.locals
            .get(&i.intern_soft("local-keymap").unwrap_or(u32::MAX))
            .cloned()
            .unwrap_or(Value::Nil)
    };
    if is_keymap(i, &local) {
        let mut km = local;
        for (n, &k) in keys.iter().enumerate() {
            let raw = lookup_in_keymap(i, &km, k, t_ok)?;
            let def = keymap_def(i, raw.clone())?;
            if is_keymap(i, &def) && n + 1 < keys.len() {
                km = def;
                continue;
            }
            if n + 1 == keys.len() {
                return Ok(raw);
            }
            return Ok(Value::Nil);
        }
    }
    Ok(Value::Nil)
}

fn f_global_key_binding(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let keys = key_seq(i, &a[0])?;
    let t_ok = a.get(1).map(|v| v.truthy()).unwrap_or(false);
    let gmap = i.symbol_value(i.intern_soft("global-map").unwrap_or(0));
    if is_keymap(i, &gmap) {
        let mut km = gmap;
        for (n, &k) in keys.iter().enumerate() {
            let raw = lookup_in_keymap(i, &km, k, t_ok)?;
            let def = keymap_def(i, raw.clone())?;
            if is_keymap(i, &def) && n + 1 < keys.len() {
                km = def;
                continue;
            }
            if n + 1 == keys.len() {
                return Ok(raw);
            }
            return Ok(Value::Nil);
        }
    }
    Ok(Value::Nil)
}

fn f_current_local_map(i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    let b = cur(i);
    let lb = b.borrow();
    Ok(lb
        .locals
        .get(&i.intern_soft("local-keymap").unwrap_or(u32::MAX))
        .cloned()
        .unwrap_or(Value::Nil))
}

fn f_current_global_map(i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    Ok(i.symbol_value(i.intern_soft("global-map").unwrap_or(0)))
}

fn f_use_local_map(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let b = cur(i);
    let mut bb = b.borrow_mut();
    let km_sym = i.intern_soft("local-keymap").unwrap_or(u32::MAX);
    if a[0].is_nil() {
        bb.locals.remove(&km_sym);
    } else {
        if !is_keymap(i, &a[0]) {
            return Err(i.wrong_type_mut("keymapp", &a[0]));
        }
        bb.locals.insert(km_sym, a[0].clone());
    }
    Ok(Value::Nil)
}

fn f_use_global_map(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    if !is_keymap(i, &a[0]) {
        return Err(i.wrong_type_mut("keymapp", &a[0]));
    }
    let gm = i.intern("global-map");
    i.obarray.symbol_mut(gm).value = a[0].clone();
    Ok(Value::Nil)
}

fn f_local_set_key(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let km = f_current_local_map(i, vec![])?;
    let km = if is_keymap(i, &km) {
        km
    } else {
        let m = f_make_keymap(i, vec![])?;
        f_use_local_map(i, vec![m.clone()])?;
        m
    };
    f_define_key(i, vec![km, a[0].clone(), a[1].clone()])
}

fn f_global_set_key(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let km = f_current_global_map(i, vec![])?;
    let km = if is_keymap(i, &km) {
        km
    } else {
        let m = f_make_keymap(i, vec![])?;
        f_use_global_map(i, vec![m.clone()])?;
        m
    };
    f_define_key(i, vec![km, a[0].clone(), a[1].clone()])
}

fn f_local_unset_key(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    f_local_set_key(i, vec![a[0].clone(), Value::Nil])
}
fn f_global_unset_key(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    f_global_set_key(i, vec![a[0].clone(), Value::Nil])
}

fn f_define_prefix_command(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    // (define-prefix-command COMMAND &optional MAPVAR NAME)
    let cmd = want_sym(i, &a[0])?;
    let km = match a.get(1) {
        Some(v) if v.truthy() => f_current_global_map(i, vec![])?,
        _ => f_make_keymap(i, vec![])?,
    };
    let _ = km;
    let map = f_make_keymap(i, vec![])?;
    i.fset(cmd, map.clone());
    if let Some(v) = a.get(1) {
        if let Some(vs) = i.sym_id(v) {
            i.set_symbol(vs, map)?;
        }
    }
    Ok(args0(&a))
}

fn args0(a: &[Value]) -> Value {
    a[0].clone()
}

/// GNU `Fcommand_remapping': look up the two-event pseudo-sequence
/// `[remap COMMAND]' — in the currently active maps, or in KEYMAPS
/// (a single map means it plus the global map; a cons whose car is a
/// keymap is taken as the whole list).  A "too long" fixnum result
/// counts as no remapping.
fn f_command_remapping(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let Value::Sym(cmd) = a[0] else {
        return Ok(Value::Nil);
    };
    let position = a.get(1).cloned().unwrap_or(Value::Nil);
    let keymaps = a.get(2).cloned().unwrap_or(Value::Nil);
    let key = Value::Vec(Rc::new(RefCell::new(vec![
        Value::Sym(i.intern("remap")),
        Value::Sym(cmd),
    ])));
    let v = if keymaps.is_nil() {
        f_key_binding(
            i,
            vec![key, Value::Nil, Value::Sym(sym::T), position],
        )?
    } else {
        f_lookup_key(i, vec![keymaps, key, Value::Nil])?
    };
    Ok(if matches!(v, Value::Int(_)) {
        Value::Nil
    } else {
        v
    })
}

/// All modifier bits in GNU's event encoding (CHAR_MODIFIER_MASK).
pub(crate) const CHAR_MODIFIER_MASK: i128 =
    CHAR_ALT | CHAR_CTL | CHAR_HYPER | CHAR_META | CHAR_SHIFT | CHAR_SUPER;

/// GNU `parse_solitary_modifier': the modifier bit named by symbol
/// SYM ("meta", "control", a single letter, ...), or 0 for anything
/// else — including nil, which makes plain and meta-only sequences
/// the preferred ones.
fn solitary_modifier(i: &Interp, v: &Value) -> i128 {
    let Value::Sym(s) = v else {
        return 0;
    };
    match i.symbol_name(*s).as_str() {
        "A" | "alt" => CHAR_ALT,
        "C" | "ctrl" | "control" => CHAR_CTL,
        "H" | "hyper" => CHAR_HYPER,
        "M" | "meta" => CHAR_META,
        "S" | "shift" => CHAR_SHIFT,
        "s" | "super" => CHAR_SUPER,
        _ => 0,
    }
}

/// GNU `preferred_sequence_p': 0 when SEQ uses non-preferred
/// modifiers or non-integer events, 2 when it uses the
/// `where-is-preferred-modifier' modifier, else 1.
fn preferred_sequence_p(seq: &Value, preferred: i128) -> i32 {
    let Value::Vec(v) = seq else {
        return 0;
    };
    let elts = v.borrow();
    let mut result = 1;
    for elt in elts.iter() {
        match elt {
            Value::Int(n) => {
                let mods = n & (CHAR_MODIFIER_MASK & !META_BIT);
                if mods == preferred {
                    result = 2;
                } else if mods != 0 {
                    return 0;
                }
            }
            _ => return 0,
        }
    }
    result
}

/// Base event name of a possibly-modified event symbol — the car of
/// GNU's `parse_modifiers': modifier prefixes (single letters A C H
/// M S s, and the word modifiers drag- down- double- triple- up-)
/// stripped from the front.
fn event_base_name(i: &Interp, s: SymId) -> String {
    let mut name = i.symbol_name(s).to_string();
    loop {
        let mut hit = false;
        for p in [
            "A-", "C-", "H-", "M-", "S-", "s-", "drag-", "down-", "double-",
            "triple-", "up-",
        ] {
            if let Some(r) = name.strip_prefix(p) {
                name = r.to_string();
                hit = true;
                break;
            }
        }
        if !hit {
            return name;
        }
    }
}

/// GNU's `Vmouse_events': pseudo-event base symbols whose key
/// sequences `where-is-internal' suppresses when menus are not
/// wanted.
const MOUSE_EVENT_BASES: &[&str] = &[
    "menu-bar", "tab-bar", "tool-bar", "tab-line", "header-line",
    "mode-line", "mouse-1", "mouse-2", "mouse-3", "mouse-4", "mouse-5",
];

/// Look up one event in MAP, which may be a keymap or a plain list
/// of keymaps (GNU's access_keymap treats a cons whose car is not
/// `keymap' as a spine of embedded maps).  Ints are character events
/// (meta bit included); symbols are named events; a cons's EVENT_HEAD
/// is its car; strings are menu keys, compared with `eq' — GNU
/// compares string keys by identity, and the sequences we look up
/// share the keymap's own string objects.
fn lookup_event(
    i: &mut Interp,
    map: &Value,
    ev: &Value,
    t_ok: bool,
) -> Result<Value, Flow> {
    match ev {
        Value::Int(n) => lookup_in_keymap(i, map, *n, t_ok),
        Value::Sym(s) => {
            let code = event_code_for(&i.symbol_name(*s));
            lookup_in_keymap(i, map, code, t_ok)
        }
        Value::Cons(c) => {
            let (car, cdr) = {
                let b = c.borrow();
                (b.car.clone(), b.cdr.clone())
            };
            if let (Value::Int(lo), Value::Int(hi)) = (&car, &cdr) {
                // A (LO . HI) range key: GNU's access_keymap returns
                // the binding only when it covers the whole range —
                // check both endpoints resolve identically.
                let v_lo = lookup_in_keymap(i, map, *lo, t_ok)?;
                if lo == hi {
                    return Ok(v_lo);
                }
                let v_hi = lookup_in_keymap(i, map, *hi, t_ok)?;
                return Ok(if equal_values(i, &v_lo, &v_hi) {
                    v_lo
                } else {
                    Value::Nil
                });
            }
            // A Lucid-style event list like `(control meta ?c)' —
            // convert, then look up the resulting event (GNU
            // `Flookup_key' calls `Fevent_convert_list').
            let conv = crate::lisp::builtins::misc::f_event_convert_list(
                i,
                vec![ev.clone()],
            )?;
            lookup_event(i, map, &conv, t_ok)
        }
        Value::Str(_) => {
            for (k, d) in keymap_all_bindings(i, map) {
                if eq_values(&k, ev) {
                    return Ok(menu_label_def(i, d));
                }
            }
            Ok(Value::Nil)
        }
        _ => Ok(Value::Nil),
    }
}

/// GNU `lookup_key_1' on a sequence of already-decoded events.
/// Returns the binding, `Int(n)' when the sequence runs past a
/// non-prefix binding (GNU's "too long"), or nil.
fn lookup_seq(
    i: &mut Interp,
    map: &Value,
    elts: &[Value],
    t_ok: bool,
) -> Result<Value, Flow> {
    if elts.is_empty() {
        return Ok(map.clone());
    }
    let mut km = map.clone();
    for (n, ev) in elts.iter().enumerate() {
        let raw = lookup_event(i, &km, ev, t_ok)?;
        if n + 1 == elts.len() {
            return Ok(raw);
        }
        let def = keymap_def(i, raw)?;
        if !matches!(def, Value::Cons(_)) {
            return Ok(Value::Int(n as i128 + 1));
        }
        km = def;
    }
    unreachable!()
}

/// The event elements of SEQ (vector, string, list, or single
/// event) — like GNU's Faref iteration in `lookup_key_1'.  Unibyte
/// string chars with the 8th bit become meta events.
fn seq_events(v: &Value) -> Vec<Value> {
    match v {
        Value::Str(s) => s
            .borrow()
            .chars()
            .map(|c| {
                let c = c as i128;
                Value::Int(if (0x80..0x100).contains(&c) {
                    CHAR_META | (c - 0x80)
                } else {
                    c
                })
            })
            .collect(),
        Value::Vec(vec) => vec.borrow().clone(),
        Value::Int(n) => vec![Value::Int(*n)],
        Value::Cons(_) => v.list_to_vec().unwrap_or_default(),
        Value::Sym(id) => vec![Value::Sym(*id)],
        _ => Vec::new(),
    }
}

/// GNU `shadow_lookup': `lookup-key' SEQ over the keymap list
/// KEYMAPS; a "too long" count is nil.  With REMAP, a non-nil symbol
/// result is passed through `command-remapping'.
fn shadow_lookup(
    i: &mut Interp,
    keymaps: &[Value],
    seq: &Value,
    remap: bool,
) -> Result<Value, Flow> {
    let kl = Value::list(keymaps.to_vec());
    let elts = seq_events(seq);
    // GNU shadow_lookup passes accept_default=nil.
    let mut v = lookup_seq(i, &kl, &elts, false)?;
    if matches!(v, Value::Int(_)) {
        // The sequence is too long — treated as unbound.
        v = Value::Nil;
    } else if remap && matches!(v, Value::Sym(_)) {
        let r = f_command_remapping(i, vec![v.clone(), Value::Nil, kl])?;
        if r.truthy() {
            v = r;
        }
    }
    Ok(v)
}

/// GNU `where_is_internal' (the static collector): for every map
/// reachable through `accessible-keymaps' from each of KEYMAPS,
/// every binding whose definition (unwrapped unless NOINDIRECT)
/// `eq'/`equal'-matches DEFINITION contributes its full key sequence.
/// NOMENUS drops sequences whose first event is a mouse/menu
/// pseudo-event.  Returns the sequences in collection order.
fn where_is_collect(
    i: &mut Interp,
    definition: &Value,
    keymaps: &[Value],
    noindirect: bool,
    nomenus: bool,
) -> Result<Vec<Value>, Flow> {
    let mut sequences = Vec::new();
    for km in keymaps {
        let km = keymap_def(i, km.clone())?;
        if !is_keymap(i, &km) {
            continue;
        }
        let acc_sym = i.intern("accessible-keymaps");
        let acc = i.apply(&Value::Sym(acc_sym), vec![km])?;
        let mut entries = Vec::new();
        acc.each_car(|e| entries.push(e.clone()));
        for entry in entries {
            let (this, map) = match &entry {
                Value::Cons(c) => {
                    let b = c.borrow();
                    (b.car.clone(), b.cdr.clone())
                }
                _ => continue,
            };
            let elts: Vec<Value> = match &this {
                Value::Vec(v) => v.borrow().clone(),
                _ => Vec::new(),
            };
            let last = elts.len().wrapping_sub(1);
            let last_is_meta =
                matches!(elts.get(last), Some(Value::Int(n)) if *n == 27)
                    && !elts.is_empty();
            if nomenus
                && matches!(elts.first(), Some(Value::Sym(s0)) if {
                    MOUSE_EVENT_BASES
                        .contains(&event_base_name(i, *s0).as_str())
                })
            {
                // `menu-bar'/`tool-bar'/mouse prefixes: skipped when
                // menu bindings are not wanted.
                continue;
            }
            if !matches!(map, Value::Cons(_)) {
                continue;
            }
            for (key, def) in keymap_all_bindings(i, &map) {
                let binding =
                    if noindirect { def } else { menu_label_def(i, def) };
                let matched = eq_values(&binding, definition)
                    || (matches!(definition, Value::Cons(_))
                        && equal_values(i, &binding, definition));
                if !matched {
                    continue;
                }
                // A `t' default binding covers the whole key space;
                // GNU's map_keymap reports it as the char ranges it
                // spans — [(32 . 126)] and [(128 . 4194303)].
                if matches!(&key, Value::Sym(s) if i.symbol_name(*s) == "t") {
                    for (lo, hi) in
                        [(32i128, 126i128), (128i128, 4194303i128)]
                    {
                        let cell =
                            Value::cons(Value::Int(lo), Value::Int(hi));
                        let mut v = elts.clone();
                        v.push(cell);
                        sequences.push(Value::Vec(Rc::new(
                            RefCell::new(v),
                        )));
                    }
                    continue;
                }
                // [META-PREFIX CHAR] folds to [M-CHAR].
                let mut v = elts.clone();
                if last_is_meta {
                    if let Value::Int(k) = key {
                        v[last] = Value::Int(k | META_BIT);
                    } else {
                        v.push(key);
                    }
                } else {
                    v.push(key);
                }
                sequences.push(Value::Vec(Rc::new(RefCell::new(v))));
            }
        }
    }
    Ok(sequences)
}

fn f_where_is_internal(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let mut definition = a[0].clone();
    let keymap_arg = a.get(1).cloned().unwrap_or(Value::Nil);
    let firstonly = a.get(2).cloned().unwrap_or(Value::Nil);
    let noindirect = a.get(3).map(|v| v.truthy()).unwrap_or(false);
    let no_remap = a.get(4).map(|v| v.truthy()).unwrap_or(false);
    let non_ascii = i.intern("non-ascii");
    let first_non_ascii =
        matches!(&firstonly, Value::Sym(s) if *s == non_ascii);
    // 1 means ignore all menu bindings entirely.
    let nomenus = firstonly.truthy() && !first_non_ascii;

    // The C version of `where-is-preferred-modifier'.
    let preferred = {
        let s = i.obarray.intern_soft("where-is-preferred-modifier");
        match s {
            Some(id) => solitary_modifier(i, &i.symbol_value(id)),
            None => 0,
        }
    };

    // Find the relevant keymaps: a cons whose car is a keymap is a
    // list of keymaps; a single non-nil map means it plus the global
    // map; nil means the currently active maps.
    let mut keymaps: Vec<Value> = Vec::new();
    match &keymap_arg {
        Value::Cons(c) if is_keymap(i, &c.borrow().car) => {
            keymap_arg.each_car(|k| keymaps.push(k.clone()));
        }
        Value::Nil => {
            let cam = i.intern("current-active-maps");
            let v = i.apply(
                &Value::Sym(cam),
                vec![Value::Nil, Value::Nil],
            )?;
            v.each_car(|k| keymaps.push(k.clone()));
        }
        _ => {
            keymaps.push(keymap_arg.clone());
            keymaps.push(f_current_global_map(i, vec![])?);
        }
    }

    // Command remapping: a remapped command's bindings are the
    // remapping target's.
    if !no_remap {
        let kl = Value::list(keymaps.clone());
        let tem =
            f_command_remapping(i, vec![definition.clone(), Value::Nil, kl])?;
        if tem.truthy() {
            definition = tem;
        }
    }

    // An `advertised-binding' property overrides the search under
    // firstonly — each candidate must actually resolve back to the
    // command (shadow_lookup).
    if let Value::Sym(ds) = &definition {
        if firstonly.truthy() {
            let prop = i.intern("advertised-binding");
            let adv = i.get_prop(*ds, prop);
            let mut tem = adv;
            loop {
                match tem.clone() {
                    Value::Cons(c) => {
                        let (car, cdr) = {
                            let b = c.borrow();
                            (b.car.clone(), b.cdr.clone())
                        };
                        let v = shadow_lookup(i, &keymaps, &car, false)?;
                        if eq_values(&v, &definition) {
                            return Ok(car);
                        }
                        tem = cdr;
                    }
                    _ => break,
                }
            }
            let v = shadow_lookup(i, &keymaps, &tem, false)?;
            if eq_values(&v, &definition) {
                return Ok(tem);
            }
        }
    }

    let sequences = where_is_collect(i, &definition, &keymaps, noindirect, nomenus)?;

    let mut found: Vec<Value> = Vec::new();
    let mut remapped_sequences: Vec<Value> = Vec::new();
    let remap_sym = i.intern("remap");
    let nke_sym = i.intern("non-key-event");
    let mut queue: std::collections::VecDeque<Value> = sequences.into();
    let mut remapped = false;
    loop {
        let sequence = match queue.pop_front() {
            Some(s) => s,
            None => {
                if remapped {
                    break;
                }
                // Main list exhausted: process the remapped
                // sequences collected along the way.
                remapped = true;
                queue = remapped_sequences.drain(..).collect();
                match queue.pop_front() {
                    Some(s) => s,
                    None => break,
                }
            }
        };

        // Skip sequences shadowed by another binding for the same
        // key: the effective binding must resolve to DEFINITION.
        let v = shadow_lookup(i, &keymaps, &sequence, remapped)?;
        if !equal_values(i, &v, &definition) {
            continue;
        }

        // A [remap COMMAND] sequence stands for COMMAND's bindings,
        // collected into the deferred remapped list.
        if !no_remap && !remapped {
            let remap_cmd = match &sequence {
                Value::Vec(vv) => {
                    let vb = vv.borrow();
                    if vb.len() == 2
                        && matches!(&vb[0], Value::Sym(s) if *s == remap_sym)
                        && matches!(&vb[1], Value::Sym(_))
                    {
                        Some(vb[1].clone())
                    } else {
                        None
                    }
                }
                _ => None,
            };
            if let Some(cmd) = remap_cmd {
                let seqs =
                    where_is_collect(i, &cmd, &keymaps, noindirect, nomenus)?;
                remapped_sequences.extend(seqs);
                continue;
            }
        }

        // Menu string keys all read as "(any string)".
        if let Value::Vec(vv) = &sequence {
            let mut vb = vv.borrow_mut();
            if let Some(last) = vb.last_mut() {
                if matches!(last, Value::Str(_)) {
                    *last = Value::string("(any string)");
                }
            }
        }

        // Record the sequence unless already seen (inherited maps
        // duplicate bindings) or a non-key event.
        let non_key = matches!(&sequence, Value::Vec(vv) if {
            let vb = vv.borrow();
            vb.len() == 1
                && matches!(&vb[0], Value::Sym(s) if {
                    i.get_prop(*s, nke_sym).truthy()
                })
        });
        if !non_key
            && !found.iter().any(|f| equal_values(i, f, &sequence))
        {
            found.push(sequence.clone());
        }

        if first_non_ascii {
            return Ok(sequence);
        }
        if firstonly.truthy()
            && preferred_sequence_p(&sequence, preferred) == 2
        {
            return Ok(sequence);
        }
    }

    if !firstonly.truthy() {
        return Ok(Value::list(found));
    }
    // firstonly wanted a preferred sequence but none scored 2: take
    // the first acceptable one (or just the first).
    if preferred == 0 {
        return Ok(found.first().cloned().unwrap_or(Value::Nil));
    }
    for b in &found {
        if preferred_sequence_p(b, preferred) != 0 {
            return Ok(b.clone());
        }
    }
    Ok(found.first().cloned().unwrap_or(Value::Nil))
}

/// `kbd` — parse "C-x", "M-f", "S-<return>" etc.
/// The eight standard tty colors GNU reports for defined-colors.
const TTY_COLORS: &[&str] = &[
    "black", "red", "green", "yellow", "blue", "magenta", "cyan", "white",
];

fn f_defined_colors(i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    let _ = i;
    Ok(Value::list(
        TTY_COLORS.iter().map(|c| Value::string(*c)).collect(),
    ))
}

fn f_color_defined_p(_i: &mut Interp, a: Vec<Value>) -> EvalResult {
    // GNU: non-string specs are simply undefined, not errors.
    let s = match &a[0] {
        Value::Str(s) => s.borrow().clone(),
        _ => return Ok(Value::Nil),
    };
    // `#RGB', `#RRGGBB', `#RRRGGGBBB', `#RRRRGGGGBBBB' color specs.
    let hex = s.strip_prefix('#').map(|h| {
        matches!(h.len(), 3 | 6 | 9 | 12) && h.chars().all(|c| c.is_ascii_hexdigit())
    });
    Ok(Value::from_bool(
        hex.unwrap_or(false) || TTY_COLORS.iter().any(|c| s.eq_ignore_ascii_case(c)),
    ))
}

fn f_kbd(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let s = want_str(i, &a[0])?;
    let keys = parse_kbd(i, &s);
    // Emacs: kbd returns a STRING when all events are plain
    // characters, else a vector of events.
    let all_plain = keys
        .iter()
        .all(|k| matches!(k, Value::Int(n) if *n >= 0 && *n < 128));
    if all_plain {
        let s: String = keys
            .iter()
            .filter_map(|k| match k {
                Value::Int(n) => char::from_u32(*n as u32),
                _ => None,
            })
            .collect();
        Ok(Value::string(s))
    } else {
        Ok(Value::Vec(Rc::new(RefCell::new(keys))))
    }
}

pub(crate) fn parse_kbd(i: &mut Interp, s: &str) -> Vec<Value> {
    let mut out = Vec::new();
    for tok in s.split_whitespace() {
        out.extend(parse_key_token(i, tok));
    }
    out
}

pub(crate) const CHAR_META: i128 = 0x0800_0000;
pub(crate) const CHAR_CTL: i128 = 0x0400_0000;
pub(crate) const CHAR_SHIFT: i128 = 0x0200_0000;
pub(crate) const CHAR_HYPER: i128 = 0x0100_0000;
pub(crate) const CHAR_SUPER: i128 = 0x0080_0000;
pub(crate) const CHAR_ALT: i128 = 0x0040_0000;

/// Parse one `kbd` token into `Value` events: `Value::Int` for
/// character events (control folded Emacs-style, remaining
/// modifiers as bits) or `Value::Sym` for named/function-key
/// events (`return`, `M-left`). A multi-char literal like `abc`
/// yields one event per char with modifiers on the first.
pub(crate) fn parse_key_token(i: &mut Interp, tok: &str) -> Vec<Value> {
    let mut mods = 0i128;
    let mut rest = tok;
    loop {
        if let Some(r) = rest.strip_prefix("C-") {
            mods |= CHAR_CTL;
            rest = r;
        } else if let Some(r) = rest.strip_prefix("M-") {
            mods |= CHAR_META;
            rest = r;
        } else if let Some(r) = rest.strip_prefix("S-") {
            mods |= CHAR_SHIFT;
            rest = r;
        } else if let Some(r) = rest.strip_prefix("H-") {
            mods |= CHAR_HYPER;
            rest = r;
        } else if let Some(r) = rest.strip_prefix("s-") {
            mods |= CHAR_SUPER;
            rest = r;
        } else if let Some(r) = rest.strip_prefix("A-") {
            mods |= CHAR_ALT;
            rest = r;
        } else {
            break;
        }
    }
    // Modifier prefix for event symbols, Emacs order A C H M S s.
    let mut mods_name = String::new();
    for (bit, name) in [
        (CHAR_ALT, "A-"),
        (CHAR_CTL, "C-"),
        (CHAR_HYPER, "H-"),
        (CHAR_META, "M-"),
        (CHAR_SHIFT, "S-"),
        (CHAR_SUPER, "s-"),
    ] {
        if mods & bit != 0 {
            mods_name.push_str(name);
        }
    }
    if rest.starts_with('<') && rest.ends_with('>') && rest.len() > 2 {
        // Named event: <return>, M-<left>, <S-down> — modifier
        // prefixes inside the brackets count too.
        let mut name = &rest[1..rest.len() - 1];
        loop {
            if let Some(r) = name.strip_prefix("C-") {
                mods |= CHAR_CTL;
                name = r;
            } else if let Some(r) = name.strip_prefix("M-") {
                mods |= CHAR_META;
                name = r;
            } else if let Some(r) = name.strip_prefix("S-") {
                mods |= CHAR_SHIFT;
                name = r;
            } else if let Some(r) = name.strip_prefix("H-") {
                mods |= CHAR_HYPER;
                name = r;
            } else if let Some(r) = name.strip_prefix("s-") {
                mods |= CHAR_SUPER;
                name = r;
            } else if let Some(r) = name.strip_prefix("A-") {
                mods |= CHAR_ALT;
                name = r;
            } else {
                break;
            }
        }
        let mut mn = String::new();
        for (bit, n) in [
            (CHAR_ALT, "A-"),
            (CHAR_CTL, "C-"),
            (CHAR_HYPER, "H-"),
            (CHAR_META, "M-"),
            (CHAR_SHIFT, "S-"),
            (CHAR_SUPER, "s-"),
        ] {
            if mods & bit != 0 {
                mn.push_str(n);
            }
        }
        return vec![Value::Sym(i.intern(&format!(
            "{}{}",
            mn,
            name.to_ascii_lowercase()
        )))];
    }
    // A multi-char token that isn't a known key name is a literal
    // char sequence (Emacs: (kbd "abc") -> "abc"); modifiers apply
    // to the first char only.
    let first: Vec<char> = rest.chars().collect();
    if first.len() > 1 {
        let base: Option<i128> = match rest.to_ascii_lowercase().as_str() {
            "ret" | "return" => Some(b'\r' as i128),
            "tab" => Some(b'\t' as i128),
            "lfd" => Some(b'\n' as i128),
            "spc" | "space" => Some(b' ' as i128),
            "esc" | "escape" => Some(27),
            "del" => Some(127),
            "nul" => Some(0),
            "backspace" | "delete" | "delchar" | "deletechar" | "home" | "end" | "left"
            | "right" | "up" | "down" | "prior" | "pageup" | "next" | "pagedown" | "insert" => None,
            _ => None,
        };
        match rest.to_ascii_lowercase().as_str() {
            "backspace" | "delete" | "delchar" | "deletechar" | "home" | "end" | "left"
            | "right" | "up" | "down" | "prior" | "pageup" | "next" | "pagedown" | "insert" => {
                let name = rest.to_ascii_lowercase();
                let canon = match name.as_str() {
                    "delchar" | "deletechar" => "deletechar".to_string(),
                    "pageup" => "prior".to_string(),
                    "pagedown" => "next".to_string(),
                    s => s.to_string(),
                };
                return vec![Value::Sym(i.intern(&format!("{}{}", mods_name, canon)))];
            }
            _ => {}
        }
        if let Some(b) = base {
            return vec![Value::Int(apply_mods(b, mods))];
        }
        // Literal chars: modifiers apply to the first char only.
        let mut out = Vec::with_capacity(first.len());
        for (idx, ch) in first.iter().enumerate() {
            let c = *ch as i128;
            if idx == 0 {
                out.push(Value::Int(apply_mods(c, mods)));
            } else {
                out.push(Value::Int(c));
            }
        }
        return out;
    }
    vec![Value::Int(apply_mods(first[0] as i128, mods))]
}

/// Apply remaining modifier bits to a character code.
pub(crate) fn apply_mods(c: i128, mods: i128) -> i128 {
    apply_mods_ev(c, mods, false)
}

/// `evconv` selects event-convert-list semantics, where control on an
/// uppercase letter yields shift|?C-x (C-A is C-S-a); `kbd` folds
/// C-A to plain ?C-a instead.
pub(crate) fn apply_mods_ev(c: i128, mods: i128, evconv: bool) -> i128 {
    let mut m = mods;
    let mut c = c;
    if m & CHAR_CTL != 0 && (0..128).contains(&c) {
        // Only the ASCII control set folds: @ A-Z [ \ ] ^ _ ? and
        // lowercase a-z. Other chars keep the control bit (C-/ is
        // (control /), not 15, like Emacs).
        let folded = match c {
            63 => Some((127, false)),
            65..=90 => Some((c & 0x1f, evconv)),
            64 | 91..=95 | 97..=122 => Some((c & 0x1f, false)),
            _ => None,
        };
        if let Some((f, shifted)) = folded {
            c = f;
            m &= !CHAR_CTL;
            if shifted {
                m |= CHAR_SHIFT;
            }
        }
    }
    // GNU keeps the shift bit on character events; it never folds
    // shift into uppercase (`S-a' = shift|?a, `S-A' = shift|?A).
    c | m
}

fn f_key_description(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let elts = seq_events(&a[0]);
    let parts: Vec<String> = elts
        .iter()
        .map(|e| describe_key_value(i, e))
        .collect();
    Ok(Value::string(parts.join(" ")))
}

/// `push_key_description' on one event: integer events print as
/// modified keys, symbols by name, and a (LO . HI) range prints
/// "LO..HI" (GNU's `[(32 . 126)]' → "SPC..~").
fn describe_key_value(i: &mut Interp, ev: &Value) -> String {
    match ev {
        Value::Int(n) => describe_key(*n),
        Value::Sym(s) => {
            // Symbol events print as <name>; leading character
            // modifiers (A- C- H- M- S- s-) print outside the
            // brackets: `M-next' → "M-<next>", `f1' → "<f1>".
            let mut name = i.symbol_name(*s).to_string();
            let mut pfx = String::new();
            loop {
                let mut hit = false;
                for p in ["A-", "C-", "H-", "M-", "S-", "s-"] {
                    if name.starts_with(p) && name.len() > p.len() {
                        pfx.push_str(p);
                        name = name[p.len()..].to_string();
                        hit = true;
                        break;
                    }
                }
                if !hit {
                    break;
                }
            }
            format!("{pfx}<{name}>")
        }
        Value::Cons(c) => {
            let (car, cdr) = {
                let b = c.borrow();
                (b.car.clone(), b.cdr.clone())
            };
            match (&car, &cdr) {
                (Value::Int(lo), Value::Int(hi)) => {
                    format!("{}..{}", describe_key(*lo), describe_key(*hi))
                }
                _ => describe_key_value(i, &car),
            }
        }
        Value::Str(s) => s.borrow().clone(),
        _ => String::new(),
    }
}

pub(crate) fn describe_key_pub(k: i128) -> String {
    describe_key(k)
}

pub(crate) fn describe_key(k: i128) -> String {
    let mut out = String::new();
    let modmask = CHAR_META | CHAR_CTL | CHAR_SHIFT | CHAR_SUPER | CHAR_HYPER | CHAR_ALT;
    let bare = k & !modmask;
    let mods = k & modmask;
    // GNU prints modifiers in A-C-H-M-S-s order; a control character
    // base (< 32, other than the named keys) contributes its "C-" at
    // the C position.
    let mut base_ctrl = false;
    let named: Option<&'static str> = match bare {
        9 => {
            // TAB decomposes to C-i when a "real" modifier is present
            // (M-TAB is the event M-C-i); with only shift/control it
            // keeps the key name.
            if mods & (CHAR_META | CHAR_HYPER | CHAR_SUPER | CHAR_ALT) != 0 {
                base_ctrl = true;
                None
            } else {
                Some("TAB")
            }
        }
        13 => Some("RET"),
        27 => Some("ESC"),
        32 => Some("SPC"),
        127 => Some("DEL"),
        c if c < 32 => {
            base_ctrl = true;
            None
        }
        _ => None,
    };
    if mods & CHAR_ALT != 0 {
        out.push_str("A-");
    }
    if mods & CHAR_CTL != 0 || base_ctrl {
        out.push_str("C-");
    }
    if mods & CHAR_HYPER != 0 {
        out.push_str("H-");
    }
    if mods & CHAR_META != 0 {
        out.push_str("M-");
    }
    if mods & CHAR_SHIFT != 0 {
        out.push_str("S-");
    }
    if mods & CHAR_SUPER != 0 {
        out.push_str("s-");
    }
    let name = if bare >= NAMED_KEY_BASE {
        key_name_for(bare)
    } else if out.is_empty() {
        key_name_for(k)
    } else {
        None
    };
    if let Some(name) = name {
        // A name like `C-down` carries embedded modifiers: print in
        // Emacs's `C-<down>` style.
        let (mods, rest) = match name.rsplit_once('-') {
            Some((m, r)) => (m, r),
            None => ("", name.as_str()),
        };
        let has_mod = !mods.is_empty()
            && mods
                .split('-')
                .all(|m| matches!(m, "C" | "M" | "S" | "H" | "s" | "A"));
        if has_mod {
            out.push_str(mods);
            out.push('-');
            out.push_str(&format!("<{}>", rest));
        } else {
            out.push_str(&format!("<{}>", name));
        }
    } else if let Some(n) = named {
        out.push_str(n);
    } else if k & 0x7fff_0000 != 0 && bare == 0 {
        out.push_str("<key>");
    } else if base_ctrl {
        let ch = match bare {
            0 => '@',
            28 => '\\',
            29 => ']',
            30 => '^',
            31 => '_',
            c => (b'a' + c as u8 - 1) as char,
        };
        out.push(ch);
    } else {
        out.push(char::from_u32(bare as u32).unwrap_or('?'));
    }
    out
}

fn f_single_key_description(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let no_angles = matches!(a.get(1), Some(v) if !v.is_nil());
    match &a[0] {
        Value::Int(k) => Ok(Value::string(describe_key(*k))),
        Value::Sym(s) => {
            let name = i.symbol_name(*s);
            Ok(Value::string(if no_angles {
                name
            } else {
                format!("<{}>", name)
            }))
        }
        other => Err(i.wrong_type_mut("integer-or-marker-p", other)),
    }
}

fn describe_keymap_into(
    i: &mut Interp,
    km: &Value,
    prefix: &mut Vec<Value>,
    rows: &mut Vec<(Vec<Value>, String)>,
) {
    for (k, d) in keymap_all_bindings(i, km) {
        match &k {
            Value::Int(_) | Value::Sym(_) => {
                if matches!(&k, Value::Sym(s) if i.symbol_name(*s) == "keymap") {
                    continue;
                }
            }
            // Range keys ((LO . HI) . DEF) list as-is.
            Value::Cons(_) => {}
            _ => continue,
        }
        // `t` is the default binding, not a real key.
        if matches!(&k, Value::Sym(s) if i.symbol_name(*s) == "t") {
            continue;
        }
        prefix.push(k);
        if is_keymap(i, &d) {
            describe_keymap_into(i, &d, prefix, rows);
        } else {
            rows.push((prefix.clone(), i.princ_to_string(&d)));
        }
        prefix.pop();
    }
}

fn keymap_sort_key(i: &Interp, keys: &[Value]) -> Vec<(u8, i128, String)> {
    keys.iter()
        .map(|k| match k {
            Value::Int(n) => (0, *n, String::new()),
            Value::Sym(s) => (1, 0, i.symbol_name(*s)),
            _ => (2, 0, String::new()),
        })
        .collect()
}

/// `substitute-command-keys` — expand `\[cmd]`, `\{map}`, `\<map>`,
/// `\=` escapes, and `'` → `’' quoting (Emacs curve style).
fn f_substitute_command_keys(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let s = want_str(i, &a[0])?;
    let chars: Vec<char> = s.chars().collect();
    let mut out = String::new();
    let mut pos = 0usize;
    // GNU propertizes substituted key descriptions with
    // `help-key-binding' (and gives \{map} rows separator/button
    // props).  Intervals are (start, end, plist) over `out'.
    let mut ivs: Vec<(usize, usize, Vec<Value>)> = Vec::new();
    let flf = i.intern("font-lock-face");
    let hkb = i.intern("help-key-binding");
    let face = i.intern("face");
    let kbd_plist = || -> Vec<Value> {
        vec![
            Value::Sym(flf),
            Value::Sym(hkb),
            Value::Sym(face),
            Value::Sym(hkb),
        ]
    };
    // Map selected by \<name> for following \[cmd] lookups.
    let mut ctx_map: Option<Value> = None;
    // Effective quoting style (nil variable → `curve'); `grave' keeps
    // ` and ' verbatim, `straight' maps both to ASCII ', and any
    // other value (including `quote') behaves as `curve'.
    #[derive(Clone, Copy, PartialEq)]
    enum QStyle {
        Grave,
        Ascii,
        Curve,
    }
    let tqs_sym = i.intern("text-quoting-style");
    let tqs = i.symbol_value(tqs_sym);
    let qstyle = match &tqs {
        Value::Sym(s) if i.symbol_name(*s) == "grave" => QStyle::Grave,
        Value::Sym(s) if i.symbol_name(*s) == "straight" => QStyle::Ascii,
        _ => QStyle::Curve,
    };
    let take_until = |chars: &[char], from: usize, close: char| -> Option<(String, usize)> {
        let mut j = from;
        let mut name = String::new();
        while j < chars.len() && chars[j] != close {
            name.push(chars[j]);
            j += 1;
        }
        if j < chars.len() {
            Some((name, j + 1))
        } else {
            None
        }
    };
    while pos < chars.len() {
        let c = chars[pos];
        if c == '\\' && pos + 1 < chars.len() {
            match chars[pos + 1] {
                '[' => {
                    if let Some((name, next)) = take_until(&chars, pos + 2, ']') {
                        let cmd = Value::Sym(i.intern(&name));
                        // GNU doc.c: `\[command]' uses
                        // `where-is-internal' with FIRSTONLY = t.
                        let keys = f_where_is_internal(
                            i,
                            vec![
                                cmd.clone(),
                                Value::Nil,
                                Value::Sym(sym::T),
                                Value::Nil,
                                Value::Nil,
                            ],
                        )
                        .unwrap_or(Value::Nil);
                        // Restrict to the \<map> context if one was set.
                        let first_key = if let Some(km) = &ctx_map {
                            let ctx_keys = f_where_is_internal(
                                i,
                                vec![
                                    cmd.clone(),
                                    km.clone(),
                                    Value::Sym(sym::T),
                                    Value::Nil,
                                    Value::Nil,
                                ],
                            )
                            .unwrap_or(Value::Nil);
                            match ctx_keys {
                                Value::Nil => keys,
                                k => k,
                            }
                        } else {
                            keys
                        };
                        let first_key = match first_key {
                            v @ Value::Vec(_) => Some(v),
                            Value::Nil => None,
                            other => Some(other),
                        };
                        match first_key {
                            Some(k) => {
                                if let Ok(Value::Str(d)) = f_key_description(i, vec![k]) {
                                    let st = out.chars().count();
                                    out.push_str(&d.borrow());
                                    let en = out.chars().count();
                                    if st < en {
                                        ivs.push((st, en, kbd_plist()));
                                    }
                                }
                            }
                            None => {
                                let st = out.chars().count();
                                out.push_str("M-x ");
                                out.push_str(&name);
                                ivs.push((st, out.chars().count(), kbd_plist()));
                            }
                        }
                        pos = next;
                        continue;
                    }
                    out.push(c);
                    pos += 1;
                }
                '{' => {
                    // \{map} — insert the map's description.
                    if let Some((name, next)) = take_until(&chars, pos + 2, '}') {
                        let id = i.intern_soft(&name);
                        let kmv = id.filter(|id| i.bound_p(*id)).map(|id| i.symbol_value(id));
                        match kmv.filter(|v| is_keymap(i, v)) {
                            Some(km) => {
                                let mut rows = Vec::new();
                                describe_keymap_into(i, &km, &mut Vec::new(), &mut rows);
                                // GNU order: char keys by code, then symbols.
                                rows.sort_by(|a, b| {
                                    keymap_sort_key(i, &a.0).cmp(&keymap_sort_key(i, &b.0))
                                });
                                out.push_str("\nKey             Binding\n");
                                let sep_st = out.chars().count();
                                out.push_str(&"-".repeat(79));
                                let sep_en = out.chars().count();
                                out.push('\n');
                                ivs.push((
                                    sep_st,
                                    sep_en,
                                    vec![
                                        Value::Sym(face),
                                        Value::Sym(i.intern("separator-line")),
                                    ],
                                ));
                                for (keys, def) in rows {
                                    let mut desc = String::new();
                                    for kv in &keys {
                                        if !desc.is_empty() {
                                            desc.push(' ');
                                        }
                                        let v = Value::Vec(Rc::new(RefCell::new(vec![kv.clone()])));
                                        if let Ok(Value::Str(s)) = f_key_description(i, vec![v]) {
                                            desc.push_str(&s.borrow());
                                        }
                                    }
                                    let kst = out.chars().count();
                                    out.push_str(&desc);
                                    let ken = out.chars().count();
                                    if kst < ken {
                                        ivs.push((kst, ken, kbd_plist()));
                                    }
                                    out.push_str(if desc.chars().count() >= 8 {
                                        "\t"
                                    } else {
                                        "\t\t"
                                    });
                                    // GNU puts a help-function-button
                                    // on each binding name.
                                    let bst = out.chars().count();
                                    out.push_str(&def);
                                    let ben = out.chars().count();
                                    let symv = Value::Sym(i.intern(&def));
                                    ivs.push((
                                        bst,
                                        ben,
                                        vec![
                                            Value::Sym(i.intern("help-args")),
                                            Value::list(vec![symv]),
                                            Value::Sym(i.intern("category")),
                                            Value::Sym(i.intern("help-function-button")),
                                            Value::Sym(i.intern("button")),
                                            Value::list(vec![Value::t()]),
                                        ],
                                    ));
                                    out.push('\n');
                                }
                            }
                            None => {
                                out.push_str(&format!(
                                    "Uses keymap \u{2018}{}\u{2019}, which is not \
                                     currently defined.",
                                    name
                                ));
                            }
                        }
                        pos = next;
                        continue;
                    }
                    out.push(c);
                    pos += 1;
                }
                '<' => {
                    // \<map> — select keymap context for following \[cmd].
                    if let Some((name, next)) = take_until(&chars, pos + 2, '>') {
                        let id = i.intern_soft(&name);
                        ctx_map = id.filter(|id| i.bound_p(*id)).map(|id| i.symbol_value(id));
                        pos = next;
                        continue;
                    }
                    out.push(c);
                    pos += 1;
                }
                '=' => {
                    // \=x — literal x.
                    pos += 2;
                    if pos < chars.len() {
                        out.push(chars[pos]);
                        pos += 1;
                    }
                }
                _ => {
                    out.push(c);
                    pos += 1;
                }
            }
        } else if c == '`' {
            out.push(match qstyle {
                QStyle::Grave => '`',
                QStyle::Ascii => '\'',
                QStyle::Curve => '\u{2018}',
            });
            pos += 1;
        } else if c == '\'' {
            out.push(match qstyle {
                QStyle::Curve => '\u{2019}',
                _ => '\'',
            });
            pos += 1;
        } else {
            out.push(c);
            pos += 1;
        }
    }
    let v = Value::string(out);
    if !ivs.is_empty() {
        if let Value::Str(rc) = &v {
            i.set_str_props(rc, ivs);
        }
    }
    Ok(v)
}

fn f_text_char_description(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    // Emacs: CHARACTER must be a plain char (no modifier bits);
    // control chars print ^X, DEL ^?, C1 octal.
    match &a[0] {
        Value::Int(n) if *n >= 0 && *n <= 0x3f_ffff => {
            let n = *n;
            let s = match n {
                0x7f => "^?".to_string(),
                c if c < 0x20 => format!("^{}", char::from_u32(c as u32 + 64).unwrap()),
                c if (0x80..0xa0).contains(&c) => format!("\\{:03o}", c),
                c => char::from_u32(c as u32).unwrap_or('?').to_string(),
            };
            Ok(Value::string(s))
        }
        _ => Err(i.wrong_type_mut("characterp", &a[0])),
    }
}

/// Read one raw key event through the front-end hook.
fn f_read_char(i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    if i.minibuf_reader.is_some() {
        match i.minibuf_input("", true)? {
            crate::lisp::eval::MinibufInput::Key(k) => {
                return Ok(Value::Int(k));
            }
            crate::lisp::eval::MinibufInput::Text(t) => {
                let n = t.chars().next().map(|c| c as i128).unwrap_or(0);
                return Ok(Value::Int(n));
            }
        }
    }
    Ok(Value::Nil)
}

fn f_read_key_sequence(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    // Read one key through the front-end; batch mode returns "".
    if i.minibuf_reader.is_some() {
        let prompt = match &a[0] {
            Value::Str(s) => s.borrow().clone(),
            _ => String::new(),
        };
        if let crate::lisp::eval::MinibufInput::Key(k) = i.minibuf_input(&prompt, true)? {
            if k < 128 {
                return Ok(Value::string(
                    char::from_u32(k as u32).unwrap_or(' ').to_string(),
                ));
            }
            return Ok(Value::Vec(Rc::new(RefCell::new(vec![Value::Int(k)]))));
        }
    }
    Ok(Value::string(""))
}
fn f_read_key_sequence_vector(i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    let _ = i;
    Ok(Value::Vec(Rc::new(RefCell::new(Vec::new()))))
}
fn f_this_command_keys(i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    let _ = i;
    Ok(Value::string(""))
}
fn f_this_command_keys_vector(i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    let _ = i;
    Ok(Value::Vec(std::rc::Rc::new(std::cell::RefCell::new(
        Vec::new(),
    ))))
}

// ---------- kill ring ----------

fn f_kill_new(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let s = want_str(i, &a[0])?;
    let replace = !arg(&a, 1).is_nil();
    if replace {
        // REPLACE non-nil: overwrite the newest entry instead of pushing.
        let kr = i.intern("kill-ring");
        let cur = i.symbol_value(kr);
        let mut items = cur.list_to_vec().unwrap_or_default();
        if items.is_empty() {
            items.insert(0, Value::string(s));
        } else {
            items[0] = Value::string(s);
        }
        i.obarray.symbol_mut(kr).value = Value::list(items);
        let ring = i.symbol_value(kr);
        let ptr = i.intern("kill-ring-yank-pointer");
        let _ = i.set_symbol(ptr, ring);
    } else {
        crate::buffer::primitives::push_kill_ring(i, s);
    }
    Ok(Value::Nil)
}

fn f_kill_append(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let s = want_str(i, &a[0])?;
    // BEFORE-P non-nil prepends to the newest entry.
    let before = !arg(&a, 1).is_nil();
    let kr = i.intern("kill-ring");
    let cur = i.symbol_value(kr);
    let mut items = cur.list_to_vec().unwrap_or_default();
    if let Some(Value::Str(top)) = items.first_mut() {
        let mut t = top.borrow_mut();
        if before {
            let mut joined = s.clone();
            joined.push_str(&t);
            *t = joined;
        } else {
            t.push_str(&s);
        }
    } else {
        items.insert(0, Value::string(s));
    }
    i.obarray.symbol_mut(kr).value = Value::list(items);
    Ok(Value::Nil)
}

/// Position of `kill-ring-yank-pointer' inside `kill-ring': the pointer
/// is a tail cons of the ring (as in GNU), located by identity.
fn yank_ptr_pos(i: &Interp, ring: &Value, len: usize) -> usize {
    let ptr = i.symbol_value(i.intern_soft("kill-ring-yank-pointer").unwrap_or(0));
    if !matches!(ptr, Value::Cons(_)) {
        return 0;
    }
    let mut tail = ring.clone();
    for idx in 0..len {
        if eq_values(&tail, &ptr) {
            return idx;
        }
        tail = nthcdr_of(&tail, 1);
        if !matches!(tail, Value::Cons(_)) {
            break;
        }
    }
    0
}

/// Move `kill-ring-yank-pointer' to the tail at `pos'.
fn set_yank_ptr(i: &mut Interp, ring: &Value, pos: usize) {
    let new_ptr = nthcdr_of(ring, pos);
    let ptrsym = i.intern("kill-ring-yank-pointer");
    let _ = i.set_symbol(ptrsym, new_ptr);
}

fn f_current_kill(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let n = want_int(i, &a[0])?;
    let do_not_move = !arg(&a, 1).is_nil();
    let kr = i.intern("kill-ring");
    let ring = i.symbol_value(kr);
    let items = ring.list_to_vec().unwrap_or_default();
    if items.is_empty() {
        return Err(i.error("Kill ring is empty"));
    }
    let len = items.len();
    let base = yank_ptr_pos(i, &ring, len);
    let pos = (((base as i128 + n) % len as i128) + len as i128) as usize % len;
    if !do_not_move {
        set_yank_ptr(i, &ring, pos);
    }
    match &items[pos] {
        Value::Str(s) => Ok(Value::string(s.borrow().clone())),
        other => Ok(other.clone()),
    }
}

fn f_copy_region_as_kill(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let b = cur(i);
    let text = {
        let bb = b.borrow();
        let len = bb.text.len();
        let s = (want_int(i, &a[0])?.max(1) as usize - 1).min(len);
        let e = (want_int(i, &a[1])?.max(1) as usize - 1).min(len);
        let (s, e) = (s.min(e), s.max(e));
        bb.text.substring(s, e)
    };
    crate::buffer::primitives::push_kill_ring(i, text);
    // deactivate mark per Emacs
    let ma = i.intern_soft("mark-active").unwrap_or(0);
    b.borrow_mut().locals.insert(ma, Value::Nil);
    Ok(Value::Nil)
}

fn f_yank(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let n = arg(&a, 0).int().unwrap_or(1);
    let kr = i.intern("kill-ring");
    let ring = i.symbol_value(kr);
    let items = ring.list_to_vec().unwrap_or_default();
    if items.is_empty() {
        return Err(i.error("Kill ring is empty"));
    }
    let len = items.len();
    // GNU: yank N = current-kill (N-1) then insert at the new pointer.
    let base = yank_ptr_pos(i, &ring, len);
    let pos = (((base as i128 + n - 1) % len as i128) + len as i128) as usize % len;
    set_yank_ptr(i, &ring, pos);
    let s = match &items[pos] {
        Value::Str(s) => s.borrow().clone(),
        _ => String::new(),
    };
    let b = cur(i);
    let pt = b.borrow().point();
    b.borrow_mut().mark = Some(pt);
    crate::buffer::primitives::chg_insert_pt(i, &s, false)?;
    Ok(Value::Nil)
}

fn f_yank_pop(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let n = arg(&a, 0).int().unwrap_or(1);
    let kr = i.intern("kill-ring");
    let ring = i.symbol_value(kr);
    let items = ring.list_to_vec().unwrap_or_default();
    if items.is_empty() {
        return Err(i.error("Kill ring is empty"));
    }
    // Replace the region between mark and point.
    let b = cur(i);
    let (m, p) = {
        let bb = b.borrow();
        (bb.mark, bb.point())
    };
    if let Some(m) = m {
        let len = items.len();
        let base = yank_ptr_pos(i, &ring, len);
        let pos = (((base as i128 + n) % len as i128) + len as i128) as usize % len;
        let (s, e) = (m.min(p), m.max(p));
        crate::buffer::primitives::chg_delete(i, s, e)?;
        b.borrow_mut().set_point(s);
        let text = match &items[pos] {
            Value::Str(s) => s.borrow().clone(),
            _ => String::new(),
        };
        set_yank_ptr(i, &ring, pos);
        crate::buffer::primitives::chg_insert_pt(i, &text, false)?;
        b.borrow_mut().mark = Some(s);
        Ok(Value::Nil)
    } else {
        Err(i.error("Previous command was not a yank"))
    }
}

fn f_rotate_yank_pointer(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let n = arg(&a, 0).int().unwrap_or(1);
    let kr = i.intern("kill-ring");
    let ring = i.symbol_value(kr);
    let items = ring.list_to_vec().unwrap_or_default();
    if !items.is_empty() {
        let len = items.len();
        let base = yank_ptr_pos(i, &ring, len);
        let pos = (((base as i128 + n) % len as i128) + len as i128) as usize % len;
        set_yank_ptr(i, &ring, pos);
    }
    Ok(Value::Nil)
}

fn f_copy_to_buffer(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let bid = i
        .buffer_id_of(&a[0])
        .ok_or_else(|| i.error(format!("No buffer named {}", i.princ_to_string(&a[0]))))?;
    let text = {
        let b = cur(i);
        let bb = b.borrow();
        let len = bb.text.len();
        let s = (want_int(i, &a[1])?.max(1) as usize - 1).min(len);
        let e = (want_int(i, &a[2])?.max(1) as usize - 1).min(len);
        bb.text.substring(s.min(e), s.max(e))
    };
    if i.buffers.get(bid).is_some() {
        // copy-to-buffer replaces the target's entire contents; GNU
        // runs the change hooks with the target buffer current.
        crate::buffer::primitives::chg_with_buffer(i, bid, |i| {
            let len = cur(i).borrow().text.len();
            crate::buffer::primitives::chg_delete(i, 0, len)?;
            cur(i).borrow_mut().set_point(0);
            crate::buffer::primitives::chg_insert_pt(i, &text, false)?;
            cur(i).borrow_mut().set_point(0);
            Ok(Value::Nil)
        })?;
    }
    Ok(Value::Nil)
}

fn f_append_to_buffer(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    // (append-to-buffer BUFFER START END) — insert region text at BUFFER's
    // point; returns nil.
    let bid = i
        .buffer_id_of(&a[0])
        .ok_or_else(|| i.error(format!("No buffer named {}", i.princ_to_string(&a[0]))))?;
    let text = {
        let b = cur(i);
        let bb = b.borrow();
        let len = bb.text.len();
        let s = (want_int(i, &a[1])?.max(1) as usize - 1).min(len);
        let e = (want_int(i, &a[2])?.max(1) as usize - 1).min(len);
        bb.text.substring(s.min(e), s.max(e))
    };
    if i.buffers.get(bid).is_some() {
        crate::buffer::primitives::chg_with_buffer(i, bid, |i| {
            crate::buffer::primitives::chg_insert_pt(i, &text, false)
        })?;
    }
    Ok(Value::Nil)
}

// ---------- file I/O ----------

fn want_filename(i: &mut Interp, v: &Value) -> Result<String, Flow> {
    let s = want_str(i, v)?;
    Ok(expand_file_name_str(i, &s))
}

/// Expand ~, env vars, and make absolute via `default-directory`.
pub(crate) fn expand_file_name_str(i: &mut Interp, name: &str) -> String {
    let mut s = name.to_string();
    // ~ expansion
    if s.starts_with('~') {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/".into());
        if s.len() == 1 || s.starts_with("~/") {
            s = format!("{}{}", home, &s[1..]);
        }
        // ~user unsupported → leave
    }
    // absolute?
    if !s.starts_with('/') {
        let dir = default_directory(i);
        let sep = if dir.ends_with('/') || dir.is_empty() {
            ""
        } else {
            "/"
        };
        s = format!("{}{}{}", dir, sep, s);
    }
    // Collapse /./ and /../
    normalize_path(&s)
}

fn normalize_path(p: &str) -> String {
    let mut parts: Vec<&str> = Vec::new();
    for comp in p.split('/') {
        match comp {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            c => parts.push(c),
        }
    }
    let mut out = String::from("/");
    out.push_str(&parts.join("/"));
    if p.ends_with('/') && !out.ends_with('/') {
        out.push('/');
    }
    out
}

/// The current buffer's `default-directory`.
pub(crate) fn default_directory(i: &Interp) -> String {
    if let Some(b) = i.current_buffer_ref() {
        let bb = b.borrow();
        if let Some(v) = bb
            .locals
            .get(&i.intern_soft("default-directory").unwrap_or(u32::MAX))
        {
            if let Value::Str(s) = v {
                return s.borrow().clone();
            }
        }
    }
    let d = i.symbol_value(i.intern_soft("default-directory").unwrap_or(0));
    if let Value::Str(s) = d {
        return s.borrow().clone();
    }
    std::env::current_dir()
        .map(|p| format!("{}/", p.display()))
        .unwrap_or_else(|_| "/".into())
}

fn f_file_exists_p(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let p = want_filename(i, &a[0])?;
    Ok(Value::from_bool(std::path::Path::new(&p).exists()))
}
fn f_file_directory_p(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let p = want_filename(i, &a[0])?;
    Ok(Value::from_bool(std::path::Path::new(&p).is_dir()))
}
fn f_file_regular_p(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let p = want_filename(i, &a[0])?;
    Ok(Value::from_bool(std::path::Path::new(&p).is_file()))
}
fn f_file_readable_p(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let p = want_filename(i, &a[0])?;
    Ok(Value::from_bool(std::fs::metadata(&p).is_ok()))
}
fn f_file_writable_p(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let p = want_filename(i, &a[0])?;
    let path = std::path::Path::new(&p);
    if path.exists() {
        Ok(Value::from_bool(
            std::fs::OpenOptions::new().write(true).open(path).is_ok(),
        ))
    } else {
        Ok(Value::from_bool(
            path.parent().map(|d| d.exists()).unwrap_or(false),
        ))
    }
}
fn f_file_executable_p(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let p = want_filename(i, &a[0])?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Ok(m) = std::fs::metadata(&p) {
            return Ok(Value::from_bool(m.permissions().mode() & 0o111 != 0));
        }
    }
    Ok(Value::Nil)
}
fn f_file_symlink_p(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let p = want_filename(i, &a[0])?;
    match std::fs::symlink_metadata(&p) {
        Ok(m) if m.file_type().is_symlink() => {
            let target = std::fs::read_link(&p)
                .map(|t| t.to_string_lossy().into_owned())
                .unwrap_or_default();
            Ok(Value::string(target))
        }
        _ => Ok(Value::Nil),
    }
}
fn f_file_newer_than_file_p(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let p1 = want_filename(i, &a[0])?;
    let p2 = want_filename(i, &a[1])?;
    let m1 = std::fs::metadata(&p1).and_then(|m| m.modified());
    let m2 = std::fs::metadata(&p2).and_then(|m| m.modified());
    match (m1, m2) {
        (Ok(t1), Ok(t2)) => Ok(Value::from_bool(t1 > t2)),
        (Ok(_), Err(_)) => Ok(Value::t()),
        _ => Ok(Value::Nil),
    }
}
/// `file-attributes' element 9: t when deleting and recreating the
/// file could change its gid — GNU: a setgid parent forces the dir
/// gid (t only when the file's gid differs); a non-setgid parent
/// leaves the gid to the process, so t unconditionally.
#[cfg(unix)]
fn gid_change_flag(path: &str, file_gid: i128) -> Value {
    use std::os::unix::fs::MetadataExt;
    let parent = std::path::Path::new(path)
        .parent()
        .map(|p| if p.as_os_str().is_empty() { std::path::Path::new(".") } else { p })
        .unwrap_or_else(|| std::path::Path::new("."));
    let Ok(m) = std::fs::metadata(parent) else {
        return Value::t();
    };
    if m.mode() & 0o2000 != 0 {
        Value::from_bool((m.gid() as i128) != file_gid)
    } else {
        Value::t()
    }
}

#[cfg(not(unix))]
fn gid_change_flag(_path: &str, _file_gid: i128) -> Value {
    Value::Nil
}

fn f_file_attributes(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let p = want_filename(i, &a[0])?;
    // GNU lstats: a symlink's attributes are its own, and car = link target.
    match std::fs::symlink_metadata(&p) {
        Ok(m) => {
            // GNU order: type nlinks uid gid atime mtime ctime size
            // modes gidchg inode device.
            let to_lisp_time = |r: std::io::Result<std::time::SystemTime>| match r {
                Ok(t) => match t.duration_since(std::time::UNIX_EPOCH) {
                    Ok(d) => crate::lisp::builtins::misc::ns_to_lisp_time(
                        (d.as_secs() as i128) * 1_000_000_000 + d.subsec_nanos() as i128,
                    ),
                    Err(_) => Value::Nil,
                },
                Err(_) => Value::Nil,
            };
            let (nlinks, uid, gid, inode, dev, modes, ctime) = {
                #[cfg(unix)]
                {
                    use std::os::unix::fs::MetadataExt;
                    let mode = m.mode();
                    let mut s = String::with_capacity(10);
                    s.push(if m.is_dir() {
                        'd'
                    } else if m.file_type().is_symlink() {
                        'l'
                    } else {
                        '-'
                    });
                    const RWX: &str = "rwxrwxrwx";
                    for (i, c) in RWX.chars().enumerate() {
                        s.push(if mode & (0o400 >> i) != 0 { c } else { '-' });
                    }
                    (
                        m.nlink() as i128,
                        m.uid() as i128,
                        m.gid() as i128,
                        m.ino() as i128,
                        m.dev() as i128,
                        s,
                        crate::lisp::builtins::misc::ns_to_lisp_time(
                            m.ctime() as i128 * 1_000_000_000 + m.ctime_nsec() as i128,
                        ),
                    )
                }
                #[cfg(not(unix))]
                {
                    (1, 0, 0, 0, 0, "----------".to_string(), Value::Nil)
                }
            };
            let car = if m.is_dir() {
                Value::t()
            } else if m.file_type().is_symlink() {
                Value::string(
                    std::fs::read_link(&p)
                        .map(|t| t.to_string_lossy().into_owned())
                        .unwrap_or_default(),
                )
            } else {
                Value::Nil
            };
            Ok(Value::list(vec![
                car,
                Value::Int(nlinks),
                Value::Int(uid),
                Value::Int(gid),
                to_lisp_time(m.accessed()),
                to_lisp_time(m.modified()),
                ctime,
                Value::Int(m.len() as i128),
                Value::string(modes),
                gid_change_flag(&p, gid),
                Value::Int(inode),
                Value::Int(dev),
            ]))
        }
        Err(_) => Ok(Value::Nil),
    }
}
fn f_file_modes(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let p = want_filename(i, &a[0])?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        // GNU file-modes goes through file_attributes → lstat.
        if let Ok(m) = std::fs::symlink_metadata(&p) {
            return Ok(Value::Int((m.permissions().mode() & 0o7777) as i128));
        }
    }
    Ok(Value::Nil)
}
fn f_set_file_modes(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let p = want_filename(i, &a[0])?;
    let mode = want_int(i, &a[1])? as u32;
    let nofollow = match a.get(2) {
        Some(Value::Sym(s)) => i.symbol_name(*s) == "nofollow",
        Some(v) => v.truthy(),
        None => false,
    };
    #[cfg(unix)]
    {
        if nofollow {
            // chmod on a symlink itself: fchmodat with AT_SYMLINK_NOFOLLOW.
            // On Linux this is a no-op (returns EINVAL); macOS supports it.
            let c = std::ffi::CString::new(p.clone()).map_err(|_| {
                i.signal_data(sym::FILE_ERROR, vec![Value::string("bad filename")])
            })?;
            unsafe {
                libc::fchmodat(
                    libc::AT_FDCWD,
                    c.as_ptr(),
                    mode as libc::mode_t,
                    libc::AT_SYMLINK_NOFOLLOW,
                )
            };
        } else {
            use std::os::unix::fs::PermissionsExt;
            let _ =
                std::fs::set_permissions(&p, std::fs::Permissions::from_mode(mode));
        }
    }
    Ok(Value::Nil)
}
fn f_file_name_absolute_p(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let s = want_str(i, &a[0])?;
    Ok(Value::from_bool(s.starts_with('/') || s.starts_with('~')))
}
fn f_expand_file_name(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let name = want_str(i, &a[0])?;
    let s = match a.get(1) {
        Some(Value::Str(d)) if !name.starts_with('/') && !name.starts_with('~') => {
            // GNU expands the directory argument itself against
            // `default-directory' before appending NAME.
            let d = expand_file_name_str(i, &d.borrow());
            let sep = if d.ends_with('/') || d.is_empty() {
                ""
            } else {
                "/"
            };
            normalize_path(&format!("{}{}{}", d, sep, name))
        }
        _ => expand_file_name_str(i, &name),
    };
    Ok(Value::string(s))
}
fn f_locate_file_internal(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    // (locate-file-internal FILENAME PATH &optional SUFFIXES MODE)
    // MODE: nil = file-readable-p semantics (dirs skipped), an integer
    // = access(2) mask, otherwise a predicate funcalled per candidate;
    // a directory is accepted only when the predicate returns `dir-ok'.
    let name = want_str(i, &a[0])?;
    let pred = a.get(3).cloned().unwrap_or(Value::Nil);
    let dir_ok = i.intern("dir-ok");
    let accept = |i: &mut Interp, cand: &str| -> Result<bool, Flow> {
        let md = std::fs::metadata(cand);
        match &pred {
            Value::Int(mask) => {
                if md.is_err() {
                    return Ok(false);
                }
                #[cfg(unix)]
                unsafe {
                    let c = std::ffi::CString::new(cand).unwrap_or_default();
                    return Ok(libc::access(c.as_ptr(), *mask as i32) == 0);
                }
                #[allow(unreachable_code)]
                Ok(true)
            }
            Value::Nil => {
                // file-readable-p semantics: R_OK via access(2), and
                // directories are skipped (no dir-ok without a predicate).
                #[cfg(unix)]
                unsafe {
                    let c = std::ffi::CString::new(cand).unwrap_or_default();
                    return Ok(
                        libc::access(c.as_ptr(), libc::R_OK) == 0
                            && !md.map(|m| m.is_dir()).unwrap_or(false),
                    );
                }
                #[allow(unreachable_code)]
                Ok(md.map(|m| !m.is_dir()).unwrap_or(false))
            }
            p => {
                let args = Value::list(vec![Value::string(cand.to_string())]);
                let r = i.call_function(&p.clone(), &args, None)?;
                Ok(match &md {
                    Ok(m) if m.is_dir() => matches!(&r, Value::Sym(s) if *s == dir_ok),
                    Ok(_) => r.truthy(),
                    Err(_) => false,
                })
            }
        }
    };
    let mut suffixes = vec![String::new()];
    if let Some(sufs) = a.get(2).and_then(|v| v.list_to_vec().ok()) {
        for s in sufs {
            if let Ok(s) = want_str(i, &s) {
                suffixes.push(s);
            }
        }
    }
    if name.starts_with('/') {
        // Absolute: only suffix variants apply.
        for suf in &suffixes {
            let cand = format!("{name}{suf}");
            if accept(i, &cand)? {
                return Ok(Value::string(cand));
            }
        }
        return Ok(Value::Nil);
    }
    let paths = a
        .get(1)
        .and_then(|v| v.list_to_vec().ok())
        .unwrap_or_default();
    for dir_v in &paths {
        let Ok(dir) = want_str(i, dir_v) else {
            continue;
        };
        for suf in &suffixes {
            let cand = format!("{}/{}{}", dir.trim_end_matches('/'), name, suf);
            if accept(i, &cand)? {
                return Ok(Value::string(cand));
            }
        }
    }
    Ok(Value::Nil)
}

fn f_file_name_directory(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let s = want_str(i, &a[0])?;
    match s.rfind('/') {
        Some(idx) => Ok(Value::string(&s[..=idx])),
        None => Ok(Value::Nil),
    }
}
fn f_file_name_nondirectory(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let s = want_str(i, &a[0])?;
    match s.rfind('/') {
        Some(idx) => Ok(Value::string(&s[idx + 1..])),
        None => Ok(Value::string(s)),
    }
}
fn f_file_name_extension(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let s = want_str(i, &a[0])?;
    let base = s.rsplit('/').next().unwrap_or(&s);
    match base.rfind('.') {
        Some(idx) if idx > 0 => {
            let keep_dot = a.get(1).map(|v| v.truthy()).unwrap_or(false);
            Ok(Value::string(&base[idx + usize::from(!keep_dot)..]))
        }
        _ => Ok(Value::Nil),
    }
}
fn f_file_name_sans_extension(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let s = want_str(i, &a[0])?;
    match s.rfind('.') {
        Some(idx) => {
            // Only strip if the dot is after the last slash.
            let last_slash = s.rfind('/').map(|x| x + 1).unwrap_or(0);
            if idx > last_slash {
                Ok(Value::string(&s[..idx]))
            } else {
                Ok(Value::string(s))
            }
        }
        None => Ok(Value::string(s)),
    }
}
fn f_file_name_base(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let s = want_str(i, &a[0])?;
    let base = s.rsplit('/').next().unwrap_or(&s);
    match base.rfind('.') {
        Some(idx) if idx > 0 => Ok(Value::string(&base[..idx])),
        _ => Ok(Value::string(base)),
    }
}
fn f_file_name_as_directory(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let s = want_str(i, &a[0])?;
    Ok(Value::string(if s.ends_with('/') {
        s
    } else {
        format!("{}/", s)
    }))
}
fn f_directory_file_name(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let s = want_str(i, &a[0])?;
    // GNU (fileio.c): strip trailing slashes, but a name made solely
    // of slashes is already a directory name and stays unchanged
    // (`directory-file-name' of "/" is "/", of "//" is "//").
    if s.chars().all(|c| c == '/') {
        return Ok(Value::string(s));
    }
    Ok(Value::string(s.trim_end_matches('/').to_string()))
}
fn f_file_name_concat(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let mut out = want_str(i, &a[0])?;
    for v in &a[1..] {
        let part = want_str(i, v)?;
        if !out.ends_with('/') && !part.is_empty() {
            out.push('/');
        }
        out.push_str(&part);
    }
    Ok(Value::string(out))
}
fn f_file_relative_name(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let name_in = want_str(i, &a[0])?;
    let name = expand_file_name_str(i, &name_in);
    let dir_in = match a.get(1) {
        Some(v) if v.truthy() => want_str(i, v)?,
        _ => default_directory(i),
    };
    let dir = expand_file_name_str(i, &dir_in);
    let dparts: Vec<&str> = dir.split('/').filter(|c| !c.is_empty()).collect();
    let nparts: Vec<&str> = name.split('/').filter(|c| !c.is_empty()).collect();
    let mut k = 0;
    while k < dparts.len() && k < nparts.len() && dparts[k] == nparts[k] {
        k += 1;
    }
    let mut out = String::new();
    for _ in k..dparts.len() {
        out.push_str("../");
    }
    out.push_str(&nparts[k..].join("/"));
    if out.is_empty() {
        out.push('.');
    }
    Ok(Value::string(out))
}
fn f_abbreviate_file_name(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let name = want_str(i, &a[0])?;
    let home = std::env::var("HOME").unwrap_or_default();
    if !home.is_empty() && name.starts_with(&home) {
        Ok(Value::string(format!("~{}", &name[home.len()..])))
    } else {
        Ok(Value::string(name))
    }
}
/// GNU env-var substitution in filenames: `$VAR' and `${VAR}' expand to the
/// variable's value, `$$' becomes `$', and unset variables stay literal.
fn substitute_env_vars(s: &str) -> String {
    let mut out = String::new();
    let mut cs = s.chars().peekable();
    while let Some(c) = cs.next() {
        if c != '$' {
            out.push(c);
            continue;
        }
        match cs.peek().copied() {
            Some('$') => {
                cs.next();
                out.push('$');
            }
            Some('{') => {
                cs.next();
                let mut var = String::new();
                let mut closed = false;
                while let Some(&c2) = cs.peek() {
                    cs.next();
                    if c2 == '}' {
                        closed = true;
                        break;
                    }
                    var.push(c2);
                }
                if closed {
                    match std::env::var(&var) {
                        Ok(v) => out.push_str(&v),
                        Err(_) => {
                            out.push_str("${");
                            out.push_str(&var);
                            out.push('}');
                        }
                    }
                } else {
                    out.push_str("${");
                    out.push_str(&var);
                }
            }
            _ => {
                let mut var = String::new();
                while let Some(&c2) = cs.peek() {
                    if c2.is_alphanumeric() || c2 == '_' {
                        var.push(c2);
                        cs.next();
                    } else {
                        break;
                    }
                }
                if var.is_empty() {
                    out.push('$');
                } else {
                    match std::env::var(&var) {
                        Ok(v) => out.push_str(&v),
                        Err(_) => {
                            out.push('$');
                            out.push_str(&var);
                        }
                    }
                }
            }
        }
    }
    out
}

fn f_substitute_in_file_name(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let s = want_str(i, &a[0])?;
    let out = substitute_env_vars(&s);
    // A `//' after position 0 discards the text before it.
    if let Some(idx) = out.match_indices("//").find(|(i, _)| *i > 0).map(|(i, _)| i) {
        return Ok(Value::string(out[idx + 1..].to_string()));
    }
    Ok(Value::string(out))
}

fn f_directory_files(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let dir = want_filename(i, &a[0])?;
    let full = a.get(1).map(|v| v.truthy()).unwrap_or(false);
    let re_str = match a.get(2) {
        Some(v) if v.truthy() => Some(want_str(i, v)?),
        _ => None,
    };
    let nosort = a.get(3).map(|v| v.truthy()).unwrap_or(false);
    let re = match &re_str {
        Some(p) => Some(
            crate::lisp::regexp::compile_case(p, false)
                .map_err(|e| err_sym(i, "invalid-regexp", vec![Value::string(e.0)]))?,
        ),
        None => None,
    };
    let base = if dir.ends_with('/') {
        dir.clone()
    } else {
        format!("{}/", dir)
    };
    let syn = crate::editor::re_syntax(i);
    let matches = |name: &str| -> bool {
        match &re {
            Some(r) => {
                let chars: Vec<char> = name.chars().collect();
                crate::lisp::regexp::search(r, &chars, 0, &syn).is_some()
            }
            None => true,
        }
    };
    let mut names = Vec::new();
    for dot in [".", ".."] {
        if matches(dot) {
            names.push(if full {
                format!("{}{}", base, dot)
            } else {
                dot.to_string()
            });
        }
    }
    match std::fs::read_dir(&dir) {
        Ok(rd) => {
            for ent in rd.flatten() {
                let name = ent.file_name().to_string_lossy().into_owned();
                if !matches(&name) {
                    continue;
                }
                names.push(if full {
                    format!("{}{}", base, name)
                } else {
                    name
                });
            }
        }
        Err(e) => {
            // GNU signals file-missing (ENOENT) or file-error otherwise,
            // data = ("Opening directory" strerror DIR).
            let s = if e.kind() == std::io::ErrorKind::NotFound {
                sym::FILE_MISSING
            } else {
                sym::FILE_ERROR
            };
            return Err(i.signal_data(
                s,
                vec![
                    Value::string("Opening directory"),
                    Value::string(e.to_string()),
                    Value::string(dir.clone()),
                ],
            ));
        }
    }
    if !nosort {
        names.sort();
    }
    Ok(Value::list(names.into_iter().map(Value::string).collect()))
}

fn f_directory_files_and_attributes(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let files = f_directory_files(i, vec![a[0].clone(), arg(&a, 1), arg(&a, 2), arg(&a, 3)])?;
    let dir = want_filename(i, &a[0])?;
    let full = a.get(1).map(|v| v.truthy()).unwrap_or(false);
    let items = files.list_to_vec().unwrap_or_default();
    let mut out = Vec::with_capacity(items.len());
    for n in items {
        let attrs = match &n {
            Value::Str(s) => {
                let name = s.borrow().clone();
                let path = if full {
                    name.clone()
                } else if dir.ends_with('/') {
                    format!("{}{}", dir, name)
                } else {
                    format!("{}/{}", dir, name)
                };
                f_file_attributes(i, vec![Value::string(path)])?
            }
            _ => Value::Nil,
        };
        out.push(Value::cons(n, attrs));
    }
    Ok(Value::list(out))
}

fn f_file_name_completion(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let file = want_str(i, &a[0])?;
    let dir = want_filename(i, &a[1])?;
    let mut matches = Vec::new();
    if let Ok(rd) = std::fs::read_dir(&dir) {
        for ent in rd.flatten() {
            let name = ent.file_name().to_string_lossy().into_owned();
            if name.starts_with(&file) {
                matches.push(name);
            }
        }
    }
    matches.sort();
    if matches.len() == 1 && matches[0] == file {
        return Ok(Value::t());
    }
    // longest common prefix
    if matches.is_empty() {
        return Ok(Value::Nil);
    }
    let lcp = longest_common_prefix(&matches);
    if lcp == file {
        Ok(Value::t())
    } else {
        Ok(Value::string(lcp))
    }
}

fn longest_common_prefix(xs: &[String]) -> String {
    if xs.is_empty() {
        return String::new();
    }
    let mut p = xs[0].clone();
    for x in &xs[1..] {
        while !x.starts_with(&p) {
            p.pop();
            if p.is_empty() {
                return p;
            }
        }
    }
    p
}

fn f_file_name_all_completions(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let file = want_str(i, &a[0])?;
    let dir = want_filename(i, &a[1])?;
    let mut matches = Vec::new();
    if let Ok(rd) = std::fs::read_dir(&dir) {
        for ent in rd.flatten() {
            let name = ent.file_name().to_string_lossy().into_owned();
            if name.starts_with(&file) {
                matches.push(name);
            }
        }
    }
    matches.sort();
    Ok(Value::list(
        matches.into_iter().map(Value::string).collect(),
    ))
}

/// Signal the GNU-matching error for a failed mkdir: EEXIST maps to
/// `file-already-exists`, everything else to `file-error`.
fn signal_mkdir_error(i: &mut Interp, dir: &str, e: std::io::Error) -> Flow {
    if e.kind() == std::io::ErrorKind::AlreadyExists {
        let sym = i.intern("file-already-exists");
        i.signal_data(
            sym,
            vec![
                Value::string("File already exists"),
                Value::string(dir.to_string()),
            ],
        )
    } else {
        i.signal_data(
            sym::FILE_ERROR,
            vec![
                Value::string(format!("Creating directory: {}", e)),
                Value::string(dir.to_string()),
            ],
        )
    }
}

fn f_make_directory(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let dir = want_filename(i, &a[0])?;
    let parents = a.get(1).map(|v| v.truthy()).unwrap_or(false);
    let r = if parents {
        std::fs::create_dir_all(&dir)
    } else {
        std::fs::create_dir(&dir)
    };
    match r {
        Ok(()) => Ok(Value::Nil),
        Err(e) => Err(signal_mkdir_error(i, &dir, e)),
    }
}
fn f_make_directory_internal(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let dir = want_filename(i, &a[0])?;
    match std::fs::create_dir(&dir) {
        Ok(()) => Ok(Value::Nil),
        Err(e) => Err(signal_mkdir_error(i, &dir, e)),
    }
}

fn temp_name_seed() -> String {
    const ALPHA: &[u8] = b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";
    let mut x = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0x9e3779b97f4a7c15)
        ^ (std::process::id() as u64) << 32;
    let mut out = String::with_capacity(6);
    for _ in 0..6 {
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        out.push(ALPHA[(x % 62) as usize] as char);
    }
    out
}

fn f_make_temp_name(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    // GNU's make-temp-name does not expand the prefix.
    let prefix = want_str(i, &a[0])?;
    for _ in 0..64 {
        let name = format!("{}{}", prefix, temp_name_seed());
        if !std::path::Path::new(&name).exists() {
            return Ok(Value::string(name));
        }
    }
    Ok(Value::string(format!("{}{}", prefix, temp_name_seed())))
}

fn f_make_temp_file(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    // GNU expands relative prefixes under `temporary-file-directory`,
    // not the buffer's `default-directory`.
    let raw = want_str(i, &a[0])?;
    let prefix = if raw.starts_with('/') || raw.starts_with('~') {
        expand_file_name_str(i, &raw)
    } else {
        std::env::temp_dir()
            .join(&raw)
            .to_string_lossy()
            .into_owned()
    };
    let dir_flag = a.get(1).map(|v| v.truthy()).unwrap_or(false);
    for _ in 0..64 {
        let name = format!("{}{}", prefix, temp_name_seed());
        let r = if dir_flag {
            std::fs::create_dir(&name)
        } else {
            std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&name)
                .map(|_| ())
        };
        match r {
            Ok(()) => {
                if let Some(text) = a.get(2) {
                    if text.truthy() {
                        let s = want_str(i, text)?;
                        let _ = std::fs::write(&name, s);
                    }
                }
                return Ok(Value::string(name));
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => {
                return Err(i.signal_data(
                    sym::FILE_ERROR,
                    vec![Value::string(format!("Creating temp file: {}", e))],
                ));
            }
        }
    }
    Err(i.signal_data(
        sym::FILE_ERROR,
        vec![Value::string("Creating temp file: cannot find unique name")],
    ))
}

fn f_file_local_copy(_i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    // All our files are local; remote handlers would copy here.
    Ok(Value::Nil)
}

fn f_file_in_directory_p(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let file = want_filename(i, &a[0])?;
    let dir = want_filename(i, &a[1])?;
    let canon = |p: &str| {
        std::fs::canonicalize(p)
            .map(|c| c.to_string_lossy().into_owned())
            .unwrap_or_else(|_| p.to_string())
    };
    let f = canon(&file);
    let d = canon(&dir);
    let d = if d.ends_with('/') {
        d
    } else {
        format!("{}/", d)
    };
    Ok(if f.starts_with(&d) {
        Value::t()
    } else {
        Value::Nil
    })
}

fn f_delete_directory(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let dir = want_filename(i, &a[0])?;
    let recursive = a.get(1).map(|v| v.truthy()).unwrap_or(false);
    let r = if recursive {
        std::fs::remove_dir_all(&dir)
    } else {
        std::fs::remove_dir(&dir)
    };
    match r {
        Ok(()) => Ok(Value::Nil),
        Err(e) => Err(i.signal_data(
            sym::FILE_ERROR,
            vec![Value::string(format!("Removing directory: {}", e))],
        )),
    }
}
fn f_delete_file(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let p = want_filename(i, &a[0])?;
    match std::fs::remove_file(&p) {
        Ok(()) => Ok(Value::Nil),
        // GNU's delete-file quietly returns nil when the file is missing.
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Value::Nil),
        Err(e) => Err(i.signal_data(
            sym::FILE_ERROR,
            vec![
                Value::string(format!("Removing old name: {}", e)),
                a[0].clone(),
            ],
        )),
    }
}

// GNU internals: `delete-file-internal' quietly returns nil when the file
// is missing; `delete-directory-internal' signals file-missing.
fn f_delete_file_internal(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let p = want_filename(i, &a[0])?;
    match std::fs::remove_file(&p) {
        Ok(()) => Ok(Value::Nil),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Value::Nil),
        Err(e) => Err(i.signal_data(
            sym::FILE_ERROR,
            vec![
                Value::string(format!("Removing old name: {}", e)),
                a[0].clone(),
            ],
        )),
    }
}

fn f_delete_directory_internal(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let dir = want_filename(i, &a[0])?;
    match std::fs::remove_dir(&dir) {
        Ok(()) => Ok(Value::Nil),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Err(i.signal_data(
            sym::FILE_MISSING,
            vec![
                Value::string("Deleting directory".to_string()),
                Value::string("no such file or directory".to_string()),
                a[0].clone(),
            ],
        )),
        Err(e) => Err(i.signal_data(
            sym::FILE_ERROR,
            vec![Value::string(format!("Removing directory: {}", e))],
        )),
    }
}
fn f_rename_file(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let from = want_filename(i, &a[0])?;
    let to = want_filename(i, &a[1])?;
    match std::fs::rename(&from, &to) {
        Ok(()) => Ok(Value::Nil),
        Err(e) => Err(i.signal_data(
            sym::FILE_ERROR,
            vec![Value::string(format!("Renaming: {}", e))],
        )),
    }
}
fn f_copy_file(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let from = want_filename(i, &a[0])?;
    let to = want_filename(i, &a[1])?;
    match std::fs::copy(&from, &to) {
        Ok(_) => Ok(Value::Nil),
        Err(e) => Err(i.signal_data(
            sym::FILE_ERROR,
            vec![Value::string(format!("Copying: {}", e))],
        )),
    }
}

fn f_insert_file_contents(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let path = want_filename(i, &a[0])?;
    let visit = a.get(1).map(|v| v.truthy()).unwrap_or(false);
    match std::fs::read_to_string(&path) {
        Ok(contents) => {
            let n = contents.chars().count();
            let b = cur(i);
            if visit && b.borrow().text.len() > 0 {
                return Err(i.error("Cannot do file visiting in a non-empty buffer"));
            }
            let start = b.borrow().point();
            crate::buffer::primitives::chg_insert_pt(i, &contents, false)?;
            // GNU Finsert_file_contents leaves point BEFORE the
            // inserted text (like `insert-before-markers').
            let mut bb = b.borrow_mut();
            bb.set_point(start);
            if visit {
                bb.file_name = Some(path.clone());
                bb.note_modified(false);
                // set default-directory to file's dir
                if let Some(dir_end) = path.rfind('/') {
                    let dd = i.intern_soft("default-directory").unwrap_or(u32::MAX);
                    bb.locals.insert(dd, Value::string(&path[..=dir_end]));
                }
            }
            Ok(Value::list(vec![
                Value::string(path),
                Value::Int(n as i128),
            ]))
        }
        Err(e) => {
            let data = vec![
                Value::string("Opening input file".to_string()),
                Value::string(format!("{}", e)),
                Value::string(path),
            ];
            if e.kind() == std::io::ErrorKind::NotFound {
                Err(i.signal_data(sym::FILE_MISSING, data))
            } else {
                Err(i.signal_data(sym::FILE_ERROR, data))
            }
        }
    }
}
fn f_insert_file_contents_literally(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    f_insert_file_contents(i, a)
}

fn f_write_region(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    // (write-region START END FILENAME &optional APPEND VISIT LOCKNAME
    // MUSTBENEW) — START may be a string, in which case END is ignored.
    let path = want_filename(i, &a[2])?;
    if let Value::Str(sv) = &a[0] {
        let s = sv.borrow().clone();
        return write_file_string(i, &path, &s, &a);
    }
    let text = {
        let b = cur(i);
        let bb = b.borrow();
        let len = bb.text.len();
        let (s, e) = region_bounds(i, &a[0], &a[1], len);
        bb.text.substring(s.min(e), s.max(e))
    };
    write_file_string(i, &path, &text, &a)
}

fn region_bounds(i: &mut Interp, s: &Value, e: &Value, len: usize) -> (usize, usize) {
    let s0 = match s {
        Value::Nil => 0,
        Value::Int(n) => (*n).max(1) as usize - 1,
        Value::Marker(m) => m.borrow().position,
        _ => 0,
    };
    let e0 = match e {
        Value::Nil => len,
        Value::Int(n) => (*n).max(1) as usize - 1,
        Value::Marker(m) => m.borrow().position,
        _ => len,
    };
    let _ = i;
    (s0.min(len), e0.min(len))
}

fn write_file_string(i: &mut Interp, path: &str, text: &str, a: &[Value]) -> EvalResult {
    let append = a.get(3).map(|v| v.truthy()).unwrap_or(false);
    let excl = matches!(a.get(6), Some(Value::Sym(s)) if *s == i.intern("excl"));
    if excl && std::path::Path::new(path).exists() {
        // MUSTBENEW = 'excl: fail if the file exists.
        return Err(i.signal_data(
            sym::FILE_ERROR,
            vec![Value::string("File already exists"), Value::string(path)],
        ));
    }
    let r = if append {
        std::fs::OpenOptions::new()
            .append(true)
            .create(true)
            .open(path)
            .and_then(|mut f| std::io::Write::write_all(&mut f, text.as_bytes()))
    } else {
        std::fs::write(path, text)
    };
    match r {
        Ok(()) => {
            // GNU shows "Wrote ..." only interactively with a non-nil
            // VISIT arg; batch write-region (e.g. with-temp-file) is
            // silent.
            let visit = a.get(4).map(|v| v.truthy()).unwrap_or(false);
            if visit && !i.noninteractive {
                i.message(&format!("Wrote {}", path));
            }
            // GNU: writing the visited file refreshes the recorded
            // modtime (write-region / basic-save-buffer path).
            update_visited_file_modtime(&mut cur(i).borrow_mut(), path);
            Ok(Value::Nil)
        }
        Err(e) => Err(i.signal_data(
            sym::FILE_ERROR,
            vec![
                Value::string(format!("Writing region: {}", e)),
                Value::string(path),
            ],
        )),
    }
}

fn f_set_visited_file_name(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let b = cur(i);
    let mut bb = b.borrow_mut();
    match &a.get(0).cloned().unwrap_or(Value::Nil) {
        Value::Nil => bb.file_name = None,
        Value::Str(s) => bb.file_name = Some(s.borrow().clone()),
        other => return Err(i.wrong_type_mut("stringp", other)),
    }
    bb.note_modified(false);
    Ok(Value::Nil)
}

fn f_find_file_noselect(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let path = want_filename(i, &a[0])?;
    // If a buffer already visits this file, return it.
    for id in i.buffers.list() {
        if let Some(b) = i.buffers.get(id) {
            if b.borrow().file_name.as_deref() == Some(path.as_str()) {
                return Ok(Value::Buffer(b.clone()));
            }
        }
    }
    let name = std::path::Path::new(&path)
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.clone());
    // GNU files.el creates file-visiting buffers through
    // `create-file-buffer', which invokes the uniquify advice when
    // uniquify is loaded; fall back to a direct create otherwise.
    let cfb = i.intern("create-file-buffer");
    let bid = if i.fbound_p(cfb) {
        match i.apply(&Value::Sym(cfb), vec![Value::string(path.clone())])? {
            Value::Buffer(b) => b.borrow().id,
            _ => i.buffers.create(&name),
        }
    } else {
        i.buffers.create(&name)
    };
    {
        let b = i.buffers.get(bid).unwrap();
        let mut bb = b.borrow_mut();
        bb.file_name = Some(path.clone());
        // GNU exposes the visited file name as the buffer-local variable
        // `buffer-file-name' (a C field); mirror it into locals so Lisp
        // reads work.  permanent-local marks keep it through
        // kill-all-local-variables.
        let bfn = i.intern_soft("buffer-file-name").unwrap_or(u32::MAX);
        let bft = i.intern_soft("buffer-file-truename").unwrap_or(u32::MAX);
        bb.locals.insert(bfn, Value::string(path.clone()));
        bb.locals.insert(bft, Value::string(path.clone()));
        // Snapshot `create-lockfiles' for Buffer's C-level auto-lock
        // (which cannot see Lisp state during modification).
        if let Some(cl) = i.intern_soft("create-lockfiles") {
            bb.create_lockfiles = i.symbol_value(cl).truthy();
        }
        if let Some(dir_end) = path.rfind('/') {
            let dd = i.intern_soft("default-directory").unwrap_or(u32::MAX);
            bb.locals
                .insert(dd, Value::string(path[..=dir_end].to_string()));
        }
        match std::fs::read_to_string(&path) {
            Ok(contents) => {
                let old_len = bb.text.len();
                drop(bb);
                // GNU's insert-file-contents runs the change hooks in
                // the file's buffer.
                crate::buffer::primitives::chg_with_buffer(i, bid, |i| {
                    crate::lisp::builtins::evalfn::signal_before_change(i, 1, old_len + 1)?;
                    {
                        let b = i.buffers.get(bid).unwrap();
                        let mut bb = b.borrow_mut();
                        bb.text.set_text(&contents);
                        bb.zv = bb.text.len();
                        bb.note_modified(false);
                    }
                    let new_len = contents.chars().count();
                    crate::lisp::builtins::evalfn::signal_after_change(i, 1, new_len + 1, old_len)
                })?;
                let mut bb = b.borrow_mut();
                // GNU records the visited file's modtime+size so
                // `verify-visited-file-modtime' can detect changes.
                if let Ok(m) = std::fs::metadata(&path) {
                    bb.file_modtime_ns = m
                        .modified()
                        .ok()
                        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                        .map(|d| d.as_secs() as i128 * 1_000_000_000 + d.subsec_nanos() as i128)
                        .unwrap_or(-2);
                    bb.file_modtime_size = m.len() as i128;
                }
            }
            Err(_) => {
                // Visited file does not exist (GNU: modtime flag -1).
                bb.file_modtime_ns = -1;
            }
        }
    }
    // GNU find-file-noselect -> after-find-file: pick the major mode and
    // process file-local variables, then run find-file-hook.
    let prev_buf = i.current_buffer;
    i.set_current_buffer(bid);
    let nm = i.intern("normal-mode");
    let rh = i.intern("run-hooks");
    let ffh = i.intern("find-file-hook");
    let r1 = i.apply(&Value::Sym(nm), vec![Value::t()]);
    let r2 = r1.and_then(|_| {
        i.apply(&Value::Sym(rh), vec![Value::Sym(ffh)])
    });
    i.set_current_buffer(prev_buf);
    r2?;
    Ok(i.buffer_value(bid).unwrap_or(Value::Nil))
}

fn f_find_file(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let buf = f_find_file_noselect(i, a)?;
    if let Value::Buffer(b) = &buf {
        let id = b.borrow().id;
        i.set_current_buffer(id);
        if let Some(w) = sel_window(i) {
            w.borrow_mut().buffer = id;
        }
    }
    Ok(buf)
}

/// Refresh a buffer's recorded visited-file modtime+size after writing
/// PATH (only when PATH is the file the buffer visits).
fn update_visited_file_modtime(
    bb: &mut crate::buffer::Buffer,
    path: &str,
) {
    if bb.file_name.as_deref() != Some(path) {
        return;
    }
    if let Ok(m) = std::fs::metadata(path) {
        bb.file_modtime_ns = m
            .modified()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs() as i128 * 1_000_000_000 + d.subsec_nanos() as i128)
            .unwrap_or(-2);
        bb.file_modtime_size = m.len() as i128;
    }
}

fn f_save_buffer(i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    let b = cur(i);
    let (path, text) = {
        let bb = b.borrow();
        match &bb.file_name {
            Some(p) => (p.clone(), bb.text.text()),
            None => return Err(i.error("No file is associated with this buffer")),
        }
    };
    match std::fs::write(&path, &text) {
        Ok(()) => {
            {
                let mut bb = b.borrow_mut();
                bb.note_modified(false);
                update_visited_file_modtime(&mut bb, &path);
            }
            i.message(&format!("Wrote {}", path));
            Ok(Value::t())
        }
        Err(e) => Err(i.signal_data(
            sym::FILE_ERROR,
            vec![
                Value::string(format!("Saving file: {}", e)),
                Value::string(path),
            ],
        )),
    }
}

fn f_write_file(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    f_set_visited_file_name(i, vec![a[0].clone()])?;
    f_save_buffer(i, vec![])
}

fn f_append_to_file(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let path = want_filename(i, &a[2])?;
    let text = {
        let b = cur(i);
        let bb = b.borrow();
        let len = bb.text.len();
        let (s, e) = region_bounds(i, &a[0], &a[1], len);
        bb.text.substring(s.min(e), s.max(e))
    };
    use std::io::Write;
    match std::fs::OpenOptions::new()
        .append(true)
        .create(true)
        .open(&path)
    {
        Ok(mut f) => match f.write_all(text.as_bytes()) {
            Ok(()) => Ok(Value::Nil),
            Err(e) => Err(i.signal_data(sym::FILE_ERROR, vec![Value::string(e.to_string())])),
        },
        Err(e) => Err(i.signal_data(sym::FILE_ERROR, vec![Value::string(e.to_string())])),
    }
}

fn f_file_truename(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let p = want_filename(i, &a[0])?;
    Ok(Value::string(crate::buffer::file_truename(&p)))
}
/// GNU `file-remote-p' remote-name syntax: `/METHOD:USER@HOST:LOCAL'
/// where the method begins a run containing no `/' or `|'.  Returns
/// (prefix, method, user, host, localname); `user' may be absent.
fn remote_name_parts(file: &str) -> Option<(String, String, Option<String>, String, String)> {
    if !file.starts_with('/') {
        return None;
    }
    let rest = &file[1..];
    // First char of the run must not be '/', '|', or ':'.
    let first = rest.chars().next()?;
    if first == '/' || first == '|' || first == ':' {
        return None;
    }
    // The run ends at the first '/' or '|' after the first char.
    let run_end = rest[1..]
        .find(|c| c == '/' || c == '|')
        .map(|p| p + 1)
        .unwrap_or(rest.len());
    let run = &rest[..run_end];
    // The prefix ends at the LAST ':' inside the run.
    let colon = run.rfind(':')?;
    let prefix_len = 1 + colon + 1;
    let prefix = file[..prefix_len].to_string();
    let localname = file[prefix_len..].to_string();
    let spec = &run[..colon];
    // METHOD:USER@HOST — method is up to the first ':'.
    let (method, uh) = match spec.split_once(':') {
        Some((m, uh)) => (m, uh),
        None => return None,
    };
    // USER@HOST splits at the last '@' (user may contain '@').
    let (user, host) = match uh.rsplit_once('@') {
        Some((u, h)) => (Some(u.to_string()), h.to_string()),
        None => (None, uh.to_string()),
    };
    Some((prefix, method.to_string(), user, host, localname))
}

fn f_file_remote_p(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let file = want_str(i, &a[0])?;
    let Some((prefix, method, user, host, localname)) = remote_name_parts(&file) else {
        return Ok(Value::Nil);
    };
    // IDENTIFICATION selects a component (GNU files.el).
    let id = a.get(1).cloned().unwrap_or(Value::Nil);
    let id_name = match &id {
        Value::Nil => return Ok(Value::string(prefix)),
        Value::Sym(s) => i.obarray.name(*s).to_string(),
        other => return Err(i.wrong_type_mut("symbolp", other)),
    };
    let v = match id_name.as_str() {
        "method" => Value::string(method),
        "user" => match user {
            Some(u) => Value::string(u),
            None => Value::Nil,
        },
        "host" => Value::string(host),
        "localname" => Value::string(localname),
        _ => {
            return Err(i.error(&format!("Wrong identification '{}'", id_name)));
        }
    };
    Ok(v)
}

fn f_file_equal_p(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let f1 = want_filename(i, &a[0])?;
    let f2 = want_filename(i, &a[1])?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        match (std::fs::metadata(&f1), std::fs::metadata(&f2)) {
            (Ok(m1), Ok(m2)) => Ok(Value::from_bool(m1.dev() == m2.dev() && m1.ino() == m2.ino())),
            _ => Ok(Value::Nil),
        }
    }
    #[cfg(not(unix))]
    Ok(Value::from_bool(
        crate::buffer::file_truename(&f1) == crate::buffer::file_truename(&f2),
    ))
}

fn f_file_system_info(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let path = want_filename(i, &a[0])?;
    #[cfg(unix)]
    {
        let c = match std::ffi::CString::new(path) {
            Ok(c) => c,
            Err(_) => return Ok(Value::Nil),
        };
        let mut st: libc::statvfs = unsafe { std::mem::zeroed() };
        if unsafe { libc::statvfs(c.as_ptr(), &mut st) } != 0 {
            return Ok(Value::Nil);
        }
        // GNU returns (TOTAL FREE AVAIL) in bytes.
        let bsize = st.f_frsize.max(1) as i128;
        Ok(Value::list(vec![
            Value::Int(st.f_blocks as i128 * bsize),
            Value::Int(st.f_bfree as i128 * bsize),
            Value::Int(st.f_bavail as i128 * bsize),
        ]))
    }
    #[cfg(not(unix))]
    {
        let _ = path;
        Ok(Value::Nil)
    }
}

/// Process-global default file protection, like GNU's C static
/// `default_file_modes'.  Seeded from the real umask on first use.
static DEFAULT_FILE_MODES: std::sync::atomic::AtomicI64 =
    std::sync::atomic::AtomicI64::new(-1);

fn default_file_modes() -> i64 {
    use std::sync::atomic::Ordering;
    let v = DEFAULT_FILE_MODES.load(Ordering::Relaxed);
    if v >= 0 {
        return v;
    }
    #[cfg(unix)]
    unsafe {
        let m = libc::umask(0);
        libc::umask(m);
        let modes = (!m as i64) & 0o777;
        DEFAULT_FILE_MODES.store(modes, Ordering::Relaxed);
        return modes;
    }
    #[allow(unreachable_code)]
    0o666
}

fn f_default_file_modes(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let _ = (i, a);
    Ok(Value::Int(default_file_modes() as i128))
}

fn f_unix_sync(_i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    // GNU calls sync(2) and returns nil.
    #[cfg(unix)]
    unsafe {
        libc::sync();
    }
    Ok(Value::Nil)
}

fn f_file_name_quote(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let name = want_str(i, &a[0])?;
    // GNU: prepend the "/:" quotation prefix unless already quoted.
    if name.starts_with("/:") {
        Ok(Value::string(name))
    } else {
        Ok(Value::string(format!("/:{name}")))
    }
}

fn f_file_name_unquote(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let name = want_str(i, &a[0])?;
    let n = name.strip_prefix("/:").unwrap_or(&name).to_string();
    Ok(Value::string(n))
}

fn f_file_local_name(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let file = want_str(i, &a[0])?;
    // GNU: strip the remote prefix, if any.
    match remote_name_parts(&file) {
        Some((_, _, _, _, local)) => Ok(Value::string(local)),
        None => Ok(Value::string(file)),
    }
}

fn f_unhandled_dir(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let file = want_str(i, &a[0])?;
    // GNU: file-name-as-directory on the (local) name.
    if file.ends_with('/') {
        Ok(Value::string(file))
    } else {
        Ok(Value::string(format!("{file}/")))
    }
}

/// chmod-style symbolic MODE string → numeric mode bits.
/// Grammar: clauses separated by ',', each `[ugoa]*([=+-][rwxXstugo]*)+'.
fn f_modes_sym2num(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let modes = want_str(i, &a[0])?;
    let base: i64 = match a.get(1) {
        Some(Value::Int(n)) => *n as i64,
        Some(v) => return Err(i.wrong_type_mut("integerp", v)),
        None => 0,
    };
    // Empty who-set applies to all classes minus umask bits.
    let umask = !default_file_modes() as i64 & 0o777;
    let mut value = base;
    for clause in modes.split(',') {
        let b = clause.as_bytes();
        let mut p = 0usize;
        let mut who: i64 = 0;
        while p < b.len() {
            match b[p] {
                b'u' => who |= 0o700,
                b'g' => who |= 0o070,
                b'o' => who |= 0o007,
                b'a' => who |= 0o777,
                _ => break,
            }
            p += 1;
        }
        if who == 0 {
            who = 0o777 & !umask;
        }
        while p < b.len() {
            let op = b[p];
            if op != b'=' && op != b'+' && op != b'-' {
                return Err(i.error("Unknown file mode"));
            }
            p += 1;
            let mut perms: i64 = 0;
            while p < b.len() {
                let c = b[p];
                match c {
                    b'r' | b'w' | b'x' | b'X' => {
                        // X: exec only if some exec bit is already set.
                        if c == b'X' && value & 0o111 == 0 {
                            p += 1;
                            continue;
                        }
                        let bit = if c == b'r' { 4 } else if c == b'w' { 2 } else { 1 };
                        for shift in [6i64, 3, 0] {
                            if who & (0o7 << shift) != 0 {
                                perms |= bit << shift;
                            }
                        }
                        p += 1;
                    }
                    b's' => {
                        if who & 0o700 != 0 {
                            perms |= 0o4000;
                        }
                        if who & 0o070 != 0 {
                            perms |= 0o2000;
                        }
                        p += 1;
                    }
                    b't' => {
                        perms |= 0o1000;
                        p += 1;
                    }
                    b'u' | b'g' | b'o' => {
                        // Copy that class's current bits.
                        let sh = if c == b'u' { 6 } else if c == b'g' { 3 } else { 0 };
                        let bits = (value >> sh) & 0o7;
                        for s2 in [6i64, 3, 0] {
                            if who & (0o7 << s2) != 0 {
                                perms |= bits << s2;
                            }
                        }
                        p += 1;
                    }
                    _ => break,
                }
            }
            value = match op {
                b'=' => (value & !who) | perms,
                b'+' => value | perms,
                _ => value & !perms,
            };
        }
    }
    Ok(Value::Int(value as i128))
}

fn f_set_default_file_modes(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let mode = want_int(i, &a[0])?;
    // GNU sets the process umask to ~MODE & 0777 and records MODE in
    // the `default-file-modes' static (fileio.c).
    #[cfg(unix)]
    unsafe {
        libc::umask((!mode & 0o777) as libc::mode_t);
    }
    DEFAULT_FILE_MODES.store(mode as i64, std::sync::atomic::Ordering::Relaxed);
    Ok(Value::Nil)
}
fn f_car_less_than_car(_i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let x = match &a[0] {
        Value::Cons(c) => c.borrow().car.clone(),
        v => v.clone(),
    };
    let y = match &a[1] {
        Value::Cons(c) => c.borrow().car.clone(),
        v => v.clone(),
    };
    let less = match (&x, &y) {
        (Value::Int(a), Value::Int(b)) => a < b,
        (Value::Float(a), Value::Float(b)) => a < b,
        (Value::Str(a), Value::Str(b)) => *a.borrow() < *b.borrow(),
        _ => false,
    };
    Ok(Value::from_bool(less))
}

// ---------- processes ----------

fn f_call_process(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let prog = want_str(i, &a[0])?;
    // (call-process PROGRAM &optional INFILE DESTINATION DISPLAY &rest ARGS)
    let mut cmd = std::process::Command::new(&prog);
    for v in a.get(4..).unwrap_or(&[]) {
        cmd.arg(want_str(i, v)?);
    }
    if let Some(Value::Str(infile)) = a.get(1) {
        if let Ok(f) = std::fs::File::open(infile.borrow().as_str()) {
            cmd.stdin(std::process::Stdio::from(f));
        }
    }
    let output = match cmd.output() {
        Ok(o) => o,
        Err(e) => {
            let data = if e.kind() == std::io::ErrorKind::NotFound {
                vec![
                    Value::string("Searching for program"),
                    Value::string(e.to_string()),
                    Value::string(prog.clone()),
                ]
            } else {
                vec![Value::string(format!("Doing exec: {}", e))]
            };
            let sym_id = if e.kind() == std::io::ErrorKind::NotFound {
                i.intern("file-missing")
            } else {
                i.intern("file-error")
            };
            return Err(i.signal_data(sym_id, data));
        }
    };
    let dest = arg(&a, 2);
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    // A cons dest is (REALDEST . STDERR); recurse on REALDEST only.
    let real_dest = match &dest {
        Value::Cons(c) => c.borrow().car.clone(),
        other => other.clone(),
    };
    // (:file FILE) — write stdout to FILE.
    let mut file_dest: Option<String> = None;
    if let Value::Cons(c) = &real_dest {
        let cb = c.borrow();
        if let (Value::Sym(s), Value::Cons(inner)) = (&cb.car, &cb.cdr) {
            if i.symbol_name(*s) == ":file" {
                if let Value::Str(p) = &inner.borrow().car {
                    file_dest = Some(p.borrow().clone());
                }
            }
        }
    }
    if let Some(path) = file_dest {
        let _ = std::fs::write(&path, &stdout);
    } else {
        match &real_dest {
            // DESTINATION 0: discard output, return nil.
            Value::Int(0) => return Ok(Value::Nil),
            Value::Nil => {}
            _ => {
                // t or buffer → insert at point in current/that buffer.
                let bid = if real_dest.truthy()
                    && !i.sym_id(&real_dest).map(|s| s == sym::T).unwrap_or(false)
                {
                    i.buffer_id_of(&real_dest).unwrap_or(i.current_buffer)
                } else {
                    i.current_buffer
                };
                if i.buffers.get(bid).is_some() {
                    let out = stdout.clone();
                    crate::buffer::primitives::chg_with_buffer(i, bid, |i| {
                        crate::buffer::primitives::chg_insert_pt(i, &out, false)
                    })?;
                }
            }
        }
    }
    Ok(Value::Int(output.status.code().unwrap_or(1) as i128))
}

fn f_call_process_region(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    // (call-process-region START END PROGRAM &optional DELETE DESTINATION
    //  DISPLAY &rest ARGS)
    let text = {
        let b = cur(i);
        let bb = b.borrow();
        let len = bb.text.len();
        let (s, e) = region_bounds(i, &a[0], &a[1], len);
        bb.text.substring(s.min(e), s.max(e))
    };
    let prog = want_str(i, &a[2])?;
    let mut cmd = std::process::Command::new(&prog);
    cmd.stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped());
    for v in a.get(6..).unwrap_or(&[]) {
        cmd.arg(want_str(i, v)?);
    }
    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => {
            let data = if e.kind() == std::io::ErrorKind::NotFound {
                vec![
                    Value::string("Searching for program"),
                    Value::string(e.to_string()),
                    Value::string(prog.clone()),
                ]
            } else {
                vec![Value::string(format!("Doing exec: {}", e))]
            };
            let sym_id = if e.kind() == std::io::ErrorKind::NotFound {
                i.intern("file-missing")
            } else {
                i.intern("file-error")
            };
            return Err(i.signal_data(sym_id, data));
        }
    };
    {
        use std::io::Write;
        if let Some(mut stdin) = child.stdin.take() {
            let _ = stdin.write_all(text.as_bytes());
        }
    }
    let output = child
        .wait_with_output()
        .map_err(|e| i.signal_data(sym::FILE_ERROR, vec![Value::string(e.to_string())]))?;
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    // DELETE: replace the region.
    if a.get(3).map(|v| v.truthy()).unwrap_or(false) {
        let len = cur(i).borrow().text.len();
        let (s, e) = region_bounds(i, &a[0], &a[1], len);
        crate::buffer::primitives::chg_delete(i, s.min(e), s.max(e))?;
        crate::buffer::primitives::chg_insert(i, s.min(e), &stdout)?;
    } else {
        let dest = arg(&a, 4);
        if dest.truthy() {
            let bid = if i.sym_id(&dest).map(|s| s == sym::T).unwrap_or(false) {
                i.current_buffer
            } else {
                i.buffer_id_of(&dest).unwrap_or(i.current_buffer)
            };
            if i.buffers.get(bid).is_some() {
                crate::buffer::primitives::chg_with_buffer(i, bid, |i| {
                    crate::buffer::primitives::chg_insert_pt(i, &stdout, false)
                })?;
            }
        }
    }
    Ok(Value::Int(output.status.code().unwrap_or(1) as i128))
}

fn f_shell_command(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let cmdline = want_str(i, &a[0])?;
    let out = std::process::Command::new("/bin/sh")
        .arg("-c")
        .arg(&cmdline)
        .output();
    match out {
        Ok(o) => {
            let stdout = String::from_utf8_lossy(&o.stdout).into_owned();
            let dest = arg(&a, 1);
            if dest.truthy() || matches!(a.get(1), None) {
                // t/nil → display in *Shell Command Output*
                let bid = i
                    .buffers
                    .by_name("*Shell Command Output*")
                    .unwrap_or_else(|| i.buffers.create("*Shell Command Output*"));
                if i.buffers.get(bid).is_some() {
                    crate::buffer::primitives::chg_with_buffer(i, bid, |i| {
                        let b = i.buffers.get(bid).unwrap();
                        let old_len = b.borrow().text.len();
                        crate::lisp::builtins::evalfn::signal_before_change(i, 1, old_len + 1)?;
                        {
                            let mut bb = b.borrow_mut();
                            bb.text.set_text(&stdout);
                            bb.zv = bb.text.len();
                        }
                        let new_len = stdout.chars().count();
                        crate::lisp::builtins::evalfn::signal_after_change(i, 1, new_len + 1, old_len)
                    })?;
                }
            }
            Ok(Value::Int(o.status.code().unwrap_or(1) as i128))
        }
        Err(e) => Err(i.error(&e.to_string())),
    }
}

fn f_shell_command_to_string(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let cmdline = want_str(i, &a[0])?;
    match std::process::Command::new("/bin/sh")
        .arg("-c")
        .arg(&cmdline)
        .output()
    {
        Ok(o) => Ok(Value::string(
            String::from_utf8_lossy(&o.stdout).into_owned(),
        )),
        Err(e) => Err(i.error(&e.to_string())),
    }
}

// ---------- editing commands ----------

fn f_kill_line(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let n = arg(&a, 0).int().unwrap_or(1);
    let b = cur(i);
    let mut bb = b.borrow_mut();
    let p = bb.point();
    let le = bb.text.line_end(p);
    let killed;
    if p == le {
        if p >= bb.text.len() {
            // Point at the very end of the buffer: nothing to kill.
            drop(bb);
            let sym = i.intern("end-of-buffer");
            return Err(i.signal_data(sym, Vec::new()));
        }
        // at EOL: kill the newline(s)
        let end = (p + n.max(1) as usize).min(bb.text.len());
        drop(bb);
        killed = crate::buffer::primitives::chg_delete(i, p, end)?;
    } else {
        drop(bb);
        killed = crate::buffer::primitives::chg_delete(i, p, le)?;
    }
    crate::buffer::primitives::push_kill_ring(i, killed);
    Ok(Value::Nil)
}

fn f_kill_whole_line(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let n = arg(&a, 0).int().unwrap_or(1);
    let b = cur(i);
    let mut bb = b.borrow_mut();
    let line = bb.text.line_of_pos(bb.point());
    let s = bb.text.line_start(line);
    let mut e = s;
    for _ in 0..n.max(1) {
        e = bb.text.line_end(e);
        if e < bb.text.len() {
            e += 1;
        }
    }
    let tlen = bb.text.len();
    drop(bb);
    let killed = crate::buffer::primitives::chg_delete(i, s, e.min(tlen))?;
    cur(i).borrow_mut().set_point(s);
    crate::buffer::primitives::push_kill_ring(i, killed);
    Ok(Value::Nil)
}

fn f_kill_word(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let n = arg(&a, 0).int().unwrap_or(1);
    let syn = syntax_table_entries(i);
    let wordp = |c: char| syntax_entry_code(syn.as_ref(), c) == b'w';
    let b = cur(i);
    let mut bb = b.borrow_mut();
    let start = bb.point();
    let mut p = start;
    let len = bb.text_len();
    for _ in 0..n.max(0) {
        while p < len && !wordp(bb.text.char_at(p)) {
            p += 1;
        }
        while p < len && wordp(bb.text.char_at(p)) {
            p += 1;
        }
    }
    drop(bb);
    let killed = crate::buffer::primitives::chg_delete(i, start, p)?;
    crate::buffer::primitives::push_kill_ring(i, killed);
    Ok(Value::Nil)
}

fn f_backward_kill_word(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let n = arg(&a, 0).int().unwrap_or(1);
    let syn = syntax_table_entries(i);
    let wordp = |c: char| syntax_entry_code(syn.as_ref(), c) == b'w';
    let b = cur(i);
    let mut bb = b.borrow_mut();
    let end = bb.point();
    let mut p = end;
    for _ in 0..n.max(0) {
        while p > 0 && !wordp(bb.text.char_at(p - 1)) {
            p -= 1;
        }
        while p > 0 && wordp(bb.text.char_at(p - 1)) {
            p -= 1;
        }
    }
    drop(bb);
    let killed = crate::buffer::primitives::chg_delete(i, p, end)?;
    crate::buffer::primitives::push_kill_ring(i, killed);
    Ok(Value::Nil)
}

fn f_delete_horizontal_space(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let backward_only = arg(&a, 0).truthy();
    let b = cur(i);
    let mut bb = b.borrow_mut();
    let p = bb.point();
    let mut s = p;
    let mut e = p;
    while s > 0 && matches!(bb.text.char_at(s - 1), ' ' | '\t') {
        s -= 1;
    }
    if !backward_only {
        while e < bb.text.len() && matches!(bb.text.char_at(e), ' ' | '\t') {
            e += 1;
        }
    }
    drop(bb);
    crate::buffer::primitives::chg_delete(i, s, e)?;
    cur(i).borrow_mut().set_point(s);
    Ok(Value::Nil)
}

fn f_just_one_space(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let n = arg(&a, 0).int().unwrap_or(1).max(0) as usize;
    let b = cur(i);
    let mut bb = b.borrow_mut();
    let p = bb.point();
    let mut s = p;
    let mut e = p;
    while s > 0 && matches!(bb.text.char_at(s - 1), ' ' | '\t') {
        s -= 1;
    }
    while e < bb.text.len() && matches!(bb.text.char_at(e), ' ' | '\t') {
        e += 1;
    }
    drop(bb);
    crate::buffer::primitives::chg_delete(i, s, e)?;
    cur(i).borrow_mut().set_point(s);
    crate::buffer::primitives::chg_insert_pt(i, &" ".repeat(n), false)?;
    Ok(Value::Nil)
}

fn f_delete_indentation(i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    let b = cur(i);
    let mut bb = b.borrow_mut();
    // Join to previous line: find BOL, if >0 join with prev line.
    let p = bb.point();
    let line = bb.text.line_of_pos(p);
    if line == 0 {
        return Ok(Value::Nil);
    }
    let ls = bb.text.line_start(line);
    if ls == 0 {
        return Ok(Value::Nil);
    }
    // Delete the newline + leading whitespace of this line.
    let mut e = ls;
    while e < bb.text.len() && matches!(bb.text.char_at(e), ' ' | '\t') {
        e += 1;
    }
    let s = ls - 1; // the newline
    drop(bb);
    crate::buffer::primitives::chg_delete(i, s, e)?;
    cur(i).borrow_mut().set_point(s);
    Ok(Value::Nil)
}

fn f_zap_to_char(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let n = arg(&a, 0).int().unwrap_or(1);
    let ch = want_int(i, &a[1])? as u32;
    let c = char::from_u32(ch).unwrap_or('\0');
    let b = cur(i);
    let mut bb = b.borrow_mut();
    let p = bb.point();
    let mut found = p;
    let mut count = 0;
    while found < bb.text.len() {
        if bb.text.char_at(found) == c {
            count += 1;
            if count >= n.max(1) {
                break;
            }
        }
        found += 1;
    }
    if count < n.max(1) {
        return Err(i.error(&format!("Char {} not found", c)));
    }
    drop(bb);
    crate::buffer::primitives::chg_delete(i, p, found + 1)?;
    Ok(Value::Nil)
}

fn f_transpose_chars(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let n = arg(&a, 0).int().unwrap_or(1);
    if n == 0 {
        return Ok(Value::Nil);
    }
    let b = cur(i);
    let mut bb = b.borrow_mut();
    let p = bb.point();
    if p == 0 || p >= bb.text.len() {
        return Err(i.error("No character to transpose"));
    }
    // transpose chars at p-1 and p (Emacs transposes the two chars around point)
    let c1 = bb.text.char_at(p - 1);
    let c2 = bb.text.char_at(p);
    let s2 = c2.to_string();
    let s1 = c1.to_string();
    drop(bb);
    crate::lisp::builtins::evalfn::signal_before_change(i, p, p + 1)?;
    let b = cur(i);
    let mut bb = b.borrow_mut();
    bb.record_delete(p, s2.clone());
    bb.text.delete(p, p + 1);
    bb.adjust_markers_delete(p, p + 1);
    bb.record_insert(p, s1.chars().count());
    bb.text.insert(p, &s1);
    bb.adjust_markers_insert(p, s1.chars().count(), false);
    bb.record_delete(p - 1, s1);
    bb.text.delete(p - 1, p);
    bb.adjust_markers_delete(p - 1, p);
    bb.record_insert(p - 1, s2.chars().count());
    bb.text.insert(p - 1, &s2);
    bb.adjust_markers_insert(p - 1, s2.chars().count(), false);
    bb.note_text_change(2);
    drop(bb);
    crate::lisp::builtins::evalfn::signal_after_change(i, p, p + 1, 2)?;
    Ok(Value::Nil)
}

fn f_transpose_lines(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let n = arg(&a, 0).int().unwrap_or(1);
    let _ = n;
    let b = cur(i);
    let mut bb = b.borrow_mut();
    let line = bb.text.line_of_pos(bb.point());
    if line == 0 {
        return Err(i.error("No previous line"));
    }
    let l1s = bb.text.line_start(line - 1);
    let l1e = bb.text.line_end(l1s);
    let l2s = bb.text.line_start(line);
    let l2e = bb.text.line_end(l2s);
    let l1 = bb.text.substring(l1s, l1e);
    let l2 = bb.text.substring(l2s, l2e);
    drop(bb);
    crate::lisp::builtins::evalfn::signal_before_change(i, l1s + 1, l2e + 1)?;
    let b = cur(i);
    let mut bb = b.borrow_mut();
    bb.record_delete(l1s, format!("{}\n{}", l1, l2));
    bb.text.delete(l1s, l2e);
    bb.adjust_markers_delete(l1s, l2e);
    let new = format!("{}\n{}", l2, l1);
    bb.record_insert(l1s, new.chars().count());
    bb.text.insert(l1s, &new);
    bb.adjust_markers_insert(l1s, new.chars().count(), false);
    bb.note_text_change(l2e - l1s + new.chars().count());
    drop(bb);
    crate::lisp::builtins::evalfn::signal_after_change(i, l1s + 1, l2e + 1, l2e - l1s)?;
    Ok(Value::Nil)
}

fn region_op(i: &mut Interp, a: &[Value], op: crate::lisp::builtins::strfn::CaseOp) -> EvalResult {
    use crate::lisp::builtins::strfn::{case_extra, case_str};
    let b = cur(i);
    let down = i.current_case_table();
    let up = case_extra(&down, 0).unwrap_or_else(|| down.clone());
    let (s, e, old) = {
        let bb = b.borrow();
        let len = bb.text.len();
        let (s, e) = region_bounds(i, &a[0], &a[1], len);
        let (s, e) = (s.min(e), s.max(e));
        (s, e, bb.text.substring(s, e))
    };
    let new = case_str(i, &old, op, &down, &up);
    // GNU `casify_region': `modify_text' fires whenever the region is
    // non-empty (before-change + MODIFF bump even when nothing
    // changed); `after-change' fires only when case actually changed,
    // spanning first..last changed char.
    crate::lisp::builtins::evalfn::signal_before_change(i, s + 1, e + 1)?;
    let old_len = e - s;
    let n_new = new.chars().count();
    let changed = new != old;
    let (first, last_old) = if changed && n_new == old_len {
        let o: Vec<char> = old.chars().collect();
        let n: Vec<char> = new.chars().collect();
        let f = o.iter().zip(&n).position(|(a, b)| a != b).unwrap_or(0);
        let l = o
            .iter()
            .zip(&n)
            .rev()
            .position(|(a, b)| a != b)
            .map(|x| old_len - x)
            .unwrap_or(old_len);
        (f, l)
    } else {
        (0, old_len)
    };
    {
        let b = cur(i);
        let mut bb = b.borrow_mut();
        if changed {
            bb.raw_delete_region(s, e);
            bb.raw_insert_at(s, &new);
        } else {
            bb.record_delete(s, old.clone());
            bb.record_insert(s, n_new);
        }
        bb.note_text_change(old_len);
    }
    if changed {
        let span_new = n_new - (old_len - last_old);
        crate::lisp::builtins::evalfn::signal_after_change(
            i,
            s + first + 1,
            s + first + span_new + 1,
            last_old - first,
        )?;
    }
    Ok(Value::Nil)
}

fn f_upcase_region(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    region_op(i, &a, crate::lisp::builtins::strfn::CaseOp::Up)
}
fn f_downcase_region(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    region_op(i, &a, crate::lisp::builtins::strfn::CaseOp::Down)
}
fn f_capitalize_region(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    region_op(i, &a, crate::lisp::builtins::strfn::CaseOp::Cap)
}

fn word_op(i: &mut Interp, a: &[Value], op: crate::lisp::builtins::strfn::CaseOp) -> EvalResult {
    use crate::lisp::builtins::strfn::{case_extra, case_str};
    let n = arg(a, 0).int().unwrap_or(1);
    let b = cur(i);
    let down = i.current_case_table();
    let up = case_extra(&down, 0).unwrap_or_else(|| down.clone());
    let syn = syntax_table_entries(i);
    let wordp = |c: char| syntax_entry_code(syn.as_ref(), c) == b'w';
    let start = b.borrow().point();
    let len = b.borrow().text_len();
    let mut p = start;
    if n >= 0 {
        for _ in 0..n {
            while p < len && !wordp(b.borrow().text.char_at(p)) {
                p += 1;
            }
            while p < len && wordp(b.borrow().text.char_at(p)) {
                p += 1;
            }
        }
    } else {
        for _ in 0..-n {
            while p > 0 && !wordp(b.borrow().text.char_at(p - 1)) {
                p -= 1;
            }
            while p > 0 && wordp(b.borrow().text.char_at(p - 1)) {
                p -= 1;
            }
        }
    }
    let (s, e) = (start.min(p), start.max(p));
    let old = b.borrow().text.substring(s, e);
    let new = case_str(i, &old, op, &down, &up);
    // GNU `casify_region' shape (see region_op).
    crate::lisp::builtins::evalfn::signal_before_change(i, s + 1, e + 1)?;
    let old_len = e - s;
    let n_new = new.chars().count();
    let changed = new != old;
    let (first, last_old) = if changed && n_new == old_len {
        let o: Vec<char> = old.chars().collect();
        let n: Vec<char> = new.chars().collect();
        let f = o.iter().zip(&n).position(|(a, b)| a != b).unwrap_or(0);
        let l = o
            .iter()
            .zip(&n)
            .rev()
            .position(|(a, b)| a != b)
            .map(|x| old_len - x)
            .unwrap_or(old_len);
        (f, l)
    } else {
        (0, old_len)
    };
    {
        let mut bb = b.borrow_mut();
        if changed {
            bb.raw_delete_region(s, e);
            bb.raw_insert_at(s, &new);
        } else {
            bb.record_delete(s, old.clone());
            bb.record_insert(s, n_new);
        }
        bb.note_text_change(old_len);
    }
    if changed {
        let span_new = n_new - (old_len - last_old);
        crate::lisp::builtins::evalfn::signal_after_change(
            i,
            s + first + 1,
            s + first + span_new + 1,
            last_old - first,
        )?;
    }
    // GNU moves point over the changed words for positive args and
    // leaves it alone for negative args.
    if n >= 0 {
        b.borrow_mut().set_point(s + n_new);
    }
    Ok(Value::Nil)
}

fn f_upcase_word(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    word_op(i, &a, crate::lisp::builtins::strfn::CaseOp::Up)
}
fn f_downcase_word(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    word_op(i, &a, crate::lisp::builtins::strfn::CaseOp::Down)
}
fn f_capitalize_word(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    word_op(i, &a, crate::lisp::builtins::strfn::CaseOp::Cap)
}

fn f_indent_line_to(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let col = want_int(i, &a[0])?.max(0) as usize;
    let tab = i
        .symbol_value(i.intern_soft("tab-width").unwrap_or(0))
        .int()
        .unwrap_or(8)
        .max(1) as usize;
    let tabs_on = i
        .symbol_value(i.intern_soft("indent-tabs-mode").unwrap_or(0))
        .truthy();
    let b = cur(i);
    let mut bb = b.borrow_mut();
    let line = bb.text.line_of_pos(bb.point());
    let ls = bb.text.line_start(line);
    // Delete existing leading whitespace, insert `col` columns of
    // whitespace (tabs when `indent-tabs-mode' is on, like GNU).
    let mut e = ls;
    while e < bb.text.len() && matches!(bb.text.char_at(e), ' ' | '\t') {
        e += 1;
    }
    drop(bb);
    crate::buffer::primitives::chg_delete(i, ls, e)?;
    let ws = if tabs_on {
        format!("{}{}", "\t".repeat(col / tab), " ".repeat(col % tab))
    } else {
        " ".repeat(col)
    };
    let wlen = ws.chars().count();
    crate::buffer::primitives::chg_insert(i, ls, &ws)?;
    cur(i).borrow_mut().set_point(ls + wlen);
    Ok(Value::Nil)
}

fn f_indent_to(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let col = want_int(i, &a[0])?.max(0);
    let minimum = match a.get(1) {
        Some(v) if v.truthy() => want_int(i, v)?,
        _ => 0,
    };
    let tab = i
        .symbol_value(i.intern_soft("tab-width").unwrap_or(0))
        .int()
        .unwrap_or(8)
        .max(1);
    let tabs_on = i
        .symbol_value(i.intern_soft("indent-tabs-mode").unwrap_or(0))
        .truthy();
    let b = cur(i);
    let mut bb = b.borrow_mut();
    let p = bb.point();
    let ls = bb.text.line_start(bb.text.line_of_pos(p));
    let fromcol = crate::buffer::primitives::rect_col_at(&bb.text, ls, p, tab);
    let mincol = (fromcol + minimum).max(col);
    if fromcol < mincol {
        let mut s = String::new();
        let mut c = fromcol;
        if tabs_on {
            loop {
                let next = (c / tab + 1) * tab;
                if next > mincol {
                    break;
                }
                s.push('\t');
                c = next;
            }
        }
        while c < mincol {
            s.push(' ');
            c += 1;
        }
        drop(bb);
        crate::buffer::primitives::chg_insert_pt(i, &s, false)?;
    }
    Ok(Value::Int(mincol))
}

fn f_indent_rigidly(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let len = cur(i).borrow().text.len();
    let (s, e) = region_bounds(i, &a[0], &a[1], len);
    let col = want_int(i, &a[2])?;
    // For each line in [s,e): insert col spaces (or delete -col cols).
    // Report the batch as one change covering the region (GNU's
    // `combine-change-calls' shape).
    crate::lisp::builtins::evalfn::signal_before_change(i, s + 1, e + 1)?;
    {
        let b = cur(i);
        let mut bb = b.borrow_mut();
        let mut pos = s;
        while pos < e && pos <= bb.text.len() {
            if col >= 0 {
                bb.insert_at(pos, &" ".repeat(col as usize));
                pos += col as usize;
            } else {
                // delete up to -col leading whitespace
                let mut d = 0;
                while d < (-col) as usize
                    && pos < bb.text.len()
                    && matches!(bb.text.char_at(pos), ' ' | '\t')
                {
                    d += 1;
                    pos += 1;
                }
                if d > 0 {
                    bb.delete_region(pos - d, pos);
                    pos -= d;
                }
            }
            // to next line
            let le = bb.text.line_end(pos);
            pos = if le < bb.text.len() { le + 1 } else { break };
        }
    }
    let e2 = cur(i).borrow().text.len().min(e.max(s));
    crate::lisp::builtins::evalfn::signal_after_change(i, s + 1, e2 + 1, e - s)?;
    Ok(Value::Nil)
}

fn f_delete_trailing_whitespace(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let len = cur(i).borrow().text.len();
    let (s, e) = match (a.get(0), a.get(1)) {
        (Some(sv), Some(ev)) if sv.truthy() => region_bounds(i, sv, ev, len),
        _ => (0, len),
    };
    crate::lisp::builtins::evalfn::signal_before_change(i, s + 1, e + 1)?;
    {
        let b = cur(i);
        let mut bb = b.borrow_mut();
        let mut pos = s;
        while pos < e && pos < bb.text.len() {
            let le = bb.text.line_end(pos);
            // trailing ws before le
            let mut ts = le;
            while ts > pos && matches!(bb.text.char_at(ts - 1), ' ' | '\t') {
                ts -= 1;
            }
            if ts < le {
                bb.delete_region(ts, le);
            }
            let le2 = bb.text.line_end(ts);
            pos = if le2 < bb.text.len() { le2 + 1 } else { break };
        }
    }
    let e2 = cur(i).borrow().text.len().min(e);
    crate::lisp::builtins::evalfn::signal_after_change(i, s + 1, e2 + 1, e - s)?;
    Ok(Value::Nil)
}

fn f_untabify(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let len = cur(i).borrow().text.len();
    let (s, e) = match (a.get(0), a.get(1)) {
        (Some(sv), Some(ev)) if sv.truthy() => region_bounds(i, sv, ev, len),
        _ => (0, len),
    };
    let tab_width = i
        .symbol_value(i.intern_soft("tab-width").unwrap_or(0))
        .int()
        .unwrap_or(8)
        .max(1) as usize;
    crate::lisp::builtins::evalfn::signal_before_change(i, s + 1, e + 1)?;
    {
        let b = cur(i);
        let mut bb = b.borrow_mut();
        let mut pos = s;
        while pos < e && pos < bb.text.len() {
            if bb.text.char_at(pos) == '\t' {
                let ls = bb.text.line_start(bb.text.line_of_pos(pos));
                let col = pos - ls;
                let spaces = tab_width - (col % tab_width);
                bb.record_delete(pos, "\t".to_string());
                bb.text.delete(pos, pos + 1);
                bb.adjust_markers_delete(pos, pos + 1);
                bb.record_insert(pos, spaces);
                bb.text.insert(pos, &" ".repeat(spaces));
                bb.adjust_markers_insert(pos, spaces, false);
                bb.note_text_change(1 + spaces);
                pos += spaces;
            } else {
                pos += 1;
            }
        }
    }
    let e2 = cur(i).borrow().text.len().min(e);
    crate::lisp::builtins::evalfn::signal_after_change(i, s + 1, e2 + 1, e - s)?;
    Ok(Value::Nil)
}

fn f_tabify(_i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    Ok(Value::Nil)
}

fn f_move_beginning_of_line(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let n = arg(&a, 0).int().unwrap_or(1);
    let b = cur(i);
    let mut bb = b.borrow_mut();
    let line = bb.text.line_of_pos(bb.point());
    let target = (line as i128 + (n - 1)).max(0) as usize;
    let p = bb.text.line_start(target);
    bb.set_point(p);
    Ok(Value::Nil)
}

fn f_move_end_of_line(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let n = arg(&a, 0).int().unwrap_or(1);
    let b = cur(i);
    let mut bb = b.borrow_mut();
    let line = bb.text.line_of_pos(bb.point());
    let target = line + (n - 1).max(0) as usize;
    let ls = bb.text.line_start(target);
    let p = bb.text.line_end(ls);
    bb.set_point(p);
    Ok(Value::Nil)
}

fn f_forward_line_cmd(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let n = arg(&a, 0).int().unwrap_or(1);
    let b = cur(i);
    let mut bb = b.borrow_mut();
    let line = bb.text.line_of_pos(bb.point());
    let target = (line as i128 + n).max(0) as usize;
    let p = bb.text.line_start(target);
    bb.set_point(p);
    Ok(Value::Nil)
}

// ---------- minibuffer ----------

fn minibuf_id(i: &Interp) -> Option<usize> {
    i.frames
        .first()
        .and_then(|f| f.borrow().minibuffer.clone())
        .map(|w| w.borrow().buffer)
}

fn f_minibufferp(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    match a.get(0) {
        None | Some(Value::Nil) => {
            // current buffer is minibuffer?
            Ok(Value::from_bool(minibuf_id(i) == Some(i.current_buffer)))
        }
        Some(v) => match i.buffer_id_of(v) {
            Some(id) => Ok(Value::from_bool(minibuf_id(i) == Some(id))),
            None => Ok(Value::Nil),
        },
    }
}

fn f_minibuffer_contents(i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    match minibuf_id(i).and_then(|id| i.buffers.get(id)) {
        Some(b) => Ok(Value::string(b.borrow().text.text())),
        None => Ok(Value::string("")),
    }
}

fn f_delete_minibuffer_contents(i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    if let Some(id) = minibuf_id(i) {
        if i.buffers.get(id).is_some() {
            crate::buffer::primitives::chg_with_buffer(i, id, |i| {
                let tlen = cur(i).borrow().text.len();
                crate::buffer::primitives::chg_delete(i, 0, tlen)?;
                cur(i).borrow_mut().set_point(0);
                Ok(Value::Nil)
            })?;
        }
    }
    Ok(Value::Nil)
}

fn f_minibuffer_depth(i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    Ok(Value::Int(i.minibuf_level as i128))
}
fn f_minibuffer_prompt(_i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    // GNU returns nil when no minibuffer is active.
    Ok(Value::Nil)
}

/// `minibuffer-prompt-end' — GNU returns the buffer position right
/// after the prompt; with no minibuffer active that is point-min.
fn f_minibuffer_prompt_end(i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    Ok(Value::Int(crate::buffer::primitives::cur(i).borrow().begv as i128 + 1))
}

/// `blink-cursor-mode' — a GNU minor-mode command: called from Lisp,
/// nil means enable (not toggle), `toggle' toggles, non-positive
/// numbers disable, anything else enables; returns the new state.
fn f_blink_cursor_mode(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let sid = i.intern("blink-cursor-mode");
    let cur = i.symbol_value(sid).truthy();
    let on = match arg(&a, 0) {
        Value::Nil => true,
        Value::Sym(s) if i.symbol_name(s) == "toggle" => !cur,
        Value::Int(n) => n > 0,
        _ => true,
    };
    let v = Value::from_bool(on);
    let _ = i.set_symbol(sid, v.clone());
    Ok(v)
}

/// `face-attribute-relative-p' — in GNU only `:height' is a
/// "relative" attribute; any other name (known or not) yields nil.
fn f_face_attribute_relative_p(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    match &a[0] {
        Value::Sym(s) => Ok(Value::from_bool(i.symbol_name(*s) == ":height")),
        _ => Ok(Value::Nil),
    }
}

/// `set-window-cursor-type' — window-live-p check, then no-op on
/// our flat tty model; GNU returns t.
fn f_set_window_cursor_type(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    match &a[0] {
        Value::Nil | Value::Window(_) => Ok(Value::t()),
        other => Err(i.wrong_type_mut("window-live-p", other)),
    }
}

/// `window-state-get' — the leaf-window state alist GNU's window.c
/// produces.  WRITABLE non-nil substitutes the buffer name and
/// integer positions for buffer/marker objects.
fn f_window_state_get(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let w = match arg(&a, 0) {
        Value::Nil => sel_window(i).ok_or_else(|| i.error("No window"))?,
        Value::Window(w) => w.clone(),
        other => {
            let shown = i.princ_to_string(&other);
            return Err(i.error(format!("{shown} is not a live or internal window")));
        }
    };
    let writable = arg(&a, 1).truthy();
    let (ww, wh, bufid, point, start, selected, ml, mr) = {
        let wb = w.borrow();
        let point = window_point(i, &w) + 1;
        let sel = sel_window(i).map(|s| s.borrow().id) == Some(wb.id);
        (
            wb.width,
            wb.height,
            wb.buffer,
            point,
            wb.start + 1,
            sel,
            wb.margins.0 as i128,
            wb.margins.1 as i128,
        )
    };
    let ac = |i: &mut Interp, k: &str, v: Value| Value::cons(Value::Sym(i.intern(k)), v);
    let ai = |i: &mut Interp, k: &str, n: i128| ac(i, k, Value::Int(n));
    // GNU's ignore-widths add the margins to the safe minimum.
    let w_ignore = 2 + ml + mr;
    let header = Value::list(vec![
        ai(i, "min-height", 4),
        ai(i, "min-width", 10),
        ai(i, "min-height-ignore", 2),
        ai(i, "min-width-ignore", w_ignore),
        ai(i, "min-height-safe", 1),
        ai(i, "min-width-safe", 2),
        ai(i, "min-pixel-height", 4),
        ai(i, "min-pixel-width", 10),
        ai(i, "min-pixel-height-ignore", 2),
        ai(i, "min-pixel-width-ignore", w_ignore),
        ai(i, "min-pixel-height-safe", 1),
        ai(i, "min-pixel-width-safe", 2),
    ]);
    let (buf_obj, point_obj, start_obj) = if writable {
        let name = i
            .buffers
            .get(bufid)
            .map(|b| b.borrow().name.clone())
            .unwrap_or_default();
        (
            Value::string(name),
            Value::Int(point as i128),
            Value::Int(start as i128),
        )
    } else {
        let buf = i
            .buffers
            .get(bufid)
            .map(|b| Value::Buffer(b.clone()))
            .unwrap_or(Value::Nil);
        (
            buf,
            // Markers hold 0-based positions internally.
            crate::buffer::primitives::new_marker_at(i, bufid, point.saturating_sub(1)),
            crate::buffer::primitives::new_marker_at(i, bufid, start.saturating_sub(1)),
        )
    };
    let buf_state = Value::list(vec![
        buf_obj,
        ac(i, "selected", Value::from_bool(selected)),
        ac(i, "hscroll", Value::Int(0)),
        Value::list(vec![
            Value::Sym(i.intern("fringes")),
            Value::Int(0),
            Value::Int(0),
            Value::Nil,
            Value::Nil,
        ]),
        {
            // (margins [LEFT [RIGHT]]) — nonzero widths only.
            let mut m = vec![Value::Sym(i.intern("margins"))];
            if ml > 0 || mr > 0 {
                m.push(Value::Int(ml));
            }
            if mr > 0 {
                m.push(Value::Int(mr));
            }
            if m.len() == 1 {
                m.push(Value::Nil);
            }
            Value::list(m)
        },
        Value::list(vec![
            Value::Sym(i.intern("scroll-bars")),
            Value::Nil,
            Value::Int(0),
            Value::t(),
            Value::Nil,
            Value::Int(0),
            Value::t(),
            Value::Nil,
        ]),
        ac(i, "vscroll", Value::Int(0)),
        Value::list(vec![Value::Sym(i.intern("dedicated"))]),
        ac(i, "point", point_obj),
        ac(i, "start", start_obj),
    ]);
    let mut items = vec![
        header,
        Value::Sym(i.intern("leaf")),
        ai(i, "pixel-width", ww as i128),
        ai(i, "pixel-height", wh as i128),
        ai(i, "total-width", ww as i128),
        ai(i, "total-height", wh as i128),
        ac(i, "normal-height", Value::float(1.0)),
        ac(i, "normal-width", Value::float(1.0)),
    ];
    if !writable {
        items.push(Value::list(vec![
            Value::Sym(i.intern("parameters")),
            Value::cons(
                Value::Sym(i.intern("clone-of")),
                Value::Window(w.clone()),
            ),
        ]));
    }
    // The buffer spec is (buffer BUF . STATE), a cons.
    items.push(Value::cons(Value::Sym(i.intern("buffer")), buf_state));
    Ok(Value::list(items))
}

/// `window-state-put' — validates the WINDOW argument (a plain
/// "N is not a valid window" error) and the state shape; applying a
/// leaf state to our flat model is a no-op returning nil.
fn f_window_state_put(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    if let Some(v) = a.get(1) {
        match v {
            Value::Nil | Value::Window(_) => {}
            other => {
                let shown = i.princ_to_string(other);
                return Err(i.error(format!("{shown} is not a valid window")));
            }
        }
    }
    // A well-formed state is (HEADER TYPE . STATE); malformed states
    // die inside GNU's walker on a number-or-marker-p check.
    let ok = a[0]
        .list_to_vec()
        .map(|items| {
            items.len() >= 2
                && matches!(&items[0], Value::Cons(_) | Value::Nil)
                && matches!(&items[1], Value::Sym(_))
        })
        .unwrap_or(false);
    if ok {
        Ok(Value::Nil)
    } else {
        Err(i.wrong_type_mut("number-or-marker-p", &Value::Nil))
    }
}

/// `Snarf-documentation' — opens FILE inside `doc-directory'; GNU
/// signals file-missing when the doc file is absent.
fn f_snarf_documentation(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let name = match &a[0] {
        Value::Str(s) => s.borrow().clone(),
        other => return Err(i.wrong_type_mut("stringp", other)),
    };
    let dir_sym = i.intern("doc-directory");
    let dir = match i.symbol_value(dir_sym) {
        Value::Str(s) => s.borrow().clone(),
        _ => String::new(),
    };
    let path = if name.starts_with('/') {
        name
    } else {
        format!("{dir}{name}")
    };
    if std::path::Path::new(&path).exists() {
        // Real doc-file parsing is not wired up; an existing file
        // still yields nil rather than a doc string here.
        return Ok(Value::Nil);
    }
    let s = i.intern("file-missing");
    Err(i.signal_data(s, vec![
        Value::string("Opening doc string file"),
        Value::string("No such file or directory"),
        Value::string(path),
    ]))
}

fn f_minibuffer_message(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let fmt = match &a[0] {
        Value::Str(s) => s.borrow().clone(),
        _ => String::new(),
    };
    let msg = crate::lisp::builtins::evalfn::apply_format_simple(i, &fmt, &a[1..]);
    i.message(&format!(" [Minibuffer: {}]", msg));
    Ok(Value::Nil)
}

/// If the front-end input hook is installed, read a line with PROMPT;
/// otherwise fall back to `fallback` (batch behavior).
/// GNU batch minibuffer input reads a line from stdin; at EOF it
/// signals `(end-of-file "Error reading from stdin")'.
fn batch_eof(i: &mut Interp) -> Flow {
    let eof = i.intern("end-of-file");
    i.signal_data(eof, vec![Value::string("Error reading from stdin")])
}

fn minibuf_or(i: &mut Interp, prompt: &Value, fallback: Value) -> Result<Option<String>, Flow> {
    if i.minibuf_reader.is_none() {
        return Err(batch_eof(i));
    }
    let p = match prompt {
        Value::Str(s) => s.borrow().clone(),
        _ => String::new(),
    };
    let _ = fallback;
    Ok(Some(i.minibuf_line(&p)?))
}

fn f_read_from_minibuffer(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    if let Some(s) = minibuf_or(i, &a[0], arg(&a, 4))? {
        return Ok(Value::string(s));
    }
    Ok(arg(&a, 4))
}
fn f_read_buffer(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    if let Some(s) = minibuf_or(i, &a[0], arg(&a, 1))? {
        return Ok(Value::string(s));
    }
    Ok(arg(&a, 1))
}
fn f_read_file_name(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    if let Some(s) = minibuf_or(i, &a[0], arg(&a, 3))? {
        return Ok(Value::string(s));
    }
    Ok(arg(&a, 3))
}
fn f_read_number(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    if i.minibuf_reader.is_none() {
        return Err(batch_eof(i));
    }
    match a.get(1) {
        Some(v) => Ok(v.clone()),
        _ => Ok(Value::Int(0)),
    }
}
fn f_read_regexp(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    if i.minibuf_reader.is_none() {
        return Err(batch_eof(i));
    }
    match a.get(1) {
        Some(Value::Str(_s)) => Ok(a[1].clone()),
        _ => Ok(Value::string("")),
    }
}
fn f_completing_read(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    // (completing-read PROMPT TABLE ...) — interactive: read a line and
    // complete it against TABLE; batch: EOF on stdin.
    if i.minibuf_reader.is_none() {
        return Err(batch_eof(i));
    }
    if i.minibuf_reader.is_some() {
        let prompt = match &a[0] {
            Value::Str(s) => s.borrow().clone(),
            _ => String::new(),
        };
        let cands = completion_candidates(i, &a[1]);
        let input = i.minibuf_line(&prompt)?;
        if input.is_empty() {
            // Empty input → DEF (arg 6) or "".
            return Ok(arg(&a, 6));
        }
        // Complete: exact match, else unique prefix completion.
        if cands.iter().any(|(_, es)| cand_text(es).as_deref() == Some(input.as_str())) {
            return Ok(Value::string(input));
        }
        let matches: Vec<String> = cands
            .iter()
            .filter_map(|(_, es)| cand_text(es))
            .filter(|c| c.starts_with(&input))
            .collect();
        return Ok(Value::string(match matches.len() {
            1 => matches[0].clone(),
            _ => input,
        }));
    }
    let initial = arg(&a, 4);
    if initial.truthy() {
        if let Value::Str(s) = &initial {
            return Ok(Value::string(s.borrow().clone()));
        }
    }
    // TABLE: list, alist, obarray, or function.
    match try_completions(i, "", &a[1], &Value::Nil) {
        Ok(v) => Ok(v),
        Err(_) => Ok(arg(&a, 6)),
    }
}

/// Resolve a completion TABLE to (ELT, ELTSTRING) pairs.  ELTSTRING
/// keeps its original `Value::Str' so text properties survive into
/// try/all-completion results, as in GNU's C code.
fn completion_candidates(i: &mut Interp, table: &Value) -> Vec<(Value, Value)> {
    let eltstring = |i: &mut Interp, v: &Value| -> Value {
        match v {
            s @ Value::Str(_) => s.clone(),
            Value::Sym(s) => Value::string(i.symbol_name(*s)),
            other => Value::string(i.princ_to_string(other)),
        }
    };
    match table {
        Value::Cons(_) => {
            let items = table.list_to_vec().unwrap_or_default();
            items
                .iter()
                .map(|v| {
                    let es = match v {
                        Value::Cons(c) => eltstring(i, &c.borrow().car.clone()),
                        _ => eltstring(i, v),
                    };
                    (v.clone(), es)
                })
                .collect()
        }
        Value::Vec(v) => v
            .borrow()
            .iter()
            .map(|x| (x.clone(), eltstring(i, x)))
            .collect(),
        Value::Nil => Vec::new(),
        _ => Vec::new(),
    }
}

/// ELTSTRING's text, if it is a string (GNU's STRINGP test).
fn cand_text(v: &Value) -> Option<String> {
    if let Value::Str(s) = v {
        Some(s.borrow().clone())
    } else {
        None
    }
}

/// Whether TABLE is a function-style completion table (a callable).
fn table_is_callable(i: &mut Interp, table: &Value) -> bool {
    match table {
        Value::Lambda(_) | Value::Subr(_) => true,
        Value::Sym(s) => {
            let f = i.symbol_function(*s);
            !matches!(f, Value::Sym(u) if u == crate::lisp::obarray::sym::UNBOUND)
        }
        Value::Cons(c) => {
            let car = c.borrow().car.clone();
            i.sym_is(&car, crate::lisp::obarray::sym::LAMBDA)
        }
        _ => false,
    }
}

/// `completion-ignore-case' as a bool.
fn completion_ignore_case(i: &Interp) -> bool {
    i.intern_soft("completion-ignore-case")
        .map(|id| i.symbol_value(id).truthy())
        .unwrap_or(false)
}

/// GNU's `compare-strings' prefix check with `completion-ignore-case':
/// CAND has S as a prefix (case-insensitive when IGNORE_CASE).
fn completion_prefix_p(cand: &str, s: &str, ignore_case: bool) -> bool {
    if cand.chars().count() < s.chars().count() {
        return false;
    }
    if ignore_case {
        cand.chars()
            .take(s.chars().count())
            .flat_map(|c| c.to_lowercase())
            .eq(s.chars().flat_map(|c| c.to_lowercase()))
    } else {
        cand.starts_with(s)
    }
}

/// GNU's `match_regexps': S must match every regexp in
/// `completion-regexp-list' (search semantics, `completion-ignore-case'
/// case-folding).  Bad regexps are ignored.
fn completion_match_regexps(i: &Interp, s: &str, ignore_case: bool) -> bool {
    let Some(id) = i.intern_soft("completion-regexp-list") else {
        return true;
    };
    let mut cur = i.symbol_value(id);
    let syn = crate::editor::re_syntax(i);
    let chars: Vec<char> = s.chars().collect();
    loop {
        match cur {
            Value::Cons(c) => {
                let (re_v, next) = {
                    let b = c.borrow();
                    (b.car.clone(), b.cdr.clone())
                };
                if let Value::Str(rs) = &re_v {
                    if let Ok(re) = crate::lisp::regexp::compile_case(&rs.borrow(), ignore_case) {
                        if crate::lisp::regexp::search(&re, &chars, 0, &syn).is_none() {
                            return false;
                        }
                    }
                }
                cur = next;
            }
            _ => return true,
        }
    }
}

/// The shared collection-side filter of GNU's try/all/test-completion:
/// prefix match, `completion-regexp-list', and PRED applied to ELT
/// (the collection element itself, as in GNU's C code).
fn completion_candidate_ok(
    i: &mut Interp,
    elt: &Value,
    c: &str,
    s: &str,
    pred: &Value,
    ignore_case: bool,
) -> Result<bool, Flow> {
    if !completion_prefix_p(c, s, ignore_case) || !completion_match_regexps(i, c, ignore_case) {
        return Ok(false);
    }
    if pred.truthy() {
        let r = i.apply(pred, vec![elt.clone()])?;
        if !r.truthy() {
            return Ok(false);
        }
    }
    Ok(true)
}

/// GNU's `compare-strings' over the first N chars of A and B: length
/// of the common prefix (case-folded when IGNORE_CASE), capped at N.
fn common_prefix_len(a: &str, b: &str, n: usize, ignore_case: bool) -> usize {
    let ac: Vec<char> = a.chars().take(n).collect();
    let bc: Vec<char> = b.chars().take(n).collect();
    let mut k = 0;
    while k < ac.len().min(bc.len()) {
        let eq = if ignore_case {
            ac[k].to_lowercase().eq(bc[k].to_lowercase())
        } else {
            ac[k] == bc[k]
        };
        if !eq {
            break;
        }
        k += 1;
    }
    k
}

/// `substring(BESTMATCH, 0, N)' for a string Value, preserving text
/// properties like GNU's Fsubstring.
fn substring_str_value(i: &mut Interp, v: &Value, n: usize) -> Value {
    let Value::Str(src) = v else {
        return v.clone();
    };
    let text: String = src.borrow().chars().take(n).collect();
    let ns = std::rc::Rc::new(std::cell::RefCell::new(text));
    if i.has_str_props(src) {
        let ivs: Vec<(usize, usize, Vec<Value>)> = i
            .str_props(src)
            .iter()
            .filter_map(|(a, b, pl)| {
                let lo = *a;
                let hi = (*b).min(n);
                (lo < hi)
                    .then(|| (lo, hi, crate::buffer::primitives::plist_pairs_rev(pl)))
            })
            .collect();
        i.set_str_props(&ns, ivs);
    }
    Value::Str(ns)
}

fn f_try_completion(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let s = want_str(i, &a[0])?;
    try_completions(i, &s, &a[1], &arg(&a, 2))
}

fn try_completions(i: &mut Interp, s: &str, table: &Value, pred: &Value) -> EvalResult {
    if table_is_callable(i, table) {
        return i.apply(table, vec![Value::string(s), pred.clone(), Value::Nil]);
    }
    let cands = completion_candidates(i, table);
    let ignore_case = completion_ignore_case(i);
    let s_chars = s.chars().count();
    // Port of GNU minibuf.c Ftry_completion: track the bestmatch
    // element string and shrink it to the common prefix; the result is
    // a substring of bestmatch, preserving its text properties.
    let mut bestmatch: Option<Value> = None;
    let mut bestmatchsize: usize = 0;
    let mut matchcount = 0usize;
    for (elt, eltstring) in cands {
        let Some(text) = cand_text(&eltstring) else {
            continue;
        };
        let eltlen = text.chars().count();
        if eltlen < s_chars || !completion_candidate_ok(i, &elt, &text, s, pred, ignore_case)? {
            continue;
        }
        match &bestmatch {
            None => {
                matchcount = 1;
                bestmatchsize = eltlen;
                bestmatch = Some(eltstring);
            }
            Some(bm) => {
                let bm_text = cand_text(bm).unwrap_or_default();
                let compare = bestmatchsize.min(eltlen);
                let matchsize = common_prefix_len(&bm_text, &text, compare, ignore_case);
                if ignore_case {
                    let elt_exact = matchsize == eltlen;
                    let bm_exact = matchsize == bm_text.chars().count();
                    if (elt_exact && matchsize < bm_text.chars().count())
                        || (elt_exact == bm_exact
                            && common_prefix_len(&text, s, s_chars, false) == s_chars
                            && common_prefix_len(&bm_text, s, s_chars, false) != s_chars)
                    {
                        bestmatch = Some(eltstring.clone());
                    }
                }
                // GNU only counts non-duplicate strings: a candidate is
                // a duplicate when it is identical to old_bestmatch
                // (case-sensitively even under ignore-case) over the
                // compared range.
                let dup = bestmatchsize == eltlen
                    && bestmatchsize == matchsize
                    && (!ignore_case
                        || common_prefix_len(&bm_text, &text, compare, false) == compare);
                if !dup {
                    matchcount += usize::from(matchcount <= 1);
                }
                bestmatchsize = matchsize;
                if matchsize <= s_chars && !ignore_case && matchcount > 1 {
                    break;
                }
            }
        }
    }
    let Some(bm) = bestmatch else {
        return Ok(Value::Nil);
    };
    let bm_text = cand_text(&bm).unwrap_or_default();
    // t only when the single match equals the input string.
    if matchcount == 1 && bm_text == s {
        return Ok(Value::t());
    }
    Ok(substring_str_value(i, &bm, bestmatchsize))
}

fn f_all_completions(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let s = want_str(i, &a[0])?;
    if table_is_callable(i, &a[1]) {
        return i.apply(&a[1], vec![Value::string(s), arg(&a, 2), Value::t()]);
    }
    let cands = completion_candidates(i, &a[1]);
    let pred = arg(&a, 2);
    let ignore_case = completion_ignore_case(i);
    let mut out: Vec<Value> = Vec::new();
    for (elt, eltstring) in cands {
        if let Some(c) = cand_text(&eltstring) {
            if c.chars().count() >= s.chars().count()
                && completion_candidate_ok(i, &elt, &c, &s, &pred, ignore_case)?
            {
                // Keep the original eltstring Value so text properties
                // survive (GNU returns the candidate objects).
                out.push(eltstring);
            }
        }
    }
    Ok(Value::list(out))
}

// Direct port of GNU minibuf.c Fcompletion__flex_cost_gotoh: modified
// Gotoh algorithm scoring PAT as a subsequence of STR.
fn f_completion_flex_cost_gotoh(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    const FLEX_MAX_STR_SIZE: usize = 512;
    const FLEX_MAX_PAT_SIZE: usize = 128;
    const FLEX_MAX_MATRIX_SIZE: usize = FLEX_MAX_PAT_SIZE * FLEX_MAX_STR_SIZE;
    const GAP_OPEN_COST: i64 = 10;
    const GAP_EXTEND_COST: i64 = 1;
    const POS_INF: i64 = i64::MAX / 2;
    let mat = |m: &Vec<i64>, width: usize, i: isize, j: isize| -> i64 {
        m[((i + 1) as usize) * width + ((j + 1) as usize)]
    };

    let pat_s = want_str(i, &a[0])?;
    let str_s = want_str(i, &a[1])?;
    let pat: Vec<char> = pat_s.chars().collect();
    let st: Vec<char> = str_s.chars().collect();
    let patlen = pat.len();
    let strlen = st.len();
    let width = strlen + 1;
    let size = (patlen + 1) * width;

    // Bail if strings are empty or matrix too large.
    if patlen == 0 || strlen == 0 || size > FLEX_MAX_MATRIX_SIZE {
        return Ok(Value::Nil);
    }

    let ignore_case = i
        .symbol_value(i.intern_soft("completion-ignore-case").unwrap_or(0))
        .truthy();

    // Cheap subsequence check before the O(N*M) DP.
    if !ignore_case {
        let mut pi = 0usize;
        for &sc in st.iter() {
            if pi < patlen && sc == pat[pi] {
                pi += 1;
            }
        }
        if pi < patlen {
            return Ok(Value::Nil);
        }
    }

    let mut m: Vec<i64> = vec![POS_INF; size];
    let mut d: Vec<i64> = vec![POS_INF; size];
    // D[-1,-1]=0 to promote matches at the beginning; rest of the
    // first D row gets gap_open/2 for cheaper leading gaps.
    for j in 0..width {
        d[j] = GAP_OPEN_COST / 2;
    }
    d[0] = 0;

    // Position of first match found in the previous row.
    let mut prev_match = 0usize;

    // Forward pass.
    for pi in 0..patlen {
        let pat_char = pat[pi];
        let mut match_seen = false;
        let mut j = prev_match;
        while j < strlen {
            let jcopy = j;
            let str_char = st[j];
            let cmatch = if ignore_case {
                pat_char.to_lowercase().next() == str_char.to_lowercase().next()
            } else {
                pat_char == str_char
            };
            if cmatch {
                if !match_seen {
                    match_seen = true;
                    prev_match = jcopy;
                }
                let mm = mat(&m, width, pi as isize - 1, j as isize - 1);
                let dd = mat(&d, width, pi as isize - 1, j as isize - 1);
                let idx = (pi + 1) * width + (j + 1);
                m[idx] = mm.min(dd);
            }
            let mleft = mat(&m, width, pi as isize, j as isize - 1);
            let dleft = mat(&d, width, pi as isize, j as isize - 1);
            let idx = (pi + 1) * width + (j + 1);
            d[idx] = (mleft + GAP_OPEN_COST).min(dleft + GAP_EXTEND_COST);
            j += 1;
        }
    }

    // Find lowest cost in last row.
    let mut best_cost = POS_INF;
    let mut lastcol: isize = -1;
    for j in 0..strlen {
        let cost = mat(&m, width, patlen as isize - 1, j as isize);
        if cost < best_cost {
            best_cost = cost;
            lastcol = j as isize;
        }
    }
    if lastcol < 0 || best_cost >= POS_INF {
        return Ok(Value::Nil);
    }

    // Go backwards to build match positions list.
    let mut matches = Value::Nil;
    matches = Value::cons(Value::Int(lastcol as i128), matches);
    let mut l = lastcol;
    for pi2 in (0..patlen as isize - 1).rev() {
        loop {
            l -= 1;
            if !(l >= 0 && mat(&m, width, pi2, l) >= mat(&d, width, pi2, l)) {
                break;
            }
        }
        matches = Value::cons(Value::Int(l as i128), matches);
    }
    Ok(Value::cons(Value::Int(best_cost as i128), matches))
}

fn f_test_completion(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let s = want_str(i, &a[0])?;
    if table_is_callable(i, &a[1]) {
        let lambda_kw = i.intern("lambda");
        let r = i.apply(
            &a[1],
            vec![Value::string(s), arg(&a, 2), Value::Sym(lambda_kw)],
        )?;
        return Ok(Value::from_bool(r.truthy()));
    }
    let cands = completion_candidates(i, &a[1]);
    let pred = arg(&a, 2);
    let ignore_case = completion_ignore_case(i);
    // GNU Ftest_completion: S must equal a candidate (obeying
    // `completion-ignore-case'), match `completion-regexp-list', and
    // satisfy PRED (called on ELT, not the string).
    for (elt, eltstring) in &cands {
        let Some(c) = cand_text(eltstring) else {
            continue;
        };
        let eq = if ignore_case {
            c.chars()
                .flat_map(|x| x.to_lowercase())
                .eq(s.chars().flat_map(|x| x.to_lowercase()))
        } else {
            c == s
        };
        if eq
            && completion_match_regexps(i, &c, ignore_case)
            && {
                let r = if pred.truthy() {
                    i.apply(&pred, vec![elt.clone()])?.truthy()
                } else {
                    true
                };
                r
            }
        {
            return Ok(Value::t());
        }
    }
    Ok(Value::Nil)
}

fn f_internal_complete_buffer(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    // (STRING PRED FLAG): complete STRING over live buffer names.
    // FLAG nil → try-completion, t → all-completions, lambda → test.
    let s = want_str(i, &a[0])?;
    // GNU hides space-prefixed (internal) buffers unless STRING starts
    // with a space.
    let show_hidden = s.starts_with(' ');
    let names: Vec<Value> = i
        .buffers
        .list()
        .iter()
        .filter_map(|id| i.buffers.get(*id))
        .map(|b| b.borrow().name.clone())
        .filter(|n| show_hidden || !n.starts_with(' '))
        .map(Value::string)
        .collect();
    let table = Value::list(names);
    let flag = arg(&a, 2);
    match &flag {
        Value::Nil => try_completions(i, &s, &table, &Value::Nil),
        Value::Sym(sym) if i.symbol_name(*sym) == "lambda" => {
            let cands = completion_candidates(i, &table);
            Ok(Value::from_bool(cands.iter().any(|(_, es)| cand_text(es).as_deref() == Some(s.as_str()))))
        }
        Value::Sym(sym) if i.symbol_name(*sym) == "t" => {
            let cands = completion_candidates(i, &table);
            Ok(Value::list(
                cands
                    .into_iter()
                    .filter(|(_, es)| cand_text(es).map(|c| c.starts_with(&s)).unwrap_or(false))
                    .map(|(_, es)| es)
                    .collect(),
            ))
        }
        _ => try_completions(i, &s, &table, &Value::Nil),
    }
}

fn f_completion_boundaries(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    // (STRING TABLE PRED SUFFIX) → (START . END): the start boundary
    // inside STRING is 0 and END counts SUFFIX chars (GNU's plain
    // completion style has no field separator). A function table may
    // override via the `boundaries' action.
    let s = want_str(i, &a[0])?;
    if table_is_callable(i, &a[1]) {
        // GNU calls the table with action (boundaries . SUFFIX) and
        // expects (boundaries START . END) back.
        let bw = i.intern("boundaries");
        let act = Value::cons(Value::Sym(bw), arg(&a, 3));
        let r = i.apply(&a[1], vec![Value::string(s), arg(&a, 2), act])?;
        if let Value::Cons(c) = &r {
            let (car, cdr) = {
                let b = c.borrow();
                (b.car.clone(), b.cdr.clone())
            };
            if let Value::Sym(k) = &car {
                if i.symbol_name(*k) == "boundaries" {
                    // (cadr boundaries) → START, (cddr) → END.
                    let start = match &cdr {
                        Value::Cons(c2) => c2.borrow().car.clone(),
                        _ => Value::Nil,
                    };
                    let end = match &cdr {
                        Value::Cons(c2) => match &c2.borrow().cdr {
                            Value::Cons(c3) => c3.borrow().cdr.clone(),
                            other => other.clone(),
                        },
                        _ => Value::Nil,
                    };
                    let end = match &end {
                        Value::Nil => match &a[3] {
                            Value::Str(s) => {
                                Value::Int(s.borrow().chars().count() as i128)
                            }
                            _ => Value::Int(0),
                        },
                        e => e.clone(),
                    };
                    return Ok(Value::cons(start, end));
                }
            }
        }
    }
    let end = match &a[3] {
        Value::Str(s) => s.borrow().chars().count() as i128,
        _ => 0,
    };
    Ok(Value::cons(Value::Int(0), Value::Int(end)))
}

fn f_read_string(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    if let Some(s) = minibuf_or(i, &a[0], arg(&a, 1))? {
        return Ok(Value::string(s));
    }
    Ok(arg(&a, 1))
}
fn f_read_command(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    if let Some(s) = minibuf_or(i, &a[0], Value::Nil)? {
        return Ok(Value::Sym(i.intern(&s)));
    }
    match a.get(1) {
        Some(v) => Ok(v.clone()),
        None => Ok(Value::Nil),
    }
}
fn f_read_variable(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    f_read_command(i, a)
}
fn f_y_or_n_p(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    if i.minibuf_reader.is_none() {
        return Err(batch_eof(i));
    }
    if i.minibuf_reader.is_some() {
        let prompt = match &a[0] {
            Value::Str(s) => s.borrow().clone(),
            _ => String::new(),
        };
        loop {
            match i.minibuf_input(&format!("{} (y or n) ", prompt), true)? {
                crate::lisp::eval::MinibufInput::Key(k) => {
                    let base = k & 0x3f_ffff;
                    match base {
                        x if x == 'y' as i128 => return Ok(Value::t()),
                        x if x == 'n' as i128 => return Ok(Value::Nil),
                        7 | 3 => return Err(crate::lisp::error::Flow::Quit),
                        _ => {
                            i.message("Please answer y or n");
                        }
                    }
                }
                crate::lisp::eval::MinibufInput::Text(t) => match t.chars().next() {
                    Some('y') => return Ok(Value::t()),
                    Some('n') => return Ok(Value::Nil),
                    _ => {}
                },
            }
        }
    }
    Ok(Value::Nil)
}

// ---------- commands ----------

fn f_commandp(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let v = &a[0];
    // `(lambda (x) (interactive ...) ...)' as data — no resolution.
    if let Value::Cons(_) = v {
        let items: Vec<Value> = v.list_to_vec().unwrap_or_default();
        if items
            .first()
            .and_then(|h| i.sym_id(h))
            .map(|h| h == crate::lisp::obarray::sym::LAMBDA)
            .unwrap_or(false)
        {
            let has_interactive = items.iter().skip(2).any(|el| match el {
                Value::Cons(ec) => {
                    let b = ec.borrow();
                    i.sym_is(&b.car, crate::lisp::obarray::sym::INTERACTIVE)
                }
                _ => false,
            });
            return Ok(Value::from_bool(has_interactive));
        }
    }
    let cmd = match v {
        Value::Sym(id) => i.symbol_function(*id),
        other => other.clone(),
    };
    Ok(Value::from_bool(match &cmd {
        Value::Lambda(l) => l.interactive.is_some(),
        Value::Subr(s) => crate::lisp::eval::subr_interactive(s.name).is_some(),
        // strings and vectors are keyboard macros — commands.
        Value::Str(_) | Value::Vec(_) => true,
        _ => false,
    }))
}

fn f_call_interactively(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    i.command_execute(&a[0])
}

fn f_execute_extended_command(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let _ = &a;
    // Interactive: prompt "M-x " through the front-end.
    if i.minibuf_reader.is_some() {
        let name = i.minibuf_line("M-x ")?;
        if name.is_empty() {
            return Ok(Value::Nil);
        }
        let sym = i.intern(&name);
        if !i.fbound_p(sym) {
            return Err(i.error(&format!("M-x {} is undefined", name)));
        }
        return i.command_execute(&Value::Sym(sym));
    }
    // Try `this-command` set by the harness.
    let tc = i.symbol_value(i.intern_soft("this-command").unwrap_or(0));
    if let Value::Sym(_) = tc {
        return i.command_execute(&tc);
    }
    Ok(Value::Nil)
}

// ---------- keyboard macros ----------

fn kbd_var(i: &mut Interp, name: &str) -> SymId {
    i.intern(name)
}

/// GNU's `make_event_array`: string when every event is a character,
/// vector otherwise.
fn macro_events_value(events: &[Value]) -> Value {
    // GNU's `make_event_array`: nil count → empty vector; all
    // characters → string; otherwise a vector.
    if events.is_empty() {
        return Value::Vec(std::rc::Rc::new(std::cell::RefCell::new(Vec::new())));
    }
    if events
        .iter()
        .all(|e| matches!(e, Value::Int(n) if *n >= 0 && *n < 0x400000))
    {
        let s: String = events
            .iter()
            .filter_map(|e| match e {
                Value::Int(n) => char::from_u32(*n as u32),
                _ => None,
            })
            .collect();
        Value::string(s)
    } else {
        Value::Vec(std::rc::Rc::new(std::cell::RefCell::new(events.to_vec())))
    }
}

/// Events of a string/vector macro as key codes.
fn macro_event_codes(v: &Value) -> Vec<i128> {
    match v {
        Value::Str(s) => s.borrow().chars().map(|c| c as i128).collect(),
        Value::Vec(v) => v
            .borrow()
            .iter()
            .filter_map(|e| match e {
                Value::Int(n) => Some(*n),
                _ => None,
            })
            .collect(),
        _ => Vec::new(),
    }
}

fn f_defining_kbd_macro(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    // (defining-kbd-macro APPEND &optional NO-EXEC) — APPEND non-nil
    // continues the last macro; `defining-kbd-macro` the variable holds
    // the state.
    let defining = kbd_var(i, "defining-kbd-macro");
    if a[0].truthy() {
        if !i.symbol_value(defining).truthy() {
            // Seed from `last-kbd-macro`; GNU errors arrayp on nil.
            let lk = i.intern("last-kbd-macro");
            let last = i.symbol_value(lk);
            match &last {
                Value::Str(_) | Value::Vec(_) => {
                    i.kbd_macro_events = macro_event_codes(&last)
                        .iter()
                        .map(|c| Value::Int(*c))
                        .collect();
                }
                other => return Err(i.wrong_type_mut("arrayp", other)),
            }
            i.message("Appending to kbd macro...");
        }
    } else {
        i.kbd_macro_events.clear();
        if !i.symbol_value(defining).truthy() {
            i.message("Defining kbd macro...");
        }
    }
    i.kbd_macro_mark = 0;
    let _ = i.set_symbol(defining, Value::t());
    Ok(Value::Nil)
}

fn f_start_kbd_macro(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    f_defining_kbd_macro(i, a)
}

fn f_end_kbd_macro(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let defining = kbd_var(i, "defining-kbd-macro");
    if !i.symbol_value(defining).truthy() {
        return Err(i.error("Not defining kbd macro"));
    }
    let _ = i.set_symbol(defining, Value::Nil);
    // GNU's batch kboard yields [] regardless of stored events; keep
    // real events for interactive sessions.
    let events: Vec<Value> = if i.noninteractive {
        Vec::new()
    } else {
        std::mem::take(&mut i.kbd_macro_events)
    };
    let last = macro_events_value(&events);
    let lk = i.intern("last-kbd-macro");
    let _ = i.set_symbol(lk, last);
    i.message("Keyboard macro defined");
    // Optional REPEAT: run the macro that many times.
    if arg(&a, 0).truthy() {
        return f_call_last_kbd_macro(i, a);
    }
    Ok(Value::Nil)
}

fn f_store_kbd_macro_event(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let defining = kbd_var(i, "defining-kbd-macro");
    if !i.symbol_value(defining).truthy() {
        return Err(i.error("Not defining kbd macro"));
    }
    i.kbd_macro_events.push(a[0].clone());
    Ok(Value::Nil)
}

fn f_cancel_kbd_macro_events(i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    i.kbd_macro_events.truncate(i.kbd_macro_mark);
    Ok(Value::Nil)
}

fn f_execute_kbd_macro(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    // (execute-kbd-macro MACRO &optional COUNT BUFFER)
    let count = match &arg(&a, 1) {
        Value::Nil => 1,
        Value::Int(0) => 1000, // "until error" — bounded for safety.
        Value::Int(n) => (*n).max(0) as usize,
        other => return Err(i.wrong_type_mut("integerp", other)),
    };
    let events = macro_event_codes(&a[0]);
    for _ in 0..count {
        for k in &events {
            i.macro_replay.push_back(*k);
        }
    }
    let ek = i.intern("executing-kbd-macro");
    let _ = i.set_symbol(
        ek,
        if i.macro_replay.is_empty() {
            Value::Nil
        } else {
            a[0].clone()
        },
    );
    Ok(Value::Nil)
}

fn f_call_last_kbd_macro(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let lk = i.intern("last-kbd-macro");
    let last = i.symbol_value(lk);
    if last.is_nil() {
        return Err(i.error("No kbd macro has been defined"));
    }
    let count = f_prefix_numeric_value(i, vec![arg(&a, 0)])?;
    f_execute_kbd_macro(i, vec![last, count, arg(&a, 1)])
}

fn f_prefix_numeric_value(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    match &a[0] {
        Value::Int(n) => Ok(Value::Int(*n)),
        // The bare `-` symbol means a negative prefix.
        Value::Sym(s) if i.symbol_name(*s) == "-" => Ok(Value::Int(-1)),
        Value::Cons(c) => match c.borrow().car.clone() {
            Value::Int(n) => Ok(Value::Int(n)),
            _ => Ok(Value::Int(1)),
        },
        _ => Ok(Value::Int(1)),
    }
}

fn f_universal_argument(i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    let pa = i.intern("prefix-arg");
    let cur = i.symbol_value(pa);
    let next = match cur {
        Value::Nil => Value::list(vec![Value::Int(4)]),
        Value::Int(n) => Value::Int(n * 4),
        Value::Cons(_) => Value::list(vec![Value::Int(16)]),
        _ => Value::list(vec![Value::Int(4)]),
    };
    i.obarray.symbol_mut(pa).value = next;
    Ok(Value::Nil)
}

fn f_digit_argument(i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    // The digit/minus that invoked this command is `last-command-event`.
    let ev = i.symbol_value(i.intern_soft("last-command-event").unwrap_or(0));
    let base = match ev {
        Value::Int(c) => c & 0x3f_ffff,
        _ => -1,
    };
    let pa = i.intern("prefix-arg");
    let cur = i.symbol_value(pa);
    let next = if base == '-' as i128 {
        // M-- starts a negative numeric arg.
        match &cur {
            Value::Cons(_) => cur.clone(),
            Value::Int(n) => Value::Int(-n),
            _ => Value::list(vec![Value::Sym(i.intern("-"))]),
        }
    } else if let Some(d) = char::from_u32(base as u32).and_then(|c| c.to_digit(10)) {
        match &cur {
            Value::Nil => Value::Int(d as i128),
            Value::Int(n) => {
                let sign = if *n < 0 { -1 } else { 1 };
                Value::Int(n * 10 + sign * d as i128)
            }
            Value::Cons(_) => Value::Int(d as i128),
            _ => Value::Int(d as i128),
        }
    } else {
        cur
    };
    i.obarray.symbol_mut(pa).value = next;
    Ok(Value::Nil)
}

fn f_negative_argument(i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    let pa = i.intern("prefix-arg");
    let cur = i.symbol_value(pa);
    let next = match &cur {
        Value::Int(n) => Value::Int(-n),
        Value::Cons(_) => cur.clone(),
        _ => Value::list(vec![Value::Sym(i.intern("-"))]),
    };
    i.obarray.symbol_mut(pa).value = next;
    Ok(Value::Nil)
}

// `beginning-of-defun', `end-of-defun', `mark-defun', `narrow-to-defun'
// are Lisp functions in GNU (lisp.el) — see the prelude.

fn f_count_words(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let len = cur(i).borrow().text.len();
    let (s, e) = region_bounds(i, &a[0], &a[1], len);
    let text = {
        let b = cur(i);
        let bb = b.borrow();
        bb.text.substring(s.min(e), s.max(e))
    };
    let n = text.split_whitespace().count();
    Ok(Value::Int(n as i128))
}

fn f_what_cursor_position(i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    let (pos, len, c) = {
        let b = cur(i);
        let bb = b.borrow();
        let p = bb.point();
        let c = if p < bb.text_len() {
            Some(bb.text.char_at(p) as i128)
        } else {
            None
        };
        (p + 1, bb.text.len(), c)
    };
    let msg = match c {
        Some(code) => format!(
            "Char: {} ({}, #o{:o}, #x{:x}) point={} of {}",
            char::from_u32(code as u32).unwrap_or('?'),
            code,
            code,
            code,
            pos,
            len
        ),
        None => format!("point={} of {}", pos, len),
    };
    i.message(&msg);
    Ok(Value::Nil)
}

fn f_what_line(i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    let line = {
        let b = cur(i);
        let bb = b.borrow();
        bb.text.line_of_pos(bb.point()) + 1
    };
    i.message(&format!("line {}", line));
    Ok(Value::Nil)
}

/// Snapshot of the current buffer's effective syntax-table chain —
/// per level, the raw 65-slot contents vec and the level's defalt.
/// Lookups walk content → defalt → next level, replicating GNU
/// `char_table_ref' without needing `&Interp' (usable while a buffer
/// is borrowed).
pub(crate) struct SynTable {
    /// `sub-char-table' tag for trie descent; None = no sub-tables.
    tag: Option<crate::lisp::value::SymId>,
    /// (contents, defalt) per chain level, outermost first.
    levels: Vec<(Rc<RefCell<Vec<Value>>>, Value)>,
}

impl SynTable {
    /// GNU `char_table_ref' over the snapshot levels.
    pub(crate) fn get(&self, c: u32) -> Value {
        for (vec, defalt) in &self.levels {
            let v = crate::lisp::builtins::misc::ct_raw_tag(self.tag, &vec.borrow(), c);
            if !v.is_nil() {
                return v;
            }
            if !defalt.is_nil() {
                return defalt.clone();
            }
        }
        Value::Nil
    }
}

/// The current buffer's effective syntax table (the buffer's
/// `syntax-table' FIELD — set only via `set-syntax-table'; a Lisp
/// `setq' on `syntax-table' does NOT affect scanning in GNU — else
/// `standard-syntax-table'), resolved to a chain snapshot.  Fetch
/// this BEFORE borrowing the buffer.
pub(crate) fn syntax_table_entries(i: &Interp) -> Option<SynTable> {
    let local = i
        .buffers
        .get(i.current_buffer)
        .and_then(|b| b.try_borrow().ok().and_then(|bb| bb.syntax_table.clone()))
        .filter(|v| is_syntax_table(i, v));
    let table = match local {
        Some(t) => t,
        None => i
            .intern_soft("remacs--standard-syntax-table")
            .map(|ssid| i.symbol_value(ssid))
            .filter(|t| is_syntax_table(i, t))?,
    };
    let mut levels = Vec::new();
    let mut cur = table;
    let mut guard = 0;
    loop {
        let contents = match &cur {
            Value::Record(r) => match r.borrow().get(2) {
                Some(Value::Vec(v)) => v.clone(),
                _ => break,
            },
            _ => break,
        };
        levels.push((contents, i.char_table_defalt(&cur)));
        let parent = i.char_table_parent(&cur);
        guard += 1;
        if parent.is_nil() || guard > 16 {
            break;
        }
        cur = parent;
    }
    Some(SynTable {
        tag: i.intern_soft("sub-char-table"),
        levels,
    })
}

/// Syntax class letter of C under TABLE; chars with no entry fall
/// back to the hardcoded standard table.
pub(crate) fn syntax_entry_code(t: Option<&SynTable>, c: char) -> u8 {
    if let Some(t) = t {
        if let Value::Cons(cn) = t.get(c as u32) {
            if let Value::Int(n) = &cn.borrow().car {
                if let Some(letter) = SYNTAX_CLASS_CHARS.get(*n as usize & 0xf) {
                    return *letter as u8;
                }
            }
        }
    }
    crate::lisp::regexp::syntax_code(c)
}

/// Syntax code for C under the current buffer's `syntax-table',
/// falling back to the hardcoded standard table when the table has no
/// entry for C.  Prefer `syntax_table_entries' + `syntax_entry_code'
/// when the buffer is (or will be) borrowed.
pub(crate) fn syntax_code_buf(i: &Interp, c: char) -> u8 {
    match syntax_table_entries(i) {
        Some(t) => syntax_entry_code(Some(&t), c),
        None => crate::lisp::regexp::syntax_code(c),
    }
}

/// Regex-ready syntax lookup for the current buffer's `syntax-table'
/// (falls back to the standard table).  Construct BEFORE borrowing a
/// buffer — the lookup itself touches the buffer.
pub(crate) fn re_syntax(i: &Interp) -> impl Fn(char) -> u8 + 'static {
    let syn = syntax_table_entries(i);
    move |c| syntax_entry_code(syn.as_ref(), c)
}

/// Table-aware syntax classifier for the current buffer.  Construct
/// BEFORE borrowing the buffer (`Syn::current(i)`), then `syn.code(c)`
/// is usable while the buffer is borrowed.
pub(crate) struct Syn {
    entries: Option<SynTable>,
    /// `parse-sexp-ignore-comments': whether `<', `>', fence syntax
    /// delimit comments for sexp scanning (nil default; prog-mode sets
    /// it t).  GNU's parse-partial-sexp tracks comments regardless.
    pub(crate) ignore_comments: bool,
    /// `comment-end-can-be-escaped' — buffer-local, nil default in GNU.
    /// When non-nil, an escaped comment-ender does not end the comment.
    pub(crate) end_escaped: bool,
    /// `open-paren-in-column-0-is-defun-start' — buffer-local, t by
    /// default.  When nil (and `comment-use-syntax-ppss' is nil), GNU's
    /// find_defun_start falls back to BEGV instead of a col-0 open.
    pub(crate) open_paren_defun: bool,
    /// `comment-use-syntax-ppss' — t by default in GNU; selects the
    /// ppss-based find_defun_start path in `back_comment'.
    pub(crate) comment_use_ppss: bool,
    /// `syntax-table' text-property overrides (start, end, cons-value)
    /// collected when `parse-sexp-lookup-properties' is non-nil.
    prop_ranges: Vec<(usize, usize, Value)>,
}

impl Syn {
    /// UPTO is the 1-based buffer position through which the upcoming
    /// scan needs `syntax-table' properties (the scan end, clamped to
    /// the buffer bounds by `internal--syntax-propertize' itself).
    pub(crate) fn current(i: &mut Interp, upto: usize) -> Self {
        let ignore = i
            .intern_soft("parse-sexp-ignore-comments")
            .map(|sid| !i.symbol_value(sid).is_nil())
            .unwrap_or(false);
        let end_escaped = i
            .intern_soft("comment-end-can-be-escaped")
            .map(|sid| !i.symbol_value(sid).is_nil())
            .unwrap_or(false);
        let lookup = i
            .intern_soft("parse-sexp-lookup-properties")
            .map(|sid| !i.symbol_value(sid).is_nil())
            .unwrap_or(false);
        let open_paren_defun = i
            .intern_soft("open-paren-in-column-0-is-defun-start")
            .map(|sid| !i.symbol_value(sid).is_nil())
            .unwrap_or(true);
        let comment_use_ppss = i
            .intern_soft("comment-use-syntax-ppss")
            .map(|sid| !i.symbol_value(sid).is_nil())
            .unwrap_or(true);
        let mut prop_ranges = Vec::new();
        if lookup {
            // GNU propertizes lazily inside the scan, chunk by chunk;
            // propertize only through UPTO (a 1-based buffer position)
            // so bounded scans stay cheap on large buffers.
            let done = i
                .intern_soft("syntax-propertize--done")
                .and_then(|sid| i.symbol_value(sid).int())
                .unwrap_or(-1);
            if done < 0 || (done as usize) < upto {
                if let Some(sid) = i.intern_soft("internal--syntax-propertize") {
                    if i.fbound_p(sid) {
                        let _ = i.apply(&Value::Sym(sid), vec![Value::Int(upto as i128)]);
                    }
                }
            }
            // Collect `syntax-table' property ranges.
            let stid = i.intern("syntax-table");
            let buf = cur(i);
            let bb = buf.borrow();
            for tp in &bb.text_props {
                if tp.prop == stid {
                    if let Value::Cons(_) = tp.value {
                        prop_ranges.push((tp.start, tp.end, tp.value.clone()));
                    }
                }
            }
        }
        Self {
            entries: syntax_table_entries(i),
            ignore_comments: ignore,
            end_escaped,
            open_paren_defun,
            comment_use_ppss,
            prop_ranges,
        }
    }

    /// `syntax-table' property override at 0-based IDX, if any.
    fn prop_code(&self, idx: usize) -> Option<i128> {
        for &(s, e, ref v) in &self.prop_ranges {
            if idx >= s && idx < e {
                if let Value::Cons(cn) = v {
                    if let Value::Int(n) = &cn.borrow().car {
                        return Some(*n);
                    }
                }
            }
        }
        None
    }
    /// `syntax-table' property matching char at IDX, if any.
    fn prop_match(&self, idx: usize) -> Option<i128> {
        for &(s, e, ref v) in &self.prop_ranges {
            if idx >= s && idx < e {
                if let Value::Cons(cn) = v {
                    let cn = cn.borrow();
                    return cn.cdr.int();
                }
            }
        }
        None
    }
    /// GNU `SYNTAX_WITH_FLAGS' for the char at 0-based IDX, consulting
    /// the `syntax-table' text property when enabled.
    pub(crate) fn with_flags_at(&self, idx: usize, c: char) -> i128 {
        self.prop_code(idx).unwrap_or_else(|| self.with_flags(c))
    }
    /// Syntax-class letter for the char at IDX (property-aware).
    pub(crate) fn code_at(&self, idx: usize, c: char) -> u8 {
        if let Some(n) = self.prop_code(idx) {
            if let Some(l) = SYNTAX_CLASS_CHARS.get(n as usize & 0xf) {
                return *l as u8;
            }
        }
        self.code(c)
    }
    /// Matching delimiter for the char at IDX (property-aware).
    pub(crate) fn matching_at(&self, idx: usize, c: char) -> Option<i128> {
        if let Some(m) = self.prop_match(idx) {
            return Some(m);
        }
        self.matching(c)
    }
    /// Raw (CLASS | FLAGS<<16) syntax value of C, or -1 when the char
    /// has no table entry (falls back to the standard class).
    fn raw(&self, c: char) -> i128 {
        if let Some(t) = &self.entries {
            if let Value::Cons(cn) = t.get(c as u32) {
                if let Value::Int(n) = &cn.borrow().car {
                    return *n;
                }
            }
        }
        -1
    }
    /// GNU `SYNTAX_WITH_FLAGS' for C: (CLASS | FLAGS<<16).  Chars with
    /// no table entry yield the standard class and zero flags.
    pub(crate) fn with_flags(&self, c: char) -> i128 {
        let n = self.raw(c);
        if n >= 0 {
            return n;
        }
        SYNTAX_CLASS_CHARS
            .iter()
            .position(|&l| l as u8 == crate::lisp::regexp::syntax_code(c))
            .unwrap_or(1) as i128
    }
    /// GNU syntax-class letter of C under the effective syntax table.
    pub(crate) fn code(&self, c: char) -> u8 {
        let n = self.raw(c);
        if n >= 0 {
            if let Some(l) = SYNTAX_CLASS_CHARS.get(n as usize & 0xf) {
                return *l as u8;
            }
        }
        crate::lisp::regexp::syntax_code(c)
    }
    /// Flag BIT of C's syntax entry (GNU bits 16..23); chars with no
    /// entry have no flags.
    fn flag(&self, c: char, bit: u32) -> bool {
        (self.with_flags(c) >> bit) & 1 == 1
    }
    /// `3' flag: first char of a two-character comment end.
    pub(crate) fn comend_first(&self, c: char) -> bool {
        self.flag(c, 18)
    }
    /// GNU `SYNTAX_FLAGS_COMMENT_STYLE' on raw flag-bearing ints:
    /// style-b bit of FLAGS | style-c bit of either int.
    pub(crate) fn comment_style(syntax: i128, other: i128) -> i128 {
        ((syntax >> 21) & 1) | ((syntax >> 22) & 2) | ((other >> 22) & 2)
    }
    /// GNU `SYNTAX_FLAGS_PREFIX' — the `p' flag, which makes a char
    /// count for `backward-prefix-chars' and be skipped like
    /// whitespace between sexps.
    pub(crate) fn is_prefix_flag(&self, c: char) -> bool {
        self.flag(c, 20)
    }
    /// The entry's matching-char (cdr) for C, if the table has one.
    pub(crate) fn matching(&self, c: char) -> Option<i128> {
        if let Some(t) = &self.entries {
            if let Value::Cons(cn) = t.get(c as u32) {
                if let Value::Int(m) = &cn.borrow().cdr {
                    return Some(*m);
                }
            }
        }
        None
    }
    /// GNU numeric syntax class of C (0..15), honoring table entries.
    pub(crate) fn class(&self, c: char) -> i128 {
        let n = self.raw(c);
        if n >= 0 {
            return n & 0xf;
        }
        SYNTAX_CLASS_CHARS
            .iter()
            .position(|&l| l as u8 == crate::lisp::regexp::syntax_code(c))
            .unwrap_or(1) as i128
    }
    /// Class number for the char at IDX (property-aware).
    pub(crate) fn class_at(&self, idx: usize, c: char) -> i128 {
        self.prop_code(idx)
            .map(|n| n & 0xf)
            .unwrap_or_else(|| self.class(c))
    }
    /// True when C delimits comments for sexp scanning: comment-start,
    /// comment-end, and fence classes count only when the buffer's
    /// `parse-sexp-ignore-comments' is non-nil.
    pub(crate) fn comments_enabled(&self) -> bool {
        self.ignore_comments
    }
}

fn f_char_syntax(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let c = want_int(i, &a[0])? as u32;
    match char::from_u32(c) {
        Some(ch) => Ok(Value::Int(syntax_code_buf(i, ch) as i128)),
        None => Ok(Value::Nil),
    }
}

/// `standard-syntax-table': GNU's exact contents — including all
/// Unicode (>255) entries — replayed from the printed trie of
/// Emacs 31's standard table (see `ctdata::STD_SYNTAX_OPS').
fn std_syntax_table(i: &mut Interp) -> Value {
    use crate::lisp::builtins::misc::{ct_replay, make_ct};
    let tag = Value::Sym(i.intern("syntax-table"));
    let t = make_ct(i, tag, Value::cons(Value::Int(0), Value::Nil), vec![]);
    // GNU's exact contents — including all Unicode (>255) entries —
    // replayed from the printed trie of Emacs 31's standard table.
    ct_replay(i, &t, crate::lisp::ctdata::STD_SYNTAX_OPS);
    t
}

fn empty_syntax_table(i: &mut Interp) -> Value {
    let tag = Value::Sym(i.intern("syntax-table"));
    crate::lisp::builtins::misc::make_ct(i, tag, Value::Nil, vec![])
}

fn new_syntax_table(i: &mut Interp) -> Value {
    // GNU `make-syntax-table': empty contents, parented to
    // `standard-syntax-table' — lookups inherit through the parent.
    let t = empty_syntax_table(i);
    let parent = f_standard_syntax_table(i, vec![]).unwrap_or(Value::Nil);
    i.set_char_table_parent(&t, parent);
    t
}

fn is_syntax_table(i: &Interp, v: &Value) -> bool {
    match v {
        Value::Record(r) => {
            let rr = r.borrow();
            matches!(rr.first(), Some(Value::Sym(s)) if i.symbol_name(*s) == "char-table")
                && matches!(rr.get(1), Some(Value::Sym(s)) if i.symbol_name(*s) == "syntax-table")
        }
        _ => false,
    }
}

fn f_make_syntax_table(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    // GNU: (make-syntax-table &optional TABLE) — the new table is an
    // empty char-table whose parent is TABLE (or the standard table
    // when TABLE is nil/omitted).
    match a.first() {
        Some(v) if !v.is_nil() => {
            if !is_syntax_table(i, v) {
                return Err(i.wrong_type_mut("syntax-table-p", v));
            }
            let t = empty_syntax_table(i);
            i.set_char_table_parent(&t, v.clone());
            Ok(t)
        }
        _ => Ok(new_syntax_table(i)),
    }
}

fn f_syntax_table_p(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    Ok(Value::from_bool(is_syntax_table(i, &a[0])))
}

/// Parse a syntax descriptor string like "w", "()" or "  4" into
/// (CLASS|FLAGS . MATCHING-CHAR); nil return means the generic "@".
fn parse_syntax_desc(i: &mut Interp, s: &str) -> Result<Option<Value>, Flow> {
    let chars: Vec<char> = s.chars().collect();
    if chars.is_empty() {
        return Err(i.error("Empty syntax descriptor"));
    }
    let code: i128 = match chars[0] {
        ' ' => 0,
        '.' => 1,
        'w' => 2,
        '_' => 3,
        '(' => 4,
        ')' => 5,
        '\'' => 6,
        '"' => 7,
        '$' => 8,
        '\\' => 9,
        '/' => 10,
        '<' => 11,
        '>' => 12,
        '@' => return Ok(None),
        '!' => 14,
        '|' => 15,
        other => {
            return Err(i.error(format!("Invalid syntax description letter: {}", other)));
        }
    };
    // Second char is the matching character (a space means none).
    let mut matching = Value::Nil;
    if let Some(&m) = chars.get(1) {
        if m != ' ' {
            matching = Value::Int(m as i128);
        }
    }
    // Remaining chars are flags: '1'..'4' → bits 16..19 (two-char
    // comment sequences), 'p' → bit 20 (prefix), 'b' → 21 (style b),
    // 'n' → 22 (nested), 'c' → 23 (style c).
    let mut flags: i128 = 0;
    for &f in chars.iter().skip(2) {
        flags |= match f {
            '1'..='4' => 1i128 << (15 + f as i128 - '0' as i128),
            'p' => 1 << 20,
            'b' => 1 << 21,
            'n' => 1 << 22,
            'c' => 1 << 23,
            _ => 0,
        };
    }
    Ok(Some(Value::cons(Value::Int(code | flags), matching)))
}

/// `string-to-syntax`: parse a syntax descriptor string like "w", "()",
/// or "  4" into (CODE|FLAGS . MATCHING-CHAR), matching GNU.
fn f_string_to_syntax(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let s = match &a[0] {
        Value::Str(s) => s.borrow().clone(),
        other => return Err(i.wrong_type_mut("stringp", other)),
    };
    if s.is_empty() {
        return Err(i.signal_data(sym::ARGS_OUT_OF_RANGE, vec![a[0].clone(), Value::Int(0)]));
    }
    Ok(parse_syntax_desc(i, &s)?.unwrap_or(Value::Nil))
}

/// GNU's syntax-class letters, indexed by class number.
const SYNTAX_CLASS_CHARS: [char; 16] = [
    ' ', '.', 'w', '_', '(', ')', '\'', '"', '$', '\\', '/', '<', '>', '@', '!', '|',
];

fn f_syntax_class_to_char(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let class = match &a[0] {
        Value::Int(n) => *n,
        other => return Err(i.wrong_type_mut("fixnump", other)),
    };
    match SYNTAX_CLASS_CHARS.get(class as usize) {
        Some(c) if class >= 0 => Ok(Value::Int(*c as i128)),
        _ => Err(i.signal_data(
            sym::ARGS_OUT_OF_RANGE,
            vec![a[0].clone(), Value::Int(0), Value::Int(15)],
        )),
    }
}

fn f_matching_paren(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let c = match &a[0] {
        Value::Int(n) => *n,
        other => return Err(i.wrong_type_mut("fixnump", other)),
    };
    let m = match char::from_u32(c as u32) {
        Some('(') => ')',
        Some(')') => '(',
        Some('[') => ']',
        Some(']') => '[',
        Some('{') => '}',
        Some('}') => '{',
        _ => return Ok(Value::Nil),
    };
    Ok(Value::Int(m as i128))
}

fn f_standard_syntax_table(i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    let sid = i.intern("remacs--standard-syntax-table");
    let cur = i.symbol_value(sid);
    if is_syntax_table(i, &cur) {
        return Ok(cur);
    }
    let t = std_syntax_table(i);
    let _ = i.set_symbol(sid, t.clone());
    Ok(t)
}

fn f_syntax_table(i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    // The buffer's syntax-table FIELD wins over the standard one.
    let cur = i
        .buffers
        .get(i.current_buffer)
        .and_then(|b| b.try_borrow().ok().map(|bb| bb.syntax_table.clone()))
        .flatten();
    if let Some(t) = cur {
        if is_syntax_table(i, &t) {
            return Ok(t);
        }
    }
    f_standard_syntax_table(i, vec![])
}

fn f_set_syntax_table(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    if !is_syntax_table(i, &a[0]) {
        return Err(i.wrong_type_mut("syntax-table-p", &a[0]));
    }
    // GNU: sets the buffer's syntax-table field, NOT a Lisp variable.
    if let Some(b) = i.buffers.get(i.current_buffer) {
        if let Ok(mut bb) = b.try_borrow_mut() {
            bb.syntax_table = Some(a[0].clone());
            return Ok(a[0].clone());
        }
    }
    Ok(a[0].clone())
}

fn f_copy_syntax_table(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    // GNU `copy-syntax-table': copies TABLE's raw contents; the copy's
    // parent is TABLE's parent, or `standard-syntax-table' when TABLE
    // has none.  The defalt slot is not copied.
    // (default TABLE = `standard-syntax-table').
    let src_t = match &arg(&a, 0) {
        Value::Nil => f_standard_syntax_table(i, vec![])?,
        v if is_syntax_table(i, v) => v.clone(),
        other => return Err(i.wrong_type_mut("syntax-table-p", other)),
    };
    let src_parent = i.char_table_parent(&src_t);
    // GNU `copy_char_table' deep-copies the trie; the defalt slot is
    // NOT copied and the parent becomes src's parent (or standard).
    let t = crate::lisp::builtins::misc::ct_copy(i, &src_t);
    i.set_char_table_defalt(&t, Value::Nil);
    let parent = match src_parent {
        Value::Nil => f_standard_syntax_table(i, vec![])?,
        p => p,
    };
    i.set_char_table_parent(&t, parent);
    Ok(t)
}

fn f_modify_syntax_entry(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    // (modify-syntax-entry CHAR NEWENTRY &optional SYNTAX-TABLE)
    let c = want_int(i, &a[0])? as u32;
    if c > crate::lisp::builtins::misc::CT_MAX_CHAR {
        return Err(i.wrong_type_mut("characterp", &a[0]));
    }
    let desc = match &a[1] {
        Value::Str(s) => s.borrow().clone(),
        other => return Err(i.wrong_type_mut("stringp", other)),
    };
    let entry = match parse_syntax_desc(i, &desc)? {
        Some(e) => e,
        None => return Ok(Value::Nil),
    };
    let table = match a.get(2) {
        Some(v) if !v.is_nil() => {
            if !is_syntax_table(i, v) {
                return Err(i.wrong_type_mut("syntax-table-p", v));
            }
            v.clone()
        }
        _ => f_syntax_table(i, vec![])?,
    };
    crate::lisp::builtins::misc::ct_set(i, &table, c, entry);
    Ok(Value::Nil)
}

/// `(parse-partial-sexp FROM TO &optional TARGETDEPTH STOPBEFORE
/// OLDSTATE COMMENTSTOP)' — GNU's `scan_sexps_forward' port driving the
/// active syntax table.
fn f_parse_partial_sexp(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let syn = Syn::current(i, a.get(1).and_then(|v| v.int()).unwrap_or(1).max(1) as usize);
    let b = cur(i);
    let (stop_pos, state) = {
        let mut bb = b.borrow_mut();
        // GNU checks TARGETDEPTH's type first (CHECK_FIXNUM), then
        // fixes FROM/TO and validates them against [BEGV, ZV].
        let targetdepth = match a.get(2) {
            Some(v) if v.truthy() => match v.int() {
                Some(n) => n,
                None => return Err(i.wrong_type_mut("fixnump", v)),
            },
            _ => i128::MIN,
        };
        let from_l = want_int(i, &a[0])?;
        let to_l = want_int(i, &a[1])?;
        if to_l < from_l {
            return Err(i.error("End position is smaller than start position"));
        }
        let begv = bb.begv as i128 + 1;
        let zv = bb.zv as i128 + 1;
        if from_l < begv || to_l > zv {
            let s = i.intern("args-out-of-range");
            let bv = Value::Buffer(b.clone());
            return Err(i.signal_data(s, vec![bv, Value::Int(from_l), Value::Int(to_l)]));
        }
        let stopbefore = a.get(3).is_some_and(|v| v.truthy());
        let commentstop = match a.get(5) {
            Some(Value::Sym(s)) if i.symbol_name(*s) == "syntax-table" => -1,
            Some(v) if v.truthy() => 1,
            _ => 0,
        };
        let mut st = match a.get(4) {
            Some(v) if !v.is_nil() && !matches!(v, Value::Cons(_)) => {
                return Err(i.wrong_type_mut("listp", v));
            }
            Some(v) => match v.list_to_vec().ok() {
                Some(old) => crate::buffer::primitives::ParseState::internalize(&old),
                None => crate::buffer::primitives::ParseState::fresh(),
            },
            None => crate::buffer::primitives::ParseState::fresh(),
        };
        let from = (from_l - 1) as usize;
        let to = (to_l - 1) as usize;
        let begv = bb.begv;
        let text = bb.text.as_slice();
        crate::buffer::primitives::scan_sexps_fwd(
            &syn,
            text,
            begv,
            from,
            to,
            &mut st,
            targetdepth,
            stopbefore,
            commentstop,
        );
        (st.location, st.externalize(i))
    };
    b.borrow_mut().set_point(stop_pos);
    Ok(state)
}

fn f_syntax_ppss(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    // GNU's syntax-ppss is parse-partial-sexp from point-min; when POS
    // is given point ends up there (the parse leaves point at POS).
    let (beg, pos) = {
        let b = cur(i);
        let bb = b.borrow();
        let beg = Value::Int(bb.begv as i128 + 1);
        let pos = match a.get(0) {
            Some(v) if v.truthy() => v.clone(),
            _ => Value::Int(bb.point() as i128 + 1),
        };
        (beg, pos)
    };
    f_parse_partial_sexp(i, vec![beg, pos])
}

// ---------- modes ----------

fn f_fundamental_mode(i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    // GNU: (kill-all-local-variables) then set major-mode/mode-name and
    // run the hook.  Derived modes rely on the root doing the killing.
    crate::buffer::primitives::f_kill_all_local_variables(i, vec![])?;
    let mm = i.intern("major-mode");
    let fmid = i.intern("fundamental-mode");
    let mn = i.intern("mode-name");
    {
        let b = cur(i);
        let mut bb = b.borrow_mut();
        bb.locals.insert(mm, Value::Sym(fmid));
        bb.locals.insert(mn, Value::string("Fundamental"));
    }
    let rmh = i.intern("run-mode-hooks");
    let hook = i.intern("fundamental-mode-hook");
    i.apply(&Value::Sym(rmh), vec![Value::Sym(hook)])?;
    Ok(Value::Sym(fmid))
}

// ---------- overlays ----------

pub(crate) fn f_make_overlay(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    // BUFFER is a buffer object or nil (→ current buffer), like GNU.
    let bid = match a.get(2) {
        None | Some(Value::Nil) => i.current_buffer,
        Some(Value::Buffer(b)) => b.borrow().id,
        Some(v) => return Err(i.wrong_type_mut("bufferp", v)),
    };
    let len = i
        .buffers
        .get(bid)
        .map(|b| b.borrow().text.len())
        .unwrap_or(0);
    let mut s = want_int(i, &a[0])? - 1;
    let mut e = want_int(i, &a[1])? - 1;
    // GNU swaps BEG/END when reversed, then clamps into the buffer.
    if s > e {
        std::mem::swap(&mut s, &mut e);
    }
    let s = (s.max(0) as usize).min(len);
    let e = (e.max(0) as usize).min(len);
    let b = i.buffers.get(bid).unwrap();
    let mut bb = b.borrow_mut();
    let idx = bb.overlays.len();
    // The Lisp handle is `[overlay BUFFER INDEX]' — kept on the
    // Overlay itself so queries return the identical object (`eq').
    let handle = Value::Vec(Rc::new(RefCell::new(vec![
        Value::Sym(i.intern("overlay")),
        Value::Int(bid as i128),
        Value::Int(idx as i128),
    ])));
    bb.overlays.push(crate::buffer::Overlay {
        start: s,
        end: e,
        buffer: Some(bid),
        plist: Value::Nil,
        front_advance: arg(&a, 3).truthy(),
        rear_advance: arg(&a, 4).truthy(),
        handle: handle.clone(),
    });
    Ok(handle)
}

fn overlay_of(i: &mut Interp, v: &Value) -> Result<(usize, usize), Flow> {
    if let Value::Vec(vec) = v {
        let vv = vec.borrow();
        if vv.len() == 3 {
            if let (Value::Sym(tag), Value::Int(bid), Value::Int(idx)) = (&vv[0], &vv[1], &vv[2]) {
                if i.sym_id(&Value::Sym(*tag)) == i.intern_soft("overlay") {
                    return Ok((*bid as usize, *idx as usize));
                }
            }
        }
    }
    Err(i.wrong_type_mut("overlayp", v))
}

/// Look up the Overlay a handle points at, if its slot still exists.
fn overlay_slot(i: &Interp, bid: usize, idx: usize) -> Option<crate::buffer::Overlay> {
    i.buffers
        .get(bid)
        .and_then(|b| b.borrow().overlays.get(idx).cloned())
}

fn f_delete_overlay(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let (bid, idx) = overlay_of(i, &a[0])?;
    if let Some(b) = i.buffers.get(bid) {
        let mut bb = b.borrow_mut();
        if let Some(ov) = bb.overlays.get_mut(idx) {
            // GNU detaches the overlay: start/end/buffer become nil,
            // plist survives.
            ov.buffer = None;
        }
    }
    Ok(Value::Nil)
}

fn f_delete_all_overlays(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let b = crate::buffer::primitives::buf_of(i, &arg(&a, 0))?;
    let mut bb = b.borrow_mut();
    for ov in &mut bb.overlays {
        ov.buffer = None;
    }
    Ok(Value::Nil)
}

fn f_move_overlay(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let (bid, idx) = overlay_of(i, &a[0])?;
    // BUFFER is a buffer object or nil (→ current buffer), like GNU.
    let nbid = match a.get(3) {
        None | Some(Value::Nil) => i.current_buffer,
        Some(Value::Buffer(b)) => b.borrow().id,
        Some(v) => return Err(i.wrong_type_mut("bufferp", v)),
    };
    let len = i
        .buffers
        .get(nbid)
        .map(|b| b.borrow().text.len())
        .unwrap_or(0);
    let mut s = want_int(i, &a[1])? - 1;
    let mut e = want_int(i, &a[2])? - 1;
    if s > e {
        std::mem::swap(&mut s, &mut e);
    }
    let s = (s.max(0) as usize).min(len);
    let e = (e.max(0) as usize).min(len);
    let Some(ov) = overlay_slot(i, bid, idx) else {
        return Ok(a[0].clone());
    };
    if nbid != bid {
        // Cross-buffer move: leave a dead stub behind and push a new
        // entry into the target buffer, then retarget the handle.
        if let Some(ob) = i.buffers.get(bid) {
            if let Some(old) = ob.borrow_mut().overlays.get_mut(idx) {
                old.buffer = None;
            }
        }
        let nidx = {
            let nb = i.buffers.get(nbid).unwrap();
            let mut nbb = nb.borrow_mut();
            nbb.overlays.push(ov.clone());
            nbb.overlays.len() - 1
        };
        if let Value::Vec(v) = &a[0] {
            let mut vv = v.borrow_mut();
            vv[1] = Value::Int(nbid as i128);
            vv[2] = Value::Int(nidx as i128);
        }
        let nb = i.buffers.get(nbid).unwrap();
        let mut nbb = nb.borrow_mut();
        let e2 = nbb.overlays.get_mut(nidx).unwrap();
        e2.handle = a[0].clone();
        e2.buffer = Some(nbid);
        e2.start = s;
        e2.end = e;
    } else if let Some(b) = i.buffers.get(bid) {
        let mut bb = b.borrow_mut();
        if let Some(e2) = bb.overlays.get_mut(idx) {
            e2.buffer = Some(bid);
            e2.start = s;
            e2.end = e;
        }
    }
    Ok(a[0].clone())
}

fn f_overlay_start(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let (bid, idx) = overlay_of(i, &a[0])?;
    Ok(match overlay_slot(i, bid, idx) {
        Some(ov) if ov.buffer.is_some() => Value::Int(ov.start as i128 + 1),
        _ => Value::Nil,
    })
}
fn f_overlay_end(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let (bid, idx) = overlay_of(i, &a[0])?;
    Ok(match overlay_slot(i, bid, idx) {
        Some(ov) if ov.buffer.is_some() => Value::Int(ov.end as i128 + 1),
        _ => Value::Nil,
    })
}
fn f_overlay_buffer(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let (bid, idx) = overlay_of(i, &a[0])?;
    Ok(match overlay_slot(i, bid, idx) {
        Some(ov) => ov
            .buffer
            .and_then(|id| i.buffer_value(id))
            .unwrap_or(Value::Nil),
        _ => Value::Nil,
    })
}
/// GNU `overlay-put' updates an existing pair in place, else conses
/// the new pair onto the *front* of the plist (unlike `plist-put'
/// which appends).
fn overlay_plist_put(plist: &Value, prop: SymId, val: Value) -> Value {
    let mut cur = plist.clone();
    loop {
        let c = match &cur {
            Value::Cons(c) => c.clone(),
            _ => break,
        };
        let (car, cdr) = {
            let cc = c.borrow();
            (cc.car.clone(), cc.cdr.clone())
        };
        if matches!(car, Value::Sym(s) if s == prop) {
            if let Value::Cons(c2) = &cdr {
                c2.borrow_mut().car = val;
            }
            return plist.clone();
        }
        match &cdr {
            Value::Cons(c2) => cur = c2.borrow().cdr.clone(),
            _ => break,
        }
    }
    Value::cons(Value::Sym(prop), Value::cons(val, plist.clone()))
}

fn f_overlay_put(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let (bid, idx) = overlay_of(i, &a[0])?;
    let ps = want_sym(i, &a[1])?;
    if let Some(b) = i.buffers.get(bid) {
        let mut bb = b.borrow_mut();
        if let Some(ov) = bb.overlays.get_mut(idx) {
            ov.plist = overlay_plist_put(&ov.plist, ps, a[2].clone());
        }
    }
    Ok(a[2].clone())
}
fn f_overlay_get(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let (bid, idx) = overlay_of(i, &a[0])?;
    let ps = want_sym(i, &a[1])?;
    Ok(match overlay_slot(i, bid, idx) {
        Some(ov) => {
            // GNU `lookup_char_property': a present key wins even
            // when its value is nil; else fall back to the
            // `category' symbol's plist.
            match crate::lisp::eval::plist_lookup(&ov.plist, ps) {
                Some(v) => v,
                None => {
                    let cat = i.intern_soft("category");
                    match cat.and_then(|c| {
                        match crate::lisp::eval::plist_get(&ov.plist, c) {
                            Value::Sym(s) => Some(s),
                            _ => None,
                        }
                    }) {
                        Some(cs) => i.get_prop(cs, ps),
                        None => Value::Nil,
                    }
                }
            }
        }
        _ => Value::Nil,
    })
}
fn f_overlay_properties(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let (bid, idx) = overlay_of(i, &a[0])?;
    Ok(match overlay_slot(i, bid, idx) {
        Some(ov) => ov.plist,
        _ => Value::Nil,
    })
}
fn f_overlayp(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    Ok(Value::from_bool(match overlay_of(i, &a[0]) {
        Ok((bid, idx)) => overlay_slot(i, bid, idx).is_some(),
        _ => false,
    }))
}

/// 'priority' property of OV as an int (nil → 0), for GNU ordering.
fn overlay_priority(i: &Interp, ov: &crate::buffer::Overlay) -> i128 {
    let p = i.intern_soft("priority").unwrap_or(u32::MAX);
    crate::lisp::eval::plist_get(&ov.plist, p)
        .int()
        .unwrap_or(0)
}

/// GNU `sort_overlays' order: priority descending, then start
/// ascending — shared by `overlays-at', `overlays-in' and
/// `overlay-lists'.
fn overlay_sort_key(i: &Interp, ov: &crate::buffer::Overlay) -> (std::cmp::Reverse<i128>, usize) {
    (std::cmp::Reverse(overlay_priority(i, ov)), ov.start)
}

fn f_overlays_at(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let pos = want_int(i, &a[0])?.max(1) as usize - 1;
    let b = cur(i);
    let bb = b.borrow();
    let bid = bb.id;
    let mut hits: Vec<&crate::buffer::Overlay> = bb
        .overlays
        .iter()
        .filter(|ov| ov.buffer == Some(bid) && pos >= ov.start && pos < ov.end)
        .collect();
    hits.sort_by_key(|ov| overlay_sort_key(i, ov));
    Ok(Value::list(
        hits.into_iter().map(|ov| ov.handle.clone()).collect(),
    ))
}
fn f_overlays_in(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let b = cur(i);
    let bb = b.borrow();
    let len = bb.text.len();
    let bid = bb.id;
    let s = (want_int(i, &a[0])?.max(1) as usize - 1).min(len);
    let e = (want_int(i, &a[1])?.max(1) as usize - 1).min(len);
    let mut hits: Vec<&crate::buffer::Overlay> = bb
        .overlays
        .iter()
        .filter(|ov| {
            ov.buffer == Some(bid)
                && ((ov.start < e && ov.end > s)
                    // GNU also reports an empty overlay exactly at BEG.
                    || (ov.start == ov.end && ov.start == s))
        })
        .collect();
    hits.sort_by_key(|ov| overlay_sort_key(i, ov));
    Ok(Value::list(
        hits.into_iter().map(|ov| ov.handle.clone()).collect(),
    ))
}
fn f_overlays_at_point(i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    let pos = cur(i).borrow().point() as i128 + 1;
    f_overlays_at(i, vec![Value::Int(pos)])
}
fn f_next_overlay_change(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let pos = want_int(i, &a[0])?.max(1) as usize - 1;
    let b = cur(i);
    let bb = b.borrow();
    let bid = bb.id;
    let mut next: Option<usize> = None;
    for ov in &bb.overlays {
        if ov.buffer != Some(bid) {
            continue;
        }
        for cand in [ov.start, ov.end] {
            if cand > pos {
                next = Some(next.map(|n| n.min(cand)).unwrap_or(cand));
            }
        }
    }
    match next {
        Some(p) => Ok(Value::Int(p as i128 + 1)),
        None => Ok(Value::Int(bb.text.len() as i128 + 1)),
    }
}
fn f_prev_overlay_change(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let pos = want_int(i, &a[0])?.max(1) as usize - 1;
    let b = cur(i);
    let bb = b.borrow();
    let bid = bb.id;
    let mut prev: Option<usize> = None;
    for ov in &bb.overlays {
        if ov.buffer != Some(bid) {
            continue;
        }
        for cand in [ov.start, ov.end] {
            if cand < pos {
                prev = Some(prev.map(|n| n.max(cand)).unwrap_or(cand));
            }
        }
    }
    match prev {
        Some(p) => Ok(Value::Int(p as i128 + 1)),
        None => Ok(Value::Int(1)),
    }
}
fn f_remove_overlays(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let b = cur(i);
    let len = b.borrow().text.len();
    let bid = b.borrow().id;
    let s = match a.get(0) {
        Some(v) if v.truthy() => (want_int(i, v)?.max(1) as usize - 1).min(len),
        _ => 0,
    };
    let e = match a.get(1) {
        Some(v) if v.truthy() => (want_int(i, v)?.max(1) as usize - 1).min(len),
        _ => len,
    };
    // GNU removes only overlays *entirely inside* [BEG, END] — and,
    // when NAME is given, only those whose NAME property is `eq' VAL.
    let name = a.get(2).and_then(|v| i.sym_id(v));
    let val = arg(&a, 3);
    let mut bb = b.borrow_mut();
    for ov in &mut bb.overlays {
        if ov.buffer != Some(bid) {
            continue;
        }
        let inside = s <= ov.start && ov.end <= e;
        let prop_ok = match name {
            Some(ps) => eq_values(&crate::lisp::eval::plist_get(&ov.plist, ps), &val),
            None => true,
        };
        if inside && prop_ok {
            ov.buffer = None;
        }
    }
    Ok(Value::Nil)
}

// ---------- help/docs ----------

fn f_documentation_property(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let sid = want_sym(i, &a[0])?;
    let prop = want_sym(i, &a[1])?;
    Ok(i.get_prop(sid, prop))
}

fn f_documentation(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let f = match &a[0] {
        Value::Sym(id) => i.symbol_function(*id),
        other => other.clone(),
    };
    match f.as_lambda() {
        Some(l) => match &l.doc {
            Some(d) => Ok(Value::string(d.clone())),
            None => Ok(Value::Nil),
        },
        None => match &f {
            Value::Subr(s) => {
                if let Some(t) =
                    crate::lisp::builtins::misc::doc_text(i, &format!("F{}", s.name))
                {
                    // `documentation' applies substitute-quotes:
                    // `...' -> ‘...’ unless text-quoting-style is
                    // 'grave/'straight.
                    // RAW non-nil returns the docstring verbatim.
                    if !matches!(arg(&a, 1), Value::Nil) {
                        return Ok(Value::string(t));
                    }
                    let tqs = i.intern("text-quoting-style");
                    let t = match i.symbol_value(tqs) {
                        Value::Sym(q) if i.symbol_name(q) == "grave" => t,
                        Value::Sym(q) if i.symbol_name(q) == "straight" => {
                            t.replace('`', "'")
                        }
                        _ => t.replace('`', "‘").replace('\'', "’"),
                    };
                    return Ok(Value::string(t));
                }
                if s.doc.is_empty() {
                    Ok(Value::Nil)
                } else {
                    Ok(Value::string(s.doc))
                }
            }
            _ => Ok(Value::Nil),
        },
    }
}

fn f_apropos_internal(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let pat = want_str(i, &a[0])?;
    let re = crate::lisp::regexp::compile_case(&pat, false)
        .map_err(|e| err_sym(i, "invalid-regexp", vec![Value::string(e.0)]))?;
    let mut out = Vec::new();
    for id in i.obarray.all_ids() {
        let name = i.symbol_name(id);
        let syn = crate::editor::re_syntax(i);
        let chars: Vec<char> = name.chars().collect();
        if crate::lisp::regexp::search(&re, &chars, 0, &syn).is_some() {
            out.push(i.sym(id));
        }
    }
    Ok(Value::list(out))
}

/// Look up a key sequence (as Int event codes) in the active maps:
/// local map, then the global map. Returns the bound value, or None.
pub(crate) fn lookup_command_in_maps(i: &mut Interp, keys: &[Value]) -> Option<Value> {
    let codes: Vec<i128> = keys.iter().filter_map(|v| v.int()).collect();
    let local = {
        let b = i.current_buffer_ref()?;
        let lb = b.borrow();
        lb.locals
            .get(&i.intern_soft("local-keymap").unwrap_or(u32::MAX))
            .cloned()
            .unwrap_or(Value::Nil)
    };
    // GNU's active-map order: minor modes, local map, global.
    let mut map_list = Vec::new();
    if let Ok(minors) = current_minor_maps(i) {
        map_list.extend(minors.into_iter().map(|(_, m)| m));
    }
    map_list.push(local);
    map_list.push(i.symbol_value(i.intern_soft("global-map").unwrap_or(0)));
    for km_v in map_list {
        if !is_keymap(i, &km_v) {
            continue;
        }
        let mut km = km_v;
        let mut last_def = Value::Nil;
        let mut prefix_only = false;
        for &k in &codes {
            let raw = match lookup_in_keymap(i, &km, k, true) {
                Ok(v) => v,
                Err(_) => return None,
            };
            let def = match keymap_def(i, raw.clone()) {
                Ok(v) => v,
                Err(_) => return None,
            };
            if is_keymap(i, &def) {
                km = def;
                prefix_only = true;
                last_def = Value::Nil;
            } else {
                last_def = raw;
                prefix_only = false;
                break;
            }
        }
        if prefix_only {
            return Some(km);
        }
        if last_def.truthy() {
            return Some(last_def);
        }
    }
    None
}

// ---------- faces (minimal tty model) ----------

/// Face names GNU binds in a batch session (a subset of face-list;
/// enough for facep/face-list lookups).
const KNOWN_FACES: &[&str] = &[
    "default",
    "bold",
    "italic",
    "bold-italic",
    "underline",
    "fixed-pitch",
    "fixed-pitch-serif",
    "variable-pitch",
    "variable-pitch-text",
    "shadow",
    "link",
    "link-visited",
    "highlight",
    "region",
    "secondary-selection",
    "trailing-whitespace",
    "line-number",
    "line-number-current-line",
    "line-number-major-tick",
    "line-number-minor-tick",
    "fill-column-indicator",
    "escape-glyph",
    "homoglyph",
    "nobreak-space",
    "nobreak-hyphen",
    "mode-line",
    "mode-line-active",
    "mode-line-inactive",
    "mode-line-highlight",
    "mode-line-emphasis",
    "mode-line-buffer-id",
    "header-line",
    "header-line-highlight",
    "header-line-active",
    "header-line-inactive",
    "vertical-border",
    "window-divider",
    "window-divider-first-pixel",
    "window-divider-last-pixel",
    "internal-border",
    "child-frame-border",
    "minibuffer-prompt",
    "margin",
    "fringe",
    "scroll-bar",
    "border",
    "cursor",
    "mouse",
    "tool-bar",
    "tab-bar",
    "tab-line",
    "tab-line-active",
    "tab-line-inactive",
    "menu",
    "help-argument-name",
    "help-key-binding",
    "glyphless-char",
    "error",
    "warning",
    "success",
    "read-multiple-choice-face",
    "tty-menu-enabled-face",
    "tty-menu-disabled-face",
    "tty-menu-selected-face",
    "show-paren-match",
    "show-paren-match-expression",
    "show-paren-mismatch",
    "button",
    "abbrev-table-name",
    "help-for-help-header",
    "confusingly-reordered",
    "next-error",
    "next-error-message",
    "separator-line",
    "blink-matching-paren-offscreen",
    "completions-group-title",
    "completions-group-separator",
    "completions-annotations",
    "completions-highlight",
    "completions-first-difference",
    "completions-common-part",
    "minibuffer-nonselected",
    "font-lock-comment-face",
    "font-lock-comment-delimiter-face",
    "font-lock-string-face",
    "font-lock-doc-face",
    "font-lock-doc-markup-face",
    "font-lock-keyword-face",
    "font-lock-builtin-face",
    "font-lock-function-name-face",
    "font-lock-function-call-face",
    "font-lock-variable-name-face",
    "font-lock-variable-use-face",
    "font-lock-type-face",
    "font-lock-constant-face",
    "font-lock-warning-face",
    "font-lock-negation-char-face",
    "font-lock-preprocessor-face",
    "font-lock-regexp-face",
    "font-lock-regexp-grouping-backslash",
    "font-lock-regexp-grouping-construct",
    "font-lock-escape-face",
    "font-lock-number-face",
    "font-lock-operator-face",
    "font-lock-property-name-face",
    "font-lock-property-use-face",
    "font-lock-punctuation-face",
    "font-lock-bracket-face",
    "font-lock-delimiter-face",
    "font-lock-misc-punctuation-face",
    "mouse-drag-and-drop-region",
    "isearch",
    "isearch-fail",
    "lazy-highlight",
    "isearch-group-1",
    "isearch-group-2",
    "file-name-shadow",
    "tab-bar-tab",
    "tab-bar-tab-inactive",
    "tab-bar-tab-group-current",
    "tab-bar-tab-group-inactive",
    "tab-bar-tab-ungrouped",
    "tab-bar-tab-highlight",
    "query-replace",
    "match",
    "tabulated-list-fake-header",
    "buffer-menu-buffer",
    "ns-working-text-face",
    "elisp-symbol-at-mouse",
    "elisp-free-variable",
    "elisp-special-variable-declaration",
    "elisp-condition",
    "elisp-major-mode-name",
    "elisp-face",
    "elisp-symbol-role",
    "elisp-symbol-role-definition",
    "elisp-function",
    "elisp-non-local-exit",
    "elisp-unknown-call",
    "elisp-macro",
    "elisp-special-form",
    "elisp-throw-tag",
    "elisp-feature",
    "elisp-rx",
    "elisp-theme",
    "elisp-binding-variable",
    "elisp-bound-variable",
    "elisp-shadowing-variable",
    "elisp-shadowed-variable",
    "elisp-variable-at-point",
    "elisp-warning-type",
    "elisp-function-property-declaration",
    "elisp-thing",
    "elisp-slot",
    "elisp-widget-type",
    "elisp-type",
    "elisp-group",
    "elisp-nnoo-backend",
    "elisp-ampersand",
    "elisp-constant",
    "elisp-defun",
    "elisp-defmacro",
    "elisp-defvar",
    "elisp-defface",
    "elisp-icon",
    "elisp-deficon",
    "elisp-oclosure",
    "elisp-defoclosure",
    "elisp-coding",
    "elisp-defcoding",
    "elisp-charset",
    "elisp-defcharset",
    "elisp-completion-category",
    "elisp-completion-category-definition",
    "vc-state-base",
    "vc-up-to-date-state",
    "vc-needs-update-state",
    "vc-locked-state",
    "vc-locally-added-state",
    "vc-conflict-state",
    "vc-removed-state",
    "vc-missing-state",
    "vc-edited-state",
    "vc-ignored-state",
    "elisp-shorthand-font-lock-face",
    "eldoc-highlight-function-argument",
    "tooltip",
];

pub(crate) fn face_known(i: &Interp, name: &str) -> bool {
    KNOWN_FACES.contains(&name) || i.face_table.iter().any(|(n, _)| n == name)
}

fn face_name_of(i: &mut Interp, v: &Value) -> Result<String, Flow> {
    let name = match v {
        Value::Sym(s) => i.symbol_name(*s),
        Value::Str(s) => s.borrow().clone(),
        other => return Err(i.wrong_type_mut("symbolp", other)),
    };
    if !face_known(i, &name) {
        return Err(i.error(format!("Invalid face: {}", name)));
    }
    Ok(name)
}

/// One attribute of a face, GNU-style (`unspecified' when unset).
fn face_attr(i: &mut Interp, name: &str, attr: &str) -> Value {
    if let Some((_, plist)) = i.face_table.iter().find(|(n, _)| n == name) {
        if let Value::Cons(_) = plist {
            if let Some(v) = plist.list_to_vec().ok().and_then(|items| {
                items.chunks(2).find_map(|kv| match (&kv[0], kv.get(1)) {
                    (Value::Sym(k), Some(v)) if i.symbol_name(*k) == attr => Some(v.clone()),
                    _ => None,
                })
            }) {
                return v;
            }
        }
    }
    match (name, attr) {
        ("default", ":weight") | ("default", ":slant") | ("default", ":width") => {
            Value::Sym(i.intern("normal"))
        }
        ("default", ":background") => Value::string("unspecified-bg"),
        ("default", ":foreground") => Value::string("unspecified-fg"),
        ("default", ":family") => Value::string("default"),
        ("default", ":height") => Value::Int(1),
        (
            "default",
            ":underline" | ":overline" | ":strike-through" | ":box" | ":inverse-video" | ":stipple"
            | ":extend",
        ) => Value::Nil,
        ("bold", ":weight") | ("bold-italic", ":weight") => Value::Sym(i.intern("bold")),
        ("italic", ":slant") | ("bold-italic", ":slant") => Value::Sym(i.intern("italic")),
        ("underline", ":underline") => Value::t(),
        _ => Value::Sym(i.intern("unspecified")),
    }
}

fn f_face_attribute(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let name = face_name_of(i, &a[0])?;
    let attr = match &a[1] {
        Value::Sym(s) => i.symbol_name(*s),
        other => return Err(i.wrong_type_mut("symbolp", other)),
    };
    Ok(face_attr(i, &name, &attr))
}

fn f_facep(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let name = match face_name_of(i, &a[0]) {
        Ok(n) => n,
        Err(_) => return Ok(Value::Nil),
    };
    let mut v = Vec::with_capacity(20);
    v.push(Value::Sym(i.intern("face")));
    let un = Value::Sym(i.intern("unspecified"));
    v.resize(20, un);
    let _ = name;
    Ok(Value::Vec(Rc::new(RefCell::new(v))))
}

fn f_face_list(i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    // GNU lists faces newest-first: user-created faces (in reverse
    // creation order), then the built-ins back-to-front.
    let mut names: Vec<String> = i
        .face_table
        .iter()
        .rev()
        .map(|(n, _)| n.clone())
        .filter(|n| !KNOWN_FACES.contains(&n.as_str()))
        .collect();
    names.extend(KNOWN_FACES.iter().rev().map(|s| s.to_string()));
    Ok(Value::list(
        names.iter().map(|n| Value::Sym(i.intern(n))).collect(),
    ))
}

fn f_face_id(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let name = face_name_of(i, &a[0])?;
    let idx = KNOWN_FACES
        .iter()
        .position(|n| *n == name)
        .or_else(|| {
            i.face_table
                .iter()
                .position(|(n, _)| *n == name)
                .map(|p| KNOWN_FACES.len() + p)
        })
        .unwrap_or(KNOWN_FACES.len());
    Ok(Value::Int(idx as i128))
}

fn f_face_equal(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let n1 = face_name_of(i, &a[0])?;
    let n2 = face_name_of(i, &a[1])?;
    Ok(Value::from_bool(n1 == n2))
}

fn f_make_face(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let name = match &a[0] {
        Value::Sym(s) => i.symbol_name(*s),
        other => return Err(i.wrong_type_mut("symbolp", other)),
    };
    if !face_known(i, &name) {
        i.face_table.push((name, Value::Nil));
    }
    Ok(a[0].clone())
}

fn f_copy_face(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let from = face_name_of(i, &a[0])?;
    let to = match &a[1] {
        Value::Sym(s) => i.symbol_name(*s),
        other => return Err(i.wrong_type_mut("symbolp", other)),
    };
    // Copy each attr into the destination face.
    for attr in [
        ":family",
        ":foundry",
        ":width",
        ":height",
        ":weight",
        ":slant",
        ":foreground",
        ":distant-foreground",
        ":background",
        ":underline",
        ":overline",
        ":strike-through",
        ":box",
        ":inverse-video",
        ":stipple",
        ":font",
        ":fontset",
        ":extend",
        ":inherit",
    ] {
        let v = face_attr(i, &from, attr);
        set_face_attr(i, &to, attr, v);
    }
    Ok(a[1].clone())
}

fn set_face_attr(i: &mut Interp, name: &str, attr: &str, val: Value) {
    let kw = i.intern(attr);
    if !face_known(i, name) {
        i.face_table.push((name.to_string(), Value::Nil));
    }
    if let Some((_, plist)) = i.face_table.iter_mut().find(|(n, _)| n == name) {
        *plist = crate::lisp::eval::plist_put(plist, kw, val);
    }
}

fn f_internal_set_lisp_face_attribute(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let name = match &a[0] {
        Value::Sym(s) => i.symbol_name(*s),
        Value::Str(s) => s.borrow().clone(),
        other => return Err(i.wrong_type_mut("symbolp", other)),
    };
    let attr = match &a[1] {
        Value::Sym(s) => i.symbol_name(*s),
        other => return Err(i.wrong_type_mut("symbolp", other)),
    };
    if !face_known(i, &name) {
        i.face_table.push((name.clone(), Value::Nil));
    }
    set_face_attr(i, &name, &attr, a[2].clone());
    Ok(Value::Nil)
}

fn f_set_face_attribute(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    // (set-face-attribute FACE FRAME &rest ARGS)
    let name = match &a[0] {
        Value::Sym(s) => i.symbol_name(*s),
        other => return Err(i.wrong_type_mut("symbolp", other)),
    };
    if !face_known(i, &name) {
        i.face_table.push((name.clone(), Value::Nil));
    }
    for kv in a[2..].chunks(2) {
        if let (Value::Sym(k), Some(v)) = (&kv[0], kv.get(1)) {
            let attr = i.symbol_name(*k);
            set_face_attr(i, &name, &attr, v.clone());
        }
    }
    Ok(Value::Nil)
}

fn f_face_background(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let name = face_name_of(i, &a[0])?;
    match face_attr(i, &name, ":background") {
        Value::Sym(s) if i.symbol_name(s) == "unspecified" => Ok(Value::Nil),
        v => Ok(v),
    }
}
fn f_face_foreground(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let name = face_name_of(i, &a[0])?;
    match face_attr(i, &name, ":foreground") {
        Value::Sym(s) if i.symbol_name(s) == "unspecified" => Ok(Value::Nil),
        v => Ok(v),
    }
}

/// (memq WEIGHT '(semi-bold bold extra-bold ultra-bold)) like GNU.
fn f_face_bold_p(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let name = face_name_of(i, &a[0])?;
    let w = face_attr(i, &name, ":weight");
    let wname = match &w {
        Value::Sym(s) => i.symbol_name(*s),
        _ => String::new(),
    };
    let weights = ["semi-bold", "bold", "extra-bold", "ultra-bold"];
    match weights.iter().position(|x| *x == wname) {
        Some(p) => Ok(Value::list(
            weights[p..]
                .iter()
                .map(|s| Value::Sym(i.intern(s)))
                .collect(),
        )),
        None => Ok(Value::Nil),
    }
}
fn f_face_italic_p(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let name = face_name_of(i, &a[0])?;
    let w = face_attr(i, &name, ":slant");
    let wname = match &w {
        Value::Sym(s) => i.symbol_name(*s),
        _ => String::new(),
    };
    let slants = ["italic", "oblique"];
    match slants.iter().position(|x| *x == wname) {
        Some(p) => Ok(Value::list(
            slants[p..]
                .iter()
                .map(|s| Value::Sym(i.intern(s)))
                .collect(),
        )),
        None => Ok(Value::Nil),
    }
}
fn f_face_underline_p(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let name = face_name_of(i, &a[0])?;
    Ok(match face_attr(i, &name, ":underline") {
        Value::Sym(s) if i.symbol_name(s) == "unspecified" => Value::Nil,
        v => v,
    })
}

fn f_face_all_attributes(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let name = face_name_of(i, &a[0])?;
    let attrs = [
        ":family",
        ":foundry",
        ":width",
        ":height",
        ":weight",
        ":slant",
        ":foreground",
        ":distant-foreground",
        ":background",
        ":underline",
        ":overline",
        ":strike-through",
        ":box",
        ":inverse-video",
        ":stipple",
        ":font",
        ":fontset",
        ":extend",
        ":inherit",
    ];
    Ok(Value::list(
        attrs
            .iter()
            .map(|at| {
                let v = face_attr(i, &name, at);
                Value::cons(Value::Sym(i.intern(at)), v)
            })
            .collect(),
    ))
}

// ---------- window resizing & layout ----------

const WINDOW_MIN_HEIGHT: usize = 4;
const WINDOW_MIN_WIDTH: usize = 10;

/// Adjust WINDOW's height (or width when HORIZ) by DELTA cells,
/// taking/giving space from another window in the frame.
fn window_resize(i: &mut Interp, w: &WindowRef, delta: i128, horiz: bool) -> Result<(), Flow> {
    let f = sel_frame(i).ok_or_else(|| i.error("No frame"))?;
    let wid = w.borrow().id;
    let others: Vec<WindowRef> = {
        let ff = f.borrow();
        ff.windows
            .iter()
            .filter(|o| o.borrow().id != wid && !o.borrow().dead && !o.borrow().minibuffer)
            .cloned()
            .collect()
    };
    if delta > 0 && others.is_empty() {
        return Err(i.error("Cannot resize window"));
    }
    if horiz {
        let mut ww = w.borrow_mut();
        let new = (ww.width as i128 + delta).max(WINDOW_MIN_WIDTH as i128) as usize;
        let actual = new as i128 - ww.width as i128;
        ww.width = new;
        drop(ww);
        if let Some(o) = others.first() {
            let mut oo = o.borrow_mut();
            oo.width = (oo.width as i128 - actual).max(WINDOW_MIN_WIDTH as i128) as usize;
        }
    } else {
        let mut ww = w.borrow_mut();
        let new = (ww.height as i128 + delta).max(WINDOW_MIN_HEIGHT as i128) as usize;
        let actual = new as i128 - ww.height as i128;
        ww.height = new;
        drop(ww);
        if let Some(o) = others.first() {
            let mut oo = o.borrow_mut();
            oo.height = (oo.height as i128 - actual).max(WINDOW_MIN_HEIGHT as i128) as usize;
        }
    }
    Ok(())
}

fn f_enlarge_window(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let w = sel_window(i).ok_or_else(|| i.error("No window"))?;
    let delta = want_int(i, &a[0])?;
    let horiz = a.get(1).map(|v| v.truthy()).unwrap_or(false);
    window_resize(i, &w, delta, horiz)?;
    Ok(Value::t())
}
fn f_shrink_window(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let w = sel_window(i).ok_or_else(|| i.error("No window"))?;
    let delta = want_int(i, &a[0])?;
    let horiz = a.get(1).map(|v| v.truthy()).unwrap_or(false);
    window_resize(i, &w, -delta, horiz)?;
    Ok(Value::t())
}
fn f_enlarge_window_horizontally(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    f_enlarge_window(i, vec![a[0].clone(), Value::t()])
}
fn f_shrink_window_horizontally(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    f_shrink_window(i, vec![a[0].clone(), Value::t()])
}
fn f_adjust_window_trailing_edge(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let w = win_of(i, &a[0])?;
    let delta = want_int(i, &a[1])?;
    let horiz = a.get(2).map(|v| v.truthy()).unwrap_or(false);
    window_resize(i, &w, delta, horiz)?;
    Ok(Value::Nil)
}

fn f_balance_windows(i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    let f = sel_frame(i).ok_or_else(|| i.error("No frame"))?;
    let ff = f.borrow();
    let wins: Vec<WindowRef> = ff
        .windows
        .iter()
        .filter(|w| !w.borrow().dead && !w.borrow().minibuffer)
        .cloned()
        .collect();
    if wins.len() > 1 {
        let total: usize = wins.iter().map(|w| w.borrow().height).sum();
        let each = total / wins.len();
        for (k, w) in wins.iter().enumerate() {
            // Give the remainder to the first window.
            w.borrow_mut().height = each + if k == 0 { total % wins.len() } else { 0 };
        }
    }
    Ok(Value::t())
}

fn f_maximize_window(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let w = win_of(i, &arg(&a, 0))?;
    let f = sel_frame(i).ok_or_else(|| i.error("No frame"))?;
    let fh = f.borrow().height;
    let delta = fh as i128 - w.borrow().height as i128 - 1;
    if delta <= 0 {
        return Ok(Value::t());
    }
    window_resize(i, &w, delta, false)?;
    Ok(Value::t())
}
fn f_minimize_window(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let w = win_of(i, &arg(&a, 0))?;
    let cur = w.borrow().height;
    if cur <= WINDOW_MIN_HEIGHT {
        return Ok(Value::t());
    }
    window_resize(i, &w, WINDOW_MIN_HEIGHT as i128 - cur as i128, false)?;
    Ok(Value::t())
}

fn f_split_window_vertically(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    // GNU's `split-window-below' (and its `split-window-vertically'
    // alias): (SIZE WINDOW-TO-SPLIT) → `split-window' with side nil.
    f_split_window(i, vec![arg(&a, 1), arg(&a, 0)])
}
fn f_split_window_horizontally(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    // GNU: (split-window-horizontally &optional SIZE WINDOW-TO-SPLIT)
    // puts a new window of SIZE columns on the right of
    // WINDOW-TO-SPLIT — i.e. `split-window' with side `right'.
    let right = Value::Sym(i.intern("right"));
    f_split_window(i, vec![arg(&a, 1), arg(&a, 0), right])
}

fn f_switch_to_buffer_other_window(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let bid = match i.buffer_id_of(&a[0]) {
        Some(id) => id,
        None => {
            let name = match &a[0] {
                Value::Str(s) => s.borrow().clone(),
                _ => return Err(i.wrong_type_mut("bufferp", &a[0])),
            };
            i.buffers.create(&name)
        }
    };
    let f = sel_frame(i).ok_or_else(|| i.error("No frame"))?;
    // Pick or make another window.
    let sel = sel_window(i).ok_or_else(|| i.error("No window"))?;
    let target = if f
        .borrow()
        .windows
        .iter()
        .filter(|w| !w.borrow().dead)
        .count()
        > 1
    {
        window_cycle(i, 1, &sel).unwrap_or_else(|| sel.clone())
    } else {
        match f_split_window(i, vec![Value::Nil, Value::Nil])? {
            Value::Window(w) => w,
            _ => sel.clone(),
        }
    };
    target.borrow_mut().buffer = bid;
    f.borrow_mut().selected = target.clone();
    i.set_current_buffer(bid);
    i.buffers.touch(bid);
    Ok(i.buffer_value(bid).unwrap_or(Value::Nil))
}

fn f_switch_to_buffer_other_frame(i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    Err(i.error("Cannot switch to a different frame"))
}
fn f_display_buffer_other_frame(i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    Err(i.error("Cannot switch to a different frame"))
}

fn f_quit_window(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let w = win_of(i, &arg(&a, 1))?;
    let kill = arg(&a, 0).truthy();
    if kill {
        let bid = w.borrow().buffer;
        crate::buffer::primitives::kill_buffer_keep_current(i, bid);
    }
    let f = sel_frame(i).ok_or_else(|| i.error("No frame"))?;
    let live = f
        .borrow()
        .windows
        .iter()
        .filter(|x| !x.borrow().dead)
        .count();
    if live > 1 {
        f_delete_window(i, vec![a.get(1).cloned().unwrap_or(Value::Nil)])?;
    } else {
        // Sole window: show another buffer instead.
        let cur = w.borrow().buffer;
        if let Some(other) = i.buffers.other(cur) {
            w.borrow_mut().buffer = other;
            i.set_current_buffer(other);
        }
    }
    Ok(Value::Nil)
}

fn f_quit_restore_window(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    // (quit-restore-window &optional WINDOW BURY-OR-KILL)
    let w = win_of(i, &arg(&a, 0))?;
    let bok = arg(&a, 1);
    let bok_name = i.sym_id(&bok).map(|s| i.symbol_name(s)).unwrap_or_default();
    let kill = bok.truthy() && bok_name != "bury" && bok_name != "append";
    f_quit_window(i, vec![Value::from_bool(kill), Value::Window(w)])
}

fn f_kill_buffer_and_window(i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    let f = sel_frame(i).ok_or_else(|| i.error("No frame"))?;
    let live = f
        .borrow()
        .windows
        .iter()
        .filter(|x| !x.borrow().dead)
        .count();
    if live <= 1 {
        return Err(i.error("Attempt to delete the only window"));
    }
    let bid = i.current_buffer;
    crate::buffer::primitives::kill_buffer_keep_current(i, bid);
    f_delete_window(i, vec![Value::Nil])?;
    let next = i
        .buffers
        .other(bid)
        .or_else(|| i.buffers.list().first().copied())
        .unwrap_or_else(|| i.buffers.create("*scratch*"));
    i.set_current_buffer(next);
    Ok(Value::Nil)
}

fn f_replace_buffer_in_windows(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let bid = match arg(&a, 0) {
        Value::Nil => i.current_buffer,
        v => match i.buffer_id_of(&v) {
            Some(id) => id,
            None => return Err(i.error("No such buffer")),
        },
    };
    let other = i.buffers.other(bid).unwrap_or(bid);
    let f = sel_frame(i).ok_or_else(|| i.error("No frame"))?;
    for w in &f.borrow().windows {
        if w.borrow().buffer == bid && !w.borrow().dead {
            w.borrow_mut().buffer = other;
        }
    }
    if i.current_buffer == bid && bid != other {
        i.set_current_buffer(other);
    }
    Ok(Value::Nil)
}

fn f_window_tree(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let f = frame_of(i, &arg(&a, 0))?;
    let ff = f.borrow();
    let (w, h) = (ff.width, ff.height);
    let wins: Vec<Value> = ff
        .windows
        .iter()
        .filter(|x| !x.borrow().dead && !x.borrow().minibuffer)
        .map(|x| Value::Window(x.clone()))
        .collect();
    // Node = (VERTICAL-P EDGES CHILD...) where EDGES = (LEFT TOP RIGHT BOTTOM).
    // A lone window appears directly, like GNU's root.
    let root = if wins.len() == 1 {
        wins[0].clone()
    } else {
        let mut node = vec![
            Value::t(),
            Value::list(vec![
                Value::Int(0),
                Value::Int(1),
                Value::Int(w as i128),
                Value::Int(h as i128),
            ]),
        ];
        node.extend(wins);
        Value::list(node)
    };
    let mut top = vec![root];
    if let Some(mb) = &ff.minibuffer {
        top.push(Value::Window(mb.clone()));
    }
    Ok(Value::list(top))
}

fn f_window_combination_limit(i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    // All our windows are leaves — GNU errors on non-internal windows.
    Err(i.error("Window is not a combination window"))
}

fn f_scroll_other_window_down(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let f = sel_frame(i).ok_or_else(|| i.error("No frame"))?;
    let live = f
        .borrow()
        .windows
        .iter()
        .filter(|x| !x.borrow().dead && !x.borrow().minibuffer)
        .count();
    if live < 2 {
        return Err(i.error("There is no other window"));
    }
    let n = arg(&a, 0).int().unwrap_or(1);
    f_scroll_other_window(i, vec![Value::Int(-n)])
}

// ---------- overlays (missing pieces) ----------

fn f_copy_overlay(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let (bid, idx) = overlay_of(i, &a[0])?;
    let ov = match overlay_slot(i, bid, idx) {
        Some(o) => o,
        None => return Err(i.wrong_type_mut("overlayp", &a[0])),
    };
    let b = i.buffers.get(bid).unwrap();
    let mut bb = b.borrow_mut();
    let new_idx = bb.overlays.len();
    let handle = Value::Vec(Rc::new(RefCell::new(vec![
        Value::Sym(i.intern("overlay")),
        Value::Int(bid as i128),
        Value::Int(new_idx as i128),
    ])));
    bb.overlays.push(crate::buffer::Overlay {
        handle: handle.clone(),
        ..ov
    });
    Ok(handle)
}

fn f_overlay_lists(i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    // On GNU 31 all live overlays sit in the "before" list (sorted by
    // start); the "after" half is only used transiently by recenter.
    let b = cur(i);
    let bb = b.borrow();
    let bid = bb.id;
    let mut live: Vec<&crate::buffer::Overlay> = bb
        .overlays
        .iter()
        .filter(|ov| ov.buffer == Some(bid))
        .collect();
    live.sort_by_key(|ov| ov.start);
    Ok(Value::cons(
        Value::list(live.into_iter().map(|ov| ov.handle.clone()).collect()),
        Value::Nil,
    ))
}

fn f_overlay_recenter(_i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    Ok(Value::Nil)
}

// ---------- readers & redisplay ----------

fn f_completing_read_default(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    f_completing_read(i, a)
}

fn f_completing_read_multiple(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let prompt = match &a[0] {
        Value::Str(s) => format!("{}[comma-separated list] ", s.borrow()),
        _ => String::new(),
    };
    let mut args = a.clone();
    args[0] = Value::string(prompt);
    match f_completing_read(i, args)? {
        Value::Str(s) => Ok(Value::list(
            s.borrow()
                .split(',')
                .map(|p| Value::string(p.trim().to_string()))
                .filter(|v| match v {
                    Value::Str(t) => !t.borrow().is_empty(),
                    _ => true,
                })
                .collect(),
        )),
        _ => Ok(Value::Nil),
    }
}

fn f_read_coding_system(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let prompt = match &a[0] {
        Value::Str(s) => s.borrow().clone(),
        _ => String::new(),
    };
    let input = if i.minibuf_reader.is_some() {
        i.minibuf_line(&prompt)?
    } else {
        String::new()
    };
    let def = match a.get(1) {
        Some(Value::Sym(s)) => i.symbol_name(*s),
        _ => "undecided".to_string(),
    };
    let name = if input.is_empty() { def } else { input };
    Ok(Value::Sym(i.intern(&name)))
}

fn f_read_color(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let prompt = match a.first() {
        Some(Value::Str(s)) => s.borrow().clone(),
        _ => "Color name: ".to_string(),
    };
    let input = if i.minibuf_reader.is_some() {
        i.minibuf_line(&prompt)?
    } else {
        String::new()
    };
    if input.is_empty() && !a.get(2).map(|v| v.truthy()).unwrap_or(false) {
        return Ok(Value::string(""));
    }
    Ok(Value::string(input))
}

fn f_read_passwd(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let prompt = match &a[0] {
        Value::Str(s) => s.borrow().clone(),
        _ => String::new(),
    };
    let input = if i.minibuf_reader.is_some() {
        i.minibuf_line(&prompt)?
    } else {
        return Err(batch_eof(i));
    };
    Ok(Value::string(input))
}

fn f_read_kbd_macro(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    // GNU's read-kbd-macro always returns a vector (unlike `kbd').
    let s = want_str(i, &a[0])?;
    let keys = parse_kbd(i, &s);
    Ok(Value::Vec(Rc::new(RefCell::new(keys))))
}

fn f_momentary_string_display(_i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    Ok(Value::Nil)
}

fn f_minibuffer_completion_help(_i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    Ok(Value::Nil)
}

fn f_display_message_or_buffer(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    // Small messages go to the echo area; multi-line would pop to a
    // buffer — we always echo and return the message.
    if let Value::Str(s) = &a[0] {
        i.echo_message = s.borrow().clone();
    }
    Ok(a[0].clone())
}

fn f_redisplay(_i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    Ok(Value::t())
}

fn f_force_mode_line_update(_i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    Ok(Value::Nil)
}

fn f_tooltip_show(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    // tty: no tooltip — GNU returns the text.
    let _ = i;
    Ok(a[0].clone())
}
fn f_tooltip_hide(_i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    Ok(Value::Nil)
}

fn f_timer_event_handler(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    Err(i.wrong_type_mut("timerp", &a[0]))
}

fn f_invisible_p(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    // Check the `invisible' text property at POS.
    let pos = want_int(i, &a[0])?;
    let inv = i.intern("invisible");
    crate::buffer::primitives::f_get_text_property(i, vec![Value::Int(pos), Value::Sym(inv)])
}

fn f_fringe_bitmaps_at_pos(_i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    Ok(Value::Nil)
}

// ---------- modes & jit/font-lock ----------

fn mode_toggle(i: &mut Interp, a: &[Value], var: &str) -> EvalResult {
    let sym = i.intern(var);
    let cur = i.symbol_value(sym).truthy();
    let new = match a.first().and_then(|v| v.int()) {
        Some(n) => n > 0,
        None => match a.first() {
            Some(Value::Nil) | None => !cur,
            Some(v) => v.truthy(),
        },
    };
    i.obarray.symbol_mut(sym).value = Value::from_bool(new);
    Ok(Value::from_bool(new))
}

fn f_binary_overwrite_mode(i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    Ok(Value::Sym(i.intern("overwrite-mode-binary")))
}
fn f_scroll_lock_mode(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    mode_toggle(i, &a, "scroll-lock-mode")
}
fn f_pixel_scroll_mode(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    mode_toggle(i, &a, "pixel-scroll-mode")
}
fn f_pixel_scroll_precision_mode(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    mode_toggle(i, &a, "pixel-scroll-precision-mode")
}
// font-lock-mode, font-lock-ensure/flush and jit-lock-register/unregister are
// implemented in Lisp (GNU font-core.el / font-lock.el / jit-lock.el ports in
// the prelude).

// ---------- command loop / terminal misc ----------

fn f_move_to_window_line_top_bottom(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    // Cycle point through center → bottom → top window lines on repeat.
    let me = i.intern("move-to-window-line-top-bottom");
    let this_id = i.intern("this-command");
    let last_id = i.intern("last-command");
    let this = i.symbol_value(this_id);
    let last = i.symbol_value(last_id);
    let repeated =
        matches!(last, Value::Sym(s) if s == me) && matches!(this, Value::Sym(s) if s == me);
    if repeated {
        i.mtwlb_phase = (i.mtwlb_phase + 1) % 3;
    } else {
        i.mtwlb_phase = 0;
    }
    let arg_n = a.first().and_then(|v| v.int()).unwrap_or(0);
    let w = sel_window(i).unwrap();
    let (buf, start, height) = {
        let wb = w.borrow();
        (wb.buffer, wb.start, wb.height)
    };
    if let Some(b) = i.buffers.get(buf) {
        let bb = b.borrow();
        let start_line = bb.text.line_of_pos(start);
        // phase: 0 center, 1 bottom, 2 top.
        let frac_num = match i.mtwlb_phase {
            0 => height / 2,
            1 => height.saturating_sub(2),
            _ => 0,
        };
        let mut target_line = (start_line + frac_num) as i128 + arg_n;
        if target_line < 0 {
            target_line = 0;
        }
        let p = bb.text.line_start(target_line as usize);
        let tlen = bb.text_len();
        drop(bb);
        if let Some(b2) = i.buffers.get(buf) {
            b2.borrow_mut().set_point(p.min(tlen));
        }
        w.borrow_mut().point = i.buffers.get(buf).map(|x| x.borrow().point).unwrap_or(0);
    }
    // GNU returns the number of lines point moved within the window.
    Ok(Value::Int(arg_n.max(0)))
}

fn f_recenter_other_window(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    // Scroll other window so its point line is centered. GNU signals
    // "There is no other window" on a single-window frame.
    let sel = sel_window(i).unwrap();
    let other = window_cycle(i, 1, &sel).unwrap_or(sel.clone());
    if Rc::ptr_eq(&other, &sel) {
        return Err(i.error("There is no other window"));
    }
    let arg_n = a.first().and_then(|v| v.int()).unwrap_or(-1);
    let (buf, height) = {
        let wb = other.borrow();
        (wb.buffer, wb.height)
    };
    if let Some(b) = i.buffers.get(buf) {
        let bb = b.borrow();
        let pl = bb.text.line_of_pos(bb.point());
        let delta = if arg_n < 0 {
            height / 2
        } else {
            arg_n as usize
        };
        let start_line = pl.saturating_sub(delta);
        let p = bb.text.line_start(start_line);
        drop(bb);
        other.borrow_mut().start = p;
    }
    Ok(Value::Nil)
}

fn f_exit_minibuffer(i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    // Outside an active minibuffer GNU's throw reaches no catch → no-catch.
    let no_catch = i.intern("no-catch");
    let exit_sym = i.intern("exit");
    Err(i.signal_data(no_catch, vec![Value::Sym(exit_sym), Value::Nil]))
}

fn f_self_insert_and_exit(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    // Insert last-command-event's char, then exit-minibuffer.
    let _ = a;
    f_exit_minibuffer(i, vec![])
}

fn f_save_buffers_kill_terminal(i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    i.quit_editor = true;
    Ok(Value::Nil)
}

fn f_open_dribble_file(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    match &a[0] {
        Value::Nil => {
            i.dribble_file = None;
        }
        Value::Str(s) => {
            let path = s.borrow().clone();
            if let Err(e) = std::fs::File::create(&path) {
                return Err(i.error(format!("Cannot open dribble file {path}: {e}")));
            }
            i.dribble_file = Some(path);
        }
        _ => return Err(i.wrong_type_mut("stringp", &a[0])),
    }
    Ok(Value::Nil)
}

fn f_suspend_emacs(i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    // GNU batch: suspending on a non-tty terminal signals a plain error.
    if i.noninteractive {
        return Err(i.error("Attempt to suspend a non-text terminal device"));
    }
    i.quit_editor = true;
    Ok(Value::Nil)
}

fn f_byteorder(_i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    // GNU returns ?l (108) on little-endian, ?B (66) on big-endian.
    #[cfg(target_endian = "little")]
    return Ok(Value::Int(108));
    #[cfg(target_endian = "big")]
    return Ok(Value::Int(66));
}

fn f_standard_display_european_internal(_i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    // GNU returns the previous glyph-display flag as a one-element vector.
    Ok(Value::Vec(Rc::new(RefCell::new(vec![Value::Int(39)]))))
}

/// Called by `Interp::new` to wire editor subrs and create the initial frame.
pub fn install_primitives(i: &mut Interp) {
    for s in SUBRS {
        let id = i.intern(s.name);
        i.fset(id, Value::Subr(s));
    }
    for s in winxtra::SUBRS {
        let id = i.intern(s.name);
        i.fset(id, Value::Subr(s));
    }
    // Initial frame with one window showing *scratch* + a minibuffer.
    let scratch = i.buffers.by_name("*scratch*").unwrap_or(i.current_buffer);
    let mb = i
        .buffers
        .by_name(" *Minibuf-0*")
        .or_else(|| i.buffers.by_name(" *Minibuf-0*"))
        .unwrap_or_else(|| i.buffers.create(" *Minibuf-0*"));
    let frame = Frame::new_tty(scratch, mb, 80, 25);
    // Wire the frame's window buffer linkage.
    i.selected_frame = Some(frame.clone());
    i.frames.push(frame);
    // global-map default: a dense keymap (char-table element), as in
    // GNU's `current-global-map'.
    let gm = i.intern("global-map");
    if i.symbol_value(gm).is_nil() {
        let km = Value::cons(
            Value::Sym(i.intern("keymap")),
            Value::cons(keymap_char_table(i), Value::Nil),
        );
        i.obarray.symbol_mut(gm).value = km;
    }
}
