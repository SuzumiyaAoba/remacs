//! Editor layer: windows, frames, keymaps, kill-ring, file I/O,
//! and editing commands built on the buffer primitives.
//!
//! Windows/frames are Lisp objects (`Value::Window`, `Value::Frame`)
//! shared via `Rc<RefCell<_>>` so Lisp code can hold references while
//! the front-end mutates geometry.

use std::cell::RefCell;
use std::rc::Rc;

use crate::lisp::Interp;
use crate::lisp::builtins::eq_values;
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
            dead: false,
        }))
    }
}

impl Frame {
    pub fn new_tty(buffer: usize, minibuf: usize, width: usize, height: usize) -> FrameRef {
        let main = Window::new(buffer);
        let mb = Window::new(minibuf);
        mb.borrow_mut().minibuffer = true;
        Rc::new(RefCell::new(Frame {
            id: next_id(),
            name: "F1".into(),
            windows: vec![main.clone()],
            selected: main,
            minibuffer: Some(mb),
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
    S!("split-window", 0, 3, f_split_window, "Split WINDOW."),
    S!(
        "split-window-below",
        0,
        1,
        f_split_window_below,
        "Split below."
    ),
    S!(
        "split-window-right",
        0,
        1,
        f_split_window_right,
        "Split right."
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
        "move-to-window-line",
        1,
        1,
        f_move_to_window_line,
        "Move point to window line N."
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
    S!("window-display-table", 0, 1, f_nil, ""),
    S!("set-window-display-table", 2, 2, f_second, ""),
    S!(
        "window-margins",
        0,
        1,
        f_window_margins,
        "Margins of WINDOW."
    ),

    S!("window-use-time", 0, 1, f_zero, ""),
    S!("window-cursor-type", 0, 1, f_t, ""),
    S!("window-safe-p", 0, 0, f_t, ""),
    S!("window-configuration-p", 1, 1, f_nil, ""),
    S!("current-window-configuration", 0, 1, f_nil, ""),
    S!("set-window-configuration", 1, 3, f_nil, ""),
    S!("window-state-get", 0, 2, f_nil, ""),
    S!("window-state-put", 1, 3, f_nil, ""),
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
    S!("frame-terminal", 0, 1, f_selected_frame, ""),
    S!("select-frame", 1, 2, f_select_frame, "Select FRAME."),
    S!("handle-switch-frame", 1, 1, f_nil, ""),
    S!("frame-focus-state", 0, 1, f_t, ""),
    S!("suspend-emacs", 0, 1, f_nil, ""),
    S!("redraw-frame", 0, 1, f_nil, ""),
    S!("redraw-display", 0, 0, f_nil, ""),
    S!("frame-visible-p", 1, 1, f_t, ""),
    S!("visible-p", 0, 0, f_t, ""),
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
    S!("keymap-prompt", 1, 1, f_nil, ""),
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
        3,
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
    S!("minor-mode-key-binding", 1, 1, f_nil, ""),
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
    S!("current-minor-mode-maps", 0, 0, f_nil, ""),
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
        4,
        4,
        f_copy_to_buffer,
        "Copy region to BUFFER."
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
        "file-name-sans-directory",
        1,
        1,
        f_file_name_nondirectory,
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
    S!("delete-directory", 1, 3, f_delete_directory, "Delete DIR."),
    S!("delete-file", 1, 2, f_delete_file, "Delete FILENAME."),
    S!(
        "rename-file",
        2,
        3,
        f_rename_file,
        "Rename FILE to NEWNAME."
    ),
    S!("copy-file", 2, 4, f_copy_file, "Copy FILE to NEWNAME."),
    S!("copy-directory", 2, 4, f_nil, ""),
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
    S!("write-region-annotate-functions", 0, 0, f_nil, ""),
    S!("write-region-post-annotation-function", 0, 0, f_nil, ""),
    S!("write-region-charset-for-write", 0, 0, f_nil, ""),
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
    S!("file-name-history", 0, 0, f_nil, ""),
    S!("insert-directory-literally", many 0, f_nil, ""),
    S!("insert-directory", many 0, f_nil, ""),
    S!("unhandled-file-name-directory", 1, 1, f_nil, ""),
    S!("file-remote-p", 1, 3, f_nil, ""),
    S!("file-local-name", 1, 1, f_identity, ""),
    S!("file-name-quote", 1, 1, f_identity, ""),
    S!("file-name-unquote", 1, 1, f_identity, ""),
    S!("file-accessible-directory-p", 1, 1, f_file_directory_p, ""),
    S!("verify-visited-file-modtime-princ", 0, 0, f_nil, ""),
    S!("set-default-file-modes", 1, 1, f_nil, ""),
    S!("default-file-modes", 0, 0, f_file_modes_default, ""),
    S!("file-modes-symbolic-to-number", 1, 3, f_zero, ""),
    S!("unix-sync", 0, 0, f_nil, ""),
    S!("file-system-info", 1, 1, f_nil, ""),
    S!("file-equal-p", 2, 2, f_nil, ""),
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
    S!(
        "start-process",
        3,
        3,
        f_start_process_stub,
        "Start async process (unsupported)."
    ),
    S!("processp", 1, 1, f_nil, ""),
    S!("process-status", 1, 1, f_nil, ""),
    S!("process-list", 0, 0, f_nil, ""),
    S!("get-process", 1, 1, f_nil, ""),
    S!("delete-process", 1, 1, f_nil, ""),
    S!("process-name", 1, 1, f_nil, ""),
    S!("process-buffer", 1, 1, f_nil, ""),
    S!("process-mark", 1, 1, f_nil, ""),
    S!("process-exit-status", 1, 1, f_nil, ""),
    S!("process-id", 1, 1, f_nil, ""),
    S!("process-send-string", 2, 2, f_nil, ""),
    S!("process-send-eof", 0, 1, f_nil, ""),
    S!("set-process-filter", 2, 2, f_nil, ""),
    S!("set-process-sentinel", 2, 2, f_nil, ""),
    S!("accept-process-output", 0, 4, f_nil, ""),
    S!("process-put", 3, 3, f_third, ""),
    S!("process-get", 2, 2, f_nil, ""),
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
        "transpose-words",
        1,
        1,
        f_transpose_words,
        "Swap words around point."
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
    S!("tab-to-tab-stop", 0, 0, f_nil, ""),
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
    S!(
        "next-line",
        0,
        1,
        f_next_line,
        "Move to next line keeping column."
    ),
    S!(
        "previous-line",
        0,
        1,
        f_previous_line,
        "Move to previous line."
    ),
    S!("beginning-of-buffer-other-window", 0, 0, f_nil, ""),
    S!("set-goal-column", 1, 1, f_nil, ""),
    S!("exchange-point-and-mark-inactive", 0, 0, f_nil, ""),
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
    S!("minibuffer-prompt-end", 0, 0, f_one, ""),
    S!(
        "active-minibuffer-window",
        0,
        0,
        f_active_minibuffer_window,
        ""
    ),
    S!("set-minibuffer-window", 1, 1, f_nil, ""),
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
    S!("completion-boundaries", 0, 0, f_nil, ""),
    S!("internal-complete-buffer", 3, 3, f_nil, ""),
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
    S!("execute-kbd-macro", 1, 2, f_nil, ""),
    S!("start-kbd-macro", 1, 2, f_nil, ""),
    S!("end-kbd-macro", 0, 1, f_nil, ""),
    S!("call-last-kbd-macro", 0, 2, f_nil, ""),
    S!("kmacro-exec-ring-item", 2, 2, f_nil, ""),
    S!("defining-kbd-macro", 0, 1, f_nil, ""),
    S!("cancel-kbd-macro-events", 0, 0, f_nil, ""),
    S!("store-kbd-macro-event", 1, 1, f_nil, ""),
    S!("kbd-macro-query", 0, 0, f_nil, ""),
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
    S!(
        "beginning-of-defun",
        0,
        1,
        f_beginning_of_defun,
        "Move to defun start."
    ),
    S!("end-of-defun", 0, 1, f_end_of_defun, "Move past defun end."),
    S!("mark-defun", 0, 0, f_mark_defun, "Mark the defun."),
    S!(
        "narrow-to-defun",
        0,
        1,
        f_narrow_to_defun,
        "Narrow to defun."
    ),
    S!("mark-page", 0, 0, f_nil, ""),
    S!("narrow-to-page", 0, 1, f_nil, ""),
    S!("count-words", 2, 2, f_count_words, "Words in region."),
    S!("count-words-region", 2, 2, f_count_words, ""),
    S!("count-lines-page", 0, 0, f_nil, ""),
    S!(
        "what-cursor-position",
        0,
        1,
        f_what_cursor_position,
        "Describe point."
    ),
    S!("what-line", 0, 0, f_what_line, "Show line number."),
    S!("char-syntax", 1, 1, f_char_syntax, "Syntax code of CHAR."),
    S!("modify-syntax-entry", 2, 3, f_nil, ""),
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
    S!("syntax-after", 1, 1, crate::buffer::primitives::f_syntax_after, ""),
    S!("syntax-class", 1, 1, f_zero, ""),
    S!("standard-syntax-table", 0, 0, f_standard_syntax_table, ""),
    S!("string-to-syntax", 1, 1, f_nil, ""),
    S!("syntax-propertize", 1, 1, f_nil, ""),
    S!("internal--syntax-propertize", 0, 0, f_nil, ""),
    S!(
        "parse-partial-sexp",
        2,
        8,
        f_parse_partial_sexp,
        "Sexp parse state (approx)."
    ),
    S!(
        "syntax-ppss",
        0,
        1,
        f_syntax_ppss,
        "Sexp parser state at POS."
    ),
    S!("inside-comment-p", 0, 0, f_nil, ""),
    S!("comment-beginning", 0, 0, f_nil, ""),
    // modes
    S!(
        "fundamental-mode",
        0,
        0,
        f_fundamental_mode,
        "The default major mode."
    ),
    S!("normal-mode", 0, 1, f_normal_mode, "Pick major mode."),
    S!("major-mode-suspend", 0, 0, f_nil, ""),
    S!(
        "delay-mode-hooks",
        raw,
        f_progn_raw,
        "Eval BODY delaying mode hooks."
    ),
    S!("run-mode-hooks", many 0, f_run_mode_hooks, "Run mode hooks."),
    S!("set-auto-mode", 0, 1, f_nil, ""),
    S!("set-auto-mode-0", 0, 0, f_nil, ""),
    S!("set-buffer-major-mode", 1, 1, f_nil, ""),
    S!("hack-local-variables", 0, 1, f_nil, ""),
    S!("hack-dir-local-variables", 0, 0, f_nil, ""),
    S!("dir-locals-set-class-variables", 1, 1, f_nil, ""),
    // timers
    S!("run-at-time", 2, 7, f_nil, ""),
    S!("run-with-timer", 2, 5, f_nil, ""),
    S!("run-with-idle-timer", 2, 8, f_nil, ""),
    S!("cancel-timer", 1, 1, f_nil, ""),
    S!("timerp", 1, 1, f_nil, ""),
    S!("timer-activate", 1, 2, f_nil, ""),
    S!(
        "with-timeout",
        raw,
        f_with_timeout_raw,
        "Eval body (timeout ignored)."
    ),
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
    S!(
        "restore-buffer-modified-p",
        1,
        1,
        crate::buffer::primitives::f_set_buffer_modified_p,
        ""
    ),
    // faces (stubs — tty has limited support)
    S!("facep", 1, 1, f_nil, ""),
    S!("internal-get-lisp-face-attribute", 2, 3, f_nil, ""),
    S!("set-face-attribute", many 2, f_nil, ""),
    S!("face-attribute", 2, 4, f_nil, ""),
    S!("face-attribute-relative-p", 2, 2, f_nil, ""),
    S!("merge-face-attribute", 3, 3, f_nil, ""),
    S!("face-all-attributes", 1, 2, f_nil, ""),
    S!("face-list", 0, 0, f_nil, ""),
    S!("make-face", 1, 1, f_first, ""),
    S!("copy-face", 2, 2, f_first, ""),
    S!("face-equal", 2, 2, f_nil, ""),
    S!("face-id", 1, 2, f_zero, ""),

    S!("internal-lisp-face-p", 1, 2, f_nil, ""),
    S!("internal-lisp-face-empty-p", 1, 2, f_nil, ""),
    S!("internal-lisp-face-equal-p", 2, 3, f_t, ""),
    S!("internal-set-lisp-face-attribute", 3, 4, f_nil, ""),
    S!("internal-lisp-face-attribute-values", 1, 1, f_nil, ""),
    S!("internal-merge-in-global-face", 2, 2, f_nil, ""),
    S!("face-attrs-more-relative-p", 2, 2, f_nil, ""),
    S!("display-color-p", 0, 1, f_display_color_p, ""),
    S!("display-grayscale-p", 0, 1, f_nil, ""),
    S!("display-mouse-p", 0, 1, f_nil, ""),
    S!("color-defined-p", 1, 1, f_color_defined_p, ""),
    S!("defined-colors", 0, 1, f_defined_colors, ""),
    S!("color-values", 1, 1, f_nil, ""),
    S!("x-color-values", 1, 1, f_nil, ""),
    S!("xw-color-values", 1, 1, f_nil, ""),
    S!("tty-color-values", 1, 1, f_nil, ""),
    S!("tty-defined-colors", 0, 1, f_nil, ""),
    S!("x-list-fonts", many 0, f_nil, ""),
    S!("internal-char-font", 1, 2, f_nil, ""),
    S!("fontp", 1, 2, f_nil, ""),
    S!("find-font", 1, 1, f_nil, ""),
    S!("font-xlfd-name", 1, 1, f_nil, ""),
    S!("clear-font-cache", 0, 0, f_nil, ""),
    S!("list-fonts", 3, 3, f_nil, ""),
    // cursor/display misc
    // `cursor-type` is a variable in Emacs, not a function —
    // calling it signals void-function like GNU.
    S!("blink-cursor-mode", 0, 1, f_nil, ""),
    S!("internal-show-cursor", 2, 2, f_nil, ""),
    S!("internal-show-cursor-p", 0, 1, f_t, ""),
    S!("set-window-cursor-type", 2, 2, f_nil, ""),
    S!("set-display-table-slot", 3, 3, f_nil, ""),
    S!("display-table-slot", 2, 2, f_nil, ""),
    S!("make-display-table", 0, 0, f_nil, ""),
    S!("display-table-p", 1, 1, f_nil, ""),
    S!("describe-display-table", 1, 1, f_nil, ""),
    S!("standard-display-table", 0, 0, f_nil, ""),
    S!("dump-glyph-matrix", 0, 0, f_nil, ""),
    S!("open-font", 1, 3, f_nil, ""),
    S!("query-font", 1, 1, f_nil, ""),
    S!("font-get", 2, 2, f_nil, ""),
    S!("font-put", 3, 3, f_nil, ""),
    S!("set-fontset-font", many 0, f_nil, ""),
    S!("new-fontset", 2, 2, f_nil, ""),
    S!("fontset-info", 1, 1, f_nil, ""),
    S!("fontset-font", 2, 3, f_nil, ""),
    S!("fontset-list", 0, 0, f_nil, ""),
    S!("fontset-list-all", 0, 0, f_nil, ""),
    // menus/popups
    S!("x-popup-menu", 2, 2, f_nil, ""),
    S!("x-popup-dialog", 2, 3, f_nil, ""),
    S!("menu-or-popup-active-p", 0, 0, f_nil, ""),
    S!("menu-bar-menu-at-x-y", 2, 2, f_nil, ""),
    S!("x-menu-bar-open-internal", 0, 1, f_nil, ""),
    // echo/help
    S!("describe-bindings-internal", 0, 2, f_nil, ""),
    S!(
        "documentation-property",
        2,
        3,
        f_documentation_property,
        "Prop on symbol."
    ),
    S!("Snarf-documentation", 1, 1, f_nil, ""),
    S!(
        "documentation",
        1,
        2,
        f_documentation,
        "Docstring of FUNCTION."
    ),
    S!("keymap-get-key", 0, 0, f_nil, ""),
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
fn f_zero(_i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    Ok(Value::Int(0))
}
fn f_one(_i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    Ok(Value::Int(1))
}
fn f_identity(_i: &mut Interp, a: Vec<Value>) -> EvalResult {
    Ok(a.into_iter().next().unwrap_or(Value::Nil))
}
fn f_first(_i: &mut Interp, a: Vec<Value>) -> EvalResult {
    Ok(a.into_iter().next().unwrap_or(Value::Nil))
}
fn f_second(_i: &mut Interp, a: Vec<Value>) -> EvalResult {
    Ok(a.into_iter().nth(1).unwrap_or(Value::Nil))
}
fn f_third(_i: &mut Interp, a: Vec<Value>) -> EvalResult {
    Ok(a.into_iter().nth(2).unwrap_or(Value::Nil))
}
fn f_progn_raw(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    i.eval_progn(&a.into_iter().next().unwrap_or(Value::Nil))
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
        .ok_or_else(|| i.error_obj("No such buffer", &a[1]))?;
    w.borrow_mut().buffer = bid;
    w.borrow_mut().point = 0;
    w.borrow_mut().start = 0;
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

fn f_window_list(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let f = frame_of(i, &arg(&a, 0))?;
    let include_mini = a.get(1).map(|v| v.truthy()).unwrap_or(false);
    let mut out: Vec<Value> = f
        .borrow()
        .windows
        .iter()
        .map(|w| Value::Window(w.clone()))
        .collect();
    if include_mini {
        if let Some(mb) = &f.borrow().minibuffer {
            out.push(Value::Window(mb.clone()));
        }
    }
    Ok(Value::list(out))
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

fn f_minibuffer_window_active_p(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    // Active iff the minibuffer has contents / is selected.
    let _ = (i, a);
    Ok(Value::Nil)
}

fn f_split_window(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let w = win_of(i, &arg(&a, 0))?;
    let side_sym = a.get(2).and_then(|v| i.sym_id(v)).map(|s| i.symbol_name(s));
    let _ = side_sym; // vertical vs horizontal — tty splits vertically
    let buf = w.borrow().buffer;
    let new = Window::new(buf);
    // Halve the height of the original.
    {
        let mut ww = w.borrow_mut();
        let half = ww.height / 2;
        ww.height = half;
        new.borrow_mut().top = ww.top + half;
        new.borrow_mut().height = ww.height;
        new.borrow_mut().width = ww.width;
        new.borrow_mut().left = ww.left;
        new.borrow_mut().point = ww.point;
        new.borrow_mut().start = ww.start;
    }
    let f = sel_frame(i).unwrap();
    f.borrow_mut().windows.push(new.clone());
    Ok(Value::Window(new))
}

fn f_split_window_below(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    f_split_window(i, vec![arg(&a, 0), Value::Nil])
}

fn f_split_window_right(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    f_split_window(i, vec![arg(&a, 0), Value::Nil])
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
        ff.windows.retain(|w2| w2.borrow().id != wid);
        w.borrow_mut().dead = true;
        if ff.selected.borrow().id == wid {
            ff.selected = ff.windows[0].clone();
        }
    }
    Ok(Value::Nil)
}

fn f_delete_other_windows(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let w = win_of(i, &arg(&a, 0))?;
    let wid = w.borrow().id;
    let f = sel_frame(i).unwrap();
    let mut ff = f.borrow_mut();
    ff.windows.retain(|w2| {
        let keep = w2.borrow().id == wid;
        if !keep {
            w2.borrow_mut().dead = true;
        }
        keep
    });
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
    if let Some(b) = i.buffers.get(w.borrow().buffer) {
        b.borrow_mut().set_point(w.borrow().point);
        i.current_buffer = w.borrow().buffer;
        i.buffers.touch(w.borrow().buffer);
    }
    Ok(a[0].clone())
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
        Some(v) => i
            .buffer_id_of(v)
            .ok_or_else(|| i.error_obj("No such buffer", v))?,
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
        Some(v) => i
            .buffer_id_of(v)
            .ok_or_else(|| i.error_obj("No such buffer", v))?,
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

fn f_move_to_window_line(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let n = want_int(i, &a[0])?;
    let w = sel_window(i).unwrap();
    let buf = w.borrow().buffer;
    if let Some(b) = i.buffers.get(buf) {
        let bb = b.borrow();
        let line = bb.text.line_of_pos(w.borrow().start);
        let target = (line as i128 + n).max(0) as usize;
        let p = bb.text.line_start(target);
        drop(bb);
        b.borrow_mut().set_point(p);
    }
    Ok(Value::Nil)
}

fn f_pos_visible_in_window_p(i: &mut Interp, a: Vec<Value>) -> EvalResult {
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
    let w = win_of(i, &arg(&a, 0))?;
    let (l, r) = w.borrow().margins;
    if l == 0 && r == 0 {
        Ok(Value::Nil)
    } else {
        Ok(Value::cons(Value::Int(l as i128), Value::Int(r as i128)))
    }
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
        "unsplittable" | "no-accept-focus" | "tab-bar-lines"
        | "menu-bar-lines" | "buried-buffer-list" | "buffer-list" => {
            Value::Nil
        }
        _ => Value::Nil,
    })
}
fn f_frame_parameters(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let f = frame_of(i, &arg(&a, 0))?;
    let (mut out, name, w, h, mbuf, bufv) = {
        let ff = f.borrow();
        let items = ff.params.list_to_vec().unwrap_or_default();
        let mut out = Vec::new();
        let mut k = 0;
        while k + 1 < items.len() {
            out.push(Value::cons(items[k].clone(), items[k + 1].clone()));
            k += 2;
        }
        let bufv = i
            .buffer_value(ff.windows.first().map(|w| w.borrow().buffer).unwrap_or(0))
            .unwrap_or(Value::Nil);
        (out, ff.name.clone(), ff.width, ff.height, ff.minibuffer.is_some(), bufv)
    };
    let ids: Vec<SymId> = ["name", "width", "height", "modeline", "minibuffer", "buffer-list"]
        .iter()
        .map(|k| i.intern(k))
        .collect();
    let have = |kid: SymId, out: &Vec<Value>| {
        out.iter().any(|v| matches!(v, Value::Cons(c) if matches!(c.borrow().car, Value::Sym(s) if s == kid)))
    };
    if !have(ids[0], &out) {
        out.push(Value::cons(Value::Sym(ids[0]), Value::string(name)));
    }
    if !have(ids[1], &out) {
        out.push(Value::cons(Value::Sym(ids[1]), Value::Int(w as i128)));
    }
    if !have(ids[2], &out) {
        out.push(Value::cons(Value::Sym(ids[2]), Value::Int(h as i128)));
    }
    if !have(ids[3], &out) {
        out.push(Value::cons(Value::Sym(ids[3]), Value::t()));
    }
    if !have(ids[4], &out) {
        out.push(Value::cons(Value::Sym(ids[4]), Value::from_bool(mbuf)));
    }
    if !have(ids[5], &out) {
        out.push(Value::cons(Value::Sym(ids[5]), Value::list(vec![bufv])));
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

fn f_make_keymap(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let _ = &a;
    Ok(Value::cons(Value::Sym(i.intern("keymap")), Value::Nil))
}
fn f_make_sparse_keymap(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    f_make_keymap(i, a)
}
fn f_keymapp(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    Ok(Value::from_bool(is_keymap(i, &a[0])))
}
fn f_copy_keymap(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    // Shallow-copy the bindings alist (bindings themselves shared).
    if !is_keymap(i, &a[0]) {
        return Err(i.wrong_type_mut("keymapp", &a[0]));
    }
    let bindings = match &a[0] {
        Value::Cons(c) => c.borrow().cdr.clone(),
        _ => Value::Nil,
    };
    let items = bindings.list_to_vec().unwrap_or_default();
    Ok(Value::cons(
        Value::Sym(i.intern("keymap")),
        Value::list(items),
    ))
}

pub(crate) fn keymap_bindings(km: &Value) -> Value {
    match km {
        Value::Cons(c) => c.borrow().cdr.clone(),
        _ => Value::Nil,
    }
}

/// Parent keymaps of KM: elements that are themselves keymaps or a
/// proper list of keymaps (composed maps from `make-composed-keymap').
pub(crate) fn keymap_parents(i: &Interp, km: &Value) -> Vec<Value> {
    let mut out = Vec::new();
    // Improper tail: (keymap P1 . P2) chains in composed maps.
    if let Value::Cons(c) = km {
        let mut cur = c.borrow().cdr.clone();
        loop {
            match cur {
                Value::Cons(cc) => cur = cc.borrow().cdr.clone(),
                tail => {
                    if is_keymap(i, &tail) {
                        out.push(tail);
                    }
                    break;
                }
            }
        }
    }
    for el in keymap_bindings(km).list_to_vec().unwrap_or_default() {
        if is_keymap(i, &el) {
            out.push(el);
        } else if let Value::Cons(_) = &el {
            let items = el.list_to_vec().unwrap_or_default();
            if !items.is_empty() && items.iter().all(|v| is_keymap(i, v)) {
                out.extend(items);
            }
        }
    }
    out
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
        // Rebuild bindings without parent elements; the parent is the
        // first element of the map (Emacs keeps it in the head slot).
        let kept: Vec<Value> = keymap_bindings(&a[0])
            .list_to_vec()
            .unwrap_or_default()
            .into_iter()
            .filter(|el| {
                if is_keymap(i, el) {
                    return false;
                }
                if let Value::Cons(_) = el {
                    let items = el.list_to_vec().unwrap_or_default();
                    if !items.is_empty() && items.iter().all(|v| is_keymap(i, v)) {
                        return false;
                    }
                }
                true
            })
            .collect();
        c.borrow_mut().cdr = if parent.is_nil() {
            Value::list(kept)
        } else {
            Value::cons(parent.clone(), Value::list(kept))
        };
    }
    Ok(a[1].clone())
}

/// Parse a key sequence (string or vector) into event codes.
pub(crate) fn key_seq(i: &mut Interp, v: &Value) -> Result<Vec<i128>, Flow> {
    match v {
        Value::Str(s) => Ok(s.borrow().chars().map(|c| c as i128).collect()),
        Value::Vec(vec) => Ok(vec
            .borrow()
            .iter()
            .filter_map(|x| match x {
                Value::Int(n) => Some(*n),
                Value::Sym(s) => Some(event_code_for(&i.symbol_name(*s))),
                _ => None,
            })
            .collect()),
        Value::Int(n) => Ok(vec![*n]),
        Value::Cons(_) => Ok(v
            .list_to_vec()
            .unwrap_or_default()
            .iter()
            .filter_map(|x| match x {
                Value::Int(n) => Some(*n),
                Value::Sym(s) => Some(event_code_for(&i.symbol_name(*s))),
                _ => None,
            })
            .collect()),
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

fn named_key_names()
    -> &'static std::sync::Mutex<Option<std::collections::HashMap<i128, String>>>
{
    static NAMES: std::sync::Mutex<
        Option<std::collections::HashMap<i128, String>>,
    > = std::sync::Mutex::new(None);
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

pub(crate) fn lookup_in_keymap(i: &Interp, km: &Value, key: i128) -> Value {
    // Binding cell is (KEY . DEF) or a vector-ish char-table; alist model.
    // A `t` key is the default binding for otherwise-unbound events.
    // Order: exact binding > default (t) binding > parent keymaps.
    let bindings = keymap_bindings(km);
    let t_code = event_code_for("t");
    let mut found = Value::Nil;
    let mut default = Value::Nil;
    bindings.each_car(|cell| {
        if let Value::Cons(c) = cell {
            let b = c.borrow();
            let k = match &b.car {
                Value::Int(n) => Some(*n),
                Value::Sym(s) => Some(event_code_for(&i.symbol_name(*s))),
                _ => None,
            };
            match k {
                Some(k) if k == key => found = b.cdr.clone(),
                Some(k) if k == t_code => default = b.cdr.clone(),
                _ => {}
            }
        }
    });
    if !found.is_nil() {
        return found;
    }
    if !default.is_nil() {
        return default;
    }
    for p in keymap_parents(i, km) {
        let v = lookup_in_keymap(i, &p, key);
        if !v.is_nil() {
            return v;
        }
    }
    Value::Nil
}

fn f_define_key(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    if !is_keymap(i, &a[0]) {
        return Err(i.wrong_type_mut("keymapp", &a[0]));
    }
    let keys = key_seq(i, &a[1])?;
    let def = a[2].clone();
    if keys.is_empty() {
        return Ok(def);
    }
    // Descend for multi-key sequences.
    let mut km = a[0].clone();
    for &k in &keys[..keys.len() - 1] {
        let next = lookup_in_keymap(i, &km, k);
        if is_keymap(i, &next) {
            km = next;
        } else {
            let sub = Value::cons(Value::Sym(i.intern("keymap")), Value::Nil);
            set_binding(i, &km, k, sub.clone());
            km = sub;
        }
    }
    set_binding(i, &km, keys[keys.len() - 1], def.clone());
    Ok(def)
}

/// Set (KEY . DEF) in keymap's alist (prepend or replace).
pub(crate) fn set_binding(i: &mut Interp, km: &Value, key: i128, def: Value) {
    if let Value::Cons(head) = km {
        // find existing binding
        let bindings = head.borrow().cdr.clone();
        let mut cur = bindings;
        loop {
            match cur {
                Value::Cons(cell) => {
                    let (car, next) = {
                        let b = cell.borrow();
                        (b.car.clone(), b.cdr.clone())
                    };
                    if let Value::Cons(pair) = &car {
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
        // Not found: prepend (KEY . DEF). Named events store the
        // event symbol so printed maps show `down`, `menu-bar`, etc.
        let kv = key_name_for(key)
            .map(|n| Value::Sym(i.intern(&n)))
            .unwrap_or(Value::Int(key));
        let pair = Value::cons(kv, def);
        let old = head.borrow().cdr.clone();
        head.borrow_mut().cdr = Value::cons(pair, old);
    }
}

fn f_lookup_key(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    if !is_keymap(i, &a[0]) {
        return Err(i.wrong_type_mut("keymapp", &a[0]));
    }
    let keys = key_seq(i, &a[1])?;
    let mut km = a[0].clone();
    let mut used = 0usize;
    for &k in &keys {
        let def = lookup_in_keymap(i, &km, k);
        used += 1;
        if is_keymap(i, &def) {
            km = def;
        } else {
            if used < keys.len() {
                // Key sequence too long.
                return Err(i.error(&format!(
                    "Key sequence {} is too long",
                    i.princ_to_string(&a[1])
                )));
            }
            return Ok(if def.is_nil() { Value::Nil } else { def });
        }
    }
    if used < keys.len() {
        return Ok(Value::Int(used as i128)); // prefix: return count
    }
    Ok(km)
}

fn f_key_binding(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    // (key-binding KEY &optional accept-defaults no-remap position)
    // Search local map, minor-mode maps, global map.
    let keys = key_seq(i, &a[0])?;
    // local map first
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
        for &k in &keys {
            let def = lookup_in_keymap(i, &km, k);
            if is_keymap(i, &def) {
                km = def;
                continue;
            }
            if def.truthy() {
                return Ok(def);
            }
            break;
        }
    }
    // global map
    let gmap = i.symbol_value(i.intern_soft("global-map").unwrap_or(0));
    if is_keymap(i, &gmap) {
        let mut km = gmap;
        for &k in &keys {
            let def = lookup_in_keymap(i, &km, k);
            if is_keymap(i, &def) {
                km = def;
                continue;
            }
            return Ok(def);
        }
    }
    Ok(Value::Nil)
}

fn f_local_key_binding(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let keys = key_seq(i, &a[0])?;
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
        for &k in &keys {
            let def = lookup_in_keymap(i, &km, k);
            if is_keymap(i, &def) {
                km = def;
                continue;
            }
            return Ok(def);
        }
    }
    Ok(Value::Nil)
}

fn f_global_key_binding(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let keys = key_seq(i, &a[0])?;
    let gmap = i.symbol_value(i.intern_soft("global-map").unwrap_or(0));
    if is_keymap(i, &gmap) {
        let mut km = gmap;
        for &k in &keys {
            let def = lookup_in_keymap(i, &km, k);
            if is_keymap(i, &def) {
                km = def;
                continue;
            }
            return Ok(def);
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

fn f_command_remapping(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    // No remapping table yet: nil (not the command itself).
    let _ = (i, a);
    Ok(Value::Nil)
}

fn f_where_is_internal(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    // Search the local+global maps for COMMAND's binding.
    let cmd = &a[0];
    let mut found = Vec::new();
    for km_v in [
        f_current_local_map(i, vec![]).unwrap_or(Value::Nil),
        f_current_global_map(i, vec![]).unwrap_or(Value::Nil),
    ] {
        if is_keymap(i, &km_v) {
            collect_keys_for(i, &km_v, cmd, &mut Vec::new(), &mut found);
        }
    }
    // Emacs reports bindings in increasing key order (chars before
    // named events); our alist prepends, so sort for parity.
    let key_rank = |v: &Value| -> i128 {
        match v {
            Value::Vec(rc) => rc
                .borrow()
                .first()
                .map(|e| match e {
                    Value::Int(n) => *n,
                    _ => i128::MAX,
                })
                .unwrap_or(i128::MAX),
            _ => i128::MAX,
        }
    };
    found.sort_by_key(|v| key_rank(v));
    if let Some(first) = found.first() {
        if a.get(3).map(|v| v.truthy()).unwrap_or(false) {
            return Ok(first.clone());
        }
    }
    Ok(Value::list(found))
}

fn collect_keys_for(
    i: &mut Interp,
    km: &Value,
    cmd: &Value,
    prefix: &mut Vec<i128>,
    out: &mut Vec<Value>,
) {
    let bindings = keymap_bindings(km);
    let cells = bindings.list_to_vec().unwrap_or_default();
    for cell in cells {
        if let Value::Cons(c) = &cell {
            let (k, d) = {
                let b = c.borrow();
                (b.car.clone(), b.cdr.clone())
            };
            // A keymap element is a parent slot, not a binding.
            if is_keymap(i, &k) {
                continue;
            }
            let key = match &k {
                Value::Int(n) => *n,
                Value::Sym(s) => {
                    let name = i.symbol_name(*s);
                    if name == "keymap" {
                        continue;
                    }
                    event_code_for(&name)
                }
                _ => continue,
            };
            // `t` is the default binding, not a real key. Emacs
            // reports it as char ranges for self-insert-command.
            if key == event_code_for("t") {
                if eq_values(&d, cmd) {
                    for range in [(32i128, 126i128), (128i128, 4194303i128)] {
                        let cell = Value::cons(
                            Value::Int(range.0),
                            Value::Int(range.1),
                        );
                        out.push(Value::Vec(Rc::new(RefCell::new(vec![cell]))));
                    }
                }
                continue;
            }
            prefix.push(key);
            if is_keymap(i, &d) {
                collect_keys_for(i, &d, cmd, prefix, out);
            } else if eq_values(&d, cmd) {
                out.push(Value::Vec(Rc::new(RefCell::new(
                    prefix
                        .iter()
                        .map(|k| {
                            key_name_for(*k)
                                .map(|n| Value::Sym(i.intern(&n)))
                                .unwrap_or(Value::Int(*k))
                        })
                        .collect(),
                ))));
            }
            prefix.pop();
        }
    }
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

fn f_color_defined_p(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let s = want_str(i, &a[0])?;
    let _ = i;
    Ok(Value::from_bool(
        TTY_COLORS.iter().any(|c| s.eq_ignore_ascii_case(c)),
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
        // Named event: <return>, M-<left> → symbols like `M-return`.
        let name = &rest[1..rest.len() - 1];
        return vec![Value::Sym(i.intern(&format!(
            "{}{}",
            mods_name,
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
    let mut m = mods;
    let mut c = c;
    if m & CHAR_CTL != 0 && (0..128).contains(&c) {
        c = if c == 63 { 127 } else { c & 0x1f };
        m &= !CHAR_CTL;
    }
    if m & CHAR_SHIFT != 0 && (97..123).contains(&c) {
        c -= 32;
        m &= !CHAR_SHIFT;
    }
    c | m
}

fn f_key_description(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let keys = key_seq(i, &a[0])?;
    let parts: Vec<String> = keys.iter().map(|k| describe_key(*k)).collect();
    Ok(Value::string(parts.join(" ")))
}

pub(crate) fn describe_key_pub(k: i128) -> String {
    describe_key(k)
}

pub(crate) fn describe_key(k: i128) -> String {
    let mut out = String::new();
    if k & CHAR_META != 0 {
        out.push_str("M-");
    }
    if k & CHAR_CTL != 0 {
        out.push_str("C-");
    }
    if k & CHAR_SHIFT != 0 {
        out.push_str("S-");
    }
    if k & CHAR_SUPER != 0 {
        out.push_str("s-");
    }
    if k & CHAR_HYPER != 0 {
        out.push_str("H-");
    }
    if k & CHAR_ALT != 0 {
        out.push_str("A-");
    }
    let base = k & 0x3f_ffff;
    let modmask = CHAR_META | CHAR_CTL | CHAR_SHIFT | CHAR_SUPER | CHAR_HYPER | CHAR_ALT;
    let bare = k & !modmask;
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
    } else if k & 0x7fff_0000 != 0 && base == 0 {
        out.push_str("<key>");
    } else {
        match base {
            13 => out.push_str("RET"),
            9 => out.push_str("TAB"),
            32 => out.push_str("SPC"),
            27 => out.push_str("ESC"),
            127 => out.push_str("DEL"),
            c if c < 32 => {
                let ch = match c {
                    0 => '@',
                    28 => '\\',
                    29 => ']',
                    30 => '^',
                    31 => '_',
                    _ => (b'a' + c as u8 - 1) as char,
                };
                out.push_str(&format!("C-{}", ch));
            }
            c => out.push(char::from_u32(c as u32).unwrap_or('?')),
        }
    }
    out
}

fn f_single_key_description(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let k = want_int(i, &a[0])?;
    Ok(Value::string(describe_key(k)))
}

/// `substitute-command-keys` — expand `\[cmd]`, `\{map}`, `\<map>`,
/// `\=` escapes, and `'` → `’' quoting (Emacs curve style).
fn f_substitute_command_keys(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let s = want_str(i, &a[0])?;
    let chars: Vec<char> = s.chars().collect();
    let mut out = String::new();
    let mut pos = 0usize;
    // Map selected by \<name> for following \[cmd] lookups.
    let mut ctx_map: Option<Value> = None;
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
                        let keys = f_where_is_internal(i, vec![cmd.clone()]).unwrap_or(Value::Nil);
                        // Restrict to the \<map> context if one was set.
                        let first_key = if let Some(km) = &ctx_map {
                            let mut found = Vec::new();
                            if is_keymap(i, km) {
                                collect_keys_for(i, km, &cmd, &mut Vec::new(), &mut found);
                            }
                            found.into_iter().next().or_else(|| {
                                keys.list_to_vec().unwrap_or_default().into_iter().next()
                            })
                        } else {
                            keys.list_to_vec().unwrap_or_default().into_iter().next()
                        };
                        match first_key {
                            Some(k) => {
                                if let Ok(Value::Str(d)) = f_key_description(i, vec![k]) {
                                    out.push_str(&d.borrow());
                                }
                            }
                            None => {
                                out.push_str("M-x ");
                                out.push_str(&name);
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
                    if let Some((_name, next)) = take_until(&chars, pos + 2, '}') {
                        // Keymap listing not yet implemented; emits nothing.
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
        } else if c == '\'' {
            out.push('\u{2019}');
            pos += 1;
        } else {
            out.push(c);
            pos += 1;
        }
    }
    Ok(Value::string(out))
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
    crate::buffer::primitives::push_kill_ring(i, s.clone());
    Ok(Value::string(s))
}

fn f_kill_append(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let s = want_str(i, &a[0])?;
    let kr = i.intern("kill-ring");
    let cur = i.symbol_value(kr);
    let mut items = cur.list_to_vec().unwrap_or_default();
    if let Some(Value::Str(top)) = items.first_mut() {
        top.borrow_mut().push_str(&s);
    } else {
        items.insert(0, Value::string(s));
    }
    i.obarray.symbol_mut(kr).value = Value::list(items);
    Ok(Value::Nil)
}

fn f_current_kill(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let n = want_int(i, &a[0])?.max(0) as usize;
    let kr = i.intern("kill-ring");
    let items = i.symbol_value(kr).list_to_vec().unwrap_or_default();
    if items.is_empty() {
        return Err(i.error("Kill ring is empty"));
    }
    // honor kill-ring-yank-pointer rotation
    let ptr = i.symbol_value(i.intern_soft("kill-ring-yank-pointer").unwrap_or(0));
    let mut ordered = items.clone();
    if let Value::Cons(_) = ptr {
        let offset = items
            .iter()
            .position(|x| {
                eq_values(
                    x,
                    &ptr.list_to_vec()
                        .unwrap_or_default()
                        .first()
                        .cloned()
                        .unwrap_or(Value::Nil),
                )
            })
            .unwrap_or(0);
        ordered.rotate_left(offset);
    }
    let idx = n % ordered.len();
    match &ordered[idx] {
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
    b.borrow_mut().mark_active = false;
    Ok(Value::Nil)
}

fn f_yank(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let n = arg(&a, 0).int().unwrap_or(1).max(1) as usize;
    let kr = i.intern("kill-ring");
    let items = i.symbol_value(kr).list_to_vec().unwrap_or_default();
    if items.is_empty() {
        return Err(i.error("Kill ring is empty"));
    }
    let s = match &items[(n - 1) % items.len()] {
        Value::Str(s) => s.borrow().clone(),
        _ => String::new(),
    };
    let b = cur(i);
    let mut bb = b.borrow_mut();
    bb.mark = Some(bb.point());
    bb.insert(&s);
    Ok(Value::Nil)
}

fn f_yank_pop(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let n = arg(&a, 0).int().unwrap_or(1).max(1) as usize;
    let kr = i.intern("kill-ring");
    let items = i.symbol_value(kr).list_to_vec().unwrap_or_default();
    if items.is_empty() {
        return Err(i.error("Kill ring is empty"));
    }
    // Replace the region between mark and point.
    let b = cur(i);
    let mut bb = b.borrow_mut();
    if let Some(m) = bb.mark {
        let p = bb.point();
        let (s, e) = (m.min(p), m.max(p));
        bb.delete_region(s, e);
        bb.set_point(s);
        let text = match &items[n % items.len()] {
            Value::Str(s) => s.borrow().clone(),
            _ => String::new(),
        };
        bb.mark = Some(s);
        bb.insert(&text);
        Ok(Value::Nil)
    } else {
        Err(i.error("Previous command was not a yank"))
    }
}

fn f_rotate_yank_pointer(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let n = arg(&a, 0).int().unwrap_or(1);
    let kr = i.intern("kill-ring");
    let mut items = i.symbol_value(kr).list_to_vec().unwrap_or_default();
    if !items.is_empty() {
        let k = ((n % items.len() as i128) + items.len() as i128) as usize % items.len();
        items.rotate_left(k);
        i.obarray.symbol_mut(kr).value = Value::list(items);
    }
    Ok(Value::Nil)
}

fn f_copy_to_buffer(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let bid = i
        .buffer_id_of(&a[0])
        .ok_or_else(|| i.error_obj("No such buffer", &a[0]))?;
    let text = {
        let b = cur(i);
        let bb = b.borrow();
        let len = bb.text.len();
        let s = (want_int(i, &a[1])?.max(1) as usize - 1).min(len);
        let e = (want_int(i, &a[2])?.max(1) as usize - 1).min(len);
        bb.text.substring(s.min(e), s.max(e))
    };
    if let Some(b) = i.buffers.get(bid) {
        b.borrow_mut().insert(&text);
    }
    Ok(Value::Nil)
}

// ---------- file I/O ----------

fn want_filename(i: &mut Interp, v: &Value) -> Result<String, Flow> {
    let s = want_str(i, v)?;
    Ok(expand_file_name_str(i, &s))
}

/// Expand ~, env vars, and make absolute via `default-directory`.
fn expand_file_name_str(i: &mut Interp, name: &str) -> String {
    let mut s = name.to_string();
    // ~ expansion
    if s.starts_with('~') {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/".into());
        if s.len() == 1 || s.starts_with("~/") {
            s = format!("{}{}", home, &s[1..]);
        }
        // ~user unsupported → leave
    }
    // $VAR expansion
    if s.contains('$') {
        let mut out = String::new();
        let mut cs = s.chars().peekable();
        while let Some(c) = cs.next() {
            if c == '$' {
                let mut var = String::new();
                if cs.peek() == Some(&'{') {
                    cs.next();
                    while let Some(&c2) = cs.peek() {
                        if c2 == '}' {
                            cs.next();
                            break;
                        }
                        var.push(c2);
                        cs.next();
                    }
                } else {
                    while let Some(&c2) = cs.peek() {
                        if c2.is_alphanumeric() || c2 == '_' {
                            var.push(c2);
                            cs.next();
                        } else {
                            break;
                        }
                    }
                }
                if let Ok(v) = std::env::var(&var) {
                    out.push_str(&v);
                }
            } else {
                out.push(c);
            }
        }
        s = out;
    }
    // absolute?
    if !s.starts_with('/') {
        let dir = default_directory(i);
        s = format!("{}{}", dir, s);
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
fn f_file_attributes(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let p = want_filename(i, &a[0])?;
    match std::fs::metadata(&p) {
        Ok(m) => {
            let isdir = m.is_dir();
            let nlinks = 1;
            let uid = 0;
            let gid = 0;
            let size = m.len() as i128;
            let modes = 0;
            Ok(Value::list(vec![
                if isdir { Value::t() } else { Value::Nil },
                Value::Int(nlinks),
                Value::Int(uid),
                Value::Int(gid),
                Value::Nil,
                Value::Nil,
                Value::Nil,
                Value::Int(modes),
                Value::Nil,
                Value::Int(size),
                Value::Nil,
                Value::Int(0),
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
        if let Ok(m) = std::fs::metadata(&p) {
            return Ok(Value::Int((m.permissions().mode() & 0o7777) as i128));
        }
    }
    Ok(Value::Nil)
}
fn f_set_file_modes(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let p = want_filename(i, &a[0])?;
    let mode = want_int(i, &a[1])? as u32;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&p, std::fs::Permissions::from_mode(mode));
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
            normalize_path(&format!("{}{}", d.borrow(), name))
        }
        _ => expand_file_name_str(i, &name),
    };
    Ok(Value::string(s))
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
    let trimmed = s.trim_end_matches('/');
    match trimmed.rfind('/') {
        Some(idx) => Ok(Value::string(&trimmed[idx + 1..])),
        None => Ok(Value::string(trimmed)),
    }
}
fn f_file_name_extension(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let s = want_str(i, &a[0])?;
    let base = s.rsplit('/').next().unwrap_or(&s);
    match base.rfind('.') {
        Some(idx) if idx > 0 => Ok(Value::string(&base[idx..])),
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
    let dir = match a.get(1) {
        Some(Value::Str(d)) => d.borrow().clone(),
        _ => default_directory(i),
    };
    let dir = if dir.ends_with('/') {
        dir
    } else {
        format!("{}/", dir)
    };
    if let Some(rel) = name.strip_prefix(&dir) {
        Ok(Value::string(rel))
    } else {
        Ok(Value::string(name))
    }
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
fn f_substitute_in_file_name(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let s = want_str(i, &a[0])?;
    Ok(Value::string(expand_file_name_str(i, &s)))
}

fn f_directory_files(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let dir = want_filename(i, &a[0])?;
    let full = a.get(1).map(|v| v.truthy()).unwrap_or(false);
    let re_str = a.get(2).map(|v| want_str(i, v)).transpose()?;
    let nosort = a.get(3).map(|v| v.truthy()).unwrap_or(false);
    let re = match &re_str {
        Some(p) => Some(
            crate::lisp::regexp::compile_case(p, false)
                .map_err(|e| err_sym(i, "invalid-regexp", vec![Value::string(e.0)]))?,
        ),
        None => None,
    };
    let mut names = Vec::new();
    if let Ok(rd) = std::fs::read_dir(&dir) {
        for ent in rd.flatten() {
            let name = ent.file_name().to_string_lossy().into_owned();
            if let Some(r) = &re {
                let chars: Vec<char> = name.chars().collect();
                if crate::lisp::regexp::search(r, &chars, 0).is_none() {
                    continue;
                }
            }
            names.push(if full {
                format!("{}{}", dir, name)
            } else {
                name
            });
        }
    }
    if !nosort {
        names.sort();
    }
    Ok(Value::list(names.into_iter().map(Value::string).collect()))
}

fn f_directory_files_and_attributes(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    // Simplified: names + nil attributes.
    let files = f_directory_files(i, vec![a[0].clone(), arg(&a, 1), arg(&a, 2), arg(&a, 3)])?;
    let items = files.list_to_vec().unwrap_or_default();
    Ok(Value::list(
        items
            .into_iter()
            .map(|n| Value::cons(n, Value::Nil))
            .collect(),
    ))
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
        Err(e) => Err(i.signal_data(
            sym::FILE_ERROR,
            vec![Value::string(format!("Creating directory: {}", e))],
        )),
    }
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
        Err(e) => Err(i.signal_data(
            sym::FILE_ERROR,
            vec![
                Value::string(format!("Removing old name: {}", e)),
                a[0].clone(),
            ],
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
            let mut bb = b.borrow_mut();
            let _start = bb.point();
            bb.insert(&contents);
            if visit {
                bb.file_name = Some(path.clone());
                bb.modified = false;
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
        Err(e) => Err(i.signal_data(
            sym::FILE_ERROR,
            vec![
                Value::string(format!("Inserting file contents: {}", e)),
                Value::string(path),
            ],
        )),
    }
}
fn f_insert_file_contents_literally(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    f_insert_file_contents(i, a)
}

fn f_write_region(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    // (write-region START END FILENAME ...) — START may be a string.
    let filename_arg =
        if matches!(&a[0], Value::Str(_)) && a.len() >= 2 && matches!(&a[1], Value::Str(_)) {
            // (write-region STRING FILENAME) isn't Emacs's signature — keep
            // the standard form: (start end filename). If a[0] is a string
            // it's the text; a[1] is the filename.
            &a[1]
        } else {
            &a[2]
        };
    let path = want_filename(i, filename_arg)?;
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

fn write_file_string(i: &mut Interp, path: &str, text: &str, _a: &[Value]) -> EvalResult {
    match std::fs::write(path, text) {
        Ok(()) => {
            // Message: Wrote /path
            i.message(&format!("Wrote {}", path));
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
    bb.modified = false;
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
    let bid = i.buffers.create(&name);
    {
        let b = i.buffers.get(bid).unwrap();
        let mut bb = b.borrow_mut();
        bb.file_name = Some(path.clone());
        if let Some(dir_end) = path.rfind('/') {
            let dd = i.intern_soft("default-directory").unwrap_or(u32::MAX);
            bb.locals
                .insert(dd, Value::string(path[..=dir_end].to_string()));
        }
        if let Ok(contents) = std::fs::read_to_string(&path) {
            bb.text.set_text(&contents);
            bb.zv = bb.text.len();
            bb.modified = false;
        }
    }
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
            b.borrow_mut().modified = false;
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
    match std::fs::canonicalize(&p) {
        Ok(real) => Ok(Value::string(real.to_string_lossy().into_owned())),
        Err(_) => Ok(Value::string(p)),
    }
}
fn f_file_modes_default(_i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    Ok(Value::Int(0o666))
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
            return Err(i.signal_data(
                sym::FILE_ERROR,
                vec![Value::string(format!("Doing exec: {}", e))],
            ));
        }
    };
    let dest = arg(&a, 2);
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    match &dest {
        Value::Nil => {}
        Value::Int(0) => {}
        Value::Cons(_) => {
            // (BUFFER . INSERT?) — insert into BUFFER.
            if let Value::Cons(c) = &dest {
                let target = c.borrow().car.clone();
                let bid = i
                    .buffer_id_of(&target)
                    .ok_or_else(|| i.error_obj("No such buffer", &target))?;
                if let Some(b) = i.buffers.get(bid) {
                    b.borrow_mut().insert(&stdout);
                }
            }
        }
        _ => {
            // t or buffer → insert at point in current/that buffer.
            let bid = if dest.truthy() && !i.sym_id(&dest).map(|s| s == sym::T).unwrap_or(false) {
                i.buffer_id_of(&dest).unwrap_or(i.current_buffer)
            } else {
                i.current_buffer
            };
            if let Some(b) = i.buffers.get(bid) {
                b.borrow_mut().insert(&stdout);
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
            return Err(i.signal_data(
                sym::FILE_ERROR,
                vec![Value::string(format!("Doing exec: {}", e))],
            ));
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
        let b = cur(i);
        let mut bb = b.borrow_mut();
        let len = bb.text.len();
        let (s, e) = region_bounds(i, &a[0], &a[1], len);
        bb.delete_region(s.min(e), s.max(e));
        bb.insert_at(s.min(e), &stdout);
    } else {
        let dest = arg(&a, 4);
        if dest.truthy() {
            let bid = if i.sym_id(&dest).map(|s| s == sym::T).unwrap_or(false) {
                i.current_buffer
            } else {
                i.buffer_id_of(&dest).unwrap_or(i.current_buffer)
            };
            if let Some(b) = i.buffers.get(bid) {
                b.borrow_mut().insert(&stdout);
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
                if let Some(b) = i.buffers.get(bid) {
                    let mut bb = b.borrow_mut();
                    bb.text.set_text(&stdout);
                    bb.zv = bb.text.len();
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

fn f_start_process_stub(i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    Err(i.error("Asynchronous processes not yet supported"))
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
        killed = bb.delete_region(p, end);
    } else {
        killed = bb.delete_region(p, le);
    }
    drop(bb);
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
    let killed = bb.delete_region(s, e.min(tlen));
    bb.set_point(s);
    drop(bb);
    crate::buffer::primitives::push_kill_ring(i, killed);
    Ok(Value::Nil)
}

fn f_kill_word(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let n = arg(&a, 0).int().unwrap_or(1);
    let b = cur(i);
    let mut bb = b.borrow_mut();
    let start = bb.point();
    let mut p = start;
    let len = bb.text_len();
    for _ in 0..n.max(0) {
        while p < len && !bb.text.char_at(p).is_alphanumeric() {
            p += 1;
        }
        while p < len && bb.text.char_at(p).is_alphanumeric() {
            p += 1;
        }
    }
    let killed = bb.delete_region(start, p);
    drop(bb);
    crate::buffer::primitives::push_kill_ring(i, killed);
    Ok(Value::Nil)
}

fn f_backward_kill_word(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let n = arg(&a, 0).int().unwrap_or(1);
    let b = cur(i);
    let mut bb = b.borrow_mut();
    let end = bb.point();
    let mut p = end;
    for _ in 0..n.max(0) {
        while p > 0 && !bb.text.char_at(p - 1).is_alphanumeric() {
            p -= 1;
        }
        while p > 0 && bb.text.char_at(p - 1).is_alphanumeric() {
            p -= 1;
        }
    }
    let killed = bb.delete_region(p, end);
    drop(bb);
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
    bb.delete_region(s, e);
    bb.set_point(s);
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
    bb.delete_region(s, e);
    bb.set_point(s);
    bb.insert(&" ".repeat(n));
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
    bb.delete_region(s, e);
    bb.set_point(s);
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
    bb.delete_region(p, found + 1);
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
    bb.text.delete(p, p + 1);
    bb.text.insert(p, &s1);
    bb.text.delete(p - 1, p);
    bb.text.insert(p - 1, &s2);
    bb.set_point(p + 1);
    Ok(Value::Nil)
}

fn f_transpose_words(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let n = arg(&a, 0).int().unwrap_or(1);
    let _ = n;
    // Find word before and word after point.
    let b = cur(i);
    let mut bb = b.borrow_mut();
    let p = bb.point();
    let len = bb.text.len();
    // word1 = word ending around p, word2 = word starting around p
    let mut w2s = p;
    while w2s < len && !bb.text.char_at(w2s).is_alphanumeric() {
        w2s += 1;
    }
    let mut w2e = w2s;
    while w2e < len && bb.text.char_at(w2e).is_alphanumeric() {
        w2e += 1;
    }
    let mut w1e = if w2s > 0 { w2s - 1 } else { 0 };
    while w1e > 0 && !bb.text.char_at(w1e).is_alphanumeric() {
        w1e -= 1;
    }
    let mut w1s = w1e;
    while w1s > 0 && bb.text.char_at(w1s - 1).is_alphanumeric() {
        w1s -= 1;
    }
    if w1s >= w2e || w2s >= len {
        return Err(i.error("Don't have two things to transpose"));
    }
    let w1 = bb.text.substring(w1s, w1e + 1);
    let mid = bb.text.substring(w1e + 1, w2s);
    let w2 = bb.text.substring(w2s, w2e);
    bb.text.delete(w1s, w2e);
    bb.text.insert(w1s, &format!("{}{}{}", w2, mid, w1));
    bb.set_point(w2e);
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
    bb.text.delete(l1s, l2e);
    bb.text.insert(l1s, &format!("{}\n{}", l2, l1));
    Ok(Value::Nil)
}

fn region_op(i: &mut Interp, a: &[Value], op: fn(&str) -> String) -> EvalResult {
    let b = cur(i);
    let mut bb = b.borrow_mut();
    let len = bb.text.len();
    let (s, e) = region_bounds(i, &a[0], &a[1], len);
    let (s, e) = (s.min(e), s.max(e));
    let old = bb.text.substring(s, e);
    let new = op(&old);
    bb.delete_region(s, e);
    bb.insert_at(s, &new);
    Ok(Value::Nil)
}

fn f_upcase_region(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    region_op(i, &a, |s| s.to_uppercase())
}
fn f_downcase_region(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    region_op(i, &a, |s| s.to_lowercase())
}
fn f_capitalize_region(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    region_op(i, &a, |s| {
        let mut out = String::with_capacity(s.len());
        let mut in_word = false;
        for c in s.chars() {
            if c.is_alphanumeric() {
                if in_word {
                    out.push(c.to_lowercase().next().unwrap_or(c));
                } else {
                    out.push(c.to_uppercase().next().unwrap_or(c));
                    in_word = true;
                }
            } else {
                in_word = false;
                out.push(c);
            }
        }
        out
    })
}

fn word_op(i: &mut Interp, a: &[Value], op: fn(&str) -> String) -> EvalResult {
    let n = arg(a, 0).int().unwrap_or(1);
    let b = cur(i);
    let mut bb = b.borrow_mut();
    let start = bb.point();
    let mut p = start;
    let len = bb.text_len();
    for _ in 0..n.max(0) {
        while p < len && !bb.text.char_at(p).is_alphanumeric() {
            p += 1;
        }
        while p < len && bb.text.char_at(p).is_alphanumeric() {
            p += 1;
        }
    }
    let old = bb.text.substring(start, p);
    let new = op(&old);
    bb.delete_region(start, p);
    bb.insert_at(start, &new);
    bb.set_point(start + new.chars().count());
    Ok(Value::Nil)
}

fn f_upcase_word(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    word_op(i, &a, |s| s.to_uppercase())
}
fn f_downcase_word(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    word_op(i, &a, |s| s.to_lowercase())
}
fn f_capitalize_word(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    word_op(i, &a, |s| {
        let mut cs = s.chars();
        match cs.next() {
            Some(c) => c.to_uppercase().collect::<String>() + &cs.as_str().to_lowercase(),
            None => String::new(),
        }
    })
}

fn f_indent_line_to(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let col = want_int(i, &a[0])?.max(0) as usize;
    let b = cur(i);
    let mut bb = b.borrow_mut();
    let line = bb.text.line_of_pos(bb.point());
    let ls = bb.text.line_start(line);
    // Delete existing leading whitespace, insert `col` spaces.
    let mut e = ls;
    while e < bb.text.len() && matches!(bb.text.char_at(e), ' ' | '\t') {
        e += 1;
    }
    bb.delete_region(ls, e);
    bb.insert_at(ls, &" ".repeat(col));
    bb.set_point(ls + col);
    Ok(Value::Nil)
}

fn f_indent_to(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let col = want_int(i, &a[0])?.max(0) as usize;
    let b = cur(i);
    let mut bb = b.borrow_mut();
    let p = bb.point();
    // Column of point.
    let ls = bb.text.line_start(bb.text.line_of_pos(p));
    let cur_col = p - ls;
    if cur_col < col {
        bb.insert(&" ".repeat(col - cur_col));
    }
    Ok(Value::Int(col as i128))
}

fn f_indent_rigidly(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let len = cur(i).borrow().text.len();
    let (s, e) = region_bounds(i, &a[0], &a[1], len);
    let col = want_int(i, &a[2])?;
    let b = cur(i);
    let mut bb = b.borrow_mut();
    // For each line in [s,e): insert col spaces (or delete -col cols).
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
    Ok(Value::Nil)
}

fn f_delete_trailing_whitespace(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let b = cur(i);
    let mut bb = b.borrow_mut();
    let len = bb.text.len();
    let (s, e) = match (a.get(0), a.get(1)) {
        (Some(sv), Some(ev)) if sv.truthy() => region_bounds(i, sv, ev, len),
        _ => (0, len),
    };
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
    let b = cur(i);
    let mut bb = b.borrow_mut();
    let mut pos = s;
    while pos < e && pos < bb.text.len() {
        if bb.text.char_at(pos) == '\t' {
            let ls = bb.text.line_start(bb.text.line_of_pos(pos));
            let col = pos - ls;
            let spaces = tab_width - (col % tab_width);
            bb.text.delete(pos, pos + 1);
            bb.text.insert(pos, &" ".repeat(spaces));
            pos += spaces;
        } else {
            pos += 1;
        }
    }
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

fn f_next_line(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let n = arg(&a, 0).int().unwrap_or(1);
    let b = cur(i);
    let mut bb = b.borrow_mut();
    let line = bb.text.line_of_pos(bb.point());
    let ls = bb.text.line_start(line);
    let col = bb.point() - ls;
    let target = line + n.max(0) as usize;
    let ts = bb.text.line_start(target);
    let te = bb.text.line_end(ts);
    bb.set_point((ts + col).min(te));
    Ok(Value::Nil)
}

fn f_previous_line(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let n = arg(&a, 0).int().unwrap_or(1);
    f_next_line(i, vec![Value::Int(-n)])
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
        if let Some(b) = i.buffers.get(id) {
            let mut bb = b.borrow_mut();
            let tlen = bb.text.len();
            bb.text.delete(0, tlen);
            bb.set_point(0);
        }
    }
    Ok(Value::Nil)
}

fn f_minibuffer_depth(i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    Ok(Value::Int(i.minibuf_level as i128))
}
fn f_minibuffer_prompt(_i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    Ok(Value::string(""))
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
fn minibuf_or(i: &mut Interp, prompt: &Value, fallback: Value) -> Result<Option<String>, Flow> {
    if i.minibuf_reader.is_none() {
        return Ok(None);
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
fn f_read_number(_i: &mut Interp, a: Vec<Value>) -> EvalResult {
    match a.get(1) {
        Some(v) => Ok(v.clone()),
        _ => Ok(Value::Int(0)),
    }
}
fn f_read_regexp(_i: &mut Interp, a: Vec<Value>) -> EvalResult {
    match a.get(1) {
        Some(Value::Str(_s)) => Ok(a[1].clone()),
        _ => Ok(Value::string("")),
    }
}
fn f_completing_read(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    // (completing-read PROMPT TABLE ...) — interactive: read a line and
    // complete it against TABLE; batch: use initial-input or default.
    if i.minibuf_reader.is_some() {
        let prompt = match &a[0] {
            Value::Str(s) => s.borrow().clone(),
            _ => String::new(),
        };
        let cands = completion_candidates(i, &a[1]);
        let input = i.minibuf_line(&prompt)?;
        if input.is_empty() {
            // Empty input → default (arg 3) or "".
            return Ok(arg(&a, 3));
        }
        // Complete: exact match, else unique prefix completion.
        if cands.iter().any(|c| c == &input) {
            return Ok(Value::string(input));
        }
        let matches: Vec<&String> = cands.iter().filter(|c| c.starts_with(&input)).collect();
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
    match try_completions(i, "", &a[1]) {
        Ok(v) => Ok(v),
        Err(_) => Ok(arg(&a, 4)),
    }
}

/// Resolve a completion TABLE to a list of candidate strings.
fn completion_candidates(i: &mut Interp, table: &Value) -> Vec<String> {
    match table {
        Value::Cons(_) => {
            let items = table.list_to_vec().unwrap_or_default();
            items
                .iter()
                .map(|v| match v {
                    Value::Str(s) => s.borrow().clone(),
                    Value::Sym(s) => i.symbol_name(*s),
                    Value::Cons(c) => {
                        let b = c.borrow();
                        i.princ_to_string(&b.car)
                    }
                    _ => String::new(),
                })
                .collect()
        }
        Value::Vec(v) => v.borrow().iter().map(|x| i.princ_to_string(x)).collect(),
        Value::Nil => Vec::new(),
        _ => Vec::new(),
    }
}

fn f_try_completion(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let s = want_str(i, &a[0])?;
    try_completions(i, &s, &a[1])
}

fn try_completions(i: &mut Interp, s: &str, table: &Value) -> EvalResult {
    let cands = completion_candidates(i, table);
    let matches: Vec<String> = cands.into_iter().filter(|c| c.starts_with(s)).collect();
    if matches.is_empty() {
        return Ok(Value::Nil);
    }
    if matches.len() == 1 && matches[0] == s {
        return Ok(Value::t());
    }
    let lcp = longest_common_prefix(&matches);
    if lcp == s && matches.iter().any(|m| m == &lcp) {
        Ok(Value::t())
    } else {
        Ok(Value::string(lcp))
    }
}

fn f_all_completions(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let s = want_str(i, &a[0])?;
    let cands = completion_candidates(i, &a[1]);
    Ok(Value::list(
        cands
            .into_iter()
            .filter(|c| c.starts_with(&s))
            .map(Value::string)
            .collect(),
    ))
}

fn f_test_completion(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let s = want_str(i, &a[0])?;
    let cands = completion_candidates(i, &a[1]);
    Ok(Value::from_bool(cands.iter().any(|c| c == &s)))
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
    Ok(Value::t())
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

fn f_prefix_numeric_value(_i: &mut Interp, a: Vec<Value>) -> EvalResult {
    match &a[0] {
        Value::Nil => Ok(Value::Int(1)),
        Value::Cons(_) => {
            // (4) → 4, (16) → 16, (-) → -1
            let car = match &a[0] {
                Value::Cons(c) => c.borrow().car.clone(),
                v => v.clone(),
            };
            match car {
                Value::Int(n) => Ok(Value::Int(n)),
                _ => Ok(Value::Int(-1)),
            }
        }
        Value::Int(n) => Ok(Value::Int(*n)),
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

fn f_beginning_of_defun(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let n = arg(&a, 0).int().unwrap_or(1);
    let b = cur(i);
    let mut bb = b.borrow_mut();
    for _ in 0..n.max(0) {
        // Find previous '(' at column 0.
        let mut p = bb.point();
        let mut found = None;
        while p > 0 {
            p -= 1;
            if bb.text.char_at(p) == '(' && (p == 0 || bb.text.char_at(p - 1) == '\n') {
                found = Some(p);
                break;
            }
        }
        match found {
            Some(x) => bb.set_point(x),
            None => {
                let bv = bb.begv;
                bb.set_point(bv);
                break;
            }
        }
    }
    Ok(Value::Nil)
}

fn f_end_of_defun(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let n = arg(&a, 0).int().unwrap_or(1);
    let b = cur(i);
    let mut bb = b.borrow_mut();
    for _ in 0..n.max(0) {
        // scan forward past balanced '(...)'
        let mut p = bb.point();
        let len = bb.text_len();
        // move to next '(' at col 0 then scan to its close
        while p < len && !(bb.text.char_at(p) == '(' && (p == 0 || bb.text.char_at(p - 1) == '\n'))
        {
            p += 1;
        }
        if p >= len {
            bb.set_point(len);
            break;
        }
        let mut depth = 0i128;
        let mut k = p;
        while k < len {
            match bb.text.char_at(k) {
                '(' => depth += 1,
                ')' => {
                    depth -= 1;
                    if depth == 0 {
                        break;
                    }
                }
                _ => {}
            }
            k += 1;
        }
        bb.set_point((k + 1).min(len));
    }
    Ok(Value::Nil)
}

fn f_mark_defun(i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    let _ = i;
    Ok(Value::Nil)
}
fn f_narrow_to_defun(i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    let _ = i;
    Ok(Value::Nil)
}

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

fn f_char_syntax(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let c = want_int(i, &a[0])? as u32;
    match char::from_u32(c) {
        Some(ch) => Ok(Value::Int(crate::lisp::regexp::syntax_code(ch) as i128)),
        None => Ok(Value::Nil),
    }
}

/// A syntax table is a char-table (#s(char-table syntax-table VEC)).
fn new_syntax_table(i: &mut Interp) -> Value {
    let vec = Value::Vec(Rc::new(RefCell::new(vec![Value::Nil; 256])));
    Value::Record(Rc::new(RefCell::new(vec![
        Value::Sym(i.intern("char-table")),
        Value::Sym(i.intern("syntax-table")),
        vec,
    ])))
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

fn f_make_syntax_table(i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    Ok(new_syntax_table(i))
}

fn f_syntax_table_p(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    Ok(Value::from_bool(is_syntax_table(i, &a[0])))
}

fn f_standard_syntax_table(i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    let sid = i.intern("remacs--standard-syntax-table");
    let cur = i.symbol_value(sid);
    if is_syntax_table(i, &cur) {
        return Ok(cur);
    }
    let t = new_syntax_table(i);
    let _ = i.set_symbol(sid, t.clone());
    Ok(t)
}

fn f_syntax_table(i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    // Buffer-local `syntax-table' variable wins over the standard one.
    let sid = i.intern("syntax-table");
    let cur = i.symbol_value(sid);
    if is_syntax_table(i, &cur) {
        return Ok(cur);
    }
    f_standard_syntax_table(i, vec![])
}

fn f_set_syntax_table(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    if !is_syntax_table(i, &a[0]) {
        return Err(i.wrong_type_mut("syntax-table-p", &a[0]));
    }
    let sid = i.intern("syntax-table");
    // Force buffer-local.
    if let Some(b) = i.buffers.get(i.current_buffer) {
        if let Ok(mut bb) = b.try_borrow_mut() {
            bb.locals.insert(sid, a[0].clone());
            return Ok(a[0].clone());
        }
    }
    let _ = i.set_symbol(sid, a[0].clone());
    Ok(a[0].clone())
}

fn f_copy_syntax_table(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let src_t = match &arg(&a, 0) {
        Value::Nil => f_syntax_table(i, vec![])?,
        v if is_syntax_table(i, v) => v.clone(),
        other => return Err(i.wrong_type_mut("syntax-table-p", other)),
    };
    if let Value::Record(r) = &src_t {
        let rr = r.borrow();
        if let Some(Value::Vec(v)) = rr.get(2) {
            let new_vec =
                Value::Vec(Rc::new(RefCell::new(v.borrow().clone())));
            return Ok(Value::Record(Rc::new(RefCell::new(vec![
                Value::Sym(i.intern("char-table")),
                Value::Sym(i.intern("syntax-table")),
                new_vec,
            ]))));
        }
    }
    Ok(src_t)
}

fn f_parse_partial_sexp(i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    // Return a plausible parse state: (depth in-parens in-string ...)
    let (depth, in_str, in_comment, quote) = {
        let b = cur(i);
        let bb = b.borrow();
        let mut depth = 0i128;
        let mut in_str = false;
        let mut in_comment = false;
        let mut quote = false;
        let end = bb.point();
        let mut k = 0;
        let mut esc = false;
        while k < end.min(bb.text.len()) {
            let c = bb.text.char_at(k);
            if esc {
                esc = false;
            } else if in_comment {
                if c == '\n' {
                    in_comment = false;
                }
            } else if in_str {
                if c == '\\' {
                    esc = true;
                } else if c == '"' {
                    in_str = false;
                }
            } else {
                match c {
                    ';' => in_comment = true,
                    '"' => in_str = true,
                    '\'' => quote = true,
                    '(' | '[' | '{' => depth += 1,
                    ')' | ']' | '}' => depth -= 1,
                    _ => quote = false,
                }
            }
            k += 1;
        }
        (depth, in_str, in_comment, quote)
    };
    Ok(Value::list(vec![
        Value::Int(depth),
        Value::Nil,
        Value::Nil,
        Value::from_bool(in_str),
        Value::from_bool(in_comment),
        Value::from_bool(quote),
        Value::Nil,
        Value::Nil,
        Value::Nil,
        Value::Nil,
        Value::Nil,
    ]))
}

fn f_syntax_ppss(i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    f_parse_partial_sexp(i, vec![])
}

// ---------- modes ----------

fn f_fundamental_mode(i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    let mm = i.intern("major-mode");
    let fmid = i.intern("fundamental-mode");
    cur(i).borrow_mut().locals.insert(mm, Value::Sym(fmid));
    Ok(Value::Sym(fmid))
}

fn f_normal_mode(i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    f_fundamental_mode(i, vec![])
}

fn f_run_mode_hooks(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    for hook in a {
        let id = match i.sym_id(&hook) {
            Some(s) => s,
            None => continue,
        };
        let v = i.symbol_value(id);
        let fns = match &v {
            Value::Cons(_) => v.list_to_vec().unwrap_or_default(),
            Value::Nil => Vec::new(),
            other => vec![other.clone()],
        };
        for f in fns {
            i.apply(&f, vec![])?;
        }
    }
    Ok(Value::Nil)
}

fn f_with_timeout_raw(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    // (with-timeout (SECONDS FORMS...) BODY...) — run body; timeout not
    // enforced yet (single-threaded eval can't preempt itself).
    let args = a.into_iter().next().unwrap_or(Value::Nil);
    let spec = match &args {
        Value::Cons(c) => c.borrow().car.clone(),
        _ => Value::Nil,
    };
    let timeout_forms = match &spec {
        Value::Cons(c) => c.borrow().cdr.clone(),
        _ => Value::Nil,
    };
    let body = match &args {
        Value::Cons(c) => c.borrow().cdr.clone(),
        _ => Value::Nil,
    };
    let timeout_sym = i.intern("timeout");
    match i.eval_progn(&body) {
        Err(Flow::Throw(tag, _)) if i.sym_is(&tag, timeout_sym) => i.eval_progn(&timeout_forms),
        other => other,
    }
}

// ---------- overlays ----------

fn f_make_overlay(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let bid = match a.get(2) {
        Some(v) if v.truthy() => i
            .buffer_id_of(v)
            .ok_or_else(|| i.error_obj("No such buffer", v))?,
        _ => i.current_buffer,
    };
    let len = i
        .buffers
        .get(bid)
        .map(|b| b.borrow().text.len())
        .unwrap_or(0);
    let s = (want_int(i, &a[0])?.max(1) as usize - 1).min(len);
    let e = (want_int(i, &a[1])?.max(1) as usize - 1).min(len);
    // Overlays are stored on the buffer; a Lisp handle is a cons
    // `(overlay BEG END . plist)`-like — use a dedicated marker pair:
    // simplest is a cons tagged with a gensym'd id.
    let b = i.buffers.get(bid).unwrap();
    let mut bb = b.borrow_mut();
    let ov = crate::buffer::Overlay {
        start: s,
        end: e,
        plist: Value::Nil,
    };
    bb.overlays.push(ov);
    let idx = bb.overlays.len() - 1;
    // Represent the overlay as (overlay MARKER . MARKER)-ish: we use a
    // vector [overlay buffer-id index].
    Ok(Value::Vec(Rc::new(RefCell::new(vec![
        Value::Sym(i.intern("overlay")),
        Value::Int(bid as i128),
        Value::Int(idx as i128),
    ]))))
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

fn f_delete_overlay(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let (bid, idx) = overlay_of(i, &a[0])?;
    if let Some(b) = i.buffers.get(bid) {
        let mut bb = b.borrow_mut();
        if idx < bb.overlays.len() {
            bb.overlays[idx].start = bb.overlays[idx].end;
        }
    }
    Ok(Value::Nil)
}

fn f_move_overlay(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let (bid, idx) = overlay_of(i, &a[0])?;
    if let Some(b) = i.buffers.get(bid) {
        let mut bb = b.borrow_mut();
        if idx < bb.overlays.len() {
            let len = bb.text.len();
            let s = (want_int(i, &a[1])?.max(1) as usize - 1).min(len);
            let e = (want_int(i, &a[2])?.max(1) as usize - 1).min(len);
            bb.overlays[idx].start = s;
            bb.overlays[idx].end = e;
        }
    }
    Ok(a[0].clone())
}

fn f_overlay_start(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let (bid, idx) = overlay_of(i, &a[0])?;
    Ok(match i.buffers.get(bid) {
        Some(b) if idx < b.borrow().overlays.len() => {
            Value::Int(b.borrow().overlays[idx].start as i128 + 1)
        }
        _ => Value::Nil,
    })
}
fn f_overlay_end(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let (bid, idx) = overlay_of(i, &a[0])?;
    Ok(match i.buffers.get(bid) {
        Some(b) if idx < b.borrow().overlays.len() => {
            Value::Int(b.borrow().overlays[idx].end as i128 + 1)
        }
        _ => Value::Nil,
    })
}
fn f_overlay_buffer(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let (bid, _idx) = overlay_of(i, &a[0])?;
    Ok(i.buffer_value(bid).unwrap_or(Value::Nil))
}
fn f_overlay_put(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let (bid, idx) = overlay_of(i, &a[0])?;
    let ps = want_sym(i, &a[1])?;
    if let Some(b) = i.buffers.get(bid) {
        let mut bb = b.borrow_mut();
        if idx < bb.overlays.len() {
            let new = crate::lisp::eval::plist_put(&bb.overlays[idx].plist, ps, a[2].clone());
            bb.overlays[idx].plist = new;
        }
    }
    Ok(a[2].clone())
}
fn f_overlay_get(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let (bid, idx) = overlay_of(i, &a[0])?;
    let ps = want_sym(i, &a[1])?;
    if let Some(b) = i.buffers.get(bid) {
        let bb = b.borrow();
        if idx < bb.overlays.len() {
            return Ok(crate::lisp::eval::plist_get(&bb.overlays[idx].plist, ps));
        }
    }
    Ok(Value::Nil)
}
fn f_overlay_properties(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let (bid, idx) = overlay_of(i, &a[0])?;
    if let Some(b) = i.buffers.get(bid) {
        let bb = b.borrow();
        if idx < bb.overlays.len() {
            return Ok(bb.overlays[idx].plist.clone());
        }
    }
    Ok(Value::Nil)
}
fn f_overlayp(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    Ok(Value::from_bool(overlay_of(i, &a[0]).is_ok()))
}
fn f_overlays_at(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let pos = want_int(i, &a[0])?.max(1) as usize - 1;
    let b = cur(i);
    let bb = b.borrow();
    let bid = bb.id;
    let mut out = Vec::new();
    for (idx, ov) in bb.overlays.iter().enumerate() {
        if pos >= ov.start && pos < ov.end {
            out.push(Value::Vec(Rc::new(RefCell::new(vec![
                Value::Sym(i.intern("overlay")),
                Value::Int(bid as i128),
                Value::Int(idx as i128),
            ]))));
        }
    }
    Ok(Value::list(out))
}
fn f_overlays_in(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let b = cur(i);
    let bb = b.borrow();
    let len = bb.text.len();
    let bid = bb.id;
    let s = (want_int(i, &a[0])?.max(1) as usize - 1).min(len);
    let e = (want_int(i, &a[1])?.max(1) as usize - 1).min(len);
    let mut out = Vec::new();
    for (idx, ov) in bb.overlays.iter().enumerate() {
        if ov.start < e && ov.end > s {
            out.push(Value::Vec(Rc::new(RefCell::new(vec![
                Value::Sym(i.intern("overlay")),
                Value::Int(bid as i128),
                Value::Int(idx as i128),
            ]))));
        }
    }
    Ok(Value::list(out))
}
fn f_overlays_at_point(i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    let pos = cur(i).borrow().point() as i128 + 1;
    f_overlays_at(i, vec![Value::Int(pos)])
}
fn f_next_overlay_change(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let pos = want_int(i, &a[0])?.max(1) as usize - 1;
    let b = cur(i);
    let bb = b.borrow();
    let mut next: Option<usize> = None;
    for ov in &bb.overlays {
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
    let mut prev: Option<usize> = None;
    for ov in &bb.overlays {
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
    let mut bb = b.borrow_mut();
    let len = bb.text.len();
    let s = match a.get(0) {
        Some(v) if v.truthy() => (want_int(i, v)?.max(1) as usize - 1).min(len),
        _ => 0,
    };
    let e = match a.get(1) {
        Some(v) if v.truthy() => (want_int(i, v)?.max(1) as usize - 1).min(len),
        _ => len,
    };
    bb.overlays.retain(|ov| !(ov.start < e && ov.end > s));
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
        let chars: Vec<char> = name.chars().collect();
        if crate::lisp::regexp::search(&re, &chars, 0).is_some() {
            out.push(i.sym(id));
        }
    }
    Ok(Value::list(out))
}

/// Look up a key sequence (as Int event codes) in the active maps:
/// local map, then the global map. Returns the bound value, or None.
pub(crate) fn lookup_command_in_maps(i: &mut Interp, keys: &[Value]) -> Option<Value> {
    let codes: Vec<i128> = keys.iter().filter_map(|v| v.int()).collect();
    for km_v in [
        {
            let b = i.current_buffer_ref()?;
            let lb = b.borrow();
            lb.locals
                .get(&i.intern_soft("local-keymap").unwrap_or(u32::MAX))
                .cloned()
                .unwrap_or(Value::Nil)
        },
        i.symbol_value(i.intern_soft("global-map").unwrap_or(0)),
    ] {
        if !is_keymap(i, &km_v) {
            continue;
        }
        let mut km = km_v;
        let mut last_def = Value::Nil;
        let mut prefix_only = false;
        for &k in &codes {
            let def = lookup_in_keymap(i, &km, k);
            if is_keymap(i, &def) {
                km = def;
                prefix_only = true;
                last_def = Value::Nil;
            } else {
                last_def = def;
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
    // global-map default (empty sparse keymap).
    let gm = i.intern("global-map");
    if i.symbol_value(gm).is_nil() {
        let km = Value::cons(Value::Sym(i.intern("keymap")), Value::Nil);
        i.obarray.symbol_mut(gm).value = km;
    }
}
