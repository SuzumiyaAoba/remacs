//! Editor layer: windows, frames, keymaps, kill-ring, file I/O,
//! and editing commands built on the buffer primitives.
//!
//! Windows/frames are Lisp objects (`Value::Window`, `Value::Frame`)
//! shared via `Rc<RefCell<_>>` so Lisp code can hold references while
//! the front-end mutates geometry.

use crate::lisp::text::{EmacsChar, EmacsStr, EmacsText, TextSource};
use std::cell::RefCell;
use std::rc::Rc;

use crate::lisp::Interp;
use crate::lisp::builtins::listfn::nthcdr_of;
use crate::lisp::builtins::{eq_values, equal_values};
use crate::lisp::error::{EvalResult, Flow};
use crate::lisp::obarray::sym;
use crate::lisp::value::{StrRef, Subr, SymId, Value};

std::thread_local! {
    // Native configuration identity must not be forgeable by a record tag.
    // Weak handles neither keep snapshots alive nor allow pointer reuse.
    static WINDOW_CONFIGURATIONS: RefCell<std::collections::HashMap<usize, std::rc::Weak<RefCell<Vec<Value>>>>> = RefCell::new(std::collections::HashMap::new());
}

pub(crate) fn window_configuration_object(value: &Value) -> bool {
    let Value::Record(record) = value else {
        return false;
    };
    WINDOW_CONFIGURATIONS.with(|objects| {
        objects
            .borrow()
            .get(&(Rc::as_ptr(record) as usize))
            .and_then(std::rc::Weak::upgrade)
            .is_some_and(|object| Rc::ptr_eq(&object, record))
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum VerticalScrollBarType {
    FrameDefault,
    Disabled,
    Left,
    Right,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum HorizontalScrollBarType {
    FrameDefault,
    Disabled,
    Bottom,
}

fn register_window_configuration(value: &Value) {
    if let Value::Record(record) = value {
        WINDOW_CONFIGURATIONS.with(|objects| {
            let mut objects = objects.borrow_mut();
            objects.retain(|_, object| object.strong_count() > 0);
            objects.insert(Rc::as_ptr(record) as usize, Rc::downgrade(record));
        });
    }
}

pub(crate) mod winxtra;
pub(crate) mod changes;

/// An editor window: a viewport into a buffer, or an internal
/// combination window in the frame's window tree (GNU's vc/hc
/// parents — no buffer, children instead).
pub struct Window {
    pub id: usize,
    /// Displayed buffer id (garbage on internal windows).
    pub buffer: usize,
    /// Point when this window is selected (0-based).
    pub point: usize,
    /// First visible char position (0-based).
    pub start: usize,
    /// End position recorded by the last redisplay for this buffer/start.
    /// The text modification tick invalidates the cached endpoint.
    pub redisplay_end: Option<(usize, u64, usize, usize)>,
    /// Horizontal scroll column.
    pub hscroll: usize,
    /// Screen layout (terminal cells, set by the front-end).
    pub top: usize,
    pub height: usize,
    pub left: usize,
    pub width: usize,
    /// GNU normal_lines/normal_cols: intended height/width fractions,
    /// retained independently of integer cell rounding during layout.
    pub normal_size: (f64, f64),
    /// GNU `suspend_auto_hscroll': set by `scroll-left'/-right'/
    /// `set-window-hscroll'; auto-hscroll stays off until point
    /// changes (compared against `old_point' at redisplay).
    pub suspend_hscroll: bool,
    /// Suppress automatic vertical scrolling until WINDOW's point moves.
    pub suspend_vscroll: bool,
    /// Last redisplay position, used as a cache before a marker is attached.
    pub old_point: usize,
    /// GNU old_pointm: the past point position, tracking buffer edits.
    pub old_point_marker: Value,
    /// GNU `min_hscroll': lower bound for auto-hscroll, set by
    /// interactive `scroll-left'/`-right' (SET-MINIMUM arg).
    pub min_hscroll: usize,
    /// Lisp dedication flag: only t refuses explicit buffer switches.
    pub dedicated: Value,
    /// Minibuffer windows are special.
    pub minibuffer: bool,
    /// Window parameters in a flat plist (GNU exposes an alist).
    pub params: Value,
    /// Fringes/margins.
    pub margins: (usize, usize),
    /// Per-window fringe widths in pixels and placement flags.
    pub fringes: Option<(Option<usize>, Option<usize>, bool, bool)>,
    /// Per-window vertical/horizontal scroll bar geometry in pixels.
    pub scroll_bars: Option<(
        Option<usize>,
        VerticalScrollBarType,
        Option<usize>,
        HorizontalScrollBarType,
        bool,
    )>,
    /// `window-use-time': tick of the last creation/selection.
    pub use_time: u64,
    /// Buffers previously shown here — list of
    /// `(buffer start-marker point-marker)' triples, newest first.
    pub prev_buffers: Value,
    /// Buffers recorded by `unrecord-window-buffer'/quit-restore.
    pub next_buffers: Value,
    /// Internal window direction: Some(true) = children side-by-side
    /// (GNU `hc'), Some(false) = stacked (`vc'), None = leaf window.
    pub horiz: Option<bool>,
    /// Children of an internal window, in display order.
    pub children: Vec<WindowRef>,
    /// Parent internal window; None for the frame root and the
    /// minibuffer window.
    pub parent: Option<std::rc::Weak<RefCell<Window>>>,
    /// `window-combination-limit' flag (internal windows only).
    pub combination_limit: bool,
    pub dead: bool,
    /// GNU `new_total'/`new_pixel': pending sizes set by
    /// `set-window-configuration'/resize and cleared by redisplay.
    /// Batch GNU reports the value left by the last config restore
    /// (22 = height minus menu+mode rows), never recomputed.
    pub new_total: i128,
    pub new_pixel: i128,
    /// GNU `new_normal': pending normal size stored by
    /// `set-window-new-normal' — any Lisp value (float ok), init 0.
    pub new_normal: Value,
    /// Buffer recorded by the previous window-change notification.
    pub old_buffer: Value,
    pub(crate) change_stamp: u64,
    pub(crate) change_frame: Option<usize>,
    pub(crate) change_clock: Option<Rc<std::cell::Cell<u64>>>,
    pub(crate) change_size: Option<[usize; 5]>,
    /// GNU `w->cursor_type': per-window cursor override, t = follow
    /// the buffer's `cursor-type' variable.  Set by
    /// `set-window-cursor-type'.
    pub cursor_type: Value,
}

pub type WindowRef = Rc<RefCell<Window>>;

/// An editor frame (a terminal or GUI frame).
pub struct Frame {
    pub id: usize,
    /// The Lisp name string, shared so mutations and text properties
    /// remain visible to frame reporting, printing, and name allocation.
    pub name: StrRef,
    /// Live leaf windows in display order (internal windows live in
    /// the tree hanging off `root', NOT here).
    pub windows: Vec<WindowRef>,
    pub selected: WindowRef,
    /// The minibuffer window (echo area in ttys).
    pub minibuffer: Option<WindowRef>,
    /// The frame's root window: a leaf while unsplit (GNU: that leaf
    /// IS the root), an internal vc/hc window once split.
    pub root: Option<WindowRef>,
    /// GNU batch quirk: the first split materializes the menu-bar
    /// line — window tops shift down one row (`window-edges' report
    /// the shifted geometry while the *inside* edges keep the
    /// pre-shift pixel rows).
    pub menu_shift: bool,
    /// GNU `FRAME_TOP_MARGIN' (menu-bar + tab-bar lines on a tty):
    /// materialized into window geometry by `resize_frame_windows'
    /// (`set-window-configuration', first split) and subtracted from
    /// `frame-text-height'/'frame-text-lines'.  0 until the first
    /// restore/split re-lays the frame.
    pub text_margin: usize,
    pub width: usize,
    pub height: usize,
    /// GNU `old_selected_window': window selected before the last
    /// frame switch; nil until the first `do_switch_frame'.
    pub old_selected: Option<WindowRef>,
    pub(crate) change_stamp: Rc<std::cell::Cell<u64>>,
    pub(crate) change_window_count: usize,
    pub(crate) change_selected: Option<usize>,
    pub(crate) change_frame_selected: bool,
    pub(crate) change_size: Option<(usize, usize)>,
    pub(crate) window_state_change: bool,
    /// Buffers selected on this frame, most recently selected first.
    pub buffer_list: Vec<usize>,
    /// Buffers buried on this frame, most recently buried first.
    pub buried_buffer_list: Vec<usize>,
    pub params: Value,
    pub dead: bool,
}

pub type FrameRef = Rc<RefCell<Frame>>;

static NEXT_ID: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(1);
fn next_id() -> usize {
    NEXT_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
}
/// `frame-id' is a per-frame ordinal (GNU frame.c: F1 → 1, F2 → 2, …)
/// — NOT shared with the window-id counter, or the first tty frame
/// reports 3 because two windows were allocated before it.
static NEXT_FRAME_ID: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(1);
pub(crate) fn next_frame_id() -> usize {
    NEXT_FRAME_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
}

/// Advance GNU's selection counter within this editor instance.
pub(crate) fn next_use_time(i: &mut Interp) -> u64 {
    i.window_select_count += 1;
    i.window_select_count
}

pub(crate) fn reserve_dump_ids(window: usize, frame: usize) {
    NEXT_ID.fetch_max(
        window.saturating_add(1),
        std::sync::atomic::Ordering::Relaxed,
    );
    NEXT_FRAME_ID.fetch_max(
        frame.saturating_add(1),
        std::sync::atomic::Ordering::Relaxed,
    );
}

impl Window {
    pub(crate) fn old_point_position(&self) -> usize {
        if let Value::Marker(marker) = &self.old_point_marker {
            let marker = marker.borrow();
            if marker.buffer == Some(self.buffer) {
                return marker.position;
            }
        }
        self.old_point
    }

    pub(crate) fn record_old_point(&mut self, position: usize) {
        self.old_point = position;
        if let Value::Marker(marker) = &self.old_point_marker {
            let mut marker = marker.borrow_mut();
            if marker.buffer == Some(self.buffer) {
                marker.position = position;
            }
        }
    }

    pub fn new(buffer: usize) -> WindowRef {
        crate::lisp::memory::allocate(
            Window {
                id: next_id(),
                buffer,
                point: 0,
                start: 0,
                redisplay_end: None,
                hscroll: 0,
                top: 0,
                height: 24,
                left: 0,
                width: 80,
                normal_size: (1.0, 1.0),
                dedicated: Value::Nil,
                minibuffer: false,
                params: Value::Nil,
                margins: (0, 0),
                fringes: None,
                scroll_bars: None,
                suspend_hscroll: false,
                suspend_vscroll: false,
                old_point: 0,
                old_point_marker: Value::Nil,
                min_hscroll: 0,
                // GNU bumps the use tick on selection, not creation; a
                // fresh window reports 0 until first selected.
                use_time: 0,
                prev_buffers: Value::Nil,
                next_buffers: Value::Nil,
                horiz: None,
                children: Vec::new(),
                parent: None,
                combination_limit: false,
                dead: false,
                new_total: 0,
                new_pixel: 0,
                new_normal: Value::Int(0),
                old_buffer: Value::Nil,
                change_stamp: 0,
                change_frame: None,
                change_clock: None,
                change_size: None,
                cursor_type: Value::t(),
            },
            crate::lisp::Value::Window,
        )
    }

    /// An internal combination window (`vc' stacked / `hc' side by
    /// side) covering the given rect.  Internal windows are "valid"
    /// but never "live" and carry no buffer.
    pub fn new_internal(
        horiz: bool,
        left: usize,
        top: usize,
        width: usize,
        height: usize,
    ) -> WindowRef {
        let r = Window::new(usize::MAX);
        {
            let mut rb = r.borrow_mut();
            rb.horiz = Some(horiz);
            rb.left = left;
            rb.top = top;
            rb.width = width;
            rb.height = height;
        }
        r
    }
}

/// W's parent internal window, if any (weak link upgrade).
pub(crate) fn w_parent(w: &WindowRef) -> Option<WindowRef> {
    w.borrow().parent.as_ref().and_then(|p| p.upgrade())
}

/// Sibling group: the parent's children, or the frame's parentless
/// chain (root … minibuffer) for root/minibuffer windows — GNU's
/// `window-next-sibling'/`-prev-sibling' semantics.  Returns the
/// ordered group and W's index into it.
pub(crate) fn w_siblings(f: &Frame, w: &WindowRef) -> (Vec<WindowRef>, usize) {
    if let Some(p) = w_parent(w) {
        let kids = p.borrow().children.clone();
        let pos = kids
            .iter()
            .position(|k| Rc::ptr_eq(k, w))
            .unwrap_or(usize::MAX);
        (kids, pos)
    } else {
        let mut group: Vec<WindowRef> = Vec::new();
        if let Some(r) = &f.root {
            group.push(r.clone());
        }
        if let Some(m) = &f.minibuffer {
            group.push(m.clone());
        }
        let pos = group
            .iter()
            .position(|k| Rc::ptr_eq(k, w))
            .unwrap_or(usize::MAX);
        (group, pos)
    }
}

/// W's frame: searches live leaves, the root subtree and the
/// minibuffer window.
pub(crate) fn frame_of_window(i: &Interp, w: &WindowRef) -> Option<FrameRef> {
    let wid = w.borrow().id;
    i.frames
        .iter()
        .find(|f| {
            let fb = f.borrow();
            fb.windows.iter().any(|x| x.borrow().id == wid)
                || fb.minibuffer.as_ref().map(|m| m.borrow().id == wid) == Some(true)
                || fb.root.as_ref().map(|r| r.borrow().id == wid) == Some(true)
        })
        .cloned()
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
        let frame = crate::lisp::memory::allocate(
            Frame {
                id: next_frame_id(),
                name: Value::string_ref("F1"),
                windows: vec![main.clone()],
                selected: main.clone(),
                minibuffer: Some(mb),
                // GNU: the lone live window IS the root window.
                root: Some(main),
                menu_shift: false,
                text_margin: 0,
                width,
                height,
                old_selected: None,
                change_stamp: Rc::new(std::cell::Cell::new(1)),
                change_window_count: 0,
                change_selected: None,
                change_frame_selected: false,
                change_size: None,
                window_state_change: false,
                buffer_list: vec![buffer],
                buried_buffer_list: Vec::new(),
                params: Value::Nil,
                dead: false,
            },
            crate::lisp::Value::Frame,
        );
        let id = frame.borrow().id;
        for window in changes::frame_windows(&frame) {
            window.borrow_mut().change_frame = Some(id);
            window.borrow_mut().change_clock = Some(frame.borrow().change_stamp.clone());
        }
        frame
    }
}

/// Reflow the selected frame's window tree to the front-end's current
/// cell dimensions. Child ratios are preserved by `relay_children`, so a
/// native resize changes live window sizes before redisplay notifications.
pub fn reflow_frame_windows(frame: &FrameRef, width: usize, height: usize) {
    let width = width.max(1);
    let height = height.max(2);
    let (root, minibuffer, root_top) = {
        let frame_data = frame.borrow();
        let root = frame_data.root.clone();
        let minibuffer = frame_data.minibuffer.clone();
        let root_top = root.as_ref().map_or(0, |root| root.borrow().top);
        (root, minibuffer, root_top)
    };
    let minibuffer_height = minibuffer
        .as_ref()
        .map_or(0, |window| window.borrow().height)
        .min(height.saturating_sub(1));
    let root_height = height.saturating_sub(minibuffer_height);
    let geometry_changed = {
        let frame_data = frame.borrow();
        frame_data.width != width
            || frame_data.height != height
            || root.as_ref().is_some_and(|root| {
                let root = root.borrow();
                root.left != 0
                    || root.top != root_top
                    || root.width != width
                    || root.height != root_height
            })
            || minibuffer.as_ref().is_some_and(|window| {
                let window = window.borrow();
                window.left != 0
                    || window.top != root_top.saturating_add(root_height)
                    || window.width != width
                    || window.height != minibuffer_height
            })
    };
    if !geometry_changed {
        return;
    }

    {
        let mut frame_data = frame.borrow_mut();
        frame_data.width = width;
        frame_data.height = height;
    }
    if let Some(root) = &root {
        {
            let mut root_data = root.borrow_mut();
            root_data.left = 0;
            root_data.top = root_top;
            root_data.width = width;
            root_data.height = root_height;
        }
        if root.borrow().horiz.is_some() {
            relay_children(root);
        }
    }
    if let Some(minibuffer) = minibuffer {
        let mut window = minibuffer.borrow_mut();
        window.left = 0;
        window.top = root_top.saturating_add(root_height);
        window.width = width;
        window.height = minibuffer_height;
    }
}

/// The frame's batch menu-bar shift (0 or 1 rows) — derived from the
/// minibuffer's top edge so it also applies to frames whose geometry
/// was installed by `set-window-configuration'.
pub(crate) fn fmenu_shift(f: &Frame) -> usize {
    let mtop = f
        .minibuffer
        .as_ref()
        .map(|m| m.borrow().top)
        .unwrap_or(f.height);
    usize::from(f.menu_shift || mtop >= f.height)
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
        2,
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
    S!("split-window", 0, 5, f_split_window, "Split WINDOW."),
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
        2,
        f_delete_other_windows,
        "Delete all but WINDOW."
    ),
    S!(
        "other-window",
        1,
        3,
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
        2,
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
        4,
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
    S!("window-total-width", 0, 2, f_window_total_width, ""),
    S!("window-text-width", 0, 2, f_window_text_width, ""),
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
    S!("window-at", 2, 3, f_window_at, "Window at X,Y."),
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
        1,
        1,
        f_window_left,
        "Window to the left of WINDOW."
    ),
    S!(
        "window-right",
        1,
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
    S!("window-left-char", 0, 1, f_window_left_char, ""),
    S!("window-top-line", 0, 1, f_window_top, ""),
    S!("window-display-table", 0, 1, f_window_display_table, ""),
    S!("set-window-display-table", 2, 2, f_set_window_display_table, ""),
    S!(
        "window-margins",
        0,
        1,
        f_window_margins,
        "Margins of WINDOW."
    ),
    S!("window-use-time", 0, 1, f_window_use_time, ""),
    S!("window-cursor-type", 0, 1, f_window_cursor_type, ""),
    S!("window-configuration-p", 1, 1, f_window_configuration_p, ""),
    S!(
        "current-window-configuration",
        0,
        1,
        f_current_window_configuration,
        ""
    ),
    S!(
        "set-window-configuration",
        1,
        3,
        f_set_window_configuration,
        ""
    ),
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
    S!("framep", 1, 1, f_framep, "Display type of OBJECT if it is a frame."),
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
    S!("frame-pixel-width", 0, 1, f_frame_pixel_width, ""),
    S!("frame-pixel-height", 0, 1, f_frame_pixel_height, ""),
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
    S!(
        "display-graphic-p",
        0,
        1,
        f_display_graphic_p,
        "t on GUI (nil on tty)."
    ),
    S!(
        "window-system",
        0,
        1,
        f_window_system,
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
    S!(
        "current-minor-mode-maps",
        0,
        0,
        f_current_minor_mode_maps,
        ""
    ),
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
        6,
        f_read_key_sequence,
        "Read a key sequence."
    ),
    S!(
        "read-key-sequence-vector",
        1,
        6,
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
    S!("input-pending-p", 0, 1, f_input_pending_p, ""),
    S!("discard-input", 0, 0, f_discard_input, ""),
    S!("last-nonminibuffer-frame", 0, 0, f_selected_frame, ""),
    // kill ring
    S!("kill-new", 1, 2, f_kill_new, "Push STRING onto kill-ring."),
    S!(
        "kill-append",
        2,
        2,
        f_kill_append,
        "Append STRING to latest kill."
    ),
    S!("current-kill", 1, 2, f_current_kill, "Nth kill-ring entry."),
    S!(
        "copy-region-as-kill",
        2,
        3,
        f_copy_region_as_kill,
        "Copy region to kill-ring."
    ),
    S!("kill-ring-save", 2, 3, f_copy_region_as_kill, ""),
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
        f_file_name_sans_versions,
        ""
    ),
    S!(
        "file-name-base",
        0,
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
        5,
        f_directory_files,
        "Files in DIRECTORY."
    ),
    S!(
        "directory-files-and-attributes",
        1,
        6,
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
        4,
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
    S!("copy-file", 2, 6, f_copy_file, "Copy FILE to NEWNAME."),
    S!(
        "add-name-to-file",
        2,
        3,
        f_add_name_to_file,
        "Hard link FILE to NEWNAME."
    ),
    S!(
        "insert-file-contents",
        1,
        5,
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
        1,
        3,
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
        3,
        f_append_to_file,
        "Append region to FILENAME."
    ),
    S!(
        "file-truename",
        1,
        3,
        f_file_truename,
        "Canonical name of FILENAME."
    ),
    S!("unhandled-file-name-directory", 1, 1, f_unhandled_dir, ""),
    S!("file-remote-p", 1, 3, f_file_remote_p, ""),
    S!("file-local-name", 1, 1, f_file_local_name, ""),
    S!("file-name-quote", 1, 2, f_file_name_quote, ""),
    S!("file-name-unquote", 1, 2, f_file_name_unquote, ""),
    S!("file-accessible-directory-p", 1, 1, f_file_accessible_directory_p, ""),
    S!("default-file-modes", 0, 0, f_default_file_modes, ""),
    S!("set-default-file-modes", 1, 1, f_set_default_file_modes, ""),
    S!("file-modes-symbolic-to-number", 1, 2, f_modes_sym2num, ""),
    S!("unix-sync", 0, 0, f_unix_sync, ""),
    S!("file-system-info", 1, 1, f_file_system_info, ""),
    S!("file-equal-p", 2, 2, f_file_equal_p, ""),
    // processes
    // GNU: (call-process PROGRAM &optional INFILE DESTINATION DISPLAY
    // &rest ARGS) — MANY arity; ARGS are passed to the program.
    S!(
        "call-process",
        many 1,
        f_call_process,
        "Run PROGRAM synchronously."
    ),
    S!(
        "call-process-region",
        many 3,
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
    // kill-line is Lisp (prelude), GNU simple.el semantics: ARG-aware,
    // kill-whole-line/trailing-whitespace handling via kill-region.
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
        "just-one-space",
        0,
        1,
        f_just_one_space,
        "One space around point."
    ),
    S!(
        "zap-to-char",
        2,
        3,
        f_zap_to_char,
        "Kill up to Nth occurrence of CHAR."
    ),
    // transpose-chars / transpose-lines are Lisp (prelude), built on
    // transpose-subr like GNU simple.el — the Rust implementations
    // diverged on negative args, eol handling, and trailing newlines.
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
    // delete-trailing-whitespace is Lisp (prelude), GNU simple.el
    // (regex-driven with syntax-table formfeed exclusion).
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
        2,
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
        7,
        f_read_from_minibuffer,
        "Read from minibuffer."
    ),
    S!("read-buffer", 1, 4, f_read_buffer, "Read a buffer name."),
    S!(
        "read-file-name",
        1,
        6,
        f_read_file_name,
        "Read a file name."
    ),
    S!("read-directory-name", 1, 6, f_read_file_name, ""),
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
        3,
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
    S!("read-key", 0, 2, f_read_event, "Read one key event."),
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
        1,
        1,
        f_command_line_1_stub,
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
    S!("read-event", 0, 3, f_read_event, "Read one input event."),
    S!("read-char", 0, 3, f_read_char, "Read one character."),
    S!(
        "read-char-exclusive",
        0,
        3,
        f_read_char_exclusive,
        "Read one character."
    ),
    S!("y-or-n-p", 1, 1, f_y_or_n_p, "Ask yes/no (batch: t)."),
    S!("yes-or-no-p", 1, 1, f_yes_or_no_p, ""),
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
        3,
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
    // GNU byte-compiles kmacro.el's `(defun kmacro ...)', leaving a
    // `subr' in the function cell — (subrp #'kmacro) is t in GNU 31.1.
    // Ours is a primitive that delegates to Lisp `kmacro--impl',
    // autoloading kmacro.el on first call.
    S!(
        "kmacro",
        1,
        3,
        f_kmacro,
        "Create a `kmacro' for macro bound to symbol or key."
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
    S!("major-mode-restore", 0, 1, f_major_mode_restore, ""),
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
        crate::buffer::primitives::f_restore_buffer_modified_p,
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
    // `face-attribute' is Lisp (prelude) on top of
    // `internal-get-lisp-face-attribute', per GNU faces.el — the subr
    // would shadow the merge/inherit logic.
    S!(
        "face-attribute-relative-p",
        2,
        2,
        f_face_attribute_relative_p,
        ""
    ),
    S!("merge-face-attribute", 3, 3, f_merge_face_attribute, ""),
    // `face-all-attributes' is Lisp (prelude) per GNU faces.el.
    S!("face-list", 0, 0, f_face_list, ""),
    S!("make-face", 1, 1, f_make_face, ""),
    S!("copy-face", 2, 4, f_copy_face, ""),
    S!("face-equal", 2, 3, f_face_equal, ""),
    S!("face-id", 1, 2, f_face_id, ""),
    S!("face-background", 1, 3, f_face_background, ""),
    S!("face-foreground", 1, 3, f_face_foreground, ""),
    S!("set-face-background", 2, 3, f_set_face_background, ""),
    S!("face-extend-p", 1, 3, f_face_extend_p, ""),
    S!("set-face-extend", 2, 3, f_set_face_extend, ""),
    S!("face-bold-p", 1, 3, f_face_bold_p, ""),
    S!("face-italic-p", 1, 3, f_face_italic_p, ""),
    S!("face-underline-p", 1, 3, f_face_underline_p, ""),
    S!("internal-lisp-face-p", 1, 2, f_facep, ""),
    S!(
        "internal-lisp-face-empty-p",
        1,
        2,
        f_lisp_face_check_nil,
        ""
    ),
    S!("internal-lisp-face-equal-p", 2, 3, f_face_equal, ""),
    S!(
        "internal-set-lisp-face-attribute",
        3,
        4,
        f_internal_set_lisp_face_attribute,
        ""
    ),
    S!(
        "internal-lisp-face-attribute-values",
        1,
        1,
        f_lisp_face_attribute_values,
        ""
    ),
    S!(
        "internal-merge-in-global-face",
        2,
        2,
        f_internal_merge_in_global_face,
        ""
    ),
    S!("display-color-p", 0, 1, f_display_color_p, ""),
    S!("display-grayscale-p", 0, 1, f_display_pred_nil, ""),
    S!("display-mouse-p", 0, 1, f_display_pred_nil, ""),
    S!("color-defined-p", 1, 2, f_color_defined_p, ""),
    S!("defined-colors", 0, 1, f_defined_colors, ""),
    S!("color-values", 1, 2, f_color_values, ""),
    S!("x-color-values", 1, 1, f_x_color_values, ""),
    S!("xw-color-values", 1, 2, f_xw_color_values, ""),
    S!("tty-color-values", 1, 1, f_tty_color_values, ""),
    S!("x-list-fonts", 1, 5, f_x_no_display, ""),
    S!("font-xlfd-name", 1, 3, f_font_xlfd_name, ""),
    S!("fontp", 1, 2, f_fontp, ""),
    S!("find-font", 1, 2, f_find_font, ""),
    S!("internal-char-font", 1, 2, f_internal_char_font, ""),
    S!("clear-font-cache", 0, 0, f_nil, ""),
    S!("list-fonts", 1, 4, f_list_fonts, ""),
    // cursor/display misc
    // `cursor-type` is a variable in Emacs, not a function —
    // calling it signals void-function like GNU.
    S!("blink-cursor-mode", 0, 1, f_blink_cursor_mode, ""),
    S!("internal-show-cursor", 2, 2, f_internal_show_cursor, ""),
    S!("internal-show-cursor-p", 0, 1, f_show_cursor_p, ""),
    S!("set-window-cursor-type", 2, 2, f_set_window_cursor_type, ""),
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
    S!("fontset-info", 1, 2, f_x_no_display, ""),
    S!("fontset-font", 2, 3, f_fontset_font_read, ""),
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
        crate::lisp::builtins::misc::f_internal_event_symbol_parse,
        ""
    ),
    S!(
        "substitute-command-keys",
        1,
        3,
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

fn f_nil(_i: &mut Interp, _a: crate::lisp::Argv) -> EvalResult {
    Ok(Value::Nil)
}
fn f_second(_i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    Ok(a.iter().nth(1).cloned().unwrap_or(Value::Nil))
}
fn arg(a: &[Value], i: usize) -> Value {
    a.get(i).cloned().unwrap_or(Value::Nil)
}

/// GNU's `(interactive "p")` / numeric-count decode: nil/absent -> 1,
/// markers -> position, other non-numbers signal
/// `wrong-type-argument number-or-marker-p'.
pub(crate) fn num_or_mark(i: &mut Interp, v: &Value) -> Result<i128, Flow> {
    match v {
        Value::Nil => Ok(1),
        Value::Int(n) => Ok(*n),
        Value::Marker(m) => Ok(m.borrow().position as i128 + 1),
        other => Err(i.wrong_type_mut("number-or-marker-p", other)),
    }
}

/// GNU `decode_buffer_position': nil is NOT a default here — only
/// integers and markers pass; everything else signals
/// `wrong-type-argument number-or-marker-p'.
/// GNU `number_or_marker' path: ints and floats pass, live markers
/// yield their 1-based position; a detached marker signals "Marker
/// does not point anywhere", other values signal
/// `wrong-type-argument number-or-marker-p'.
pub(crate) fn want_num_or_mark(i: &mut Interp, v: &Value) -> Result<i128, Flow> {
    match v {
        Value::Int(n) => Ok(*n),
        Value::BigInt(_) => crate::lisp::builtins::want_int(i, v),
        Value::Float(x) => Ok(**x as i128),
        Value::Marker(m) if m.borrow().buffer.is_some() => Ok(m.borrow().position as i128 + 1),
        Value::Marker(_) => Err(i.error("Marker does not point anywhere")),
        other => Err(i.wrong_type_mut("number-or-marker-p", other)),
    }
}

pub(crate) fn err_sym(i: &mut Interp, name: &str, data: Vec<Value>) -> Flow {
    let id = i.intern(name);
    i.signal_data(id, data)
}

fn want_str(i: &mut Interp, v: &Value) -> Result<EmacsText, Flow> {
    match v {
        Value::Str(s) => Ok(s.borrow().clone()),
        _ => Err(i.wrong_type_mut("stringp", v)),
    }
}

fn want_int(i: &mut Interp, v: &Value) -> Result<i128, Flow> {
    match v {
        Value::Int(n) => Ok(*n),
        // GNU CHECK_FIXNUM_COERCE_MARKER: detached markers signal
        // "Marker does not point anywhere", not a stale position.
        Value::Marker(m) if m.borrow().buffer.is_some() => Ok(m.borrow().position as i128 + 1),
        Value::Marker(_) => Err(i.error("Marker does not point anywhere")),
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

/// GNU `decode_valid_window': non-window args signal `window-valid-p',
/// dead windows signal it too (valid = exists AND alive).
pub(crate) fn win_valid(i: &mut Interp, v: &Value) -> Result<WindowRef, Flow> {
    match v {
        Value::Nil => sel_window(i).ok_or_else(|| i.error("No window")),
        Value::Window(w) if !w.borrow().dead => Ok(w.clone()),
        other => Err(i.wrong_type_mut("window-valid-p", other)),
    }
}

/// GNU `decode_live_window': like `win_valid' but also rejects internal
/// (non-leaf) windows — GNU's live means leaf AND attached.
pub(crate) fn win_live(i: &mut Interp, v: &Value) -> Result<WindowRef, Flow> {
    match v {
        Value::Nil => sel_window(i).ok_or_else(|| i.error("No window")),
        Value::Window(w) if !w.borrow().dead && w.borrow().horiz.is_none() => Ok(w.clone()),
        other => Err(i.wrong_type_mut("window-live-p", other)),
    }
}

/// GNU `decode_live_window' for *required* WINDOW arguments: nil is
/// a datum, not "selected window" — `select-window'-style callers.
pub(crate) fn win_live_req(i: &mut Interp, v: &Value) -> Result<WindowRef, Flow> {
    match v {
        Value::Window(w) if !w.borrow().dead && w.borrow().horiz.is_none() => Ok(w.clone()),
        other => Err(i.wrong_type_mut("window-live-p", other)),
    }
}

/// Bare `windowp' check — for readers where GNU's `decode_any_window'
/// accepts a dead window object (e.g. `window-buffer').
pub(crate) fn win_any(i: &mut Interp, v: &Value) -> Result<WindowRef, Flow> {
    match v {
        Value::Nil => sel_window(i).ok_or_else(|| i.error("No window")),
        Value::Window(w) => Ok(w.clone()),
        other => Err(i.wrong_type_mut("windowp", other)),
    }
}

/// Back-compat alias: default GNU subrs that used to funnel through the
/// unchecked helper keep the strictest valid semantics.
pub(crate) fn win_of(i: &mut Interp, v: &Value) -> Result<WindowRef, Flow> {
    // Default window decoder: GNU's decode_valid_window →
    // wrong-type-argument `window-valid-p' on non-windows/dead
    // windows.  Functions that use decode_live_window call
    // `win_live' explicitly.
    win_valid(i, v)
}

/// GNU `decode_valid_window' before the wta upgrade path: non-windows
/// and dead windows signal `(error "<VAL> is not a valid window")'.
pub(crate) fn win_err_valid(i: &mut Interp, v: &Value) -> Result<WindowRef, Flow> {
    match v {
        Value::Nil => sel_window(i).ok_or_else(|| i.error("No window")),
        Value::Window(w) if !w.borrow().dead => Ok(w.clone()),
        other => {
            let s = crate::lisp::eval::Interp::sink_text(
                &i.princ_to_string_cb(other).unwrap_or_default(),
            )
            .to_owned();
            Err(i.error(format!("{} is not a valid window", s)))
        }
    }
}

/// GNU `decode_live_window' error string — dead or internal windows
/// signal `(error "<VAL> is not a live window")'.
pub(crate) fn win_err_live(i: &mut Interp, v: &Value) -> Result<WindowRef, Flow> {
    match v {
        Value::Nil => sel_window(i).ok_or_else(|| i.error("No window")),
        Value::Window(w) if !w.borrow().dead && w.borrow().horiz.is_none() => Ok(w.clone()),
        other => {
            let s = crate::lisp::eval::Interp::sink_text(
                &i.princ_to_string_cb(other).unwrap_or_default(),
            )
            .to_owned();
            Err(i.error(format!("{} is not a live window", s)))
        }
    }
}

// ---------- windows ----------

fn f_selected_window(i: &mut Interp, _a: crate::lisp::Argv) -> EvalResult {
    match sel_window(i) {
        Some(w) => Ok(Value::Window(w)),
        None => Ok(Value::Nil),
    }
}

fn f_windowp(_i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    Ok(Value::from_bool(matches!(&a[0], Value::Window(_))))
}

/// GNU `decode_any_frame': dead frames are fine, non-frames signal
/// `framep'.  Nil resolves to the selected frame; windows are NOT
/// coerced (window.c's decoders accept frame objects only).
pub(crate) fn frame_any(i: &mut Interp, v: &Value) -> Result<FrameRef, Flow> {
    match v {
        Value::Nil => sel_frame(i).ok_or_else(|| i.error("No frame")),
        Value::Frame(f) => Ok(f.clone()),
        other => Err(i.wrong_type_mut("framep", other)),
    }
}

pub(crate) fn frame_of(i: &mut Interp, v: &Value) -> Result<FrameRef, Flow> {
    frame_any(i, v)
}

/// GNU `decode_live_frame' (CHECK_LIVE_FRAME): a non-frame argument —
/// dead or alive — signals `wrong-type-argument frame-live-p', not
/// `framep'.  Nil resolves to the selected frame (live by definition).
pub(crate) fn frame_live(i: &mut Interp, v: &Value) -> Result<FrameRef, Flow> {
    match v {
        Value::Nil => sel_frame(i).ok_or_else(|| i.error("No frame")),
        Value::Frame(f) if !f.borrow().dead => Ok(f.clone()),
        other => Err(i.wrong_type_mut("frame-live-p", other)),
    }
}

/// Face-family FRAME decode (GNU xfaces.c `check_lface_frame'): nil
/// or a live frame resolve; `t' selects the new-frame defaults layer —
/// anything else signals `frame-live-p'.  Distinguishable from
/// `frame_live', which rejects `t' for frame primitives.
fn face_frame_live(i: &mut Interp, v: &Value) -> Result<(), Flow> {
    match v {
        Value::Nil | Value::Frame(_) => {
            let _ = frame_live(i, v)?;
            Ok(())
        }
        Value::Sym(s) if *s == sym::T => Ok(()),
        // GNU `check_lface_frame' also accepts fixnum 0: "all frames
        // plus new-frame defaults" — Lisp `set-face-attribute' rewrites
        // a nil FRAME to 0 before calling `internal-set-lisp-face-
        // attribute' (faces.el:861), so integer 0 is the common batch
        // path once the real faces.el definition shadows the subr.
        Value::Int(0) => Ok(()),
        other => Err(i.wrong_type_mut("frame-live-p", other)),
    }
}

/// GNU `decode_any_frame'-with-window variant used by
/// `frame-root-window'/`frame-selected-window'/`frame-first-window':
/// window args resolve to the window's frame; otherwise it's
/// `decode_live_frame' semantics (non-frame -> frame-live-p).
pub(crate) fn frame_or_win(i: &mut Interp, v: &Value) -> Result<FrameRef, Flow> {
    if let Value::Window(w) = v {
        if let Some(f) = frame_of_window(i, w) {
            return Ok(f);
        }
    }
    frame_live(i, v)
}

/// nil result after a `decode_live_frame' check on the optional arg —
/// GNU's font-family-list/frame-font-cache shape.
pub(crate) fn f_nil_frame_arg(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let _ = frame_live(i, &arg(&a, 0))?;
    Ok(Value::Nil)
}
fn f_window_live_p(_i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    match &a[0] {
        // Live = leaf window still attached to a frame; internal
        // combination windows are valid but never live (GNU).
        Value::Window(w) => {
            let w = w.borrow();
            Ok(Value::from_bool(!w.dead && w.horiz.is_none()))
        }
        _ => Ok(Value::Nil),
    }
}

fn f_window_buffer(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let w = win_any(i, &arg(&a, 0))?;
    if w.borrow().dead {
        return Ok(Value::Nil);
    }
    Ok(i.buffer_value(w.borrow().buffer).unwrap_or(Value::Nil))
}

/// GNU set_window_buffer updates display metadata even when the
/// window already shows this buffer.  Merely selecting a buffer does not.
pub(crate) fn record_buffer_display(i: &mut Interp, bid: usize) {
    let count = i.q("buffer-display-count");
    let time = i.q("buffer-display-time");
    let now = crate::lisp::builtins::misc::current_time_value(i);
    if let Some(buffer) = i.buffers.get(bid) {
        let mut b = buffer.borrow_mut();
        if let Some(Value::Int(value)) = b.locals.get(&count) {
            let next = value.saturating_add(1);
            b.locals.insert(count, Value::Int(next));
        }
        b.locals.insert(time, now);
    }
}

/// GNU `record_buffer': selection updates both global and frame-local MRU.
/// Keep the selected frame/window consistent before running its Lisp hook.
pub(crate) fn record_buffer_selection(i: &mut Interp, bid: usize) -> Result<(), Flow> {
    i.buffers.touch(bid);
    if let Some(f) = sel_frame(i) {
        let mut f = f.borrow_mut();
        f.buffer_list.retain(|id| *id != bid);
        f.buffer_list.insert(0, bid);
        f.buried_buffer_list.retain(|id| *id != bid);
    }
    crate::buffer::primitives::run_buffer_list_update_checked(i, bid)
}

/// GNU `bury-buffer-internal' changes only the selected frame's histories.
/// Global ordering and hook execution belong to the caller.
pub(crate) fn bury_buffer_history(i: &mut Interp, bid: usize) {
    if let Some(f) = sel_frame(i) {
        let mut f = f.borrow_mut();
        f.buffer_list.retain(|id| *id != bid);
        f.buried_buffer_list.retain(|id| *id != bid);
        f.buried_buffer_list.insert(0, bid);
    }
}

/// A killed buffer must disappear from every frame's local histories.
pub(crate) fn forget_buffer_history(i: &mut Interp, bid: usize) {
    for f in &i.frames {
        let mut f = f.borrow_mut();
        f.buffer_list.retain(|id| *id != bid);
        f.buried_buffer_list.retain(|id| *id != bid);
    }
}

/// GNU unshow_buffer: retain the outgoing view's point unless it is
/// owned by the selected window or another still-displayed point owner.
fn unshow_window_buffer(i: &Interp, window: &WindowRef, selected_buffer: Option<usize>) {
    let buffer_id = window.borrow().buffer;
    if let Some(buffer) = i.buffers.get(buffer_id) {
        let last = buffer
            .borrow()
            .last_selected_window
            .as_ref()
            .and_then(|w| w.upgrade());
        let another_point_owner = last.as_ref().is_some_and(|last| {
            !Rc::ptr_eq(last, window) && !last.borrow().dead && last.borrow().buffer == buffer_id
        });
        let mut buffer = buffer.borrow_mut();
        buffer.last_window_start = window.borrow().start;
        if selected_buffer != Some(buffer_id) && !another_point_owner {
            buffer.set_point(window.borrow().point);
        }
        if last.as_ref().is_some_and(|last| Rc::ptr_eq(last, window)) {
            buffer.last_selected_window = None;
        }
    }
}

pub(crate) fn f_set_window_buffer(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let w = win_live(i, &arg(&a, 0))?;
    // GNU resolves BUFFER-OR-NAME with Fget_buffer, then CHECK_BUFFER.
    let bid = match &a[1] {
        Value::Buffer(b) => b.borrow().id,
        Value::Str(s) => {
            let name = s.borrow().clone();
            i.buffers
                .by_name(&name)
                .ok_or_else(|| i.wrong_type_mut("bufferp", &Value::Nil))?
        }
        other => return Err(i.wrong_type_mut("stringp", other)),
    };
    if !i.buffer_live(bid) {
        return Err(i.error("Attempt to display deleted buffer"));
    }
    let old_bid = w.borrow().buffer;
    if old_bid != bid {
        if matches!(w.borrow().dedicated, Value::Sym(sym::T)) {
            let name = i
                .buffers
                .get(old_bid)
                .map(|b| b.borrow().name.clone())
                .unwrap_or_default();
            return Err(i.error_q("Window is dedicated to `%s'", &[&name]));
        }
        w.borrow_mut().dedicated = Value::Nil;
        // Reuse GNU's Lisp history policy, including insignificant-buffer
        // filtering, marker identity/insertion types, next-history reset,
        // and buffer-list-update-hook (whose errors must propagate).
        let record = Value::Sym(i.q("record-window-buffer"));
        i.apply(&record, vec![Value::Window(w.clone())])?;
    }
    let selected = sel_window(i);
    let selected_buffer = selected.as_ref().map(|window| window.borrow().buffer);
    unshow_window_buffer(i, &w, selected_buffer);
    let (target_point, target_start) = i
        .buffers
        .get(bid)
        .map(|b| {
            let b = b.borrow();
            (b.point(), b.last_window_start.clamp(b.begv, b.zv))
        })
        .unwrap_or((0, 0));
    {
        let mut wb = w.borrow_mut();
        // The newly displayed buffer is no longer a previous/next buffer.
        if let Ok(entries) = wb.prev_buffers.list_to_vec() {
            wb.prev_buffers = Value::list(
                entries
                    .into_iter()
                    .filter(|entry| {
                        !matches!(entry, Value::Cons(c)
                    if matches!(&c.borrow().car, Value::Buffer(b) if b.borrow().id == bid))
                    })
                    .collect(),
            );
        }
        if let Ok(entries) = wb.next_buffers.list_to_vec() {
            wb.next_buffers = Value::list(
                entries
                    .into_iter()
                    .filter(|entry| !matches!(entry, Value::Buffer(b) if b.borrow().id == bid))
                    .collect(),
            );
        }
        wb.buffer = bid;
        if old_bid != bid || arg(&a, 2).is_nil() {
            wb.point = target_point;
            set_window_old_point(i, &mut wb, target_point);
            wb.start = target_start;
            wb.hscroll = 0;
            wb.min_hscroll = 0;
            wb.suspend_hscroll = false;
        }
    }
    if selected
        .as_ref()
        .is_some_and(|selected| Rc::ptr_eq(selected, &w))
    {
        if let Some(buffer) = i.buffers.get(bid) {
            buffer.borrow_mut().last_selected_window = Some(Rc::downgrade(&w));
        }
    }
    record_buffer_display(i, bid);
    // Displaying a buffer neither selects it nor promotes either MRU list.
    Ok(Value::Nil)
}

/// The selected window owns its buffer's live point, even when a different
/// buffer is current via `set-buffer'.  Other windows use their own point.
fn window_point(i: &Interp, w: &WindowRef) -> usize {
    if sel_window(i).is_some_and(|selected| Rc::ptr_eq(&selected, w)) {
        i.buffers
            .get(w.borrow().buffer)
            .map(|b| b.borrow().point())
            .unwrap_or_else(|| w.borrow().point)
    } else {
        w.borrow().point
    }
}

fn f_window_point(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let w = win_live(i, &arg(&a, 0))?;
    Ok(Value::Int(window_point(i, &w) as i128 + 1))
}

fn f_set_window_point(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let w = win_live(i, &arg(&a, 0))?;
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

fn f_window_start(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let w = win_live(i, &arg(&a, 0))?;
    Ok(Value::Int(w.borrow().start as i128 + 1))
}

/// `window-top-line' — GNU `w->top_line': screen row of the window's
/// top edge (0 = top of the frame's window area).
fn f_window_top(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let w = win_of(i, &arg(&a, 0))?;
    Ok(Value::Int(w.borrow().top as i128))
}
/// `window-left-char' — the window's left column on screen.
fn f_window_left_char(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let w = win_of(i, &arg(&a, 0))?;
    Ok(Value::Int(w.borrow().left as i128))
}

/// `window-cursor-type' — GNU CHECK_LIVE_WINDOW; returns the
/// window's stored `cursor-type' (t = follow the buffer variable).
fn f_window_cursor_type(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let w = win_live(i, &arg(&a, 0))?;
    Ok(w.borrow().cursor_type.clone())
}

fn f_set_window_start(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let w = win_live(i, &arg(&a, 0))?;
    let pos = want_int(i, &a[1])?.max(1) as usize - 1;
    let mut ww = w.borrow_mut();
    ww.start = pos;
    ww.redisplay_end = None;
    Ok(a[1].clone())
}

fn f_window_end(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let w = win_live(i, &arg(&a, 0))?;
    let update = a.get(1).is_some_and(Value::truthy);
    let (buffer, start) = {
        let ww = w.borrow();
        (ww.buffer, ww.start)
    };
    let Some(buffer_ref) = i.buffers.get(buffer) else {
        return Ok(Value::Int(1));
    };
    let (zv, tick) = {
        let bb = buffer_ref.borrow();
        (bb.zv, bb.text.mod_tick.get())
    };
    if update {
        if !i.noninteractive
            && let (Some(frame), Some(selected)) = (&i.selected_frame, frame_of_window(i, &w))
            && std::rc::Rc::ptr_eq(frame, &selected)
        {
            let (width, height) = {
                let frame = frame.borrow();
                (frame.width, frame.height)
            };
            let _ = crate::frontend::render_grid(i, width, height);
        } else {
            // Batch has no visible grid; its displayed endpoint is the
            // buffer's accessible end, matching GNU's batch redisplay.
            w.borrow_mut().redisplay_end = Some((buffer, tick, start, zv));
        }
    }
    let end = w
        .borrow()
        .redisplay_end
        .filter(|(b, t, s, _)| *b == buffer && *t == tick && *s == start)
        .map(|(_, _, _, end)| end)
        .unwrap_or(zv);
    Ok(Value::Int(end as i128 + 1))
}

fn f_window_frame(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let w = win_valid(i, &arg(&a, 0))?;
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

pub(crate) fn f_window_list(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU Fwindow_list: WINDOW decodes first
    // (CHECK_WINDOW -> windowp), then FRAME stays un-decoded — a nil
    // frame becomes the selected frame and anything else must satisfy
    // EQ against the window's frame (`Window is on a different frame').
    let warg = arg(&a, 2);
    let w = match &warg {
        Value::Nil => sel_window(i).ok_or_else(|| i.error("No window"))?,
        Value::Window(w) => w.clone(),
        other => return Err(i.wrong_type_mut("windowp", other)),
    };
    let fval = arg(&a, 0);
    let f = if fval.is_nil() {
        sel_frame(i).ok_or_else(|| i.error("No frame"))?
    } else {
        match &fval {
            Value::Frame(f) => f.clone(),
            _ => {
                // Non-frame frame spec: EQ can never hold.
                return Err(i.error(format!("Window is on a different frame")));
            }
        }
    };
    // EQ: compare frame identities.
    {
        let wf = frame_of_window(i, &w);
        if !wf.map(|wf| Rc::ptr_eq(&wf, &f)).unwrap_or(false) {
            return Err(i.error("Window is on a different frame"));
        }
    }
    // GNU: MINIBUF t = always include the minibuffer window; nil =
    // include only when active; any other non-nil = never include.
    let include_mini = match a.get(1) {
        Some(v) if v.truthy() => i.sym_id(v) == Some(sym::T),
        _ => i.minibuf_level > 0,
    };
    // GNU returns the frame's window chain in cyclic order starting
    // at WINDOW — the minibuffer window (when included) sits in the
    // chain after the last content window.
    let (mut ws, start_id) = {
        let fb = f.borrow();
        let mut ws = fb.windows.clone();
        if include_mini {
            if let Some(mb) = &fb.minibuffer {
                ws.push(mb.clone());
            }
        }
        (ws, w.borrow().id)
    };
    if let Some(pos) = ws.iter().position(|w| w.borrow().id == start_id) {
        ws.rotate_left(pos);
    }
    Ok(Value::list(
        ws.iter().map(|w| Value::Window(w.clone())).collect(),
    ))
}

fn f_window_minibuffer_p(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let w = win_valid(i, &arg(&a, 0))?;
    Ok(Value::from_bool(w.borrow().minibuffer))
}

fn f_minibuffer_window(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let f = frame_live(i, &arg(&a, 0))?;
    match &f.borrow().minibuffer {
        Some(w) => Ok(Value::Window(w.clone())),
        None => Ok(Value::Nil),
    }
}

fn f_active_minibuffer_window(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
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
    let t = Value::record(vec![
        Value::Sym(i.q("terminal")),
        Value::Int(0),
        Value::string("initial_terminal"),
    ]);
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

/// Window-specific display tables are kept with the window parameters.
fn f_window_display_table(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let window = win_valid(i, &arg(&a, 0))?;
    let property = Value::Sym(i.q("display-table"));
    Ok(crate::lisp::eval::plist_get_v(
        &window.borrow().params,
        &property,
    ))
}

fn f_set_window_display_table(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let window = win_valid(i, &a[0])?;
    let property = Value::Sym(i.q("display-table"));
    let parameters = crate::lisp::eval::plist_put_front(
        &window.borrow().params,
        &property,
        a[1].clone(),
    );
    window.borrow_mut().params = parameters;
    Ok(a[1].clone())
}

/// `merge-face-attribute` — GNU: VALUE1 wins unless `unspecified' or
/// `:ignore-defface', in which case VALUE2.  (`:height' merges
/// relative specs; we take VALUE1.)
fn f_merge_face_attribute(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU xfaces.c: VALUE1 wins unless `unspecified'/`:ignore-defface';
    // :height runs merge_face_heights — a float VALUE1 scales VALUE2.
    if let Value::Sym(s) = &a[1] {
        let n = i.symbol_name(*s);
        if n == "unspecified" || n == ":ignore-defface" {
            return Ok(a[2].clone());
        }
    }
    let is_height = matches!(&a[0], Value::Sym(s) if i.symbol_name(*s) == ":height");
    if is_height {
        if let Value::Int(_) = &a[1] {
            return Ok(a[1].clone()); // absolute, use as-is
        }
        if let Value::Float(f1) = &a[1] {
            match &a[2] {
                Value::Int(n) => return Ok(Value::Int((**f1 * *n as f64) as i128)),
                Value::Float(f2) => {
                    return Ok(Value::float(**f1 * **f2));
                }
                Value::Sym(s) if i.symbol_name(*s) == "unspecified" => return Ok(a[1].clone()),
                _ => {}
            }
        }
    }
    Ok(a[1].clone())
}

/// `keymap-prompt' — GNU scans KEYMAP's cdr for the first string
/// element (the menu prompt); non-keymaps return nil, no error.
fn f_keymap_prompt(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
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
fn f_syntax_class(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    match &a[0] {
        Value::Nil => Ok(Value::Nil),
        Value::Cons(c) => {
            let car = c.borrow().car.clone();
            match car {
                // GNU `Fsyntax_class' returns SYNTAX_CODE_CLASS(code)
                // = code & 0xff — flag bits are masked off.
                Value::Int(n) => Ok(Value::Int(n & 0xff)),
                Value::Marker(m) => Ok(Value::Int((m.borrow().position as i128) & 0xff)),
                ref other => return Err(i.wrong_type_mut("integer-or-marker-p", other)),
            }
        }
        other => Err(i.wrong_type_mut("listp", other)),
    }
}

/// `major-mode-suspend' — remember the buffer's local `major-mode'
/// (nil/`fundamental-mode' isn't recorded), reset it to fundamental,
/// and return the remembered mode.  Repeated calls return the recorded
/// value, like GNU's pdump/suspend machinery.
fn f_major_mode_suspend(i: &mut Interp, _a: crate::lisp::Argv) -> EvalResult {
    let sid = i.q("major-mode");
    let fundamental = i.q("fundamental-mode");
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
fn f_major_mode_restore(i: &mut Interp, _a: crate::lisp::Argv) -> EvalResult {
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
fn f_fontset_list(_i: &mut Interp, _a: crate::lisp::Argv) -> EvalResult {
    Ok(Value::list(vec![Value::string(
        "-*-*-*-*-*-*-*-*-*-*-*-*-fontset-default",
    )]))
}

/// Window configuration specs retain live markers for point/start so
/// edits between capture and restore move saved positions like GNU.
/// Record: [tag frame SPECS SELECTED CURRENT-BUFFER TREE FRAME-SIZE CHANGE-STAMP]. Each spec is
/// [window buffer-id point start hscroll top height left width dedicated
///  minibuffer params prev-buffers next-buffers mark left-margin right-margin
///  min-hscroll suspend-hscroll old-point normal-size old-buffer]. Positions are 0-based internally.
fn window_config_marker(i: &Interp, buffer: usize, position: usize, insertion_type: bool) -> Value {
    let Some(buffer) = i.buffers.get(buffer) else {
        return Value::Nil;
    };
    if !buffer.borrow().live {
        return Value::Nil;
    }
    let marker = crate::lisp::memory::allocate(
        crate::lisp::value::Marker {
            buffer: Some(buffer.borrow().id),
            position: position.min(buffer.borrow().text.len()),
            insertion_type,
        },
        Value::Marker,
    );
    buffer.borrow_mut().register_marker(&marker);
    Value::Marker(marker)
}

fn window_config_point_insertion_type(i: &Interp, buffer: usize) -> bool {
    let Some(symbol) = i.q_soft("window-point-insertion-type") else {
        return false;
    };
    i.buffers
        .get(buffer)
        .and_then(|buffer| buffer.borrow().locals.get(&symbol).cloned())
        .unwrap_or_else(|| i.obarray.symbol(symbol).value.clone())
        .truthy()
}

pub(crate) fn set_window_old_point(i: &Interp, window: &mut Window, position: usize) {
    window.old_point = position;
    let insertion_type = window_config_point_insertion_type(i, window.buffer);
    if let Value::Marker(marker) = &window.old_point_marker {
        let mut marker = marker.borrow_mut();
        if marker.buffer == Some(window.buffer) {
            marker.position = position;
            marker.insertion_type = insertion_type;
            return;
        }
    }
    window.old_point_marker = window_config_marker(i, window.buffer, position, insertion_type);
}

fn window_config_persistent_keys(i: &mut Interp) -> Vec<Value> {
    let symbol = i.q("window-persistent-parameters");
    let symbol = i.var_alias_target(symbol);
    let mut tail = i.symbol_value(symbol);
    let mut seen = std::collections::HashSet::new();
    let mut keys = Vec::new();
    while let Value::Cons(cell) = tail {
        if !seen.insert(Rc::as_ptr(&cell) as usize) {
            // GNU clears a cyclic specification directly, without
            // notifying variable watchers, instead of traversing forever.
            if let Some(buffer) = i.current_buffer_ref() {
                if let Some(value) = buffer.borrow_mut().locals.get_mut(&symbol) {
                    *value = Value::Nil;
                    return Vec::new();
                }
            }
            i.obarray.symbol_mut(symbol).value = Value::Nil;
            return Vec::new();
        }
        let entry = cell.borrow().car.clone();
        if let Value::Cons(pair) = entry {
            let pair = pair.borrow();
            if !pair.cdr.is_nil() {
                keys.push(pair.car.clone());
            }
        }
        tail = cell.borrow().cdr.clone();
    }
    keys
}

fn window_config_parameters(parameters: &Value, keys: &[Value]) -> Value {
    // Copy associations, but retain the parameter values themselves.
    Value::list(
        keys.iter()
            .rev()
            .map(|key| Value::cons(key.clone(), crate::lisp::eval::plist_get_v(parameters, key)))
            .collect(),
    )
}

fn restore_window_config_parameters(parameters: &mut Value, saved: &Value) {
    let mut tail = saved.clone();
    while let Value::Cons(cell) = tail {
        if let Value::Cons(pair) = &cell.borrow().car {
            let pair = pair.borrow();
            // A missing/nil saved value clears an existing non-nil
            // association but never creates a new nil association.
            if !pair.cdr.is_nil() || !crate::lisp::eval::plist_get_v(parameters, &pair.car).is_nil()
            {
                *parameters =
                    crate::lisp::eval::plist_put_front(parameters, &pair.car, pair.cdr.clone());
            }
        }
        tail = cell.borrow().cdr.clone();
    }
}

fn window_normal_size_value(window: &Window) -> Value {
    Value::cons(
        Value::float(window.normal_size.0),
        Value::float(window.normal_size.1),
    )
}

fn read_window_normal_size(value: &Value) -> Option<(f64, f64)> {
    let Value::Cons(pair) = value else {
        return None;
    };
    let pair = pair.borrow();
    Some((state_float(&pair.car)?, state_float(&pair.cdr)?))
}

fn window_config_tree(window: &WindowRef, keys: &[Value]) -> Value {
    let window_data = window.borrow();
    Value::vector(vec![
        Value::Window(window.clone()),
        window_data
            .horiz
            .map_or(Value::Nil, |horizontal| Value::Int(i128::from(horizontal))),
        Value::list(
            window_data
                .children
                .iter()
                .map(|child| window_config_tree(child, keys))
                .collect(),
        ),
        Value::from_bool(window_data.combination_limit),
        if window_data.horiz.is_some() {
            window_config_parameters(&window_data.params, keys)
        } else {
            Value::Nil
        },
        window_normal_size_value(&window_data),
    ])
}

struct SavedWindowTree {
    window: WindowRef,
    horizontal: Option<bool>,
    children: Vec<SavedWindowTree>,
    combination_limit: bool,
    parameters: Value,
    normal_size: Option<(f64, f64)>,
}

fn read_window_config_tree(
    value: &Value,
    seen: &mut std::collections::HashSet<usize>,
    leaves: &mut std::collections::HashSet<usize>,
) -> Option<SavedWindowTree> {
    let Value::Vec(vector) = value else {
        return None;
    };
    let fields = vector.borrow().clone();
    let Value::Window(window) = fields.first()? else {
        return None;
    };
    let id = window.borrow().id;
    if !seen.insert(id) {
        return None;
    }
    let horizontal = match fields.get(1)? {
        Value::Nil => None,
        Value::Int(0) => Some(false),
        Value::Int(1) => Some(true),
        _ => return None,
    };
    let children = fields
        .get(2)?
        .list_to_vec()
        .ok()?
        .iter()
        .map(|value| read_window_config_tree(value, seen, leaves))
        .collect::<Option<Vec<_>>>()?;
    if horizontal.is_none() != children.is_empty() {
        return None;
    }
    if horizontal.is_none() {
        leaves.insert(id);
    }
    Some(SavedWindowTree {
        window: window.clone(),
        horizontal,
        children,
        combination_limit: fields.get(3).is_some_and(Value::truthy),
        parameters: fields.get(4).cloned().unwrap_or(Value::Nil),
        normal_size: fields.get(5).and_then(read_window_normal_size),
    })
}

fn restore_window_config_tree(tree: &SavedWindowTree, parent: Option<&WindowRef>) -> WindowRef {
    let children: Vec<_> = tree
        .children
        .iter()
        .map(|child| restore_window_config_tree(child, Some(&tree.window)))
        .collect();
    let bounds = children
        .iter()
        .map(|child| {
            let child = child.borrow();
            (
                child.left,
                child.top,
                child.left.saturating_add(child.width),
                child.top.saturating_add(child.height),
            )
        })
        .reduce(|(l, t, r, b), (cl, ct, cr, cb)| (l.min(cl), t.min(ct), r.max(cr), b.max(cb)));
    let mut window = tree.window.borrow_mut();
    window.dead = false;
    window.parent = parent.map(Rc::downgrade);
    window.horiz = tree.horizontal;
    window.children = children;
    window.combination_limit = tree.combination_limit;
    if let Some(normal_size) = tree.normal_size {
        window.normal_size = normal_size;
    }
    if tree.horizontal.is_some() {
        restore_window_config_parameters(&mut window.params, &tree.parameters);
    }
    if let Some((left, top, right, bottom)) = bounds {
        window.left = left;
        window.top = top;
        window.width = right.saturating_sub(left);
        window.height = bottom.saturating_sub(top);
        window.new_total = window.height as i128;
        window.new_pixel = window.height as i128;
    }
    drop(window);
    tree.window.clone()
}

fn window_configuration(i: &mut Interp) -> Value {
    let tag = Value::Sym(i.q("window-configuration"));
    let persistent_keys = window_config_persistent_keys(i);
    let frame_v = i
        .selected_frame
        .as_ref()
        .map(|f| Value::Frame(f.clone()))
        .unwrap_or(Value::Nil);
    let mut specs = Vec::new();
    let mut selected = Value::Nil;
    let mut tree = Value::Nil;
    let mut frame_size = Value::Nil;
    let mut change_stamp = Value::Int(0);
    if let Some(f) = i.selected_frame.as_ref() {
        let fr = f.borrow();
        change_stamp = Value::Int(fr.change_stamp.get() as i128);
        tree = fr.root.as_ref().map_or(Value::Nil, |root| {
            window_config_tree(root, &persistent_keys)
        });
        let menu_lines = i
            .intern_soft("menu-bar-lines")
            .and_then(|symbol| {
                crate::lisp::eval::plist_get_v(&fr.params, &Value::Sym(symbol)).int()
            })
            .unwrap_or(1);
        frame_size = Value::vector(vec![
            Value::Int(fr.width as i128),
            Value::Int(fr.height as i128),
            Value::Int(menu_lines),
        ]);
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
                .and_then(|b| b.borrow().mark_position())
                .map(|m| Value::Int(m as i128))
                .unwrap_or(Value::Nil);
            let insertion_type = window_config_point_insertion_type(i, wb.buffer);
            specs.push(Value::list(vec![
                Value::Window(w.clone()),
                Value::Int(wb.buffer as i128),
                window_config_marker(i, wb.buffer, point, insertion_type),
                window_config_marker(i, wb.buffer, wb.start, false),
                Value::Int(wb.hscroll as i128),
                Value::Int(wb.top as i128),
                Value::Int(wb.height as i128),
                Value::Int(wb.left as i128),
                Value::Int(wb.width as i128),
                wb.dedicated.clone(),
                Value::from_bool(wb.minibuffer),
                window_config_parameters(&wb.params, &persistent_keys),
                wb.prev_buffers.clone(),
                wb.next_buffers.clone(),
                mark,
                Value::Int(wb.margins.0 as i128),
                Value::Int(wb.margins.1 as i128),
                Value::Int(wb.min_hscroll as i128),
                Value::from_bool(wb.suspend_hscroll),
                window_config_marker(i, wb.buffer, wb.old_point_position(), insertion_type),
                window_normal_size_value(&wb),
                wb.old_buffer.clone(),
            ]));
        }
        // GNU configs record the minibuffer window's geometry too; it
        // restores to its saved rectangle (no rescaling).
        if let Some(m) = &fr.minibuffer {
            let mb = m.borrow();
            specs.push(Value::list(vec![
                Value::Window(m.clone()),
                Value::Int(mb.buffer as i128),
                window_config_marker(i, mb.buffer, mb.point, false),
                window_config_marker(i, mb.buffer, mb.start, false),
                Value::Int(mb.hscroll as i128),
                Value::Int(mb.top as i128),
                Value::Int(mb.height as i128),
                Value::Int(mb.left as i128),
                Value::Int(mb.width as i128),
                mb.dedicated.clone(),
                Value::Sym(sym::T),
                window_config_parameters(&mb.params, &persistent_keys),
                mb.prev_buffers.clone(),
                mb.next_buffers.clone(),
                Value::Nil,
                Value::Int(mb.margins.0 as i128),
                Value::Int(mb.margins.1 as i128),
                Value::Int(mb.min_hscroll as i128),
                Value::from_bool(mb.suspend_hscroll),
                window_config_marker(i, mb.buffer, mb.old_point_position(), false),
                window_normal_size_value(&mb),
                mb.old_buffer.clone(),
            ]));
        }
    }
    let configuration = Value::record(vec![
        tag,
        frame_v,
        Value::list(specs),
        selected,
        // GNU save_window_data also records the current buffer;
        // `set-window-configuration' restores it via
        // `set_buffer_internal' (slot index 4).
        i.buffer_value(i.current_buffer).unwrap_or(Value::Nil),
        tree,
        frame_size,
        change_stamp,
    ]);
    register_window_configuration(&configuration);
    configuration
}

/// Numeric/marker field N, with positions kept 0-based internally.
fn spec_int(v: &[Value], n: usize) -> i128 {
    match v.get(n) {
        Some(Value::Int(x)) => *x,
        Some(Value::Marker(marker)) => marker.borrow().position as i128,
        _ => 0,
    }
}

fn restore_window_view(i: &Interp, window: &mut Window, spec: &[Value], restore_positions: bool) {
    if let Some(normal_size) = spec.get(20).and_then(read_window_normal_size) {
        window.normal_size = normal_size;
    }
    if restore_positions {
        let mut old_point = None;
        if let Some(buffer) = i.buffers.get(window.buffer) {
            let buffer = buffer.borrow();
            let clip = |n| (spec_int(spec, n).max(0) as usize).clamp(buffer.begv, buffer.zv);
            window.point = clip(2);
            window.start = clip(3);
            if spec.len() > 19 {
                old_point = Some(clip(19));
            }
        }
        if let Some(old_point) = old_point {
            set_window_old_point(i, window, old_point);
        }
    }
    window.hscroll = spec_int(spec, 4).max(0) as usize;
    if spec.len() > 16 {
        window.margins = (
            spec_int(spec, 15).max(0) as usize,
            spec_int(spec, 16).max(0) as usize,
        );
    }
    if spec.len() > 17 {
        window.min_hscroll = spec_int(spec, 17).max(0) as usize;
    }
    if let Some(value) = spec.get(18) {
        window.suspend_hscroll = value.truthy();
    }
}

fn window_config_parts(tag: SymId, v: &Value) -> Option<(Value, Vec<Value>, Value)> {
    if !window_configuration_object(v) {
        return None;
    }
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

/// Saved current buffer of a config record (slot 4), or None for
/// configs written before the slot existed.
fn window_config_curbuf(tag: SymId, v: &Value) -> Option<Value> {
    if !window_configuration_object(v) {
        return None;
    }
    let r = match v {
        Value::Record(r) => r.borrow(),
        _ => return None,
    };
    if !matches!(r.first(), Some(Value::Sym(s)) if *s == tag) {
        return None;
    }
    r.get(4).cloned()
}

fn f_window_configuration_p(_i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    Ok(Value::from_bool(window_configuration_object(&a[0])))
}

pub(crate) fn f_current_window_configuration(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // Optional FRAME: frame-live-p check.
    if let Some(v) = a.first() {
        match v {
            Value::Nil | Value::Frame(_) => {}
            other => return Err(i.wrong_type_mut("frame-live-p", other)),
        }
    }
    Ok(window_configuration(i))
}

pub(crate) fn f_set_window_configuration(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let tag = i.q("window-configuration");
    let (_frame_v, specs, selected) = match window_config_parts(tag, &a[0]) {
        Some(p) => p,
        None => return Err(i.wrong_type_mut("window-configuration-p", &a[0])),
    };
    // Parse the immutable topology before changing live windows. In
    // particular, reject duplicate/cyclic nodes and mismatched leaf sets.
    let tree_value = match &a[0] {
        Value::Record(record) => record.borrow().get(5).cloned(),
        _ => None,
    };
    let saved_tree = if let Some(value) = tree_value.filter(|value| !value.is_nil()) {
        let mut seen = std::collections::HashSet::new();
        let mut leaves = std::collections::HashSet::new();
        let tree = read_window_config_tree(&value, &mut seen, &mut leaves)
            .ok_or_else(|| i.wrong_type_mut("window-configuration-p", &a[0]))?;
        let expected: std::collections::HashSet<_> = specs
            .iter()
            .filter_map(|spec| {
                let fields = spec.list_to_vec().ok()?;
                if fields.get(10).is_some_and(Value::truthy) {
                    return None;
                }
                match fields.first()? {
                    Value::Window(window) => Some(window.borrow().id),
                    _ => None,
                }
            })
            .collect();
        if leaves != expected {
            return Err(i.wrong_type_mut("window-configuration-p", &a[0]));
        }
        Some(tree)
    } else {
        None
    };
    let f = sel_frame(i).ok_or_else(|| i.error("No frame"))?;
    let saved_change_stamp = match &a[0] {
        Value::Record(record) => record.borrow().get(7).and_then(Value::int).unwrap_or(0) as u64,
        _ => 0,
    };
    let restore_old_buffer = saved_change_stamp == f.borrow().change_stamp.get();
    // Preserve the saved current buffer's present point. If restoration
    // changes which window shows it as selected, preserve that final
    // window's own point rather than copying the old selected cursor.
    let preserved = match window_config_curbuf(tag, &a[0]) {
        Some(Value::Buffer(buffer)) if buffer.borrow().live => {
            let buffer_id = buffer.borrow().id;
            let mut point = buffer.borrow().point();
            if let Value::Window(final_window) = &selected {
                let old = f.borrow().selected.clone();
                let final_window = final_window.borrow();
                if !final_window.dead
                    && final_window.buffer == buffer_id
                    && final_window.id != old.borrow().id
                    && (i.current_buffer != buffer_id || old.borrow().buffer == buffer_id)
                {
                    point = final_window.point;
                }
            }
            Some((buffer_id, point))
        }
        _ => None,
    };
    // GNU swaps selected point into the window before unshowing the
    // old tree, then saves outgoing points to buffers in reverse order.
    let old_selected = f.borrow().selected.clone();
    let selected_buffer = old_selected.borrow().buffer;
    if let Some(buffer) = i.buffers.get(selected_buffer) {
        old_selected.borrow_mut().point = buffer.borrow().point();
    }
    let mut outgoing = f.borrow().windows.clone();
    if let Some(minibuffer) = &f.borrow().minibuffer {
        outgoing.push(minibuffer.clone());
    }
    for window in outgoing.iter().rev() {
        if !window.borrow().dead {
            unshow_window_buffer(i, window, Some(selected_buffer));
        }
    }
    // Kill live windows that the configuration does not contain, then
    // rebuild the frame's window list from the saved specs (GNU writes
    // the saved contents back into the same window objects).
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
    if let Some(root) = f.borrow().root.clone() {
        // The saved topology holds its own child lists and revives the
        // saved nodes. Unsaved internal windows must become dead too.
        mark_tree_dead(i, &root);
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
            wb.dedicated = v.get(9).cloned().unwrap_or(Value::Nil);
            if restore_old_buffer {
                wb.old_buffer = v.get(21).cloned().unwrap_or(Value::Nil);
            }
            if let Some(parameters) = v.get(11) {
                restore_window_config_parameters(&mut wb.params, parameters);
            }
            wb.dead = false;
            restore_window_view(i, &mut wb, &v, true);
            mini_top = Some(spec_int(&v, 5));
        }
    }
    let map = extent_of(&content_specs).zip(extent_of(&cur_specs)).map(
        |((st, sl, sb, sr), (ct, cl, cb, cr))| {
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
        },
    );
    for v in &content_specs {
        let w = match v.first() {
            Some(Value::Window(w)) => w.clone(),
            _ => continue,
        };
        {
            let mut wb = w.borrow_mut();
            let bid = spec_int(v, 1).max(0) as usize;
            // GNU set_window_configuration: a saved buffer that was
            // killed since the snapshot gets replaced like
            // replace_buffer_in_windows — other-buffer, then any live
            // buffer, then a fresh *scratch*.
            wb.buffer = if i.buffers.get(bid).map(|b| b.borrow().live).unwrap_or(false) {
                bid
            } else {
                i.buffers
                    .other(bid)
                    .or_else(|| i.buffers.list().first().copied())
                    .unwrap_or_else(|| i.create_buffer("*scratch*"))
            };
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
            // GNU set_window_configuration writes the pending sizes too:
            // `window-new-total'/`window-new-pixel' read these until the
            // next resize/redisplay overwrites them.  GNU stores the
            // window's BODY height (total minus the mode-line row).
            wb.new_total = (nh - 1).max(0);
            wb.new_pixel = (nh - 1).max(0);
            wb.dedicated = v.get(9).cloned().unwrap_or(Value::Nil);
            if restore_old_buffer {
                wb.old_buffer = v.get(21).cloned().unwrap_or(Value::Nil);
            }
            if let Some(parameters) = v.get(11) {
                restore_window_config_parameters(&mut wb.params, parameters);
            }
            wb.prev_buffers = v.get(12).cloned().unwrap_or(Value::Nil);
            wb.next_buffers = v.get(13).cloned().unwrap_or(Value::Nil);
            wb.dead = false;
            let saved_buffer_live = i.buffer_live(bid);
            restore_window_view(i, &mut wb, v, saved_buffer_live);
        }
        if let Some(Value::Int(m)) = v.get(14) {
            marks.push((spec_int(v, 1).max(0) as usize, (*m).max(0) as usize));
        }
        new_windows.push(w);
    }
    {
        let mut fr = f.borrow_mut();
        fr.windows = new_windows;
        // Detach stale leaf links, then reconstruct the saved tree
        // from the same window objects and preserved directions.
        for w in &fr.windows {
            let mut wb = w.borrow_mut();
            wb.parent = None;
            wb.children.clear();
            wb.horiz = None;
        }
        fr.root = if let Some(tree) = &saved_tree {
            Some(restore_window_config_tree(tree, None))
        } else {
            match fr.windows.len() {
                0 => None,
                1 => Some(fr.windows[0].clone()),
                _ => {
                    let mini_top = fr
                        .minibuffer
                        .as_ref()
                        .map(|m| m.borrow().top)
                        .unwrap_or(fr.height);
                    let top = fr.windows.iter().map(|w| w.borrow().top).min().unwrap_or(0);
                    let r =
                        Window::new_internal(false, 0, top, fr.width, mini_top.saturating_sub(top));
                    r.borrow_mut().children = fr.windows.clone();
                    for w in &fr.windows {
                        w.borrow_mut().parent = Some(Rc::downgrade(&r));
                    }
                    Some(r)
                }
            }
        };
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
        w.borrow_mut().use_time = next_use_time(i);
        let (bid, pt) = {
            let mut wb = w.borrow_mut();
            if let Some((buffer, point)) = preserved {
                if wb.buffer == buffer {
                    wb.point = point;
                }
            }
            (wb.buffer, wb.point)
        };
        if let Some(b) = i.buffers.get(bid) {
            {
                let mut buffer = b.borrow_mut();
                buffer.set_point(pt);
                buffer.last_selected_window = Some(Rc::downgrade(w));
            }
            i.set_current_buffer(bid);
        }
    }
    for (bid, pos) in marks {
        if let Some(b) = i.buffers.get(bid) {
            b.borrow_mut().set_mark(pos);
        }
    }
    // GNU set_window_configuration's tail: the config's saved current
    // buffer becomes current again when still live (dead → leave as is).
    if let Some(Value::Buffer(b)) = window_config_curbuf(tag, &a[0]) {
        let bid = b.borrow().id;
        if i.buffer_live(bid) {
            i.set_current_buffer(bid);
            if let Some((buffer, point)) = preserved {
                if buffer == bid {
                    b.borrow_mut().set_point(point);
                }
            }
        }
    }
    Ok(Value::Sym(sym::T))
}

fn window_config_tree_shape_equal(left: &Value, right: &Value) -> bool {
    if left.is_nil() || right.is_nil() {
        return left.is_nil() && right.is_nil();
    }
    let read = |value| {
        read_window_config_tree(
            value,
            &mut std::collections::HashSet::new(),
            &mut std::collections::HashSet::new(),
        )
    };
    let (Some(left), Some(right)) = (read(left), read(right)) else {
        return false;
    };
    let mut pending = vec![(&left, &right)];
    while let Some((left, right)) = pending.pop() {
        if left.horizontal != right.horizontal || left.children.len() != right.children.len() {
            return false;
        }
        pending.extend(left.children.iter().zip(&right.children));
    }
    true
}

fn window_config_frame_size_equal(left: Option<&Value>, right: Option<&Value>) -> bool {
    match (left, right) {
        (None, None) | (Some(Value::Nil), Some(Value::Nil)) => true,
        (Some(Value::Vec(left)), Some(Value::Vec(right))) => {
            let left = left.borrow();
            let right = right.borrow();
            left.len() == 3
                && right.len() == 3
                && left
                    .iter()
                    .zip(right.iter())
                    .all(|(a, b)| matches!((a,b), (Value::Int(a),Value::Int(b)) if a == b))
        }
        _ => false,
    }
}

/// The explicit layout comparison ignores scrolling, cursor positions and
/// window identities; ordinary equal retains native object identity.
pub(crate) fn window_configuration_layout_equal(left: &Value, right: &Value) -> bool {
    let (Value::Record(left), Value::Record(right)) = (left, right) else {
        return false;
    };
    if Rc::ptr_eq(left, right) {
        return true;
    }
    let left = left.borrow();
    let right = right.borrow();
    if !eq_values(
        left.get(1).unwrap_or(&Value::Nil),
        right.get(1).unwrap_or(&Value::Nil),
    ) || !eq_values(
        left.get(4).unwrap_or(&Value::Nil),
        right.get(4).unwrap_or(&Value::Nil),
    ) || !window_config_frame_size_equal(left.get(6), right.get(6))
        || !window_config_tree_shape_equal(
            left.get(5).unwrap_or(&Value::Nil),
            right.get(5).unwrap_or(&Value::Nil),
        )
    {
        return false;
    }
    let specs = |fields: &[Value]| fields.get(2).and_then(|value| value.list_to_vec().ok());
    let (Some(left_specs), Some(right_specs)) = (specs(&left), specs(&right)) else {
        return false;
    };
    if left_specs.len() != right_specs.len() {
        return false;
    }
    let selected_left = left.get(3).unwrap_or(&Value::Nil);
    let selected_right = right.get(3).unwrap_or(&Value::Nil);
    for (left, right) in left_specs.iter().zip(&right_specs) {
        let (Ok(left), Ok(right)) = (left.list_to_vec(), right.list_to_vec()) else {
            return false;
        };
        if eq_values(left.first().unwrap_or(&Value::Nil), selected_left)
            != eq_values(right.first().unwrap_or(&Value::Nil), selected_right)
        {
            return false;
        }
        // Buffer, complete geometry and margins define the visible layout.
        // GNU ignores point/start/hscroll, dedication and combination limits.
        for field in [1, 5, 6, 7, 8, 15, 16] {
            if !eq_values(
                left.get(field).unwrap_or(&Value::Nil),
                right.get(field).unwrap_or(&Value::Nil),
            ) {
                return false;
            }
        }
    }
    true
}

pub(crate) fn f_window_configuration_equal_p(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let tag = i.q("window-configuration");
    for value in &a[..2] {
        if window_config_parts(tag, value).is_none() {
            return Err(i.wrong_type_mut("window-configuration-p", value));
        }
    }
    Ok(Value::from_bool(window_configuration_layout_equal(
        &a[0], &a[1],
    )))
}

pub(crate) fn f_window_configuration_frame(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let tag = i.q("window-configuration");
    match window_config_parts(tag, &a[0]) {
        Some((frame_v, _, _)) => Ok(frame_v),
        None => Err(i.wrong_type_mut("window-configuration-p", &a[0])),
    }
}

/// `window-system' — GNU Fwindow_system: nil on a batch/tty frame,
/// but FRAME is decode_any_frame-checked first (non-frames are
/// `framep' errors).
fn f_window_system(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let frame = frame_of(i, &arg(&a, 0))?;
    let params = frame.borrow().params.clone();
    let window_system = i.q("window-system");
    Ok(crate::lisp::eval::plist_get_v(
        &params,
        &Value::Sym(window_system),
    ))
}
fn f_frame_terminal(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // Validate optional FRAME like GNU, then return its terminal.
    if let Some(v) = a.first() {
        let _ = frame_live(i, v)?;
    }
    Ok(terminal_token(i))
}

/// The tag name when V is one of our font objects: a record tagged
/// `font-spec', `font-entity', or `font-object'.
fn font_kind(i: &Interp, v: &Value) -> Option<String> {
    if v.record_kind() == Some(crate::lisp::value::RecordKind::FontSpec) {
        return Some("font-spec".into());
    }
    if let Value::Record(r) = v {
        if let Some(Value::Sym(tag)) = r.borrow().first() {
            let name = i.symbol_name(*tag);
            if matches!(name.as_str(), "font-entity" | "font-object") {
                return Some(name);
            }
        }
    }
    None
}

fn f_fontp(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let kind = font_kind(i, &a[0]);
    let ok = match a.get(1) {
        None | Some(Value::Nil) => kind.is_some(),
        Some(v @ Value::Sym(s)) => {
            // GNU: TYPE must be one of the font-extra-type names;
            // anything else signals font-extra-type on TYPE.
            let n = i.symbol_name(*s);
            match n.as_str() {
                "font-spec" | "font-entity" | "font-object" => kind.as_deref() == Some(n.as_str()),
                _ => return Err(i.wrong_type_mut("font-extra-type", v)),
            }
        }
        Some(v) => return Err(i.wrong_type_mut("font-extra-type", v)),
    };
    Ok(Value::from_bool(ok))
}

fn f_find_font(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU font.c: decode_live_frame(FRAME, arg 1) before the
    // font-spec check on arg 0; batch finds no matching font → nil.
    if let Some(v) = a.get(1) {
        let _ = frame_live(i, v)?;
    }
    if font_kind(i, &a[0]).as_deref() != Some("font-spec") {
        return Err(i.wrong_type_mut("font-spec", &a[0]));
    }
    Ok(Value::Nil)
}
fn f_list_fonts(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU font.c: decode_live_frame(FRAME, arg 1) fires before
    // the font-spec check on arg 0 (and before SEPARATE/COUNT too).
    if let Some(v) = a.get(1) {
        let _ = frame_live(i, v)?;
    }
    if font_kind(i, &a[0]).as_deref() != Some("font-spec") {
        return Err(i.wrong_type_mut("font-spec", &a[0]));
    }
    Ok(Value::Nil)
}

fn f_open_font(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU font.c: optional FRAME (arg 2) decodes via frame_live_p
    // before the font-entity check on FONT-ENTITY (arg 0).
    if let Some(v) = a.get(2) {
        let _ = frame_live(i, v)?;
    }
    if font_kind(i, &a[0]).as_deref() != Some("font-entity") {
        return Err(i.wrong_type_mut("font-entity", &a[0]));
    }
    Ok(Value::Nil)
}

fn f_query_font(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    if font_kind(i, &a[0]).as_deref() != Some("font-object") {
        return Err(i.wrong_type_mut("font-object", &a[0]));
    }
    Ok(Value::Nil)
}

fn f_set_minibuffer_window(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // The minibuffer window is fixed in our frame model.  Validate
    // its kind and return WINDOW itself, as GNU does.
    match &a[0] {
        Value::Window(window) if window.borrow().minibuffer => Ok(a[0].clone()),
        Value::Window(_) => Err(i.error("Window is not a minibuffer window")),
        other => Err(i.wrong_type_mut("windowp", other)),
    }
}

fn f_minibuffer_window_active_p(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // Active iff the minibuffer has contents / is selected.
    let _ = (i, a);
    Ok(Value::Nil)
}

pub(crate) fn f_split_window(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let w = win_err_valid(i, &arg(&a, 0))?;
    {
        let wb = w.borrow();
        if wb.dead || wb.horiz.is_some() {
            let shown = i.princ_to_string(&a[0]);
            return Err(i.error(format!("{shown} is not a valid window")));
        }
        if wb.minibuffer {
            return Err(i.error("Cannot split minibuffer window"));
        }
    }
    // GNU Fsplit_window: `above'/`below' split vertically, `left'
    // horizontally with the new window on the left, and ANY other
    // non-nil side (including `right', t, bogus) horizontally with
    // the new window on the right.
    let side = match a
        .get(2)
        .and_then(|v| i.sym_id(v))
        .map(|s| i.symbol_name(s))
        .as_deref()
    {
        Some("above") => 1,
        Some("left") => 3,
        Some("below") | None => 0,
        _ => 2,
    };
    let hor = side >= 2;
    // GNU: positive SIZE is the part the PARENT keeps; negative SIZE
    // is the size of the NEW window; nil halves the parent.
    let size = arg(&a, 1).int();
    let Some(f) = sel_frame(i) else {
        return Err(i.error("No frame"));
    };
    // Reject impossible splits before changing frame geometry, allocating a
    // window, or recording a buffer display. Nil SIZE honors the preferred
    // minimum; explicit SIZE is allowed down to the safe minimum.
    {
        let total = {
            let wb = w.borrow();
            (if hor { wb.width } else { wb.height }) as i128
        };
        let get_min = |i: &mut Interp, name: &str, default: i128| {
            let id = i.intern(name);
            i.symbol_value(id).int().unwrap_or(default)
        };
        let safe = if hor {
            get_min(i, "window-safe-min-width", 2).max(1)
        } else {
            get_min(i, "window-safe-min-height", 1).max(1)
        };
        let too_small = match size {
            Some(s) if s >= 0 => s < safe || s >= total || total - s < safe,
            Some(s) => s
                .checked_neg()
                .is_none_or(|new| new < safe || total - new < safe),
            None => {
                let decorations = if hor {
                    let margins = w.borrow().margins;
                    margins.0 as i128 + margins.1 as i128
                } else {
                    let buffer = w.borrow().buffer;
                    ["mode-line-format", "header-line-format", "tab-line-format"]
                        .iter()
                        .filter(|name| {
                            let id = i.intern(name);
                            i.buffers
                                .get(buffer)
                                .and_then(|b| b.borrow().locals.get(&id).cloned())
                                .unwrap_or_else(|| i.obarray.symbol(id).value.clone())
                                .truthy()
                        })
                        .count() as i128
                };
                let preferred = if hor {
                    get_min(i, "window-min-width", 10)
                } else {
                    get_min(i, "window-min-height", 4)
                }
                .max(safe.saturating_add(decorations));
                (total + 1) / 2 < preferred
            }
        };
        if too_small {
            let shown = i.princ_to_string(&Value::Window(w.clone()));
            return Err(i.error(format!("Window {shown} too small for splitting")));
        }
        // The low-level splitter still requires one row / two columns,
        // including when preferred and safe minima have been lowered.
        let (old, new) = match size {
            Some(s) if s >= 0 => (s, total - s),
            Some(s) => (total + s, -s),
            None => (total - total / 2, total / 2),
        };
        let floor = if hor { 2 } else { 1 };
        if old < floor {
            return Err(i.error("Resizing old window failed"));
        }
        if new < floor {
            return Err(i.error("Size of new window too small (after split)"));
        }
    }
    // GNU batch/tty quirk: the first *vertical* split materializes the
    // menu-bar line — the content area shifts down one row and the
    // minibuffer window moves down with it (`frame-height' keeps
    // reporting the old value; observed GNU -Q: 'below split gives
    // root (0 1 80 25), minibuffer (0 25 80 26)).  A horizontal
    // ('right/'left) split does NOT shift: GNU reports
    // (0 0 40 24)/(40 0 80 24).
    {
        let mut ff = f.borrow_mut();
        if !ff.menu_shift && !hor {
            let mut mtop = 0;
            for w2 in &ff.windows {
                let mut wb = w2.borrow_mut();
                wb.top += 1;
                mtop = mtop.max(wb.top + wb.height);
            }
            if let Some(r) = &ff.root {
                let mut rb = r.borrow_mut();
                if rb.horiz.is_some() {
                    rb.top += 1;
                    rb.height = mtop.saturating_sub(rb.top);
                }
            }
            if let Some(mb) = &ff.minibuffer {
                let top = mb.borrow().top;
                mb.borrow_mut().top = top + 1;
            }
            ff.menu_shift = true;
        }
    }
    let buf = w.borrow().buffer;
    let point = window_point(i, &w);
    let new = Window::new(buf);
    new.borrow_mut().change_frame = Some(f.borrow().id);
    new.borrow_mut().change_clock = Some(f.borrow().change_stamp.clone());
    record_buffer_display(i, buf);
    let (wl, wt, ww_, wh, normal_size) = {
        let wb = w.borrow();
        (wb.left, wb.top, wb.width, wb.height, wb.normal_size)
    };
    let total = if hor { ww_ } else { wh };
    let new_extent = match size {
        None => (total / 2) as f64,
        Some(size) if size >= 0 => (total as i128 - size) as f64,
        Some(size) => (-size) as f64,
    };
    let new_normal = if size.is_none() {
        0.5
    } else {
        new_extent / total as f64
    };
    let keep_normal = 1.0 - new_normal;
    // Geometry: halve W along the split axis and place NEW on SIDE.
    let (w_new_geo, n_geo) = {
        let (keep, sz) = if hor {
            let total = ww_;
            match size {
                Some(s) if s >= 0 => (
                    (s.max(0) as usize).min(total),
                    total.saturating_sub(s.max(0) as usize),
                ),
                Some(s) => {
                    let n = (-s).max(0) as usize;
                    (total.saturating_sub(n), n.min(total))
                }
                None => (total - total / 2, total / 2),
            }
        } else {
            let total = wh;
            match size {
                Some(s) if s >= 0 => (
                    (s.max(0) as usize).min(total),
                    total.saturating_sub(s.max(0) as usize),
                ),
                Some(s) => {
                    let n = (-s).max(0) as usize;
                    (total.saturating_sub(n), n.min(total))
                }
                None => (total - total / 2, total / 2),
            }
        };
        if hor {
            if side == 2 {
                // new window on the right
                ((wl, wt, keep, wh), (wl + keep, wt, sz, wh))
            } else {
                // `left': new window on the left
                ((wl + sz, wt, keep, wh), (wl, wt, sz, wh))
            }
        } else if side == 0 {
            // `below'
            ((wl, wt, ww_, keep), (wl, wt + keep, ww_, sz))
        } else {
            // `above'
            ((wl, wt + sz, ww_, keep), (wl, wt, ww_, sz))
        }
    };
    {
        let mut wb = w.borrow_mut();
        let mut nw = new.borrow_mut();
        nw.point = point;
        set_window_old_point(i, &mut nw, point);
        nw.start = wb.start;
        nw.left = n_geo.0;
        nw.top = n_geo.1;
        nw.width = n_geo.2;
        nw.height = n_geo.3;
        wb.left = w_new_geo.0;
        wb.top = w_new_geo.1;
        wb.width = w_new_geo.2;
        wb.height = w_new_geo.3;
    }
    // GNU Fsplit_window_internal: when W's parent is a combination in
    // the same direction without a combination limit, the new window
    // is grafted onto it; otherwise a fresh internal window replaces
    // W in the tree and parents both.
    let parent = w_parent(&w);
    let reuse = parent
        .as_ref()
        .map(|p| {
            let pb = p.borrow();
            pb.horiz == Some(hor) && !pb.combination_limit
        })
        .unwrap_or(false);
    if reuse {
        let p = parent.unwrap();
        {
            let original = if hor { normal_size.1 } else { normal_size.0 };
            let parent_total = {
                let parent = p.borrow();
                if hor { parent.width } else { parent.height }
            };
            let added = if size.is_none() {
                original / 2.0
            } else {
                new_extent / parent_total as f64
            };
            let kept = original - added;
            if hor {
                w.borrow_mut().normal_size.1 = kept;
                new.borrow_mut().normal_size.1 = added;
            } else {
                w.borrow_mut().normal_size.0 = kept;
                new.borrow_mut().normal_size.0 = added;
            }
        }
        let mut pb = p.borrow_mut();
        let pos = pb
            .children
            .iter()
            .position(|k| Rc::ptr_eq(k, &w))
            .unwrap_or(pb.children.len().saturating_sub(1));
        let at = pos + usize::from(side == 0 || side == 2);
        pb.children.insert(at, new.clone());
        new.borrow_mut().parent = Some(Rc::downgrade(&p));
    } else {
        let p = Window::new_internal(hor, wl, wt, ww_, wh);
        p.borrow_mut().change_frame = Some(f.borrow().id);
        p.borrow_mut().change_clock = Some(f.borrow().change_stamp.clone());
        w.borrow_mut().normal_size = if hor {
            (1.0, keep_normal)
        } else {
            (keep_normal, 1.0)
        };
        new.borrow_mut().normal_size = if hor {
            (1.0, new_normal)
        } else {
            (new_normal, 1.0)
        };
        {
            let mut pb = p.borrow_mut();
            pb.normal_size = normal_size;
            pb.parent = w.borrow().parent.clone();
            // below/right → (W NEW); above/left → (NEW W).
            if side == 0 || side == 2 {
                pb.children = vec![w.clone(), new.clone()];
            } else {
                pb.children = vec![new.clone(), w.clone()];
            }
        }
        match parent {
            Some(par) => {
                let mut parb = par.borrow_mut();
                for k in parb.children.iter_mut() {
                    if Rc::ptr_eq(k, &w) {
                        *k = p.clone();
                    }
                }
            }
            None => {
                // W was the root (a lone leaf): the new internal
                // window becomes the frame root.
                f.borrow_mut().root = Some(p.clone());
            }
        }
        w.borrow_mut().parent = Some(Rc::downgrade(&p));
        new.borrow_mut().parent = Some(Rc::downgrade(&p));
    }
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
    drop(ff);
    // GNU's internal buffer installation runs the primitive hook;
    // the public Lisp splitter then calls the public runner as well.
    // Release frame borrows before either notification enters Lisp.
    winxtra::f_run_window_scroll_functions(i, vec![Value::Window(new.clone())].into())?;
    let run_scroll = i.q("run-window-scroll-functions");
    i.apply(&Value::Sym(run_scroll), vec![Value::Window(new.clone())])?;
    Ok(Value::Window(new))
}

/// Re-lay out the children of internal window P so they tile P's
/// rect along its split axis (proportional to their current sizes).
pub(crate) fn relay_children(p: &WindowRef) {
    let (horiz, pl, pt, pw, ph) = {
        let pb = p.borrow();
        (pb.horiz == Some(true), pb.left, pb.top, pb.width, pb.height)
    };
    let kids = p.borrow().children.clone();
    let total: usize = kids
        .iter()
        .map(|k| {
            let kb = k.borrow();
            if horiz { kb.width } else { kb.height }
        })
        .sum();
    let total = total.max(1);
    let mut pos = if horiz { pl } else { pt };
    let end = pos + if horiz { pw } else { ph };
    let n = kids.len();
    for (idx, k) in kids.iter().enumerate() {
        let mut kb = k.borrow_mut();
        let sz = if horiz { kb.width } else { kb.height };
        let nsz = if idx + 1 == n {
            end.saturating_sub(pos)
        } else {
            ((if horiz { pw } else { ph }) * sz / total).max(1)
        };
        if horiz {
            kb.left = pos;
            kb.width = nsz;
            kb.top = pt;
            kb.height = ph;
        } else {
            kb.top = pos;
            kb.height = nsz;
            kb.left = pl;
            kb.width = pw;
        }
        pos += nsz;
        if kb.horiz.is_some() {
            drop(kb);
            relay_children(k);
        }
    }
}

/// Recursively mark every window in W's subtree dead (for removing
/// an internal node: its whole combination dies with it).
pub(crate) fn mark_tree_dead(i: &Interp, w: &WindowRef) {
    let kids = {
        let mut wb = w.borrow_mut();
        if !wb.dead && wb.horiz.is_none() {
            wb.old_buffer = i.buffer_value(wb.buffer).unwrap_or(Value::Nil);
        }
        wb.dead = true;
        std::mem::take(&mut wb.children)
    };
    for k in kids {
        mark_tree_dead(i, &k);
    }
}

fn materialize_menu_bar_after_root_collapse(frame: &mut Frame) {
    if frame.menu_shift {
        return;
    }
    let Some(root) = frame.root.clone() else {
        return;
    };
    if root.borrow().horiz.is_some() {
        return;
    }
    root.borrow_mut().top += 1;
    if let Some(minibuffer) = &frame.minibuffer {
        minibuffer.borrow_mut().top += 1;
    }
    frame.menu_shift = true;
}

fn f_delete_window(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let w = win_err_valid(i, &arg(&a, 0))?;
    {
        let wb = w.borrow();
        if wb.dead {
            let shown = i.princ_to_string(&a[0]);
            return Err(i.error(format!("{shown} is not a valid window")));
        }
        if wb.minibuffer {
            return Err(i.error("Attempt to delete the minibuffer window"));
        }
        if wb.horiz.is_some() {
            return Err(i.error("Attempt to delete an internal window"));
        }
    }
    let wid = w.borrow().id;
    let f = sel_frame(i).unwrap();
    let mut ff = f.borrow_mut();
    if ff.windows.len() <= 1 {
        return Err(i.error("Attempt to delete minibuffer or sole ordinary window"));
    }
    let root_was_internal = ff
        .root
        .as_ref()
        .is_some_and(|root| root.borrow().horiz.is_some());
    let del_idx = ff
        .windows
        .iter()
        .position(|w2| w2.borrow().id == wid)
        .unwrap_or(0);
    if let Some(p) = w_parent(&w) {
        let mut pb = p.borrow_mut();
        let pos = pb
            .children
            .iter()
            .position(|k| Rc::ptr_eq(k, &w))
            .unwrap_or(0);
        let horiz = pb.horiz == Some(true);
        let (dsize, dl, dt) = {
            let d = w.borrow();
            (if horiz { d.width } else { d.height }, d.left, d.top)
        };
        // GNU gives the deleted window's space to its sibling:
        // prefer the previous one, else the next.
        let target = if pos > 0 {
            pb.children.get(pos - 1).cloned()
        } else {
            pb.children.get(pos + 1).cloned()
        };
        if let Some(t) = &target {
            let (tl, tt, tw, th) = {
                let tb = t.borrow();
                (tb.left, tb.top, tb.width, tb.height)
            };
            let mut tb = t.borrow_mut();
            if horiz {
                tb.normal_size.1 += w.borrow().normal_size.1;
            } else {
                tb.normal_size.0 += w.borrow().normal_size.0;
            }
            if horiz {
                if tl + tw == dl {
                    // T directly left of W: extend right.
                    tb.width += dsize;
                } else {
                    // T directly right of W: extend left.
                    tb.left = dl;
                    tb.width += dsize;
                }
            } else if tt + th == dt {
                // T directly above W: extend down.
                tb.height += dsize;
            } else {
                // T directly below W: extend up.
                tb.top = dt;
                tb.height += dsize;
            }
        }
        pb.children.retain(|k| !Rc::ptr_eq(k, &w));
        drop(pb);
        // GNU collapses a single-child combination: the remaining child
        // takes the parent's slot and geometry, recursively upward.
        let mut cur = p;
        loop {
            let (pl, pt, pw, ph, pparent, kids, normal_size) = {
                let cb = cur.borrow();
                (
                    cb.left,
                    cb.top,
                    cb.width,
                    cb.height,
                    cb.parent.clone(),
                    cb.children.clone(),
                    cb.normal_size,
                )
            };
            if kids.len() != 1 {
                break;
            }
            let sole = kids[0].clone();
            {
                let mut sb = sole.borrow_mut();
                sb.left = pl;
                sb.top = pt;
                sb.width = pw;
                sb.height = ph;
                sb.parent = pparent.clone();
                sb.normal_size = normal_size;
            }
            {
                let mut cb = cur.borrow_mut();
                cb.children.clear();
                cb.dead = true;
            }
            match pparent.and_then(|p| p.upgrade()) {
                Some(gp) => {
                    {
                        let mut gpb = gp.borrow_mut();
                        for k in gpb.children.iter_mut() {
                            if Rc::ptr_eq(k, &cur) {
                                *k = sole.clone();
                            }
                        }
                    }
                    cur = gp;
                }
                None => {
                    ff.root = Some(sole);
                    break;
                }
            }
        }
    }
    w.borrow_mut().parent = None;
    let old_buffer = i.buffer_value(w.borrow().buffer).unwrap_or(Value::Nil);
    w.borrow_mut().old_buffer = old_buffer;
    w.borrow_mut().dead = true;
    let sel_dead = ff.selected.borrow().id == wid;
    ff.windows.retain(|w2| w2.borrow().id != wid);
    if sel_dead {
        // GNU: deleting the selected window selects the window
        // FOLLOWING it in the frame's window chain.
        let idx = del_idx % ff.windows.len().max(1);
        ff.selected = ff.windows[idx].clone();
    }
    if root_was_internal && ff.windows.len() == 1 {
        materialize_menu_bar_after_root_collapse(&mut ff);
    }
    Ok(Value::Nil)
}

fn f_delete_other_windows(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let w = win_err_valid(i, &arg(&a, 0))?;
    let wid = w.borrow().id;
    let old_buffer = w.borrow().old_buffer.clone();
    let f = sel_frame(i).unwrap();
    let mut ff = f.borrow_mut();
    let root_was_internal = ff
        .root
        .as_ref()
        .is_some_and(|root| root.borrow().horiz.is_some());
    // GNU expands the kept window over the whole root area: the
    // frame's root window extent.
    let (rl, rt, rw, rh) = ff
        .root
        .as_ref()
        .map(|r| {
            let rb = r.borrow();
            (rb.left, rb.top, rb.width, rb.height)
        })
        .unwrap_or_else(|| {
            let mb_top = ff
                .minibuffer
                .as_ref()
                .map(|m| m.borrow().top)
                .unwrap_or(ff.height);
            (0, 0, ff.width, mb_top)
        });
    // Kill every other leaf and the internal nodes above W.
    let dead_internal: Vec<WindowRef> = if let Some(r) = &ff.root {
        if r.borrow().horiz.is_some() && !Rc::ptr_eq(r, &w) {
            vec![r.clone()]
        } else {
            Vec::new()
        }
    } else {
        Vec::new()
    };
    for r in dead_internal {
        mark_tree_dead(i, &r);
    }
    ff.windows.retain(|w2| {
        let keep = w2.borrow().id == wid;
        if !keep {
            w2.borrow_mut().dead = true;
        }
        keep
    });
    {
        let mut wb = w.borrow_mut();
        wb.left = rl;
        wb.top = rt;
        wb.width = rw;
        wb.height = rh;
        wb.parent = None;
        wb.dead = false;
        wb.old_buffer = old_buffer;
        wb.normal_size = (1.0, 1.0);
    }
    ff.root = Some(w.clone());
    ff.selected = w.clone();
    if root_was_internal {
        materialize_menu_bar_after_root_collapse(&mut ff);
    }
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

fn f_other_window(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let n = want_num_or_mark(i, &arg(&a, 0))?;
    let sel = sel_window(i).unwrap();
    let mut cur = sel;
    for _ in 0..n.abs() {
        cur = window_cycle(i, n.signum(), &cur).unwrap_or(cur);
    }
    f_select_window(i, vec![Value::Window(cur.clone())].into())?;
    // GNU -Q --batch: other-window selects the window but returns nil.
    if i.noninteractive {
        Ok(Value::Nil)
    } else {
        Ok(Value::Window(cur))
    }
}

pub(crate) fn f_select_window(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let w = win_live_req(i, &a[0])?;
    let f = frame_of_window(i, &w).ok_or_else(|| i.error("No frame"))?;
    let old_sel = sel_window(i);
    let same_window = old_sel.as_ref().is_some_and(|old| Rc::ptr_eq(old, &w));
    if !same_window {
        // GNU saves the old displayed buffer's point even when a different
        // buffer was made current with `set-buffer'.
        if let Some(old) = old_sel {
            let old_bid = old.borrow().buffer;
            if let Some(buffer) = i.buffers.get(old_bid) {
                old.borrow_mut().point = buffer.borrow().point();
            }
        }
        let changed_frame = i
            .selected_frame
            .as_ref()
            .is_none_or(|old| !Rc::ptr_eq(old, &f));
        if changed_frame {
            let previous = f.borrow().selected.clone();
            f.borrow_mut().old_selected = Some(previous);
        }
        f.borrow_mut().selected = w.clone();
        i.selected_frame = Some(f);
        let (bid, point) = {
            let wb = w.borrow();
            (wb.buffer, wb.point)
        };
        if let Some(buffer) = i.buffers.get(bid) {
            let mut buffer = buffer.borrow_mut();
            buffer.set_point(point);
            buffer.last_selected_window = Some(Rc::downgrade(&w));
        }
    }
    let bid = w.borrow().buffer;
    i.set_current_buffer(bid);
    // Selecting the already-selected window records selection but must not
    // overwrite the buffer's current point with a stale window point.
    if arg(&a, 1).is_nil() {
        w.borrow_mut().use_time = next_use_time(i);
        record_buffer_selection(i, bid)?;
    }
    Ok(a[0].clone())
}

fn f_window_use_time(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let w = win_live(i, &arg(&a, 0))?;
    Ok(Value::Int(w.borrow().use_time as i128))
}

fn f_one_window_p(i: &mut Interp, _a: crate::lisp::Argv) -> EvalResult {
    let f = sel_frame(i).unwrap();
    Ok(Value::from_bool(f.borrow().windows.len() == 1))
}

fn next_window_in_cycle(i: &mut Interp, a: crate::lisp::Argv, forward: bool) -> EvalResult {
    let from = win_live(i, &arg(&a, 0))?;
    let Some(origin) = frame_of_window(i, &from) else {
        return Ok(Value::Window(from));
    };
    let minibuf = arg(&a, 1);
    let mini = origin.borrow().minibuffer.clone();
    let active_mini = if minibuf.is_nil() && i.minibuf_level > 0 {
        sel_frame(i).and_then(|f| f.borrow().minibuffer.clone())
    } else {
        None
    };
    let include_all_mini = i.sym_id(&minibuf) == Some(sym::T);
    let all_frames = arg(&a, 2);
    let frames = match &all_frames {
        Value::Frame(f) => {
            let f = frame_live(i, &Value::Frame(f.clone()))?;
            if !Rc::ptr_eq(&f, &origin) {
                let first = f.borrow().windows.first().cloned();
                return Ok(first.map(Value::Window).unwrap_or(Value::Window(from)));
            }
            vec![f]
        }
        Value::Sym(s) if *s == sym::T || i.symbol_name(*s) == "visible" => i.frames.clone(),
        Value::Int(0) => i.frames.clone(),
        Value::Nil if include_all_mini || active_mini.is_some() => i
            .frames
            .iter()
            .filter(|f| {
                Rc::ptr_eq(f, &origin)
                    || mini.as_ref().is_some_and(|m| {
                        f.borrow()
                            .minibuffer
                            .as_ref()
                            .is_some_and(|other| Rc::ptr_eq(m, other))
                    })
            })
            .cloned()
            .collect(),
        _ => vec![origin],
    };
    // Frames in the current terminal model are visible. Keep minibuffer
    // windows in the ring even when excluded, so FROM can locate one.
    let mut windows = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for frame in frames {
        let frame = frame.borrow();
        if frame.dead {
            continue;
        }
        for window in frame.windows.iter().chain(frame.minibuffer.iter()) {
            let w = window.borrow();
            if !w.dead && w.horiz.is_none() && seen.insert(w.id) {
                windows.push(window.clone());
            }
        }
    }
    let Some(start) = windows.iter().position(|w| Rc::ptr_eq(w, &from)) else {
        return Ok(Value::Window(from));
    };
    for step in 1..=windows.len() {
        let offset = if forward { step } else { windows.len() - step };
        let candidate = &windows[(start + offset) % windows.len()];
        if !candidate.borrow().minibuffer
            || include_all_mini
            || active_mini
                .as_ref()
                .is_some_and(|m| Rc::ptr_eq(m, candidate))
        {
            return Ok(Value::Window(candidate.clone()));
        }
    }
    Ok(Value::Window(from))
}

fn f_next_window(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    next_window_in_cycle(i, a, true)
}

fn f_previous_window(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    next_window_in_cycle(i, a, false)
}

fn f_walk_windows(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let fun = a[0].clone();
    let f = sel_frame(i).unwrap();
    let ws: Vec<WindowRef> = f.borrow().windows.clone();
    for w in ws {
        i.apply(&fun, vec![Value::Window(w)])?;
    }
    Ok(Value::Nil)
}

/// Live windows in cyclic display order, starting at the selected window
/// (or the specified frame's selected window).  All current tty frames are
/// visible; their minibuffers occur after the ordinary leaf windows.
fn buffer_window_cycle(i: &Interp, all_frames: &Value, minibuf: bool) -> Vec<WindowRef> {
    let frames = match all_frames {
        Value::Frame(f) => vec![f.clone()],
        Value::Nil => i.selected_frame.iter().cloned().collect(),
        _ => i.frames.clone(),
    };
    let start = match all_frames {
        Value::Frame(f) => Some(f.borrow().selected.clone()),
        _ => sel_window(i),
    };
    let mut windows = Vec::new();
    for f in frames {
        let f = f.borrow();
        if f.dead {
            continue;
        }
        for w in f.windows.iter().chain(f.minibuffer.iter()) {
            let wb = w.borrow();
            if !wb.dead && wb.horiz.is_none() && (!wb.minibuffer || minibuf) {
                windows.push(w.clone());
            }
        }
    }
    if let Some(start) = start {
        if let Some(pos) = windows.iter().position(|w| Rc::ptr_eq(w, &start)) {
            windows.rotate_left(pos);
        }
    }
    windows
}

pub(crate) fn f_get_buffer_window(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let bid = match a.first() {
        None | Some(Value::Nil) => i.current_buffer,
        Some(Value::Buffer(b)) => b.borrow().id,
        Some(Value::Str(s)) => match i.buffers.by_name(&s.borrow()) {
            Some(b) => b,
            None => return Ok(Value::Nil),
        },
        Some(other) => return Err(i.wrong_type_mut("stringp", other)),
    };
    if !i.buffer_live(bid) {
        return Ok(Value::Nil);
    }
    let windows = buffer_window_cycle(i, &arg(&a, 1), i.minibuf_level > 0);
    let selected = sel_window(i);
    let selected_frame = sel_frame(i);
    let mut best = None;
    let mut best_on_selected_frame = false;
    for w in windows {
        if w.borrow().buffer != bid {
            continue;
        }
        if selected.as_ref().is_some_and(|s| Rc::ptr_eq(s, &w)) {
            return Ok(Value::Window(w));
        }
        let on_selected_frame = frame_of_window(i, &w)
            .is_some_and(|f| selected_frame.as_ref().is_some_and(|s| Rc::ptr_eq(s, &f)));
        if best.is_none() || (on_selected_frame && !best_on_selected_frame) {
            best = Some(w);
            best_on_selected_frame = on_selected_frame;
        }
    }
    Ok(best.map(Value::Window).unwrap_or(Value::Nil))
}

fn f_get_buffer_window_list(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU window.el's Lisp version: (BUFFER-OR-NAME MINIBUF
    // ALL-FRAMES INDIRECT).  Buffer normalization matches
    // `window-normalize-buffer': nil → current buffer; a dead buffer
    // object or a name with no live buffer → `error "No such live
    // buffer"'; a non-string non-buffer → `stringp' type error.
    let bid = match a.first() {
        None => i.current_buffer,
        Some(Value::Nil) => i.current_buffer,
        Some(Value::Buffer(b)) => {
            let id = b.borrow().id;
            if !i.buffer_live(id) {
                return Err(i.error(format!("No such live buffer {}", i.prin1_to_string(&a[0]))));
            }
            id
        }
        Some(Value::Str(s)) => match i.buffers.by_name(&s.borrow()) {
            Some(id) => id,
            None => return Err(i.error(format!("No such live buffer {}", s.borrow()))),
        },
        // GNU `window-normalize-buffer' reports a marker/number arg with
        // `error "No such buffer %s"' (prin1 of the arg), not a type error.
        Some(other) => {
            let repr = i.prin1_to_string(other);
            return Err(i.error(format!("No such buffer {}", repr)));
        }
    };
    // MINIBUF: t → always include the minibuffer window, nil → only
    // while the minibuffer is active, anything else → never.
    let minibuf_ok = match a.get(1) {
        Some(v) if v.truthy() => i.sym_is(v, crate::lisp::obarray::sym::T),
        _ => i.minibuf_level > 0,
    };
    let windows = buffer_window_cycle(i, &arg(&a, 2), minibuf_ok);
    let indirect = a.get(3).map(|v| v.truthy()).unwrap_or(false);
    let base_id = |id: usize| {
        i.buffers
            .get(id)
            .and_then(|b| b.borrow().base_buffer.as_ref().map(|bb| bb.borrow().id))
    };
    let buf_base = base_id(bid);
    let mut out: Vec<Value> = Vec::new();
    for w in windows {
        let wb = w.borrow();
        if wb.buffer == bid {
            out.push(Value::Window(w.clone()));
        } else if indirect {
            let wbase = base_id(wb.buffer);
            if buf_base == Some(wb.buffer)
                || wbase == Some(bid)
                || (buf_base.is_some() && buf_base == wbase)
            {
                out.push(Value::Window(w.clone()));
            }
        }
    }
    Ok(Value::list(out))
}

fn f_window_height(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let w = win_of(i, &arg(&a, 0))?;
    Ok(Value::Int(w.borrow().height as i128))
}
fn f_window_body_height(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let w = win_live(i, &arg(&a, 0))?;
    let header = winxtra::window_format_line_height(i, &w, "header-line-format")?;
    let tab = winxtra::window_format_line_height(i, &w, "tab-line-format")?;
    // Body excludes the header, tab, and mode lines.
    let mode_line = winxtra::window_mode_line_height(i, &w)?;
    let height = w.borrow().height;
    let body_height = height.saturating_sub(mode_line.saturating_add(header).saturating_add(tab));
    if a.get(1).is_some_and(Value::truthy) {
        let frame = frame_of_window(i, &w).ok_or_else(|| i.error("No frame"))?;
        let total_pixels = (height as f64 * winxtra::window_pixel_scale_y(i, &w)).round();
        let decoration_pixels =
            (mode_line + header + tab).saturating_mul(frame_char_height(i, &frame)) as f64;
        Ok(Value::Int(
            (total_pixels - decoration_pixels).max(0.0) as i128,
        ))
    } else {
        Ok(Value::Int(body_height as i128))
    }
}
/// Body width excludes margins and the one-column window divider that
/// separates horizontally adjacent windows on a tty.
fn win_body_width(i: &Interp, w: &WindowRef) -> usize {
    let ww = w.borrow();
    let frame_w = i
        .frames
        .iter()
        .find(|f| f.borrow().windows.iter().any(|x| Rc::ptr_eq(x, w)))
        .map(|f| f.borrow().width);
    let margins = ww.margins.0.saturating_add(ww.margins.1);
    let divider = usize::from(frame_w.is_some_and(|fw| ww.left + ww.width < fw));
    ww.width.saturating_sub(margins).saturating_sub(divider)
}

fn f_window_width(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let w = win_of(i, &arg(&a, 0))?;
    Ok(Value::Int(win_body_width(i, &w) as i128))
}
fn f_window_total_width(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let w = win_of(i, &arg(&a, 0))?;
    Ok(Value::Int(w.borrow().width as i128))
}
fn f_window_body_width(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let w = win_live(i, &arg(&a, 0))?;
    let body_width = win_body_width(i, &w);
    if a.get(1).is_some_and(Value::truthy) {
        Ok(Value::Int(
            (body_width as f64 * winxtra::window_pixel_scale_x(i, &w)).round() as i128,
        ))
    } else {
        Ok(Value::Int(body_width as i128))
    }
}
fn f_window_text_width(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let w = win_live(i, &arg(&a, 0))?;
    let wb = w.borrow();
    let margins = wb.margins.0.saturating_add(wb.margins.1);
    Ok(Value::Int(wb.width.saturating_sub(margins) as i128))
}
fn f_window_hscroll(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let w = win_live(i, &arg(&a, 0))?;
    Ok(Value::Int(w.borrow().hscroll as i128))
}
fn f_set_window_hscroll(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU Fset_window_hscroll: CHECK_FIXNUM (ncol) comes FIRST, then
    // decode_live_window.  Sets hscroll and suspends auto-hscroll
    // until point moves; returns the new hscroll.
    let ncol = crate::lisp::builtins::want_fixnum(i, &a[1])?.max(0) as usize;
    let w = win_live(i, &arg(&a, 0))?;
    {
        let mut wb = w.borrow_mut();
        wb.hscroll = ncol;
        wb.suspend_hscroll = true;
    }
    Ok(Value::Int(ncol as i128))
}
fn f_window_edges(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU `Fwindow_edges' (window.c):
    // decode_any_window on WINDOW (nil → selected, dead windows ok);
    // BODY non-nil requires a LIVE window and excludes the mode line.
    // ABSOLUTE reads the frame's display edges — nil on a batch tty, so
    // the arithmetic lands on `number-or-marker-p nil'.  PIXELWISE is a
    // no-op under canonical char geometry.
    let body = a.get(1).map(|v| v.truthy()).unwrap_or(false);
    let w = if body {
        win_live(i, &arg(&a, 0))?
    } else {
        win_err_valid(i, &arg(&a, 0))?
    };
    // GNU BODY=nontrivial (i.e. `window-inside-edges' / `window-body-edges')
    // subtracts the batch menu-bar line too — the text area lives in the
    // pre-split row space where the menu bar was materialized on top.
    let shift = if body {
        frame_of_window(i, &w)
            .map(|f| fmenu_shift(&f.borrow()))
            .unwrap_or(0)
    } else {
        0
    };
    let (header, tab, mode_line) = if body {
        (
            winxtra::window_format_line_height(i, &w, "header-line-format")?,
            winxtra::window_format_line_height(i, &w, "tab-line-format")?,
            winxtra::window_mode_line_height(i, &w)?,
        )
    } else {
        (0, 0, 0)
    };
    let ww = w.borrow();
    let left = if body {
        ww.left.saturating_add(ww.margins.0)
    } else {
        ww.left
    };
    let right = if body {
        ww.left
            .saturating_add(ww.width)
            .saturating_sub(ww.margins.1)
    } else {
        ww.left.saturating_add(ww.width)
    };
    if a.get(2).map(|v| v.truthy()).unwrap_or(false) {
        drop(ww);
        return Err(i.wrong_type_mut("number-or-marker-p", &Value::Nil));
    }
    Ok(Value::list(vec![
        Value::Int(left as i128),
        Value::Int(
            ww.top
                .saturating_add(header)
                .saturating_add(tab)
                .saturating_sub(shift) as i128,
        ),
        Value::Int(right as i128),
        Value::Int((ww.top + ww.height).saturating_sub(shift + mode_line) as i128),
    ]))
}
fn f_window_at(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU Fwindow_at decodes the optional FRAME first (nil → selected
    // frame), then CHECK_NUMBERs X and Y (`integerp', markers don't
    // coerce).
    let frame = match a.get(2) {
        None | Some(Value::Nil) => sel_frame(i),
        Some(v) => Some(frame_live(i, v)?),
    };
    let x = crate::lisp::builtins::want_int(i, &a[0])?;
    let y = crate::lisp::builtins::want_int(i, &a[1])?;
    let frames: Vec<FrameRef> = match &frame {
        Some(f) => vec![f.clone()],
        None => i.frames.clone(),
    };
    for f in &frames {
        let fb = f.borrow();
        let shift = fmenu_shift(&fb);
        for w in &fb.windows {
            let ww = w.borrow();
            // GNU's coordinate lookup runs on the inside (pre-shift)
            // geometry: a window's rectangle covers its text area
            // plus its own mode line, which lands on the next
            // window's shifted top row.
            let top = ww.top.saturating_sub(shift);
            let bot = (ww.top + ww.height).saturating_sub(shift);
            if x >= ww.left as i128
                && x < (ww.left + ww.width) as i128
                && y >= top as i128
                && y < bot as i128
            {
                return Ok(Value::Window(w.clone()));
            }
        }
    }
    Ok(Value::Nil)
}

fn f_recenter(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let w = sel_window(i).unwrap();
    let n = arg(&a, 0).int();
    let (buf_id, height) = {
        let ww = w.borrow();
        (ww.buffer, ww.height)
    };
    // GNU signals (error "'recenter'ing a window that does not
    // display current-buffer") when the selected window shows a
    // different buffer — e.g. `with-temp-buffer' in batch.
    if buf_id != i.current_buffer {
        return Err(i.error_q::<str>(
            "`recenter'ing a window that does not display current-buffer", &[]));
    }
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

/// Numeric scroll count from a `^P'/`P'-style raw prefix value: nil →
/// the caller's page default, integers → themselves, `(4)'/`-'/etc.
/// via `prefix-numeric-value' rules.  Interactive `P' specs deliver
/// raw prefixes like `(4)', so conses must be accepted here.
pub(crate) fn scroll_arg(i: &mut Interp, v: &Value, default: i128) -> Result<i128, Flow> {
    match v {
        Value::Nil => Ok(default),
        Value::Int(n) => Ok(*n),
        Value::Cons(c) => match c.borrow().car.clone() {
            Value::Int(n) => Ok(n),
            _ => Ok(1),
        },
        Value::Sym(s) if i.symbol_name(*s) == "-" => Ok(-1),
        // GNU Fscroll_up converts ARG through `prefix-numeric-value':
        // every non-integer, non-cons value (symbols, floats, strings,
        // markers) means 1 — no type check at all.
        _ => Ok(1),
    }
}

/// GNU `window_scroll': move WINDOW's `start' N lines (N > 0 → text
/// scrolls up / start advances; N < 0 → scrolls down).  When the
/// scroll would leave point outside the window, point moves like
/// GNU's `window_scroll_line_based': a forward scroll puts it at the
/// start of the first visible line, a backward scroll at the start
/// of the last visible line — so a second scroll shows new text.
/// The selected window's point is the buffer's point; any other
/// window's is `window-point'.  Without this the redisplay layer's
/// point-visible snap (`render_grid') cancels every scroll whose
/// point escapes the window.  Returns false when the scroll hit the
/// buffer boundary before N lines (partial scroll, like GNU).
fn window_scroll(i: &mut Interp, w: &WindowRef, n: i128) -> bool {
    let (buf, selected, start, height) = {
        let sel = i
            .selected_frame
            .as_ref()
            .map(|f| std::rc::Rc::ptr_eq(w, &f.borrow().selected))
            .unwrap_or(false);
        let wb = w.borrow();
        (wb.buffer, sel, wb.start, wb.height)
    };
    let Some(b) = i.buffers.get(buf) else {
        return true;
    };
    // Text rows = window height minus mode line AND the row the
    // frame's mini-window occupies (see render_grid's wh).
    let text_rows = height.saturating_sub(2).max(1);
    let (new_start, new_point, hit_edge) = {
        let bb = b.borrow();
        let len = bb.text.len();
        let point = if selected {
            bb.point()
        } else {
            w.borrow().point
        };
        let mut new_start = start;
        let mut hit_edge = false;
        if n > 0 {
            let mut p = start;
            for _ in 0..n {
                let e = bb.text.line_end(p);
                if e >= len {
                    hit_edge = true;
                    break;
                }
                p = e + 1;
            }
            new_start = p;
        } else if n < 0 {
            let line = bb.text.line_of_pos(start);
            // unsigned_abs: -(i128::MIN) would overflow.
            let steps = n.unsigned_abs() as usize;
            if line < steps {
                hit_edge = true;
            }
            new_start = bb.text.line_start(line.saturating_sub(steps));
        }
        // Start of the last visible line under new_start.
        let mut last_vis = new_start;
        {
            let mut q = new_start;
            for _ in 0..text_rows.saturating_sub(1) {
                let e = bb.text.line_end(q);
                if e >= len {
                    break;
                }
                q = e + 1;
                last_vis = q;
            }
        }
        let new_point = if n > 0 && point < new_start {
            Some(new_start)
        } else if n < 0 && point > bb.text.line_end(last_vis) {
            Some(last_vis)
        } else {
            None
        };
        (new_start, new_point, hit_edge)
    };
    w.borrow_mut().start = new_start;
    if let Some(p) = new_point {
        if selected {
            b.borrow_mut().set_point(p);
        } else {
            w.borrow_mut().point = p;
        }
    }
    !hit_edge
}

/// Signal `end-of-buffer'/`beginning-of-buffer' for a partial scroll
/// when `scroll-error-top-bottom' is nil (GNU Fscroll_up semantics).
fn scroll_edge_signal(i: &mut Interp, up: bool) -> EvalResult {
    let setb = i.q("scroll-error-top-bottom");
    let v = i.symbol_value(setb);
    if v.is_nil() || matches!(v, Value::Sym(s) if s == crate::lisp::sym::UNBOUND) {
        let sym = i.intern(if up {
            "end-of-buffer"
        } else {
            "beginning-of-buffer"
        });
        return Err(i.signal_data(sym, Vec::new()));
    }
    Ok(Value::Nil)
}

/// Default scroll amount: the window's text height minus
/// `next-screen-context-lines' (GNU `window_scroll' page default).
fn scroll_default_lines(i: &mut Interp, w: &WindowRef) -> i128 {
    let ctx_id = i.q("next-screen-context-lines");
    let ctx = i.symbol_value(ctx_id).int().unwrap_or(2);
    (w.borrow().height.saturating_sub(2) as i128 - ctx).max(1)
}

fn f_scroll_up(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let w = sel_window(i).unwrap();
    let default = scroll_default_lines(i, &w);
    let n = scroll_arg(i, &arg(&a, 0), default)?;
    if n < 0 {
        return f_scroll_down(i, crate::lisp::Argv::new(vec![Value::Int(-n)]));
    }
    // GNU window_scroll treats n==0 like a forward scroll: it
    // "succeeds" scrolling zero lines but still signals
    // end-of-buffer.
    if n == 0 || !window_scroll(i, &w, n) {
        scroll_edge_signal(i, true)?;
    }
    Ok(Value::Nil)
}

fn f_scroll_down(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let w = sel_window(i).unwrap();
    let default = scroll_default_lines(i, &w);
    let n = scroll_arg(i, &arg(&a, 0), default)?;
    if n < 0 {
        return f_scroll_up(i, crate::lisp::Argv::new(vec![Value::Int(-n)]));
    }
    // GNU: scroll-down delegates to window_scroll with -n; a zero
    // count is a *forward* zero-line scroll → end-of-buffer (the
    // subr always signals when nothing moved).
    if n == 0 {
        scroll_edge_signal(i, true)?;
    } else if !window_scroll(i, &w, -n) {
        scroll_edge_signal(i, false)?;
    }
    Ok(Value::Nil)
}

fn f_scroll_up_line(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
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
    f_scroll_up(i, crate::lisp::Argv::new(vec![Value::Int(n)]))
}
fn f_scroll_down_line(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
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
    f_scroll_down(i, crate::lisp::Argv::new(vec![Value::Int(n)]))
}
fn f_window_left(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU window.el excludes nil and parentless windows, unlike the
    // general previous-sibling primitive's root/minibuffer chain.
    if a[0].is_nil() || win_valid(i, &a[0])?.borrow().parent.is_none() {
        return Ok(Value::Nil);
    }
    crate::editor::winxtra::siblings(i, &arg(&a, 0), false)
}
fn f_window_right(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    if a[0].is_nil() || win_valid(i, &a[0])?.borrow().parent.is_none() {
        return Ok(Value::Nil);
    }
    crate::editor::winxtra::siblings(i, &arg(&a, 0), true)
}
fn f_scroll_up_command(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    f_scroll_up(i, a)
}
fn f_scroll_down_command(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    f_scroll_down(i, a)
}
/// GNU `other_window_for_scrolling' (window.c): the window
/// `scroll-other-window*' commands act on.  In order: the
/// minibuffer's `minibuffer-scroll-window'; a live
/// `other-window-scroll-buffer' (a window showing it, else
/// `display-buffer' pops one up); `other-window-scroll-default'
/// called with no args; finally `next-window' on the selected
/// frame and, when that wraps to the selected window, on every
/// frame.  A dead/non-window result signals `window-live-p';
/// landing back on the selected window signals
/// (error "There is no other window") — BEFORE ARG is converted.
fn other_scroll_neighbor(i: &mut Interp, sel: &WindowRef) -> Result<WindowRef, Flow> {
    // Fnext_window(selected, 'lambda, 'visible): cycle the live
    // leaf windows of all frames (remacs keeps each frame's
    // minibuffer window out of `windows').
    let flat: Vec<WindowRef> = i
        .frames
        .iter()
        .flat_map(|f| {
            f.borrow()
                .windows
                .iter()
                .filter(|w| !w.borrow().dead && w.borrow().horiz.is_none())
                .cloned()
                .collect::<Vec<_>>()
        })
        .collect();
    let sel_id = sel.borrow().id;
    match flat.iter().position(|w| w.borrow().id == sel_id) {
        Some(ix) => Ok(flat[(ix + 1) % flat.len()].clone()),
        None => Ok(sel.clone()),
    }
}
fn other_window_for_scrolling(i: &mut Interp) -> Result<WindowRef, Flow> {
    let sel = sel_window(i).ok_or_else(|| i.error("No window"))?;
    let sel_id = sel.borrow().id;
    let mut found: Option<Value> = None;
    if sel.borrow().minibuffer {
        let msw = i.q("minibuffer-scroll-window");
        let v = i.symbol_value(msw);
        if !v.is_nil() {
            found = Some(v);
        }
    }
    if found.is_none() {
        let owsb = i.q("other-window-scroll-buffer");
        if let Value::Buffer(b) = i.symbol_value(owsb) {
            let bid = b.borrow().id;
            if i.buffer_live(bid) {
                // get-buffer-window among live leaves on all frames,
                // else display_buffer — batch/tty splits a window to
                // show the buffer.
                let mut w: Option<WindowRef> = None;
                'outer: for f in &i.frames {
                    for w2 in &f.borrow().windows {
                        if !w2.borrow().dead
                            && w2.borrow().horiz.is_none()
                            && w2.borrow().buffer == bid
                        {
                            w = Some(w2.clone());
                            break 'outer;
                        }
                    }
                }
                let w = match w {
                    Some(w) => w,
                    None => {
                        let v = f_split_window(i, vec![Value::Nil].into())?;
                        match v {
                            Value::Window(w) => {
                                let bval = i.buffer_value(bid).unwrap_or(Value::Nil);
                                f_set_window_buffer(
                                    i,
                                    vec![Value::Window(w.clone()), bval].into(),
                                )?;
                                w
                            }
                            _ => sel.clone(),
                        }
                    }
                };
                found = Some(Value::Window(w));
            }
        }
    }
    if found.is_none() {
        let owsd = i.q("other-window-scroll-default");
        let f = i.symbol_value(owsd);
        // FUNCTIONP: a symbol with a live function binding or a
        // non-symbol function object.
        let callable = match &f {
            Value::Sym(s) => i.fbound_p(*s),
            Value::Subr(_) | Value::Lambda(_) => true,
            _ => false,
        };
        if callable {
            found = Some(i.apply(&f, vec![])?);
        }
    }
    let v = match found {
        Some(v) => v,
        None => Value::Window(other_scroll_neighbor(i, &sel)?),
    };
    // CHECK_LIVE_WINDOW, then the self-check.
    let w = win_live(i, &v)?;
    if w.borrow().id == sel_id {
        return Err(i.error("There is no other window"));
    }
    Ok(w)
}

/// GNU `scroll_command'/'window_scroll' edge contract (as used by
/// the scroll subrs): nil ARG or `-` scroll by a screenful
/// (`whole'), a numeric ARG by that many lines in DIRECTION; a
/// scroll that cannot complete signals `beginning-of-buffer' /
/// `end-of-buffer' unconditionally.
fn other_scroll_raw(i: &mut Interp, w: &WindowRef, arg: &Value, dir: i128) -> EvalResult {
    let n = if arg.is_nil() {
        dir
    } else if matches!(arg, Value::Sym(s) if i.symbol_name(*s) == "-") {
        -dir
    } else {
        let p = f_prefix_numeric_value(i, vec![arg.clone()].into())?;
        p.int().unwrap_or(1) * dir
    };
    let whole = arg.is_nil() || matches!(arg, Value::Sym(s) if i.symbol_name(*s) == "-");
    let lines = if whole {
        n * scroll_default_lines(i, w)
    } else {
        n
    };
    if !window_scroll(i, w, lines) && lines != 0 {
        let sym = i.intern(if lines > 0 {
            "end-of-buffer"
        } else {
            "beginning-of-buffer"
        });
        return Err(i.signal_data(sym, Vec::new()));
    }
    Ok(Value::Nil)
}

/// GNU window.el `scroll-up-command' / `scroll-down-command'
/// semantics run on an already-selected window: with
/// `scroll-error-top-bottom' nil it is the plain subr; non-nil
/// catches the edge signal (unless point is already at that edge)
/// and instead moves point — ARG lines by `forward-line', or to
/// `point-min'/`point-max' when ARG is nil.
fn scroll_command_cmd(i: &mut Interp, dir: i128, arg: &Value) -> EvalResult {
    let setb = i.q("scroll-error-top-bottom");
    let v = i.symbol_value(setb);
    if v.is_nil() || matches!(v, Value::Sym(s) if s == crate::lisp::sym::UNBOUND) {
        return other_scroll_raw(i, &sel_window(i).unwrap(), arg, dir);
    }
    if matches!(arg, Value::Sym(s) if i.symbol_name(*s) == "-") {
        return scroll_command_cmd(i, -dir, &Value::Nil);
    }
    let pv = f_prefix_numeric_value(i, vec![arg.clone()].into())?;
    if pv.int().unwrap_or(1) < 0 {
        return scroll_command_cmd(i, -dir, &pv);
    }
    let at_edge = {
        let b = cur(i);
        let bb = b.borrow();
        let len = bb.zv.max(bb.begv).min(bb.text_len());
        if dir > 0 {
            bb.point() >= len
        } else {
            bb.point() <= bb.begv
        }
    };
    if at_edge {
        // Point already at the edge: the scroll errors out.
        return other_scroll_raw(i, &sel_window(i).unwrap(), arg, dir);
    }
    let w = sel_window(i).unwrap();
    match other_scroll_raw(i, &w, arg, dir) {
        Ok(v) => Ok(v),
        Err(_) => {
            // The edge signal was caught: move point instead.
            if arg.is_nil() {
                let b = cur(i);
                let p = {
                    let bb = b.borrow();
                    let len = bb.zv.max(bb.begv).min(bb.text_len());
                    if dir > 0 { len } else { bb.begv }
                };
                b.borrow_mut().set_point(p);
            } else {
                // `(forward-line ARG)' for scroll-up-command,
                // `(forward-line (- ARG))' for -down — a non-number
                // ARG signals `number-or-marker-p' via `-'.
                let negated = match want_num_or_mark(i, arg) {
                    Ok(n) => -n,
                    Err(e) => return Err(e),
                };
                let delta = if dir > 0 { -negated } else { negated };
                let fl = i.q("forward-line");
                i.apply(&Value::Sym(fl), vec![Value::Int(delta)])?;
            }
            Ok(Value::Nil)
        }
    }
}

/// GNU `scroll-other-window' / `scroll-other-window-down'
/// (window.el): `(with-selected-window (other-window-for-scrolling)
///   (funcall (or (command-remapping #'scroll-COMMAND)
///                #'scroll-COMMAND) LINES))'.
/// The selection excursion restores both the selected window and
/// the current buffer (GNU's unwind also restores point in the
/// original buffer through save-current-buffer).
fn scroll_other_command(i: &mut Interp, arg: &Value, cmd_name: &str) -> EvalResult {
    let other = other_window_for_scrolling(i)?;
    let sel = sel_window(i).unwrap();
    let old_buf = i.current_buffer;
    let mut cmd = Value::Sym(i.intern(cmd_name));
    let cr = i.q("command-remapping");
    if let Ok(r) = i.apply(&Value::Sym(cr), vec![cmd.clone()]) {
        if !r.is_nil() {
            cmd = r;
        }
    }
    f_select_window(i, vec![Value::Window(other.clone()), Value::t()].into())?;
    let cmd_id = i.q("scroll-up-command");
    let cmd2 = i.q("scroll-down-command");
    let run = match &cmd {
        Value::Sym(s) if *s == cmd_id => scroll_command_cmd(i, 1, arg),
        Value::Sym(s) if *s == cmd2 => scroll_command_cmd(i, -1, arg),
        f => i.apply(f, vec![arg.clone()]),
    };
    let _ = f_select_window(i, vec![Value::Window(sel), Value::t()].into());
    i.set_current_buffer(old_buf);
    run
}

fn f_scroll_other_window(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    scroll_other_command(i, &arg(&a, 0), "scroll-up-command")
}

fn f_pos_visible_in_window_p(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU Fpos_visible_in_window_p decodes
    // the window via CHECK_LIVE_WINDOW BEFORE POS — `(pos-… "a" "a")'
    // signals `window-live-p', not `integer-or-marker-p'.  Then POS:
    // nil/omitted → point; non-integer-or-marker → `integer-or-marker-p'
    // wta.  In batch nothing is ever visible — but only after the args
    // are validated (GNU computes visibility from the glyph matrix; the
    // type checks still run first).
    let w = win_live(i, &arg(&a, 1))?;
    let pos = match arg(&a, 0) {
        Value::Nil => cur(i).borrow().point() as i128 + 1,
        // GNU fix_position: a marker yields marker_position (detached →
        // "Marker does not point anywhere"); other non-ints are
        // integer-or-marker-p.
        Value::Marker(m) => {
            let mm = m.borrow();
            if mm.buffer.is_none() {
                return Err(i.error("Marker does not point anywhere"));
            }
            mm.position as i128 + 1
        }
        v => match v.int() {
            Some(n) => n,
            None => return Err(i.wrong_type_mut("integer-or-marker-p", &v)),
        },
    };
    if i.noninteractive {
        return Ok(Value::Nil);
    }
    let start = w.borrow().start as i128 + 1;
    Ok(Value::from_bool(pos >= start))
}

fn f_window_dedicated_p(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let w = win_live(i, &arg(&a, 0))?;
    Ok(w.borrow().dedicated.clone())
}
fn f_set_window_dedicated_p(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let w = win_live(i, &arg(&a, 0))?;
    w.borrow_mut().dedicated = a[1].clone();
    Ok(a[1].clone())
}
fn f_window_parameter(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let w = win_any(i, &arg(&a, 0))?;
    Ok(crate::lisp::eval::plist_get_v(&w.borrow().params, &a[1]))
}
fn f_set_window_parameter(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let w = win_any(i, &arg(&a, 0))?;
    let new = crate::lisp::eval::plist_put_front(&w.borrow().params, &a[1], a[2].clone());
    w.borrow_mut().params = new;
    Ok(a[2].clone())
}
fn f_window_parameters(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let w = win_valid(i, &arg(&a, 0))?;
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
fn f_window_margins(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let w = match &arg(&a, 0) {
        Value::Nil => sel_window(i).ok_or_else(|| i.error("No window"))?,
        Value::Window(w) if !w.borrow().dead => w.clone(),
        other => return Err(i.wrong_type_mut("window-live-p", other)),
    };
    let (l, r) = w.borrow().margins;
    // GNU always returns (LEFT . RIGHT), nil sides for zero margins.
    let lv = if l == 0 {
        Value::Nil
    } else {
        Value::Int(l as i128)
    };
    let rv = if r == 0 {
        Value::Nil
    } else {
        Value::Int(r as i128)
    };
    Ok(Value::cons(lv, rv))
}
// ---------- frames ----------

/// GNU `(display-color-p &optional DISPLAY)' / `-grayscale-p' /
/// `-mouse-p' / `-selections-p': the optional arg goes through
/// `get-device-terminal' first (a bogus name/object signals); on a
/// tty batch frame the predicates then all return nil.
fn f_display_pred_nil(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let v = a.get(0).cloned().unwrap_or(Value::Nil);
    let _ = crate::lisp::builtins::misc::decode_display(i, &v)?;
    Ok(Value::Nil)
}

fn frame_parameter_value(i: &Interp, frame: &crate::editor::FrameRef, name: &str) -> Value {
    let Some(parameter) = i.intern_soft(name) else {
        return Value::Nil;
    };
    let params = frame.borrow().params.clone();
    crate::lisp::eval::plist_get_v(&params, &Value::Sym(parameter))
}

pub(crate) fn frame_parameter_positive_size(
    i: &Interp,
    frame: &crate::editor::FrameRef,
    name: &str,
    fallback: usize,
) -> usize {
    frame_parameter_value(i, frame, name)
        .int()
        .and_then(|value| usize::try_from(value).ok())
        .filter(|value| *value > 0)
        .unwrap_or(fallback)
}

pub(crate) fn frame_char_width(i: &Interp, frame: &crate::editor::FrameRef) -> usize {
    let fallback = if frame_is_graphical(i, frame) { 8 } else { 1 };
    frame_parameter_positive_size(i, frame, "char-width", fallback)
}

pub(crate) fn frame_char_height(i: &Interp, frame: &crate::editor::FrameRef) -> usize {
    let fallback = if frame_is_graphical(i, frame) { 19 } else { 1 };
    frame_parameter_positive_size(i, frame, "char-height", fallback)
}

pub(crate) fn frame_internal_border_width(i: &Interp, frame: &crate::editor::FrameRef) -> usize {
    if !frame_is_graphical(i, frame) {
        return 0;
    }
    frame_parameter_value(i, frame, "internal-border-width")
        .int()
        .and_then(|width| usize::try_from(width).ok())
        .unwrap_or(0)
}

pub(crate) fn frame_pixel_width(i: &Interp, frame: &crate::editor::FrameRef) -> usize {
    let border = frame_internal_border_width(i, frame).saturating_mul(2);
    let fallback = frame
        .borrow()
        .width
        .saturating_mul(frame_char_width(i, frame))
        .saturating_add(border);
    frame_parameter_positive_size(i, frame, "pixel-width", fallback)
}

pub(crate) fn frame_pixel_height(i: &Interp, frame: &crate::editor::FrameRef) -> usize {
    let border = frame_internal_border_width(i, frame).saturating_mul(2);
    let fallback = frame
        .borrow()
        .height
        .saturating_mul(frame_char_height(i, frame))
        .saturating_add(border);
    frame_parameter_positive_size(i, frame, "pixel-height", fallback)
}

pub(crate) fn frame_pixel_left(i: &Interp, frame: &crate::editor::FrameRef) -> i128 {
    frame_parameter_value(i, frame, "left").int().unwrap_or(0)
}

pub(crate) fn frame_pixel_top(i: &Interp, frame: &crate::editor::FrameRef) -> i128 {
    frame_parameter_value(i, frame, "top").int().unwrap_or(0)
}

pub(crate) fn frame_system_type(i: &Interp, frame: &crate::editor::FrameRef) -> Value {
    let window_system = frame_parameter_value(i, frame, "window-system");
    if window_system.is_nil() {
        Value::t()
    } else {
        window_system
    }
}

pub(crate) fn frame_is_graphical(i: &Interp, frame: &crate::editor::FrameRef) -> bool {
    let system = frame_parameter_value(i, frame, "window-system");
    match system {
        Value::Sym(system) => matches!(
            i.symbol_name(system).as_str(),
            "x" | "w32" | "ns" | "pgtk" | "haiku" | "android" | "gpui"
        ),
        _ => false,
    }
}

fn f_display_color_p(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let display = a.get(0).cloned().unwrap_or(Value::Nil);
    let frame = crate::lisp::builtins::misc::decode_display(i, &display)?;
    Ok(Value::from_bool(frame_is_graphical(i, &frame)))
}

fn f_selected_frame(i: &mut Interp, _a: crate::lisp::Argv) -> EvalResult {
    match &i.selected_frame {
        Some(f) => Ok(Value::Frame(f.clone())),
        None => Ok(Value::Nil),
    }
}
fn f_framep(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    match &a[0] {
        Value::Frame(frame) => Ok(frame_system_type(i, frame)),
        _ => Ok(Value::Nil),
    }
}
fn f_frame_live_p(_i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    match &a[0] {
        Value::Frame(f) => Ok(Value::from_bool(!f.borrow().dead)),
        _ => Ok(Value::Nil),
    }
}

pub(crate) fn set_frame_visibility(
    i: &mut Interp,
    frame: &FrameRef,
    visible: bool,
) -> Result<(), Flow> {
    let visibility_key = Value::Sym(i.intern("visibility"));
    let known_key = Value::Sym(i.intern("remacs--visibility-known"));
    let value = if visible {
        Value::t()
    } else {
        Value::Sym(i.intern("icon"))
    };
    let params = crate::lisp::eval::plist_put_front(
        &frame.borrow().params,
        &visibility_key,
        value,
    );
    frame.borrow_mut().params =
        crate::lisp::eval::plist_put_front(&params, &known_key, Value::t());
    if frame_is_graphical(i, frame)
        && let Some(backend) = i.frame_window_backend.clone()
    {
        backend
            .set_visible(frame.borrow().id, visible)
            .map_err(|error| i.error(format!("Frame visibility failed: {error}")))?;
    }
    Ok(())
}

fn f_frame_visible_p(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    match &a[0] {
        Value::Frame(f) if !f.borrow().dead => {
            if frame_parameter_value(i, f, "remacs--visibility-known").truthy() {
                Ok(frame_parameter_value(i, f, "visibility"))
            } else {
                Ok(Value::t())
            }
        }
        other => Err(i.wrong_type_mut("frame-live-p", other)),
    }
}

/// `internal-show-cursor' — GNU Finternal_show_cursor CHECK_WINDOWs
/// the first arg; headless: no visible effect, returns nil.
fn f_internal_show_cursor(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU decode_any_window: nil means the SELECTED window.
    match &a[0] {
        Value::Nil | Value::Window(_) => Ok(Value::Nil),
        other => Err(i.wrong_type_mut("windowp", other)),
    }
}

fn f_show_cursor_p(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU: optional WINDOW arg (nil = selected); ours always shows.
    match a.first() {
        None | Some(Value::Nil) | Some(Value::Window(_)) => Ok(Value::t()),
        Some(other) => Err(i.wrong_type_mut("windowp", other)),
    }
}

fn f_frame_focus_state(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU: terminal arguments return nil; graphical frames report the
    // last focus event recorded by `handle-focus-in/out'.
    let frame = match a.first() {
        None | Some(Value::Nil) => sel_frame(i),
        Some(Value::Frame(frame)) if !frame.borrow().dead => Some(frame.clone()),
        Some(value) if is_terminal(i, value) => return Ok(Value::Nil),
        Some(other) => return Err(i.wrong_type_mut("terminal-live-p", other)),
    };
    let Some(frame) = frame else {
        return Ok(Value::Nil);
    };
    if crate::editor::frame_is_graphical(i, &frame) {
        Ok(frame_parameter_value(i, &frame, "last-focus-update"))
    } else {
        Ok(Value::Nil)
    }
}
fn f_frame_list(i: &mut Interp, _a: crate::lisp::Argv) -> EvalResult {
    Ok(Value::list(
        i.frames
            .iter()
            .filter(|f| !f.borrow().dead)
            .map(|f| Value::Frame(f.clone()))
            .collect(),
    ))
}
fn f_delete_frame(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let f = frame_any(i, &arg(&a, 0))?;
    if f.borrow().dead {
        return Ok(Value::Nil);
    }
    if i.frames.iter().filter(|x| !x.borrow().dead).count() <= 1 {
        let message = if arg(&a, 1).truthy() {
            "Attempt to delete the only frame"
        } else {
            "Attempt to delete the sole visible or iconified frame"
        };
        return Err(i.error(message));
    }
    let run_hook_with_args = Value::Sym(i.intern("run-hook-with-args"));
    let delete_frame_functions = Value::Sym(i.intern("delete-frame-functions"));
    i.apply(
        &run_hook_with_args,
        vec![delete_frame_functions, Value::Frame(f.clone())],
    )?;
    if sel_frame(i).is_some_and(|selected| Rc::ptr_eq(&selected, &f)) {
        let replacement = i
            .frames
            .iter()
            .filter(|other| !Rc::ptr_eq(other, &f) && !other.borrow().dead)
            .max_by_key(|other| other.borrow().selected.borrow().use_time)
            .cloned()
            .unwrap();
        let window = replacement.borrow().selected.clone();
        f_select_window(i, vec![Value::Window(window)].into())?;
    }
    {
        let mut f = f.borrow_mut();
        if let Some(root) = &f.root {
            mark_tree_dead(i, root);
        }
        for w in f.windows.iter().chain(f.minibuffer.iter()) {
            w.borrow_mut().dead = true;
        }
        f.windows.clear();
        f.root = None;
        f.minibuffer = None;
        f.old_selected = None;
        f.buffer_list.clear();
        f.buried_buffer_list.clear();
        f.dead = true;
    }
    i.frames.retain(|other| !Rc::ptr_eq(other, &f));
    let after_delete_frame_functions = Value::Sym(i.intern("after-delete-frame-functions"));
    i.apply(
        &run_hook_with_args,
        vec![after_delete_frame_functions, Value::Frame(f)],
    )?;
    Ok(Value::Nil)
}

fn f_frame_parameter(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let f = frame_of(i, &arg(&a, 0))?;
    let ps = want_sym(i, &a[1])?;
    let name = i.symbol_name(ps);
    if f.borrow().dead {
        return Ok(Value::Nil);
    }
    if name == "name" {
        return Ok(Value::Str(f.borrow().name.clone()));
    }
    if name == "font" {
        let stored = crate::lisp::eval::plist_get_v(&f.borrow().params, &i.sym(ps));
        if !stored.is_nil() {
            return Ok(stored);
        }
        return Ok(Value::string("tty"));
    }
    if name == "buffer-list" || name == "buried-buffer-list" {
        let f = f.borrow();
        let ids = if name == "buffer-list" {
            &f.buffer_list
        } else {
            &f.buried_buffer_list
        };
        return Ok(Value::list(
            ids.iter().filter_map(|id| i.buffer_value(*id)).collect(),
        ));
    }
    let v = crate::lisp::eval::plist_get_v(&f.borrow().params, &i.sym(ps));
    if !v.is_nil() {
        return Ok(v);
    }
    let name = i.symbol_name(ps);
    let ff = f.borrow();
    Ok(match name.as_str() {
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
        // Values mirror the `f_frame_parameters' defaults list.
        "tab-bar-lines" => Value::Int(0),
        "menu-bar-lines" => Value::Int(1),
        "background-color" => Value::string("unspecified-bg"),
        "foreground-color" => Value::string("unspecified-fg"),
        "cursor-color" => Value::string("white"),
        "background-mode" => Value::Sym(i.q("dark")),
        "display-type" => Value::Sym(i.q("mono")),
        "unsplittable" | "no-accept-focus" => Value::Nil,
        _ => Value::Nil,
    })
}
pub(crate) fn f_frame_parameters(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let f = frame_of(i, &arg(&a, 0))?;
    if f.borrow().dead {
        return Ok(Value::Nil);
    }
    let (stored, name, w, h, mbuf, buffers, buried) = {
        let ff = f.borrow();
        let items = ff.params.list_to_vec().unwrap_or_default();
        let mut stored = Vec::new();
        let mut k = 0;
        while k + 1 < items.len() {
            stored.push(Value::cons(items[k].clone(), items[k + 1].clone()));
            k += 2;
        }
        let buffers = Value::list(
            ff.buffer_list
                .iter()
                .filter_map(|id| i.buffer_value(*id))
                .collect(),
        );
        let buried = Value::list(
            ff.buried_buffer_list
                .iter()
                .filter_map(|id| i.buffer_value(*id))
                .collect(),
        );
        (
            stored,
            Value::Str(ff.name.clone()),
            ff.width,
            ff.height,
            ff.minibuffer.is_some(),
            buffers,
            buried,
        )
    };
    // GNU's tty frame parameter list, in order.
    let dark = i.q("dark");
    let mono = i.q("mono");
    let defaults: Vec<(&str, Value)> = vec![
        ("no-accept-focus", Value::Nil),
        ("visibility", Value::t()),
        ("tab-bar-lines", Value::Int(0)),
        ("menu-bar-lines", Value::Int(1)),
        ("buried-buffer-list", buried),
        ("buffer-list", buffers),
        ("unsplittable", Value::Nil),
        ("modeline", Value::t()),
        ("width", Value::Int(w as i128)),
        ("height", Value::Int(h as i128)),
        ("name", name),
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
            _ if k == "name" || k == "font" => None,
            Value::Cons(c) if matches!(c.borrow().car, Value::Sym(s) if s == kid) => {
                Some(c.borrow().cdr.clone())
            }
            _ => None,
        });
        out.push(Value::cons(Value::Sym(kid), from_store.unwrap_or(dv)));
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
fn next_frame_name(i: &mut Interp) -> String {
    loop {
        let name = format!("F{}", i.next_frame_name);
        i.next_frame_name += 1;
        if !i
            .frames
            .iter()
            .any(|f| !f.borrow().dead && *f.borrow().name.borrow() == name)
        {
            return name;
        }
    }
}

fn automatic_frame_name(name: &EmacsStr) -> bool {
    name.starts_with('F') && name.len() > 1 && name[1..].chars().all(|b| b.is_ascii_digit())
}

fn store_frame_parameter(
    i: &mut Interp,
    f: &FrameRef,
    key: Value,
    mut value: Value,
) -> Result<(), Flow> {
    let ps = i.sym_id(&key);
    let name = ps.map(|ps| i.symbol_name(ps));
    let key = ps.map(|ps| i.sym(ps)).unwrap_or(key);
    if name.as_deref() == Some("minibuffer") {
        let mut old = crate::lisp::eval::plist_get_v(&f.borrow().params, &key);
        if old.is_nil() && f.borrow().minibuffer.is_some() {
            old = Value::t();
        }
        if let Value::Window(window) = &value {
            if window.borrow().dead || !window.borrow().minibuffer {
                return Err(i.error_q::<str>(
                    "The `minibuffer' parameter does not specify a valid minibuffer window",
                    &[]));
            }
            let only = i.q("only");
            if i.sym_is(&old, sym::T) || i.sym_is(&old, only) {
                if !f
                    .borrow()
                    .minibuffer
                    .as_ref()
                    .is_some_and(|w| Rc::ptr_eq(w, window))
                {
                    let message = if i.sym_is(&old, only) {
                        "Can't change the minibuffer window of a minibuffer-only frame"
                    } else {
                        "Can't change the minibuffer window of a frame with its own minibuffer"
                    };
                    return Err(i.error_q::<str>(message, &[]));
                }
                value = old;
            } else {
                f.borrow_mut().minibuffer = Some(window.clone());
            }
        } else if !old.is_nil() {
            if matches!(old, Value::Window(_)) && value.is_nil() {
                value = old;
            } else if !eq_values(&old, &value) {
                return Err(i.error_q::<str>("Can't change the `minibuffer' parameter of this frame", &[]));
            }
        }
    }
    if matches!(name.as_deref(), Some("buffer-list" | "buried-buffer-list")) {
        let mut ids = Vec::new();
        let mut tail = value;
        while let Value::Cons(c) = tail {
            let (head, rest) = {
                let c = c.borrow();
                (c.car.clone(), c.cdr.clone())
            };
            if let Value::Buffer(b) = head {
                let b = b.borrow();
                if b.live {
                    ids.push(b.id);
                }
            }
            tail = rest;
        }
        if name.as_deref() == Some("buffer-list") {
            f.borrow_mut().buffer_list = ids;
        } else {
            f.borrow_mut().buried_buffer_list = ids;
        }
    } else {
        let new = crate::lisp::eval::plist_put_front(&f.borrow().params, &key, value.clone());
        f.borrow_mut().params = new;
        match (name.as_deref(), value.int()) {
            (Some("width"), Some(width)) => {
                f.borrow_mut().width = usize::try_from(width.max(1)).unwrap_or(usize::MAX);
            }
            (Some("height"), Some(height)) => {
                f.borrow_mut().height = usize::try_from(height.max(1)).unwrap_or(usize::MAX);
            }
            _ => {}
        }
        if name.as_deref() == Some("name") {
            let current = f.borrow().name.borrow().clone();
            let new_value = if value.is_nil() {
                if automatic_frame_name(&current) {
                    return Ok(());
                }
                Value::string(next_frame_name(i))
            } else {
                let new_name = want_str(i, &value)?;
                if new_name == current {
                    return Ok(());
                }
                if automatic_frame_name(&new_name)
                    && i.frames
                        .iter()
                        .any(|frame| !frame.borrow().dead && *frame.borrow().name.borrow() == new_name)
                {
                    return Err(i.error_q(
                        "Frame of the form F<num> named `%s' already exists",
                        &[&new_name],
                    ));
                }
                value
            };
            let Value::Str(name) = new_value else {
                unreachable!()
            };
            f.borrow_mut().name = name;
        }
    }
    if name.as_deref() == Some("cursor-color") {
        if let (Some(cursor), Some(override_property)) = (
            i.intern_soft("cursor"),
            i.intern_soft("remacs--cursor-face-background-override"),
        ) {
            // A later frame-parameter update takes precedence over the face.
            i.put_prop(cursor, override_property, Value::Nil);
        }
    }
    Ok(())
}

fn f_modify_frame_parameters(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let f = frame_live(i, &arg(&a, 0))?;
    let alist = crate::lisp::builtins::want_list(i, &a[1])?;
    let mut parameters = Vec::with_capacity(alist.len());
    for pair in alist {
        parameters.push(match pair {
            Value::Nil => (Value::Nil, Value::Nil),
            Value::Cons(c) => {
                let b = c.borrow();
                (b.car.clone(), b.cdr.clone())
            }
            other => return Err(i.wrong_type_mut("listp", &other)),
        });
    }
    for (key, value) in parameters.into_iter().rev() {
        store_frame_parameter(i, &f, key, value)?;
    }
    Ok(Value::Nil)
}
fn f_set_frame_parameter(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let f = frame_live(i, &arg(&a, 0))?;
    store_frame_parameter(i, &f, a[1].clone(), a[2].clone())?;
    Ok(Value::Nil)
}

fn f_set_frame_selected_window(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let f = frame_live(i, &arg(&a, 0))?;
    let w = win_live_req(i, &a[1])?;
    if !frame_of_window(i, &w).is_some_and(|wf| Rc::ptr_eq(&wf, &f)) {
        return Err(i.error("In `set-frame-selected-window', WINDOW is not on FRAME"));
    }
    if sel_frame(i).is_some_and(|sf| Rc::ptr_eq(&sf, &f)) {
        return f_select_window(i, vec![a[1].clone(), arg(&a, 2)].into());
    }
    f.borrow_mut().selected = w;
    Ok(a[1].clone())
}

fn f_frame_width(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let f = frame_of(i, &arg(&a, 0))?;
    Ok(Value::Int(f.borrow().width as i128))
}

fn f_frame_pixel_width(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let frame = frame_of(i, &arg(&a, 0))?;
    Ok(Value::Int(frame_pixel_width(i, &frame) as i128))
}

fn f_frame_pixel_height(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let frame = frame_of(i, &arg(&a, 0))?;
    Ok(Value::Int(frame_pixel_height(i, &frame) as i128))
}
fn f_frame_height(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let f = frame_of(i, &arg(&a, 0))?;
    Ok(Value::Int(f.borrow().height as i128))
}

fn f_frame_position(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let frame = frame_live(i, &arg(&a, 0))?;
    if !frame_is_graphical(i, &frame) {
        return Ok(Value::cons(Value::Int(0), Value::Int(0)));
    }
    let params = frame.borrow().params.clone();
    let parameter = |i: &Interp, name: &str| {
        i.q_soft(name)
            .map(|symbol| crate::lisp::eval::plist_get_v(&params, &Value::Sym(symbol)))
            .and_then(|value| value.int())
            .unwrap_or(0)
    };
    Ok(Value::cons(
        Value::Int(parameter(i, "left")),
        Value::Int(parameter(i, "top")),
    ))
}
/// GNU frame.c `frame-edges' et al decode with a plain
/// `(error "X is not a live frame")' — not a `frame-live-p' wta.
pub(crate) fn frame_live_plain(i: &mut Interp, v: &Value) -> Result<FrameRef, Flow> {
    match v {
        Value::Frame(f) if !f.borrow().dead => Ok(f.clone()),
        Value::Nil => sel_frame(i).ok_or_else(|| i.error("No frame")),
        other => Err(i.error(format!("{} is not a live frame", i.princ_to_string(other)))),
    }
}
fn f_frame_edges(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU Fframe_edges (window.c): decode_live_frame-style plain
    // error, then a tty frame has no recorded edges → nil; callers
    // like `frame-outer-width' die on `wta number-or-marker-p nil'.
    let _ = frame_live_plain(i, &arg(&a, 0))?;
    let _ = arg(&a, 1); // TYPE (outer/native/inner) — all nil on tty.
    Ok(Value::Nil)
}

fn f_make_frame(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU Fmake_frame CHECK_LISTs the parameter alist — a dotted
    // tail signals listp too — before terminal-init gets a chance
    // to error.
    let pl = arg(&a, 0);
    let is_list = match &pl {
        Value::Nil => true,
        v => v.list_to_vec().is_ok(),
    };
    if !is_list {
        return Err(i.wrong_type_mut("listp", &pl));
    }
    // GNU batch: `make-frame' bails on terminal-init with
    // "Unknown terminal type" — no usable display exists.
    if i.noninteractive {
        return Err(i.error("Unknown terminal type"));
    }
    // On a tty, a "new frame" is a new full-screen view sharing the tty.
    // A graphical frontend consumes the same frame object and creates its
    // native window from the stored parameters after this call returns.
    let selected = i.selected_frame.clone();
    let buf = i.current_buffer;
    let minibuf = selected
        .as_ref()
        .and_then(|frame| frame.borrow().minibuffer.clone())
        .map(|window| window.borrow().buffer)
        .unwrap_or(buf);
    let default_size = selected
        .as_ref()
        .map(|frame| {
            let frame = frame.borrow();
            (frame.width, frame.height)
        })
        .unwrap_or((80, 25));
    let mut width = default_size.0;
    let mut height = default_size.1;
    let mut parameters = pl.clone();
    if let Ok(entries) = pl.list_to_vec() {
        let mut has_window_system = false;
        for entry in entries {
            let Value::Cons(pair) = entry else {
                continue;
            };
            let (key, value) = {
                let pair = pair.borrow();
                (pair.car.clone(), pair.cdr.clone())
            };
            let name = match key {
                Value::Sym(symbol) => i.symbol_name(symbol),
                _ => continue,
            };
            match name.as_str() {
                "width" => {
                    if let Some(value) = value.int().filter(|value| *value > 0) {
                        width = value as usize;
                    }
                }
                "height" => {
                    if let Some(value) = value.int().filter(|value| *value > 1) {
                        height = value as usize;
                    }
                }
                "window-system" => has_window_system = true,
                _ => {}
            }
        }
        if !has_window_system
            && let Some(frame) = &selected
        {
            let window_system = frame_parameter_value(i, frame, "window-system");
            if !window_system.is_nil() {
                parameters = Value::cons(
                    Value::cons(Value::Sym(i.intern("window-system")), window_system),
                    parameters,
                );
            }
        }
    }
    let f = Frame::new_tty(buf, minibuf, width, height);
    let name = next_frame_name(i);
    f.borrow_mut().name = crate::lisp::Value::string_ref(name);
    if let Err(flow) = f_modify_frame_parameters(
        i,
        vec![Value::Frame(f.clone()), parameters].into(),
    ) {
        f.borrow_mut().dead = true;
        return Err(flow);
    }
    i.frames.push(f.clone());
    f.borrow_mut().width = width;
    f.borrow_mut().height = height;
    reflow_frame_windows(&f, width, height);
    Ok(Value::Frame(f))
}

fn f_select_frame(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    if matches!(arg(&a, 0), Value::Nil) {
        return Err(i.wrong_type_mut("frame-live-p", &a[0]));
    }
    let f = frame_live(i, &arg(&a, 0))?;
    if !sel_frame(i).is_some_and(|selected| Rc::ptr_eq(&selected, &f)) {
        let w = f.borrow().selected.clone();
        f_select_window(i, vec![Value::Window(w), arg(&a, 1)].into())?;
    }
    Ok(a[0].clone())
}

fn f_handle_switch_frame(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    match &a[0] {
        Value::Frame(f) if f.borrow().dead => Ok(Value::Nil),
        Value::Frame(_) => f_select_frame(i, a),
        other => Err(i.wrong_type_mut("framep", other)),
    }
}

/// `tty-color-desc' result's RGB tail ((NAME IDX R G B) -> (R G B)).
fn desc_rgb(i: &mut Interp, color: &Value, frame: &Value) -> EvalResult {
    let f = Value::Sym(i.q("tty-color-desc"));
    let desc = i.apply(&f, vec![color.clone(), frame.clone()])?;
    Ok(match desc {
        Value::Cons(_) => crate::lisp::builtins::listfn::nthcdr_of(&desc, 2),
        _ => Value::Nil,
    })
}

/// `display-graphic-p' — GNU Fdisplay_graphic_p decodes the optional
/// DISPLAY through `get-device-terminal' (frame/live-frame/string-name);
/// nil on a tty.
fn f_display_graphic_p(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let v = a.get(0).cloned().unwrap_or(Value::Nil);
    let frame = crate::lisp::builtins::misc::decode_display(i, &v)?;
    Ok(Value::from_bool(frame_is_graphical(i, &frame)))
}

fn f_color_values(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU's second arg is a DISPLAY (default: selected); a live frame
    // is accepted and resolved to its terminal, a string names a display.
    let v = a.get(1).cloned().unwrap_or(Value::Nil);
    let _ = crate::lisp::builtins::misc::decode_display(i, &v)?;
    // Consulting the color database initializes the color machinery.
    i.color_db_init = true;
    // On a tty/batch frame GNU resolves names through the tty color
    // database and returns the best approximation's palette RGB.
    desc_rgb(i, &a[0], &Value::Nil)
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

/// `tty-color-values' — resolve COLOR through the tty color database:
/// supported names map to their palette RGB, anything else is
/// approximated to the closest supported color (or nil when unknown).
fn f_tty_color_values(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // Consulting the tty color database initializes the color
    // machinery; afterwards `x-color-values' stops erroring.
    i.color_db_init = true;
    desc_rgb(i, &a[0], a.get(1).unwrap_or(&Value::Nil))
}

/// `x-color-values' — GNU errors "Window system is not in use or not
/// initialized" until either an X connection attempt or a tty color
/// database lookup initialized the color machinery.
fn f_x_color_values(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    if !i.x_display_attempted && !i.color_db_init {
        return Err(i.error("Window system is not in use or not initialized"));
    }
    Ok(color_rgb_list(&a[0]))
}

/// `xw-color-values' — unlike `x-color-values' this needs a real X
/// connection attempt; a tty color lookup is not enough.  GNU's xw
/// palette (rgb.txt on X, the NS palette on macOS) differs slightly
/// from the tty table for the standard names.
fn f_xw_color_values(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
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
    // GNU Fxw_color_values: CHECK_STRING(color) — non-strings signal
    // stringp before the palette lookup; unknown names return nil.
    if !matches!(&a[0], Value::Str(_)) {
        return Err(i.wrong_type_mut("stringp", &a[0]));
    }
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
fn f_x_no_display(i: &mut Interp, _a: crate::lisp::Argv) -> EvalResult {
    Err(i.error("Window system is not in use or not initialized"))
}

/// Fontsets created this session via `new-fontset' — GNU keeps real
/// fontset objects; we model just existence so `set-fontset-font'
/// reaches its target validation.
fn fontset_exists(i: &Interp, name: &EmacsStr) -> bool {
    let aid = match i.q_soft("fontset-alias-alist") {
        Some(s) => s,
        None => return false,
    };
    i.symbol_value(aid)
        .list_to_vec()
        .unwrap_or_default()
        .iter()
        .any(|e| {
            if let Value::Cons(c) = e {
                let b = c.borrow();
                matches!(&b.car, Value::Str(s) if *s.borrow() == name)
                    || matches!(&b.cdr, Value::Str(s) if *s.borrow() == name)
            } else {
                false
            }
        })
}
fn fontset_register(i: &mut Interp, name: &EmacsStr) {
    if fontset_exists(i, name) {
        return;
    }
    let aid = i.q("fontset-alias-alist");
    let cur = i.symbol_value(aid);
    let entry = Value::cons(Value::string(name), Value::string(name));
    let _ = i.set_symbol(aid, Value::cons(entry, cur));
}

/// `fontset-font' — NAME string + existing fontset returns nil.
fn f_fontset_font_read(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    match &a[0] {
        Value::Str(s) => {
            let name = s.borrow().clone();
            if fontset_exists(i, &name) {
                Ok(Value::Nil)
            } else {
                Err(i.error_q("Fontset `%s' does not exist", &[&name]))
            }
        }
        other => Err(i.wrong_type_mut("stringp", other)),
    }
}

/// `set-fontset-font' — GNU fontset.c Fset_fontset_font check order:
/// (1) FONTSET arg: nil/t use the default fontset, else CHECK_STRING;
/// (2) TARGET validated (characterp / range / list / script name);
/// (3) the fontset must exist ("Fontset `X' does not exist");
/// (4) ASCII ranges must be complete or absent.
fn f_fontset_font(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let name = match &a[0] {
        Value::Str(s) => s.borrow().clone(),
        Value::Nil => "fontset-default".to_string().into(),
        Value::Sym(_) => {
            // GNU: only `t' selects the default fontset — other
            // symbols fail CHECK_STRING.
            (if matches!(a[0], Value::Sym(s) if s == sym::T) {
                "fontset-default".to_string()
            } else {
                return Err(i.wrong_type_mut("stringp", &a[0]));
            }).into()
        }
        other => return Err(i.wrong_type_mut("stringp", other)),
    };
    // GNU fontset.c Fset_fontset_font: after CHECK_STRING on FONTSET,
    // the FRAME argument (arg 3, 0-indexed) decodes (frame-live-p)
    // BEFORE the fontset-existence check — `(set-fontset-font
    // "nosuch" 97 nil 5)' reports `frame-live-p 5', and existence
    // precedes TARGET validation.
    match a.get(3) {
        None | Some(Value::Nil) | Some(Value::Frame(_)) => {}
        Some(other) => return Err(i.wrong_type_mut("frame-live-p", other)),
    }
    if !fontset_exists(i, &name) {
        return Err(i.error_q("Fontset `%s' does not exist", &[&name]));
    }
    let target = arg(&a, 1).clone();
    let mut ascii_span: Option<(i128, i128)> = None;
    match &target {
        Value::Int(n) => {
            if !(0..=0x3FFFFF).contains(n) {
                return Err(i.wrong_type_mut("characterp", &target));
            }
            ascii_span = Some((*n, *n));
        }
        Value::Cons(c) => {
            let (car, cdr) = (c.borrow().car.clone(), c.borrow().cdr.clone());
            match (&car, &cdr) {
                // (FROM . TO)
                (Value::Int(f), Value::Int(t)) => {
                    if !(0..=0x3FFFFF).contains(f) || !(0..=0x3FFFFF).contains(t) {
                        return Err(i.wrong_type_mut("characterp", &cdr));
                    }
                    ascii_span = Some((*f, *t));
                }
                // list of chars
                _ => {
                    if let Ok(items) = target.list_to_vec() {
                        for it in &items {
                            match it {
                                Value::Int(n) if (0..=0x3FFFFF).contains(n) => {}
                                other => {
                                    return Err(i.wrong_type_mut("characterp", other));
                                }
                            }
                        }
                    } else {
                        return Err(
                            i.error("Invalid second argument for setting a font in a fontset")
                        );
                    }
                }
            }
        }
        Value::Nil => {}
        Value::Sym(_) => {
            // charset/script name — GNU validates it exists.
            let sname = i.symbol_name(i.sym_id(&target).unwrap()).to_string();
            let known = crate::lisp::builtins::charset::charset_defined(i, &sname)
                || crate::lisp::builtins::misc::coding_known(i, &target).is_some()
                || matches!(
                    sname.as_str(),
                    "emoji"
                        | "latin"
                        | "symbol"
                        | "symbol-subgroup"
                        | "mathematical"
                        | "phonetic"
                        | "dingbat"
                        | "braille"
                );
            if !known {
                return Err(i.error(&format!("Invalid script or charset name: {sname}")));
            }
        }
        _ => {
            return Err(i.error("Invalid second argument for setting a font in a fontset"));
        }
    }
    if let Some((f, t)) = ascii_span {
        // GNU: ASCII spans must cover all of 0..127 or none of it.
        if (f <= 127 || t <= 127) && !(f <= 127 && t >= 127) {
            return Err(i.error("Can\u{2019}t set a font for partial ASCII range"));
        }
    }
    Ok(Value::Nil)
}

/// `new-fontset' — GNU requires a string name in XLFD form whose
/// registry field matches "fontset-*".
fn f_new_fontset(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let Value::Str(s) = &a[0] else {
        return Err(i.wrong_type_mut("stringp", &a[0]));
    };
    let name = s.borrow().clone();
    // GNU x_check_fontset_name: XLFD names (-*-...-fontset-*) pass on
    // their registry field; non-XLFD names are valid only if they
    // already appear in `fontset-alias-alist' (which is what lets the
    // literal "fontset-default" through at -Q).
    let is_alias = {
        let aid = i.q("fontset-alias-alist");
        let aliases = i.symbol_value(aid);
        let mut hit = false;
        let mut cur = aliases;
        while let Value::Cons(c) = cur {
            let (car, tail) = (c.borrow().car.clone(), c.borrow().cdr.clone());
            if let Value::Cons(entry) = &car {
                let (key, val) = (entry.borrow().car.clone(), entry.borrow().cdr.clone());
                if matches!(&key, Value::Str(k) if *k.borrow() == name)
                    || matches!(&val, Value::Str(vv) if *vv.borrow() == name)
                {
                    hit = true;
                    break;
                }
            }
            cur = tail;
        }
        hit
    };
    if !is_alias {
        if !name.contains('-') {
            return Err(i.error("Fontset name must be in XLFD format"));
        }
        let registry = name.rsplit('-').next().unwrap_or_default();
        if !registry.starts_with("fontset-") {
            return Err(i.error("Registry field of fontset name must be \"fontset-*\""));
        }
    }
    fontset_register(i, &name);
    Ok(a[0].clone())
}

/// `internal-char-font' — GNU signature is (POSITION &optional CH),
/// checking POSITION with `integer-or-marker-p', its bounds against
/// [BEGV, ZV) via `args-out-of-range (POS BEGV ZV)', then CH (or the
/// char at point when POSITION is nil) as a character.
fn f_internal_char_font(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // Explicit POSITION: integer-or-marker-p, live markers only, and
    // must satisfy BEGV <= pos < ZV (positions past the last char are
    // out of range — `(internal-char-font ZV)' errors).
    match &a[0] {
        Value::Nil => {
            // GNU takes the char at point: CHECK_CHARACTER (characterp).
            let ch = match a.get(1) {
                Some(Value::Nil) | None => {
                    let b = cur(i);
                    let bb = b.borrow();
                    let pt = bb.point();
                    if pt < bb.text.len() {
                        Some((bb.text.char_at(pt)).code() as u32 as i128)
                    } else {
                        None
                    }
                }
                Some(v) => v.int(),
            };
            match ch {
                Some(c) if (0..=crate::lisp::builtins::misc::CT_MAX_CHAR as i128).contains(&c) => {}
                _ => {
                    return Err(
                        i.wrong_type_mut("characterp", &a.get(1).cloned().unwrap_or(Value::Nil))
                    );
                }
            }
        }
        pos => {
            let p = match pos {
                Value::Int(n) => *n,
                Value::Marker(m) => match m.borrow().buffer {
                    Some(_) => m.borrow().position as i128 + 1,
                    None => return Err(i.error("Marker does not point anywhere")),
                },
                other => return Err(i.wrong_type_mut("integer-or-marker-p", other)),
            };
            let (begv, zv) = {
                let b = cur(i);
                let bb = b.borrow();
                (bb.begv as i128 + 1, bb.text.len() as i128 + 1)
            };
            if p < begv || p >= zv {
                return Err(i.signal_data(
                    sym::ARGS_OUT_OF_RANGE,
                    vec![a[0].clone(), Value::Int(begv), Value::Int(zv)],
                ));
            }
            // CHECK_NATNUM on the explicit CH: `wholenump'.
            if let Some(v) = a.get(1) {
                if !v.is_nil() {
                    match v {
                        Value::Int(n) if *n >= 0 => {}
                        other => return Err(i.wrong_type_mut("wholenump", other)),
                    }
                }
            }
        }
    }
    Ok(Value::Nil)
}

/// `internal-lisp-face-empty-p' / `internal-lisp-face-attribute-
/// values' — GNU signals a plain "Invalid face" error for unknown
/// faces; a real face yields nil on a tty.
/// `internal-lisp-face-empty-p' — t when the face's spec holds no
/// attribute besides nil/`unspecified' (GNU checks the face vector's
/// attr slots).  Unknown faces signal "Invalid face".
fn f_lisp_face_check_nil(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let _ = face_frame_live(i, &arg(&a, 1))?;
    let name = face_name_of(i, &a[0])?;
    // FRAME=t reads the new-frame defaults layer (GNU LFACE t): a face
    // with no t-defaults yet is empty even when its realized spec is set.
    let t_layer = matches!(arg(&a, 1), Value::Sym(s) if s == sym::T);
    // `remacs--raw-face' sentinel (internal-make-lisp-face's name-only
    // registration) means "Invalid face"; `unspecified' attr values don't
    // count toward non-empty.
    let raw = i.q("remacs--raw-face");
    let unsp = i.q("unspecified");
    let (is_raw, plist_empty, found) = {
        let table = if t_layer {
            &i.face_t_table
        } else {
            &i.face_table
        };
        match table.iter().find(|(n, _)| *n == name) {
            Some((_, Value::Sym(s))) => (*s == raw, true, true),
            Some((_, p)) => (
                false,
                p.list_to_vec()
                    .map(|items| {
                        !items.chunks(2).any(|kv| {
                            kv.get(1)
                                .map(|v| v.truthy() && !matches!(v, Value::Sym(s) if *s == unsp))
                                .unwrap_or(false)
                        })
                    })
                    .unwrap_or(true),
                true,
            ),
            None => (false, true, false),
        }
    };
    // internal-make-lisp-face registered the name only — GNU's
    // lface_from_face_name still fails on it.
    if is_raw {
        return Err(invalid_face(i, &a[0]));
    }
    // A face missing from the realized table keeps GNU's non-empty
    // default; missing from the t defaults table is empty there.
    let empty = plist_empty && (found || t_layer);
    Ok(Value::from_bool(empty))
}

/// `internal-lisp-face-attribute-values' — on a tty GNU reports the
/// boolean-valued attributes as `(t nil)' and everything else as nil;
/// unknown attribute names likewise yield nil.
fn f_lisp_face_attribute_values(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let Value::Sym(id) = &a[0] else {
        return Err(i.wrong_type_mut("symbolp", &a[0]));
    };
    match i.symbol_name(*id).as_str() {
        ":underline" | ":overline" | ":strike-through" | ":inverse-video" | ":extend" => {
            Ok(Value::list(vec![Value::t(), Value::Nil]))
        }
        _ => Ok(Value::Nil),
    }
}

/// `internal-merge-in-global-face' — FACE must name a known face and
/// FRAME must be live; returns nil.
fn f_internal_merge_in_global_face(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU Finternal_merge_in_global_face:
    // decode_live_frame(FRAME) runs before the face check — a nil or
    // dead frame signals frame-live-p even when FACE is invalid.
    match &a[1] {
        Value::Frame(f) if !f.borrow().dead => {}
        other => return Err(i.wrong_type_mut("frame-live-p", other)),
    }
    match &a[0] {
        Value::Sym(s) => {
            let name = *s;
            if !face_known(i, &name) {
                return Err(invalid_face(i, &a[0]));
            }
        }
        other => return Err(invalid_face(i, other)),
    }
    Ok(Value::Nil)
}

/// `describe-display-table' — requires a char-table argument.
fn f_describe_display_table(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    if crate::lisp::builtins::misc::is_char_table(i, &a[0]) {
        Ok(Value::Nil)
    } else {
        Err(i.wrong_type_mut("char-table-p", &a[0]))
    }
}

/// `redraw-frame' — frame-live-p check, then nil (no display).
fn f_redraw_frame(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
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
fn font_spec_family(i: &Interp, spec: &Value) -> Option<EmacsText> {
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
fn f_font_xlfd_name(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    if font_kind(i, &a[0]).is_none() {
        return Err(i.wrong_type_mut("font", &a[0]));
    }
    let sym_name = |i: &Interp, v: Option<Value>| -> Option<String> {
        match v {
            Some(Value::Sym(id)) => Some(i.symbol_name(id).to_string()),
            _ => None,
        }
    };
    let family = font_spec_family(i, &a[0]).unwrap_or_else(|| ("*".to_string()).into());
    let weight =
        sym_name(i, font_spec_prop(i, &a[0], ":weight")).unwrap_or_else(|| "*".to_string());
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
fn f_font_get(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
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

/// `font-put' — GNU Ffont_put CHECK_FONTs its first arg (type name
/// `font'), then returns VALUE.
fn f_font_put(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let is_spec = a[0].record_kind() == Some(crate::lisp::value::RecordKind::FontSpec);
    if !is_spec {
        return Err(i.wrong_type_mut("font-spec", &a[0]));
    }
    Ok(a[2].clone())
}

/// `x-popup-menu' — GNU menu.c x_popup_menu_1: POSITION is decoded
/// first (see decode_popup_position), then the menu (TITLE PANE...)
/// is validated — each pane a list whose car is a string and whose
/// cdr is a list of items.  Returns nil on a tty.
fn f_x_popup_menu(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    if matches!(a[0], Value::Nil) {
        return Ok(Value::Nil); // obsolete precalculate call
    }
    crate::lisp::builtins::misc::decode_popup_position(i, &a[0])?;
    check_popup_menu(i, &a[1], true)?;
    if i
        .selected_frame
        .as_ref()
        .is_some_and(|frame| crate::editor::frame_is_graphical(i, frame))
    {
        let (x, y) = i.gui_mouse_position.unwrap_or((0, 0));
        i.gui_popup_menu = Some((a[1].clone(), x, y));
    }
    Ok(Value::Nil)
}
/// `x-popup-dialog' — GNU menu.c Fx_popup_dialog: POSITION decodes a
/// window via the posn structure, THEN the contents list is checked.
/// t / (menu-bar|tab-bar|tool-bar ...) → selected window; cons →
/// posn-window car(cdr); window/frame → itself; anything else (incl.
/// nil) → window=nil and CHECK_WINDOW signals `windowp nil'.
fn f_x_popup_dialog(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    fn dialog_car(i: &mut Interp, v: &Value) -> Result<Value, Flow> {
        match v {
            Value::Cons(c) => Ok(c.borrow().car.clone()),
            Value::Nil => Ok(Value::Nil),
            other => Err(i.wrong_type_mut("listp", other)),
        }
    }
    let sel_win = sel_window(i).map(Value::Window).unwrap_or(Value::Nil);
    let window: Value = match &a[0] {
        Value::Sym(s) if *s == sym::T => sel_win.clone(),
        Value::Cons(c) => {
            let head = c.borrow().car.clone();
            let cdr = c.borrow().cdr.clone();
            let bar = |v: &Value| -> bool {
                matches!(v, Value::Sym(s) if matches!(&*i.symbol_name(*s), "menu-bar" | "tab-bar" | "tool-bar"))
            };
            if bar(&head) {
                sel_win.clone()
            } else if matches!(head, Value::Cons(_)) {
                // tem = car(position) is a cons: window = Fcar(cdr(position)).
                dialog_car(i, &cdr)?
            } else {
                // tem = Fcar(cdr(position)) — EVENT_START; window = Fcar(tem).
                let tem = dialog_car(i, &cdr)?;
                dialog_car(i, &tem)?
            }
        }
        Value::Window(_) | Value::Frame(_) => a[0].clone(),
        _ => Value::Nil,
    };
    match &window {
        Value::Frame(_) => {}
        Value::Window(_) => {}
        other => return Err(i.wrong_type_mut("windowp", other)),
    }
    // CONTENTS: emulate_dialog_with_menu → single_keymap_panes —
    // contents must be a cons (listp), first element the title string.
    match &a[1] {
        Value::Cons(c) => {
            let title = c.borrow().car.clone();
            if !matches!(title, Value::Str(_)) {
                return Err(i.wrong_type_mut("stringp", &title));
            }
        }
        Value::Nil => return Err(i.wrong_type_mut("stringp", &Value::Nil)),
        other => return Err(i.wrong_type_mut("listp", other)),
    }
    let (title, mut items) = match &a[1] {
        Value::Cons(contents) => {
            let contents = contents.borrow();
            (contents.car.clone(), contents.cdr.clone())
        }
        _ => unreachable!("contents were checked above"),
    };
    let Value::Str(title) = title else {
        unreachable!("title was checked above")
    };
    let title = title.borrow().to_unicode_lossy();
    let detail = a.get(2).and_then(|value| match value {
        Value::Str(text) => Some(text.borrow().to_unicode_lossy()),
        _ => None,
    });
    let frame = match &window {
        Value::Frame(frame) => Some(frame.clone()),
        Value::Window(window) => frame_of_window(i, window),
        _ => None,
    };
    let Some(frame) = frame.filter(|frame| frame_is_graphical(i, frame)) else {
        return Ok(Value::Nil);
    };
    let Some(backend) = i.dialog_backend.clone() else {
        return Ok(Value::Nil);
    };
    let mut buttons = Vec::new();
    let mut values = Vec::new();
    while let Value::Cons(cell) = items {
        let (item, rest) = {
            let cell = cell.borrow();
            (cell.car.clone(), cell.cdr.clone())
        };
        items = rest;
        let Value::Cons(item) = item else {
            continue;
        };
        let (label, value) = {
            let item = item.borrow();
            (item.car.clone(), item.cdr.clone())
        };
        let Value::Str(label) = label else {
            return Err(i.wrong_type_mut("stringp", &label));
        };
        buttons.push(label.borrow().to_unicode_lossy());
        values.push(value);
    }

    // Keep the interpreter on its logic thread while the frontend presents
    // the native prompt.
    if !buttons.is_empty() {
        let frame_id = frame.borrow().id;
        let selected = backend
            .choose(frame_id, &title, detail.as_deref(), &buttons)
            .map_err(|error| i.error(format!("Dialog failed: {error}")))?;
        return Ok(selected
            .and_then(|index| values.get(index).cloned())
            .unwrap_or(Value::Nil));
    }
    Ok(Value::Nil)
}

// GNU x_popup_menu_1 menu validation (menu.c):
//   keymap / list-of-keymaps → keymap_panes (no item checks);
//   otherwise "old-fashioned menu": title=Fcar(menu) CHECK_STRING,
//   then list_of_panes(Fcdr menu): each cons-tail elt gives
//   pane_name=Fcar(elt) CHECK_STRING and pane_data=Fcdr(elt)
//   CHECK_CONS (consp error); list_of_items walks the pane: strings
//   push, conses CHECK_STRING their car, other atoms are silent
//   boundaries.  Atomic tails of the pane/item lists are ignored.
fn check_popup_menu(i: &mut Interp, menu: &Value, deep: bool) -> Result<(), Flow> {
    if is_keymap(i, menu) {
        return Ok(());
    }
    let Value::Cons(mc) = menu else {
        // get_keymap(atom) → nil → else-branch Fcar(menu).
        if matches!(menu, Value::Nil) {
            return Err(i.wrong_type_mut("stringp", &Value::Nil));
        }
        return Err(i.wrong_type_mut("listp", menu));
    };
    {
        let b = mc.borrow();
        if is_keymap(i, &b.car) {
            return Ok(()); // list of keymaps
        }
        if !matches!(b.car, Value::Str(_)) {
            let t = b.car.clone();
            return Err(i.wrong_type_mut("stringp", &t));
        }
    }
    if !deep {
        return Ok(());
    }
    // list_of_panes over Fcdr(menu).
    let mut tail = mc.borrow().cdr.clone();
    while let Value::Cons(c) = tail {
        let elt = c.borrow().car.clone();
        tail = c.borrow().cdr.clone();
        let edata = match &elt {
            Value::Cons(e) => {
                let b = e.borrow();
                if !matches!(b.car, Value::Str(_)) {
                    let t = b.car.clone();
                    return Err(i.wrong_type_mut("stringp", &t));
                }
                b.cdr.clone()
            }
            Value::Nil => return Err(i.wrong_type_mut("stringp", &Value::Nil)),
            other => return Err(i.wrong_type_mut("listp", other)),
        };
        match &edata {
            Value::Cons(_) => {
                // list_of_items: cons items CHECK_STRING their car.
                let mut it = edata.clone();
                while let Value::Cons(ic) = it {
                    let item = ic.borrow().car.clone();
                    it = ic.borrow().cdr.clone();
                    if let Value::Cons(ic2) = &item {
                        if !matches!(ic2.borrow().car, Value::Str(_)) {
                            let t = ic2.borrow().car.clone();
                            return Err(i.wrong_type_mut("stringp", &t));
                        }
                    }
                }
            }
            Value::Nil => return Err(i.wrong_type_mut("consp", &Value::Nil)),
            other => return Err(i.wrong_type_mut("consp", other)),
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
                let newf = crate::lisp::builtins::evalfn::autoload_do_load(i, f.clone(), false)?;
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
    let id = i.q("keymap--builtin-maps");
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
    let tag = Value::Sym(i.q("keymap"));
    crate::lisp::builtins::misc::make_ct(i, tag, Value::Nil, vec![])
}

fn f_make_keymap(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU: (keymap CHARTABLE . ALIST) — CHARTABLE holds unmodified
    // char bindings; any non-nil argument (menu prompt, or e.g. a
    // keymap which then acts as a searchable element) follows it.
    let ct = keymap_char_table(i);
    let tail = match arg(&a, 0) {
        Value::Nil => Value::cons(ct, Value::Nil),
        v => Value::cons(ct, Value::list(vec![v.clone()])),
    };
    let km = Value::cons(Value::Sym(i.q("keymap")), tail);
    register_builtin_keymap(i, &km);
    Ok(km)
}
fn f_make_sparse_keymap(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU: (keymap . ALIST) — sparse maps have no char-table.  A
    // non-nil argument (menu prompt STRING, or any object — a keymap
    // there becomes a searchable embedded element) is the first
    // alist element.
    let rest = match arg(&a, 0) {
        Value::Nil => Value::Nil,
        v => Value::list(vec![v.clone()]),
    };
    let km = Value::cons(Value::Sym(i.q("keymap")), rest);
    register_builtin_keymap(i, &km);
    Ok(km)
}
fn f_keymapp(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
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
        if !i.sym_is(
            &c.borrow().car,
            i.obarray.intern_soft("autoload").unwrap_or(u32::MAX),
        ) {
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
/// Copy only the menu wrapper and literal subkeymaps in a definition.
/// Command conses/vectors, labels, properties and symbolic prefixes share
/// their objects, as in GNU keymap.c's copy_keymap_item.
fn copy_keymap_definition(i: &mut Interp, value: &Value, depth: usize) -> EvalResult {
    let Value::Cons(cell) = value else { return Ok(value.clone()); };
    if is_keymap(i, value) {
        return copy_keymap_map(i, value, depth);
    }
    let first = cell.borrow().car.clone();
    let menu_item = i.q("menu-item");
    let new_menu = i.sym_is(&first, menu_item);
    let cells = if new_menu {
        3
    } else if matches!(first, Value::Str(_)) {
        let rest = cell.borrow().cdr.clone();
        if matches!(&rest, Value::Cons(c) if matches!(&c.borrow().car, Value::Str(_))) { 2 } else { 1 }
    } else {
        return Ok(value.clone());
    };
    let mut rest = value.clone();
    let mut heads = Vec::new();
    for _ in 0..cells {
        let Value::Cons(cell) = &rest else { break; };
        let (car, cdr) = { let cell = cell.borrow(); (cell.car.clone(), cell.cdr.clone()) };
        heads.push(car);
        rest = cdr;
    }
    if new_menu {
        if heads.len() == 3 && is_keymap(i, &heads[2]) {
            heads[2] = copy_keymap_map(i, &heads[2], depth)?;
        }
    } else if is_keymap(i, &rest) {
        rest = copy_keymap_map(i, &rest, depth)?;
    }
    for head in heads.into_iter().rev() {
        rest = Value::cons(head, rest);
    }
    Ok(rest)
}

fn copy_keymap_map(i: &mut Interp, value: &Value, depth: usize) -> EvalResult {
    use crate::lisp::builtins::misc;
    // GNU limits nested submaps, not the number of cells in a map's spine.
    if depth > 100 {
        return Err(i.error("Possible infinite recursion when copying keymap"));
    }
    let map = crate::lisp::builtins::data::f_indirect_function(i, vec![value.clone()].into())?;
    if !is_keymap(i, &map) {
        return Err(i.wrong_type_mut("keymapp", value));
    }
    let Value::Cons(head) = &map else { unreachable!(); };
    let mut cursor = head.borrow().cdr.clone();
    let result = Value::cons(Value::Sym(i.q("keymap")), Value::Nil);
    let Value::Cons(tail) = &result else { unreachable!(); };
    let mut tail = tail.clone();
    let mut steps = 0usize;
    while let Value::Cons(cell) = &cursor {
        let (element, next) = { let cell = cell.borrow(); (cell.car.clone(), cell.cdr.clone()) };
        // The parent map is the original tail, not another owned binding.
        if i.sym_is(&element, i.q_soft("keymap").unwrap_or(u32::MAX)) {
            break;
        }
        let copied = match &element {
            Value::Record(_) if misc::is_char_table(i, &element) => {
                let table = misc::ct_copy(i, &element);
                let ranges = misc::ct_effective_runs(i, &table, true);
                for (from, to, definition) in ranges.into_iter().filter(|(_, _, v)| !v.is_nil()) {
                    let definition = copy_keymap_definition(i, &definition, depth + 1)?;
                    misc::ct_set_range(i, &table, from, to, definition);
                }
                table
            }
            Value::Vec(items) if !i.is_overlay(&element) => {
                let definitions = items.borrow().clone();
                let mut copied = Vec::with_capacity(definitions.len());
                for definition in definitions {
                    copied.push(copy_keymap_definition(i, &definition, depth + 1)?);
                }
                Value::vector(copied)
            }
            Value::Cons(binding) if !is_keymap(i, &element) => {
                let (key, definition) = { let b = binding.borrow(); (b.car.clone(), b.cdr.clone()) };
                Value::cons(key, copy_keymap_definition(i, &definition, depth + 1)?)
            }
            Value::Cons(_) => copy_keymap_map(i, &element, depth + 1)?,
            _ => element,
        };
        let new_tail = Value::cons(copied, Value::Nil);
        tail.borrow_mut().cdr = new_tail.clone();
        let Value::Cons(cell) = new_tail else { unreachable!(); };
        tail = cell;
        cursor = next;
        steps = steps.wrapping_add(1);
        if steps & 1023 == 0 { i.maybe_quit()?; }
    }
    tail.borrow_mut().cdr = cursor;
    Ok(result)
}

fn f_copy_keymap(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    copy_keymap_map(i, &a[0], 0)
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
            if i.sym_is(&b.car, i.obarray.intern_soft("keymap").unwrap_or(u32::MAX)) {
                b.cdr.clone()
            } else {
                km.clone()
            }
        }
        _ => Value::Nil,
    }
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
                let def = if matches!(val, Value::Sym(s) if i.symbol_name(s) == "t") {
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
    let merged: Vec<(Value, Value)> = chars.drain(..).map(|(_, k, d)| (k, d)).collect();
    out.splice(pos..pos, merged);
}

/// Own bindings of KEYMAP in GNU `map_keymap_internal' order: the
/// walk stops at the improper parent tail and at an embedded keymap
/// element.  Char-table contents appear at the table's spine
/// position, then alist conses follow in stored order.  In a dense
/// map, alist entries whose keys are characters or char ranges
/// (the >255 overflow our flat table cannot hold) merge into the
/// character section sorted by key, as GNU's trie iterates them.
pub(crate) fn keymap_own_bindings(i: &Interp, km: &Value) -> Vec<(Value, Value)> {
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
        if crate::lisp::builtins::misc::is_char_table(i, &elem) || matches!(elem, Value::Vec(_)) {
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
pub(crate) fn keymap_all_bindings(i: &Interp, km: &Value) -> Vec<(Value, Value)> {
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

fn f_keymap_parent(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU Fkeymap_parent: get_keymap signals
    // `keymapp' on non-keymaps (including nil).
    if !is_keymap(i, &a[0]) {
        return Err(i.wrong_type_mut("keymapp", &a[0]));
    }
    // GNU's `keymap-parent' reports only the improper spine tail (the
    // slot `set-keymap-parent' writes); bare keymap ELEMENTS embedded
    // in the alist are searched by lookup but are not the parent.  A
    // symbol tail resolves to its keymap function cell.
    let mut cur = match &a[0] {
        Value::Cons(c) => c.borrow().cdr.clone(),
        _ => return Ok(Value::Nil),
    };
    loop {
        cur = match cur.clone() {
            Value::Cons(c) => {
                if is_keymap(i, &cur) {
                    return Ok(cur);
                }
                c.borrow().cdr.clone()
            }
            tail => {
                let r = keymap_def_noautoload(i, &tail);
                return Ok(if is_keymap(i, &r) { r } else { Value::Nil });
            }
        };
    }
}

fn f_set_keymap_parent(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    if !is_keymap(i, &a[0]) {
        return Err(i.wrong_type_mut("keymapp", &a[0]));
    }
    let parent = a[1].clone();
    if !parent.is_nil() && !is_keymap(i, &parent) {
        return Err(i.wrong_type_mut("keymapp", &parent));
    }
    // Reject inheritance cycles before touching the child's spine.
    // Embedded keymap elements are bindings, not parent links.
    let mut ancestor = parent.clone();
    let mut seen = std::collections::HashSet::new();
    while let Value::Cons(cell) = &ancestor {
        if crate::lisp::builtins::eq_values(&ancestor, &a[0])
            || !seen.insert(Rc::as_ptr(cell) as usize)
        {
            return Err(i.error("Cyclic keymap inheritance"));
        }
        ancestor = f_keymap_parent(i, vec![ancestor].into())?;
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
pub(crate) fn key_seq_elt(i: &mut Interp, x: &Value) -> Result<Option<i128>, Flow> {
    match x {
        Value::Int(n) => Ok(Some(*n)),
        Value::Sym(s) => {
            // GNU reorders modifier prefixes on event symbols
            // (reorder_modifiers): M-C-left ≡ C-M-left; the parse
            // also caches event-symbol-elements on the original.
            Ok(Some(reordered_event_code(i, *s)?))
        }
        Value::Nil => Ok(Some(reordered_event_code(i, sym::NIL)?)),
        Value::Cons(c) => {
            // A (LO . HI) character range isn't a Lucid event list —
            // GNU keeps it as a range key, which our i128 codes can't
            // express; skip rather than feed it to event-convert-list.
            if matches!(&c.borrow().car, Value::Int(_)) && matches!(&c.borrow().cdr, Value::Int(_)) {
                return Ok(None);
            }
            match crate::lisp::builtins::misc::f_event_convert_list(i, vec![x.clone()].into())? {
                Value::Int(n) => Ok(Some(n)),
                Value::Sym(s) => Ok(Some(reordered_event_code(i, s)?)),
                Value::Nil => Ok(Some(reordered_event_code(i, sym::NIL)?)),
                _ => Ok(None),
            }
        }
        _ => Ok(None),
    }
}

fn reordered_event_code(i: &mut Interp, symbol: SymId) -> Result<i128, Flow> {
    let event = crate::lisp::builtins::misc::reorder_event_symbol(i, symbol)?;
    if let Value::Int(code) = event { return Ok(code); }
    let Some(symbol) = i.sym_id(&event) else { return Err(i.wrong_type_mut("symbolp", &event)); };
    Ok(event_code_for(&i.symbol_name(symbol)))
}

/// Parse a key sequence (string or vector) into event codes.
pub(crate) fn key_seq(i: &mut Interp, v: &Value) -> Result<Vec<i128>, Flow> {
    match v {
        Value::Str(s) => {
            // GNU listify_key_sequence: non-ASCII chars in a UNIBYTE
            // string are 8-bit bytes → metafied; multibyte chars
            // (like é) pass through at their own code.
            let unibyte = i.is_unibyte_str(s);
            Ok(s.borrow()
                .chars()
                .map(|character| {
                    let c = character.code() as i128;
                    if (0x80..0x100).contains(&c) && unibyte {
                        CHAR_META | (c - 0x80)
                    } else if let Some(b) =
                        crate::lisp::value::eight_bit_byte(character)
                    {
                        // Eight-bit proxy chars in unibyte strings —
                        // a raw byte is the metafied key code.
                        CHAR_META | (b as i128 - 0x80)
                    } else {
                        c
                    }
                })
                .collect())
        }
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
            // A symbol key like `quit` or `f1` (modifier prefixes
            // reordered, GNU reorder_modifiers).
            Ok(vec![reordered_event_code(i, *id)?])
        }
        other => Err(i.wrong_type_mut("sequencep", other)),
    }
}

/// Named-function-key codes. The base sits above CHAR_META and the
/// hash is masked to the low 21 bits so codes never collide with the
/// modifier bits (CHAR_ALT and above).
pub(crate) const NAMED_KEY_BASE: i128 = 0x4000_0000;
pub(crate) const NAMED_KEY_MASK: i128 = 0x1f_ffff;

const NAMED_KEY_MODIFIERS: [(i128, &str); 6] = [
    (CHAR_ALT, "A-"),
    (CHAR_CTL, "C-"),
    (CHAR_HYPER, "H-"),
    (CHAR_META, "M-"),
    (CHAR_SHIFT, "S-"),
    (CHAR_SUPER, "s-"),
];

pub fn event_code_for(mut name: &str) -> i128 {
    // Keymaps supply symbols such as C-home; front-ends supply the
    // home code with CHAR_CTL set. Hash only the base name so both
    // paths describe the same event (#297).
    let mut modifiers = 0;
    while name.len() > 2 {
        let Some((bit, prefix)) = NAMED_KEY_MODIFIERS
            .iter()
            .find(|(_, prefix)| name.starts_with(*prefix))
        else {
            break;
        };
        modifiers |= *bit;
        name = &name[prefix.len()..];
    }
    let h = {
        let mut h = 0usize;
        for b in name.bytes() {
            h = h.wrapping_mul(31).wrapping_add(b as usize);
        }
        h as i128
    };
    // 21 bits of a small hash collide often (`dp' ≡ `f2') — linear-probe
    // past occupied slots so each name gets a distinct, stable code.
    if let Ok(mut g) = named_key_names().lock() {
        let map = g.get_or_insert_with(Default::default);
        let mut code = NAMED_KEY_BASE + (h & NAMED_KEY_MASK);
        loop {
            match map.get(&code) {
                None => {
                    map.insert(code, name.to_string());
                    return code | modifiers;
                }
                Some(existing) if existing == name => return code | modifiers,
                Some(_) => {
                    code += 1;
                    if code > NAMED_KEY_BASE + NAMED_KEY_MASK {
                        code = NAMED_KEY_BASE;
                    }
                }
            }
        }
    }
    (NAMED_KEY_BASE + (h & NAMED_KEY_MASK)) | modifiers
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
pub fn key_name_for(code: i128) -> Option<String> {
    let modifier_mask = NAMED_KEY_MODIFIERS.iter().fold(0, |mask, (bit, _)| mask | bit);
    let bare = code & !modifier_mask;
    if bare < NAMED_KEY_BASE || bare > NAMED_KEY_BASE + NAMED_KEY_MASK {
        return None;
    }
    let name = named_key_names()
        .lock()
        .ok()
        .and_then(|g| g.as_ref().and_then(|m| m.get(&bare).cloned()))?;
    let mut result = String::new();
    for (bit, prefix) in NAMED_KEY_MODIFIERS {
        if code & bit != 0 {
            result.push_str(prefix);
        }
    }
    result.push_str(&name);
    Some(result)
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
    noinherit: bool,
) -> Result<Option<Value>, Flow> {
    // Only character events use the meta-prefix submap. A named event
    // such as M-left remains one symbol even though its packed code
    // now carries the same modifier bits as front-end input.
    if key < NAMED_KEY_BASE && key & META_BIT != 0 {
        let esc_b = access_keymap(i, km, 27, t_ok, noinherit)?;
        let esc = match &esc_b {
            Some(v) => keymap_def(i, v.clone())?,
            None => Value::Nil,
        };
        if is_keymap(i, &esc) {
            return access_keymap(i, &esc, key & !META_BIT, t_ok, noinherit);
        }
        return if t_ok {
            // No meta map: only the default (t) binding can match.
            key = event_code_for("t");
            access_keymap_int(i, km, key, t_ok, noinherit)
        } else {
            // An explicit nil meta binding means nil; anything else
            // leaves the key unbound here.
            match esc_b {
                Some(v) if v.is_nil() => Ok(Some(Value::Nil)),
                _ => Ok(None),
            }
        };
    }
    access_keymap_int(i, km, key, t_ok, noinherit)
}

/// The element-walk of `access_keymap_1' once meta translation is
/// done (KEY is a plain code or the `t' default key).  NOINHERIT
/// stops at the parent spine — the map's own elements only.
fn access_keymap_int(
    i: &mut Interp,
    km: &Value,
    key: i128,
    mut t_ok: bool,
    noinherit: bool,
) -> Result<Option<Value>, Flow> {
    let t_code = event_code_for("t");
    let t_sym = i.q("t");
    let keymap_sym = i.q("keymap");
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
            // GNU: `if (noinherit || NILP (retval)) break' — define-key's
            // prefix descent must not see inherited bindings at all.
            if noinherit {
                break;
            }
            match &retval {
                // An explicit nil binding shadows the parent.
                Some(v) if v.is_nil() => break,
                // A keymap result absorbs the parent's binding for
                // KEY when that is also a keymap, then stops.
                Some(_) => {
                    let pv = access_keymap_int(i, &cons, key, t_ok, false)?.unwrap_or(Value::Nil);
                    let pv = keymap_def(i, pv)?;
                    if is_keymap(i, &pv) {
                        append_keymap_hit(i, &mut retval, &mut retval_tail, pv);
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
            access_keymap_int(i, &elem, key, t_ok, noinherit)?
        } else if matches!(&elem, Value::Sym(_)) {
            // A bare symbol element whose function cell is a keymap
            // (composed maps can store raw symbols like `ESC-prefix').
            match keymap_def(i, elem.clone())? {
                v if is_keymap(i, &v) => access_keymap_int(i, &v, key, t_ok, noinherit)?,
                _ => None,
            }
        } else if crate::lisp::builtins::misc::is_char_table(i, &elem) {
            // Plain character bindings live in the char-table; a nil
            // slot is unbound (no entry at all).  GNU looks the key
            // up through `char_table_ref' (defalt/parent included).
            if key >= 0 && key & CHAR_MODIFIER_MASK == 0 {
                let s = crate::lisp::builtins::misc::char_table_ref(i, &elem, key as usize);
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
                    (Value::Int(lo), Value::Int(hi)) if *lo <= key && key <= *hi => {
                        Some(b.cdr.clone())
                    }
                    _ => None,
                }
            } else {
                let k = match &b.car {
                    Value::Int(n) => Some(*n),
                    Value::Sym(s) => Some(event_code_for(&i.symbol_name(*s))),
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
            // GNU get_keyelt(val, autoload=1): unwrap menu-items and
            // evaluate :filter — C-<down-mouse-3>'s binding yields
            // the mode's menu keymap, not `ignore'.
            let v = keyelt_command(i, &v)?;
            // GNU KEYMAPP(val) is get_keymap(val, 0, 0): symbol
            // function-cells resolve, but autoloads never load — and
            // RETVAL stores the raw VAL (a `Control-X-prefix' symbol
            // stays a symbol in the result).
            let kv = keymap_def_noautoload(i, &v);
            if is_keymap(i, &kv) {
                append_keymap_hit(i, &mut retval, &mut retval_tail, v);
            } else {
                if retval.is_none() || matches!(&retval, Some(r) if r.is_nil()) {
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
                    Value::Sym(i.q("keymap")),
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
    Ok(access_keymap(i, km, key, accept_default, false)?.unwrap_or(Value::Nil))
}

/// `access_keymap' with NOINHERIT — the map's own elements only; used
/// for define-key's prefix descent (parent bindings must not leak in).
fn lookup_in_keymap_noinherit(
    i: &mut Interp,
    km: &Value,
    key: i128,
    accept_default: bool,
) -> Result<Value, Flow> {
    Ok(access_keymap(i, km, key, accept_default, true)?.unwrap_or(Value::Nil))
}

fn f_define_key(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU resolves symbol map args via the function cell.
    let map = keymap_def(i, a[0].clone())?;
    if !is_keymap(i, &map) {
        return Err(i.wrong_type_mut("keymapp", &a[0]));
    }
    // GNU checks KEY after resolving KEYMAP, before interpreting events
    // or changing any binding.  General sequences are not sufficient.
    if !matches!(a[1], Value::Str(_) | Value::Vec(_)) {
        return Err(i.wrong_type_mut("arrayp", &a[1]));
    }
    // GNU's char-table keymaps support (LO . HI) range keys; our alist
    // model stores them as ((LO . HI) . DEF) pairs.
    if let Value::Vec(v) = &a[1] {
        let vv = v.borrow();
        if vv.len() == 1 {
            if let Value::Cons(r) = &vv[0] {
                let rb = r.borrow();
                if let (Value::Int(lo), Value::Int(hi)) = (&rb.car, &rb.cdr) {
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
    // GNU Fdefine_key rewrites a vector DEF through `lispy_event' on each
    // element: `[(48)]' stores as `[48]', `[(control 48)]' as an event
    // code — so `key-description'/describer see real events, not lists.
    let def = match &a[2] {
        Value::Vec(v) => {
            let mut changed = false;
            let mut elts = Vec::with_capacity(v.borrow().len());
            for x in v.borrow().iter() {
                let converted = if let Value::Cons(_) = x {
                    match crate::lisp::builtins::misc::f_event_convert_list(
                        i,
                        vec![x.clone()].into(),
                    )? {
                        Value::Int(n) => Value::Int(n),
                        Value::Sym(s) => Value::Sym(s),
                        _ => x.clone(),
                    }
                } else {
                    x.clone()
                };
                if !crate::lisp::builtins::equal_values(i, &converted, x)? {
                    changed = true;
                }
                elts.push(converted);
            }
            if changed {
                Value::vector(elts)
            } else {
                a[2].clone()
            }
        }
        _ => a[2].clone(),
    };
    if raw_keys.is_empty() {
        // GNU returns nil for an empty key sequence.
        return Ok(Value::Nil);
    }
    // GNU Fdefine_key metizes: a character carrying the meta bit is
    // defined through the meta-prefix (27) submap with the bit
    // stripped — [?\M-x] means [27 ?x].
    let mut keys = Vec::with_capacity(raw_keys.len());
    for &k in &raw_keys {
        if k < NAMED_KEY_BASE && k & CHAR_META != 0 {
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
        let next_raw = lookup_in_keymap_noinherit(i, &km, k, false)?;
        let next = keymap_def(i, next_raw)?;
        if is_keymap(i, &next) {
            km = next;
        } else if next.is_nil() {
            let sub = Value::cons(Value::Sym(i.q("keymap")), Value::Nil);
            register_builtin_keymap(i, &sub);
            set_binding(i, &km, k, sub.clone());
            km = sub;
        } else {
            // GNU Fdefine_key: "Key sequence %s starts with non-prefix
            // key %s" — sequence and prefix rendered via key-description.
            let render = |i: &mut Interp, ks: &[i128]| -> EmacsText {
                let v = Value::vector(ks.iter().map(|&c| Value::Int(c)).collect());
                f_key_description(i, vec![v].into())
                    .map(|r| i.princ_to_string(&r))
                    .unwrap_or_default()
            };
            let full_s = render(i, &keys);
            let pfx_s = render(i, &keys[..n + 1]);
            return Err(i.error(format!(
                "Key sequence {} starts with non-prefix key {}",
                full_s, pfx_s
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

/// GNU `get_keyelt' + `menu_item_eval_property' for command lookup:
/// `(menu-item LABEL DEF :filter FN ...)' unwraps to DEF, then the
/// `:filter' is applied as (funcall FN DEF) — a nil result hides the
/// binding (fido's C-s/C-r menu items vanish under `fido-vertical-mode').
/// `(MENUSTRING . DEFN)' strips the menu name; anything else is the value.
pub(crate) fn keyelt_command(i: &mut Interp, object: &Value) -> Result<Value, Flow> {
    let menu_item = i.q("menu-item");
    let filter_sym = i.q(":filter");
    let mut v = object.clone();
    loop {
        let Value::Cons(c) = &v else {
            return Ok(v);
        };
        let (car, cdr) = {
            let b = c.borrow();
            (b.car.clone(), b.cdr.clone())
        };
        if matches!(car, Value::Str(_)) {
            v = cdr;
            continue;
        }
        if matches!(&car, Value::Sym(s) if *s == menu_item) {
            // cdr = (LABEL DEF . PROPS)
            let mut def = Value::Nil;
            let mut props = Value::Nil;
            if let Value::Cons(x) = &cdr {
                let rest = x.borrow().cdr.clone();
                if let Value::Cons(y) = &rest {
                    let b = y.borrow();
                    def = b.car.clone();
                    props = b.cdr.clone();
                } else {
                    def = rest;
                }
            }
            let filter = crate::lisp::eval::plist_get(&props, filter_sym);
            v = if filter.truthy() {
                i.apply(&filter, vec![def])?
            } else {
                def
            };
            continue;
        }
        return Ok(v);
    }
}

/// `(menu-item NAME DEFN ...)' unwraps to DEFN (the `:filter' path is
/// only used when AUTOLOAD, which `describe-vector' never passes),
/// and `(MENUSTRING . DEFN)' strips the menu name.  Anything else is
/// already the value.
pub(crate) fn keyelt_value(i: &mut Interp, object: &Value) -> Value {
    let menu_item = i.q("menu-item");
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
                    crate::lisp::builtins::misc::ct_set(i, &car, key as u32, Value::Nil);
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
                    Value::Sym(s) => Some(event_code_for(&i.symbol_name(*s))),
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
        let t_sym = i.q("t");
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
                    if crate::lisp::builtins::misc::is_char_table(i, &car) {
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
                            crate::lisp::builtins::misc::ct_set(i, &car, key as u32, stored);
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
pub(crate) fn set_range_binding(i: &mut Interp, km: &Value, lo: i128, hi: i128, def: Value) {
    if let Value::Cons(head) = km {
        let t_sym = i.q("t");
        let stored = if def.is_nil() { Value::Sym(t_sym) } else { def };
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
                    if crate::lisp::builtins::misc::is_char_table(i, &car) {
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
            crate::lisp::builtins::misc::ct_set_range(i, &ct, lo as u32, hi as u32, stored);
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

fn f_lookup_key(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
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
    // GNU CHECK_VECTOR_OR_STRING: a non-array KEY (nil, numbers, ...)
    // signals wrong-type-argument rather than returning the keymap.
    if !matches!(&a[1], Value::Vec(_) | Value::Str(_)) {
        return Err(i.wrong_type_mut("arrayp", &a[1]));
    }
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
pub(crate) fn current_minor_maps(i: &mut Interp) -> Result<Vec<(Value, Value)>, Flow> {
    let emul_id = i.q("emulation-mode-map-alists");
    let over_id = i.q("minor-mode-overriding-map-alist");
    let reg_id = i.q("minor-mode-map-alist");
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
                        if matches!(&a.borrow().car, Value::Sym(s) if *s == var_id) {
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
            if !map.is_nil() && !matches!(&map, Value::Sym(s) if *s == sym::UNBOUND) {
                out.push((Value::Sym(var_id), map));
            }
        }
    }
    Ok(out)
}

fn f_current_minor_mode_maps(i: &mut Interp, _a: crate::lisp::Argv) -> EvalResult {
    let pairs = current_minor_maps(i)?;
    Ok(Value::list(pairs.into_iter().map(|(_, m)| m).collect()))
}

fn f_minor_mode_key_binding(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let accept = a.get(1).cloned().unwrap_or(Value::Nil);
    let mut out: Vec<Value> = Vec::new();
    for (mode, map) in current_minor_maps(i)? {
        if map.is_nil() {
            continue;
        }
        let binding = f_lookup_key(i, vec![map, a[0].clone(), accept.clone()].into())?;
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

fn f_key_binding(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // (key-binding KEY &optional accept-defaults no-remap position)
    // GNU: KEY must be an array (string or vector) — bare symbols,
    // ints, nil all signal `arrayp' before any keymap lookup; then
    // lookup over the full active-maps list — overriding maps,
    // local map, minor-mode maps, then global.
    match &a[0] {
        Value::Str(_) | Value::Vec(_) => {}
        other => return Err(i.wrong_type_mut("arrayp", other)),
    }
    let keys = key_seq(i, &a[0])?;
    let t_ok = a.get(1).map(|v| v.truthy()).unwrap_or(false);
    let am_id = i.q("current-active-maps");
    let maps = i.apply(&Value::Sym(am_id), vec![Value::t(), arg(&a, 3)])?;
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
                // GNU Fkey_binding: a command's [remap CMD] entry
                // applies unless NO-REMAP was passed; POSITION only
                // selects which buffer's keymaps are consulted.
                if let Value::Sym(cmd) = &raw {
                    let noremap = a.get(2).map(|v| v.truthy()).unwrap_or(false);
                    if !noremap {
                        let r = f_command_remapping(
                            i,
                            vec![Value::Sym(*cmd), arg(&a, 3), Value::Nil].into(),
                        )?;
                        if r.truthy() {
                            return Ok(r);
                        }
                    }
                }
                return Ok(raw);
            }
            break;
        }
    }
    Ok(Value::Nil)
}

fn f_local_key_binding(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let keys = key_seq(i, &a[0])?;
    let t_ok = a.get(1).map(|v| v.truthy()).unwrap_or(false);
    let local = {
        let b = cur(i);
        let lb = b.borrow();
        lb.locals
            .get(&i.q_soft("local-keymap").unwrap_or(u32::MAX))
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

fn f_global_key_binding(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let keys = key_seq(i, &a[0])?;
    let t_ok = a.get(1).map(|v| v.truthy()).unwrap_or(false);
    let gmap = i.symbol_value(i.q_soft("global-map").unwrap_or(0));
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

fn f_current_local_map(i: &mut Interp, _a: crate::lisp::Argv) -> EvalResult {
    let b = cur(i);
    let lb = b.borrow();
    Ok(lb
        .locals
        .get(&i.q_soft("local-keymap").unwrap_or(u32::MAX))
        .cloned()
        .unwrap_or(Value::Nil))
}

fn f_current_global_map(i: &mut Interp, _a: crate::lisp::Argv) -> EvalResult {
    Ok(i.symbol_value(i.q_soft("global-map").unwrap_or(0)))
}

fn f_use_local_map(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let b = cur(i);
    let mut bb = b.borrow_mut();
    let km_sym = i.q_soft("local-keymap").unwrap_or(u32::MAX);
    if a[0].is_nil() {
        bb.locals.shift_remove(&km_sym);
    } else {
        if !is_keymap(i, &a[0]) {
            return Err(i.wrong_type_mut("keymapp", &a[0]));
        }
        bb.locals.insert(km_sym, a[0].clone());
    }
    Ok(Value::Nil)
}

fn f_use_global_map(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    if !is_keymap(i, &a[0]) {
        return Err(i.wrong_type_mut("keymapp", &a[0]));
    }
    let gm = i.q("global-map");
    i.obarray.symbol_mut(gm).value = a[0].clone();
    Ok(Value::Nil)
}

fn f_local_set_key(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let km = f_current_local_map(i, vec![].into())?;
    let km = if is_keymap(i, &km) {
        km
    } else {
        let m = f_make_keymap(i, vec![].into())?;
        f_use_local_map(i, vec![m.clone()].into())?;
        m
    };
    f_define_key(i, vec![km, a[0].clone(), a[1].clone()].into())
}

fn f_global_set_key(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let km = f_current_global_map(i, vec![].into())?;
    let km = if is_keymap(i, &km) {
        km
    } else {
        let m = f_make_keymap(i, vec![].into())?;
        f_use_global_map(i, vec![m.clone()].into())?;
        m
    };
    f_define_key(i, vec![km, a[0].clone(), a[1].clone()].into())
}

fn f_local_unset_key(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU `Flocal_unset_key': KEY must be an array (string or
    // vector); then it only touches an existing local map.
    match &a[0] {
        Value::Str(_) | Value::Vec(_) => {}
        other => return Err(i.wrong_type_mut("arrayp", other)),
    }
    // unlike `local-set-key' it never creates one.
    let km = f_current_local_map(i, vec![].into())?;
    if is_keymap(i, &km) {
        f_define_key(i, vec![km, a[0].clone(), Value::Nil].into())?;
    }
    Ok(Value::Nil)
}
fn f_global_unset_key(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU `Fglobal_unset_key' CHECK_ARRAYs the key first.
    match &a[0] {
        Value::Str(_) | Value::Vec(_) => {}
        other => return Err(i.wrong_type_mut("arrayp", other)),
    }
    f_global_set_key(i, vec![a[0].clone(), Value::Nil].into())
}

fn f_define_prefix_command(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // (define-prefix-command COMMAND &optional MAPVAR NAME)
    let cmd = want_sym(i, &a[0])?;
    // GNU: a sparse keymap named NAME goes into COMMAND's function cell.
    let map = f_make_sparse_keymap(i, vec![arg(&a, 2)].into())?;
    i.fset(cmd, map.clone());
    // GNU: MAPVAR, when non-nil, receives the keymap as its value;
    // otherwise COMMAND's own value cell does.
    let target = match a.get(1).filter(|v| v.truthy()).and_then(|v| i.sym_id(v)) {
        Some(vs) => vs,
        None => cmd,
    };
    i.set_symbol(target, map)?;
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
fn f_command_remapping(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let Value::Sym(cmd) = a[0] else {
        return Ok(Value::Nil);
    };
    let position = a.get(1).cloned().unwrap_or(Value::Nil);
    let keymaps = a.get(2).cloned().unwrap_or(Value::Nil);
    let key = Value::vector(vec![Value::Sym(i.q("remap")), Value::Sym(cmd)]);
    let v = if keymaps.is_nil() {
        f_key_binding(
            i,
            vec![key, Value::Nil, Value::Sym(sym::T), position].into(),
        )?
    } else {
        f_lookup_key(i, vec![keymaps, key, Value::Nil].into())?
    };
    Ok(if matches!(v, Value::Int(_)) {
        Value::Nil
    } else {
        v
    })
}

/// `command-remapping' without POSITION/keymaps — the dispatcher's
/// GNU command_loop_1 `[remap CMD]' check.
pub(crate) fn command_remapping_pub(i: &mut Interp, cmd: &Value) -> Result<Value, Flow> {
    f_command_remapping(i, vec![cmd.clone(), Value::Nil, Value::Nil].into())
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
            "A-", "C-", "H-", "M-", "S-", "s-", "drag-", "down-", "double-", "triple-", "up-",
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
    "menu-bar",
    "tab-bar",
    "tool-bar",
    "tab-line",
    "header-line",
    "mode-line",
    "mouse-1",
    "mouse-2",
    "mouse-3",
    "mouse-4",
    "mouse-5",
];

/// Look up one event in MAP, which may be a keymap or a plain list
/// of keymaps (GNU's access_keymap treats a cons whose car is not
/// `keymap' as a spine of embedded maps).  Ints are character events
/// (meta bit included); symbols are named events; a cons's EVENT_HEAD
/// is its car; strings are menu keys, compared with `eq' — GNU
/// compares string keys by identity, and the sequences we look up
/// share the keymap's own string objects.
fn lookup_event(i: &mut Interp, map: &Value, ev: &Value, t_ok: bool) -> Result<Value, Flow> {
    match ev {
        Value::Int(n) => lookup_in_keymap(i, map, *n, t_ok),
        Value::Sym(s) => {
            // GNU access_keymap runs reorder_modifiers: [M-C-x]
            // finds a [C-M-x] binding (and caches the parse).
            let code = reordered_event_code(i, *s)?;
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
                return Ok(if equal_values(i, &v_lo, &v_hi)? {
                    v_lo
                } else {
                    Value::Nil
                });
            }
            // A Lucid-style event list like `(control meta ?c)' —
            // convert, then look up the resulting event (GNU
            // `Flookup_key' calls `Fevent_convert_list').
            let conv =
                crate::lisp::builtins::misc::f_event_convert_list(i, vec![ev.clone()].into())?;
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
pub(crate) fn lookup_seq(
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
pub(crate) fn seq_events(v: &Value) -> Vec<Value> {
    match v {
        Value::Str(s) => s
            .borrow()
            .chars()
            .map(|c| {
                let c = (c).code() as i128;
                Value::Int(if (0x80..0x100).contains(&c) {
                    CHAR_META | (c - 0x80)
                } else if let Some(b) =
                    crate::lisp::value::eight_bit_byte(char::from_u32(c as u32).unwrap_or('\u{0}'))
                {
                    // Eight-bit proxy chars — the byte's high bit
                    // becomes the meta bit.
                    CHAR_META | (b as i128 - 0x80)
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
pub(crate) fn shadow_lookup(
    i: &mut Interp,
    keymaps: &[Value],
    seq: &Value,
    remap: bool,
) -> Result<Value, Flow> {
    shadow_lookup_ad(i, keymaps, seq, false, remap)
}

/// `shadow_lookup' with an explicit ACCEPT-DEFAULT — describe_vector
/// passes t so a `t' (default) binding shadows too.
pub(crate) fn shadow_lookup_ad(
    i: &mut Interp,
    keymaps: &[Value],
    seq: &Value,
    accept_default: bool,
    remap: bool,
) -> Result<Value, Flow> {
    let kl = Value::list(keymaps.to_vec());
    let elts = seq_events(seq);
    let mut v = lookup_seq(i, &kl, &elts, accept_default)?;
    if matches!(v, Value::Int(_)) {
        // The sequence is too long — treated as unbound.
        v = Value::Nil;
    } else if remap && matches!(v, Value::Sym(_)) {
        let r = f_command_remapping(i, vec![v.clone(), Value::Nil, kl].into())?;
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
        let resolved = keymap_def(i, km.clone())?;
        if !is_keymap(i, &resolved) {
            return Err(i.wrong_type_mut("keymapp", km));
        }
        let acc_sym = i.q("accessible-keymaps");
        let acc = i.apply(&Value::Sym(acc_sym), vec![resolved])?;
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
                matches!(elts.get(last), Some(Value::Int(n)) if *n == 27) && !elts.is_empty();
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
                let binding = if noindirect {
                    def
                } else {
                    menu_label_def(i, def)
                };
                let matched = eq_values(&binding, definition)
                    || (matches!(definition, Value::Cons(_))
                        && equal_values(i, &binding, definition)?);
                if !matched {
                    continue;
                }
                // A `t' default binding covers the whole key space;
                // GNU's map_keymap reports it as the char ranges it
                // spans — [(32 . 126)] and [(128 . 4194303)].
                if matches!(&key, Value::Sym(s) if i.symbol_name(*s) == "t") {
                    for (lo, hi) in [(32i128, 126i128), (128i128, 4194303i128)] {
                        let cell = Value::cons(Value::Int(lo), Value::Int(hi));
                        let mut v = elts.clone();
                        v.push(cell);
                        sequences.push(Value::vector(v));
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
                sequences.push(Value::vector(v));
            }
        }
    }
    Ok(sequences)
}

fn f_where_is_internal(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let mut definition = a[0].clone();
    let keymap_arg = a.get(1).cloned().unwrap_or(Value::Nil);
    let firstonly = a.get(2).cloned().unwrap_or(Value::Nil);
    let noindirect = a.get(3).map(|v| v.truthy()).unwrap_or(false);
    let no_remap = a.get(4).map(|v| v.truthy()).unwrap_or(false);
    let non_ascii = i.q("non-ascii");
    let first_non_ascii = matches!(&firstonly, Value::Sym(s) if *s == non_ascii);
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
        Value::Cons(c) if is_keymap(i, &keymap_def_noautoload(i, &c.borrow().car)) => {
            keymap_arg.each_car(|k| keymaps.push(k.clone()));
            // Validate the whole list before advertised bindings can
            // return early, even when its first map already has a match.
            for km in &keymaps {
                let resolved = keymap_def(i, km.clone())?;
                if !is_keymap(i, &resolved) {
                    return Err(i.wrong_type_mut("keymapp", km));
                }
            }
        }
        Value::Nil => {
            let cam = i.q("current-active-maps");
            let v = i.apply(
                &Value::Sym(cam),
                crate::lisp::Argv::new(vec![Value::Nil, Value::Nil]),
            )?;
            v.each_car(|k| keymaps.push(k.clone()));
        }
        _ => {
            // GNU get_keymap: symbols resolve through their function
            // cell (autoload keymaps load), then CHECK_KEYMAP errors
            // keymapp on anything else.
            let km = keymap_def(i, keymap_arg.clone())?;
            if !is_keymap(i, &km) {
                return Err(i.wrong_type_mut("keymapp", &keymap_arg));
            }
            keymaps.push(km);
            keymaps.push(f_current_global_map(i, vec![].into())?);
        }
    }

    // Command remapping: a remapped command's bindings are the
    // remapping target's.
    if !no_remap {
        let kl = Value::list(keymaps.clone());
        let tem = f_command_remapping(i, vec![definition.clone(), Value::Nil, kl].into())?;
        if tem.truthy() {
            definition = tem;
        }
    }

    // An `advertised-binding' property overrides the search under
    // firstonly — each candidate must actually resolve back to the
    // command (shadow_lookup).
    if let Value::Sym(ds) = &definition {
        if firstonly.truthy() {
            let prop = i.q("advertised-binding");
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
    let remap_sym = i.q("remap");
    let nke_sym = i.q("non-key-event");
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
        if !equal_values(i, &v, &definition)? {
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
                let seqs = where_is_collect(i, &cmd, &keymaps, noindirect, nomenus)?;
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
        if !non_key {
            let mut duplicate = false;
            for f in &found {
                if equal_values(i, f, &sequence)? {
                    duplicate = true;
                    break;
                }
            }
            if !duplicate {
                found.push(sequence.clone());
            }
        }

        if first_non_ascii {
            return Ok(sequence);
        }
        if firstonly.truthy() && preferred_sequence_p(&sequence, preferred) == 2 {
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

fn f_defined_colors(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let v = a.get(0).cloned().unwrap_or(Value::Nil);
    let _ = crate::lisp::builtins::misc::decode_display(i, &v)?;
    Ok(Value::list(
        TTY_COLORS.iter().map(|c| Value::string(*c)).collect(),
    ))
}

fn f_color_defined_p(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU Fxw_color_defined_p decodes the FRAME/display arg FIRST via
    // get-device-terminal — a bad frame signals even when COLOR isn't a
    // string.  A non-string COLOR then returns nil (no CHECK_STRING);
    // a string goes through the tty color-database lookup.
    let frame_v = a.get(1).cloned().unwrap_or(Value::Nil);
    let _ = crate::lisp::builtins::misc::decode_display(i, &frame_v)?;
    let s = match &a[0] {
        Value::Str(s) => s.borrow().clone(),
        _ => return Ok(Value::Nil),
    };
    let desc = crate::lisp::builtins::misc::tty_color_desc(i, &a[0], &frame_v)?;
    Ok(Value::from_bool(
        matches!(desc, Value::Cons(_)) || s.is_empty(),
    ))
}

fn f_kbd(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let s = want_str(i, &a[0])?;
    let mut keys = parse_kbd(i, &(s).to_unicode_lossy())?;
    // GNU `kbd' strips the "C-x ( … C-x )" wrapper (read-kbd-macro
    // legacy): when the first two events are ?\C-x ( and the last two
    // are ?\C-x ), the inner events are the sequence.
    let cx = 24i128; // ?\C-x — the control-folded x.
    if keys.len() >= 4
        && matches!(keys[0], Value::Int(n) if n == cx)
        && matches!(keys[1], Value::Int(40))
        && matches!(keys[keys.len() - 2], Value::Int(n) if n == cx)
        && matches!(keys[keys.len() - 1], Value::Int(41))
    {
        keys = keys[2..keys.len() - 2].to_vec();
    }
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
        Ok(Value::vector(keys))
    }
}

/// GNU `key-parse' (keymap.el): whitespace-separated words, with one
/// exception — a `<...>' token may contain spaces (its body starts
/// with a non-space, non-`<', non-`>' char and ends at the next `>').
pub(crate) fn parse_kbd(i: &mut Interp, s: &str) -> Result<Vec<Value>, Flow> {
    let mut out = Vec::new();
    let b = s.as_bytes();
    let mut pos = 0usize;
    while pos < b.len() {
        while pos < b.len() && matches!(b[pos], b' ' | b'\t' | b'\n' | 0x0c) {
            pos += 1;
        }
        if pos >= b.len() {
            break;
        }
        let beg = pos;
        // <...> words: first char after < must not be space/</>/tab;
        // then everything up to the first '>'.
        if b[pos] == b'<'
            && pos + 1 < b.len()
            && !matches!(b[pos + 1], b' ' | b'<' | b'>' | b'\t' | b'\n' | 0x0c)
        {
            match s[pos..].find('>') {
                Some(off) => pos += off + 1,
                None => {
                    while pos < b.len() && !matches!(b[pos], b' ' | b'\t' | b'\n' | 0x0c) {
                        pos += 1;
                    }
                }
            }
        } else {
            while pos < b.len() && !matches!(b[pos], b' ' | b'\t' | b'\n' | 0x0c) {
                pos += 1;
            }
        }
        let mut word = &s[beg..pos];
        // N* repeat prefix (GNU: "\\([0-9]+\\)\\*." anywhere).
        let mut times = 1usize;
        if let Some(star) = word.find('*') {
            let (digs, rest) = word.split_at(star);
            if !digs.is_empty() && digs.bytes().all(|c| c.is_ascii_digit()) && !rest[1..].is_empty()
            {
                times = digs.parse().unwrap_or(1);
                word = &rest[1..];
            }
        }
        // REM / ;; comments extend to end-of-line.
        if word == "REM" || word.starts_with(";;") {
            match s[pos..].find('\n') {
                Some(off) => pos += off,
                None => pos = s.len(),
            }
            continue;
        }
        let evs = parse_key_word(i, word)?;
        for _ in 0..times {
            out.extend(evs.iter().cloned());
        }
    }
    Ok(out)
}

/// GNU `key-parse' per-word dispatch: `<<...>>' menu keys,
/// `(MODS-)*<NAME>' symbols, then the generic word parser (modifiers,
/// ^X shorthand, octal, the NUL/RET/TAB/LFD/ESC/SPC/DEL assoc table).
fn parse_key_word(i: &mut Interp, word: &str) -> Result<Vec<Value>, Flow> {
    if word.starts_with("<<") && word.ends_with(">>") && word.len() > 4 {
        // Menu shortcut: current binding of M-x (or its first
        // where-is binding), then the literal body chars and \r.
        let mx = i.q("execute-extended-command");
        let bound = f_key_binding(
            i,
            vec![Value::vector(vec![Value::Int(CHAR_META + b'x' as i128)])].into(),
        )
        .unwrap_or(Value::Nil);
        let mut evs: Vec<Value> = if matches!(&bound, Value::Sym(s) if *s == mx) {
            vec![Value::Int(CHAR_META + b'x' as i128)]
        } else {
            match f_where_is_internal(i, vec![Value::Sym(mx), Value::Nil, Value::t()].into()) {
                Ok(Value::Cons(c)) => vec![c.borrow().car.clone()],
                _ => vec![Value::Int(CHAR_META + b'x' as i128)],
            }
        };
        evs.extend(
            word[2..word.len() - 2]
                .chars()
                .map(|c| Value::Int(c as i128)),
        );
        evs.push(Value::Int(13));
        return Ok(evs);
    }
    // General word (including `(MODS-)*<NAME>' symbol events):
    // `parse_key_token' implements GNU's generic branch — modifier
    // prefixes, the ^X control shorthand, octal \NNN, the special
    // ASCII name table, and the joined-word rule for <NAME>.
    parse_key_token(i, word)
}

pub const CHAR_META: i128 = 0x0800_0000;
pub const CHAR_CTL: i128 = 0x0400_0000;
pub const CHAR_SHIFT: i128 = 0x0200_0000;
pub const CHAR_HYPER: i128 = 0x0100_0000;
pub const CHAR_SUPER: i128 = 0x0080_0000;
pub const CHAR_ALT: i128 = 0x0040_0000;

/// Parse one `kbd` token into `Value` events: `Value::Int` for
/// character events (control folded Emacs-style, remaining
/// modifiers as bits) or `Value::Sym` for named/function-key
/// events (`return`, `M-left`). A multi-char literal like `abc`
/// yields one event per char with modifiers on the first.
pub(crate) fn parse_key_token(i: &mut Interp, tok: &str) -> Result<Vec<Value>, Flow> {
    let mut mods = 0i128;
    let mut rest = tok;
    // GNU adds modifier bits arithmetically (C-C- on a char folds to
    // M-: C|C = META).  Prefixes strip in scan order, one char each.
    loop {
        // GNU's prefix regexp requires a character after the hyphen.
        if rest.len() <= 2 {
            break;
        }
        if let Some(r) = rest.strip_prefix("C-") {
            mods += CHAR_CTL;
            rest = r;
        } else if let Some(r) = rest.strip_prefix("M-") {
            mods += CHAR_META;
            rest = r;
        } else if let Some(r) = rest.strip_prefix("S-") {
            mods += CHAR_SHIFT;
            rest = r;
        } else if let Some(r) = rest.strip_prefix("H-") {
            mods += CHAR_HYPER;
            rest = r;
        } else if let Some(r) = rest.strip_prefix("s-") {
            mods += CHAR_SUPER;
            rest = r;
        } else if let Some(r) = rest.strip_prefix("A-") {
            mods += CHAR_ALT;
            rest = r;
        } else {
            break;
        }
    }
    // GNU interns `M-C-down' preserving the LITERAL modifier order
    // written by the user (not the canonical A C H M S s order).
    let mods_name = tok[..tok.len() - rest.len()].to_string();
    if rest.starts_with('<') && rest.ends_with('>') && rest.len() > 2 {
        // GNU `key-parse' joins outer modifiers with the bracketed
        // name, then: the joined word ending in `-SPECIAL' (or being
        // `SPECIAL') re-enters the generic path so inner X- prefixes
        // become modifier bits (`<C-RET>' → C-\r, `<M-C-down>' keeps
        // its inner mods as part of the symbol name).
        let name = &rest[1..rest.len() - 1];
        let joined = format!("{}{}", mods_name, name);
        for (n, _) in [
            ("NUL", 0i128),
            ("RET", 13),
            ("LFD", 10),
            ("TAB", 9),
            ("ESC", 27),
            ("SPC", 32),
            ("DEL", 127),
        ] {
            if joined == n
                || (joined.ends_with(n) && joined.as_bytes()[joined.len() - n.len() - 1] == b'-')
            {
                return parse_key_token(i, &joined);
            }
        }
        return Ok(vec![Value::Sym(i.intern(&joined))]);
    }
    // GNU `key-parse' generic branch: prefixes were consumed, `word'
    // is what remains.  The ^X shorthand contributes the control bit;
    // the NUL/RET/TAB/LFD/ESC/SPC/DEL table maps to control chars;
    // \NNN is an octal char code; then modifiers decide the event.
    let mut consumed = tok.len() - rest.len();
    let mut word = rest;
    if word.len() == 2 && word.starts_with('^') {
        mods += CHAR_CTL;
        word = &word[1..];
        consumed += 1;
    }
    let named: Option<i128> = match word {
        "NUL" => Some(0),
        "RET" => Some(13),
        "LFD" => Some(10),
        "TAB" => Some(9),
        "ESC" => Some(27),
        "SPC" => Some(32),
        "DEL" => Some(127),
        _ => None,
    };
    if let Some(n) = named {
        // GNU adds the modifier bits arithmetically (no ctl fold).
        return Ok(vec![Value::Int(mods + n)]);
    }
    if word.len() >= 2
        && word.starts_with('\\')
        && word[1..].bytes().all(|c| matches!(c, b'0'..=b'7'))
    {
        let n = i128::from_str_radix(&word[1..], 8).unwrap_or(0);
        return Ok(vec![Value::Int(mods + n)]);
    }
    let chars: Vec<char> = word.chars().collect();
    if mods == 0 {
        // Literal characters — one event each (kbd "abc").
        return Ok(chars.iter().map(|c| Value::Int(*c as i128)).collect());
    }
    if mods == CHAR_META
        && !chars.is_empty()
        && word.bytes().all(|c| c == b'-' || c.is_ascii_digit())
    {
        // M- on a digit string applies meta to each char.
        return Ok(chars
            .iter()
            .map(|c| Value::Int(mods + *c as i128))
            .collect());
    }
    if chars.len() != 1 {
        // GNU: "M- must prefix a single character, not ret".
        return Err(i.error(format!(
            "{} must prefix a single character, not {}",
            &tok[..consumed],
            word
        )));
    }
    // GNU key-parse ctl fold: bits - C + (c & 31) for @-_ a-z only
    // (? is NOT folded: C-? keeps the control bit); else bits + c.
    let c = chars[0] as i128;
    let ev = if mods & CHAR_CTL != 0 && (matches!(c, 64..=95) || (97..=122).contains(&c)) {
        mods - CHAR_CTL + (c & 31)
    } else {
        mods + c
    };
    Ok(vec![Value::Int(ev)])
}

/// GNU event-convert-list's character path, including make_ctrl_char.
pub(crate) fn apply_mods_ev(mut c: i128, mut mods: i128) -> i128 {
    if mods & CHAR_SHIFT != 0 && (97..=122).contains(&c) {
        c -= 32;
        mods &= !CHAR_SHIFT;
    }
    if mods & CHAR_CTL == 0 {
        return mods | c;
    }
    // make_ctrl_char takes a C int; non-ASCII values retain control.
    c = i128::from(c as i32);
    c = match c {
        65..=90 => (c & 31) | CHAR_SHIFT,
        64 | 91..=95 | 97..=122 => c & 31,
        0..=31 => c,
        _ => c | CHAR_CTL,
    };
    (mods & !CHAR_CTL) | c
}

fn f_key_description(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU Fkey_description:
    // KEYS (and PREFIX) must be sequences; non-sequence signals
    // `sequencep'.
    for v in [a[0].clone(), arg(&a, 1)] {
        match v {
            Value::Nil | Value::Str(_) | Value::Cons(_) | Value::Vec(_) => {}
            other => return Err(i.wrong_type_mut("sequencep", &other)),
        }
    }
    // PREFIX events are described first; a bare `meta-prefix-char' (ESC,
    // 27) folds a CHAR_META bit into the *next* event instead of printing
    // as "ESC".
    let meta = i
        .intern_soft("meta-prefix-char")
        .and_then(|s| i.symbol_value(s).int())
        .unwrap_or(27);
    let meta = meta & !(1 << 27); // GNU strips the meta bit off the char
    let meta = if meta == 0 { 27 } else { meta };
    let mut elts = Vec::new();
    if let Some(p) = a.get(1) {
        if !p.is_nil() {
            elts = seq_events(p);
        }
    }
    elts.extend(seq_events(&a[0]));
    let mut parts: Vec<EmacsText> = Vec::new();
    let mut add_meta = false;
    for e in &elts {
        if add_meta {
            match e {
                Value::Int(k) if *k == meta => {
                    // GNU: a meta-prefix event after ESC prints ESC and
                    // *keeps* add_meta — the next event is still metized.
                    parts.push(describe_key_value(i, &Value::Int(meta)));
                    continue;
                }
                Value::Int(k) if (*k & (1 << 27)) == 0 => {
                    let folded = Value::Int(k | (1 << 27));
                    parts.push(describe_key_value(i, &folded));
                }
                _ => {
                    parts.push(describe_key_value(i, &Value::Int(meta)));
                    parts.push(describe_key_value(i, e));
                }
            }
            add_meta = false;
            continue;
        }
        if matches!(e, Value::Int(k) if *k == meta) {
            add_meta = true;
            continue;
        }
        parts.push(describe_key_value(i, e));
    }
    if add_meta {
        parts.push(describe_key_value(i, &Value::Int(meta)));
    }
    // GNU builds the description on a multibyte temp string — the result
    // is always multibyte, so non-ASCII keys decode on insertion.
    let s = Value::string_with_kind(EmacsText::join(&parts, " "), true);
    if let Value::Str(r) = &s {
        i.mark_multibyte(r);
    }
    Ok(s)
}

/// `push_key_description' on one event: integer events print as
/// modified keys, symbols by name, and a (LO . HI) range prints
/// "LO..HI" (GNU's `[(32 . 126)]' → "SPC..~").
fn describe_key_value(i: &mut Interp, ev: &Value) -> EmacsText {
    match ev {
        Value::Int(n) => describe_key(*n),
        Value::Sym(s) => {
            // Symbol events print as <name>; leading character
            // modifiers (A- C- H- M- S- s-) print outside the
            // brackets: `M-next' → "M-<next>", `f1' → "<f1>".
            let mut name = i.symbol_name(*s).to_string();
            let mut pfx = EmacsText::new();
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
            format!("{pfx}<{name}>").into()
        }
        Value::Cons(c) => {
            let (car, cdr) = {
                let b = c.borrow();
                (b.car.clone(), b.cdr.clone())
            };
            match (&car, &cdr) {
                (Value::Int(lo), Value::Int(hi)) => {
                    format!("{}..{}", describe_key(*lo), describe_key(*hi)).into()
                }
                _ => {
                    // GNU key_description: a Lucid event list like
                    // `(control meta ?x)' converts to one event first.
                    if let Ok(v) = crate::lisp::builtins::misc::f_event_convert_list(
                        i,
                        vec![ev.clone()].into(),
                    ) {
                        if !v.is_nil() {
                            return describe_key_value(i, &v);
                        }
                    }
                    describe_key_value(i, &car)
                }
            }
        }
        Value::Str(s) => s.borrow().clone(),
        _ => EmacsText::new(),
    }
}

pub(crate) fn describe_key_pub(k: i128) -> String {
    describe_key(k).to_unicode_lossy()
}

pub(crate) fn describe_key(k: i128) -> EmacsText {
    let mut out = EmacsText::new();
    let modmask = CHAR_META | CHAR_CTL | CHAR_SHIFT | CHAR_SUPER | CHAR_HYPER | CHAR_ALT;
    let bare = k & !modmask;
    let mods = k & modmask;
    // GNU prints modifiers in A-C-H-M-S-s order; a control character
    // base (< 32, other than the named keys) contributes its "C-" at
    // the C position.
    let mut base_ctrl = false;
    let named: Option<&'static str> = match bare {
        9 => {
            // GNU tab_as_ci decomposes TAB only with the meta bit.
            if mods & CHAR_META != 0 {
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
    } else if base_ctrl {
        // Control char → its letter form: C-h prints 'h', C-@ prints
        // '@', C-\\ prints '\\\\'.
        let ch = match bare {
            0 => '@',
            28 => '\\',
            29 => ']',
            30 => '^',
            31 => '_',
            63 => '?', // DEL base already handled as "DEL"
            c if (1..=31).contains(&c) => (b'a' + c as u8 - 1) as char,
            // Out-of-range bases (e.g. key -1 → bare -1) print the low
            // byte like GNU's `push_key_description' (c & 0377).
            c => char::from_u32((c & 0xff) as u32).unwrap_or('?'),
        };
        out.push(ch);
    } else if k & 0x7fff_0000 != 0 && bare == 0 {
        out.push_str("<key>");
    } else {
        match crate::lisp::value::lisp_char(bare as u32) {
            Some(ch) => out.push(ch),
            // GNU push_key_description emits the mule encoding for
            // codes above the Unicode range; the seq proxy keeps the
            // description one char wide like GNU's single code.
            None => out.push(crate::lisp::value::mule_seq_char(bare as u32)),
        }
    }
    out
}

fn f_single_key_description(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let no_angles = matches!(a.get(1), Some(v) if !v.is_nil());
    single_key_desc(i, &a[0], no_angles)
}

/// GNU lucid_event_type_list_p: a proper list of ints/symbols whose
/// car isn't one of the excluded pseudo-event heads.
fn lucid_event_list_p(i: &Interp, v: &Value) -> bool {
    let Value::Cons(c) = v else { return false };
    let head = c.borrow().car.clone();
    if let Some(sym) = i.sym_id(&head) {
        let n = i.symbol_name(sym);
        if matches!(
            n.as_str(),
            "help-echo" | "vertical-line" | "mode-line" | "tab-line" | "header-line"
        ) {
            return false;
        }
    }
    let Ok(items) = v.list_to_vec() else {
        return false;
    };
    items
        .iter()
        .all(|e| matches!(e, Value::Int(_) | Value::Sym(_)))
}

fn single_key_desc(i: &mut Interp, key: &Value, no_angles: bool) -> EvalResult {
    let mut key = key.clone();
    if lucid_event_list_p(i, &key) {
        key = crate::lisp::builtins::misc::f_event_convert_list(i, vec![key].into())?;
    }
    // (LO . HI) interval from a map-char-table.
    if let Value::Cons(c) = &key {
        let (car, cdr) = {
            let cc = c.borrow();
            (cc.car.clone(), cc.cdr.clone())
        };
        if let (Value::Int(lo), Value::Int(hi)) = (&car, &cdr) {
            if car.fixnump() && cdr.fixnump() {
                let description = Value::string(format!(
                    "{}..{}",
                    describe_key(*lo & 0x0fff_ffff),
                    describe_key(*hi & 0x0fff_ffff),
                ));
                if let Value::Str(text) = &description {
                    i.mark_multibyte(text);
                }
                return Ok(description);
            }
        }
    }
    // EVENT_HEAD: cons → car, else the object itself.
    if let Value::Cons(c) = &key {
        let head = c.borrow().car.clone();
        key = head;
    }
    match &key {
        Value::Int(k) if key.fixnump() => {
            // GNU masks bits above meta and always makes a multibyte string.
            let description = Value::string(describe_key(*k & 0x0fff_ffff));
            if let Value::Str(text) = &description {
                i.mark_multibyte(text);
            }
            Ok(description)
        }
        Value::Str(_) => crate::lisp::builtins::seq::f_copy_sequence(i, vec![key.clone()].into()),
        Value::Sym(_) | Value::Nil => {
            let symbol = i.sym_id(&key).unwrap();
            let name = i.symbol_name(symbol);
            if no_angles {
                return crate::lisp::builtins::data::f_symbol_name(i, vec![key.clone()].into());
            }
            // GNU splits leading modifier prefixes and angles the base
            // key: `S-return' → "S-<return>", `C-M-down' → "C-M-<down>".
            let mut rest = name.as_str();
            let mut prefix = String::new();
            while let Some(p) = rest.find('-') {
                let head = &rest[..p + 1];
                // GNU only consumes a prefix when more than one
                // character follows it — `C-x'/'S-a' stay `<C-x>'.
                if rest.len() - (p + 1) > 1
                    && matches!(head, "C-" | "M-" | "S-" | "H-" | "A-" | "s-")
                {
                    prefix.push_str(head);
                    rest = &rest[p + 1..];
                } else {
                    break;
                }
            }
            Ok(Value::string(if prefix.is_empty() {
                format!("<{}>", name)
            } else {
                format!("{}<{}>", prefix, rest)
            }))
        }
        // GNU's Fsingle_key_description: anything else is a hard
        // error, not passthrough.
        _ => Err(i.error("KEY must be an integer, cons, symbol, or string")),
    }
}

fn describe_keymap_into(
    i: &mut Interp,
    km: &Value,
    prefix: &mut Vec<Value>,
    rows: &mut Vec<(Vec<Value>, EmacsText)>,
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
fn f_substitute_command_keys(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    if a[0].is_nil() {
        return Ok(Value::Nil);
    }
    let s = want_str(i, &a[0])?;
    let chars: Vec<EmacsChar> = s.chars().collect();
    let mut out = EmacsText::new();
    let mut pos = 0usize;
    // GNU propertizes substituted key descriptions with
    // `help-key-binding' (and gives \{map} rows separator/button
    // props).  Intervals are (start, end, plist) over `out'.
    let mut ivs: Vec<(usize, usize, Vec<Value>)> = Vec::new();
    let flf = i.q("font-lock-face");
    let hkb = i.q("help-key-binding");
    let face = i.q("face");
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
    let qstyle = match crate::lisp::builtins::evalfn::text_quoting_style(i) {
        crate::lisp::builtins::evalfn::QuoteStyle::Grave => QStyle::Grave,
        crate::lisp::builtins::evalfn::QuoteStyle::Straight => QStyle::Ascii,
        crate::lisp::builtins::evalfn::QuoteStyle::Curve => QStyle::Curve,
    };
    let take_until = |chars: &[EmacsChar], from: usize, close: char| -> Option<(EmacsText, usize)> {
        let mut j = from;
        let mut name = EmacsText::new();
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
            match chars[pos + 1].unicode().unwrap_or('\0') {
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
                            ]
                            .into(),
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
                                ]
                                .into(),
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
                                if let Ok(Value::Str(d)) = f_key_description(i, vec![k].into()) {
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
                        let id = i.intern_soft(&(name).to_unicode_lossy());
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
                                    vec![Value::Sym(face), Value::Sym(i.q("separator-line"))],
                                ));
                                for (keys, def) in rows {
                                    let mut desc = EmacsText::new();
                                    for kv in &keys {
                                        if !desc.is_empty() {
                                            desc.push(' ');
                                        }
                                        let v = Value::vector(vec![kv.clone()]);
                                        if let Ok(Value::Str(s)) =
                                            f_key_description(i, vec![v].into())
                                        {
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
                                            Value::Sym(i.q("help-args")),
                                            Value::list(vec![symv]),
                                            Value::Sym(i.q("category")),
                                            Value::Sym(i.q("help-function-button")),
                                            Value::Sym(i.q("button")),
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
                        let id = i.intern_soft(&(name).to_unicode_lossy());
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

fn f_text_char_description(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU keymap.c: ASCII controls use caret notation in a unibyte
    // string; every non-ASCII character remains one multibyte character.
    match &a[0] {
        Value::Int(n) if *n >= 0 && *n <= 0x3f_ffff => {
            let n = *n;
            let s = match n {
                0x7f => EmacsText::from("^?"),
                c if c < 0x20 => format!("^{}", char::from_u32(c as u32 + 64).unwrap()).into(),
                c => crate::lisp::value::lisp_char(c as u32)
                    .unwrap_or_else(|| crate::lisp::value::mule_seq_char(c as u32))
                    .to_string(),
            };
            let value = Value::string(s);
            if let Value::Str(string) = &value {
                if n < 128 {
                    i.mark_unibyte(string);
                } else {
                    i.mark_multibyte(string);
                }
            }
            Ok(value)
        }
        _ => Err(i.wrong_type_mut("characterp", &a[0])),
    }
}

fn pop_event_list(i: &mut Interp, name: &str) -> Option<Value> {
    let symbol = i.intern_soft(name)?;
    let Value::Cons(cell) = i.symbol_value(symbol) else { return None; };
    let (event, tail) = { let cell = cell.borrow(); (cell.car.clone(), cell.cdr.clone()) };
    i.store_internal_value(symbol, tail);
    Some(event)
}

/// Re-read command events without notifying variable watchers, as keyboard.c
/// does for its C-backed queues. Non-list values are not pending events.
pub(crate) fn pop_unread_command_event(i: &mut Interp) -> Option<Value> {
    let mut event = pop_event_list(i, "unread-command-events")?;
    if let Value::Cons(cell) = &event {
        let (head, tail) = { let cell = cell.borrow(); (cell.car.clone(), cell.cdr.clone()) };
        if i.sym_is(&head, sym::T) || i.sym_id(&head).is_some_and(|s| i.symbol_name(s) == "no-record") {
            event = tail;
        }
    }
    if let Value::Cons(cell) = &event {
        let (head, tail) = { let cell = cell.borrow(); (cell.car.clone(), cell.cdr.clone()) };
        if i.sym_id(&tail).is_some_and(|s| i.symbol_name(s) == "disabled")
            && (head.fixnump() || i.sym_id(&head).is_some()) { event = head; }
    }
    Some(event)
}

fn pop_input_method_event(i: &mut Interp, name: &str) -> Option<Value> {
    let event = pop_event_list(i, name)?;
    if let Value::Cons(cell) = &event {
        let cell = cell.borrow();
        if cell.cdr.is_nil() && (cell.car.fixnump() || i.sym_id(&cell.car).is_some()) {
            return Some(cell.car.clone());
        }
    }
    Some(event)
}

fn record_keyboard_event(i: &mut Interp, event: Value) -> Value {
    let symbol = i.q("last-input-event");
    i.store_internal_value(symbol, event.clone());
    event
}

fn pop_keyboard_event(i: &mut Interp) -> Option<Value> {
    if let Some(event) = pop_input_method_event(i, "unread-post-input-method-events") {
        return Some(record_keyboard_event(i, event));
    }
    let device = i.q("last-event-device");
    i.store_internal_value(device, Value::Nil);
    let event = pop_unread_command_event(i)
        .or_else(|| pop_input_method_event(i, "unread-input-method-events"));
    if let Some(event) = event { return Some(record_keyboard_event(i, event)); }
    if i.macro_replaying {
        if let Some(code) = i.macro_replay.pop_front() {
            let frame = i.q("last-event-frame");
            let macro_symbol = Value::Sym(i.q("macro"));
            i.store_internal_value(frame, macro_symbol);
            let event = keyboard_event_value(i, code);
            return Some(record_keyboard_event(i, event));
        }
    }
    if !i.unread_switch_frame.is_nil() {
        let event = std::mem::replace(&mut i.unread_switch_frame, Value::Nil);
        return Some(record_keyboard_event(i, event));
    }
    None
}

/// `discard-input': clears pending input — without a front-end that
/// means clearing `unread-command-events' (GNU also drops the read
/// socket backlog, which we don't have).
fn f_discard_input(i: &mut Interp, _a: crate::lisp::Argv) -> EvalResult {
    let defining = kbd_var(i, "defining-kbd-macro");
    if i.symbol_value(defining).truthy() {
        f_cancel_kbd_macro_events(i, vec![].into())?;
        end_kbd_macro_recording(i);
    }
    if let Some(sid) = i.q_soft("unread-command-events") {
        i.store_internal_value(sid, Value::Nil);
    }
    Ok(Value::Nil)
}

/// `input-pending-p': non-nil `unread-command-events' counts as
/// pending input (GNU's get_input_pending checks unread events).
/// Without a front-end there is no other pending-input source.
fn f_input_pending_p(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let pending = |i: &Interp| {
        let value = |name| i.intern_soft(name).map(|symbol| i.symbol_value(symbol)).unwrap_or(Value::Nil);
        matches!(value("unread-command-events"), Value::Cons(_))
            || !value("unread-input-method-events").is_nil()
            || !value("unread-post-input-method-events").is_nil()
    };
    if pending(i) { return Ok(Value::t()); }
    if arg(&a, 0).truthy() { crate::lisp::builtins::evalfn::timer_check(i)?; }
    Ok(Value::from_bool(pending(i)))
}

pub(crate) fn f_read_char(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    read_filtered_event(i, a, Some(true))
}

fn f_read_event(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    read_filtered_event(i, a, None)
}

fn f_read_char_exclusive(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    read_filtered_event(i, a, Some(false))
}

fn keyboard_interaction_allowed(i: &mut Interp) -> Result<(), Flow> {
    let inhibited = i.q("inhibit-interaction");
    if i.bound_p(inhibited) && i.symbol_value(inhibited).truthy() {
        let condition = i.q("inhibited-interaction");
        return Err(i.signal_data(condition, vec![]));
    }
    Ok(())
}

fn keyboard_event_value(i: &mut Interp, code: i128) -> Value {
    match key_name_for(code) {
        Some(name) => Value::Sym(i.intern(&name)),
        None => Value::Int(code),
    }
}

#[derive(Clone, Copy)]
struct EventDeadline {
    numeric: bool,
    end: Option<std::time::Instant>,
}

fn event_deadline(seconds: &Value) -> EventDeadline {
    let secs = match seconds {
        Value::Int(n) => Some(*n as f64),
        Value::Float(f) => Some(**f),
        Value::BigInt(n) => {
            use num_traits::ToPrimitive;
            Some(n.to_f64().unwrap_or_else(|| if n.sign() == num_bigint::Sign::Minus {
                f64::NEG_INFINITY
            } else { f64::INFINITY }))
        }
        _ => None,
    };
    let now = std::time::Instant::now();
    let end = secs.and_then(|secs| if secs > 0.0 {
        std::time::Duration::try_from_secs_f64(secs).ok().and_then(|duration| now.checked_add(duration))
    } else { Some(now) });
    EventDeadline { numeric: secs.is_some(), end }
}

fn keyboard_prompt(i: &mut Interp, prompt: &Value) -> EmacsText {
    if prompt.is_nil() { return EmacsText::new(); }
    let text = i.princ_to_string(prompt);
    i.message(&text);
    text
}

fn checked_event_car(i: &mut Interp, value: &Value) -> EvalResult {
    match value {
        Value::Nil => Ok(Value::Nil),
        Value::Cons(cell) => Ok(cell.borrow().car.clone()),
        other => Err(i.wrong_type_mut("listp", other)),
    }
}

fn checked_event_cdr(i: &mut Interp, value: &Value) -> EvalResult {
    match value {
        Value::Nil => Ok(Value::Nil),
        Value::Cons(cell) => Ok(cell.borrow().cdr.clone()),
        other => Err(i.wrong_type_mut("listp", other)),
    }
}

/// keyboard.c read_filtered_event; None accepts all events, Some controls
/// whether non-characters signal or are skipped. The deadline spans retries.
fn read_filtered_event(i: &mut Interp, a: crate::lisp::Argv, characters: Option<bool>) -> EvalResult {
    keyboard_interaction_allowed(i)?;
    let prompt = keyboard_prompt(i, &arg(&a, 0));
    let deadline = event_deadline(&arg(&a, 2));
    let mut delayed = Value::Nil;
    let result = (|| loop {
        let mut event = read_keyboard_event(i, &(prompt).to_unicode_lossy(), deadline)?;
        if matches!(event, Value::Buffer(_)) { continue; }
        if characters.is_some() {
            if let Value::Cons(cell) = &event {
                let head = cell.borrow().car.clone();
                let Some(symbol) = i.sym_id(&head) else { return Err(i.wrong_type_mut("symbolp", &head)); };
                let kind = i.q("event-kind");
                let switch_frame = i.q("switch-frame");
                if i.sym_is(&i.get_prop(symbol, kind), switch_frame) {
                    delayed = event;
                    continue;
                }
            }
            if !(deadline.numeric && event.is_nil()) {
                if let Some(symbol) = i.sym_id(&event) {
                    let mask_property = i.q("event-symbol-element-mask");
                    let mask = i.get_prop(symbol, mask_property);
                    if !mask.is_nil() {
                        let base = checked_event_car(i, &mask)?;
                        let Some(base) = i.sym_id(&base) else { return Err(i.wrong_type_mut("symbolp", &base)); };
                        let ascii_property = i.q("ascii-character");
                        let ascii = i.get_prop(base, ascii_property);
                        if ascii.fixnump() {
                            let tail = checked_event_cdr(i, &mask)?;
                            let modifiers = checked_event_car(i, &tail)?;
                            if modifiers.fixnump() {
                                event = Value::Int(ascii.int().unwrap() | modifiers.int().unwrap());
                            }
                        }
                    }
                }
                if !event.fixnump() {
                    if characters == Some(true) {
                        let queue = i.q("unread-command-events");
                        i.store_internal_value(queue, Value::list(vec![event]));
                        return Err(i.error("Non-character input-event"));
                    }
                    continue;
                }
            }
            if let Value::Int(code) = event {
                event = Value::Int(crate::lisp::builtins::charset::resolve_modifier_mask(code));
            }
        }
        return Ok(event);
    })();
    // Keep delayed events live while signal observers run, before unwinding.
    let result = i.offer_flow(result);
    if result.is_ok() && !delayed.is_nil() { i.unread_switch_frame = delayed; }
    result
}

fn read_keyboard_event(i: &mut Interp, prompt: &str, deadline: EventDeadline) -> EvalResult {
    if let Some(event) = pop_keyboard_event(i) { return Ok(event); }
    if deadline.numeric || (i.minibuf_reader.is_none() && i.noninteractive) {
        loop {
            // GNU's read_char waits inside
            // `wait_reading_process_output', which runs `timer_check'
            // — `with-timeout' must be able to fire while polling.
            crate::lisp::builtins::evalfn::thread_wait_events(i)?;
            i.maybe_quit()?;
            if let Some(ev) = pop_keyboard_event(i) {
                return Ok(ev);
            }
            if deadline.end.is_some_and(|end| std::time::Instant::now() >= end) {
                return Ok(Value::Nil);
            }
            crate::lisp::eval::threads::pause(i, std::time::Duration::from_millis(1))?;
        }
    }
    if i.minibuf_reader.is_some() {
        match i.minibuf_input(prompt, true)? {
            crate::lisp::eval::MinibufInput::Key(k) => {
                let event = keyboard_event_value(i, k);
                return Ok(record_keyboard_event(i, event));
            }
            crate::lisp::eval::MinibufInput::Event(event) => {
                return Ok(record_keyboard_event(i, event));
            }
            crate::lisp::eval::MinibufInput::Call(_) => {
                return Err(i.error("exit function in key read"));
            }
            crate::lisp::eval::MinibufInput::Text(t) => {
                let n = t.chars().next().map(|c| (c).code() as i128).unwrap_or(0);
                return Ok(record_keyboard_event(i, Value::Int(n)));
            }
        }
    }
    // No front-end and not batch: a real event read can never
    // complete.  `read-event'/`read-char' with a SECONDS argument are
    // timeout reads — GNU would return nil after the delay, so a nil
    // return is correct there.  An unbounded read must not return nil
    // (callers spin forever waiting for input); signal EOF like
    // batch mode so `condition-case'/`ci' callers move on.
    Err(batch_eof(i))
}

pub(crate) fn f_read_key_sequence(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    read_key_sequence_events(i, a, false)
}
pub(crate) fn f_read_key_sequence_vector(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    read_key_sequence_events(i, a, true)
}

fn read_key_sequence_events(i: &mut Interp, a: crate::lisp::Argv, vector: bool) -> EvalResult {
    let prompt = keyboard_prompt(i, &arg(&a, 0));
    let deadline = EventDeadline { numeric: false, end: None };
    let mut events = Vec::new();
    loop {
        events.push(read_keyboard_event(i, &(prompt).to_unicode_lossy(), deadline)?);
        let keys = Value::vector(events.clone());
        let binding = f_key_binding(i, vec![keys, Value::t(), Value::t()].into())?;
        let binding = keymap_def(i, binding)?;
        if !is_keymap(i, &binding) { break; }
    }
    if vector || events.iter().any(|event| !event.fixnump()
        || event.int().is_none_or(|code| code & ((1 << 27) - 1) >= 128)) {
        return Ok(Value::vector(events));
    }
    // alloc.c make_event_array packs ASCII plus Meta into a unibyte string.
    let text: String = events.iter().map(|event| {
        let code = event.int().unwrap();
        let byte = code as u8 | if code & CHAR_META != 0 { 0x80 } else { 0 };
        char::from(byte)
    }).collect();
    let result = Value::string(text);
    if let Value::Str(string) = &result { i.mark_unibyte(string); }
    Ok(result)
}
fn f_this_command_keys(i: &mut Interp, _a: crate::lisp::Argv) -> EvalResult {
    let _ = i;
    Ok(Value::string(""))
}
fn f_this_command_keys_vector(i: &mut Interp, _a: crate::lisp::Argv) -> EvalResult {
    let _ = i;
    Ok(Value::vector(Vec::new()))
}

// ---------- kill ring ----------

fn f_kill_new(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU Fkill_new's arg chain:
    // strings pass; non-string SEQUENCES (nil, conses, vectors,
    // char-tables, bool-vectors) signal stringp; non-sequences (numbers,
    // symbols) signal sequencep — verified empirically on GNU 31.1.
    match &a[0] {
        Value::Str(_) => {}
        Value::Nil | Value::Cons(_) | Value::Vec(_) => {
            return Err(i.wrong_type_mut("stringp", &a[0]));
        }
        Value::Record(_) => {
            if crate::lisp::builtins::misc::is_bool_vector(i, &a[0])
                || crate::lisp::builtins::misc::is_char_table(i, &a[0])
            {
                return Err(i.wrong_type_mut("stringp", &a[0]));
            }
            return Err(i.wrong_type_mut("sequencep", &a[0]));
        }
        other => return Err(i.wrong_type_mut("sequencep", other)),
    }
    let text = a[0].clone();
    let ring_symbol = i.q("kill-ring");
    let ring = i.symbol_value(ring_symbol);
    let duplicates = i.q("kill-do-not-save-duplicates");
    if i.symbol_value(duplicates).truthy() {
        let head = match &ring {
            Value::Nil => Value::Nil,
            Value::Cons(cell) => cell.borrow().car.clone(),
            other => return Err(i.wrong_type_mut("listp", other)),
        };
        if crate::lisp::builtins::equal_values_including_properties(i, &text, &head)? {
            let pointer = i.q("kill-ring-yank-pointer");
            i.set_symbol(pointer, ring)?;
            crate::buffer::primitives::interprogram_cut(i, text)?;
            return Ok(Value::Nil);
        }
    }
    if arg(&a, 1).truthy() && !ring.is_nil() {
        // GNU REPLACE changes the existing head cons, preserving its tail.
        let Value::Cons(cell) = &ring else {
            return Err(i.wrong_type_mut("listp", &ring));
        };
        cell.borrow_mut().car = text.clone();
        let pointer = i.q("kill-ring-yank-pointer");
        i.set_symbol(pointer, ring)?;
        crate::buffer::primitives::interprogram_cut(i, text)?;
    } else {
        crate::buffer::primitives::push_kill_ring_value(i, text)?;
    }
    Ok(Value::Nil)
}
fn f_kill_append(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU's kill-append is Lisp: it feeds `(concat cur STRING)' to
    // kill-new — so STRING is any sequence (sequencep), not just strings.
    let mut s = EmacsText::new();
    crate::lisp::builtins::strfn::concat_seq(i, &a[0], &mut s)?;
    // BEFORE-P non-nil prepends to the newest entry.
    let before = !arg(&a, 1).is_nil();
    let kr = i.q("kill-ring");
    let cur = i.symbol_value(kr);
    let mut items = cur.list_to_vec().unwrap_or_default();
    if let Some(Value::Str(top)) = items.first_mut() {
        let mut t = top.borrow_mut();
        if before {
            let mut joined = s.clone();
            joined.push_str(&*t);
            *t = joined.into();
        } else {
            t.push_str(&s);
        }
    } else {
        items.insert(0, Value::string(s));
    }
    i.obarray.symbol_mut(kr).value = Value::list(items);
    // GNU's kill-append delegates to kill-new, so the merged string
    // also goes through `interprogram-cut-function'.
    if let Some(icf_id) = i.q_soft("interprogram-cut-function") {
        let icf = i.symbol_value(icf_id);
        if icf.truthy() {
            let items = i.symbol_value(kr).list_to_vec().unwrap_or_default();
            if let Some(top) = items.into_iter().next() {
                i.apply(&icf, vec![top])?;
            }
        }
    }
    Ok(Value::Nil)
}

fn f_current_kill(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // These are byte-op primitives in GNU's compiled current-kill.
    fn primitive(i: &mut Interp, name: &str, args: Vec<Value>) -> EvalResult {
        let subr = crate::lisp::builtins::arith::SUBRS
            .iter()
            .chain(crate::lisp::builtins::listfn::SUBRS.iter())
            .find(|subr| subr.name == name)
            .expect("current-kill primitive");
        (subr.func)(i, args.into())
    }
    let n = arg(&a, 0);
    let zero = primitive(i, "=", vec![n.clone(), Value::Int(0)])?.truthy();
    let paste_symbol = i.q("interprogram-paste-function");
    let paste_function = i.symbol_value(paste_symbol);
    let paste = if zero && paste_function.truthy() {
        i.apply(&paste_function, vec![])?
    } else {
        Value::Nil
    };
    let ring_symbol = i.q("kill-ring");
    if paste.truthy() {
        let depth = i.specbind_depth();
        let result = (|| {
            let cut = i.q("interprogram-cut-function");
            i.specbind(cut, Value::Nil)?;
            i.specbind(paste_symbol, Value::Nil)?;
            let kill_new = Value::Sym(i.q("kill-new"));
            if matches!(paste, Value::Cons(_)) {
                // GNU reverses a copy so the external list is untouched.
                let reverse = Value::Sym(i.q("reverse"));
                let reversed = i.apply(&reverse, vec![paste])?;
                for string in crate::lisp::builtins::want_list(i, &reversed)? {
                    i.apply(&kill_new, vec![string])?;
                }
            } else {
                i.apply(&kill_new, vec![paste])?;
            }
            Ok(Value::Nil)
        })();
        i.unbind_to(depth)?;
        result?;
        return primitive(i, "car", vec![i.symbol_value(ring_symbol)]);
    }
    let ring = i.symbol_value(ring_symbol);
    if ring.is_nil() {
        return Err(i.error("Kill ring is empty"));
    }
    let pointer_symbol = i.q("kill-ring-yank-pointer");
    let pointer_length = primitive(i, "length", vec![i.symbol_value(pointer_symbol)])?;
    let offset = primitive(i, "-", vec![n.clone(), pointer_length])?;
    let ring_length = primitive(i, "length", vec![ring.clone()])?;
    let index = primitive(i, "mod", vec![offset, ring_length])?;
    let tail = primitive(i, "nthcdr", vec![index, ring])?;
    if !arg(&a, 1).truthy() {
        i.set_symbol(pointer_symbol, tail.clone())?;
        let selection = i.q("yank-pop-change-selection");
        if i.symbol_value(selection).truthy() && primitive(i, ">", vec![n, Value::Int(0)])?.truthy()
        {
            let cut_symbol = i.q("interprogram-cut-function");
            let cut = i.symbol_value(cut_symbol);
            if cut.truthy() {
                let string = primitive(i, "car", vec![tail.clone()])?;
                i.apply(&cut, vec![string])?;
            }
        }
    }
    primitive(i, "car", vec![tail])
}
fn f_copy_region_as_kill(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let b = cur(i);
    let region = a.get(2).cloned().unwrap_or(Value::Nil);
    // GNU simple.el: a non-nil REGION ignores BEG/END and extracts the
    // marked region via `region-extract-function'. Buffer substring keeps
    // the text properties on the value copied into the kill ring.
    let (start, end) = if region.truthy() {
        let (lo, hi) = crate::buffer::primitives::mark_region_bounds(i)?;
        (lo as i128 + 1, hi as i128 + 1)
    } else {
        // GNU `validate_region': out-of-[BEGV,ZV] args signal
        // args-out-of-range (buffer beg end), never clamped.
        let s0 = want_int(i, &a[0])?;
        let e0 = want_int(i, &a[1])?;
        let (s1, e1) = (s0.min(e0), s0.max(e0));
        let bb = b.borrow();
        if s1 < bb.begv as i128 + 1 || e1 > bb.zv as i128 + 1 {
            let bufv = i.buffer_value(i.current_buffer).unwrap_or(Value::Nil);
            return Err(err_sym(
                i,
                "args-out-of-range",
                vec![bufv, a[0].clone(), a[1].clone()],
            ));
        }
        (s1, e1)
    };
    let text = crate::buffer::primitives::f_buffer_substring(
        i,
        vec![Value::Int(start), Value::Int(end)].into(),
    )?;
    // GNU: `(if (eq last-command 'kill-region) (kill-append str (< end
    // beg)) (kill-new str))' — the `<' evaluates END first, so junk
    // BEG/END under a non-nil REGION still signals number-or-marker-p.
    crate::buffer::primitives::kill_or_append(
        i,
        &text,
        a.get(0).unwrap_or(&Value::Nil),
        a.get(1).unwrap_or(&Value::Nil),
        false,
    )?;
    // deactivate mark per Emacs
    let ma = i.q_soft("mark-active").unwrap_or(0);
    b.borrow_mut().locals.insert(ma, Value::Nil);
    Ok(Value::Nil)
}

fn f_yank(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let prefix = arg(&a, 0);
    let window_start = Value::Sym(i.q("window-start"));
    let start = i.apply(&window_start, vec![])?;
    let yank_start = i.q("yank-window-start");
    i.set_symbol(yank_start, start)?;
    let this_command = i.q("this-command");
    i.set_symbol(this_command, Value::t())?;
    let push_mark = Value::Sym(i.q("push-mark"));
    i.apply(&push_mark, vec![])?;
    let minus = i.q("-");
    let index = match &prefix {
        Value::Nil | Value::Cons(_) => Value::Int(0),
        Value::Sym(symbol) if *symbol == minus => Value::Int(-2),
        _ => {
            let subtract = crate::lisp::builtins::arith::SUBRS
                .iter()
                .find(|subr| subr.name == "1-")
                .unwrap();
            (subtract.func)(i, vec![prefix.clone()].into())?
        }
    };
    let current_kill = Value::Sym(i.q("current-kill"));
    let string = i.apply(&current_kill, vec![index])?;
    let insert = Value::Sym(i.q("insert-for-yank"));
    i.apply(&insert, vec![string])?;
    if matches!(prefix, Value::Cons(_)) {
        let mark = Value::Sym(i.q("mark"));
        let before = i.apply(&mark, vec![Value::t()])?;
        let mark_marker = Value::Sym(i.q("mark-marker"));
        let marker = i.apply(&mark_marker, vec![])?;
        let point = cur(i).borrow().point() + 1;
        let buffer = i.buffer_value(i.current_buffer).unwrap();
        let set_marker = Value::Sym(i.q("set-marker"));
        i.apply(&set_marker, vec![marker, Value::Int(point as i128), buffer])?;
        let goto = Value::Sym(i.q("goto-char"));
        i.apply(&goto, vec![before])?;
    }
    if i.sym_is(&i.symbol_value(this_command), crate::lisp::obarray::sym::T) {
        let yank = Value::Sym(i.q("yank"));
        i.set_symbol(this_command, yank)?;
    }
    Ok(Value::Nil)
}
fn f_yank_pop(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU simple.el 31.1: when last-command isn't `yank', this is
    // `(yank-from-kill-ring (read-from-kill-ring "Yank from kill-ring: ")
    //  current-prefix-arg)' — in batch the minibuffer read EOFs.
    let lc_id = i.q("last-command");
    let lc = i.symbol_value(lc_id);
    let yk = i.q("yank");
    if !matches!(lc, Value::Sym(s) if s == yk) {
        let rfkr = i.q("read-from-kill-ring");
        let s = i.apply(
            &Value::Sym(rfkr),
            vec![Value::string("Yank from kill-ring: ")],
        )?;
        let cpa_id = i.q("current-prefix-arg");
        let cpa = i.symbol_value(cpa_id);
        let yfkr = i.q("yank-from-kill-ring");
        return i.apply(&Value::Sym(yfkr), vec![s, cpa]);
    }
    // (setq this-command 'yank)
    let tc = i.q("this-command");
    let _ = i.set_symbol(tc, Value::Sym(yk));
    // (unless arg (setq arg 1))
    let arg_v = if a.is_empty() {
        Value::Int(1)
    } else {
        a[0].clone()
    };
    // (let ((inhibit-read-only t) (before (< (point) (mark t)))) ...)
    let iro = i.q("inhibit-read-only");
    let mark_depth = i.specbind_depth();
    let _ = i.specbind(iro, Value::t());
    let result = (|| -> EvalResult {
        let b = cur(i);
        let (m, p) = {
            let bb = b.borrow();
            (bb.mark_position(), bb.point())
        };
        // GNU: `(< (point) (mark t))' with an unset mark signals
        // wrong-type-argument number-or-marker-p nil.
        let m = match m {
            Some(m) => m,
            None => return Err(i.wrong_type_mut("number-or-marker-p", &Value::Nil)),
        };
        let before = p < m;
        // (funcall (or yank-undo-function 'delete-region) ...)
        let yuf = i.q("yank-undo-function");
        let fn_v = match i.symbol_value(yuf) {
            Value::Nil => {
                let dr = i.q("delete-region");
                Value::Sym(dr)
            }
            v => v,
        };
        let (s, e) = if before { (p, m) } else { (m, p) };
        i.apply(&fn_v, vec![Value::Int(s as i128), Value::Int(e as i128)])?;
        // (setq yank-undo-function nil)
        let _ = i.set_symbol(yuf, Value::Nil);
        // (set-marker (mark-marker) (point) (current-buffer))
        {
            let pt = b.borrow().point();
            b.borrow_mut().set_mark(pt);
        }
        // (insert-for-yank (current-kill arg))
        let ck = i.q("current-kill");
        let killed = i.apply(&Value::Sym(ck), vec![arg_v])?;
        let ify = i.q("insert-for-yank");
        i.apply(&Value::Sym(ify), vec![killed])?;
        // (set-window-start (selected-window) yank-window-start t)
        let yws = i.q("yank-window-start");
        if i.bound_p(yws) {
            let ws = i.symbol_value(yws);
            if matches!(ws, Value::Int(_) | Value::Marker(_)) {
                let sws = i.q("set-window-start");
                let sw = i.q("selected-window");
                if let Ok(w) = i.apply(&Value::Sym(sw), vec![]) {
                    let _ = i.apply(&Value::Sym(sws), vec![w, ws, Value::t()]);
                }
            }
        }
        // (goto-char (prog1 (mark t) (set-marker (mark-marker) (point))))
        if before {
            let np = b.borrow().point();
            b.borrow_mut().set_point(np);
            let mp = b.borrow().mark_position().unwrap_or(np);
            b.borrow_mut().set_point(mp);
        }
        Ok(Value::Nil)
    })();
    i.unbind_to(mark_depth)?;
    result
}

fn f_rotate_yank_pointer(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU simple.el: `(current-kill ARG)' with
    // do-not-move nil — nil/empty kill-ring signals "Kill ring is
    // empty" and the value is `(car ARGth-kill-element)', not nil.
    let n = want_num_or_mark(i, &arg(&a, 0))?;
    let kr = i.q("kill-ring");
    let ipf = i.q("interprogram-paste-function");
    // N=0 consults the interprogram paste function first; a non-nil
    // result is killed into the ring and its first item returned.
    if n == 0 {
        let f = i.symbol_value(ipf);
        if f.truthy() {
            if let Ok(paste) = i.apply(&f, vec![]) {
                if paste.truthy() {
                    let m = i.specbind_depth();
                    let icf = i.q("interprogram-cut-function");
                    let _ = i.specbind(icf, Value::Nil);
                    let _ = i.specbind(ipf, Value::Nil);
                    let killed = if let Ok(items) = paste.list_to_vec() {
                        let mut r: EvalResult = Ok(Value::Nil);
                        for it in items.iter().rev() {
                            let kn = i.q("kill-new");
                            r = i.apply(&Value::Sym(kn), vec![it.clone()]);
                            if r.is_err() {
                                break;
                            }
                        }
                        r
                    } else {
                        // Scalar (non-list) paste: a single kill-new.
                        let kn = i.q("kill-new");
                        i.apply(&Value::Sym(kn), vec![paste])
                    };
                    i.unbind_to(m)?;
                    killed?;
                    let top = nthcdr_of(&i.symbol_value(kr), 0);
                    return Ok(match &top {
                        Value::Cons(c) => c.borrow().car.clone(),
                        _ => Value::Nil,
                    });
                }
            }
        }
    }
    let ring = i.symbol_value(kr);
    if matches!(ring, Value::Nil) {
        return Err(i.error("Kill ring is empty"));
    }
    let ptrsym = i.q("kill-ring-yank-pointer");
    // GNU evaluates `(length kill-ring-yank-pointer)' before
    // `(length kill-ring)' — both are CHECK_SEQUENCE'd, so a
    // non-sequence pointer or ring signals `sequencep'.
    let length_sym = i.q("length");
    let plen = match i.apply(&Value::Sym(length_sym), vec![i.symbol_value(ptrsym)])? {
        Value::Int(n) => n,
        _ => 0,
    };
    let rlen = match i.apply(&Value::Sym(length_sym), vec![ring.clone()])? {
        Value::Int(n) => n,
        _ => 0,
    };
    if rlen == 0 {
        return Err(i.error("Kill ring is empty"));
    }
    let pos = (((n - plen) % rlen) + rlen) as usize % rlen as usize;
    // GNU `(nthcdr POS kill-ring)': a non-list ring (a string is a
    // sequence but not a list) signals `listp' on the ring itself.
    if !matches!(ring, Value::Cons(_) | Value::Nil) {
        return Err(i.wrong_type_mut("listp", &ring));
    }
    let elem = nthcdr_of(&ring, pos);
    if !matches!(elem, Value::Cons(_)) {
        return Err(i.wrong_type_mut("listp", &elem));
    }
    // do-not-move nil: set the pointer, and when
    // `yank-pop-change-selection' is set push the new top kill
    // through `interprogram-cut-function'.
    let _ = i.set_symbol(ptrsym, elem.clone());
    let first = match &elem {
        Value::Cons(c) => c.borrow().car.clone(),
        _ => Value::Nil,
    };
    let ypcs = i.q("yank-pop-change-selection");
    if i.symbol_value(ypcs).truthy() && n > 0 {
        let icf = i.q("interprogram-cut-function");
        let f = i.symbol_value(icf);
        if f.truthy() {
            i.apply(&f, vec![first.clone()])?;
        }
    }
    Ok(first)
}

fn f_copy_to_buffer(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
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

fn f_append_to_buffer(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU simple.el: retain the source buffer, resolve the destination and
    // its windows, then insert through the property-aware transfer primitive.
    let source = Value::Buffer(cur(i));
    let get_buffer = Value::Sym(i.q("get-buffer-create"));
    let target = i.apply(&get_buffer, vec![a[0].clone()])?;
    let get_windows = Value::Sym(i.q("get-buffer-window-list"));
    let mut windows = i.apply(&get_windows, vec![target.clone(), Value::t(), Value::t()])?;
    let excursion = i.save_excursion_state(false);
    let result = (|| {
        let set_buffer = Value::Sym(i.q("set-buffer"));
        i.apply(&set_buffer, vec![target])?;
        let point = Value::Int(cur(i).borrow().point() as i128 + 1);
        let writable = Value::Sym(i.q("barf-if-buffer-read-only"));
        i.apply(&writable, vec![])?;
        let insert = Value::Sym(i.q("insert-buffer-substring"));
        i.apply(&insert, vec![source, a[1].clone(), a[2].clone()])?;
        loop {
            let (window, next) = match windows {
                Value::Nil => break,
                Value::Cons(ref cell) => {
                    let cell = cell.borrow();
                    (cell.car.clone(), cell.cdr.clone())
                }
                ref other => return Err(i.wrong_type_mut("listp", other)),
            };
            let window_point = Value::Sym(i.q("window-point"));
            let position = i.apply(&window_point, vec![window.clone()])?;
            let equal = Value::Sym(i.q("="));
            if i.apply(&equal, vec![position, point.clone()])?.truthy() {
                let position = Value::Int(cur(i).borrow().point() as i128 + 1);
                let set_point = Value::Sym(i.q("set-window-point"));
                i.apply(&set_point, vec![window, position])?;
            }
            windows = next;
        }
        Ok(Value::Nil)
    })();
    i.restore_excursion_state(excursion);
    result
}

// ---------- file I/O ----------

/// Convert at the native filesystem boundary after filename handlers run.
pub(crate) fn native_path<T: TextSource + ?Sized>(i: &Interp, name: &T) -> Result<std::path::PathBuf, Flow> {
    name.with_text(|name| name.to_os_string().map(std::path::PathBuf::from).map_err(|_| {
        let condition = i.q_soft("file-error").unwrap_or(sym::ERROR);
        i.signal_data(condition, vec![Value::string("Cannot encode file name"), Value::string(name)])
    }))
}

fn want_filename(i: &mut Interp, v: &Value) -> Result<EmacsText, Flow> {
    // GNU `check_filename': STRINGP with no embedded NUL byte, then
    // expanded.  The predicate reported is `filenamep'.
    match v {
        Value::Str(s) => {
            let raw = s.borrow().clone();
            if raw.contains('\0') {
                return Err(i.wrong_type_mut("filenamep", v));
            }
            Ok(expand_file_name_str(i, &raw))
        }
        other => Err(i.wrong_type_mut("stringp", other)),
    }
}

fn expand_and_dir_to_file(i: &mut Interp, original: &Value) -> EvalResult {
    let expanded = want_filename(i, original)?;
    let filename = file_name_argument(original, &expanded);
    if expanded.chars().count() > 1 && expanded.ends_with('/') {
        f_directory_file_name(i, vec![filename].into())
    } else {
        Ok(filename)
    }
}

/// GNU checks the source name first, then the destination (or VISIT).
/// Keep operation filtering and recursive-handler inhibition in one finder.
pub(crate) fn file_handler<T: crate::lisp::text::TextSource + ?Sized>(
    i: &mut Interp,
    operation: &str,
    files: &[&T],
) -> Option<(Value, Value)> {
    let op = Value::Sym(i.intern(operation));
    for file in files {
        if let Some(handler) = crate::buffer::extra::find_file_name_handler(i, file, op.clone()) {
            return Some((handler, op));
        }
    }
    None
}

/// expand-file-name keeps an already expanded filename object, including
/// its properties and byte kind. Preserve that identity at handler calls.
pub(crate) fn file_name_argument<T: crate::lisp::text::TextSource + ?Sized>(original: &Value, expanded: &T) -> Value {
    expanded.with_text(|expanded| match original {
        Value::Str(name) if name.borrow().as_str() == expanded => original.clone(),
        _ => Value::string(expanded),
    })
}

fn copy_target_name<T: TextSource + ?Sized>(from: &T, to: EmacsText) -> EmacsText {
    from.with_text(|from| {
        if to.ends_with('/') {
            let mut result = to.trim_end_matches('/').to_string();
            result.push('/');
            result.push_str(from.rsplit('/').next().unwrap_or(from));
            result
        } else { to }
    })
}

/// Read the Lisp environment without converting its value to Unicode.
pub(crate) fn lisp_getenv<T: TextSource + ?Sized>(i: &Interp, name: &T) -> Option<EmacsText> {
    name.with_text(|name| {
        let pe = i.q_soft("process-environment")?;
        let proc_env = i.symbol_value(pe);
        let mut prefix = name.to_string();
        prefix.push('=');
        let mut hit = None;
        if let Value::Cons(_) = &proc_env {
            proc_env.each_car(|value| {
                if hit.is_none() {
                    if let Value::Str(text) = value {
                        let text = text.borrow();
                        if text.starts_with(&prefix) { hit = Some(text[prefix.len()..].to_string()); }
                    }
                }
            });
        }
        hit
    })
}

/// Look up `~USER''s home directory via getpwnam(3); None when the
/// user does not exist (GNU expand-file-name then treats the name
/// as an ordinary relative path).
#[cfg(unix)]
fn tilde_user_home(user: &EmacsStr) -> Option<EmacsText> {
    let c = std::ffi::CString::new(user.to_external_bytes()).ok()?;
    unsafe {
        let pw = libc::getpwnam(c.as_ptr());
        if pw.is_null() || (*pw).pw_dir.is_null() { return None; }
        Some(EmacsText::from_external_bytes(std::ffi::CStr::from_ptr((*pw).pw_dir).to_bytes()))
    }
}

/// Expand ~, env vars, and make absolute via `default-directory'.
/// Returns None when NAME starts with `~' but is not expandable
/// (`~user' for an unknown user): GNU `expand-file-name' then
/// falls back to joining the name onto the DIRECTORY argument.
fn expand_file_name_raw_opt<T: TextSource + ?Sized>(i: &mut Interp, name: &T) -> Option<EmacsText> {
    name.with_text(|name| {
        let mut text = name.to_string();
        if let Some(rest) = text.strip_prefix('~') {
            let (user, tail) = match rest.find('/') {
                Some(p) => (&rest[..p], &rest[p..]),
                None => (rest, &rest[rest.len()..]),
            };
            let mut home = if user.is_empty() {
                lisp_getenv(i, "HOME")
                    .or_else(|| std::env::var_os("HOME").map(|v| EmacsText::from_os_str(&v)))
                    .unwrap_or_else(|| "/".into())
            } else { tilde_user_home(user)? };
            home.push_str(tail);
            text = home;
        }
        if !text.starts_with('/') { text = directory_join(&default_directory(i), &text); }
        Some(text)
    })
}

fn expand_file_name_opt<T: TextSource + ?Sized>(i: &mut Interp, name: &T) -> Option<EmacsText> {
    expand_file_name_raw_opt(i, name).map(|text| normalize_path(&text))
}

pub fn expand_file_name_str<T: TextSource + ?Sized>(i: &mut Interp, name: &T) -> EmacsText {
    normalize_path(&expand_file_name_raw_str(i, name))
}

fn directory_join(directory: &EmacsStr, name: &EmacsStr) -> EmacsText {
    let mut result = directory.to_string();
    if !directory.ends_with('/') && !directory.is_empty() { result.push('/'); }
    result.push_str(name);
    result
}

/// Preserve physical path components until file-truename follows links.
fn expand_file_name_raw_str<T: TextSource + ?Sized>(i: &mut Interp, name: &T) -> EmacsText {
    name.with_text(|name| expand_file_name_raw_opt(i, name)
        .unwrap_or_else(|| directory_join(&default_directory(i), name)))
}

/// Collapse logical path components while preserving exactly two leading slashes.
fn normalize_path<T: TextSource + ?Sized>(path: &T) -> EmacsText {
    path.with_text(|path| {
        let double_slash = path.starts_with("//") && !path.starts_with("///");
        let body = if double_slash { &path[1..] } else { path };
        let mut parts: Vec<&EmacsStr> = Vec::new();
        for component in body.split('/') {
            if component.is_empty() || component == "." { continue; }
            if component == ".." { parts.pop(); }
            else { parts.push(component); }
        }
        let mut result = EmacsText::from(if double_slash { "//" } else { "/" });
        for (index, component) in parts.into_iter().enumerate() {
            if index != 0 { result.push('/'); }
            result.push_str(component);
        }
        if path.ends_with('/') && !result.ends_with('/') { result.push('/'); }
        result
    })
}

#[cfg(test)]
mod wide_path_tests {
    use super::*;
    use crate::lisp::text::EmacsChar;
    #[test]
    fn normalization_preserves_distinct_non_ascii_components() {
        let source = EmacsText::from_codes([47, 0xf0080, 47, 46, 47, 0x3fff80, 47, 46, 46, 47, 0xd800, 47, 0x110000]).unwrap();
        assert_eq!(normalize_path(&source).codes().collect::<Vec<_>>(), [47, 0xf0080, 47, 0xd800, 47, 0x110000]);
    }
    #[test]
    fn leading_slashes_and_directory_join_remain_logical_text() {
        let mut double = EmacsText::from("//"); double.push(EmacsChar::new(0x200000).unwrap()); double.push_str("//");
        assert_eq!(normalize_path(&double).codes().collect::<Vec<_>>(), [47, 47, 0x200000, 47]);
        let mut triple = EmacsText::from("/"); triple.append(&double);
        assert_eq!(normalize_path(&triple).codes().collect::<Vec<_>>(), [47, 0x200000, 47]);
        let directory = EmacsText::from_codes([47, 0xf0100]).unwrap();
        let name = EmacsText::from_codes([0xd800, 0x3fff80]).unwrap();
        assert_eq!(directory_join(&directory, &name).codes().collect::<Vec<_>>(), [47, 0xf0100, 47, 0xd800, 0x3fff80]);
    }

    #[test]
    fn directory_helpers_and_completion_prefix_distinguish_extended_codes() {
        let literal = EmacsText::from_codes([0xf0080, b'x' as u32]).unwrap();
        let byte8 = EmacsText::from_codes([0x3fff80, b'y' as u32]).unwrap();
        assert!(longest_common_prefix(&[literal.clone(), byte8]).is_empty());
        let directory = file_name_as_directory_str(&literal);
        assert_eq!(directory.codes().collect::<Vec<_>>(), [0xf0080, b'x' as u32, 47]);
        assert_eq!(file_name_directory_str(&directory).unwrap(), directory);
        assert!(completion_prefix_p(&EmacsText::from("a\u{f0080}"), &EmacsText::from("A"), true));
        assert!(!completion_prefix_p(&EmacsText::from_codes([0x3fff80, b'x' as u32]).unwrap(), &literal, false));
    }

    #[cfg(unix)]
    #[test]
    fn native_directory_stream_preserves_private_use_names() {
        let unique = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let directory = std::env::temp_dir().join(format!("remacs-wide-directory-{}-{unique}", std::process::id()));
        std::fs::create_dir(&directory).unwrap();
        struct Cleanup(std::path::PathBuf);
        impl Drop for Cleanup { fn drop(&mut self) { let _ = std::fs::remove_dir_all(&self.0); } }
        let _cleanup = Cleanup(directory.clone());
        let name = EmacsText::from_codes([0xf0080, 0xf0c80, b'x' as u32]).unwrap();
        std::fs::write(directory.join(name.to_os_string().unwrap()), b"probe").unwrap();
        let entries = directory_entry_names(&EmacsText::from_os_str(directory.as_os_str())).unwrap();
        assert!(entries.contains(&name));
        assert!(entries.iter().any(|name| name == "."));
        assert!(entries.iter().any(|name| name == ".."));
        assert!(!entries.iter().any(|name| name.codes().any(|code| (0x3fff80..=0x3fffff).contains(&code))));
    }
}

/// GNU `emacs_wd' (sysdep.c): the logical working directory — $PWD is
/// used when it names the same directory as getcwd (device+inode
/// match), otherwise the physical getcwd path.  Keeps `cd /tmp' (a
/// symlink to /private/tmp on macOS) reporting /tmp.
pub fn emacs_wd() -> String {
    let phys = std::env::current_dir()
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_else(|_| "/".to_string());
    if let Ok(pwd) = std::env::var("PWD") {
        if pwd.starts_with('/') {
            let pm = std::fs::metadata(&pwd);
            let cm = std::fs::metadata(&phys);
            if let (Ok(pm), Ok(cm)) = (pm, cm) {
                use std::os::unix::fs::MetadataExt;
                if pm.dev() == cm.dev() && pm.ino() == cm.ino() {
                    return pwd;
                }
            }
        }
    }
    phys
}
/// The current buffer's `default-directory`.
pub(crate) fn default_directory(i: &Interp) -> EmacsText {
    if let Some(b) = i.current_buffer_ref() {
        let bb = b.borrow();
        if let Some(v) = bb
            .locals
            .get(&i.q_soft("default-directory").unwrap_or(u32::MAX))
        {
            if let Value::Str(s) = v {
                return s.borrow().clone();
            }
        }
    }
    let d = i.symbol_value(i.q_soft("default-directory").unwrap_or(0));
    if let Value::Str(s) = d {
        return s.borrow().clone();
    }
    // Fallback (no buffer-local/global value): logical cwd + slash.
    let mut d = EmacsText::from(emacs_wd());
    if !d.ends_with('/') {
        d.push('/');
    }
    d
}

fn f_file_exists_p(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let p = want_filename(i, &a[0])?;
    if let Some((handler, op)) = file_handler(i, "file-exists-p", &[&p]) {
        return i.apply(&handler, vec![op, file_name_argument(&a[0], &p)]);
    }
    Ok(Value::from_bool(native_path(i, &p)?.exists()))
}
fn f_file_directory_p(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    f_file_directory_p_operation(i, a, "file-directory-p")
}

fn f_file_accessible_directory_p(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    f_file_directory_p_operation(i, a, "file-accessible-directory-p")
}

fn f_file_directory_p_operation(
    i: &mut Interp,
    a: crate::lisp::Argv,
    operation: &str,
) -> EvalResult {
    let filename = expand_and_dir_to_file(i, &a[0])?;
    let p = want_str(i, &filename)?;
    if let Some((handler, op)) = file_handler(i, operation, &[&p]) {
        return i.apply(&handler, vec![op, filename]);
    }
    let path = native_path(i, &p)?;
    let is_dir = path.is_dir();
    if !is_dir || operation == "file-directory-p" {
        return Ok(Value::from_bool(is_dir));
    }
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStrExt;
        let accessible = std::ffi::CString::new(path.as_os_str().as_bytes())
            .ok()
            .is_some_and(|path| unsafe { libc::access(path.as_ptr(), libc::R_OK | libc::X_OK) == 0 });
        return Ok(Value::from_bool(accessible));
    }
    #[cfg(not(unix))]
    Ok(Value::from_bool(is_dir))
}

fn f_file_regular_p(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let filename = expand_and_dir_to_file(i, &a[0])?;
    let p = want_str(i, &filename)?;
    if let Some((handler, op)) = file_handler(i, "file-regular-p", &[&p]) {
        return i.apply(&handler, vec![op, filename]);
    }
    Ok(Value::from_bool(native_path(i, &p)?.is_file()))
}

fn f_file_readable_p(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let p = want_filename(i, &a[0])?;
    if let Some((handler, op)) = file_handler(i, "file-readable-p", &[&p]) {
        return i.apply(&handler, vec![op, file_name_argument(&a[0], &p)]);
    }
    // GNU fileio.c `check_file_access': access(2) R_OK — a file whose
    // mode denies read (EACCES) is nil even though stat() succeeds,
    // and directories count as readable.  Mere existence is not
    // enough (that is what `file-exists-p' reports).
    let path = native_path(i, &p)?;
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStrExt;
        if let Ok(cpath) = std::ffi::CString::new(path.as_os_str().as_bytes()) {
            return Ok(Value::from_bool(
                unsafe { libc::access(cpath.as_ptr(), libc::R_OK) } == 0,
            ));
        }
    }
    Ok(Value::from_bool(
        path.is_dir() || std::fs::File::open(path).is_ok(),
    ))
}
fn f_file_writable_p(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let p = want_filename(i, &a[0])?;
    if let Some((handler, op)) = file_handler(i, "file-writable-p", &[&p]) {
        return i.apply(&handler, vec![op, file_name_argument(&a[0], &p)]);
    }
    let path = native_path(i, &p)?;
    if path.exists() {
        // GNU uses access(2) W_OK, which also works on directories;
        // opening a directory with write access always fails.
        #[cfg(unix)]
        {
            use std::os::unix::ffi::OsStrExt;
            if let Ok(cpath) = std::ffi::CString::new(path.as_os_str().as_bytes()) {
                return Ok(Value::from_bool(
                    unsafe { libc::access(cpath.as_ptr(), libc::W_OK) } == 0,
                ));
            }
        }
        Ok(Value::from_bool(
            std::fs::OpenOptions::new().write(true).open(path).is_ok(),
        ))
    } else {
        // GNU fileio.c: for a nonexistent file, writability means the
        // (nearest existing ancestor) directory is writable — access(2)
        // W_OK on it, not mere existence.
        let dir = path.parent().unwrap_or(&path);
        #[cfg(unix)]
        {
            use std::os::unix::ffi::OsStrExt;
            if let Ok(cpath) = std::ffi::CString::new(dir.as_os_str().as_bytes()) {
                return Ok(Value::from_bool(
                    unsafe { libc::access(cpath.as_ptr(), libc::W_OK) } == 0,
                ));
            }
        }
        Ok(Value::from_bool(dir.exists()))
    }
}
fn f_file_executable_p(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let p = want_filename(i, &a[0])?;
    if let Some((handler, op)) = file_handler(i, "file-executable-p", &[&p]) {
        return i.apply(&handler, vec![op, file_name_argument(&a[0], &p)]);
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Ok(m) = std::fs::metadata(native_path(i, &p)?) {
            return Ok(Value::from_bool(m.permissions().mode() & 0o111 != 0));
        }
    }
    Ok(Value::Nil)
}

fn f_file_symlink_p(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let p = want_filename(i, &a[0])?;
    if let Some((handler, op)) = file_handler(i, "file-symlink-p", &[&p]) {
        return i.apply(&handler, vec![op, file_name_argument(&a[0], &p)]);
    }
    match std::fs::symlink_metadata(native_path(i, &p)?) {
        Ok(m) if m.file_type().is_symlink() => {
            let target = std::fs::read_link(native_path(i, &p)?)
                .map(|t| t.to_string_lossy().into_owned())
                .unwrap_or_default();
            Ok(Value::string(target))
        }
        _ => Ok(Value::Nil),
    }
}

fn f_file_newer_than_file_p(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // C validates both input strings before either directory-name handler.
    for value in &a[..2] {
        if !matches!(value, Value::Str(_)) {
            return Err(i.wrong_type_mut("stringp", value));
        }
    }
    let first = expand_and_dir_to_file(i, &a[0])?;
    let second = expand_and_dir_to_file(i, &a[1])?;
    let p1 = want_str(i, &first)?;
    let p2 = want_str(i, &second)?;
    if let Some((handler, op)) = file_handler(i, "file-newer-than-file-p", &[&p1, &p2]) {
        return i.apply(&handler, vec![op, first, second]);
    }
    let m1 = std::fs::metadata(native_path(i, &p1)?).and_then(|m| m.modified());
    let m2 = std::fs::metadata(native_path(i, &p2)?).and_then(|m| m.modified());
    match (m1, m2) {
        (Ok(t1), Ok(t2)) => Ok(Value::from_bool(t1 > t2)),
        (Ok(_), Err(_)) => Ok(Value::t()),
        _ => Ok(Value::Nil),
    }
}

/// `getpwuid (uid)' -> `pw_name' like GNU's stat_uname.
#[cfg(unix)]
fn uid_to_name(uid: i128) -> Option<String> {
    unsafe {
        let pw = libc::getpwuid(uid as libc::uid_t);
        if pw.is_null() {
            return None;
        }
        Some(
            std::ffi::CStr::from_ptr((*pw).pw_name)
                .to_string_lossy()
                .into_owned(),
        )
    }
}

/// `getgrgid (gid)' -> `gr_name' like GNU's stat_gname.
#[cfg(unix)]
fn gid_to_name(gid: i128) -> Option<String> {
    unsafe {
        let gr = libc::getgrgid(gid as libc::gid_t);
        if gr.is_null() {
            return None;
        }
        Some(
            std::ffi::CStr::from_ptr((*gr).gr_name)
                .to_string_lossy()
                .into_owned(),
        )
    }
}

/// GNU filemodestring (dired.c): file-type letter + rwx with
/// setuid/setgid/sticky overrides, 10 chars.
fn file_mode_string(mode: u32, m: &std::fs::Metadata) -> String {
    use std::os::unix::fs::FileTypeExt;
    let mut s = String::with_capacity(10);
    s.push(if m.is_dir() {
        'd'
    } else if m.file_type().is_symlink() {
        'l'
    } else if m.file_type().is_fifo() {
        'p'
    } else if m.file_type().is_socket() {
        's'
    } else if m.file_type().is_char_device() {
        'c'
    } else if m.file_type().is_block_device() {
        'b'
    } else {
        '-'
    });
    const RWX: &str = "rwxrwxrwx";
    for (k, c) in RWX.chars().enumerate() {
        s.push(if mode & (0o400 >> k) != 0 { c } else { '-' });
    }
    if mode & 0o4000 != 0 {
        let c = if mode & 0o100 != 0 { 's' } else { 'S' };
        s.replace_range(3..4, &c.to_string());
    }
    if mode & 0o2000 != 0 {
        let c = if mode & 0o010 != 0 { 's' } else { 'S' };
        s.replace_range(6..7, &c.to_string());
    }
    if mode & 0o1000 != 0 {
        let c = if mode & 0o001 != 0 { 't' } else { 'T' };
        s.replace_range(9..10, &c.to_string());
    }
    s
}

fn f_file_attributes(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU: a non-filename arg yields nil, not a type error.
    let p = match &a[0] {
        Value::Str(s) => s.borrow().clone(),
        _ => return Ok(Value::Nil),
    };
    // GNU expands FILENAME (tilde, `$HOME', relative paths resolve
    // against default-directory) before the lstat.
    let p = expand_file_name_str(i, &p);
    if let Some((handler, op)) = file_handler(i, "file-attributes", &[&p]) {
        // GNU omits a nil ID-FORMAT for older file name handlers.
        let mut arguments = vec![op, file_name_argument(&a[0], &p)];
        let format = arg(&a, 1);
        if !format.is_nil() {
            arguments.push(format);
        }
        return i.apply(&handler, arguments);
    }
    file_attributes_at(i, &p, &arg(&a, 1))
}

/// Read an already-resolved path without collapsing directory entries
/// such as `alias/.' or `alias/sub/..' across a symbolic link.
fn file_attributes_at<T: TextSource + ?Sized>(i: &mut Interp, p: &T, id_format: &Value) -> EvalResult {
    let native = native_path(i, p)?;
    match std::fs::symlink_metadata(&native) {
        Ok(m) => {
            // GNU order: type nlinks uid gid atime mtime ctime size
            // modes gidchg inode device.
            let to_lisp_time = |i: &mut Interp,
                                r: std::io::Result<std::time::SystemTime>| match r {
                Ok(t) => match t.duration_since(std::time::UNIX_EPOCH) {
                    Ok(d) => crate::lisp::builtins::misc::ns_to_lisp_time_i(
                        i,
                        (d.as_secs() as i128) * 1_000_000_000 + d.subsec_nanos() as i128,
                    ),
                    Err(before_epoch) => {
                        let d = before_epoch.duration();
                        let ns = -((d.as_secs() as i128) * 1_000_000_000
                            + d.subsec_nanos() as i128);
                        crate::lisp::builtins::misc::ns_to_lisp_time_i(i, ns)
                    }
                },
                Err(_) => Value::Nil,
            };
            // GNU Ffile_attributes: uid/gid are numeric when ID-FORMAT is
            // nil or `integer', else the looked-up name (numeric fallback
            // when the lookup fails).  Element 9 (GIDCHG) is always t in
            // this code path.
            let str_ids = match id_format {
                Value::Nil => false,
                Value::Sym(s) => i.symbol_name(*s) != "integer",
                _ => true,
            };
            #[cfg(unix)]
            use std::os::unix::fs::MetadataExt;
            #[cfg(unix)]
            let (uname, gname) = if str_ids {
                (uid_to_name(m.uid() as i128), gid_to_name(m.gid() as i128))
            } else {
                (None, None)
            };
            #[cfg(not(unix))]
            let (uname, gname): (Option<String>, Option<String>) = (None, None);
            let (nlinks, uid, gid, inode, dev, modes, ctime) = {
                #[cfg(unix)]
                {
                    let mode = m.mode();
                    (
                        m.nlink() as i128,
                        match uname {
                            Some(ref n) => Value::string(n.clone()),
                            None => Value::Int(m.uid() as i128),
                        },
                        match gname {
                            Some(ref n) => Value::string(n.clone()),
                            None => Value::Int(m.gid() as i128),
                        },
                        m.ino() as i128,
                        m.dev() as i128,
                        file_mode_string(mode, &m),
                        crate::lisp::builtins::misc::ns_to_lisp_time_i(
                            i,
                            m.ctime() as i128 * 1_000_000_000 + m.ctime_nsec() as i128,
                        ),
                    )
                }
                #[cfg(not(unix))]
                {
                    (
                        1,
                        Value::Int(0),
                        Value::Int(0),
                        0,
                        0,
                        "----------".to_string(),
                        Value::Nil,
                    )
                }
            };
            let car = if m.is_dir() {
                Value::t()
            } else if m.file_type().is_symlink() {
                Value::string(
                    std::fs::read_link(&native)
                        .map(|t| EmacsText::from_os_str(t.as_os_str()))
                        .unwrap_or_default(),
                )
            } else {
                Value::Nil
            };
            Ok(Value::list(vec![
                car,
                Value::Int(nlinks),
                uid,
                gid,
                to_lisp_time(i, m.accessed()),
                to_lisp_time(i, m.modified()),
                ctime,
                Value::Int(m.len() as i128),
                Value::string(modes),
                // GNU Ffile_attributes pushes Qt literally (the flag is
                // meaningful only via file-attributes-more).
                Value::t(),
                Value::Int(inode),
                Value::Int(dev),
            ]))
        }
        Err(_) => Ok(Value::Nil),
    }
}
fn f_file_modes(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let p = want_filename(i, &a[0])?;
    // GNU expand_and_dir_to_file strips trailing slashes before stat,
    // including when FLAG asks for the symlink's own mode.
    let p = if p.chars().all(|c| c == '/') {
        p.as_str()
    } else {
        p.trim_end_matches('/')
    };
    if let Some((handler, op)) = file_handler(i, "file-modes", &[p]) {
        return i.apply(&handler, vec![op, file_name_argument(&a[0], p), arg(&a, 1)]);
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        // GNU treats every non-nil FLAG as `nofollow'.
        let metadata = if arg(&a, 1).truthy() {
            std::fs::symlink_metadata(native_path(i, p)?)
        } else {
            std::fs::metadata(native_path(i, p)?)
        };
        if let Ok(m) = metadata {
            return Ok(Value::Int((m.permissions().mode() & 0o7777) as i128));
        }
    }
    Ok(Value::Nil)
}
fn f_set_file_modes(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU Fset_file_modes: CHECK_FIXNUM (mode) runs before
    // Fexpand_file_name on FILENAME, so a bogus MODE reports fixnump even
    // when FILENAME isn't a string.
    let mode = crate::lisp::builtins::want_fixnum(i, &a[1])? as u32;
    let p = want_filename(i, &a[0])?;
    if let Some((handler, op)) = file_handler(i, "set-file-modes", &[&p]) {
        return i.apply(
            &handler,
            vec![op, file_name_argument(&a[0], &p), a[1].clone(), arg(&a, 2)],
        );
    }
    let nofollow = match a.get(2) {
        Some(Value::Sym(s)) => i.symbol_name(*s) == "nofollow",
        Some(v) => v.truthy(),
        None => false,
    };
    #[cfg(unix)]
    {
        let res = if nofollow {
            // chmod on a symlink itself: fchmodat with
            // AT_SYMLINK_NOFOLLOW.  On Linux this is a no-op (returns
            // EINVAL); macOS supports it.
            let c = std::ffi::CString::new(p.to_external_bytes())
                .map_err(|_| i.signal_data(sym::FILE_ERROR, vec![Value::string("bad filename")]))?;
            unsafe {
                libc::fchmodat(
                    libc::AT_FDCWD,
                    c.as_ptr(),
                    mode as libc::mode_t,
                    libc::AT_SYMLINK_NOFOLLOW,
                )
            }
        } else {
            use std::os::unix::ffi::OsStrExt;
            let c = std::ffi::CString::new(native_path(i, &p)?.as_os_str().as_bytes())
                .map_err(|_| i.signal_data(sym::FILE_ERROR, vec![Value::string("bad filename")]))?;
            unsafe { libc::chmod(c.as_ptr(), mode as libc::mode_t) }
        };
        if res != 0 {
            // GNU report_file_error: ("Doing chmod" strerror FILE),
            // ENOENT → `file-missing'.
            let e = std::io::Error::last_os_error();
            let s = if e.kind() == std::io::ErrorKind::NotFound {
                sym::FILE_MISSING
            } else {
                sym::FILE_ERROR
            };
            return Err(i.signal_data(
                s,
                vec![
                    Value::string("Doing chmod"),
                    Value::string(file_error_string(&e)),
                    Value::string(p.clone()),
                ],
            ));
        }
    }
    Ok(Value::Nil)
}
fn f_file_name_absolute_p(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let s = want_str(i, &a[0])?;
    // GNU's predicate examines the NUL-terminated filename without
    // expanding it.  A named tilde component needs a valid home;
    // bare `~' and `~/' are absolute independently of HOME's value.
    let s = s.split('\0').next().unwrap_or_default();
    let absolute = if s.starts_with('/') {
        true
    } else if let Some(rest) = s.strip_prefix('~') {
        let user = rest.split('/').next().unwrap_or_default();
        user.is_empty() || tilde_user_home(user).is_some_and(|home| home.starts_with('/'))
    } else {
        false
    };
    Ok(Value::from_bool(absolute))
}
/// GNU `Fexpand_file_name(NAME, DIR)': an expandable `~' (or absolute
/// path) in NAME ignores DIR entirely; otherwise NAME joins DIR
/// (itself expanded against default-directory), then `/.'/`/..' are
/// normalized.
fn expand_against_dir<N: TextSource + ?Sized, D: TextSource + ?Sized>(i: &mut Interp, name: &N, dir: &D) -> EmacsText {
    name.with_text(|name| dir.with_text(|dir| {
        if let Some(expanded) = expand_file_name_opt(i, name) {
            if name.starts_with('~') || name.starts_with('/') { return expanded; }
        }
        let directory = expand_file_name_str(i, dir);
        if name.is_empty() { directory.trim_end_matches('/').to_string() }
        else { normalize_path(&directory_join(&directory, name)) }
    }))
}
pub(crate) fn f_expand_file_name(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let name = want_str(i, &a[0])?;
    if name.contains('\0') {
        return Err(i.wrong_type_mut("filenamep", &a[0]));
    }
    let handler_directory = match a.get(1) {
        Some(Value::Str(directory)) => Some(directory.borrow().clone()),
        Some(Value::Nil) | None => Some(default_directory(i)),
        _ => None,
    };
    let mut handler_paths = vec![name.as_str()];
    if let Some(directory) = &handler_directory {
        handler_paths.push(directory);
    }
    if let Some((handler, operation)) = file_handler(i, "expand-file-name", &handler_paths) {
        return i.apply(
            &handler,
            vec![operation, a[0].clone(), a.get(1).cloned().unwrap_or(Value::Nil)],
        );
    }
    // GNU expands DIRECTORY against `default-directory' and appends
    // NAME; an expandable `~' in NAME ignores DIRECTORY entirely, an
    // unexpandable `~user' is treated as relative to DIRECTORY.
    let s = match expand_file_name_opt(i, &name) {
        Some(exp) if name.starts_with('~') || name.starts_with('/') => exp,
        _ => {
            let d = match a.get(1) {
                Some(Value::Str(d)) => expand_file_name_str(i, &d.borrow()),
                Some(Value::Nil) | None => default_directory(i),
                // GNU Fexpand_file_name: a non-string non-nil directory
                // falls back to the filesystem root, not an error.
                Some(_) => EmacsText::from("/"),
            };
            if name.is_empty() {
                // GNU returns DIRECTORY with its directory separator
                // dropped — (expand-file-name "" "/x/") is "/x".
                d.trim_end_matches('/').to_string()
            } else {
                normalize_path(&directory_join(&d, &name))
            }
        }
    };
    Ok(Value::string(s))
}
fn f_locate_file_internal(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // (locate-file-internal FILENAME PATH &optional SUFFIXES MODE)
    // MODE: nil = file-readable-p semantics (dirs skipped), an integer
    // = access(2) mask, otherwise a predicate funcalled per candidate;
    // a directory is accepted only when the predicate returns `dir-ok'.
    let raw_name = want_str(i, &a[0])?;
    // GNU checks `file_name_absolute_p' on the raw name — expanding
    // first would make every relative name absolute and skip the PATH
    // search entirely.
    let name = raw_name;
    let pred = a.get(3).cloned().unwrap_or(Value::Nil);
    let dir_ok = i.q("dir-ok");
    let accept = |i: &mut Interp, cand: &EmacsStr| -> Result<bool, Flow> {
        let md = std::fs::metadata(native_path(i, cand)?);
        match &pred {
            Value::Int(mask) => {
                if md.is_err() {
                    return Ok(false);
                }
                // GNU: directories are never accepted through the access
                // mask — only a predicate returning `dir-ok' admits them.
                if md.as_ref().map(|m| m.is_dir()).unwrap_or(false) {
                    return Ok(false);
                }
                #[cfg(unix)]
                unsafe {
                    let c = std::ffi::CString::new(cand.to_external_bytes()).unwrap_or_default();
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
                    let c = std::ffi::CString::new(cand.to_external_bytes()).unwrap_or_default();
                    return Ok(libc::access(c.as_ptr(), libc::R_OK) == 0
                        && !md.map(|m| m.is_dir()).unwrap_or(false));
                }
                #[allow(unreachable_code)]
                Ok(md.map(|m| !m.is_dir()).unwrap_or(false))
            }
            p => {
                // GNU calls the predicate on every candidate, existing or
                // not — a void/foreign function just signals (the
                // `builtin_dirs' fallback below is gated on Nil/Int for
                // exactly this reason).
                let r = i.apply(&p.clone(), vec![Value::string(cand.to_string())])?;
                Ok(match &md {
                    Ok(m) if m.is_dir() => matches!(&r, Value::Sym(s) if *s == dir_ok),
                    Ok(_) => r.truthy(),
                    Err(_) => false,
                })
            }
        }
    };
    // GNU suffixes decoding: nil → the single empty suffix; a list →
    // its elements (each must be a string); any other value → NO
    // candidates at all (the call answers nil without consulting MODE).
    let mut suffixes = vec![];
    match a.get(2) {
        None | Some(Value::Nil) => suffixes.push(EmacsText::new()),
        Some(v) => match v.list_to_vec() {
            Ok(sufs) => {
                for s in sufs {
                    // GNU signals wrong-type stringp on non-string
                    // elements, not a silent skip.
                    suffixes.push(want_str(i, &s)?);
                }
            }
            Err(_) => {}
        },
    }
    // GNU expand_and_dir_to_file expands each candidate before testing —
    // a leading `~' in NAME (or in a PATH dir) resolves like
    // `expand-file-name' does.  The absolute check runs on the RAW name
    // (`file_name_absolute_p'): expanding a relative NAME against
    // default-directory would make it absolute and skip PATH entirely.
    if name.starts_with('/') {
        let name_exp = expand_file_name_str(i, &name);
        for suf in &suffixes {
            let mut cand = name_exp.clone(); cand.append(suf);
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
        // GNU openp: filename = Fexpand_file_name(str, XCAR(path)) —
        // a leading `~' or `/' in NAME resolves BEFORE the dir is
        // consulted, and still-relative results fall back to
        // default-directory; SUFFIXES append to the expanded basename.
        let base = expand_against_dir(i, &name, &dir);
        let base = if base.starts_with('/') {
            base
        } else {
            expand_file_name_str(i, &base)
        };
        for suf in &suffixes {
            let mut cand = base.clone(); cand.append(suf);
            if accept(i, &cand)? {
                return Ok(Value::string(cand));
            }
        }
    }
    // Bundled libraries live in the exe-relative lisp/ dir — the same
    // fallback `load's `locate' uses — so `require-with-check' and
    // `locate-file' see them even when `load-path' doesn't list it.
    // Only for Nil/Int predicates (existence/readability checks GNU
    // answers with access(2)): a function predicate has no GNU candidates
    // to run on when PATH produced none, and calling it on our extras
    // fabricates calls (e.g. void-function) GNU never made.
    if matches!(pred, Value::Nil | Value::Int(_)) {
        for dir in crate::lisp::load::builtin_dirs() {
            for suf in &suffixes {
                let mut cand = directory_join(&EmacsText::from_os_str(dir.as_os_str()), &name); cand.append(suf);
                if accept(i, &cand)? {
                    return Ok(Value::string(cand));
                }
            }
        }
    }
    Ok(Value::Nil)
}

fn f_file_name_directory(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let s = want_str(i, &a[0])?;
    let operation = Value::Sym(i.q("file-name-directory"));
    if let Some(handler) = crate::buffer::extra::find_file_name_handler(i, &s, operation.clone()) {
        let result = i.apply(&handler, vec![operation, a[0].clone()])?;
        return Ok(if matches!(result, Value::Str(_)) {
            result
        } else {
            Value::Nil
        });
    }
    match s.rfind('/') {
        Some(idx) => Ok(file_name_result_like(i, &a[0], s[..=idx].to_string())),
        None => Ok(Value::Nil),
    }
}
fn f_file_name_nondirectory(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let s = want_str(i, &a[0])?;
    if let Some((handler, operation)) = file_handler(i, "file-name-nondirectory", &[&s]) {
        let result = i.apply(&handler, vec![operation, a[0].clone()])?;
        return Ok(if matches!(result, Value::Str(_)) {
            result
        } else {
            Value::Nil
        });
    }
    match s.rfind('/') {
        Some(idx) => Ok(file_name_result_like(i, &a[0], s[idx + 1..].to_string())),
        None => Ok(file_name_result_like(i, &a[0], s)),
    }
}

fn file_name_result_like(i: &mut Interp, original: &Value, text: EmacsText) -> Value {
    let result = Value::string_ref(text);
    if crate::lisp::builtins::strfn::string_multibyte_p(i, original) {
        i.mark_multibyte(&result);
    } else {
        i.mark_unibyte(&result);
    }
    Value::Str(result)
}
fn call_file_name_lisp(i: &mut Interp, name: &str, args: Vec<Value>) -> EvalResult {
    let function = i.q(name);
    i.apply(&Value::Sym(function), args)
}
fn f_file_name_extension(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let nondirectory = call_file_name_lisp(i, "file-name-nondirectory", vec![a[0].clone()])?;
    let file = call_file_name_lisp(i, "file-name-sans-versions", vec![nondirectory])?;
    let base = want_str(i, &file)?;
    let keep_dot = a.get(1).is_some_and(|v| v.truthy());
    match base.rfind('.') {
        Some(idx) if idx > 0 => crate::lisp::builtins::strfn::f_substring(
            i,
            vec![
                file,
                Value::Int((idx + usize::from(!keep_dot)) as i128),
                Value::Nil,
            ]
            .into(),
        ),
        // With PERIOD, GNU returns an empty string for no extension.
        _ if keep_dot => crate::lisp::builtins::strfn::f_substring(
            i,
            vec![file.clone(), Value::Int(base.chars().count() as i128)].into(),
        ),
        _ => Ok(Value::Nil),
    }
}
fn f_file_name_sans_extension(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let nondirectory = call_file_name_lisp(i, "file-name-nondirectory", vec![a[0].clone()])?;
    let file = call_file_name_lisp(i, "file-name-sans-versions", vec![nondirectory])?;
    let s = want_str(i, &file)?;
    match s.rfind('.') {
        Some(idx) => {
            if idx > 0 {
                let sans_extension = crate::lisp::builtins::strfn::f_substring(
                    i,
                    vec![file, Value::Int(0), Value::Int(idx as i128)].into(),
                )?;
                let directory =
                    call_file_name_lisp(i, "file-name-directory", vec![a[0].clone()])?;
                if directory.truthy() {
                    crate::lisp::builtins::strfn::f_concat(
                        i,
                        vec![directory, sans_extension].into(),
                    )
                } else {
                    Ok(sans_extension)
                }
            } else {
                Ok(a[0].clone())
            }
        }
        None => Ok(a[0].clone()),
    }
}
fn f_file_name_sans_versions(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let s = want_str(i, &a[0])?;
    if let Some((handler, operation)) = file_handler(i, "file-name-sans-versions", &[&s]) {
        return i.apply(&handler, vec![operation, a[0].clone(), arg(&a, 1)]);
    }
    if arg(&a, 1).truthy() {
        return Ok(a[0].clone());
    }
    let regexp_id = i.q("file-name-version-regexp");
    let regexp = i.symbol_value(regexp_id);
    let pattern = crate::lisp::builtins::strfn::f_concat(
        i,
        vec![regexp, Value::string("\\'")].into(),
    )?;
    let string_match = i.q("string-match");
    match i.apply(&Value::Sym(string_match), vec![pattern, a[0].clone()])? {
        Value::Int(start) => crate::lisp::builtins::strfn::f_substring(
            i,
            vec![a[0].clone(), Value::Int(0), Value::Int(start)].into(),
        ),
        _ => Ok(a[0].clone()),
    }
}
fn f_file_name_base(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU: FILENAME nil/omitted → `buffer-file-name' of the current
    // buffer.
    let sv = arg(&a, 0);
    let filename = match &sv {
        Value::Nil => cur(i)
            .borrow()
            .file_name
            .clone()
            .map(Value::string)
            .unwrap_or(Value::Nil),
        _ => sv,
    };
    let nondirectory = call_file_name_lisp(i, "file-name-nondirectory", vec![filename])?;
    call_file_name_lisp(i, "file-name-sans-extension", vec![nondirectory])
}
fn f_file_name_as_directory(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let s = want_str(i, &a[0])?;
    if let Some((handler, operation)) = file_handler(i, "file-name-as-directory", &[&s]) {
        let result = i.apply(&handler, vec![operation, a[0].clone()])?;
        return Ok(if matches!(result, Value::Str(_)) {
            result
        } else {
            Value::Nil
        });
    }
    // GNU: an empty name means the current directory ("./").
    Ok(file_name_result_like(i, &a[0], if s.is_empty() {
        EmacsText::from("./")
    } else if s.ends_with('/') {
        s
    } else {
        file_name_as_directory_str(&s)
    }))
}
fn f_directory_file_name(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let s = want_str(i, &a[0])?;
    if let Some((handler, op)) = file_handler(i, "directory-file-name", &[&s]) {
        let handled = i.apply(&handler, vec![op, a[0].clone()])?;
        if matches!(&handled, Value::Str(_)) {
            return Ok(handled);
        }
        return Err(i.error("Invalid handler in `file-name-handler-alist'"));
    }
    // GNU (fileio.c): strip trailing slashes, but a name made solely
    // of slashes is already a directory name and stays unchanged
    // (`directory-file-name' of "/" is "/", of "//" is "//").
    if s.chars().all(|c| c == '/') {
        return Ok(file_name_result_like(
            i,
            &a[0],
            if s.len() > 2 { ("/".to_string()).into() } else { s },
        ));
    }
    Ok(file_name_result_like(
        i,
        &a[0],
        s.trim_end_matches('/').to_string(),
    ))
}

fn f_file_name_concat(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU files.el: nil components count as empty strings; empty
    // leading components don't force a leading slash.
    let mut multibyte = crate::lisp::builtins::strfn::string_multibyte_p(i, &a[0]);
    let mut out = match &a[0] {
        Value::Nil => EmacsText::new(),
        v => want_str(i, v)?,
    };
    for v in &a[1..] {
        let part = match v {
            Value::Nil => continue,
            _ => want_str(i, v)?,
        };
        multibyte |= crate::lisp::builtins::strfn::string_multibyte_p(i, v);
        if part.is_empty() {
            continue;
        }
        if !out.is_empty() && !out.ends_with('/') {
            out.push('/');
        }
        out.push_str(&part);
    }
    let result = Value::string_ref(out);
    if multibyte {
        i.mark_multibyte(&result);
    } else {
        i.mark_unibyte(&result);
    }
    Ok(Value::Str(result))
}
fn f_file_relative_name(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU files.el's string algorithm, verbatim:
    //   directory ← file-name-as-directory(expand-file-name(or dir dd))
    //   filename  ← expand-file-name(filename)
    //   loop until (string-prefix-p directory filename-dir) or (string-prefix-p
    //   directory filename), walking directory up and growing ancestor "..".
    // Note the check order: DIRECTORY is expanded (and CHECK_STRING'd)
    // before FILENAME.
    let dir_input = match a.get(1) {
        Some(v) if v.truthy() => v.clone(),
        _ => Value::string(default_directory(i)),
    };
    let dir_exp = call_file_name_lisp(i, "expand-file-name", vec![dir_input])?;
    let dir_value = call_file_name_lisp(i, "file-name-as-directory", vec![dir_exp])?;
    let mut dir = want_str(i, &dir_value)?;
    let filename_value = call_file_name_lisp(i, "expand-file-name", vec![a[0].clone()])?;
    let filename = want_str(i, &filename_value)?;
    let fold = f_file_name_case_insensitive_p_impl(i, &filename);
    let fdir_value = call_file_name_lisp(
        i,
        "file-name-as-directory",
        vec![filename_value.clone()],
    )?;
    let fdir = want_str(i, &fdir_value)?;
    let mut ancestor = ".".to_string();
    while !(string_prefix_p(i, &dir, &fdir, fold)?
        || string_prefix_p(i, &dir, &filename, fold)?)
    {
        let prefix = Value::string(&dir[..dir.len().saturating_sub(1)]);
        dir = match call_file_name_lisp(i, "file-name-directory", vec![prefix])? {
            Value::Str(directory) => directory.borrow().clone(),
            _ => String::new().into(),
        };
        ancestor = if ancestor == "." {
            "..".to_string()
        } else {
            format!("../{}", ancestor)
        };
    }
    if string_prefix_p(i, &dir, &filename, fold)? {
        let rest = &filename[dir.len()..];
        if ancestor == "." && !rest.is_empty() {
            return Ok(Value::string(rest));
        }
        Ok(Value::string(format!(
            "{}{}",
            file_name_as_directory_str(&ancestor),
            rest
        )))
    } else {
        Ok(Value::string(ancestor))
    }
}

/// `expand-file-name' on an already-decoerced string, honoring the
/// "" → drop-separator rule the subr applies ("" "" is "", not "./").
fn expand_string_arg<T: TextSource + ?Sized>(i: &mut Interp, text: &T) -> Result<EmacsText, Flow> {
    text.with_text(|text| match expand_file_name_opt(i, text) {
        Some(expanded) if text.starts_with('~') || text.starts_with('/') => Ok(expanded),
        _ if text.is_empty() => Ok(default_directory(i).trim_end_matches('/').to_string()),
        _ => Ok(normalize_path(&directory_join(&default_directory(i), text))),
    })
}

/// `(file-name-as-directory S)': append '/' unless S already ends in
/// one; empty S is "./".
fn file_name_as_directory_str<T: TextSource + ?Sized>(text: &T) -> EmacsText {
    text.with_text(|text| {
        if text.is_empty() { return EmacsText::from("./"); }
        let mut result = text.to_string();
        if !text.ends_with('/') { result.push('/'); }
        result
    })
}

fn file_name_directory_str<T: TextSource + ?Sized>(text: &T) -> Option<EmacsText> {
    text.with_text(|text| text.rfind('/').map(|position| text[..=position].to_string()))
}

/// `(string-prefix-p PREFIX STRING IGNORE-CASE)' on raw bytes; the
/// case-fold path folds ASCII only (paths are byte strings, matching
/// compare-strings' result for ASCII names).
fn string_prefix_p(
    i: &mut Interp,
    prefix: &EmacsText,
    s: &EmacsText,
    ignore_case: bool,
) -> Result<bool, Flow> {
    let function = i.q("string-prefix-p");
    let result = i.apply(
        &Value::Sym(function),
        vec![
            Value::string(prefix),
            Value::string(s),
            Value::from_bool(ignore_case),
        ],
    )?;
    Ok(result.truthy())
}

fn f_file_name_case_insensitive_p_impl<T: TextSource + ?Sized>(i: &mut Interp, path: &T) -> bool {
    path.with_text(|path| match crate::lisp::builtins::misc::f_file_name_case_insensitive_p(i, vec![Value::string(path)].into()) {
        Ok(value) => value.truthy(), Err(_) => false,
    })
}

fn f_abbreviate_file_name(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let name = want_str(i, &a[0])?;
    // GNU fileio.c: each `directory-abbrev-alist' entry whose regexp
    // matches FILENAME substitutes its expansion for the matched
    // portion (first match wins); otherwise a $HOME prefix becomes ~.
    let daa = i
        .intern_soft("directory-abbrev-alist")
        .map(|id| i.symbol_value(id))
        .unwrap_or(Value::Nil);
    let mut entries = Vec::new();
    daa.each_car(|v| {
        if let Value::Cons(c) = v {
            let b = c.borrow();
            entries.push((b.car.clone(), b.cdr.clone()));
        }
    });
    let name_chars: Vec<crate::lisp::text::EmacsChar> = name.chars().collect();
    for (k, val) in &entries {
        let (Value::Str(pat), Value::Str(rep)) = (k, val) else {
            continue;
        };
        let Ok(re) = crate::lisp::regexp::compile_case(&pat.borrow(), false) else {
            continue;
        };
        let syn = re_syntax(i);
        let cat = re_category(i);
        if let Some((cs, ce)) = crate::lisp::regexp::search(
            &re,
            &name_chars,
            0,
            &syn,
            &cat,
            crate::lisp::regexp::NO_POINT,
        ) {
            // Replace chars [cs, ce) with the expansion.
            let mut out: EmacsText = name_chars[..cs].iter().collect();
            out.push_str(&rep.borrow());
            out.extend(name_chars[ce..].iter().copied());
            return Ok(Value::string(out));
        }
    }
    let home = lisp_getenv(i, "HOME")
        .or_else(|| std::env::var_os("HOME").map(|v| EmacsText::from_os_str(&v)))
        .unwrap_or_default();
    if !home.is_empty() && name.starts_with(&home) {
        let mut result = EmacsText::from("~"); result.push_str(&name[home.len()..]);
        Ok(Value::string(result))
    } else {
        Ok(Value::string(name))
    }
}
/// GNU env-var substitution in filenames: `$VAR' and `${VAR}' expand to the
/// variable's value, `$$' becomes `$', and unset variables stay literal.
/// Values come from `process-environment' (what GNU's egetenv reads).
fn substitute_env_vars(i: &Interp, s: &EmacsStr) -> EmacsText {
    let mut out = EmacsText::new();
    let mut cs = s.chars().peekable();
    while let Some(c) = cs.next() {
        if c != '$' {
            out.push(c);
            continue;
        }
        match cs.peek().and_then(|c| c.unicode()) {
            Some('$') => {
                cs.next();
                out.push('$');
            }
            Some('{') => {
                cs.next();
                let mut var = EmacsText::new();
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
                    match lisp_getenv(i, &var) {
                        Some(v) => out.push_str(&v),
                        None => {
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
                let mut var = EmacsText::new();
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
                    match lisp_getenv(i, &var) {
                        Some(v) => out.push_str(&v),
                        None => {
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

fn f_substitute_in_file_name(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let s = want_str(i, &a[0])?;
    // GNU fileio.c drops the `/:' quotation prefix first.
    let s = s.strip_prefix("/:").unwrap_or(&s).to_string();
    let out = substitute_env_vars(i, &s);
    // A `//' or `/~X' (X = `/', end of string, or a real user name)
    // discards the text before it; the LAST such marker wins, so
    // "x//a/~/b" yields "~/b".
    let mut cut: Option<usize> = None;
    let characters: Vec<_> = out.chars().collect();
    for idx in 0..characters.len() {
        if characters[idx] != '/' {
            continue;
        }
        match characters.get(idx + 1).and_then(|c| c.unicode()) {
            Some('/') => cut = Some(idx + 1),
            Some('~') => {
                let rest = &out[idx + 2..];
                let (user, _) = match rest.find('/') {
                    Some(p) => (&rest[..p], ()),
                    None => (rest, ()),
                };
                if user.is_empty() || tilde_user_home(user).is_some() {
                    // GNU drops the '/' too: "/a/~/b" → "~/b"
                    // (unlike "//", where the second slash stays).
                    cut = Some(idx + 1);
                }
            }
            _ => {}
        }
    }
    if let Some(idx) = cut {
        return Ok(Value::string(out[idx..].to_string()));
    }
    Ok(Value::string(out))
}

/// Read the actual directory stream, including dot entries and its native
/// order. std::fs::read_dir deliberately hides `.' and `..', so it cannot
/// reproduce GNU's COUNT selection or reversed-readdir completion order.
fn directory_entry_names<T: TextSource + ?Sized>(dir: &T) -> std::io::Result<Vec<EmacsText>> {
    dir.with_text(|dir| {
    #[cfg(unix)]
    {
        use std::ffi::{CStr, CString};
        struct Directory(*mut libc::DIR);
        impl Drop for Directory {
            fn drop(&mut self) {
                unsafe { libc::closedir(self.0) };
            }
        }
        let path = CString::new(dir.to_external_bytes())?;
        let stream = unsafe { libc::opendir(path.as_ptr()) };
        if stream.is_null() {
            return Err(std::io::Error::last_os_error());
        }
        let stream = Directory(stream);
        let mut names = Vec::new();
        loop {
            let entry = unsafe { libc::readdir(stream.0) };
            if entry.is_null() {
                break;
            }
            let name = unsafe { CStr::from_ptr((*entry).d_name.as_ptr()) };
            names.push(EmacsText::from_external_bytes(name.to_bytes()));
        }
        Ok(names)
    }
    #[cfg(not(unix))]
    {
        let entries = std::fs::read_dir(dir.to_os_string().map_err(|_| std::io::Error::new(std::io::ErrorKind::InvalidInput, "Cannot encode directory name"))?)?;
        let mut names = vec![".".into(), "..".into()];
        for entry in entries {
            names.push(EmacsText::from_os_str(&entry?.file_name()));
        }
        Ok(names)
    }

    })
}

fn directory_names<T: TextSource + ?Sized>(i: &mut Interp, dir: &T) -> Result<Vec<EmacsText>, Flow> {
    dir.with_text(|dir| {
    directory_entry_names(dir).map_err(|error| {
        let symbol = if error.kind() == std::io::ErrorKind::NotFound {
            sym::FILE_MISSING
        } else {
            sym::FILE_ERROR
        };
        i.signal_data(
            symbol,
            vec![
                Value::string("Opening directory"),
                Value::string(file_error_string(&error)),
                Value::string(dir),
            ],
        )
    })

    })
}

fn f_directory_files(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let dir = want_filename(i, &a[0])?;
    // GNU Fdirectory_files validates COUNT (arg5) before MATCH:
    // `wholenump' wins over `stringp' on a mixed-invalid call.
    let count: Option<usize> = match a.get(4) {
        Some(Value::Nil) | None => None,
        Some(Value::Int(n)) if *n >= 0 => Some(*n as usize),
        Some(other) => return Err(i.wrong_type_mut("wholenump", other)),
    };
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
    if let Some((handler, op)) = file_handler(i, "directory-files", &[&dir]) {
        let mut handler_args = vec![op, file_name_argument(&a[0], &dir)];
        handler_args.extend(a[1..].iter().cloned());
        return i.apply(&handler, handler_args);
    }
    let base = if dir.ends_with('/') {
        dir.clone()
    } else {
        file_name_as_directory_str(&dir)
    };
    let entries = directory_names(i, &dir)?;
    let syn = crate::editor::re_syntax(i);
    let matches = |name: &EmacsStr| -> bool {
        match &re {
            Some(r) => {
                let chars: Vec<crate::lisp::text::EmacsChar> = name.chars().collect();
                crate::lisp::regexp::search(
                    r,
                    &chars,
                    0,
                    &syn,
                    &crate::editor::re_category(i),
                    crate::lisp::regexp::NO_POINT,
                )
                .is_some()
            }
            None => true,
        }
    };
    let mut names: Vec<EmacsText> = entries
        .into_iter()
        .filter(|name| matches(name))
        .map(|name| if full { directory_join(&base, &name) } else { name })
        .collect();
    // COUNT limits matching entries in stream order, before consing
    // reverses the selection and before the optional final sort.
    if let Some(c) = count {
        names.truncate(c);
    }
    if nosort {
        names.reverse();
    } else {
        names.sort();
    }
    Ok(Value::list(names.into_iter().map(Value::string).collect()))
}

fn f_directory_files_and_attributes(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let dir = want_filename(i, &a[0])?;
    if let Some((handler, op)) = file_handler(i, "directory-files-and-attributes", &[&dir]) {
        let mut handler_args = vec![op, file_name_argument(&a[0], &dir)];
        handler_args.extend(a[1..].iter().cloned());
        return i.apply(&handler, handler_args);
    }
    let files = f_directory_files(
        i,
        vec![a[0].clone(), arg(&a, 1), arg(&a, 2), arg(&a, 3), arg(&a, 5)].into(),
    )?;
    let full = a.get(1).map(|v| v.truthy()).unwrap_or(false);
    let items = files.list_to_vec().unwrap_or_default();
    let mut out = Vec::with_capacity(items.len());
    for n in items {
        let attrs = match &n {
            Value::Str(s) => {
                let name = s.borrow().clone();
                let path = if full {
                    name.clone()
                } else {
                    directory_join(&dir, &name)
                };
                // The directory has already been expanded.  Keep its
                // entry suffix for the filesystem to resolve, like
                // GNU's directory-relative attribute lookup.
                file_attributes_at(i, &path, &arg(&a, 4))?
            }
            _ => Value::Nil,
        };
        out.push(Value::cons(n, attrs));
    }
    Ok(Value::list(out))
}

/// Collect DIR's entries matching prefix FILE, GNU-style: `.` and
/// `..' are offered as candidates too, and directories carry a
/// trailing `/'.  Errors like GNU's directory_files: ENOENT maps to
/// `file-missing' with ("Opening directory" strerror DIR) data.
fn dir_completion_names<F: TextSource + ?Sized, D: TextSource + ?Sized>(i: &mut Interp, file: &F, dir: &D) -> Result<Vec<EmacsText>, Flow> {
    file.with_text(|file| dir.with_text(|dir| {
        let mut matches = Vec::new();
        let directory = native_path(i, dir)?;
        for mut name in directory_names(i, dir)? {
            if !name.starts_with(file) { continue; }
            if directory.join(native_path(i, &name)?).is_dir() { name.push('/'); }
            matches.push(name);
        }
        Ok(matches)
    }))
}

fn f_file_name_completion(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let dir = want_filename(i, &a[1])?;
    let file = want_str(i, &a[0])?;
    if let Some((handler, op)) = file_handler(i, "file-name-completion", &[&dir]) {
        let mut handler_args = vec![op, a[0].clone(), file_name_argument(&a[1], &dir)];
        handler_args.extend(a[2..].iter().cloned());
        return i.apply(&handler, handler_args);
    }
    let mut matches = dir_completion_names(i, &file, &dir)?;
    // GNU excludes the trivial directory entries once a real candidate
    // exists. all-completions, in contrast, retains them unconditionally.
    if matches.iter().any(|name| name != "./" && name != "../") {
        matches.retain(|name| name != "./" && name != "../");
    }
    // GNU calls PREDICATE on each candidate with `default-directory'
    // dynamically bound to DIRECTORY, so predicates like
    // `file-directory-p' resolve names relative to it.
    if let Some(pred) = a.get(2) {
        if pred.truthy() {
            let dd = i.q("default-directory");
            let mark = i.specbind_depth();
            i.specbind(dd, Value::string(dir.clone()))?;
            let mut kept = Vec::new();
            let mut res: Result<(), Flow> = Ok(());
            for name in matches {
                match i.apply(pred, vec![Value::string(name.clone())]) {
                    Ok(v) if v.truthy() => kept.push(name),
                    Ok(_) => {}
                    Err(e) => {
                        res = Err(e);
                        break;
                    }
                }
            }
            let _ = i.unbind(i.specbind_depth() - mark);
            res?;
            matches = kept;
        }
    }
    if matches.len() == 1 && matches[0] == file {
        return Ok(Value::t());
    }
    // longest common prefix
    if matches.is_empty() {
        return Ok(Value::Nil);
    }
    // Only the sole exact match above returns t.  An ambiguous
    // completion is a string even when it cannot extend FILE.
    Ok(Value::string(longest_common_prefix(&matches)))
}

fn longest_common_prefix(xs: &[EmacsText]) -> EmacsText {
    if xs.is_empty() {
        return EmacsText::new();
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

fn f_file_name_all_completions(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU checks DIRECTORY first, then FILE; nil
    // DIRECTORY means `default-directory'.
    let dir = want_filename(i, &a[1])?;
    let file = want_str(i, &a[0])?;
    if let Some((handler, op)) = file_handler(i, "file-name-all-completions", &[&dir]) {
        return i.apply(
            &handler,
            vec![op, a[0].clone(), file_name_argument(&a[1], &dir)],
        );
    }
    let matches = dir_completion_names(i, &file, &dir)?;
    Ok(Value::list(
        matches.into_iter().rev().map(Value::string).collect(),
    ))
}

/// Signal the GNU-matching error for a failed mkdir: EEXIST maps to
/// strerror(3) text for an io::Error — GNU reports errno strings
/// like "No such file or directory" without Rust's "(os error N)"
/// suffix.
pub(crate) fn file_error_string(e: &std::io::Error) -> String {
    if let Some(code) = e.raw_os_error() {
        unsafe {
            return std::ffi::CStr::from_ptr(libc::strerror(code))
                .to_string_lossy()
                .into_owned();
        }
    }
    e.to_string()
}

/// GNU fileio.c `report_file_errno': signals
///   (file-missing|permission-denied|file-error STRING strerror . NAMES)
/// where the errno picks the symbol — ENOENT → `file-missing',
/// EACCES → `permission-denied', everything else → `file-error' —
/// except EEXIST, which signals `file-already-exists' with data
/// (strerror . NAMES) and NO action string (get_file_errno_data).
pub(crate) fn report_file_errno(
    i: &mut Interp,
    string: &str,
    names: Vec<Value>,
    e: &std::io::Error,
) -> Flow {
    let errstring = file_error_string(e);
    match e.raw_os_error() {
        Some(libc::EEXIST) => {
            let s = i.q("file-already-exists");
            let mut d = Vec::with_capacity(names.len() + 1);
            d.push(Value::string(errstring));
            d.extend(names);
            i.signal_data(s, d)
        }
        code => {
            let s = match code {
                Some(libc::ENOENT) => sym::FILE_MISSING,
                Some(libc::EACCES) => i.q("permission-denied"),
                _ => sym::FILE_ERROR,
            };
            let mut d = Vec::with_capacity(names.len() + 2);
            d.push(Value::string(string));
            d.push(Value::string(errstring));
            d.extend(names);
            i.signal_data(s, d)
        }
    }
}

/// Signal the GNU-matching error for a failed mkdir: EEXIST maps to
/// `file-already-exists` with "File exists" data, everything else to
/// `file-error' ("Creating directory" strerror DIR).
fn signal_mkdir_error(i: &mut Interp, dir: &EmacsStr, e: std::io::Error) -> Flow {
    // GNU `report_file_errno' maps errno → symbol: ENOENT is
    // `file-missing', EACCES/EPERM are `permission-denied', EEXIST is
    // `file-already-exists' (data "File exists" DIR); the rest stay
    // `file-error' ("Creating directory" strerror DIR).
    use std::io::ErrorKind::*;
    match e.kind() {
        AlreadyExists => {
            let s = i.q("file-already-exists");
            i.signal_data(
                s,
                vec![Value::string("File exists"), Value::string(dir.to_string())],
            )
        }
        NotFound => i.signal_data(
            sym::FILE_MISSING,
            vec![
                Value::string("Creating directory"),
                Value::string(file_error_string(&e)),
                Value::string(dir.to_string()),
            ],
        ),
        PermissionDenied => {
            let s = i.q("permission-denied");
            i.signal_data(
                s,
                vec![
                    Value::string("Creating directory"),
                    Value::string(file_error_string(&e)),
                    Value::string(dir.to_string()),
                ],
            )
        }
        _ => i.signal_data(
            sym::FILE_ERROR,
            vec![
                Value::string("Creating directory"),
                Value::string(file_error_string(&e)),
                Value::string(dir.to_string()),
            ],
        ),
    }
}

fn f_make_directory(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let dir = want_filename(i, &a[0])?;
    let parents = a.get(1).map(|v| v.truthy()).unwrap_or(false);
    if parents {
        // GNU make-directory with PARENTS returns t (not nil) when
        // the directory already exists.
        if native_path(i, &dir)?.is_dir() {
            return Ok(Value::t());
        }
        // GNU walks from the leaf up collecting missing parents, then
        // creates them outermost-first with `make-directory-internal' —
        // a failure reports the component mkdir failed on (e.g. "/proc"
        // for "/proc/x"), not the leaf.
        let mut comps: Vec<EmacsText> = Vec::new();
        let mut p = dir.clone();
        while !native_path(i, &p)?.is_dir() {
            let parent = match native_path(i, &p)?.parent() {
                Some(par) => EmacsText::from_os_str(par.as_os_str()),
                None => break,
            };
            if parent == p {
                break;
            }
            comps.push(p.clone());
            p = parent;
        }
        for comp in comps.iter().rev() {
            match std::fs::create_dir(native_path(i, comp)?) {
                Ok(()) => {}
                Err(e)
                    if e.kind() == std::io::ErrorKind::AlreadyExists
                        && native_path(i, comp)?.is_dir() =>
                {
                    // mkdir EEXIST on a real dir is ignored by GNU's
                    // `files--ensure-directory' fallback.
                }
                Err(e) => return Err(signal_mkdir_error(i, comp, e)),
            }
        }
        return Ok(Value::Nil);
    }
    match std::fs::create_dir(native_path(i, &dir)?) {
        Ok(()) => Ok(Value::Nil),
        Err(e) => Err(signal_mkdir_error(i, &dir, e)),
    }
}
fn f_make_directory_internal(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let dir = want_filename(i, &a[0])?;
    match std::fs::create_dir(native_path(i, &dir)?) {
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

fn f_make_temp_name(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU's make-temp-name does not expand the prefix.
    let prefix = want_str(i, &a[0])?;
    for _ in 0..64 {
        let mut name = prefix.clone(); name.push_str(temp_name_seed());
        if !native_path(i, &name)?.exists() {
            return Ok(Value::string(name));
        }
    }
    let mut name = prefix; name.push_str(temp_name_seed());
    Ok(Value::string(name))
}

fn f_make_temp_file(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
    // GNU expands relative prefixes under `temporary-file-directory`,
    // GNU's Lisp `make-temp-file' signals SEQUENCEP on a non-string
    // prefix (its `(zerop (length prefix))' hits length first).
    let raw = match &a[0] {
        Value::Str(s) => s.borrow().clone(),
        other => return Err(i.wrong_type_mut("sequencep", other)),
    };
    let temporary = i.q("temporary-file-directory");
    let directory = i.symbol_value(temporary);
    let prefix = if raw.is_empty() || raw == "." || raw == ".." {
        // GNU keeps these literal prefixes instead of normalizing them.
        let directory = f_file_name_as_directory(i, vec![directory].into())?;
        let mut prefix = want_str(i, &directory)?; prefix.append(&raw); prefix
    } else {
        let expanded = f_expand_file_name(i, vec![a[0].clone(), directory].into())?;
        want_str(i, &expanded)?
    };
    let dir_flag = a.get(1).map(|v| v.truthy()).unwrap_or(false);
    let suffix = match a.get(2) {
        Some(v) if v.truthy() => want_str(i, v)?,
        _ => EmacsText::new(),
    };
    for _ in 0..64 {
        let mut name = prefix.clone(); name.push_str(temp_name_seed()); name.append(&suffix);
        let native = native_path(i, &name)?;
        let r = if dir_flag {
            std::fs::DirBuilder::new()
                .mode(0o700)
                .create(&native)
                .map(|_| None)
        } else {
            std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(0o600)
                .open(&native)
                .map(Some)
        };
        match r {
            Ok(mut output) => {
                if let Some(text) = a.get(3) {
                    if matches!(text, Value::Str(string) if !string.borrow().is_empty()) {
                        let bytes = temp_file_text_bytes(i, &name, text)?;
                        if let Some(file) = &mut output {
                            use std::io::Write;
                            if let Err(error) = file.write_all(&bytes) {
                                return Err(report_file_errno(
                                    i,
                                    "Write error",
                                    vec![Value::string(name)],
                                    &error,
                                ));
                            }
                        } else {
                            let _ = std::fs::write(&native, bytes);
                        }
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

fn f_file_local_copy(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU files.el dispatches on the original, unexpanded FILE.
    let name = want_str(i, &a[0])?;
    if let Some((handler, op)) = file_handler(i, "file-local-copy", &[&name]) {
        return i.apply(&handler, vec![op, a[0].clone()]);
    }
    Ok(Value::Nil)
}

fn f_file_in_directory_p(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let file = want_filename(i, &a[0])?;
    let dir = want_filename(i, &a[1])?;
    // GNU files.el: "DIR must be an existing directory, otherwise
    // the function returns nil."
    if !native_path(i, &dir)?.is_dir() {
        return Ok(Value::Nil);
    }
    let canon = |p: &EmacsStr| -> Result<EmacsText, Flow> {
        Ok(std::fs::canonicalize(native_path(i, p)?)
            .map(|path| EmacsText::from_os_str(path.as_os_str()))
            .unwrap_or_else(|_| p.to_string()))
    };
    let f = canon(&file)?;
    let d = canon(&dir)?;
    // GNU: "a directory is considered to be a parent of itself".
    if f == d {
        return Ok(Value::t());
    }
    let d = file_name_as_directory_str(&d);
    Ok(if f.starts_with(&d) {
        Value::t()
    } else {
        Value::Nil
    })
}

fn f_delete_directory(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let dir = want_filename(i, &a[0])?;
    // GNU files.el `delete-directory': (directory-file-name
    // (expand-file-name DIRECTORY)) — strip any trailing slash.
    let dir = dir.trim_end_matches('/').to_string();
    let dir = if dir.is_empty() { EmacsText::from("/") } else { dir };
    let recursive = a.get(1).map(|v| v.truthy()).unwrap_or(false);
    // GNU decides between "check the dir first" and
    // `delete-directory-internal' like this:
    //   (when (or (not recursive) (file-symlink-p dir)
    //             (let ((files (files--force t #'directory-files dir ...)))
    //               (when (listp files) (delete each))
    //               (listp files)))
    //     (files--force recursive #'delete-directory-internal dir))
    // So with RECURSIVE non-nil a missing directory short-circuits the
    // whole `when' and returns nil — the inner files--force never runs.
    let do_internal = if !recursive {
        true
    } else if std::fs::symlink_metadata(native_path(i, &dir)?)
        .map(|m| m.file_type().is_symlink())
        .unwrap_or(false)
    {
        true
    } else {
        match std::fs::read_dir(native_path(i, &dir)?) {
            Ok(rd) => {
                // GNU deletes each entry via (file-directory-p && !symlink)
                // → recursive delete-directory, else delete-file.  On a
                // non-empty dir remove_dir_all is equivalent.
                drop(rd);
                match std::fs::remove_dir_all(native_path(i, &dir)?) {
                    Ok(()) => return Ok(Value::Nil),
                    Err(e) => {
                        let s = if e.kind() == std::io::ErrorKind::NotFound {
                            sym::FILE_MISSING
                        } else {
                            sym::FILE_ERROR
                        };
                        return Err(i.signal_data(
                            s,
                            vec![
                                Value::string("Removing directory"),
                                Value::string(file_error_string(&e)),
                                Value::string(dir.clone()),
                            ],
                        ));
                    }
                }
            }
            // directory-files failed → the `when' body is nil and
            // delete-directory-internal is never called → nil.
            Err(_) => false,
        }
    };
    if !do_internal {
        return Ok(Value::Nil);
    }
    // delete-directory-internal — but under (files--force recursive ...):
    // file-missing is swallowed when RECURSIVE is non-nil.
    match std::fs::remove_dir(native_path(i, &dir)?) {
        Ok(()) => Ok(Value::Nil),
        // GNU report_file_error: ("Removing directory" strerror DIR),
        // with ENOENT mapping to `file-missing'.
        Err(e) => {
            if e.kind() == std::io::ErrorKind::NotFound && recursive {
                return Ok(Value::Nil);
            }
            let s = if e.kind() == std::io::ErrorKind::NotFound {
                sym::FILE_MISSING
            } else {
                sym::FILE_ERROR
            };
            Err(i.signal_data(
                s,
                vec![
                    Value::string("Removing directory"),
                    Value::string(file_error_string(&e)),
                    Value::string(dir.clone()),
                ],
            ))
        }
    }
}
fn f_delete_file(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let p = want_filename(i, &a[0])?;
    // GNU files.el delete-file: (file-directory-p) non-symlink dirs
    // signal before any syscall, carrying the UNEXPANDED filename.
    if let Some(Value::Str(name)) = a.first() {
        let is_dir = std::fs::metadata(native_path(i, &p)?).map(|m| m.is_dir()).unwrap_or(false);
        let is_link = std::fs::symlink_metadata(native_path(i, &p)?)
            .map(|m| m.file_type().is_symlink())
            .unwrap_or(false);
        if is_dir && !is_link {
            return Err(i.signal_data(
                sym::FILE_ERROR,
                vec![
                    Value::string("Removing old name: is a directory"),
                    Value::Str(name.clone()),
                ],
            ));
        }
    }
    if let Some((handler, op)) = file_handler(i, "delete-file", &[&p]) {
        return i.apply(
            &handler,
            vec![op, file_name_argument(&a[0], &p), arg(&a, 1)],
        );
    }
    match std::fs::remove_file(native_path(i, &p)?) {
        Ok(()) => Ok(Value::Nil),
        // GNU's delete-file quietly returns nil when the file is missing.
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Value::Nil),
        // GNU report_file_error: (file-error "Removing old name"
        // strerror FILE) with the EXPANDED name.
        Err(e) => Err(i.signal_data(
            sym::FILE_ERROR,
            vec![
                Value::string("Removing old name"),
                Value::string(file_error_string(&e)),
                Value::string(p),
            ],
        )),
    }
}

// GNU internals: `delete-file-internal' quietly returns nil when the file
// is missing; `delete-directory-internal' signals file-missing.
fn f_delete_file_internal(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let p = want_filename(i, &a[0])?;
    match std::fs::remove_file(native_path(i, &p)?) {
        Ok(()) => Ok(Value::Nil),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Value::Nil),
        Err(e) => Err(i.signal_data(
            sym::FILE_ERROR,
            vec![
                Value::string("Removing old name"),
                Value::string(file_error_string(&e)),
                Value::string(p),
            ],
        )),
    }
}

fn f_delete_directory_internal(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let dir = want_filename(i, &a[0])?;
    match std::fs::remove_dir(native_path(i, &dir)?) {
        Ok(()) => Ok(Value::Nil),
        // GNU report_file_error on rmdir: ("Removing directory" strerror
        // EXPANDED-DIR); ENOENT maps to `file-missing'.
        Err(e) => {
            let s = if e.kind() == std::io::ErrorKind::NotFound {
                sym::FILE_MISSING
            } else {
                sym::FILE_ERROR
            };
            Err(i.signal_data(
                s,
                vec![
                    Value::string("Removing directory"),
                    Value::string(file_error_string(&e)),
                    Value::string(dir),
                ],
            ))
        }
    }
}
fn f_rename_file(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let from = want_filename(i, &a[0])?;
    let to = copy_target_name(from.trim_end_matches('/'), want_filename(i, &a[1])?);
    if let Some((handler, op)) = file_handler(i, "rename-file", &[&from, &to]) {
        return i.apply(
            &handler,
            vec![
                op,
                file_name_argument(&a[0], &from),
                file_name_argument(&a[1], &to),
                arg(&a, 2),
            ],
        );
    }
    // GNU checks the source FIRST: a missing FILE signals
    // file-missing ("Renaming" strerror FILE NEWNAME) before the
    // destination collision check runs.
    if let Err(e) = std::fs::symlink_metadata(native_path(i, &from)?) {
        return Err(report_file_errno(
            i,
            "Renaming",
            vec![Value::string(from), Value::string(to)],
            &e,
        ));
    }
    // A dangling symlink is an existing directory entry, even though its
    // referent is missing. Do not follow it for overwrite protection.
    if std::fs::symlink_metadata(native_path(i, &to)?).is_ok() {
        match a.get(2) {
            None | Some(Value::Nil) | Some(Value::Int(_)) => {
                let tag = i.q("file-already-exists");
                return Err(i.signal_data(
                    tag,
                    vec![Value::string("File already exists"), Value::string(to)],
                ));
            }
            _ => {}
        }
    }
    match std::fs::rename(native_path(i, &from)?, native_path(i, &to)?) {
        Ok(()) => Ok(Value::Nil),
        Err(e) => Err(report_file_errno(
            i,
            "Renaming",
            vec![Value::string(from), Value::string(to)],
            &e,
        )),
    }
}

/// `add-name-to-file' — GNU makes NEWNAME a hard link to FILE
/// (the original name stays), with the same OK-IF-ALREADY-EXISTS
/// rule as `rename-file'.
fn f_add_name_to_file(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let from = want_filename(i, &a[0])?;
    let to = want_filename(i, &a[1])?;
    // GNU checks the source FIRST (same as rename-file): a missing
    // FILE signals file-missing before the collision check.
    if !native_path(i, &from)?.exists() {
        return Err(i.signal_data(
            sym::FILE_MISSING,
            vec![
                Value::string("Adding new name"),
                Value::string("No such file or directory"),
                Value::string(from),
                Value::string(to),
            ],
        ));
    }
    if native_path(i, &to)?.exists() {
        match a.get(2) {
            None | Some(Value::Nil) | Some(Value::Int(_)) => {
                let tag = i.q("file-already-exists");
                return Err(i.signal_data(
                    tag,
                    vec![Value::string("File already exists"), Value::string(to)],
                ));
            }
            // GNU unlinks the old name first when overwriting.
            _ => {
                let _ = std::fs::remove_file(native_path(i, &to)?);
            }
        }
    }
    match std::fs::hard_link(native_path(i, &from)?, native_path(i, &to)?) {
        Ok(()) => Ok(Value::Nil),
        // GNU report_file_error: ("Adding new name" strerror FILE NEWNAME),
        // ENOENT → file-missing.
        Err(e) => {
            let s = if e.kind() == std::io::ErrorKind::NotFound {
                sym::FILE_MISSING
            } else {
                sym::FILE_ERROR
            };
            Err(i.signal_data(
                s,
                vec![
                    Value::string("Adding new name"),
                    Value::string(file_error_string(&e)),
                    Value::string(from),
                    Value::string(to),
                ],
            ))
        }
    }
}
fn f_copy_file(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // (copy-file FILENAME NEWNAME &optional OK-IF-ALREADY-EXISTS
    //   KEEP-TIME PRESERVE-UID-GID PRESERVE-PERMISSIONS)
    let from = want_filename(i, &a[0])?;
    let to = copy_target_name(&from, want_filename(i, &a[1])?);
    if let Some((handler, op)) = file_handler(i, "copy-file", &[&from, &to]) {
        return i.apply(
            &handler,
            vec![
                op,
                file_name_argument(&a[0], &from),
                file_name_argument(&a[1], &to),
                arg(&a, 2),
                arg(&a, 3),
                arg(&a, 4),
                arg(&a, 5),
            ],
        );
    }
    let keep_time = a.get(3).map(|v| v.truthy()).unwrap_or(false);
    let keep_uid_gid = a.get(4).map(|v| v.truthy()).unwrap_or(false);
    let preserve_permissions = a.get(5).map(|v| v.truthy()).unwrap_or(false);
    let meta = match std::fs::metadata(native_path(i, &from)?) {
        Ok(m) => m,
        Err(e) => {
            // GNU opens the input first: ("Opening input file" strerror
            // FILE), ENOENT → file-missing.
            let s = if e.kind() == std::io::ErrorKind::NotFound {
                sym::FILE_MISSING
            } else {
                sym::FILE_ERROR
            };
            return Err(i.signal_data(
                s,
                vec![
                    Value::string("Opening input file"),
                    Value::string(file_error_string(&e)),
                    Value::string(from),
                ],
            ));
        }
    };
    // GNU rejects non-regular files with "Non-regular file".
    if !meta.is_file() {
        return Err(i.signal_data(
            sym::FILE_ERROR,
            vec![
                Value::string("Non-regular file"),
                Value::string(format!("{}", std::io::Error::from_raw_os_error(21))),
                Value::string(from),
            ],
        ));
    }
    // GNU opens (and truncates) the output before comparing file identities.
    // Compare the open descriptors so aliases and renamed paths refer to the
    // files actually opened. In particular, a self-copy still truncates the
    // input, but reports an error instead of succeeding with an empty copy.
    let mut input = std::fs::File::open(native_path(i, &from)?)
        .map_err(|e| report_file_errno(i, "Opening input file", vec![Value::string(&from)], &e))?;
    let mut options = std::fs::OpenOptions::new();
    // O_EXCL checks directory entries, including dangling/looping links.
    // For an existing entry GNU reopens with O_TRUNC but without O_CREAT,
    // so overwriting a dangling link must not create its missing referent.
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
        let mask = if keep_uid_gid { 0o700 } else { 0o777 };
        options.mode(meta.mode() & mask);
    }
    let (mut output, already_exists) = match options.open(native_path(i, &to)?) {
        Ok(output) => (output, false),
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
            match a.get(2) {
                // Integers request confirmation; the headless path refuses.
                None | Some(Value::Nil) | Some(Value::Int(_)) => {
                    let tag = i.q("file-already-exists");
                    return Err(i.signal_data(
                        tag,
                        vec![Value::string("File already exists"), Value::string(to)],
                    ));
                }
                _ => {}
            }
            let output = std::fs::OpenOptions::new()
                .write(true)
                .truncate(true)
                .open(native_path(i, &to)?)
                .map_err(|e| {
                    report_file_errno(i, "Opening output file", vec![Value::string(&to)], &e)
                })?;
            (output, true)
        }
        Err(e) => {
            return Err(report_file_errno(
                i,
                "Opening output file",
                vec![Value::string(&to)],
                &e,
            ))
        }
    };
    #[cfg(unix)]
    let same_file = {
        use std::os::unix::fs::MetadataExt;
        let input_meta = input.metadata().map_err(|e| {
            report_file_errno(i, "Input file status", vec![Value::string(&from)], &e)
        })?;
        let output_meta = output.metadata().map_err(|e| {
            report_file_errno(i, "Output file status", vec![Value::string(&to)], &e)
        })?;
        input_meta.dev() == output_meta.dev() && input_meta.ino() == output_meta.ino()
    };
    #[cfg(not(unix))]
    let same_file =
        f_file_equal_p(i, vec![Value::string(&from), Value::string(&to)].into())?.truthy();
    if same_file {
        return Err(i.signal_data(
            sym::FILE_ERROR,
            vec![
                Value::string("Input and output files are the same"),
                Value::string(file_error_string(&std::io::Error::from_raw_os_error(0))),
                Value::string(from),
                Value::string(to),
            ],
        ));
    }
    // fs::copy can clone file metadata on macOS, even when KEEP-TIME
    // is nil. Copy data into an open destination, as GNU does, retaining
    // an existing destination's atime and giving new files current times.
    let copied = (|| -> std::io::Result<(std::fs::File, std::fs::File)> {
        std::io::copy(&mut input, &mut output)?;
        Ok((input, output))
    })();
    match copied {
        Ok((_input, output)) => {
            if keep_time {
                let times = std::fs::FileTimes::new()
                    .set_accessed(meta.accessed().unwrap_or(std::time::SystemTime::UNIX_EPOCH))
                    .set_modified(meta.modified().unwrap_or(std::time::SystemTime::UNIX_EPOCH));
                let _ = output.set_times(times);
            }
            if keep_uid_gid {
                #[cfg(unix)]
                {
                    use std::os::unix::fs::MetadataExt;
                    unsafe {
                        let cpath = std::ffi::CString::new(to.to_external_bytes()).unwrap();
                        libc::chown(cpath.as_ptr(), meta.uid(), meta.gid());
                    }
                }
            }
            #[cfg(target_os = "macos")]
            if preserve_permissions {
                use std::os::fd::AsRawFd;
                // Copy ACLs explicitly without cloning timestamps.
                if unsafe {
                    libc::fcopyfile(
                        _input.as_raw_fd(),
                        output.as_raw_fd(),
                        std::ptr::null_mut(),
                        libc::COPYFILE_ACL,
                    )
                } != 0
                {
                    let error = std::io::Error::last_os_error();
                    return Err(report_file_errno(
                        i,
                        "Copying permissions to",
                        vec![Value::string(&to)],
                        &error,
                    ));
                }
            }
            // GNU retains an existing destination's modes unless
            // PRESERVE-PERMISSIONS is requested. New files use source
            // mode bits masked by the default file permissions.
            if preserve_permissions || !already_exists {
                #[cfg(unix)]
                {
                    use std::os::unix::fs::{MetadataExt, PermissionsExt};
                    let mask = if preserve_permissions {
                        0o7777
                    } else {
                        default_file_modes() as u32 & 0o777
                    };
                    let _ =
                        output.set_permissions(std::fs::Permissions::from_mode(meta.mode() & mask));
                }
                #[cfg(not(unix))]
                let _ = output.set_permissions(meta.permissions());
            }
            Ok(Value::Nil)
        }
        Err(e) => Err(i.signal_data(
            sym::FILE_ERROR,
            vec![
                Value::string("Copying"),
                Value::string(format!("{}", e)),
                Value::string(from),
                Value::string(to),
            ],
        )),
    }
}

/// REPLACE decoders inspect the entire accessible text, but only a character
/// edit changes the count returned by insert-file-contents. Property-only
/// changes and no-op decoders keep the size of the actual inserted middle.
fn replace_file_transform(
    i: &mut Interp,
    fun: &Value,
    visit: Option<Value>,
) -> Result<Option<i128>, Flow> {
    let b = cur(i);
    let (point, len, tick) = {
        let mut bb = b.borrow_mut();
        let point = bb.point();
        let len = bb.text_len() - bb.begv;
        let tick = bb.text.chars_mod_tick.get();
        let begv = bb.begv;
        bb.set_point(begv);
        (point, len, tick)
    };
    let format = visit.is_some();
    let args = match visit {
        Some(visit) => vec![Value::Nil, Value::Int(len as i128), visit],
        None => vec![Value::Int(len as i128)],
    };
    let depth = i.specbind_depth();
    let result = (|| {
        let modifications = i.q("inhibit-modification-hooks");
        let motion = i.q("inhibit-point-motion-hooks");
        let undo = i.q("buffer-undo-list");
        i.specbind(modifications, Value::t())?;
        i.specbind(motion, Value::t())?;
        i.specbind(undo, Value::t())?;
        i.apply(fun, args)
    })();
    i.unbind_to(depth)?;
    let value = result?;
    if value.is_nil() && !format {
        return Ok(None);
    }
    let max = {
        let bb = cur(i);
        let bb = bb.borrow();
        (bb.text_len() - bb.point()) as i128
    };
    let count = match value {
        Value::Int(n) if n >= 0 && n <= max => n,
        other => return Err(i.wrong_type_mut("inserted-chars", &other)),
    };
    let current = cur(i);
    if current.borrow().text.chars_mod_tick.get() == tick {
        current.borrow_mut().set_point(point);
        Ok(None)
    } else {
        Ok(Some(count))
    }
}

fn post_insert_file_contents(
    i: &mut Interp,
    a: &[Value],
    mut inserted_chars: i128,
    do_replace: bool,
) -> Result<i128, Flow> {
    fn count(i: &mut Interp, value: Value) -> Result<i128, Flow> {
        let b = cur(i);
        let bb = b.borrow();
        let available = (bb.text_len() - bb.point()) as i128;
        match value {
            Value::Int(n) if n >= 0 && n <= available => Ok(n),
            other => Err(i.wrong_type_mut("inserted-chars", &other)),
        }
    }
    let aifs = i.q_soft("after-insert-file-set-coding");
    if aifs.is_some_and(|s| i.fbound_p(s)) {
        let visitv = a.get(1).cloned().unwrap_or(Value::Nil);
        let value = i.apply(
            &Value::Sym(aifs.unwrap()),
            vec![Value::Int(inserted_chars), visitv],
        )?;
        if value.truthy() {
            inserted_chars = count(i, value)?;
        }
    }
    if inserted_chars > 0 {
        let fd = i.q_soft("format-decode");
        if fd.is_some_and(|s| i.fbound_p(s)) {
            let visitv = a.get(1).cloned().unwrap_or(Value::Nil);
            if do_replace {
                if let Some(m) = replace_file_transform(i, &Value::Sym(fd.unwrap()), Some(visitv))?
                {
                    inserted_chars = m;
                }
            } else {
                let value = i.apply(
                    &Value::Sym(fd.unwrap()),
                    vec![Value::Nil, Value::Int(inserted_chars), visitv],
                )?;
                inserted_chars = count(i, value)?;
            }
        }
        let aiff = i.q_soft("after-insert-file-functions");
        let fnlist = aiff
            .map(|s| i.symbol_value(s))
            .and_then(|v| v.list_to_vec().ok())
            .unwrap_or_default();
        for f in fnlist {
            if do_replace {
                if let Some(m) = replace_file_transform(i, &f, None)? {
                    inserted_chars = m;
                }
            } else {
                let value = i.apply(&f, vec![Value::Int(inserted_chars)])?;
                if value.truthy() {
                    inserted_chars = count(i, value)?;
                }
            }
        }
    }
    Ok(inserted_chars)
}

fn insert_file_contents_impl(i: &mut Interp, a: crate::lisp::Argv, literal: bool) -> EvalResult {
    let visit = arg(&a, 1);
    if visit.truthy() && cur(i).borrow().base_buffer.is_some() {
        return Err(i.error("Cannot do file visiting in an indirect buffer"));
    }
    crate::buffer::primitives::check_writable(i)?;
    let path = want_filename(i, &a[0])?;
    if let Some((handler, op)) = file_handler(i, "insert-file-contents", &[&path]) {
        // Handlers own insertion and point placement, even for REPLACE.
        // GNU still runs the common decoding/hook tail after dispatch.
        let filename = file_name_argument(&a[0], &path);
        let result = i.apply(
            &handler,
            vec![
                op,
                filename.clone(),
                visit.clone(),
                arg(&a, 2),
                arg(&a, 3),
                arg(&a, 4),
            ],
        )?;
        let available = {
            let b = cur(i);
            let bb = b.borrow();
            (bb.text_len() - bb.point()) as i128
        };
        // GNU inspects only two cons cells, including dotted/cyclic values.
        let second = match &result {
            Value::Cons(first) => match &first.borrow().cdr {
                Value::Cons(second) => second.borrow().car.clone(),
                _ => Value::Nil,
            },
            _ => Value::Nil,
        };
        let count = match second {
            Value::Int(n) if n >= 0 && n <= available => n,
            _ => 0,
        };
        if visit.truthy() {
            // GNU assigns SAVE_MODIFF/AUTOSAVE_MODIFF directly here;
            // set-buffer-modified-p would additionally release a live lock.
            let b = cur(i);
            let mut bb = b.borrow_mut();
            let tick = bb.text.mod_tick.get();
            bb.text.save_tick.set(tick);
            bb.auto_save_tick = tick;
        }
        let count = post_insert_file_contents(i, &a, count, arg(&a, 4).truthy())?;
        return if result.is_nil() {
            Ok(Value::list(vec![filename, Value::Int(count)]))
        } else {
            Ok(result)
        };
    }
    // GNU restores REPLACE's original point through a marker, including when
    // a modification or post-insert hook signals an error.
    let saved = a
        .get(4)
        .filter(|v| v.truthy())
        .map(|_| i.save_excursion_state(false));
    let mut windows = Vec::new();
    let mut positions = Vec::new();
    if let Some(saved) = &saved {
        positions.push((saved.point.clone(), saved.point.borrow().position));
        for frame in &i.frames {
            for window in &frame.borrow().windows {
                let w = window.borrow();
                if !w.dead && !w.minibuffer && w.buffer == saved.buffer {
                    let marker = crate::lisp::memory::allocate(
                        crate::lisp::value::Marker {
                            buffer: Some(saved.buffer),
                            position: w.point,
                            insertion_type: false,
                        },
                        crate::lisp::Value::Marker,
                    );
                    cur(i).borrow_mut().register_marker(&marker);
                    positions.push((marker.clone(), w.point));
                    windows.push((window.clone(), marker, std::cell::Cell::new(w.point)));
                }
            }
        }
    }
    let result = insert_file_contents_inner(i, a, literal, &positions, &windows);
    for (window, marker, last_point) in windows {
        if !window.borrow().dead
            && window.borrow().point == last_point.get()
            && marker.borrow().buffer == Some(window.borrow().buffer)
        {
            window.borrow_mut().point = marker.borrow().position;
        }
        marker.borrow_mut().buffer = None;
    }
    if let Some(saved) = saved {
        if saved.point.borrow().buffer.is_none() {
            return Err(i.error("Marker does not point anywhere"));
        }
        let position = saved.point.borrow().position;
        cur(i).borrow_mut().set_point(position);
        saved.point.borrow_mut().buffer = None;
    }
    result
}

fn insert_file_contents_inner(
    i: &mut Interp,
    a: crate::lisp::Argv,
    literal: bool,
    saved_positions: &[(crate::lisp::value::MarkerRef, usize)],
    windows: &[(
        WindowRef,
        crate::lisp::value::MarkerRef,
        std::cell::Cell<usize>,
    )],
) -> EvalResult {
    let path = want_filename(i, &a[0])?;
    let visit = a.get(1).map(|v| v.truthy()).unwrap_or(false);
    let beg = a.get(2).cloned().unwrap_or(Value::Nil);
    let end = a.get(3).cloned().unwrap_or(Value::Nil);
    let replace = a.get(4).cloned().unwrap_or(Value::Nil);
    // GNU fileio.c Finsert_file_contents order: the VISIT + offsets
    // combination check runs BEFORE the file-offset type checks —
    // `(insert-file-contents F t B E)' always reports "Attempt to
    // visit less than an entire file" even for malformed B/E.
    if visit && (!beg.is_nil() || !end.is_nil()) {
        return Err(i.error("Attempt to visit less than an entire file"));
    }
    // GNU then runs file_offset(BEG)/file_offset(END) — still before the
    // open — so `wrong-type-argument file-offset' beats any file error.
    // A file offset must be a nonnegative integer (or integral float)
    // that fits off_t.
    fn check_file_offset(i: &mut Interp, v: &Value) -> Result<i128, Flow> {
        match v {
            Value::Int(n) if *n >= 0 && *n <= i64::MAX as i128 => Ok(*n),
            Value::Float(f) if **f >= 0.0 && **f < (i64::MAX as f64) + 1.0 && f.fract() == 0.0 => {
                Ok(**f as i128)
            }
            other => Err(i.wrong_type_mut("file-offset", other)),
        }
    }
    let beg_offset = if beg.is_nil() {
        0
    } else {
        check_file_offset(i, &beg)?
    };
    let end_offset = if end.is_nil() {
        i64::MAX as i128
    } else {
        check_file_offset(i, &end)?
    };
    // GNU: END_OFFSET = max (END_OFFSET, BEG_OFFSET).
    let end_offset = end_offset.max(beg_offset);
    let is_regular = std::fs::metadata(native_path(i, &path)?)
        .map(|m| m.is_file())
        .unwrap_or(true);
    // GNU opens the file (errors: "Opening input file" errno ABSNAME),
    // fstats it ("Input file status"), then reports directories with
    // "Read error" EISDIR before reading — std::fs::read would blur the
    // open vs read distinction, so do it in two steps.
    use std::io::Read;
    let mut fd = match std::fs::File::open(native_path(i, &path)?) {
        Ok(f) => f,
        Err(e) => {
            return Err(crate::editor::report_file_errno(
                i,
                "Opening input file",
                vec![Value::string(path)],
                &e,
            ));
        }
    };
    let meta = match fd.metadata() {
        Ok(m) => m,
        Err(e) => {
            return Err(crate::editor::report_file_errno(
                i,
                "Input file status",
                vec![Value::string(path)],
                &e,
            ));
        }
    };
    if meta.is_dir() {
        // GNU: report_file_errno ("Read error", EISDIR) on directories.
        return Err(crate::editor::report_file_errno(
            i,
            "Read error",
            vec![Value::string(path)],
            &std::io::Error::from_raw_os_error(libc::EISDIR),
        ));
    }
    let raw = {
        let mut b = Vec::new();
        match fd.read_to_end(&mut b) {
            Ok(_) => b,
            Err(e) => {
                return Err(crate::editor::report_file_errno(
                    i,
                    "Read error",
                    vec![Value::string(path)],
                    &e,
                ));
            }
        }
    };
    {
        // GNU: BEG/END are byte offsets into the file.
        let lo = (beg_offset as usize).min(raw.len());
        let hi = (end_offset as usize).min(raw.len());
        let slice = &raw[lo..hi.max(lo)];
        let mut requested = i
            .intern_soft("coding-system-for-read")
            .map(|symbol| i.symbol_value(symbol))
            .unwrap_or(Value::Nil);
        if requested.is_nil() && !literal {
            if is_regular {
                requested = crate::lisp::builtins::misc::auto_file_coding(i, &path, slice)?;
            }
            if requested.is_nil() {
                let operation = Value::Sym(i.q("insert-file-contents"));
                let finder = i.q("find-operation-coding-system");
                let mut arguments = vec![operation];
                arguments.extend(a.iter().cloned());
                arguments.resize(6, Value::Nil);
                if let Value::Cons(pair) = i.apply(&Value::Sym(finder), arguments)? {
                    requested = pair.borrow().car.clone();
                }
            }
        }
        let (decoded, coding) =
            crate::lisp::builtins::misc::decode_file_bytes(i, slice, &requested, literal)?;
        let contents = want_str(i, &decoded)?;
        let mut n = contents.chars().count();
        let b = cur(i);
        // GNU: REPLACE bypasses the non-empty check for VISIT.
        let if_reg = i.q_soft("if-regular").unwrap_or(u32::MAX);
        let do_replace = replace.truthy() && (!i.sym_is(&replace, if_reg) || is_regular);
        if visit && !do_replace && b.borrow().text.len() > 0 {
            return Err(i.error("Cannot do file visiting in a non-empty buffer"));
        }
        crate::buffer::primitives::check_writable(i)?;
        let (inserted_at, inserted_string) = if do_replace {
            let (begv, end, old) = {
                let bb = b.borrow();
                (
                    bb.begv,
                    bb.text_len(),
                    bb.text.substring(bb.begv, bb.text_len()),
                )
            };
            let old: Vec<crate::lisp::value::EmacsChar> = old.chars().collect();
            let new: Vec<crate::lisp::text::EmacsChar> = contents.chars().collect();
            let prefix = old.iter().zip(&new).take_while(|(a, b)| a == b).count();
            let suffix = old[prefix..]
                .iter()
                .rev()
                .zip(new[prefix..].iter().rev())
                .take_while(|(a, b)| a == b)
                .count();
            let start = begv + prefix;
            let old_end = end - suffix;
            let new_end = new.len() - suffix;
            n = new_end - prefix;
            let inserted = crate::lisp::builtins::strfn::f_substring(
                i,
                vec![
                    decoded.clone(),
                    Value::Int(prefix as i128),
                    Value::Int(new_end as i128),
                ]
                .into(),
            )?;
            let middle = want_str(i, &inserted)?;
            if start < old_end {
                crate::buffer::primitives::chg_delete(i, start, old_end)?;
            }
            let multibyte = i.q("enable-multibyte-characters");
            let requested_name = i.sym_id(&requested).map(|id| i.symbol_name(id));
            let raw_fast_path = literal
                || (!i.symbol_value(multibyte).truthy()
                    && requested_name.is_some_and(|name| {
                        name == "no-conversion" || name == "binary" || name.ends_with("-unix")
                    }));
            if n == 0 && prefix < new.len() && !raw_fast_path {
                // The decoded replacement path reports its empty insertion
                // after deleting a middle before a preserved suffix.
                b.borrow_mut().set_point(start);
                crate::lisp::builtins::evalfn::signal_after_change(i, start + 1, start + 1, 0)?;
            }
            if n > 0 {
                b.borrow_mut().set_point(start);
                crate::buffer::primitives::chg_insert_pt(i, &middle, false)?;
                b.borrow_mut().set_point(start);
                // GNU gives point (unlike ordinary markers) proportional
                // placement within the replaced middle (bug#19161).
                for (marker, point) in saved_positions {
                    if *point > start && *point <= old_end {
                        let growth = n as f64 / (old_end - start) as f64;
                        marker.borrow_mut().position =
                            start + (growth * (*point - start) as f64) as usize;
                    }
                }
            }
            (start, inserted)
        } else {
            let start = b.borrow().point();
            crate::buffer::primitives::chg_insert_pt(i, &contents, false)?;
            // GNU Finsert_file_contents leaves point BEFORE the inserted text.
            b.borrow_mut().set_point(start);
            (start, decoded.clone())
        };
        for (window, marker, last_point) in windows {
            if !window.borrow().dead && marker.borrow().buffer == Some(window.borrow().buffer) {
                let point = marker.borrow().position;
                window.borrow_mut().point = point;
                last_point.set(point);
            }
        }
        if let Value::Str(string) = &inserted_string {
            crate::buffer::primitives::copy_str_props(i, string, inserted_at);
        }
        if let Value::Int(converted_count) =
            crate::lisp::builtins::misc::post_file_conversion(i, &coding, inserted_at, n)?
        {
            n = converted_count.max(0) as usize;
        }
        if visit {
            // GNU Finsert_file_contents records the visited name,
            // truename, modtime, and default-directory.
            let truename = crate::buffer::file_truename(&path);
            let mut bb = b.borrow_mut();
            bb.file_name = Some(path.clone());
            let bft = i.q_soft("buffer-file-truename").unwrap_or(u32::MAX);
            let bfn = i.q_soft("buffer-file-name").unwrap_or(u32::MAX);
            bb.locals.insert(bft, Value::string(truename));
            bb.locals.insert(bfn, Value::string(path.clone()));
            bb.note_modified(false);
            if let Ok(m) = std::fs::metadata(native_path(i, &path)?) {
                bb.file_modtime_ns = m
                    .modified()
                    .ok()
                    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                    .map(|d| d.as_secs() as i128 * 1_000_000_000 + d.subsec_nanos() as i128)
                    .unwrap_or(-2);
                bb.file_modtime_size = m.len() as i128;
            }
            // set default-directory to file's dir
            if let Some(dir_end) = path.rfind('/') {
                let dd = i.q_soft("default-directory").unwrap_or(u32::MAX);
                bb.locals.insert(dd, Value::string(&path[..=dir_end]));
            }
        }
        // GNU Finsert_file_contents: record the coding system the
        // read was decoded with (`Vlast_coding_system_used'), then run
        // the post-insert chain — `after-insert-file-set-coding' sets
        // the buffer-local `buffer-file-coding-system' (info.el and
        // filepos-to-bufferpos depend on a concrete value rather than
        // the undecided placeholder), `format-decode' handles text
        // formats, and `after-insert-file-functions' may adjust the
        // character count returned to the caller.
        if let Some(lcs) = i.q_soft("last-coding-system-used") {
            i.set_symbol(lcs, coding)?;
        }
        let inserted_chars = post_insert_file_contents(i, &a, n as i128, do_replace)?;
        Ok(Value::list(vec![
            Value::string(path),
            Value::Int(inserted_chars),
        ]))
    }
}

fn f_insert_file_contents(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    insert_file_contents_impl(i, a, false)
}

fn f_insert_file_contents_literally(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU files.el's wrapper inhibits transformations, including when a
    // handler delegates recursively to insert-file-contents.
    let depth = i.specbind_depth();
    let result = (|| {
        let operation = Value::Sym(i.q("insert-file-contents"));
        let inhibit_op = i.q("inhibit-file-name-operation");
        let inhibit_handlers = i.q("inhibit-file-name-handlers");
        let previous = if crate::lisp::builtins::eq_values(&i.symbol_value(inhibit_op), &operation)
        {
            i.symbol_value(inhibit_handlers)
        } else {
            Value::Nil
        };
        let handlers = [
            "jka-compr-handler",
            "image-file-handler",
            "epa-file-handler",
        ]
        .into_iter()
        .rev()
        .fold(previous, |tail, name| {
            Value::cons(Value::Sym(i.intern(name)), tail)
        });
        for name in ["format-alist", "after-insert-file-functions"] {
            let symbol = i.intern(name);
            i.specbind(symbol, Value::Nil)?;
        }
        let coding = Value::Sym(i.q("no-conversion"));
        for name in ["coding-system-for-read", "coding-system-for-write"] {
            let symbol = i.intern(name);
            i.specbind(symbol, coding.clone())?;
        }
        i.specbind(inhibit_handlers, handlers)?;
        i.specbind(inhibit_op, operation)?;
        insert_file_contents_impl(i, a, true)
    })();
    i.unbind_to(depth)?;
    result
}

fn f_write_region(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let visit = arg(&a, 4);
    let visiting =
        matches!(&visit, Value::Str(_)) || matches!(&visit, Value::Sym(s) if *s == sym::T);
    if visiting && cur(i).borrow().base_buffer.is_some() {
        return Err(i.error("Cannot do file visiting in an indirect buffer"));
    }
    // validate_region coerces markers and sorts endpoints before dispatch.
    // Nil/string START instead forwards END unchanged to the handler.
    let (start, end) = if a[0].is_nil() || matches!(&a[0], Value::Str(_)) {
        (a[0].clone(), a[1].clone())
    } else {
        let x = want_int(i, &a[0])?;
        let y = want_int(i, &a[1])?;
        let (x, y) = (x.min(y), x.max(y));
        let b = cur(i);
        let bb = b.borrow();
        if x < bb.begv as i128 + 1 || y > bb.zv as i128 + 1 {
            return Err(i.signal_data(
                sym::ARGS_OUT_OF_RANGE,
                vec![Value::Buffer(b.clone()), a[0].clone(), a[1].clone()],
            ));
        }
        (Value::Int(x), Value::Int(y))
    };
    let path = want_filename(i, &a[2])?;
    let mustbenew = arg(&a, 6);
    let excl = Value::Sym(i.q("excl"));
    if mustbenew.truthy() && !crate::lisp::builtins::eq_values(&mustbenew, &excl) {
        if let Ok(meta) = std::fs::symlink_metadata(native_path(i, &path)?) {
            if meta.is_dir() {
                return Err(i.signal_data(
                    sym::FILE_ERROR,
                    vec![Value::string("File is a directory"), Value::string(&path)],
                ));
            }
            let query = Value::Sym(i.q("y-or-n-p"));
            let answer = i.apply(
                &query,
                vec![Value::string(format!(
                    "File {} already exists; overwrite anyway? ",
                    path
                ))],
            )?;
            if answer.is_nil() {
                let tag = i.q("file-already-exists");
                return Err(i.signal_data(
                    tag,
                    vec![Value::string("File already exists"), Value::string(&path)],
                ));
            }
        }
    }
    let filename = file_name_argument(&a[2], &path);
    let visit_file = match &visit {
        Value::Str(_) => file_name_argument(&visit, &want_filename(i, &visit)?),
        _ => filename.clone(),
    };
    let lockname = if arg(&a, 5).is_nil() {
        visit_file.clone()
    } else {
        arg(&a, 5)
    };
    let visit_raw = match &visit {
        Value::Str(name) => Some(name.borrow().clone()),
        _ => None,
    };
    let mut names = vec![path.as_str()];
    if let Some(name) = &visit_raw {
        names.push(name);
    }
    if let Some((handler, op)) = file_handler(i, "write-region", &names) {
        let result = i.apply(
            &handler,
            vec![
                op,
                start,
                end,
                filename.clone(),
                arg(&a, 3),
                visit,
                lockname,
                mustbenew,
            ],
        )?;
        if visiting {
            // GNU's handler branch records the visited name and save tick,
            // leaving truename, default-directory, and modtime to the handler.
            let bfn = i.q("buffer-file-name");
            let visit_name = want_str(i, &visit_file)?;
            let b = cur(i);
            let mut bb = b.borrow_mut();
            bb.file_name = Some(visit_name);
            bb.locals.insert(bfn, visit_file);
            // The handler path preserves any lock on the previous name.
            bb.text.save_tick.set(bb.text.mod_tick.get());
        }
        return Ok(result);
    }
    // Physical output still validates LOCKNAME before opening the file.
    if !matches!(&lockname, Value::Str(_)) {
        return Err(i.wrong_type_mut("stringp", &lockname));
    }
    let text = match &start {
        Value::Str(_) => start.clone(),
        Value::Nil => {
            let len = cur(i).borrow().text.len();
            buffer_file_text(i, 0, len)
        }
        Value::Int(x) => buffer_file_text(i, *x as usize - 1, end.int().unwrap() as usize - 1),
        _ => unreachable!(),
    };
    write_file_string(i, &path, &text, &a)
}

fn buffer_file_text(i: &mut Interp, start: usize, end: usize) -> Value {
    let b = cur(i);
    let bb = b.borrow();
    let multibyte = i.q("enable-multibyte-characters");
    let value = Value::string_with_kind(bb.text.substring(start, end), i.symbol_value(multibyte).truthy());
    if let Value::Str(r) = &value {
        i.set_str_props(r, bb.text.props.borrow().slice(start, end));
        let multibyte = i.q("enable-multibyte-characters");
        if i.symbol_value(multibyte).truthy() {
            i.mark_multibyte(r);
        } else {
            i.mark_unibyte(r);
        }
    }
    value
}

/// GNU fileio.c choose_write_coding_system: explicit coding, local
/// buffer coding, file-name policy, then defaults and safe encoding.
fn choose_write_coding<T: TextSource + ?Sized>(i: &mut Interp, path: &T, a: &[Value]) -> EvalResult {
    path.with_text(|path| {
    use crate::lisp::builtins::misc::{coding_inherit_file_eol, raw_file_coding};
    let write = i.q("coding-system-for-write");
    let explicit = i.symbol_value(write);
    if !explicit.is_nil() {
        return coding_inherit_file_eol(i, &explicit, &Value::Nil);
    }
    let (start, end) = if matches!(a[0], Value::Str(_)) {
        (a[0].clone(), arg(a, 1))
    } else if a[0].is_nil() {
        (
            Value::Int(1),
            Value::Int(cur(i).borrow().text_len() as i128 + 1),
        )
    } else {
        let x = want_int(i, &a[0])?;
        let y = want_int(i, &a[1])?;
        (Value::Int(x.min(y)), Value::Int(x.max(y)))
    };
    let buffer_coding = i.q("buffer-file-coding-system");
    let original = i.symbol_value(buffer_coding);
    let local = cur(i).borrow().locals.contains_key(&buffer_coding);
    let multibyte = i.q("enable-multibyte-characters");
    let mut raw = !local && !i.symbol_value(multibyte).truthy();
    let mut coding = if local { original.clone() } else { Value::Nil };
    if coding.is_nil() {
        let finder = i.q("find-operation-coding-system");
        let lockname = if arg(a, 5).is_nil() {
            match arg(a, 4) {
                Value::Str(name) => Value::string(expand_file_name_str(i, &name.borrow())),
                _ => Value::string(path),
            }
        } else {
            arg(a, 5)
        };
        let arguments = vec![
            Value::Sym(i.q("write-region")),
            start.clone(),
            end.clone(),
            Value::string(path),
            arg(a, 3),
            arg(a, 4),
            lockname,
        ];
        if let Value::Cons(pair) = i.apply(&Value::Sym(finder), arguments)? {
            coding = pair.borrow().cdr.clone();
        }
    }
    let using_default = coding.is_nil();
    if using_default {
        coding = original;
    }
    if !coding.is_nil() {
        let plist =
            crate::lisp::builtins::misc::f_coding_system_plist(i, vec![coding.clone()].into())?;
        let kind = i.q(":coding-type");
        raw |= i
            .sym_id(&crate::lisp::eval::plist_get(&plist, kind))
            .is_some_and(|s| i.symbol_name(s) == "raw-text");
    }
    let selector = i.q("select-safe-coding-system-function");
    let selector = i.symbol_value(selector);
    if !raw && i.sym_id(&selector).is_some_and(|s| i.fbound_p(s)) {
        let buffer = cur(i);
        let restriction = {
            let b = buffer.borrow();
            (b.begv, b.zv)
        };
        if a[0].is_nil() {
            let mut b = buffer.borrow_mut();
            b.begv = 0;
            b.zv = b.text_len();
        }
        let selected = i.apply(
            &selector,
            vec![start, end, coding, Value::Nil, Value::string(path)],
        );
        {
            let mut b = buffer.borrow_mut();
            b.begv = restriction.0;
            b.zv = restriction.1.min(b.text_len());
        }
        coding = selected?;
    }
    if !using_default {
        let default = i.obarray.symbol(buffer_coding).value.clone();
        coding = coding_inherit_file_eol(i, &coding, &default)?;
    }
    if raw {
        coding = raw_file_coding(i, &coding)?;
    }
    // An unspecified external EOL format inherits the host's Unix EOL.
    coding_inherit_file_eol(i, &coding, &Value::Nil)

    })
}

fn region_bounds(i: &mut Interp, s: &Value, e: &Value, len: usize) -> Result<(usize, usize), Flow> {
    // GNU Fwrite_region: nil START/END mean point-min/point-max
    // (build_annotations via a1=a2=nil path); present non-numbers
    // signal integer-or-marker-p.  Marker positions are 1-based.
    let mut pos = |v: &Value| -> Result<Option<i128>, Flow> {
        match v {
            Value::Nil => Ok(None),
            Value::Int(n) => Ok(Some(*n)),
            Value::Marker(m) => {
                if m.borrow().buffer.is_some() {
                    Ok(Some(m.borrow().position as i128 + 1))
                } else {
                    Err(i.error("Marker does not point anywhere"))
                }
            }
            other => Err(i.wrong_type_mut("integer-or-marker-p", other)),
        }
    };
    let sp = pos(s)?.unwrap_or(1);
    let ep = pos(e)?.unwrap_or(len as i128 + 1);
    if sp < 1 || ep < 1 || sp > len as i128 + 1 || ep > len as i128 + 1 || sp > ep {
        let bufv = i
            .current_buffer_ref()
            .map(|b| Value::Buffer(b))
            .unwrap_or(Value::Nil);
        return Err(i.signal_data(sym::ARGS_OUT_OF_RANGE, vec![bufv, s.clone(), e.clone()]));
    }
    Ok((sp as usize - 1, ep as usize - 1))
}

/// GNU `validate_region' (coding.c) — unlike the permissive
/// `region_bounds' (write/append/call-process accept nil for the
/// whole buffer), nil args are rejected outright.
fn region_bounds_strict(
    i: &mut Interp,
    s: &Value,
    e: &Value,
    len: usize,
) -> Result<(usize, usize), Flow> {
    // GNU validate_region checks START then END, each via
    // integer-or-marker (nil has no whole-buffer default here).
    for v in [s, e] {
        if !matches!(v, Value::Int(_) | Value::Marker(_)) {
            return Err(i.wrong_type_mut("integer-or-marker-p", v));
        }
    }
    region_bounds(i, s, e, len)
}

/// Encode buffer/string text for file output: GNU's raw-text
/// encoding writes eight-bit chars (0x3FFF80..=0x3FFFFF, held as PUA
/// proxies) as their single byte values.
fn file_bytes_of_str<T: TextSource + ?Sized>(text: &T) -> Vec<u8> {
    text.with_text(EmacsStr::to_external_bytes)
}

/// GNU's `write-region' VISIT side effects: make the buffer visit
/// NAME (buffer-file-name, truename, default-directory, modtime)
/// and clear the modified flag.
fn set_visit_state<T: TextSource + ?Sized>(i: &mut Interp, path: &T) {
    path.with_text(|path| {
    let truename = crate::buffer::file_truename(path);
    let b = cur(i);
    let mut bb = b.borrow_mut();
    bb.file_name = Some(path.to_string());
    let bfn = i.q_soft("buffer-file-name").unwrap_or(u32::MAX);
    let bft = i.q_soft("buffer-file-truename").unwrap_or(u32::MAX);
    bb.locals.insert(bfn, Value::string(path.to_string()));
    bb.locals.insert(bft, Value::string(truename));
    if let Some(dir_end) = path.rfind('/') {
        let dd = i.q_soft("default-directory").unwrap_or(u32::MAX);
        bb.locals
            .insert(dd, Value::string(path[..=dir_end].to_string()));
    }
    update_visited_file_modtime(&mut bb, path);
    bb.note_modified(false);

    })
}

fn file_text_bytes<T: TextSource + ?Sized>(
    i: &mut Interp,
    path: &T,
    text: &Value,
    arguments: &[Value],
) -> Result<Vec<u8>, Flow> {
    path.with_text(|path| {
    let coding = choose_write_coding(i, path, arguments)?;
    let encoded =
        crate::lisp::builtins::misc::f_encode_coding_string(i, vec![text.clone(), coding].into())?;
    Ok(file_bytes_of_str(&want_str(i, &encoded)?))

    })
}

pub(crate) fn temp_file_text_bytes<T: TextSource + ?Sized>(
    i: &mut Interp,
    path: &T,
    text: &Value,
) -> Result<Vec<u8>, Flow> {
    path.with_text(|path| {
    // GNU make-temp-file-internal calls write_region with the original
    // string and default optional arguments, writing through its open fd.
    file_text_bytes(
        i,
        path,
        text,
        &[text.clone(), Value::Nil, Value::string(path)],
    )

    })
}

fn write_file_string<T: TextSource + ?Sized>(i: &mut Interp, path: &T, text: &Value, a: &[Value]) -> EvalResult {
    path.with_text(|path| {
    let native = native_path(i, path)?;
    let append = a.get(3).cloned().unwrap_or(Value::Nil);
    let excl = matches!(a.get(6), Some(Value::Sym(s)) if *s == i.q("excl"));
    if excl && native.exists() {
        // MUSTBENEW = 'excl: fail if the file exists.
        return Err(i.signal_data(
            sym::FILE_ERROR,
            vec![Value::string("File already exists"), Value::string(path)],
        ));
    }
    let bytes = file_text_bytes(i, path, text, a)?;
    let r = match &append {
        // GNU: an integer APPEND overwrites at that byte offset
        // (the file keeps its trailing bytes; no truncation).
        Value::Int(off) => std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .open(&native)
            .and_then(|mut f| {
                use std::io::{Seek, SeekFrom, Write};
                f.seek(SeekFrom::Start((*off).max(0) as u64))
                    .and_then(|_| f.write_all(&bytes))
            }),
        v if v.truthy() => std::fs::OpenOptions::new()
            .append(true)
            .create(true)
            .open(&native)
            .and_then(|mut f| std::io::Write::write_all(&mut f, &bytes)),
        _ => std::fs::write(&native, &bytes),
    };
    match r {
        Ok(()) => {
            // GNU: writing the visited file refreshes the recorded
            // modtime (write-region / basic-save-buffer path).
            update_visited_file_modtime(&mut cur(i).borrow_mut(), path);
            let visit = a.get(4).cloned().unwrap_or(Value::Nil);
            match &visit {
                // VISIT a string → the buffer visits THAT name.
                Value::Str(name) => {
                    set_visit_state(i, &name.borrow().clone());
                    if !i.noninteractive {
                        i.message(&format!("Wrote {}", path));
                    }
                }
                // GNU: only `t' makes the buffer visit FILENAME; other
                // truthy values ('quiet, 'nomsg, 'silent, ...) just
                // suppress the "Wrote" message without visiting.
                Value::Sym(s) if *s == sym::T => {
                    set_visit_state(i, path);
                    if !i.noninteractive {
                        i.message(&format!("Wrote {}", path));
                    }
                }
                // VISIT nil → GNU still prints "Wrote file"
                // interactively (only batch and non-nil/non-t/non-string
                // values suppress it).
                Value::Nil => {
                    if !i.noninteractive {
                        i.message(&format!("Wrote {}", path));
                    }
                }
                _ => {}
            }
            Ok(Value::Nil)
        }
        Err(e) => {
            // GNU report_file_error: ("Opening output file" strerror
            // FILENAME), ENOENT → `file-missing'.
            let s = match e.kind() {
                std::io::ErrorKind::NotFound => sym::FILE_MISSING,
                // GNU report_file_errno maps EACCES/EPERM to the
                // `permission-denied' error symbol (fileio.c).
                std::io::ErrorKind::PermissionDenied => i.q("permission-denied"),
                _ => sym::FILE_ERROR,
            };
            Err(i.signal_data(
                s,
                vec![
                    Value::string("Opening output file"),
                    Value::string(file_error_string(&e)),
                    Value::string(path),
                ],
            ))
        }
    }

    })
}

fn f_set_visited_file_name(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU files.el set-visited-file-name, verbatim steps:
    //   indirect-buffer error → filename expansion (truename under
    //   find-file-visit-truename) → empty-basename error → visiting-
    //   buffer y-or-n-p (unless NO-QUERY) → lock/unlock → record name,
    //   truename, default-directory, rename buffer → buffer-backed-up
    //   nil → clear-visited-file-modtime unless ALONG-WITH-FILE →
    //   buffer-file-number → kill-local write-file vars, vc-mode →
    //   file-name-picked major-mode tail.
    let arg0 = a.get(0).cloned().unwrap_or(Value::Nil);
    let no_query = a.get(1).map(|v| v.truthy()).unwrap_or(false);
    let along_with_file = a.get(2).map(|v| v.truthy()).unwrap_or(false);
    {
        let b = cur(i);
        if b.borrow().base_buffer.is_some() {
            return Err(i.error("An indirect buffer cannot visit a file"));
        }
    }
    let mut fname = match &arg0 {
        Value::Nil => None,
        Value::Str(s) if s.borrow().is_empty() => None,
        Value::Str(s) => Some(expand_file_name_str(i, &s.borrow())),
        other => return Err(i.wrong_type_mut("stringp", &other.clone())),
    };
    let truename = fname.as_ref().map(|f| crate::buffer::file_truename(f));
    if fname.is_some() {
        let ffvt = i
            .intern_soft("find-file-visit-truename")
            .map(|id| i.symbol_value(id).truthy())
            .unwrap_or(false);
        if ffvt {
            fname = truename.clone();
        }
    }
    if let Some(f) = &fname {
        let new_name = f.rsplit('/').next().unwrap_or(f).to_string();
        if new_name.is_empty() {
            return Err(i.error("Empty file name"));
        }
    }
    if let Some(f) = &fname {
        // (find-buffer-visiting filename) → prompt unless no-query.
        let fbv = i.q("find-buffer-visiting");
        let visitor = i.apply(&Value::Sym(fbv), vec![Value::string(f.clone())]);
        match visitor {
            Err(fl) => return Err(fl),
            Ok(Value::Buffer(vb)) => {
                let same = vb.borrow().id == i.current_buffer;
                if !same && !no_query {
                    let msg = Value::string(format!("A buffer is visiting {f}; proceed? "));
                    let yonp = i.q("y-or-n-p");
                    match i.apply(&Value::Sym(yonp), vec![msg]) {
                        Err(fl) => return Err(fl),
                        Ok(v) if !v.truthy() => {
                            let ue = i.q("user-error");
                            return Err(i.signal_data(ue, vec![Value::string("Aborted")]));
                        }
                        _ => {}
                    }
                }
            }
            Ok(_) => {}
        }
    }
    let bid = i.current_buffer;
    let old_name = cur(i).borrow().file_name.clone();
    if !crate::lisp::builtins::equal_values(
        i,
        &match &fname {
            Some(f) => Value::string(f.clone()),
            None => Value::Nil,
        },
        &match &old_name {
            Some(f) => Value::string(f.clone()),
            None => Value::Nil,
        },
    )? {
        if let Some(f) = &fname {
            let lb = i.q("lock-buffer");
            if let Err(fl) = i.apply(&Value::Sym(lb), vec![Value::string(f.clone())]) {
                return Err(fl);
            }
        }
        let ub = i.q("unlock-buffer");
        if let Err(fl) = i.apply(&Value::Sym(ub), vec![]) {
            return Err(fl);
        }
    }
    {
        let b = cur(i);
        let mut bb = b.borrow_mut();
        let bfn = i.q_soft("buffer-file-name").unwrap_or(u32::MAX);
        let _bft = i.q_soft("buffer-file-truename").unwrap_or(u32::MAX);
        bb.locals.insert(
            bfn,
            match &fname {
                Some(f) => Value::string(f.clone()),
                None => Value::Nil,
            },
        );
        bb.file_name = fname.clone();
        if let Some(f) = &fname {
            let new_name = f.rsplit('/').next().unwrap_or(f).to_string();
            let dir = f[..f.len() - new_name.len()].to_string();
            let dd = i.q_soft("default-directory").unwrap_or(u32::MAX);
            bb.locals.insert(dd, Value::string(dir));
        }
        // GNU: (setq buffer-backed-up nil)
        let bu = i.q_soft("buffer-backed-up").unwrap_or(u32::MAX);
        bb.locals.insert(bu, Value::Nil);
    }
    if let Some(f) = &fname {
        let new_name = f.rsplit('/').next().unwrap_or(f).to_string();
        if !new_name.is_empty() {
            let cur_name = cur(i).borrow().name.clone();
            if cur_name != new_name {
                // (rename-buffer new-name t): uniquify on clash.
                i.buffers.rename(bid, &new_name);
            }
        }
    }
    if !along_with_file {
        crate::buffer::primitives::f_clear_visited_file_modtime(i, vec![].into())?;
    }
    {
        let b = cur(i);
        let mut bb = b.borrow_mut();
        let bft = i.q_soft("buffer-file-truename").unwrap_or(u32::MAX);
        let bfn = i.q_soft("buffer-file-name").unwrap_or(u32::MAX);
        if let Some(t) = &truename {
            // GNU: buffer-file-truename is abbreviated; under
            // find-file-visit-truename buffer-file-name follows.
            let ab = i.q("abbreviate-file-name");
            let abv = i
                .apply(&Value::Sym(ab), vec![Value::string(t.clone())])
                .unwrap_or_else(|_| Value::string(t.clone()));
            bb.locals.insert(bft, abv);
            let ffvt = i
                .intern_soft("find-file-visit-truename")
                .map(|id| i.symbol_value(id).truthy())
                .unwrap_or(false);
            if ffvt {
                bb.locals.insert(bfn, Value::string(t.clone()));
                bb.file_name = Some(t.clone());
            }
        } else {
            bb.locals.insert(bft, Value::Nil);
        }
    }
    {
        // (setq buffer-file-number (file-attribute-file-identifier
        //   (file-attributes buffer-file-name)))
        let num = match &fname {
            Some(f) => {
                let fa = i.q("file-attributes");
                match i.apply(&Value::Sym(fa), vec![Value::string(f.clone())]) {
                    Err(_) => Value::Nil,
                    Ok(attrs) => {
                        let fid = i.q("file-attribute-file-identifier");
                        i.apply(&Value::Sym(fid), vec![attrs])
                            .unwrap_or(Value::Nil)
                    }
                }
            }
            None => Value::Nil,
        };
        let b = cur(i);
        let mut bb = b.borrow_mut();
        let n = i.q_soft("buffer-file-number").unwrap_or(u32::MAX);
        bb.locals.insert(n, num);
    }
    for name in [
        "write-file-functions",
        "local-write-file-hooks",
        "revert-buffer-function",
        "backup-inhibited",
    ] {
        let klv = i.q("kill-local-variable");
        let sym = i.intern(name);
        let _ = i.apply(&Value::Sym(klv), vec![Value::Sym(sym)]);
    }
    {
        let b = cur(i);
        let mut bb = b.borrow_mut();
        let vc = i.q_soft("vc-mode").unwrap_or(u32::MAX);
        let vc_set = bb
            .locals
            .get(&vc)
            .map(|v| v.truthy())
            .unwrap_or(false);
        if vc_set {
            let ro = i.q_soft("buffer-read-only").unwrap_or(u32::MAX);
            bb.locals.insert(ro, Value::Nil);
        }
        bb.locals.shift_remove(&vc);
    }
    // GNU files.el set-visited-file-name tail: unless the mode is
    // `special', let the file name pick the major mode
    // (set-auto-mode) and re-run file-local variables when the mode
    // changed.  Errors are swallowed (condition-case ... (error nil)).
    // This is what gives e.g. .txt files `text-mode' and its
    // buffer-local `require-final-newline' in `write-file'.
    {
        let cmmw = i
            .intern_soft("change-major-mode-with-file-name")
            .map(|id| i.symbol_value(id).truthy())
            .unwrap_or(true);
        let get = i.q_soft("get");
        let mc = i.q_soft("mode-class");
        let mm_id = i.q_soft("major-mode");
        let mode_val = mm_id
            .and_then(|mm| cur(i).borrow().locals.get(&mm).cloned())
            .unwrap_or(Value::Nil);
        let special = if !cmmw {
            true
        } else {
            match (mode_val.clone(), get, mc) {
                (Value::Sym(m), Some(g), Some(mc)) => i
                    .apply(&Value::Sym(g), vec![Value::Sym(m), Value::Sym(mc)])
                    .map(|v| v.truthy())
                    .unwrap_or(false),
                _ => false,
            }
        };
        if !special {
            let hlv = i.q("hack-local-variables");
            let applied = i
                .apply(&Value::Sym(hlv), vec![Value::t()])
                .map(|v| v.truthy())
                .unwrap_or(false);
            if !applied {
                let sam = i.q("set-auto-mode");
                if i.fbound_p(sam) {
                    let _ = i.apply(&Value::Sym(sam), vec![Value::t()]);
                    let now_mode = mm_id
                        .and_then(|mm| cur(i).borrow().locals.get(&mm).cloned())
                        .unwrap_or(Value::Nil);
                    if !eq_values(&now_mode, &mode_val) {
                        let _ = i.apply(&Value::Sym(hlv), vec![Value::Nil]);
                    }
                }
            }
        }
    }
    Ok(Value::Nil)
}

/// GNU `find-file-noselect' wildcard check: WILDCARDS arg +
/// `find-file-wildcards' + glob chars in FILENAME.
fn find_file_wildcards_p(i: &Interp, filename: &EmacsStr, a: &[Value]) -> bool {
    let wildcards = a.get(3).map(|v| v.truthy()).unwrap_or(false);
    if !wildcards {
        return false;
    }
    let ffw = i
        .intern_soft("find-file-wildcards")
        .map(|id| i.symbol_value(id).truthy())
        .unwrap_or(true);
    ffw && filename.chars().any(|c| (c == '*' || c == '?' || c == '['))
}

fn f_find_file_noselect(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let path = want_filename(i, &a[0])?;
    // GNU fileio.c Ffind_file_noselect dispatches through
    // `file-name-handler-alist' FIRST (operation `find-file-noselect') —
    // a stray ("" . handler) entry makes GNU call it and typically die
    // with invalid-function; parity means we must call it too.
    {
        let op = Value::Sym(i.q("find-file-noselect"));
        if let Some(h) = crate::buffer::extra::find_file_name_handler(i, &path, op.clone()) {
            let mut args = vec![op];
            args.extend(a.iter().cloned());
            return i.apply(&h, args);
        }
    }
    // GNU files.el: with WILDCARDS non-nil and `find-file-wildcards'
    // enabled, a name containing glob metacharacters visits every
    // match and returns a LIST of the buffers.
    if find_file_wildcards_p(i, &path, &a) {
        let few = i.q("file-expand-wildcards");
        if i.fbound_p(few) {
            let files = i.apply(
                &Value::Sym(few),
                vec![Value::string(path.clone()), Value::t()],
            )?;
            let mut bufs = Vec::new();
            let mut fs = files;
            loop {
                match fs {
                    Value::Cons(c) => {
                        let (car, cdr) = {
                            let b = c.borrow();
                            (b.car.clone(), b.cdr.clone())
                        };
                        if let Value::Str(f) = car {
                            let b = f_find_file_noselect(
                                i,
                                vec![Value::Str(f.clone()), arg(&a, 1), arg(&a, 2), Value::Nil]
                                    .into(),
                            )?;
                            bufs.push(b);
                        }
                        fs = cdr;
                    }
                    _ => break,
                }
            }
            return Ok(Value::list(bufs));
        }
    }
    // GNU files.el `find-file-noselect': a directory argument goes to
    // `find-directory-functions' (dired-noselect) instead of visiting
    // it as a file — that's what makes `C-x C-f DIR RET' open Dired.
    if native_path(i, &path)?.is_dir() {
        let ffrd = i
            .intern_soft("find-file-run-dired")
            .map(|id| i.symbol_value(id));
        if ffrd.map_or(false, |v| v.truthy()) {
            // With `find-file-visit-truename' non-nil GNU passes the
            // abbreviated truename to the hook functions.
            let arg = if i
                .intern_soft("find-file-visit-truename")
                .map_or(false, |id| i.symbol_value(id).truthy())
            {
                let ft = i.q("file-truename");
                let tr = if i.fbound_p(ft) {
                    i.apply(&Value::Sym(ft), vec![Value::string(path.clone())])?
                } else {
                    Value::string(path.clone())
                };
                let ab = i.q("abbreviate-file-name");
                if i.fbound_p(ab) {
                    i.apply(&Value::Sym(ab), vec![tr])?
                } else {
                    tr
                }
            } else {
                Value::string(path.clone())
            };
            let hook = i.q("run-hook-with-args-until-success");
            let fdf = i.q("find-directory-functions");
            let r = i.apply(&Value::Sym(hook), vec![Value::Sym(fdf), arg])?;
            if r.truthy() {
                return Ok(r);
            }
        }
        return Err(i.error(format!("{} is a directory", path)));
    }
    // If a buffer already visits this file, return it.  GNU also
    // recognizes the file's TRUENAME (a symlink to a visited file
    // returns the target's buffer).  With NOWARN nil GNU re-verifies
    // the file's modtime first: unchanged → return as-is; changed +
    // `revert-without-query' filename match → auto-revert; changed +
    // `query-about-changed-file' nil → message only; otherwise GNU
    // calls `yes-or-no-p', which in batch reads EOF off /dev/null.
    let truename = crate::buffer::file_truename(&path);
    let mut existing: Option<std::rc::Rc<std::cell::RefCell<crate::buffer::Buffer>>> = None;
    for id in i.buffers.list() {
        if let Some(b) = i.buffers.get(id) {
            let bb = b.borrow();
            if bb.file_name.as_deref() == Some(path.as_str())
                || bb.file_name.as_deref() == Some(truename.as_str())
            {
                existing = Some(b.clone());
                break;
            }
            let bft = i.q_soft("buffer-file-truename").unwrap_or(u32::MAX);
            if let Some(Value::Str(tn)) = bb.locals.get(&bft) {
                if *tn.borrow() == truename {
                    existing = Some(b.clone());
                    break;
                }
            }
        }
    }
    if let Some(b) = existing {
        if !arg(&a, 1).truthy() {
            let bv = Value::Buffer(b.clone());
            let fresh =
                crate::buffer::primitives::f_verify_visited_file_modtime(i, vec![bv].into())?
                    .truthy();
            if !fresh {
                let bid = b.borrow().id;
                let modified = i
                    .buffers
                    .get(bid)
                    .map(|bb| bb.borrow().is_modified())
                    .unwrap_or(false);
                let rwq = i
                    .intern_soft("revert-without-query")
                    .map(|id| i.symbol_value(id))
                    .unwrap_or(Value::Nil);
                let mut auto = false;
                let mut l = rwq;
                while let Value::Cons(c) = l {
                    let (car, cdr) = {
                        let cb = c.borrow();
                        (cb.car.clone(), cb.cdr.clone())
                    };
                    if let Value::Str(re) = car {
                        let sm = i.q("string-match");
                        let hit = i
                            .apply(
                                &Value::Sym(sm),
                                vec![Value::Str(re.clone()), Value::string(path.clone())],
                            )
                            .map(|v| v.truthy())
                            .unwrap_or(false);
                        if hit {
                            auto = true;
                            break;
                        }
                    }
                    l = cdr;
                }
                if auto && !modified {
                    // GNU: (revert-buffer t t) inside the buffer.
                    let prev = i.current_buffer;
                    i.set_current_buffer(bid);
                    let rb = i.q("revert-buffer");
                    let r = if i.fbound_p(rb) {
                        i.apply(&Value::Sym(rb), vec![Value::t(), Value::t()])
                    } else {
                        Ok(Value::Nil)
                    };
                    i.set_current_buffer(prev);
                    let _ = r;
                } else {
                    let qac = i
                        .intern_soft("query-about-changed-file")
                        .map(|id| i.symbol_value(id))
                        .unwrap_or(Value::t());
                    if qac.truthy() && i.noninteractive {
                        let s = i.q("end-of-file");
                        return Err(i.signal_data(
                            s,
                            vec![Value::string("Error reading from stdin".to_string())],
                        ));
                    }
                    // GNU's message-only path (`query-about-changed-file'
                    // nil) just informs; return the buffer as-is.
                }
            }
        }
        return Ok(i.buffer_value(b.borrow().id).unwrap_or(Value::Nil));
    }
    let name = native_path(i, &path)?
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| (path.clone()).to_unicode_lossy());
    // GNU files.el creates file-visiting buffers through
    // `create-file-buffer', which invokes the uniquify advice when
    // uniquify is loaded; fall back to a direct create otherwise.
    let cfb = i.q("create-file-buffer");
    let bid = if i.fbound_p(cfb) {
        match i.apply(&Value::Sym(cfb), vec![Value::string(path.clone())])? {
            Value::Buffer(b) => b.borrow().id,
            _ => i.create_buffer(&name),
        }
    } else {
        i.create_buffer(&name)
    };
    {
        let b = i.buffers.get(bid).unwrap();
        let mut bb = b.borrow_mut();
        bb.file_name = Some(path.clone());
        // GNU exposes the visited file name as the buffer-local variable
        // `buffer-file-name' (a C field); mirror it into locals so Lisp
        // reads work.  permanent-local marks keep it through
        // kill-all-local-variables.
        let bfn = i.q_soft("buffer-file-name").unwrap_or(u32::MAX);
        let bft = i.q_soft("buffer-file-truename").unwrap_or(u32::MAX);
        bb.locals.insert(bfn, Value::string(path.clone()));
        bb.locals.insert(bft, Value::string(truename.clone()));
        // Snapshot `create-lockfiles' for Buffer's C-level auto-lock
        // (which cannot see Lisp state during modification).
        if let Some(cl) = i.q_soft("create-lockfiles") {
            bb.create_lockfiles = i.symbol_value(cl).truthy();
        }
        if let Some(dir_end) = path.rfind('/') {
            let dd = i.q_soft("default-directory").unwrap_or(u32::MAX);
            bb.locals
                .insert(dd, Value::string(path[..=dir_end].to_string()));
        }
        bb.file_modtime_ns = -1;
    }
    let raw = arg(&a, 2).truthy();
    let read = crate::buffer::primitives::chg_with_buffer(i, bid, |i| {
        if raw {
            let multibyte = i.q("enable-multibyte-characters");
            i.set_buffer_local_toplevel_value(multibyte, bid, Value::Nil);
            let literal = i.q("find-file-literally");
            i.set_buffer_local_toplevel_value(literal, bid, Value::t());
        }
        insert_file_contents_impl(i, vec![Value::string(path.clone()), Value::t()].into(), raw)
    });
    match read {
        Err(Flow::Signal(signal))
            if signal.is_symbol(sym::FILE_MISSING) && !native_path(i, &path)?.exists() => {}
        result => {
            result?;
        }
    }
    // GNU find-file-noselect -> after-find-file: pick the major mode and
    // process file-local variables, then run find-file-hook.
    let prev_buf = i.current_buffer;
    i.set_current_buffer(bid);
    let nm = i.q("normal-mode");
    let rh = i.q("run-hooks");
    let ffh = i.q("find-file-hook");
    let r1 = if raw {
        Ok(Value::Nil)
    } else {
        i.apply(&Value::Sym(nm), vec![Value::t()])
    };
    let r2 = r1.and_then(|_| i.apply(&Value::Sym(rh), vec![Value::Sym(ffh)]));
    i.set_current_buffer(prev_buf);
    r2?;
    Ok(i.buffer_value(bid).unwrap_or(Value::Nil))
}

fn f_find_file(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let buf = f_find_file_noselect(i, a)?;
    if let Value::Buffer(b) = &buf {
        let id = b.borrow().id;
        i.set_current_buffer(id);
        if let Some(w) = sel_window(i) {
            w.borrow_mut().buffer = id;
            let point = b.borrow().point();
            set_window_old_point(i, &mut w.borrow_mut(), point);
            record_buffer_display(i, id);
        }
    }
    Ok(buf)
}

/// Refresh a buffer's recorded visited-file modtime+size after writing
/// PATH (only when PATH is the file the buffer visits).
fn update_visited_file_modtime<T: TextSource + ?Sized>(buffer: &mut crate::buffer::Buffer, path: &T) {
    path.with_text(|path| {
        if buffer.file_name.as_deref() != Some(path) { return; }
        let Ok(native) = path.to_os_string() else { return; };
        if let Ok(metadata) = std::fs::metadata(native) {
            buffer.file_modtime_ns = metadata.modified().ok()
                .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|time| time.as_secs() as i128 * 1_000_000_000 + time.subsec_nanos() as i128)
                .unwrap_or(-2);
            buffer.file_modtime_size = metadata.len() as i128;
        }
    });
}

fn f_save_buffer(i: &mut Interp, _a: crate::lisp::Argv) -> EvalResult {
    let b = cur(i);
    let path = {
        let bb = b.borrow();
        match &bb.file_name {
            Some(p) => p.clone(),
            // GNU basic-save-buffer prompts for a file name; in batch
            // that read hits end-of-file.
            None if i.noninteractive => {
                let s = i.q("end-of-file");
                return Err(i.signal_data(s, vec![Value::string("Error reading from stdin")]));
            }
            None => return Err(i.error("No file is associated with this buffer")),
        }
    };
    // GNU basic-save-buffer: with `require-final-newline' non-nil a
    // buffer that doesn't end in a newline gets one appended before
    // writing (the buffer itself is extended, like save-excursion at
    // point-max).
    let rfn = i
        .intern_soft("require-final-newline")
        .map(|id| i.symbol_value(id))
        .unwrap_or(Value::Nil);
    {
        let bb = b.borrow();
        let len = bb.text.len();
        let needs_nl = len > 0 && !bb.text.substring(len - 1, len).ends_with('\n') && rfn.truthy();
        drop(bb);
        if needs_nl {
            let len2 = cur(i).borrow().text.len();
            crate::buffer::primitives::chg_insert(i, len2, "\n")?;
        }
    }
    // GNU basic-save-buffer's dir-existence prompt runs before
    // basic-save-buffer-2's writable checks: when neither FILE nor DIR
    // exists, GNU asks "create?" — in batch the read hits EOF.
    {
        let fx = f_file_exists_p(i, vec![Value::string(path.clone())].into())?;
        if !fx.truthy() {
            let dir = native_path(i, &path)?
                .parent()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_default();
            let fdx = f_file_exists_p(i, vec![Value::string(dir.clone())].into())?;
            if !fdx.truthy() {
                let s = i.q("end-of-file");
                return Err(i.signal_data(s, vec![Value::string("Error reading from stdin")]));
            }
        }
    }
    // GNU basic-save-buffer-2's writable checks before touching the disk:
    //   (if (not (file-writable-p file))
    //     (if (not (file-directory-p dir))  → "%s is not a directory" /
    //                                       "%s: No such directory"
    //         (not (file-exists-p file))  → "Directory %s write-protected"
    //         (yes-or-no-p ...)           → "Attempt to save to a file
    //                                        that you aren't allowed
    //                                        to write"))
    {
        let fw = f_file_writable_p(i, vec![Value::string(path.clone())].into())?;
        if !fw.truthy() {
            let dir = native_path(i, &path)?
                .parent()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_default();
            let dirp = f_file_directory_p(i, vec![Value::string(dir.clone())].into())?;
            if !dirp.truthy() {
                if native_path(i, &dir)?.exists() {
                    return Err(i.error(format!("{} is not a directory", dir)));
                }
                return Err(i.error(format!("{}: No such directory", dir)));
            }
            let fx = f_file_exists_p(i, vec![Value::string(path.clone())].into())?;
            if !fx.truthy() {
                return Err(i.error(format!("Directory {} write-protected", dir)));
            }
            // The file exists but isn't writable: GNU asks
            // "try to save anyway?" — in batch the read hits EOF.
            if i.noninteractive {
                let s = i.q("end-of-file");
                return Err(i.signal_data(s, vec![Value::string("Error reading from stdin")]));
            }
            return Err(i.error("Attempt to save to a file that you aren't allowed to write"));
        }
    }
    let len = b.borrow().text_len();
    let text = buffer_file_text(i, 0, len);
    write_file_string(
        i,
        &path,
        &text,
        &[
            Value::Nil,
            Value::Nil,
            Value::string(path.clone()),
            Value::Nil,
            Value::t(),
        ],
    )?;
    let last = i.q("last-coding-system-used");
    let used = i.symbol_value(last);
    let buffer_coding = i.q("buffer-file-coding-system");
    i.set_symbol(buffer_coding, used)?;
    Ok(Value::Nil)
}

fn f_write_file(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU files.el `write-file':
    //   (if (directory-name-p filename)      — name ends in "/" —
    //       (setq filename (concat filename
    //        (file-name-nondirectory (or buffer-file-name (buffer-name))))))
    //   (and confirm (file-exists-p filename)
    //        (or (y-or-n-p "File `%s' exists; overwrite? ")
    //            (user-error "Canceled")))
    //   → set-visited-file-name + basic-save-buffer.
    let mut fname = match &a[0] {
        Value::Str(s) => s.borrow().clone(),
        Value::Nil => {
            // GNU: nil/"" → "" signals via expand inside
            // set-visited-file-name; nil stays "" here too.
            (String::new()).into()
        }
        other => return Err(i.wrong_type_mut("stringp", &other.clone())),
    };
    if fname.ends_with('/') {
        let base = {
            let b = cur(i);
            let bb = b.borrow();
            bb.file_name.clone().unwrap_or_else(|| bb.name.clone())
        };
        let base = base.rsplit('/').next().unwrap_or(&base).to_string();
        fname.push_str(&base);
    }
    let confirm = a.get(1).map(|v| v.truthy()).unwrap_or(false);
    let expanded = expand_file_name_str(i, &fname);
    if confirm && native_path(i, &expanded)?.exists() {
        let msg = Value::string(format!("File `{fname}' exists; overwrite? "));
        let yonp = i.q("y-or-n-p");
        match i.apply(&Value::Sym(yonp), vec![msg]) {
            Err(fl) => return Err(fl),
            Ok(v) if !v.truthy() => {
                let ue = i.q("user-error");
                return Err(i.signal_data(ue, vec![Value::string("Canceled")]));
            }
            _ => {}
        }
    }
    f_set_visited_file_name(i, vec![Value::string(fname)].into())?;
    f_save_buffer(i, vec![].into())?;
    Ok(Value::Nil)
}

fn f_append_to_file(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    f_write_region(
        i,
        vec![a[0].clone(), a[1].clone(), a[2].clone(), Value::t()].into(),
    )
}

fn f_file_truename(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU files.el `file-truename' defun (compiled): the
    // `cond' coerces a symbol FILENAME via string= (its name), then
    // `substring' rejects it with `arrayp'; a cons/number/vector fails
    // string='s `stringp' check first.
    let p = match &a[0] {
        Value::Str(s) => s.borrow().clone(),
        Value::Nil | Value::Sym(_) => return Err(i.wrong_type_mut("arrayp", &a[0])),
        other => return Err(i.wrong_type_mut("stringp", other)),
    };
    // Resolve home/default directory while retaining components: GNU
    // follows a symlink before interpreting a following `..'.
    let p = {
        let ex = expand_file_name_raw_str(i, &p);
        if ex.is_empty() { "/".to_string() } else { (ex).to_unicode_lossy() }
    };
    // GNU Ffile_truename: COUNTER (arg2) is a cons whose car must be a
    // number — listp first on non-lists, then number-or-marker-p on its
    // car.
    match a.get(1) {
        None | Some(Value::Nil) => {}
        Some(Value::Cons(c)) => {
            let car = c.borrow().car.clone();
            match car {
                Value::Int(_) | Value::Float(_) | Value::Marker(_) => {}
                other => return Err(i.wrong_type_mut("number-or-marker-p", &other)),
            }
        }
        Some(other) => return Err(i.wrong_type_mut("listp", other)),
    }
    Ok(Value::string(crate::buffer::file_truename(&p)))
}

fn f_file_remote_p(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let file = want_str(i, &a[0])?;
    let identification = a.get(1).cloned().unwrap_or(Value::Nil);
    let connected = a.get(2).cloned().unwrap_or(Value::Nil);
    // GNU files.el `file-remote-p': dispatch through
    // `file-name-handler-alist' (Tramp supplies `tramp-file-name-handler',
    // which fills default user/host components like "root" for /sudo::).
    // When no handler claims the name — including inside
    // `tramp-run-real-handler', which inhibits all tramp handlers — GNU
    // returns nil; there is no syntactic fallback.  Answering non-nil
    // anyway made tramp-integration's shortdoc entry believe
    // "/ssh:user@host:/tmp/foo" was remote while `tramp-methods' was still
    // empty mid-load, so it probed "/sudo::" and aborted the tramp.el load.
    let op_sym = Value::Sym(i.q("file-remote-p"));
    if let Some(handler) = crate::buffer::extra::find_file_name_handler(i, &file, op_sym.clone()) {
        return i.apply(
            &handler,
            vec![
                op_sym,
                Value::string(&file),
                identification.clone(),
                connected.clone(),
            ],
        );
    }
    Ok(Value::Nil)
}

/// GNU `file-remote-p' with no arguments beyond FILE: the remote prefix
/// string, or nil.  Used by `file-local-name', which strips exactly the
/// prefix `file-remote-p' reports.
fn remote_prefix(i: &mut Interp, file: &EmacsStr) -> EvalResult {
    let op_sym = Value::Sym(i.q("file-remote-p"));
    match crate::buffer::extra::find_file_name_handler(i, file, op_sym.clone()) {
        Some(handler) => i.apply(&handler, vec![op_sym, Value::string(file)]),
        None => Ok(Value::Nil),
    }
}

fn f_file_equal_p(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU's Lisp wrapper checks the first raw name for a handler before
    // it examines the second name, and forwards both original values.
    let first = want_str(i, &a[0])?;
    if let Some((handler, op)) = file_handler(i, "file-equal-p", &[&first]) {
        return i.apply(&handler, vec![op, a[0].clone(), a[1].clone()]);
    }
    let second = want_str(i, &a[1])?;
    if let Some((handler, op)) = file_handler(i, "file-equal-p", &[&second]) {
        return i.apply(&handler, vec![op, a[0].clone(), a[1].clone()]);
    }
    let f1 = want_filename(i, &a[0])?;
    let f2 = want_filename(i, &a[1])?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        match (std::fs::metadata(native_path(i, &f1)?), std::fs::metadata(native_path(i, &f2)?)) {
            (Ok(m1), Ok(m2)) => Ok(Value::from_bool(
                m1.dev() == m2.dev() && m1.ino() == m2.ino(),
            )),
            _ => Ok(Value::Nil),
        }
    }
    #[cfg(not(unix))]
    Ok(Value::from_bool(
        crate::buffer::file_truename(&f1) == crate::buffer::file_truename(&f2),
    ))
}

fn f_file_system_info(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let path = want_filename(i, &a[0])?;
    if let Some((handler, op)) = file_handler(i, "file-system-info", &[&path]) {
        return i.apply(&handler, vec![op, file_name_argument(&a[0], &path)]);
    }
    #[cfg(unix)]
    {
        let c = match std::ffi::CString::new(path.to_external_bytes()) {
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
static DEFAULT_FILE_MODES: std::sync::atomic::AtomicI64 = std::sync::atomic::AtomicI64::new(-1);

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

fn f_default_file_modes(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let _ = (i, a);
    Ok(Value::Int(default_file_modes() as i128))
}

fn f_unix_sync(_i: &mut Interp, _a: crate::lisp::Argv) -> EvalResult {
    // GNU calls sync(2) and returns nil.
    #[cfg(unix)]
    unsafe {
        libc::sync();
    }
    Ok(Value::Nil)
}

fn f_file_name_quote(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let name = want_str(i, &a[0])?;
    // GNU: prepend the "/:" quotation prefix unless already quoted.
    if name.starts_with("/:") {
        Ok(Value::string(name))
    } else {
        Ok(Value::string(format!("/:{name}")))
    }
}

fn f_file_name_unquote(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let name = want_str(i, &a[0])?;
    // GNU fileio.c: a "/:" that quotes the filesystem root itself
    // unquotes to "/" rather than the empty string.
    let n = match name.strip_prefix("/:") {
        Some(rest) if rest.is_empty() => EmacsText::from("/"),
        Some(rest) => rest.to_string(),
        None => name,
    };
    Ok(Value::string(n))
}

fn f_file_local_name(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let file = want_str(i, &a[0])?;
    // GNU files.el `file-local-name': (let ((remote (file-remote-p name)))
    // (if remote (substring name (length remote)) name)).
    match remote_prefix(i, &file)? {
        Value::Str(pref) => {
            let p = pref.borrow();
            let plen = p.chars().count();
            let rest: EmacsText = file.chars().skip(plen).collect();
            Ok(Value::string(rest))
        }
        _ => Ok(Value::string(file)),
    }
}

fn f_unhandled_dir(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let file = want_str(i, &a[0])?;
    let operation = Value::Sym(i.q("unhandled-file-name-directory"));
    if let Some(handler) = crate::buffer::extra::find_file_name_handler(i, &file, operation.clone()) {
        return i.apply(&handler, vec![operation, a[0].clone()]);
    }
    // GNU: file-name-as-directory on the (local) name.
    if file.is_empty() {
        Ok(Value::string("./"))
    } else if file.ends_with('/') {
        Ok(Value::string(file))
    } else {
        Ok(Value::string(format!("{file}/")))
    }
}

/// chmod-style symbolic MODE string → numeric mode bits: a literal port
/// of GNU files.el's `file-modes-symbolic-to-number' +
/// `file-modes-rights-to-number' (kept a subr so `subrp' reports t like
/// the nativecomp GNU build).  Errors carry GNU's literal grave-quote
/// message — `(error ...)' does not translate to curly quotes.
fn f_modes_sym2num(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let modes = want_str(i, &a[0])?;
    // GNU `(or from 0)': FROM is never type-checked — an empty MODES
    // returns it verbatim; a bad type only errors when a clause's
    // logand/logior touches it.
    let mut num_modes: Value = match a.get(1) {
        Some(Value::Nil) | None => Value::Int(0),
        Some(v) => v.clone(),
    };
    // file-modes-char-to-who: u→0o4700 g→0o2070 o→0o1007 a→0o7777.
    fn who_bits(c: u8) -> i64 {
        match c {
            b'u' => 0o4700,
            b'g' => 0o2070,
            b'o' => 0o1007,
            _ => 0o7777,
        }
    }
    // file-modes-char-to-right; FROM is the running num-rights.
    fn right_bits(c: u8, from: i128) -> i128 {
        match c {
            b'r' => 0o444,
            b'w' => 0o222,
            b'x' => 0o111,
            b's' => 0o6000,
            b't' => 0o1000,
            b'X' => {
                if from & 0o111 == 0 {
                    0
                } else {
                    0o111
                }
            }
            // GNU's literal (FIXME-tagged) expressions — port as written.
            b'u' => {
                let ur = from & 0o4700;
                ur + ur / 0o10 + ur / 0o100
            }
            b'g' => {
                let gr = from & 0o2070;
                gr + gr / 0o10 + gr * 0o10
            }
            b'o' => {
                let or_ = from & 0o1007;
                or_ + or_ * 0o10 + or_ * 0o100
            }
            _ => unreachable!(),
        }
    }
    // file-modes-rights-to-number on a span of `([+=-][rwxXstugo]*)+'.
    // num_rights stays i128: GNU `lognot' is infinite-precision, so a
    // bignum FROM must survive `& !who_mask' intact.
    fn rights_to_number(rights: &[u8], who_mask: i64, from: i128) -> i128 {
        let mut num_rights = from;
        let mut p = 0usize;
        while p < rights.len() && matches!(rights[p], b'+' | b'-' | b'=') {
            let op = rights[p];
            p += 1;
            let mut num_right: i128 = 0;
            while p < rights.len()
                && matches!(
                    rights[p],
                    b'r' | b'w' | b'x' | b'X' | b's' | b't' | b'u' | b'g' | b'o'
                )
            {
                num_right |= right_bits(rights[p], num_rights);
                p += 1;
            }
            num_right &= who_mask as i128;
            num_rights = match op {
                b'+' => num_rights | num_right,
                b'-' => num_rights & !num_right,
                _ => (num_rights & !(who_mask as i128)) | num_right,
            };
        }
        num_rights
    }
    // Clause loop mirroring the GNU regexp
    // "^\\([ugoa]*\\)\\([+=-][rwxXstugo]*\\)+\\(,\\|\\)": optional who,
    // then one or more op-groups, ending at ',' or end of string.
    let b = modes.to_external_bytes();
    let end = b.len();
    let mut cstart = 0usize;
    while cstart < end {
        let who_start = cstart;
        let mut p = cstart;
        while p < end && matches!(b[p], b'u' | b'g' | b'o' | b'a') {
            p += 1;
        }
        let mut who: i64 = 0;
        for &wc in &b[who_start..p] {
            who |= who_bits(wc);
        }
        // Greedy op-group span.
        let ops_start = p;
        while p < end && matches!(b[p], b'+' | b'-' | b'=') {
            p += 1;
            while p < end
                && matches!(
                    b[p],
                    b'r' | b'w' | b'x' | b'X' | b's' | b't' | b'u' | b'g' | b'o'
                )
            {
                p += 1;
            }
        }
        if p == ops_start || (p < end && b[p] != b',') {
            // Clause match failed: the remaining MODES string is reported
            // from where this iteration (or the next, when op-groups
            // absorbed part of it) started.
            let near = if p == ops_start { cstart } else { p };
            // GNU `(error "Parse error in modes near `%s'" ...)': the
            // args-bearing error formats with format-message, so the
            // graves become curly quotes.
            return Err(i.error_q("Parse error in modes near `%s'", &[&modes[near..]]));
        }
        if who == 0 {
            who = 0o7000 | default_file_modes();
        }
        num_modes = match &num_modes {
            Value::Int(n) => Value::Int(rights_to_number(&b[ops_start..p], who, *n)),
            // GNU `(or from 0)' never checks FROM: the first logand inside
            // file-modes-rights-to-number signals integerp on it.
            other => return Err(i.wrong_type_mut("integerp", other)),
        };
        cstart = p + 1; // skip ',' — a trailing comma ends the loop
    }
    Ok(num_modes)
}

fn f_set_default_file_modes(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let mode = crate::lisp::builtins::want_fixnum(i, &a[0])?;
    // GNU sets the process umask to ~MODE & 0777 and records MODE in
    // the `default-file-modes' static (fileio.c).
    #[cfg(unix)]
    unsafe {
        libc::umask((!mode & 0o777) as libc::mode_t);
    }
    DEFAULT_FILE_MODES.store(mode as i64, std::sync::atomic::Ordering::Relaxed);
    Ok(Value::Nil)
}
fn f_car_less_than_car(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU Fcar_less_than_car extracts both cars (listp check on
    // the args) then calls Flessp — the cars must be number-or-marker.
    let car_of = |i: &mut Interp, v: &Value| -> Result<Value, Flow> {
        match v {
            Value::Cons(c) => Ok(c.borrow().car.clone()),
            Value::Nil => Ok(Value::Nil),
            other => Err(i.wrong_type_mut("listp", other)),
        }
    };
    let x = car_of(i, &a[0])?;
    let y = car_of(i, &a[1])?;
    // Exact comparison (no f64 widening on bignum ints).
    enum Num {
        I(i128),
        F(f64),
    }
    let num = |i: &mut Interp, v: &Value| -> Result<Num, Flow> {
        match v {
            Value::Int(n) => Ok(Num::I(*n)),
            Value::Float(f) => Ok(Num::F(**f)),
            Value::Marker(m) => Ok(Num::I(m.borrow().position as i128 + 1)),
            other => Err(i.wrong_type_mut("number-or-marker-p", other)),
        }
    };
    let (nx, ny) = (num(i, &x)?, num(i, &y)?);
    let less = match (&nx, &ny) {
        (Num::I(p), Num::I(q)) => p < q,
        (Num::I(p), Num::F(q)) => (*p as f64) < *q,
        (Num::F(p), Num::I(q)) => *p < (*q as f64),
        (Num::F(p), Num::F(q)) => p < q,
    };
    Ok(Value::from_bool(less))
}

// ---------- processes ----------

/// GNU get_current_directory: use the launching buffer's directory,
/// with the local directory (or home) supplied by a file-name handler.
/// Validate in the parent so failures carry the original Lisp name.
pub(crate) fn process_directory(i: &mut Interp) -> Result<EmacsText, Flow> {
    let symbol = i.q("default-directory");
    let original = i.symbol_value(symbol);
    let raw = want_str(i, &original)?;
    let unhandled = f_unhandled_dir(i, vec![original.clone()].into())?;
    let name = if unhandled.is_nil() {
        EmacsText::from("~")
    } else {
        want_str(i, &unhandled)?
    };
    // GNU expands a relative default-directory against the directory
    // of the executable before using it as the expansion base.
    let invocation = i
        .intern_soft("invocation-directory")
        .map(|symbol| i.symbol_value(symbol));
    let invocation = match invocation {
        Some(Value::Str(value)) => value.borrow().clone(),
        _ => EmacsText::from(emacs_wd()),
    };
    let base = expand_against_dir(i, &raw, &invocation);
    let directory = expand_against_dir(i, &name, &base);
    let directory = directory.strip_prefix("/:").unwrap_or(&directory);
    let directory = directory.split('\0').next().unwrap_or_default().to_string();
    let native = native_path(i, &directory)?;
    let accessible = (|| -> std::io::Result<()> {
        let metadata = std::fs::metadata(&native)?;
        if !metadata.is_dir() {
            return Err(std::io::Error::from_raw_os_error(libc::ENOTDIR));
        }
        let path = std::ffi::CString::new(directory.to_external_bytes()).unwrap();
        if unsafe { libc::access(path.as_ptr(), libc::X_OK) } != 0 {
            return Err(std::io::Error::last_os_error());
        }
        Ok(())
    })();
    accessible.map_err(|error| {
        report_file_errno(i, "Setting current directory", vec![original], &error)
    })?;
    Ok(directory)
}

/// Search exec-path in order, like GNU openp. A nil path/element uses
/// the launching directory; other elements follow expand-file-name's
/// directory rules. Only make-process bypasses the path for an absolute
/// program (call-process still needs a list to search).
pub(crate) fn dif_resolve_program<P: TextSource + ?Sized, D: TextSource + ?Sized>(
    i: &mut Interp,
    prog: &P,
    directory: &D,
    direct_absolute: bool,
) -> Result<EmacsText, Flow> {
    prog.with_text(|prog| directory.with_text(|directory| {
    let mut path = i
        .intern_soft("exec-path")
        .map(|symbol| i.symbol_value(symbol))
        .unwrap_or(Value::Nil);
    let absolute = f_file_name_absolute_p(i, vec![Value::string(prog)].into())?.truthy();
    if path.is_nil() || (direct_absolute && absolute) {
        path = Value::list(vec![Value::Nil]);
    }
    // Keep nil positions and dotted prefixes. Bound circular input to
    // its distinct cons cells rather than getting stuck collecting it.
    let mut entries = Vec::new();
    let mut seen = std::collections::HashSet::new();
    while let Value::Cons(cell) = path {
        if !seen.insert(Rc::as_ptr(&cell)) {
            break;
        }
        let cell = cell.borrow();
        entries.push(cell.car.clone());
        path = cell.cdr.clone();
    }
    let mut last_error = std::io::Error::from_raw_os_error(libc::ENOENT);
    for entry in entries {
        let base = match entry {
            Value::Nil => directory.to_string(),
            Value::Str(value) => expand_against_dir(i, &value.borrow(), directory),
            // GNU expand-file-name treats a non-string non-nil base as /.
            _ => EmacsText::from("/"),
        };
        let candidate = expand_against_dir(i, prog, &base);
        let candidate = candidate.strip_prefix("/:").unwrap_or(&candidate);
        let candidate = candidate.split('\0').next().unwrap_or_default();
        let native = native_path(i, candidate)?;
        let accessible = (|| -> std::io::Result<()> {
            let path = std::ffi::CString::new(candidate.to_external_bytes()).unwrap();
            if unsafe {
                libc::faccessat(libc::AT_FDCWD, path.as_ptr(), libc::X_OK, libc::AT_EACCESS)
            } != 0
            {
                return Err(std::io::Error::last_os_error());
            }
            if std::fs::metadata(&native)?.is_dir() {
                return Err(std::io::Error::from_raw_os_error(libc::EISDIR));
            }
            Ok(())
        })();
        match accessible {
            Ok(()) => return Ok(candidate.to_string()),
            Err(error) => {
                // A later missing entry does not erase a more specific
                // failure, such as EACCES, EISDIR or ELOOP.
                if !matches!(error.raw_os_error(), Some(libc::ENOENT | libc::ENOTDIR)) {
                    last_error = error;
                }
            }
        }
        if absolute {
            break;
        }
    }
    Err(report_file_errno(
        i,
        "Searching for program",
        vec![Value::string(prog)],
        &last_error,
    ))

    }))
}

/// A private, uniquely-created capture file. Duplicated descriptors share
/// its offset, preserving the child's stdout/stderr write order. Keep the
/// file open for reading and remove it on every success/error exit.
struct SyncOutputCapture {
    file: Option<std::fs::File>,
    path: std::path::PathBuf,
}

impl Drop for SyncOutputCapture {
    fn drop(&mut self) {
        self.file.take();
        let _ = std::fs::remove_file(&self.path);
    }
}

impl SyncOutputCapture {
    fn new() -> std::io::Result<Self> {
        use std::sync::atomic::{AtomicU64, Ordering};
        static NEXT: AtomicU64 = AtomicU64::new(0);
        loop {
            let path = std::env::temp_dir().join(format!(
                "remacs-output-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            let mut options = std::fs::OpenOptions::new();
            options.read(true).write(true).create_new(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt;
                options.mode(0o600);
            }
            match options.open(&path) {
                Ok(file) => {
                    return Ok(Self {
                        file: Some(file),
                        path,
                    });
                }
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(error),
            }
        }
    }

    fn read(&mut self) -> std::io::Result<Vec<u8>> {
        use std::io::{Read, Seek, SeekFrom};
        let file = self.file.as_mut().unwrap();
        file.seek(SeekFrom::Start(0))?;
        let mut output = Vec::new();
        file.read_to_end(&mut output)?;
        Ok(output)
    }
}

fn sync_process_destination(
    i: &mut Interp,
    command: &mut std::process::Command,
    destination: &Value,
) -> Result<(Value, Option<SyncOutputCapture>), Flow> {
    let file_keyword = i.q(":file");
    let (real_destination, stderr) = match destination {
        Value::Cons(cell) if !matches!(&cell.borrow().car, Value::Sym(s) if *s == file_keyword) => {
            let cell = cell.borrow();
            // Only an explicit second element overrides the default.
            // (DEST), (DEST . t), and other dotted tails still merge.
            let stderr = match &cell.cdr {
                Value::Cons(second) => second.borrow().car.clone(),
                _ => Value::t(),
            };
            (cell.car.clone(), stderr)
        }
        other => (other.clone(), Value::t()),
    };
    command.stdout(std::process::Stdio::piped());
    match stderr {
        Value::Nil => {
            command.stderr(std::process::Stdio::null());
            Ok((real_destination, None))
        }
        Value::Str(path) => {
            let path = expand_file_name_str(i, &path.borrow());
            let file = std::fs::File::create(native_path(i, &path)?).map_err(|error| {
                report_file_errno(
                    i,
                    "Opening process output file",
                    vec![Value::string(&path)],
                    &error,
                )
            })?;
            command.stderr(std::process::Stdio::from(file));
            Ok((real_destination, None))
        }
        _ => {
            let capture = SyncOutputCapture::new().map_err(|error| {
                i.error(format!("Cannot create process output capture: {error}"))
            })?;
            let file = capture.file.as_ref().unwrap();
            let stdout = file
                .try_clone()
                .map_err(|error| i.error(error.to_string()))?;
            let stderr = file
                .try_clone()
                .map_err(|error| i.error(error.to_string()))?;
            command.stdout(std::process::Stdio::from(stdout));
            command.stderr(std::process::Stdio::from(stderr));
            Ok((real_destination, Some(capture)))
        }
    }
}

fn sync_process_output(
    i: &mut Interp,
    output: &[u8],
    capture: &mut Option<SyncOutputCapture>,
) -> Result<String, Flow> {
    let bytes = match capture {
        Some(capture) => capture.read().map_err(|error| i.error(error.to_string()))?,
        None => output.to_vec(),
    };
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

fn deliver_sync_output(i: &mut Interp, destination: &Value, output: &str) -> Result<(), Flow> {
    if let Value::Cons(cell) = destination {
        let cell = cell.borrow();
        if matches!(&cell.car, Value::Sym(s) if i.symbol_name(*s) == ":file") {
            let path = match &cell.cdr {
                Value::Cons(file) => file.borrow().car.clone(),
                other => other.clone(),
            };
            let path = want_str(i, &path)?;
            let path = expand_file_name_str(i, &path);
            return std::fs::write(native_path(i, &path)?, output).map_err(|error| {
                report_file_errno(
                    i,
                    "Opening process output file",
                    vec![Value::string(path)],
                    &error,
                )
            });
        }
    }
    if destination.is_nil() || destination.int().is_some() {
        return Ok(());
    }
    let bid = if matches!(destination, Value::Sym(s) if *s == sym::T) {
        i.current_buffer
    } else {
        match i.buffer_id_of(destination) {
            Some(buffer) => buffer,
            None => match destination {
                Value::Str(name) => i.create_buffer(&name.borrow()),
                _ => i.current_buffer,
            },
        }
    };
    crate::buffer::primitives::chg_with_buffer(i, bid, |i| {
        crate::buffer::primitives::chg_insert_pt(i, output, false)
    })?;
    Ok(())
}

fn f_call_process(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let prog = want_str(i, &a[0])?;
    // (call-process PROGRAM &optional INFILE DESTINATION DISPLAY &rest ARGS)
    // GNU arg checks, in order: PROGRAM string; INFILE (nil-or-string);
    // the rest ARGS (strings); then the stderr spec in DESTINATION's
    // cdr-car; only then exec-path lookup.  DISPLAY isn't type-checked.
    {
        let infile = &arg(&a, 1);
        if !matches!(infile, Value::Nil) {
            want_str(i, infile)?;
        }
    }
    for v in a.get(4..).unwrap_or(&[]) {
        want_str(i, v)?;
    }
    // DESTINATION check, GNU callproc.c order (before exec-path
    // resolution): see `dest_check' for the acceptance rules.
    dest_check(i, &arg(&a, 2))?;
    let directory = process_directory(i)?;
    // GNU callproc.c order: INFILE is opened BEFORE the exec-path
    // search — a missing input file reports "Opening process input
    // file" even when the program itself doesn't exist.
    let infile_handle = if let Some(infile) = a.get(1) {
        match infile {
            Value::Str(f) => {
                let f = expand_against_dir(i, &f.borrow(), &directory);
                match std::fs::File::open(native_path(i, &f)?) {
                    Ok(fh) => Some(fh),
                    Err(e) => {
                        let (sym, msg) = if e.raw_os_error() == Some(13) {
                            ("permission-denied", "Permission denied")
                        } else {
                            ("file-missing", "No such file or directory")
                        };
                        let full = expand_file_name_str(i, &f);
                        let sid = i.intern(sym);
                        return Err(i.signal_data(
                            sid,
                            vec![
                                Value::string("Opening process input file"),
                                Value::string(msg),
                                Value::string(full),
                            ],
                        ));
                    }
                }
            }
            // GNU: INFILE is nil-or-string only (no integer).
            Value::Nil => None,
            other => return Err(i.wrong_type_mut("stringp", other)),
        }
    } else {
        None
    };
    let resolved = dif_resolve_program(i, &prog, &directory, false)?;
    let mut cmd = std::process::Command::new(native_path(i, &resolved)?);
    cmd.current_dir(native_path(i, &directory)?);
    crate::lisp::process::environment::apply(i, &mut cmd);
    for v in a.get(4..).unwrap_or(&[]) {
        let argument = want_str(i, v)?;
        cmd.arg(native_path(i, &argument)?);
    }
    let (real_dest, mut capture) = sync_process_destination(i, &mut cmd, &arg(&a, 2))?;
    if let Some(fh) = infile_handle {
        cmd.stdin(std::process::Stdio::from(fh));
    }
    let file_err = i.q("file-error");
    let output = cmd.output();
    drop(cmd);
    let output = output.map_err(|e| {
        i.signal_data(
            file_err,
            vec![
                Value::string(format!("Running {resolved}")),
                Value::string(file_error_string(&e)),
            ],
        )
    })?;
    let stdout = sync_process_output(i, &output.stdout, &mut capture)?;
    if real_dest.int().is_some() {
        return Ok(Value::Nil);
    }
    deliver_sync_output(i, &real_dest, &stdout)?;
    Ok(Value::Int(output.status.code().unwrap_or(1) as i128))
}

/// GNU call-process-region destination acceptance (callproc.c):
/// nil/t/int/string/buffer; conses take car ∈ nil/t/int/string/buffer
/// and stderr-spec ∈ nil/t/string.  The `(:file FILE)' form's FILE
/// must be a string.
fn dest_ok_tib(v: &Value) -> bool {
    match v {
        Value::Nil | Value::Int(_) | Value::Str(_) | Value::Buffer(_) => true,
        Value::Sym(s) => *s == sym::T,
        _ => false,
    }
}
fn dest_check(i: &mut Interp, dest: &Value) -> Result<(), Flow> {
    let file_sym = i.q(":file");
    match dest {
        Value::Cons(c) => {
            let (car_v, cadr_v) = {
                let cb = c.borrow();
                let cadr = match &cb.cdr {
                    Value::Cons(cc) => cc.borrow().car.clone(),
                    other => other.clone(),
                };
                (cb.car.clone(), cadr)
            };
            if matches!(&car_v, Value::Sym(s) if *s == file_sym) {
                if !matches!(&cadr_v, Value::Str(_)) {
                    return Err(i.wrong_type_mut("stringp", &cadr_v));
                }
            } else {
                // REAL-DEST may itself be `(:file FILE)'.
                let car_ok = match &car_v {
                    Value::Cons(cc) => {
                        let b = cc.borrow();
                        matches!(&b.car, Value::Sym(s) if *s == file_sym)
                            && matches!(&b.cdr, Value::Cons(ic) if matches!(&ic.borrow().car, Value::Str(_)))
                    }
                    other => dest_ok_tib(other),
                };
                if !car_ok {
                    return Err(i.wrong_type_mut("stringp", &car_v));
                }
                if matches!(&c.borrow().cdr, Value::Cons(_))
                    && !matches!(&cadr_v, Value::Nil | Value::Str(_))
                    && !matches!(&cadr_v, Value::Sym(s) if *s == sym::T)
                {
                    return Err(i.wrong_type_mut("stringp", &cadr_v));
                }
            }
            Ok(())
        }
        other => {
            if dest_ok_tib(other) {
                Ok(())
            } else {
                Err(i.wrong_type_mut("stringp", other))
            }
        }
    }
}

fn f_call_process_region(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // (call-process-region START END PROGRAM &optional DELETE DESTINATION
    //  DISPLAY &rest ARGS)
    // GNU order (callproc.c Fcall_process_region): START may be a
    // string (literal input — END ignored), nil (current buffer's whole
    // text), or a number-or-marker region which validate_region
    // bounds-checks.  A non-empty input goes through create_temp_file →
    // Ffind_operation_coding_system, whose target is args[2] — a
    // non-string PROGRAM there errors "Invalid argument 3 of operation
    // `call-process-region'"; an empty input falls through to
    // call_process which CHECK_STRINGs PROGRAM (wrong-type stringp).
    let empty_input = match &a[0] {
        Value::Str(s) => s.borrow().is_empty(),
        Value::Nil => {
            let b = cur(i);
            let bb = b.borrow();
            bb.text.len() == 0
        }
        Value::Int(_) | Value::Marker(_) => {
            // validate_region on the current buffer.
            let b = cur(i);
            let bb = b.borrow();
            let len = bb.text.len();
            let (s, e) = region_bounds(i, &a[0], &a[1], len)?;
            s == e
        }
        other => return Err(i.wrong_type_mut("integer-or-marker-p", other)),
    };
    if !empty_input && !matches!(&a[2], Value::Str(_)) {
        return Err(i.error("Invalid argument 3 of operation `call-process-region'"));
    }
    // GNU deletes the input AFTER extracting it (callproc.c:
    // Fcall_process_region substring→delete_region→call_process): a
    // string START goes through Fdelete_region → validate_region, so it
    // signals `integer-or-marker-p' BEFORE the program is spawned; nil
    // START deletes the whole buffer (post-widen), int/marker bounds
    // delete the region.  Bounds for the delete are validated while the
    // text is extracted — re-checking them post-delete would compare
    // against the already-emptied buffer.
    let text = match &a[0] {
        Value::Str(s) => s.borrow().clone(),
        Value::Nil => {
            let b = cur(i);
            b.borrow().text.text()
        }
        _ => {
            let b = cur(i);
            let bb = b.borrow();
            let len = bb.text.len();
            let (s, e) = region_bounds(i, &a[0], &a[1], len)?;
            bb.text.substring(s.min(e), s.max(e))
        }
    };
    if a.get(3).map(|v| v.truthy()).unwrap_or(false) {
        match &a[0] {
            Value::Str(_) => {
                return Err(i.wrong_type_mut("integer-or-marker-p", &a[0]));
            }
            Value::Nil => {
                let b = cur(i);
                let z = b.borrow().text.len();
                {
                    let mut bb = b.borrow_mut();
                    bb.begv = 0;
                    bb.zv = z;
                    let pt = bb.point().min(z);
                    bb.set_point(pt);
                }
                crate::buffer::primitives::chg_delete(i, 0, z)?;
            }
            Value::Int(_) | Value::Marker(_) => {
                let b = cur(i);
                let len = b.borrow().text.len();
                let (s, e) = region_bounds(i, &a[0], &a[1], len)?;
                crate::buffer::primitives::chg_delete(i, s.min(e), s.max(e))?;
            }
            _ => {}
        }
    }
    let prog = want_str(i, &a[2])?;
    dest_check(i, &arg(&a, 4))?;
    for v in a.get(6..).unwrap_or(&[]) {
        want_str(i, v)?;
    }
    let directory = process_directory(i)?;
    let prog = dif_resolve_program(i, &prog, &directory, false)?;
    let mut cmd = std::process::Command::new(native_path(i, &prog)?);
    cmd.current_dir(native_path(i, &directory)?);
    crate::lisp::process::environment::apply(i, &mut cmd);
    cmd.stdin(std::process::Stdio::piped());
    let (real_dest, mut capture) = sync_process_destination(i, &mut cmd, &arg(&a, 4))?;
    for v in a.get(6..).unwrap_or(&[]) {
        let argument = want_str(i, v)?;
        cmd.arg(native_path(i, &argument)?);
    }
    let spawned = cmd.spawn();
    drop(cmd);
    let mut child = match spawned {
        Ok(c) => c,
        Err(e) => {
            let data = if e.kind() == std::io::ErrorKind::NotFound {
                vec![
                    Value::string("Searching for program"),
                    Value::string("No such file or directory"),
                    Value::string(prog.clone()),
                ]
            } else {
                vec![Value::string(format!("Doing exec: {}", e))]
            };
            let sym_id = if e.kind() == std::io::ErrorKind::NotFound {
                i.q("file-missing")
            } else {
                i.q("file-error")
            };
            return Err(i.signal_data(sym_id, data));
        }
    };
    {
        use std::io::Write;
        if let Some(mut stdin) = child.stdin.take() {
            let _ = stdin.write_all(text.to_external_bytes().as_slice());
        }
    }
    let output = child
        .wait_with_output()
        .map_err(|e| i.signal_data(sym::FILE_ERROR, vec![Value::string(e.to_string())]))?;
    let stdout = sync_process_output(i, &output.stdout, &mut capture)?;
    // (The DELETE delete happened above, before spawn — GNU order.)
    if real_dest.int().is_some() {
        return Ok(Value::Nil);
    }
    deliver_sync_output(i, &real_dest, &stdout)?;
    Ok(Value::Int(output.status.code().unwrap_or(1) as i128))
}

/// Build a shell invocation from the current Lisp bindings, like the
/// call-process-shell-command wrappers. Keep exec-path and child-directory
/// lookup consistent with other synchronous processes (#295).
fn shell_process_command(i: &mut Interp, cmdline: &str) -> Result<std::process::Command, Flow> {
    let shell_symbol = i.q("shell-file-name");
    let shell_value = i.eval(&Value::Sym(shell_symbol))?;
    let shell = want_str(i, &shell_value)?;
    let switch_symbol = i.q("shell-command-switch");
    let switch_value = i.eval(&Value::Sym(switch_symbol))?;
    let switch = want_str(i, &switch_value)?;
    let directory = process_directory(i)?;
    let shell = dif_resolve_program(i, &shell, &directory, false)?;
    let mut command = std::process::Command::new(native_path(i, &shell)?);
    command.current_dir(native_path(i, &directory)?);
    crate::lisp::process::environment::apply(i, &mut command);
    command.arg(native_path(i, &switch)?).arg(cmdline);
    Ok(command)
}

fn f_shell_command(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let cmdline = want_str(i, &a[0])?;
    let mut command = shell_process_command(i, &(cmdline).to_unicode_lossy())?;
    let mut capture = if arg(&a, 2).is_nil() {
        sync_process_destination(i, &mut command, &Value::t())?.1
    } else {
        None
    };
    let out = command.output();
    drop(command);
    match out {
        Ok(o) => {
            let stdout = sync_process_output(i, &o.stdout, &mut capture)?;
            let stderr = String::from_utf8_lossy(&o.stderr).to_owned();
            let dest = arg(&a, 1);
            let bid = if dest.truthy() && !i.sym_id(&dest).map(|s| s == sym::T).unwrap_or(false) {
                i.buffer_id_of(&dest).unwrap_or(i.current_buffer)
            } else {
                i.buffers
                    .by_name("*Shell Command Output*")
                    .unwrap_or_else(|| i.create_buffer("*Shell Command Output*"))
            };
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
                    crate::lisp::builtins::evalfn::signal_after_change(i, 1, new_len + 1, old_len as i128)
                })?;
            }
            // ERROR-BUFFER captures stderr separately.
            let errb = arg(&a, 2);
            if errb.truthy() {
                if let Some(eid) = i.buffer_id_of(&errb) {
                    if i.buffers.get(eid).is_some() {
                        let s = stderr.clone();
                        crate::buffer::primitives::chg_with_buffer(i, eid, |i| {
                            let b = i.buffers.get(eid).unwrap();
                            let old_len = b.borrow().text.len();
                            crate::lisp::builtins::evalfn::signal_before_change(i, 1, old_len + 1)?;
                            {
                                let mut bb = b.borrow_mut();
                                bb.text.set_text(&s);
                                bb.zv = bb.text.len();
                            }
                            let new_len = s.chars().count();
                            crate::lisp::builtins::evalfn::signal_after_change(
                                i,
                                1,
                                new_len + 1,
                                old_len as i128,
                            )
                        })?;
                    }
                }
            }
            Ok(Value::Int(o.status.code().unwrap_or(1) as i128))
        }
        Err(e) => Err(i.error(&e.to_string())),
    }
}

fn f_shell_command_to_string(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let cmdline = want_str(i, &a[0])?;
    // GNU callproc.c: with no error-buffer argument the child's stderr
    // is merged into stdout ("discard" would need an explicit
    // error-file spec, which shell-command-to-string doesn't pass).
    let mut command = shell_process_command(i, &(cmdline).to_unicode_lossy())?;
    let (_, mut capture) = sync_process_destination(i, &mut command, &Value::t())?;
    let result = command.output();
    drop(command);
    match result {
        Ok(output) => Ok(Value::string(sync_process_output(
            i, &output.stdout, &mut capture,
        )?)),
        Err(e) => Err(i.error(&e.to_string())),
    }
}

// ---------- editing commands ----------

fn f_kill_whole_line(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU simple.el: `(or arg (setq arg 1))' then
    // numeric comparisons — a non-number arg is `(number-or-marker-p A)'.
    let av = arg(&a, 0);
    let n = match &av {
        Value::Int(v) => *v,
        v @ Value::Marker(_) => want_num_or_mark(i, v)?,
        Value::Nil => 1,
        v => return Err(i.wrong_type_mut("number-or-marker-p", v)),
    };
    {
        let b = cur(i);
        let bb = b.borrow();
        // GNU: `(and (> arg 0) (eobp) (save-excursion (forward-visible-line
        // 0) (eobp)))' signals end-of-buffer; point is at ZV and already
        // at a line start, so the whole current line is empty.
        if n > 0
            && bb.point() >= bb.zv
            && (bb.point() == 0 || bb.text.char_at(bb.point() - 1) == '\n')
        {
            drop(bb);
            return Err(err_sym(i, "end-of-buffer", vec![]));
        }
        // `(and (< arg 0) (bobp) (save-excursion (end-of-visible-line)
        // (bobp)))' signals beginning-of-buffer: empty buffer.
        if n < 0 && bb.point() <= bb.begv && bb.begv >= bb.zv {
            drop(bb);
            return Err(err_sym(i, "beginning-of-buffer", vec![]));
        }
    }
    let b = cur(i);
    let (s, e, tlen) = {
        let bb = b.borrow_mut();
        let line = bb.text.line_of_pos(bb.point());
        let s = bb.text.line_start(line);
        let mut e = s;
        for _ in 0..n.max(1) {
            e = bb.text.line_end(e);
            if e < bb.text.len() {
                e += 1;
            }
        }
        (s, e, bb.text.len())
    };
    // GNU kill-whole-line's Lisp calls kill-region for the delete+push,
    // which handles merging and this-command.
    crate::buffer::primitives::f_kill_region_pub(
        i,
        vec![
            Value::Int(s as i128 + 1),
            Value::Int(e.min(tlen) as i128 + 1),
        ]
        .into(),
    )?;
    cur(i).borrow_mut().set_point(s);
    // GNU's Lisp `kill-whole-line' ends with `(set-marker regions-begin
    // nil)' which RETURNS the (now-detached) marker, not nil.
    Ok(
        crate::lisp::value::Value::Marker(crate::lisp::memory::allocate(
            crate::lisp::value::Marker {
                buffer: None,
                position: 0,
                insertion_type: false,
            },
            crate::lisp::Value::Marker,
        ))
        .tracked(),
    )
}

fn f_kill_word(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU Fkill_word: count decodes with CHECK_FIXNUM —
    // nil and non-numbers signal fixnump.
    let n = crate::lisp::builtins::want_fixnum(i, &arg(&a, 0))?;
    let syn = syntax_table_entries(i);
    let wordp = |c: EmacsChar| syntax_entry_code(syn.as_ref(), c) == b'w';
    let b = cur(i);
    let bb = b.borrow_mut();
    let start = bb.point();
    let len = bb.text_len();
    let mut p = start;
    if n >= 0 {
        for _ in 0..n {
            while p < len && !wordp(bb.text.char_at(p)) {
                p += 1;
            }
            while p < len && wordp(bb.text.char_at(p)) {
                p += 1;
            }
        }
    } else {
        // GNU forward-word with a negative count moves backward.
        for _ in 0..-n {
            while p > 0 && !wordp(bb.text.char_at(p - 1)) {
                p -= 1;
            }
            while p > 0 && wordp(bb.text.char_at(p - 1)) {
                p -= 1;
            }
        }
    }
    drop(bb);
    // GNU kill-word is `(kill-region (point) (progn (forward-word arg)
    // (point)))' — route through f_kill_region so consecutive kills
    // merge like GNU's command loop.
    crate::buffer::primitives::f_kill_region_pub(
        i,
        vec![Value::Int(start as i128 + 1), Value::Int(p as i128 + 1)].into(),
    )?;
    Ok(Value::Nil)
}

fn f_backward_kill_word(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let n = want_num_or_mark(i, &arg(&a, 0))?;
    let syn = syntax_table_entries(i);
    let wordp = |c: EmacsChar| syntax_entry_code(syn.as_ref(), c) == b'w';
    let b = cur(i);
    let bb = b.borrow_mut();
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
    // GNU backward-kill-word = kill-word with a negative arg: the call is
    // (kill-region old-point new-point) with old-point > new-point, so
    // `(< end beg)' is t and the kill PREPENDS to the ring head.
    crate::buffer::primitives::f_kill_region_pub(
        i,
        vec![Value::Int(end as i128 + 1), Value::Int(p as i128 + 1)].into(),
    )?;
    cur(i).borrow_mut().set_point(p);
    Ok(Value::Nil)
}

fn f_just_one_space(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let n = arg(&a, 0).int().unwrap_or(1).max(0) as usize;
    let b = cur(i);
    let bb = b.borrow_mut();
    let p = bb.point();
    let mut s = p;
    let mut e = p;
    while s > 0 && (bb.text.char_at(s - 1) == ' ' || bb.text.char_at(s - 1) == '\t') {
        s -= 1;
    }
    while e < bb.text.len() && (bb.text.char_at(e) == ' ' || bb.text.char_at(e) == '\t') {
        e += 1;
    }
    drop(bb);
    crate::buffer::primitives::chg_delete(i, s, e)?;
    cur(i).borrow_mut().set_point(s);
    crate::buffer::primitives::chg_insert_pt(i, &" ".repeat(n), false)?;
    Ok(Value::Nil)
}

fn f_zap_to_char(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU simple.el's Lisp `zap-to-char': (interactive nil in batch) →
    // `char-to-string' fires `characterp' on CHAR BEFORE ARG reaches
    // `search-forward' (whose REPEAT wants `integer-or-marker-p').
    // A negative count searches backward; failure signals
    // `search-failed' with the char printed as a string.
    let ch = match &a[1] {
        Value::Int(c) if (0..=0x3f_ffff).contains(c) => *c as u32,
        other => return Err(i.wrong_type_mut("characterp", other)),
    };
    let n = match &a[0] {
        Value::Int(n) => *n,
        other => return Err(i.wrong_type_mut("fixnump", other)),
    };
    let c = char::from_u32(ch).unwrap_or('\0');
    let _kill_flag = a.get(2).map(|v| v.truthy()).unwrap_or(false);
    let b = cur(i);
    let bb = b.borrow_mut();
    let p = bb.point();
    let mut lo = p;
    let mut hi = p;
    if n > 0 {
        let mut found = p;
        let mut count = 0;
        while found < bb.text.len() {
            if bb.text.char_at(found) == c {
                count += 1;
                if count >= n {
                    break;
                }
            }
            found += 1;
        }
        if count < n {
            let sf = i.q("search-failed");
            return Err(i.signal_data(sf, vec![Value::string(c.to_string())]));
        }
        hi = found + 1;
    } else if n < 0 {
        // GNU searches backward; the nth match's end is the delete start,
        // point's char-index is the delete end.
        let mut found = p as i64 - 1;
        let mut count = 0;
        while found >= 0 {
            if bb.text.char_at(found as usize) == c {
                count += 1;
                if count >= -n {
                    break;
                }
            }
            found -= 1;
        }
        if count < -n {
            let sf = i.q("search-failed");
            return Err(i.signal_data(sf, vec![Value::string(c.to_string())]));
        }
        lo = found as usize + 1;
        hi = p;
    }
    drop(bb);
    // GNU zap-to-char kills via `(kill-region (point) match-pos)' —
    // point first, the match position second, so a backward zap's
    // `(< end beg)' is t and the kill PREPENDS to the ring head.
    crate::buffer::primitives::f_kill_region_pub(
        i,
        vec![Value::Int(p as i128 + 1), Value::Int(hi as i128 + 1)].into(),
    )?;
    if n <= 0 {
        cur(i).borrow_mut().set_point(lo);
    }
    Ok(Value::Nil)
}

pub(crate) fn region_op(
    i: &mut Interp,
    a: &[Value],
    op: crate::lisp::builtins::strfn::CaseOp,
) -> EvalResult {
    use crate::lisp::builtins::strfn::{apply_case_text, case_extra, case_str_with_widths};
    let b = cur(i);
    let down = i.current_case_table();
    let up = case_extra(&down, 0).unwrap_or_else(|| down.clone());
    let title = match op {
        crate::lisp::builtins::strfn::CaseOp::Cap | crate::lisp::builtins::strfn::CaseOp::CapUp => {
            crate::lisp::builtins::charset::unicode_prop_table(i, "titlecase")
        }
        _ => Value::Nil,
    };
    let (s, e, old) = {
        let bb = b.borrow();
        let len = bb.text.len();
        // GNU casify_pnc_region: a non-nil REGION-NONCONTIGUOUS-P
        // ignores BEG/END and casifies `(region-extract-function
        // 'bounds)' — the mark/point stretch, erroring without a mark.
        let (s, e) = if a.get(2).map(|v| v.truthy()).unwrap_or(false) {
            let (lo, hi) = crate::buffer::primitives::mark_region_bounds(i)?;
            (lo.min(len), hi.min(len))
        } else {
            region_bounds_strict(i, &a[0], &a[1], len)?
        };
        let (s, e) = (s.min(e), s.max(e));
        (s, e, bb.text.substring(s, e))
    };
    let (new, widths) = case_str_with_widths(i, &old, op, &down, &up, &title);
    // GNU `casify_region': `modify_text' fires whenever the region is
    // non-empty (before-change + MODIFF bump even when nothing
    // changed); `after-change' fires only when case actually changed,
    // spanning first..last changed char.
    crate::lisp::builtins::evalfn::signal_before_change(i, s + 1, e + 1)?;
    let old_len = e - s;
    let n_new = new.chars().count();
    let changed = new != old;
    let (first, last_old) = if changed && n_new == old_len {
        let o: Vec<crate::lisp::value::EmacsChar> = old.chars().collect();
        let n: Vec<crate::lisp::value::EmacsChar> = new.chars().collect();
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
    apply_case_text(i, &b, s, &old, &new, &widths);
    b.borrow_mut().note_text_change(old_len);
    if changed {
        let span_new = n_new - (old_len - last_old);
        crate::lisp::builtins::evalfn::signal_after_change(
            i,
            s + first + 1,
            s + first + span_new + 1,
            last_old as i128 - first as i128,
        )?;
    }
    Ok(Value::Nil)
}
fn f_upcase_region(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    region_op(i, &a, crate::lisp::builtins::strfn::CaseOp::Up)
}
fn f_downcase_region(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    region_op(i, &a, crate::lisp::builtins::strfn::CaseOp::Down)
}
fn f_capitalize_region(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    region_op(i, &a, crate::lisp::builtins::strfn::CaseOp::Cap)
}

fn word_op(i: &mut Interp, a: &[Value], op: crate::lisp::builtins::strfn::CaseOp) -> EvalResult {
    use crate::lisp::builtins::strfn::{apply_case_text, case_extra, case_str_with_widths};
    // GNU casify_word: the optional ARG is decoded with CHECK_FIXNUM
    // — nil and non-numbers both signal wrong-type-argument fixnump.
    let n = crate::lisp::builtins::want_fixnum(i, &arg(a, 0))?;
    let b = cur(i);
    let down = i.current_case_table();
    let up = case_extra(&down, 0).unwrap_or_else(|| down.clone());
    let title = match op {
        crate::lisp::builtins::strfn::CaseOp::Cap | crate::lisp::builtins::strfn::CaseOp::CapUp => {
            crate::lisp::builtins::charset::unicode_prop_table(i, "titlecase")
        }
        _ => Value::Nil,
    };
    let syn = syntax_table_entries(i);
    let wordp = |c: EmacsChar| syntax_entry_code(syn.as_ref(), c) == b'w';
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
    let (new, widths) = case_str_with_widths(i, &old, op, &down, &up, &title);
    // GNU `casify_region' shape (see region_op).
    crate::lisp::builtins::evalfn::signal_before_change(i, s + 1, e + 1)?;
    let old_len = e - s;
    let n_new = new.chars().count();
    let changed = new != old;
    let (first, last_old) = if changed && n_new == old_len {
        let o: Vec<crate::lisp::value::EmacsChar> = old.chars().collect();
        let n: Vec<crate::lisp::value::EmacsChar> = new.chars().collect();
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
    apply_case_text(i, &b, s, &old, &new, &widths);
    b.borrow_mut().note_text_change(old_len);
    if changed {
        let span_new = n_new - (old_len - last_old);
        crate::lisp::builtins::evalfn::signal_after_change(
            i,
            s + first + 1,
            s + first + span_new + 1,
            last_old as i128 - first as i128,
        )?;
    }
    // GNU moves point over the changed words for positive args and
    // leaves it alone for negative args.
    if n >= 0 {
        b.borrow_mut().set_point(s + n_new);
    }
    Ok(Value::Nil)
}
fn f_upcase_word(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    word_op(i, &a, crate::lisp::builtins::strfn::CaseOp::Up)
}
fn f_downcase_word(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    word_op(i, &a, crate::lisp::builtins::strfn::CaseOp::Down)
}
fn f_capitalize_word(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    word_op(i, &a, crate::lisp::builtins::strfn::CaseOp::Cap)
}

fn f_indent_line_to(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU: number-or-marker-p
    // gate first; markers and non-integral floats → `fixnump'; negative
    // integers (incl. integral floats) → `wholenump'.
    let col = match &a[0] {
        Value::Int(n) => {
            if *n < 0 {
                return Err(i.wrong_type_mut("wholenump", &a[0]));
            }
            *n as usize
        }
        // GNU indent-line-to is Lisp: a non-negative float reaches
        // `indent-to' (fixnump), a negative one `move-to-column'
        // (wholenump) — integrality is never accepted.
        Value::Float(f) => {
            let pred = if **f < 0.0 { "wholenump" } else { "fixnump" };
            return Err(i.wrong_type_mut(pred, &a[0]));
        }
        Value::Marker(m) => {
            if m.borrow().buffer.is_none() {
                return Err(i.error("Marker does not point anywhere"));
            }
            return Err(i.wrong_type_mut("fixnump", &a[0]));
        }
        other => return Err(i.wrong_type_mut("number-or-marker-p", other)),
    };
    let raw_tab_width = i.symbol_value(i.q_soft("tab-width").unwrap_or(0));
    let zero_tab_width = match &raw_tab_width {
        Value::Int(n) => *n == 0,
        Value::Float(n) => **n == 0.0,
        _ => false,
    };
    let tab = crate::buffer::primitives::sane_tab_width(i) as usize;
    let tabs_on = i
        .symbol_value(i.q_soft("indent-tabs-mode").unwrap_or(0))
        .truthy();
    let b = cur(i);
    let bb = b.borrow_mut();
    let line = bb.text.line_of_pos(bb.point());
    let ls = bb.text.line_start(line);
    // Delete existing leading whitespace, insert `col` columns of
    // whitespace (tabs when `indent-tabs-mode' is on, like GNU).
    let mut e = ls;
    let mut curcol = 0usize;
    while e < bb.text.len() && (bb.text.char_at(e) == ' ' || bb.text.char_at(e) == '\t') {
        curcol += if bb.text.char_at(e) == '\t' {
            (curcol / tab + 1) * tab - curcol
        } else {
            1
        };
        e += 1;
    }
    if zero_tab_width && curcol < col {
        drop(bb);
        return Err(i.signal_data(sym::ARITH_ERROR, vec![]));
    }
    drop(bb);
    crate::buffer::primitives::chg_delete(i, ls, e)?;
    let ws = if tabs_on {
        format!("{}{}", "\t".repeat(col / tab), " ".repeat(col % tab))
    } else {
        " ".repeat(col)
    };
    let wlen = ws.chars().count();
    if wlen > 0 {
        cur(i).borrow_mut().set_point(ls);
        crate::buffer::primitives::chg_insert_and_inherit_pt(i, &ws)?;
    }
    cur(i).borrow_mut().set_point(ls + wlen);
    // GNU `indent-line-to' returns COLUMN only when it indented forward;
    // dedenting or already-at-column returns nil.
    Ok(if curcol < col {
        Value::Int(col as i128)
    } else {
        Value::Nil
    })
}

fn f_indent_to(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let col = crate::lisp::builtins::want_fixnum(i, &a[0])?.max(0);
    let minimum = match a.get(1) {
        Some(v) if v.truthy() => want_int(i, v)?,
        _ => 0,
    };
    let tab = crate::buffer::primitives::sane_tab_width(i);
    let tabs_on = i
        .symbol_value(i.q_soft("indent-tabs-mode").unwrap_or(0))
        .truthy();
    let b = cur(i);
    let bb = b.borrow_mut();
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
        crate::buffer::primitives::chg_insert_and_inherit_pt(i, &s)?;
    }
    Ok(Value::Int(mincol))
}

fn f_indent_rigidly(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let len = cur(i).borrow().text.len();
    let (s, e) = region_bounds_strict(i, &a[0], &a[1], len)?;
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
                    && (bb.text.char_at(pos) == ' ' || bb.text.char_at(pos) == '\t')
                {
                    d += 1;
                    pos += 1;
                }
                if d > 0 {
                    crate::buffer::primitives::delete_preserving_undo(
                        i,
                        &mut bb,
                        pos - d,
                        pos,
                        false,
                    );
                    pos -= d;
                }
            }
            // to next line
            let le = bb.text.line_end(pos);
            pos = if le < bb.text.len() { le + 1 } else { break };
        }
    }
    let e2 = cur(i).borrow().text.len().min(e.max(s));
    crate::lisp::builtins::evalfn::signal_after_change(i, s + 1, e2 + 1, (e - s) as i128)?;
    Ok(Value::Nil)
}

fn f_move_beginning_of_line(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let n = num_or_mark(i, &arg(&a, 0))?;
    let b = cur(i);
    let mut bb = b.borrow_mut();
    let line = bb.text.line_of_pos(bb.point());
    let last = bb.text.line_of_pos(bb.text.len());
    // GNU line-move: moving past the last line stops at its
    // beginning (forward-line fails there when there's no
    // trailing newline); past BOB stops at point-min.
    let target = (line as i128 + (n - 1)).clamp(0, last as i128) as usize;
    let p = bb.text.line_start(target);
    bb.set_point(p);
    // GNU's Lisp defun returns `(goto-char ...)' — the new point.
    Ok(Value::Int(bb.point() as i128 + 1))
}

fn f_move_end_of_line(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let n = num_or_mark(i, &arg(&a, 0))?;
    let b = cur(i);
    let mut bb = b.borrow_mut();
    let line = bb.text.line_of_pos(bb.point());
    if n <= 0 {
        // GNU `move-end-of-line' runs `(line-move arg t)' which does
        // forward-line(arg): arg<=0 goes to bol of line cur+arg-1 —
        // past BOB, the line-0 bol (point-min).
        let target = line as i128 + (n - 1);
        let p = if target < 0 {
            bb.text.line_start(0)
        } else {
            let ls = bb.text.line_start(target as usize);
            bb.text.line_end(ls)
        };
        bb.set_point(p);
        return Ok(Value::Nil);
    }
    let last = bb.text.line_of_pos(bb.text.len());
    let target = (line + (n - 1).max(0) as usize).min(last);
    let ls = bb.text.line_start(target);
    let p = bb.text.line_end(ls);
    bb.set_point(p);
    // GNU's Lisp defun ends in a `while' loop — returns nil.
    Ok(Value::Nil)
}

fn f_forward_line_cmd(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
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

fn f_command_line_1_stub(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU Fcommand_line_1 CHECK_LISTs COMMAND-LINE-ARGS
    // before dispatching; the work itself was already done by the
    // startup path, so a list argument just returns nil.
    match &a[0] {
        Value::Nil | Value::Cons(_) => Ok(Value::Nil),
        other => Err(i.wrong_type_mut("listp", other)),
    }
}

fn f_minibufferp(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU Fminibufferp: (memq buffer Vminibuffer_list), or with
    // LIVE non-nil `live_minibuffer_p' — only depths 1..=minibuf_level
    // (the echo-area ` *Minibuf-0*' and idle deeper buffers are out).
    let id = match a.get(0) {
        None | Some(Value::Nil) => i.current_buffer,
        // GNU resolves a string name via get_buffer (nil when absent);
        // non-buffer non-string non-nil args get CHECK_BUFFER.
        Some(v) => match i.buffer_id_of(v) {
            Some(id) => id,
            None if matches!(v, Value::Str(_)) => return Ok(Value::Nil),
            None => return Err(i.wrong_type_mut("bufferp", v)),
        },
    };
    let live = a.get(1).map(|v| v.truthy()).unwrap_or(false);
    if live {
        // live_minibuffer_p: skip index 0, bounded by minibuf_level.
        let hi = i.minibuf_level.max(0) as usize;
        Ok(Value::from_bool(
            (1..=hi).any(|d| i.minibuf_list.get(d) == Some(&id)),
        ))
    } else {
        Ok(Value::from_bool(i.is_minibuffer(id)))
    }
}

fn f_minibuffer_contents(i: &mut Interp, _a: crate::lisp::Argv) -> EvalResult {
    // GNU Fminibuffer_contents: in a minibuffer, the text after the
    // prompt; otherwise the whole buffer text.
    let whole = match i.buffers.get(i.current_buffer) {
        Some(b) => b.borrow().text.text(),
        None => String::new().into(),
    };
    if !i.is_minibuffer(i.current_buffer) {
        return Ok(Value::string(whole));
    }
    let pe = match f_minibuffer_prompt_end(i, vec![].into())? {
        Value::Int(n) => (n.max(1) - 1) as usize,
        _ => 0,
    };
    let b = cur(i);
    let bb = b.borrow();
    let e = bb.text_len();
    let s = pe.min(e);
    Ok(Value::string(bb.text.substring(s, e)))
}

fn f_delete_minibuffer_contents(i: &mut Interp, _a: crate::lisp::Argv) -> EvalResult {
    // GNU delete-minibuffer-contents (subr.el): `(delete-region
    // (minibuffer-prompt-end) (point-max))' — works on the CURRENT buffer
    // even outside a minibuffer (`minibuffer-prompt-end' answers
    // point-min there), and inside one erases only past the prompt since
    // the prompt text carries `read-only'.
    let start = match f_minibuffer_prompt_end(i, vec![].into()) {
        Ok(Value::Int(p)) => (p - 1).max(0) as usize,
        _ => 0,
    };
    let tlen = cur(i).borrow().text.len();
    if start < tlen {
        crate::buffer::primitives::chg_delete(i, start, tlen)?;
    }
    let pt = cur(i).borrow().point;
    if pt > start {
        cur(i).borrow_mut().set_point(start);
    }
    Ok(Value::Nil)
}

fn f_minibuffer_depth(i: &mut Interp, _a: crate::lisp::Argv) -> EvalResult {
    Ok(Value::Int(i.minibuf_level as i128))
}
fn f_minibuffer_prompt(i: &mut Interp, _a: crate::lisp::Argv) -> EvalResult {
    // GNU Fminibuffer_prompt: the innermost active minibuffer's
    // prompt, nil when no minibuffer is active.
    match i.minibuf_prompts.last() {
        Some(p) => Ok(Value::string(p.clone())),
        None => Ok(Value::Nil),
    }
}

/// `minibuffer-prompt-end' — GNU: (point-min) outside a minibuffer,
/// else `field-end' of the prompt `field' region at BEGV (or BEGV when
/// the field runs to ZV without a `field' property).
fn f_minibuffer_prompt_end(i: &mut Interp, _a: crate::lisp::Argv) -> EvalResult {
    let beg = cur(i).borrow().begv as i128 + 1;
    if !i.is_minibuffer(i.current_buffer) {
        return Ok(Value::Int(beg));
    }
    let fe = i.q("field-end");
    let end = i.apply(
        &Value::Sym(fe),
        vec![Value::Int(beg), Value::Nil, Value::Nil],
    )?;
    let zv = cur(i).borrow().zv as i128 + 1;
    let end_n = end.int().unwrap_or(zv);
    if end_n == zv {
        let gcp = i.q("get-char-property");
        let fld = i.q("field");
        let at_beg = i.apply(
            &Value::Sym(gcp),
            vec![Value::Int(beg), Value::Sym(fld), Value::Nil],
        )?;
        if at_beg.is_nil() {
            return Ok(Value::Int(beg));
        }
    }
    Ok(Value::Int(end_n))
}

/// `blink-cursor-mode' — a GNU minor-mode command: called from Lisp,
/// nil means enable (not toggle), `toggle' toggles, non-positive
/// numbers disable, anything else enables; returns the new state.
fn f_blink_cursor_mode(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let sid = i.q("blink-cursor-mode");
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

/// `face-attribute-relative-p' — GNU xfaces.c: `unspecified' (and
/// `ignore-defface') are relative for every attribute; only :height
/// distinguishes fixnum (absolute) from float/other (relative).
fn f_face_attribute_relative_p(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let un = i.q("unspecified");
    let ig = i.q("ignore-defface");
    let rel = if i.sym_is(&a[1], un) || i.sym_is(&a[1], ig) {
        true
    } else if matches!(&a[0], Value::Sym(s) if i.symbol_name(*s) == ":height") {
        !matches!(a[1], Value::Int(_))
    } else {
        false
    };
    Ok(Value::from_bool(rel))
}

/// `set-window-cursor-type' — GNU window.c: TYPE must be nil, t,
/// box, hollow, bar, hbar, or a cons (box|bar|hbar . INTEGER);
/// anything else signals "Invalid cursor type".  Returns TYPE.
fn f_set_window_cursor_type(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let w = win_live(i, &a[0])?;
    let t = &a[1];
    let ok = match t {
        Value::Nil => true,
        Value::Sym(s) => {
            let n = i.symbol_name(*s);
            matches!(n.as_str(), "t" | "box" | "hollow" | "bar" | "hbar")
        }
        Value::Cons(c) => {
            let cc = c.borrow();
            (match &cc.car {
                Value::Sym(s) => matches!(i.symbol_name(*s).as_str(), "box" | "bar" | "hbar"),
                _ => false,
            }) && cc.cdr.int().is_some()
        }
        _ => false,
    };
    if !ok {
        return Err(i.error("Invalid cursor type"));
    }
    w.borrow_mut().cursor_type = t.clone();
    Ok(t.clone())
}

/// GNU window--min-size-1 in the grid frame's 1x1 character metrics.
/// State headers and the public minimum-size query share this calculation.
pub(crate) fn window_minimum(
    i: &mut Interp,
    w: &WindowRef,
    horizontal: bool,
    ignore: &Value,
    pixelwise: bool,
) -> i128 {
    let (children, direction, buffer, minibuffer, margins, size, params) = {
        let w = w.borrow();
        (
            w.children.clone(),
            w.horiz,
            w.buffer,
            w.minibuffer,
            w.margins,
            if horizontal { w.width } else { w.height },
            w.params.clone(),
        )
    };
    if let Some(direction) = direction {
        let sizes: Vec<_> = children
            .iter()
            .map(|child| window_minimum(i, child, horizontal, ignore, pixelwise))
            .collect();
        return if direction == horizontal {
            sizes.into_iter().sum()
        } else {
            sizes.into_iter().max().unwrap_or(0)
        };
    }
    let variable = |i: &mut Interp, name: &str| {
        let symbol = i.intern(name);
        i.buffers
            .get(buffer)
            .and_then(|b| b.borrow().locals.get(&symbol).cloned())
            .unwrap_or_else(|| i.obarray.symbol(symbol).value.clone())
    };
    let safe = variable(
        i,
        if horizontal {
            "window-safe-min-width"
        } else {
            "window-safe-min-height"
        },
    )
    .int()
    .unwrap_or(if horizontal { 2 } else { 1 });
    let ignore_safe = i.sym_id(ignore).is_some_and(|s| i.symbol_name(s) == "safe");
    let ignore_restrictions = match ignore {
        Value::Window(other) if !other.borrow().dead => Rc::ptr_eq(w, other),
        Value::Nil => false,
        Value::Sym(s) if i.symbol_name(*s) == "preserved" => false,
        _ => true,
    };
    if !ignore_restrictions {
        let fixed = variable(i, "window-size-fixed");
        let fixed_axis = if let Value::Sym(symbol) = fixed {
            match i.symbol_name(symbol).as_str() {
                "height" => !horizontal,
                "width" => horizontal,
                "t" => true,
                _ => false,
            }
        } else {
            false
        };
        if fixed_axis {
            return size as i128;
        }
    }
    if ignore_safe {
        return safe;
    }
    let decoration = if horizontal {
        let min_margins = i.q("min-margins");
        let min_margins = crate::lisp::eval::plist_get(&params, min_margins);
        let (left, right) = match min_margins {
            Value::Cons(c) => {
                let c = c.borrow();
                (c.car.clone(), c.cdr.clone())
            }
            _ => (Value::Nil, Value::Nil),
        };
        let margin = |value: Value, fallback: usize| match value {
            Value::Int(n) => n as f64,
            Value::Float(n) => *n,
            _ => fallback as f64,
        };
        (margin(left, margins.0) + margin(right, margins.1)).ceil() as i128
    } else if minibuffer {
        0
    } else {
        ["mode-line-format", "header-line-format", "tab-line-format"]
            .iter()
            .filter(|name| variable(i, name).truthy())
            .count() as i128
    };
    let preferred = if ignore_restrictions {
        0
    } else if pixelwise && !horizontal && minibuffer {
        1
    } else {
        variable(
            i,
            if horizontal {
                "window-min-width"
            } else {
                "window-min-height"
            },
        )
        .int()
        .unwrap_or(if horizontal { 10 } else { 4 })
    };
    (safe + decoration).max(preferred)
}

/// Minimum-size tuples in the exact header order, including independently
/// computed pixel minima (the minibuffer's pixel minimum is special).
fn win_node_mins(i: &mut Interp, w: &WindowRef) -> [i128; 12] {
    let safe = Value::Sym(i.q("safe"));
    let mut minima = [0; 12];
    for (pixelwise, offset) in [(false, 0), (true, 6)] {
        for (index, ignore) in [Value::Nil, Value::t(), safe.clone()]
            .into_iter()
            .enumerate()
        {
            minima[offset + index * 2] = window_minimum(i, w, false, &ignore, pixelwise);
            minima[offset + index * 2 + 1] = window_minimum(i, w, true, &ignore, pixelwise);
        }
    }
    minima
}

/// Is V a list whose car is SYM named NAME?
fn list_car_is(i: &Interp, v: &Value, name: &str) -> bool {
    if let Value::Cons(c) = v {
        if let Value::Sym(s) = c.borrow().car {
            return i.symbol_name(s) == name;
        }
    }
    false
}

/// `(name . VALUE)' lookup over a list of alist-style elements; the
/// cdr of `(name)' alone is nil, matching `assq'.
fn state_assq<'a>(i: &Interp, items: &'a [Value], name: &str) -> Option<&'a Value> {
    items.iter().find(|v| list_car_is(i, v, name))
}

fn ac(i: &mut Interp, k: &str, v: Value) -> Value {
    Value::cons(Value::Sym(i.intern(k)), v)
}

fn ac_i(i: &mut Interp, k: &str, n: i128) -> Value {
    ac(i, k, Value::Int(n))
}

/// `window--state-get-1' node body: returns the list ITEMS — TAG,
/// optional `(last . t)', size slots, slots, then children for
/// internal windows.  The caller prepends the min-size header.
fn win_state_items(
    i: &mut Interp,
    w: &WindowRef,
    writable: bool,
    last: bool,
) -> Result<Vec<Value>, Flow> {
    let (horiz, ww, wh, climit, params, kids) = {
        let wb = w.borrow();
        (
            wb.horiz,
            wb.width,
            wb.height,
            wb.combination_limit,
            wb.params.clone(),
            wb.children.clone(),
        )
    };
    let (nh, nw) = w.borrow().normal_size;
    let mut items = match horiz {
        Some(hor) => {
            let tag = i.intern(if hor { "hc" } else { "vc" });
            let mut items = vec![Value::Sym(tag)];
            if last {
                items.push(ac(i, "last", Value::t()));
            }
            items.extend([
                ac_i(i, "pixel-width", ww as i128),
                ac_i(i, "pixel-height", wh as i128),
                ac_i(i, "total-width", ww as i128),
                ac_i(i, "total-height", wh as i128),
                ac(i, "normal-height", Value::float(nh)),
                ac(i, "normal-width", Value::float(nw)),
                ac(i, "combination-limit", Value::from_bool(climit)),
            ]);
            items
        }
        None => {
            let (bufid, point, start, selected, ml, mr) = {
                let point = window_point(i, w) + 1;
                let sel = sel_window(i).map(|s| s.borrow().id) == Some(w.borrow().id);
                (
                    w.borrow().buffer,
                    point,
                    w.borrow().start + 1,
                    sel,
                    w.borrow().margins.0 as i128,
                    w.borrow().margins.1 as i128,
                )
            };
            let _ = (ml, mr);
            let mut items = vec![Value::Sym(i.q("leaf"))];
            if last {
                items.push(ac(i, "last", Value::t()));
            }
            items.extend([
                ac_i(i, "pixel-width", ww as i128),
                ac_i(i, "pixel-height", wh as i128),
                ac_i(i, "total-width", ww as i128),
                ac_i(i, "total-height", wh as i128),
                ac(i, "normal-height", Value::float(nh)),
                ac(i, "normal-width", Value::float(nw)),
            ]);
            items.push(leaf_buffer_state(
                i, w, writable, bufid, point, start, selected, ml, mr,
            ));
            items
        }
    };
    // GNU copies persistent associations, not the internal flat plist.
    let mut pitems: Vec<Value> = Vec::new();
    if let Ok(ps) = params.list_to_vec() {
        for pair in ps.chunks_exact(2) {
            if window_state_parameter_persistent(i, &pair[0], writable)? {
                pitems.push(Value::cons(pair[0].clone(), pair[1].clone()));
            }
        }
    }
    pitems.reverse();
    let clone_of = Value::Sym(i.q("clone-of"));
    if window_state_parameter_persistent(i, &clone_of, writable)?
        && !pitems.iter().any(|pair| match pair {
            Value::Cons(cell) => crate::lisp::builtins::eq_values(&cell.borrow().car, &clone_of),
            _ => false,
        })
    {
        pitems.insert(0, Value::cons(clone_of, Value::Window(w.clone())));
    }
    if !pitems.is_empty() {
        let mut pv = vec![Value::Sym(i.q("parameters"))];
        pv.extend(pitems);
        // GNU emits parameters BEFORE the buffer slot on leaves and
        // after combination-limit on internals.
        if horiz.is_none() {
            let buf = items.pop().unwrap();
            items.push(Value::list(pv));
            items.push(buf);
        } else {
            items.push(Value::list(pv));
        }
    }
    if horiz.is_some() {
        let n = kids.len();
        for (idx, k) in kids.iter().enumerate() {
            let mut child = win_state_items(i, k, writable, idx + 1 == n)?;
            items.push(Value::list(std::mem::take(&mut child)));
        }
    }
    Ok(items)
}

fn window_state_parameter_persistent(
    i: &mut Interp,
    key: &Value,
    writable: bool,
) -> Result<bool, Flow> {
    let specification = i.q("window-persistent-parameters");
    let specification = i.var_alias_target(specification);
    let assq = Value::Sym(i.q("assq"));
    let association = i.apply(&assq, vec![key.clone(), i.symbol_value(specification)])?;
    let flag = match association {
        Value::Cons(cell) => cell.borrow().cdr.clone(),
        _ => Value::Nil,
    };
    Ok(flag.truthy()
        && (!writable
            || crate::lisp::builtins::eq_values(&flag, &Value::Sym(i.q("writable")))))
}

fn restore_window_state_parameters(
    i: &mut Interp,
    window: &WindowRef,
    items: &[Value],
) -> Result<(), Flow> {
    // State restoration resets current values even when no parameters
    // were saved; GNU retains the existing associations with nil values.
    let existing = window.borrow().params.list_to_vec().unwrap_or_default();
    for pair in existing.chunks_exact(2) {
        let mut data = window.borrow_mut();
        data.params = crate::lisp::eval::plist_put_front(&data.params, &pair[0], Value::Nil);
    }
    if let Some(Value::Cons(slot)) = state_assq(i, items, "parameters") {
        let parameters = slot.borrow().cdr.clone();
        for pair in crate::lisp::builtins::want_list(i, &parameters)? {
            let (key, value) = match pair {
                Value::Cons(cell) => {
                    let cell = cell.borrow();
                    (cell.car.clone(), cell.cdr.clone())
                }
                Value::Nil => (Value::Nil, Value::Nil),
                other => return Err(i.wrong_type_mut("listp", &other)),
            };
            let mut data = window.borrow_mut();
            data.params = crate::lisp::eval::plist_put_front(&data.params, &key, value);
        }
    }
    Ok(())
}

/// The leaf `(buffer BUFFER . STATE)' spec — buffer object (or name
/// when WRITABLE), then selected/hscroll/fringes/margins/scroll-bars/
/// vscroll/dedicated/point/start.
fn leaf_buffer_state(
    i: &mut Interp,
    w: &WindowRef,
    writable: bool,
    bufid: usize,
    point: usize,
    start: usize,
    selected: bool,
    ml: i128,
    mr: i128,
) -> Value {
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
        ac_i(i, "hscroll", 0),
        Value::list(vec![
            Value::Sym(i.q("fringes")),
            Value::Int(0),
            Value::Int(0),
            Value::Nil,
            Value::Nil,
        ]),
        {
            // (margins [LEFT [RIGHT]]) — nonzero widths only.
            let mut m = vec![Value::Sym(i.q("margins"))];
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
            Value::Sym(i.q("scroll-bars")),
            Value::Nil,
            Value::Int(0),
            Value::t(),
            Value::Nil,
            Value::Int(0),
            Value::t(),
            Value::Nil,
        ]),
        ac_i(i, "vscroll", 0),
        ac(i, "dedicated", w.borrow().dedicated.clone()),
        ac(i, "point", point_obj),
        ac(i, "start", start_obj),
    ]);
    Value::cons(Value::Sym(i.q("buffer")), buf_state)
}

/// `window-state-get' — serializes the frame's window tree (or the
/// given node).  Element 0 is the min-size header, element 1 the
/// `leaf'/`vc'/`hc' tag; WRITABLE substitutes buffer names and
/// integer positions for objects and saves only writable parameters.
fn f_window_state_get(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let w = match arg(&a, 0) {
        // GNU serializes the frame's ROOT window by default.
        Value::Nil => sel_frame(i)
            .and_then(|f| f.borrow().root.clone())
            .or_else(|| sel_window(i))
            .ok_or_else(|| i.error("No window"))?,
        Value::Window(w) => {
            if w.borrow().dead {
                let shown = i.princ_to_string(&a[0]);
                return Err(i.error(format!("{shown} is not a valid window")));
            }
            w.clone()
        }
        other => {
            let shown = i.princ_to_string(&other);
            return Err(i.error(format!("{shown} is not a live or internal window")));
        }
    };
    let writable = arg(&a, 1).truthy();
    let m = win_node_mins(i, &w);
    let header = Value::list(vec![
        ac_i(i, "min-height", m[0]),
        ac_i(i, "min-width", m[1]),
        ac_i(i, "min-height-ignore", m[2]),
        ac_i(i, "min-width-ignore", m[3]),
        ac_i(i, "min-height-safe", m[4]),
        ac_i(i, "min-width-safe", m[5]),
        ac_i(i, "min-pixel-height", m[6]),
        ac_i(i, "min-pixel-width", m[7]),
        ac_i(i, "min-pixel-height-ignore", m[8]),
        ac_i(i, "min-pixel-width-ignore", m[9]),
        ac_i(i, "min-pixel-height-safe", m[10]),
        ac_i(i, "min-pixel-width-safe", m[11]),
    ]);
    let mut items = vec![header];
    items.extend(win_state_items(i, &w, writable, false)?);
    Ok(Value::list(items))
}

/// Collect the live leaves of W's subtree in display order.
pub(crate) fn subtree_leaves(w: &WindowRef, out: &mut Vec<WindowRef>) {
    let (internal, kids, dead) = {
        let wb = w.borrow();
        (wb.horiz.is_some(), wb.children.clone(), wb.dead)
    };
    if internal {
        for k in kids {
            subtree_leaves(&k, out);
        }
    } else if !dead {
        out.push(w.clone());
    }
}

/// f64 from an Int/Float value (state normal sizes are floats).
fn state_float(v: &Value) -> Option<f64> {
    match v {
        Value::Int(n) => Some(*n as f64),
        Value::Float(f) => Some(**f),
        _ => None,
    }
}

/// Materialize one `window-state' node as a fresh window covering
/// (L,T,W,H) and return it.  Child node lists get their share of the
/// axis via their recorded normal size (equal split when absent).
fn win_state_materialize(
    i: &mut Interp,
    f: &FrameRef,
    items: &[Value],
    l: usize,
    t: usize,
    wd: usize,
    h: usize,
    selected: &mut Option<WindowRef>,
) -> Result<WindowRef, Flow> {
    let tag = items
        .first()
        .and_then(|v| i.sym_id(v))
        .map(|s| i.symbol_name(s))
        .unwrap_or_else(|| "leaf".to_string());
    let horiz = tag == "hc";
    if tag == "vc" || tag == "hc" {
        let node = Window::new_internal(horiz, l, t, wd, h);
        node.borrow_mut().change_frame = Some(f.borrow().id);
        node.borrow_mut().change_clock = Some(f.borrow().change_stamp.clone());
        // Children are the trailing elements that are `leaf'/`vc'/
        // `hc' lists; a `(last . t)' marker rides on the final child.
        let child_states: Vec<Vec<Value>> = items[1..]
            .iter()
            .filter(|v| {
                list_car_is(i, v, "leaf") || list_car_is(i, v, "vc") || list_car_is(i, v, "hc")
            })
            .filter_map(|v| v.list_to_vec().ok())
            .collect();
        let n = child_states.len();
        // Each child's share of the axis comes from its normal size
        // (fraction of the parent); missing entries split evenly.
        let fracs: Vec<f64> = child_states
            .iter()
            .map(|cs| {
                let name = if horiz {
                    "normal-width"
                } else {
                    "normal-height"
                };
                state_assq(i, &cs[1.min(cs.len())..], name)
                    .and_then(|e| match e {
                        Value::Cons(c) => state_float(&c.borrow().cdr),
                        _ => None,
                    })
                    .unwrap_or(1.0 / n.max(1) as f64)
            })
            .collect();
        let fsum: f64 = fracs.iter().sum();
        let axis = if horiz { wd } else { h };
        let mut pos = if horiz { l } else { t };
        let end = pos + axis;
        let mut kids: Vec<WindowRef> = Vec::new();
        for (idx, cs) in child_states.iter().enumerate() {
            let size = if idx + 1 == n {
                end.saturating_sub(pos)
            } else {
                ((axis as f64 * fracs[idx] / fsum.max(f64::EPSILON)) as usize).max(1)
            };
            let (cl, ct, cw, ch) = if horiz {
                (pos, t, size, h)
            } else {
                (l, pos, wd, size)
            };
            let child = win_state_materialize(i, f, cs, cl, ct, cw, ch, selected)?;
            let normal = size as f64 / axis.max(1) as f64;
            child.borrow_mut().normal_size = if horiz { (1.0, normal) } else { (normal, 1.0) };
            child.borrow_mut().parent = Some(Rc::downgrade(&node));
            kids.push(child);
            pos += size;
        }
        node.borrow_mut().children = kids;
        if let Some(cl) = state_assq(i, &items[1..], "combination-limit") {
            if let Value::Cons(c) = cl {
                node.borrow_mut().combination_limit = c.borrow().cdr.truthy();
            }
        }
        restore_window_state_parameters(i, &node, &items[1..])?;
        let _ = f;
        Ok(node)
    } else {
        // Leaf: create a window on the recorded buffer.
        let bufid = state_assq(i, &items[1..], "buffer")
            .and_then(|e| match e {
                Value::Cons(c) => {
                    let b = c.borrow();
                    match b.cdr.clone() {
                        Value::Cons(lst) => Some(lst.borrow().car.clone()),
                        other => Some(other),
                    }
                }
                _ => None,
            })
            .and_then(|bobj| match bobj {
                Value::Buffer(b) => Some(b.borrow().id),
                Value::Str(s) => i
                    .buffers
                    .by_name(&s.borrow())
                    .or_else(|| Some(i.create_buffer(&s.borrow()))),
                _ => None,
            })
            .unwrap_or(i.current_buffer);
        let w = Window::new(bufid);
        w.borrow_mut().change_frame = Some(f.borrow().id);
        w.borrow_mut().change_clock = Some(f.borrow().change_stamp.clone());
        let point = i.buffers.get(bufid).map_or(0, |buffer| buffer.borrow().point());
        w.borrow_mut().point = point;
        set_window_old_point(i, &mut w.borrow_mut(), point);
        record_buffer_display(i, bufid);
        {
            let mut wb = w.borrow_mut();
            wb.left = l;
            wb.top = t;
            wb.width = wd;
            wb.height = h;
        }
        // Point/start/dedicated/parameters.
        let bs = state_assq(i, &items[1..], "buffer").cloned();
        if let Some(Value::Cons(c)) = &bs {
            let state: Vec<Value> = c.borrow().cdr.list_to_vec().unwrap_or_default();
            let int_slot = |i: &Interp, name: &str| -> Option<i128> {
                state_assq(i, &state, name).and_then(|e| match e {
                    Value::Cons(c) => c.borrow().cdr.int(),
                    _ => None,
                })
            };
            if let Some(p) = int_slot(i, "point") {
                w.borrow_mut().point = (p.max(1) - 1) as usize;
            }
            if let Some(s) = int_slot(i, "start") {
                w.borrow_mut().start = (s.max(1) - 1) as usize;
            }
            if state_assq(i, &state, "selected")
                .and_then(|e| match e {
                    Value::Cons(c) => Some(c.borrow().cdr.truthy()),
                    _ => None,
                })
                .unwrap_or(false)
            {
                *selected = Some(w.clone());
            }
            if let Some(Value::Cons(d)) = state_assq(i, &state, "dedicated") {
                w.borrow_mut().dedicated = d.borrow().cdr.clone();
            }
        }
        restore_window_state_parameters(i, &w, &items[1..])?;
        Ok(w)
    }
}

/// `window-state-put' — GNU's window.el protocol: a `leaf' state is
/// applied onto a leaf WINDOW in place; a composite state, or a leaf
/// state replacing an internal WINDOW, installs a fresh subtree.
fn f_window_state_put(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU window.el order:
    // `(window-live-p window)' gate first — nil WINDOW resolves to the
    // selected window via `window-normalize-window', a non-window/dead
    // one is the plain "N is not a valid window" error.  THEN the state
    // argument's `assq' reads signal `wrong-type-argument listp'.
    let w = match arg(&a, 1) {
        Value::Nil => sel_window(i).ok_or_else(|| i.error("No window"))?,
        Value::Window(w) => {
            if w.borrow().dead {
                let shown = i.princ_to_string(&arg(&a, 1));
                return Err(i.error(format!("{shown} is not a valid window")));
            }
            w
        }
        other => {
            let shown = i.princ_to_string(&other);
            return Err(i.error(format!("{shown} is not a valid window")));
        }
    };
    if !matches!(a[0], Value::Cons(_) | Value::Nil) {
        return Err(i.wrong_type_mut("listp", &a[0]));
    }
    // A well-formed state is a list (HEADER TAG . SLOTS); a short list
    // dies on GNU's number-or-marker check.
    let items = match a[0].list_to_vec() {
        Ok(v) if v.len() >= 2 => v,
        _ => return Err(i.wrong_type_mut("number-or-marker-p", &Value::Nil)),
    };
    let tag = i
        .sym_id(&items[1])
        .map(|s| i.symbol_name(s))
        .unwrap_or_default();
    let target_is_internal = w.borrow().horiz.is_some();
    if tag == "leaf" && !target_is_internal {
        // Apply the leaf spec onto W in place: display its buffer,
        // restore point/start/params.
        if let Some(bs) = state_assq(i, &items[2..], "buffer").cloned() {
            if let Value::Cons(c) = &bs {
                let (bobj, state) = {
                    let cb = c.borrow();
                    let lst = cb.cdr.clone();
                    let bobj = match &lst {
                        Value::Cons(head) => Some(head.borrow().car.clone()),
                        _ => None,
                    };
                    (bobj, lst.list_to_vec().unwrap_or_default())
                };
                if let Some(bobj) = bobj {
                    let bid = match &bobj {
                        Value::Buffer(b) => Some(b.borrow().id),
                        Value::Str(s) => i.buffers.by_name(&s.borrow()),
                        _ => None,
                    };
                    if let Some(bid) = bid {
                        let point = i.buffers.get(bid).map_or(0, |buffer| buffer.borrow().point());
                        w.borrow_mut().buffer = bid;
                        set_window_old_point(i, &mut w.borrow_mut(), point);
                        record_buffer_display(i, bid);
                    }
                }
                for e in &state {
                    if let Value::Cons(slot) = e {
                        let (name, val) = {
                            let sb = slot.borrow();
                            (sb.car.clone(), sb.cdr.clone())
                        };
                        if let Value::Sym(s) = &name {
                            match (i.symbol_name(*s).as_str(), val.int()) {
                                ("point", Some(p)) => {
                                    w.borrow_mut().point = (p.max(1) - 1) as usize;
                                }
                                ("start", Some(s2)) => {
                                    w.borrow_mut().start = (s2.max(1) - 1) as usize;
                                }
                                ("dedicated", _) => {
                                    w.borrow_mut().dedicated = val.clone();
                                }
                                _ => {}
                            }
                        }
                    }
                }
            }
        }
        restore_window_state_parameters(i, &w, &items[2..])?;
        if sel_window(i)
            .as_ref()
            .is_some_and(|selected| Rc::ptr_eq(selected, &w))
        {
            let (buffer_id, point) = {
                let window = w.borrow();
                (window.buffer, window.point)
            };
            if let Some(buffer) = i.buffers.get(buffer_id) {
                buffer.borrow_mut().set_point(point);
            }
        }
        return Ok(Value::Nil);
    }
    if tag != "leaf" && tag != "vc" && tag != "hc" {
        return Err(i.error("Invalid window state"));
    }
    // Rebuild: replace W with a fresh leaf or internal subtree.
    let Some(f) = frame_of_window(i, &w).or_else(|| sel_frame(i)) else {
        return Err(i.error("No frame"));
    };
    let (wl, wt, ww, wh) = {
        let wb = w.borrow();
        (wb.left, wb.top, wb.width, wb.height)
    };
    let par = w_parent(&w);
    let sel_was_w = f.borrow().selected.borrow().id == w.borrow().id;
    let mut selected: Option<WindowRef> = None;
    let node = win_state_materialize(i, &f, &items[1..], wl, wt, ww, wh, &mut selected)?;
    node.borrow_mut().normal_size = w.borrow().normal_size;
    // Splice the new node into the tree where W sat.
    match &par {
        Some(p) => {
            let mut pb = p.borrow_mut();
            for k in pb.children.iter_mut() {
                if Rc::ptr_eq(k, &w) {
                    *k = node.clone();
                }
            }
            node.borrow_mut().parent = Some(Rc::downgrade(p));
        }
        None => {
            f.borrow_mut().root = Some(node.clone());
        }
    }
    // Kill W's old subtree (W itself; internals take their children).
    mark_tree_dead(i, &w);
    // Refresh the frame's live-window list (preorder leaves).
    let mut leaves = Vec::new();
    if let Some(r) = f.borrow().root.clone() {
        subtree_leaves(&r, &mut leaves);
    }
    {
        let mut fb = f.borrow_mut();
        fb.windows = leaves;
        if let Some(s) = &selected {
            fb.selected = s.clone();
        } else if sel_was_w {
            if let Some(first) = fb.windows.first() {
                fb.selected = first.clone();
            }
        }
    }
    if let Some(s) = &selected {
        let bid = s.borrow().buffer;
        if let Some(b) = i.buffers.get(bid) {
            b.borrow_mut().set_point(s.borrow().point);
            i.set_current_buffer(bid);
        }
    }
    Ok(Value::Nil)
}

/// `Snarf-documentation' — opens FILE inside `doc-directory'; GNU
/// signals file-missing when the doc file is absent.
fn f_snarf_documentation(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let name = match &a[0] {
        Value::Str(s) => s.borrow().clone(),
        other => return Err(i.wrong_type_mut("stringp", other)),
    };
    let dir_sym = i.q("doc-directory");
    let dir = match i.symbol_value(dir_sym) {
        Value::Str(s) => s.borrow().clone(),
        _ => String::new().into(),
    };
    let path = if name.starts_with('/') {
        name
    } else {
        format!("{dir}{name}").into()
    };
    if native_path(i, &path)?.exists() {
        // Real doc-file parsing is not wired up; an existing file
        // still yields nil rather than a doc string here.
        return Ok(Value::Nil);
    }
    let s = i.q("file-missing");
    Err(i.signal_data(
        s,
        vec![
            Value::string("Opening doc string file"),
            Value::string("No such file or directory"),
            Value::string(path),
        ],
    ))
}

fn f_minibuffer_message(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU minibuffer.el:
    // (minibuffer-message MESSAGE &rest ARGS) — outside a minibuffer this
    // is `(apply #'message message args)', returning the formatted string;
    // inside one it wraps MESSAGE in " [...]", properties it, and shows it
    // via an after-string overlay (returning nil).
    let v = crate::lisp::builtins::strfn::f_format_message(i, a.clone())?;
    let in_minibuf = {
        let cur = i.buffer_value(i.current_buffer).unwrap_or(Value::Nil);
        let mp = i.q("minibufferp");
        i.apply(&Value::Sym(mp), vec![cur, Value::t()])
            .map(|v| v.truthy())
            .unwrap_or(false)
    };
    if !in_minibuf {
        return crate::lisp::builtins::evalfn::f_message(i, a);
    }
    let msg = match &v {
        Value::Str(s) => s.borrow().clone(),
        other => i.princ_to_string(other),
    };
    i.message(&format!(" [Minibuffer: {}]", msg));
    Ok(Value::Nil)
}

/// If the front-end input hook is installed, read a line with PROMPT;
/// otherwise fall back to `fallback` (batch behavior).
/// GNU batch minibuffer input reads a line from stdin; at EOF it
/// signals `(end-of-file "Error reading from stdin")'.
fn batch_eof(i: &mut Interp) -> Flow {
    let eof = i.q("end-of-file");
    i.signal_data(eof, vec![Value::string("Error reading from stdin")])
}

/// `read-buffer' prompt (GNU Fread_buffer): when a default is given,
/// strip a trailing ":", " ", or ": " from PROMPT and run it through
/// `format-prompt' so "(default X)" lands before the colon.
fn read_buffer_prompt(i: &mut Interp, prompt: &Value, default: &Value) -> Result<EmacsText, Flow> {
    // GNU Fread_buffer: when DEF is non-nil, PROMPT (any Lisp object)
    // is passed through format-prompt → substitute-command-keys, so a
    // non-string errors `char-or-string-p'.  With nil DEF the prompt
    // is used verbatim — CHECK_STRING in read_minibuf → stringp.
    if !default.is_nil() {
        let mut p = match prompt {
            Value::Str(s) => s.borrow().clone(),
            _ => EmacsText::new().into(),
        };
        if p.ends_with(": ") {
            p.truncate(p.len() - 2);
        } else if p.ends_with(':') || p.ends_with(' ') {
            p.truncate(p.len() - 1);
        }
        let fp = i.q("format-prompt");
        // Pass the ORIGINAL prompt object (not the string copy) so a
        // non-string prompt hits substitute-command-keys' check.
        let orig = if matches!(prompt, Value::Str(_)) {
            Value::string(p)
        } else {
            prompt.clone()
        };
        match i.apply(&Value::Sym(fp), vec![orig, default.clone()])? {
            Value::Str(s) => return Ok(s.borrow().clone()),
            other => return Ok(i.prin1_to_string(&other)),
        }
    }
    match prompt {
        Value::Str(s) => Ok(s.borrow().clone()),
        other => Err(i.wrong_type_mut("stringp", other)),
    }
}

/// `read-number' prompt (GNU subr.el): when a default is given,
/// insert " (default X)" before a trailing colon's preceding position
/// ("Num: " → "Num (default 7): "); without a colon, trailing
/// whitespace is replaced ("N?" → "N? (default 0)").
fn read_number_prompt(i: &mut Interp, prompt: &Value, default: &Value) -> EmacsText {
    let p = match prompt {
        Value::Str(s) => s.borrow().clone(),
        _ => EmacsText::new().into(),
    };
    let d1 = match default {
        Value::Cons(c) => c.borrow().car.clone(),
        _ => default.clone(),
    };
    if d1.is_nil() {
        return p;
    }
    let ds = i.princ_to_string(&d1);
    let trimmed = p.trim_end_matches([' ', '\t']);
    if let Some(base) = trimmed.strip_suffix(':') {
        // Insert before the colon, keeping it and the trailing space.
        format!("{base} (default {ds}):{}", &p[trimmed.len()..]).into()
    } else {
        format!("{trimmed} (default {ds})").into()
    }
}

fn minibuf_or(
    i: &mut Interp,
    prompt: &Value,
    args: crate::lisp::eval::MinibufArgs,
) -> Result<Option<Value>, Flow> {
    // GNU `read_minibuf_noninteractive'/`read_minibuf' CHECK_STRINGs
    // the prompt outright — nil is NOT accepted.
    let p = match prompt {
        Value::Str(s) => s.borrow().clone(),
        other => return Err(i.wrong_type_mut("stringp", other)),
    };
    if let Some(line) = i.batch_minibuf_line(&p)? {
        // GNU `read_minibuf_noninteractive' returns the raw line.
        return Ok(Some(line));
    }
    if i.minibuf_reader.is_none() {
        return Err(batch_eof(i));
    }
    Ok(Some(Value::string(i.minibuf_read(&p, args)?)))
}

fn f_read_from_minibuffer(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // Args: PROMPT INITIAL-CONTENTS KEYMAP READ HISTORY DEFAULT-VALUE
    // INHERIT-INPUT-METHOD.
    let args = crate::lisp::eval::MinibufArgs {
        hist: arg(&a, 4),
        defalt: arg(&a, 5),
        initial: arg(&a, 1),
        keymap: arg(&a, 2),
    };
    if let Some(s) = minibuf_or(i, &a[0], args)? {
        if !arg(&a, 3).is_nil() {
            // READ flag: parse the line as a Lisp object
            // (GNU `read_minibuf_noninteractive' expflag).
            return i.string_to_object(&s, &arg(&a, 5));
        }
        return Ok(s);
    }
    Ok(arg(&a, 5))
}
fn f_read_buffer(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU `read-buffer' (simple.el) is Lisp: completing-read over
    // `internal-complete-buffer' with MUST-MATCH = REQUIRE-MATCH arg,
    // plus `read-buffer-function' override.  In batch the raw line is
    // returned verbatim (empty → DEF or "").
    let d = match arg(&a, 1) {
        Value::Buffer(b) => Value::string(b.borrow().name.clone()),
        v => v,
    };
    let p = read_buffer_prompt(i, &a[0], &d)?;
    if let Some(line) = i.batch_minibuf_line(&p)? {
        if line.is_empty_string() {
            return Ok(if d.is_nil() { Value::string("") } else { d });
        }
        return Ok(line);
    }
    if i.minibuf_reader.is_none() {
        return Err(batch_eof(i));
    }
    let rbf = i.q("read-buffer-function");
    if i.bound_p(rbf) {
        let fval = i.symbol_value(rbf);
        if fval.truthy() {
            return i.apply(&fval, vec![Value::string(p), d, arg(&a, 2)]);
        }
    }
    // (completing-read prompt internal-complete-buffer nil require-match
    // nil 'buffer-name-history default nil)
    let cr = i.q("completing-read");
    let table = Value::Sym(i.q("internal-complete-buffer"));
    let hist = Value::Sym(i.q("buffer-name-history"));
    i.apply(
        &Value::Sym(cr),
        vec![
            Value::string(p),
            table,
            Value::Nil,
            arg(&a, 2),
            Value::Nil,
            hist,
            d,
            Value::Nil,
        ],
    )
}
fn f_read_file_name(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU `read-file-name' (minibuffer.el) is a thin wrapper over
    // `read-file-name-function' (default `read-file-name-default') —
    // the Lisp function inserts the default directory, selects the
    // filename completion map, substitutes-in-file-name on exit, and
    // handles MUSTMATCH/confirm.  Route the same way.
    let rfnf = i.q("read-file-name-function");
    let fval = {
        let v = if i.bound_p(rfnf) {
            i.symbol_value(rfnf)
        } else {
            Value::Nil
        };
        if v.truthy() {
            v
        } else {
            Value::Sym(i.q("read-file-name-default"))
        }
    };
    let mut argv = a.clone();
    Rc::make_mut(&mut argv.0).resize(6, Value::Nil);
    i.apply(&fval, argv)
}
fn f_read_number(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let prompt = read_number_prompt(i, a.get(0).unwrap_or(&Value::Nil), &arg(&a, 1));
    // GNU read-number (subr.el): `read' the input; non-numbers and
    // empty-without-default reprompt after "Please enter a number.".
    loop {
        let line = if i.minibuf_reader.is_none() && i.noninteractive {
            i.batch_read_line(&prompt)?
        } else if i.minibuf_reader.is_some() {
            Value::string(i.minibuf_line(&prompt)?)
        } else {
            return Err(batch_eof(i));
        };
        let n = if line.is_empty_string() {
            arg(&a, 1)
        } else {
            // GNU `(read str)' errors (e.g. unbalanced parens)
            // propagate out of read-number.
            let text = want_str(i, &line)?;
            i.read_from_string(&text, 0)?.0
        };
        match n {
            Value::Int(_) | Value::Float(_) => return Ok(n),
            _ => {
                i.message("Please enter a number.");
                std::thread::sleep(std::time::Duration::from_secs(1));
            }
        }
    }
}
fn f_read_regexp(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU read-regexp (replace.el): a prompt already ending in ":" is
    // used as-is; otherwise format-prompt adds the default.  Empty
    // input yields the default (car of a list).
    let d = match &arg(&a, 1) {
        Value::Cons(c) => c.borrow().car.clone(),
        v => v.clone(),
    };
    let prompt = match a.get(0) {
        Some(Value::Str(s)) => s.borrow().clone(),
        _ => String::new().into(),
    };
    let trailing = prompt.trim_end_matches([' ', '\t']);
    let p = if trailing.ends_with(':') {
        prompt
    } else if d.is_nil() {
        prompt
    } else {
        let fp = i.q("format-prompt");
        match i.apply(
            &Value::Sym(fp),
            vec![Value::string(prompt.clone()), d.clone()],
        ) {
            Ok(Value::Str(s)) => s.borrow().clone(),
            _ => prompt,
        }
    };
    if let Some(line) = i.batch_minibuf_line(&p)? {
        if line.is_empty_string() {
            return Ok(d);
        }
        return Ok(line);
    }
    if i.minibuf_reader.is_none() {
        return Err(batch_eof(i));
    }
    let args = crate::lisp::eval::MinibufArgs {
        hist: Value::Sym(i.q("regexp-history")),
        defalt: d.clone(),
        ..Default::default()
    };
    let line = i.minibuf_read(&p, args)?;
    if line.is_empty() {
        return Ok(d);
    }
    Ok(Value::string(line))
}
fn f_completing_read(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // minibuf.c delegates all eight arguments, even in batch mode. Validation
    // and physical input belong to the selected implementation.
    let variable = i.q("completing-read-function");
    let function = i.symbol_value(variable);
    let mut arguments = a;
    Rc::make_mut(&mut arguments.0).resize(8, Value::Nil);
    i.apply(&function, arguments)
}

/// Resolve a completion TABLE to (ELT, ELTSTRING) pairs.  ELTSTRING
/// keeps its original `Value::Str' so text properties survive into
/// try/all-completion results, as in GNU's C code.
/// Enumerate TABLE's candidates per GNU's collection dispatch
/// (minibuf.c): alist, vector-as-obarray (via `resolve_obarray', which
/// performs the obarray_slow bucket check and promotion), obarray
/// record, or hash table.  Each entry is (ELT, ELTSTRING, EXTRA):
/// ELT is the collection element PRED's first arg, ELTSTRING is the
/// completion text (None for elements with no string/symbol name —
/// GNU skips them), and EXTRA (hash-table values only) is PRED's
/// second argument.
fn completion_candidates(
    i: &mut Interp,
    table: &Value,
) -> Result<Vec<(Value, Option<Value>, Option<Value>)>, Flow> {
    /// GNU's eltstring derivation: from a symbol its name, from a cons
    /// cell its CAR (which must again be a string or symbol).
    /// Anything else has no completion text.
    fn elt_text(i: &mut Interp, v: &Value) -> Option<Value> {
        match v {
            s @ Value::Str(_) => Some(s.clone()),
            Value::Sym(s) => Some(Value::string(i.symbol_name(*s))),
            _ => None,
        }
    }
    let one = |i: &mut Interp, v: &Value| match v {
        Value::Cons(c) => elt_text(i, &c.borrow().car.clone()).map(|e| (v.clone(), Some(e), None)),
        _ => elt_text(i, v).map(|e| (v.clone(), Some(e), None)),
    };
    match table {
        Value::Cons(_) => {
            // GNU walks the collection list element-by-element until a
            // non-cons tail (a dotted tail is not an error here).
            // A tortoise guards against circular collections.
            let mut out: Vec<(Value, Option<Value>, Option<Value>)> = Vec::new();
            let mut cur = table.clone();
            let mut tortoise = table.clone();
            let mut steps = 0usize;
            while let Value::Cons(c) = &cur {
                let (item, next) = {
                    let b = c.borrow();
                    (b.car.clone(), b.cdr.clone())
                };
                if let Some(cand) = one(i, &item) {
                    out.push(cand);
                }
                cur = next;
                steps += 1;
                if steps % 2 == 0 {
                    let tnext = match &tortoise {
                        Value::Cons(tc) => tc.borrow().cdr.clone(),
                        _ => tortoise.clone(),
                    };
                    tortoise = tnext;
                    if eq_values(&cur, &tortoise) {
                        break;
                    }
                }
            }
            Ok(out)
        }
        // A vector is a legacy obarray: empty vectors are valid (empty)
        // obarrays, otherwise `resolve_obarray' validates the buckets
        // and promotes (slot 0 becomes the obarray record).
        Value::Vec(v) => {
            if v.borrow().is_empty() {
                return Ok(Vec::new());
            }
            match crate::lisp::builtins::data::resolve_obarray(i, Some(table))? {
                Some(ob) if crate::lisp::builtins::data::is_default_obarray(i, &ob) => Ok(i
                    .obarray
                    .all_ids()
                    .iter()
                    .map(|&id| {
                        let s = i.sym(id);
                        (s.clone(), elt_text(i, &s), None)
                    })
                    .collect()),
                Some(ob) => Ok(crate::lisp::builtins::data::obarray_syms(i, &ob)
                    .iter()
                    .map(|x| (x.clone(), elt_text(i, x), None))
                    .collect()),
                None => Ok(Vec::new()),
            }
        }
        Value::Nil => Ok(Vec::new()),
        // Hash-table collection: candidates are the keys; PRED is
        // called on both key and value.
        Value::Hash(h) => Ok(h
            .borrow()
            .keys
            .iter()
            .map(|(hk, k)| {
                let eltstr = elt_text(i, k);
                let extra = h.borrow().map.get(hk).cloned();
                (k.clone(), eltstr, extra)
            })
            .collect()),
        // An obarray is a tagged record; the `obarray' variable's record
        // denotes the global symbol table (enumerated via all_ids),
        // while `obarray-make' records carry their own symbol vec.
        other if crate::lisp::builtins::data::is_default_obarray(i, other) => Ok(i
            .obarray
            .all_ids()
            .iter()
            .map(|&id| {
                let s = i.sym(id);
                (s.clone(), elt_text(i, &s), None)
            })
            .collect()),
        other if crate::lisp::builtins::data::is_obarray(i, other) => {
            Ok(crate::lisp::builtins::data::obarray_syms(i, other)
                .iter()
                .map(|x| (x.clone(), elt_text(i, x), None))
                .collect())
        }
        _ => Ok(Vec::new()),
    }
}

/// ELTSTRING's text, if it is a string (GNU's STRINGP test).
fn cand_text(v: &Value) -> Option<EmacsText> {
    if let Value::Str(s) = v {
        Some(s.borrow().clone())
    } else {
        None
    }
}

/// Whether TABLE is one of GNU's collection types (list, obarray,
/// vector-as-obarray, hash table, nil).  Anything else is treated as a
/// function-style completion table and is funcalled, like GNU's
/// call3 fallback (a non-function then signals `invalid-function').
fn completion_is_collection(i: &mut Interp, table: &Value) -> bool {
    match table {
        Value::Nil | Value::Vec(_) | Value::Hash(_) => true,
        // A `(lambda ...)' cons is a function, not a candidate list
        // (completion-table-dynamic et al return lambda forms).
        Value::Cons(c) => {
            let car = c.borrow().car.clone();
            !i.sym_is(&car, crate::lisp::obarray::sym::LAMBDA)
        }
        // A raw `nil' symbol (shouldn't occur post-eval) is the empty
        // collection too.
        Value::Sym(s) if *s == crate::lisp::obarray::sym::NIL => true,
        v => crate::lisp::builtins::data::is_obarray(i, v),
    }
}

/// Whether TABLE is a function-style completion table (a callable).
/// Kept for `completion-boundaries', where GNU consults the table
/// function for the `boundaries' action.
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
    i.q_soft("completion-ignore-case")
        .map(|id| i.symbol_value(id).truthy())
        .unwrap_or_default()
}

/// GNU's `compare-strings' prefix check with `completion-ignore-case':
/// CAND has S as a prefix (case-insensitive when IGNORE_CASE).
fn completion_prefix_p(cand: &EmacsStr, s: &EmacsStr, ignore_case: bool) -> bool {
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
/// case-folding). Validate each reached element and propagate regexp errors.
fn completion_match_regexps(i: &mut Interp, s: &EmacsStr, ignore_case: bool) -> Result<bool, Flow> {
    let Some(id) = i.q_soft("completion-regexp-list") else {
        return Ok(true);
    };
    let mut cur = i.symbol_value(id);
    let syn = crate::editor::re_syntax(i);
    let chars: Vec<EmacsChar> = s.chars().collect();
    loop {
        match cur {
            Value::Cons(c) => {
                let (re_v, next) = {
                    let b = c.borrow();
                    (b.car.clone(), b.cdr.clone())
                };
                let pattern = want_str(i, &re_v)?;
                let re = crate::lisp::regexp::compile_case(&pattern, ignore_case)
                    .map_err(|error| err_sym(i, "invalid-regexp", vec![Value::string(error.0)]))?;
                if crate::lisp::regexp::search(
                    &re,
                    &chars,
                    0,
                    &syn,
                    &crate::editor::re_category(i),
                    crate::lisp::regexp::NO_POINT,
                )
                .is_none()
                {
                    return Ok(false);
                }
                cur = next;
            }
            _ => return Ok(true),
        }
    }
}

/// The shared collection-side filter of GNU's try/all/test-completion:
/// prefix match, `completion-regexp-list', and PRED applied to ELT
/// (the collection element itself, as in GNU's C code).
fn completion_candidate_ok(
    i: &mut Interp,
    elt: &Value,
    extra: &Option<Value>,
    c: &EmacsStr,
    s: &EmacsStr,
    pred: &Value,
    ignore_case: bool,
) -> Result<bool, Flow> {
    if !completion_prefix_p(c, s, ignore_case) || !completion_match_regexps(i, c, ignore_case)? {
        return Ok(false);
    }
    if pred.truthy() {
        let mut pargs = vec![elt.clone()];
        if let Some(x) = extra {
            pargs.push(x.clone());
        }
        let r = i.apply(pred, pargs)?;
        if !r.truthy() {
            return Ok(false);
        }
    }
    Ok(true)
}

/// GNU's `compare-strings' over the first N chars of A and B: length
/// of the common prefix (case-folded when IGNORE_CASE), capped at N.
fn common_prefix_len(a: &EmacsStr, b: &EmacsStr, n: usize, ignore_case: bool) -> usize {
    let ac: Vec<EmacsChar> = a.chars().take(n).collect();
    let bc: Vec<EmacsChar> = b.chars().take(n).collect();
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
/// properties and byte representation like GNU's Fsubstring.
fn substring_str_value(i: &mut Interp, v: &Value, n: usize) -> Value {
    let Value::Str(src) = v else {
        return v.clone();
    };
    let text: crate::lisp::text::EmacsText = src.borrow().chars().take(n).collect();
    let ns = crate::lisp::Value::string_ref_with_kind(text, crate::lisp::builtins::strfn::string_multibyte_p(i, v));
    if crate::lisp::builtins::strfn::string_multibyte_p(i, v) {
        i.mark_multibyte(&ns);
    } else {
        i.mark_unibyte(&ns);
    }
    if i.has_str_props(src) {
        let ivs: Vec<(usize, usize, Vec<Value>)> = i
            .str_props(src)
            .iter()
            .filter_map(|(a, b, pl)| {
                let lo = *a;
                let hi = (*b).min(n);
                (lo < hi).then(|| (lo, hi, crate::buffer::primitives::plist_pairs_rev(pl)))
            })
            .collect();
        i.set_str_props(&ns, ivs);
    }
    Value::Str(ns)
}

fn f_try_completion(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let s = want_str(i, &a[0])?;
    if !completion_is_collection(i, &a[1]) {
        return i.apply(&a[1], vec![a[0].clone(), arg(&a, 2), Value::Nil]);
    }
    try_completions(i, &s, &a[1], &arg(&a, 2))
}

// Collection-only path, also used by internal-complete-buffer.
fn try_completions(i: &mut Interp, s: &EmacsStr, table: &Value, pred: &Value) -> EvalResult {
    let cands = completion_candidates(i, table)?;
    let ignore_case = completion_ignore_case(i);
    let s_chars = s.chars().count();
    // Port of GNU minibuf.c Ftry_completion: track the bestmatch
    // element string and shrink it to the common prefix; the result is
    // a substring of bestmatch, preserving its text properties.
    let mut bestmatch: Option<Value> = None;
    let mut bestmatchsize: usize = 0;
    let mut matchcount = 0usize;
    for (elt, eltstring, extra) in cands {
        let Some(eltstring) = eltstring else { continue };
        let Some(text) = cand_text(&eltstring) else {
            continue;
        };
        let eltlen = text.chars().count();
        if eltlen < s_chars
            || !completion_candidate_ok(i, &elt, &extra, &text, s, pred, ignore_case)?
        {
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

fn f_all_completions(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let s = want_str(i, &a[0])?;
    if !completion_is_collection(i, &a[1]) {
        return i.apply(&a[1], vec![a[0].clone(), arg(&a, 2), Value::t()]);
    }
    // Default-obarray fast path (e.g. `eldoc-add-command-completions'
    // scanning `commandp' over ~30k symbols ×34 prefixes at startup):
    // filter on the &str name and only materialize `Str'/`Sym' values
    // for candidates that survive prefix + regexp checks.
    if crate::lisp::builtins::data::is_default_obarray(i, &a[1]) {
        return all_completions_obarray(i, &s, &arg(&a, 2));
    }
    let cands = completion_candidates(i, &a[1])?;
    let pred = arg(&a, 2);
    let ignore_case = completion_ignore_case(i);
    let mut out: Vec<Value> = Vec::new();
    for (elt, eltstring, extra) in cands {
        let Some(eltstring) = eltstring else { continue };
        if let Some(c) = cand_text(&eltstring) {
            if c.chars().count() >= s.chars().count()
                && completion_candidate_ok(i, &elt, &extra, &c, &s, &pred, ignore_case)?
            {
                // Keep the original eltstring Value so text properties
                // survive (GNU returns the candidate objects).
                out.push(eltstring);
            }
        }
    }
    Ok(Value::list(out))
}

/// `all-completions' specialized to the default obarray: iterate
/// `Obarray' ids directly, running the prefix and `completion-regexp-list'
/// gates on `&str' names so no `Value' is allocated per reject.  Symbol
/// and name string are built only for survivors — GNU returns the
/// candidate strings.
fn all_completions_obarray(i: &mut Interp, s: &EmacsStr, pred: &Value) -> EvalResult {
    let ids = i.obarray.all_ids();
    let ignore_case = completion_ignore_case(i);
    let s_len = s.chars().count();
    let mut out: Vec<Value> = Vec::new();
    for id in ids {
        let name = i.obarray.name(id);
        if name.chars().count() < s_len || !completion_prefix_p(&EmacsText::from(name), s, ignore_case) {
            continue;
        }
        let name = name.to_string();
        if !completion_match_regexps(i, &EmacsText::from(&name), ignore_case)? {
            continue;
        }
        if pred.truthy() {
            let elt = Value::Sym(id);
            if !i.apply(pred, vec![elt])?.truthy() {
                continue;
            }
        }
        out.push(Value::string(name));
    }
    Ok(Value::list(out))
}

// Direct port of GNU minibuf.c Fcompletion__flex_cost_gotoh: modified
// Gotoh algorithm scoring PAT as a subsequence of STR.
fn f_completion_flex_cost_gotoh(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
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
    let pat: Vec<EmacsChar> = pat_s.chars().collect();
    let st: Vec<EmacsChar> = str_s.chars().collect();
    let patlen = pat.len();
    let strlen = st.len();
    let width = strlen + 1;
    let size = (patlen + 1) * width;

    // Bail if strings are empty or matrix too large.
    if patlen == 0 || strlen == 0 || size > FLEX_MAX_MATRIX_SIZE {
        return Ok(Value::Nil);
    }

    let ignore_case = i
        .symbol_value(i.q_soft("completion-ignore-case").unwrap_or(0))
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

fn f_test_completion(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let s = want_str(i, &a[0])?;
    if !completion_is_collection(i, &a[1]) {
        let lambda_kw = i.q("lambda");
        return i.apply(&a[1], vec![a[0].clone(), arg(&a, 2), Value::Sym(lambda_kw)]);
    }
    // Default-obarray fast path: same gates, no candidate Vec<Value>.
    let pred = arg(&a, 2);
    let ignore_case = completion_ignore_case(i);
    if crate::lisp::builtins::data::is_default_obarray(i, &a[1]) {
        for id in i.obarray.all_ids() {
            let name = i.obarray.name(id);
            let eq = if ignore_case {
                name.chars()
                    .flat_map(|x| x.to_lowercase())
                    .eq(s.chars().flat_map(|x| x.to_lowercase()))
            } else {
                name == s.to_unicode_lossy()
            };
            if !eq {
                continue;
            }
            let name = name.to_string();
            if !completion_match_regexps(i, &EmacsText::from(&name), ignore_case)? {
                continue;
            }
            let ok = if pred.truthy() {
                i.apply(&pred, vec![Value::Sym(id)])?.truthy()
            } else {
                true
            };
            if ok {
                return Ok(Value::t());
            }
        }
        return Ok(Value::Nil);
    }
    let cands = completion_candidates(i, &a[1])?;
    // GNU Ftest_completion: S must equal a candidate (obeying
    // `completion-ignore-case'), match `completion-regexp-list', and
    // satisfy PRED (called on ELT, not the string; a hash-table
    // collection passes key and value).
    for (elt, eltstring, extra) in &cands {
        let Some(eltstring) = eltstring else { continue };
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
        if eq && completion_match_regexps(i, &c, ignore_case)? && {
            let r = if pred.truthy() {
                let mut pargs = vec![elt.clone()];
                if let Some(x) = extra {
                    pargs.push(x.clone());
                }
                i.apply(&pred, pargs)?.truthy()
            } else {
                true
            };
            r
        } {
            return Ok(Value::t());
        }
    }
    Ok(Value::Nil)
}

fn f_internal_complete_buffer(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // (STRING PRED FLAG): complete STRING over live buffer names.
    // FLAG dispatch comes first: nil → try-completion, t →
    // all-completions, lambda → test-completion, anything else → nil
    // without ever checking STRING's type.
    let pred = arg(&a, 1);
    let flag = arg(&a, 2);
    enum Mode {
        Try,
        All,
        Test,
    }
    let mode = match &flag {
        Value::Nil => Mode::Try,
        Value::Sym(sym) if i.symbol_name(*sym) == "lambda" => Mode::Test,
        Value::Sym(sym) if i.symbol_name(*sym) == "t" => Mode::All,
        _ => return Ok(Value::Nil),
    };
    let s = want_str(i, &a[0])?;
    // GNU hides space-prefixed (internal) buffers unless STRING starts
    // with a space.
    let show_hidden = s.starts_with(' ');
    // GNU completes over `Vbuffer_alist': candidates are (NAME . BUFFER)
    // conses and PRED is applied to each candidate element.
    let items: Vec<Value> = i
        .buffers
        .list()
        .iter()
        .filter_map(|id| i.buffers.get(*id))
        .filter(|b| show_hidden || !b.borrow().name.starts_with(' '))
        .map(|b| {
            Value::cons(
                Value::string(b.borrow().name.clone()),
                Value::Buffer(b.clone()),
            )
        })
        .collect();
    let table = Value::list(items);
    match mode {
        Mode::Try => try_completions(i, &s, &table, &pred),
        Mode::Test => {
            let cands = completion_candidates(i, &table)?;
            let ignore_case = completion_ignore_case(i);
            for (elt, es, extra) in cands {
                let Some(es) = es else { continue };
                if let Some(c) = cand_text(&es) {
                    if c == s.as_str() || (ignore_case && c.eq_ignore_ascii_case(s.as_str())) {
                        if completion_candidate_ok(i, &elt, &extra, &c, &s, &pred, ignore_case)? {
                            return Ok(Value::t());
                        }
                    }
                }
            }
            Ok(Value::Nil)
        }
        Mode::All => {
            let cands = completion_candidates(i, &table)?;
            let ignore_case = completion_ignore_case(i);
            let mut out: Vec<Value> = Vec::new();
            for (elt, es, extra) in cands {
                let Some(es) = es else { continue };
                if let Some(c) = cand_text(&es) {
                    if completion_candidate_ok(i, &elt, &extra, &c, &s, &pred, ignore_case)? {
                        out.push(es);
                    }
                }
            }
            Ok(Value::list(out))
        }
    }
}

fn f_completion_boundaries(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // (STRING TABLE PRED SUFFIX) → (START . END): the start boundary
    // inside STRING is 0 and END counts SUFFIX chars (GNU's plain
    // completion style has no field separator). A function table may
    // override via the `boundaries' action.
    // GNU's defun never validates STRING; it only calls `funcall' on a
    // function table.  SUFFIX goes through `length' → sequencep.
    fn suffix_seq_len(i: &mut Interp, v: &Value) -> Result<i128, Flow> {
        match v {
            Value::Nil => Ok(0),
            Value::Str(s) => Ok(s.borrow().chars().count() as i128),
            Value::Cons(_) => Ok(v.list_to_vec().map(|xs| xs.len() as i128).unwrap_or(0)),
            Value::Vec(v) => Ok(v.borrow().len() as i128),
            other => Err(i.wrong_type_mut("sequencep", other)),
        }
    }
    let s = a[0].clone();
    if table_is_callable(i, &a[1]) {
        // GNU calls the table with action (boundaries . SUFFIX) and
        // expects (boundaries START . END) back.
        let bw = i.q("boundaries");
        let act = Value::cons(Value::Sym(bw), arg(&a, 3));
        let r = i.apply(&a[1], vec![s.clone(), arg(&a, 2), act])?;
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
                        Value::Nil => Value::Int(suffix_seq_len(i, &arg(&a, 3))?),
                        e => e.clone(),
                    };
                    return Ok(Value::cons(start, end));
                }
            }
        }
    }
    let end = suffix_seq_len(i, &arg(&a, 3))?;
    Ok(Value::cons(Value::Int(0), Value::Int(end)))
}

fn f_read_string(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU Fread_string: empty input with a non-nil DEFAULT-VALUE
    // (4th arg) yields the default (its car when a list).
    let d = arg(&a, 3);
    let args = crate::lisp::eval::MinibufArgs {
        defalt: d.clone(),
        ..Default::default()
    };
    if let Some(s) = minibuf_or(i, &a[0], args)? {
        if s.is_empty_string() && !d.is_nil() {
            return Ok(match &d {
                Value::Cons(c) => c.borrow().car.clone(),
                _ => d,
            });
        }
        return Ok(s);
    }
    Ok(d)
}
fn f_read_command(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU Fread_command: completing-read over commands; batch reads a
    // raw line — empty input yields the default, else the name is
    // interned (even when it names no command).
    let d = arg(&a, 1);
    let args = crate::lisp::eval::MinibufArgs {
        hist: Value::Sym(i.q("extended-command-history")),
        defalt: d.clone(),
        ..Default::default()
    };
    if let Some(s) = minibuf_or(i, &a[0], args)? {
        if s.is_empty_string() && !d.is_nil() {
            return Ok(match &d {
                Value::Cons(c) => c.borrow().car.clone(),
                _ => d,
            });
        }
        return crate::lisp::builtins::data::f_intern(i, vec![s].into());
    }
    Ok(d)
}
fn f_read_variable(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    f_read_command(i, a)
}
/// GNU `y-or-n-p' padding (subr.el): PROMPT plus a separating space
/// when needed, then the "(y or n) " suffix.
fn y_or_n_prompt(prompt: &str) -> String {
    let mut p = prompt.to_string();
    if !p.is_empty() && !p.ends_with(' ') {
        p.push(' ');
    }
    p.push_str("(y or n) ");
    p
}

fn f_y_or_n_p(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    if i.minibuf_reader.is_none() && i.noninteractive {
        // GNU batch (subr.el): read-string loop — a full line of
        // "y"/"Y"/"n"/"N" answers; anything else reprompts with a
        // "Please answer y or n.  " prefix.
        let prompt = match &a[0] {
            Value::Str(s) => s.borrow().clone(),
            _ => String::new().into(),
        };
        let padded = y_or_n_prompt(&(prompt).to_unicode_lossy());
        let mut temp = padded.clone();
        loop {
            let line = i.batch_read_line(&temp)?;
            match want_str(i, &line)?.to_unicode_lossy().as_str() {
                "y" | "Y" => return Ok(Value::t()),
                "n" | "N" => return Ok(Value::Nil),
                "h" | "H" => {
                    if i.q_soft("help-form")
                        .map(|id| !i.symbol_value(id).is_nil())
                        .unwrap_or(false)
                    {
                        let hf_id = i.q("help-form");
                        let hf = i.symbol_value(hf_id);
                        let evaled = i.eval(&hf)?;
                        let pr = i.prin1_to_string(&evaled);
                        crate::stdio::write(format!("{pr}\n").as_bytes());
                        crate::stdio::flush();
                    } else {
                        temp = format!("Please answer y or n.  {padded}");
                    }
                }
                _ => temp = format!("Please answer y or n.  {padded}"),
            }
        }
    }
    if i.minibuf_reader.is_none() {
        return Err(batch_eof(i));
    }
    if i.minibuf_reader.is_some() {
        let prompt = match &a[0] {
            Value::Str(s) => s.borrow().clone(),
            _ => String::new().into(),
        };
        // GNU y-or-n-p → read-char-from-minibuffer → read-from-minibuffer:
        // a real minibuffer read (setup/exit hooks run); only the first
        // typed character counts.
        loop {
            let line = i.minibuf_line(&y_or_n_prompt(&(prompt).to_unicode_lossy()))?;
            match line.chars().next().and_then(|c| c.unicode()) {
                Some('y') | Some('Y') => return Ok(Value::t()),
                Some('n') | Some('N') => return Ok(Value::Nil),
                _ => {
                    i.message("Please answer y or n");
                }
            }
        }
    }
    Ok(Value::Nil)
}

fn f_yes_or_no_p(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU Fyes_or_no_p: `use-short-answers' delegates to y-or-n-p.
    if i.q_soft("use-short-answers")
        .map(|id| !i.symbol_value(id).is_nil())
        .unwrap_or(false)
    {
        return f_y_or_n_p(i, a);
    }
    let prompt = match &a[0] {
        Value::Str(s) => s.borrow().clone(),
        _ => String::new().into(),
    };
    let suffix = i
        .intern_soft("yes-or-no-prompt")
        .and_then(|id| match i.symbol_value(id) {
            Value::Str(s) => Some(s.borrow().clone()),
            _ => None,
        })
        .unwrap_or_else(|| "(yes or no) ".to_string().into());
    let mut full = prompt;
    if !full.is_empty() && !full.ends_with(' ') {
        full.push(' ');
    }
    full.push_str(&suffix);
    if i.minibuf_reader.is_none() && i.noninteractive {
        loop {
            let line = i.batch_read_line(&full)?;
            match want_str(i, &line)?.to_lowercase().to_unicode_lossy().as_str() {
                "yes" => return Ok(Value::t()),
                "no" => return Ok(Value::Nil),
                _ => {
                    // GNU: ding (\a on stdout), message, sleep 2,
                    // then reprompt with the same prompt.
                    crate::stdio::write(b"\x07");
                    crate::stdio::flush();
                    i.message("Please answer yes or no.");
                    std::thread::sleep(std::time::Duration::from_secs(2));
                }
            }
        }
    }
    if i.minibuf_reader.is_none() {
        return Err(batch_eof(i));
    }
    loop {
        let line = i.minibuf_line(&full)?;
        match line.to_lowercase().to_unicode_lossy().as_str() {
            "yes" => return Ok(Value::t()),
            "no" => return Ok(Value::Nil),
            _ => i.message("Please answer yes or no."),
        }
    }
}

// ---------- commands ----------

fn f_commandp(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
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
    let mut cmd = match v {
        Value::Sym(id) => i.symbol_function(*id),
        other => other.clone(),
    };
    // Advised command: peel the trampoline layers to the base, then
    // resolve a possible symbol-alias tail (GNU reaches the innermost
    // definition through the oclosure's cdr chain).
    for _ in 0..64 {
        let peeled = i.advice_base_value(&cmd);
        match peeled {
            Value::Sym(id) => {
                cmd = i.symbol_function(id);
                if matches!(cmd, Value::Sym(s) if s == crate::lisp::obarray::sym::UNBOUND) {
                    break;
                }
            }
            other => {
                cmd = other;
                break;
            }
        }
    }
    Ok(Value::from_bool(match &cmd {
        Value::Lambda(l) => {
            if l.interactive.is_some()
                || crate::lisp::builtins::data::bc_interactive_form(i, l).is_some()
            {
                true
            } else {
                // OClosure: the docstring slot (doc_value, or bc_items[4]
                // for byte-code objects) holds the type symbol.  GNU's
                // `interactive-form' delegates these to the
                // `oclosure-interactive-form' generic; commandp is
                // `!NILP(interactive-form)', so call the generic and
                // test its result.
                let tag = match &l.doc_value {
                    Some(Value::Sym(s)) => Some(*s),
                    _ => match l
                        .bc_items
                        .as_ref()
                        .and_then(|it| it.borrow().get(4).cloned())
                    {
                        Some(Value::Sym(s)) => Some(s),
                        _ => None,
                    },
                };
                match tag {
                    Some(_) => match i.q_soft("oclosure-interactive-form") {
                        Some(g) if i.fbound_p(g) => {
                            let gf = i.symbol_function(g);
                            match i.apply(&gf, crate::lisp::Argv::from(vec![cmd.clone()])) {
                                Ok(v) => return Ok(Value::from_bool(v.truthy())),
                                Err(e) => return Err(e),
                            }
                        }
                        _ => false,
                    },
                    None => false,
                }
            }
        }
        Value::Subr(s) => {
            match crate::lisp::builtins::comp::native_interactive_form(s) {
                Some(form) => !form.is_nil(),
                None => crate::lisp::eval::subr_interactive(s.name).is_some(),
            }
        }
        // strings and vectors are keyboard macros — commands, but
        // FOR-CALL-INTERACTIVELY=t rejects them (GNU Fcommandp).
        Value::Str(_) | Value::Vec(_) => !a.get(1).map(|v| v.truthy()).unwrap_or(false),
        // `(autoload FILE DOC INTERACTIVE TYPE)': a command when its
        // interactive flag is non-nil (GNU's Fcommandp checks the same
        // slot before deciding whether to load the file).
        Value::Cons(_) => {
            let auto_id = i.q("autoload");
            let items: Vec<Value> = cmd.list_to_vec().unwrap_or_default();
            matches!(items.first(), Some(h) if i.sym_is(h, auto_id))
                && !items.get(3).map(|x| x.is_nil()).unwrap_or(true)
        }
        _ => false,
    }))
}

fn f_call_interactively(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU validates KEYS before interactive-form can trigger an autoload.
    if let Some(k) = a.get(2) {
        if !matches!(k, Value::Nil | Value::Vec(_)) {
            return Err(i.wrong_type_mut("vectorp", k));
        }
    }
    i.call_interactively_with_keys(&a[0], &arg(&a, 2))
}

fn f_execute_extended_command(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU: COMMAND-NAME (interactively from `read-extended-command')
    // skips the "M-x " prompt.
    let supplied = a.get(1).and_then(|v| match v {
        Value::Str(s) => Some(s.borrow().clone()),
        _ => None,
    });
    let name = match supplied {
        Some(n) => n,
        None => {
            // GNU execute-extended-command reads COMMAND-NAME via
            // `read-extended-command' — completing-read over obarray
            // with `commandp' + must-match (TAB works, junk rejected);
            // batch reads the name from stdin like every minibuffer read.
            let rec = i.q("read-extended-command");
            match i.apply(&Value::Sym(rec), vec![])? {
                Value::Str(s) => s.borrow().clone(),
                v => i.princ_to_string(&v),
            }
        }
    };
    if name.is_empty() {
        return Ok(Value::Nil);
    }
    let sym = i.intern(&name);
    // GNU M-x on an undefined name: "`NAME' is not a valid command name"
    // (callint.c `commandp' failure message, not "M-x ... undefined").
    // `error_q' applies text-quoting-style to the grave quotes like GNU's
    // doprnt does — the file name argument stays verbatim.
    if !i.fbound_p(sym) || !f_commandp(i, vec![Value::Sym(sym), Value::t()].into())?.truthy() {
        return Err(i.error_q("`%s' is not a valid command name", &[&name]));
    }
    i.command_execute(&Value::Sym(sym))
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
        return Value::vector(Vec::new());
    }
    if events
        .iter()
        .all(|e| matches!(e, Value::Int(n) if *n >= 0 && *n < 0x400000))
    {
        let s: crate::lisp::text::EmacsText = events
            .iter()
            .filter_map(|e| match e {
                Value::Int(n) => crate::lisp::text::EmacsChar::new(*n as u32),
                _ => None,
            })
            .collect();
        Value::string(s)
    } else {
        Value::vector(events.to_vec())
    }
}

#[cfg(test)]
mod wide_macro_event_tests {
    use super::*;
    #[test]
    fn recorded_characters_keep_full_emacs_codes() {
        let codes = [0, 65, 0xf0080, 0xd800, 0x1a0000, 0x3fff80, 0x3fffff];
        let events: Vec<_> = codes.into_iter().map(Value::Int).collect();
        let Value::Str(text) = macro_events_value(&events) else { panic!("character macro must be a string") };
        assert_eq!(text.borrow().chars().map(|c| c.code() as i128).collect::<Vec<_>>(), codes);
    }
    #[test]
    fn empty_and_non_character_macros_remain_vectors() {
        let Value::Vec(empty) = macro_events_value(&[]) else { panic!("empty macro must be a vector") };
        assert!(empty.borrow().is_empty());
        let Value::Vec(events) = macro_events_value(&[Value::Int(0xd800), Value::Int(0x400000)]) else { panic!("non-character macro must be a vector") };
        assert!(matches!(events.borrow()[0], Value::Int(0xd800)));
        assert!(matches!(events.borrow()[1], Value::Int(0x400000)));
    }
}

/// Events of a string/vector macro as key codes.
fn macro_event_codes(v: &Value) -> Vec<i128> {
    match v {
        Value::Str(s) => s.borrow().chars().map(|c| (c).code() as i128).collect(),
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

fn f_defining_kbd_macro(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // (defining-kbd-macro APPEND &optional NO-EXEC) — APPEND non-nil
    // continues the last macro; `defining-kbd-macro` the variable holds
    // the state.
    let defining = kbd_var(i, "defining-kbd-macro");
    if a[0].truthy() {
        if !i.symbol_value(defining).truthy() {
            // Seed from `last-kbd-macro`; GNU errors arrayp on nil.
            let lk = i.q("last-kbd-macro");
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

fn f_start_kbd_macro(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    f_defining_kbd_macro(i, a)
}

fn f_end_kbd_macro(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let defining = kbd_var(i, "defining-kbd-macro");
    if !i.symbol_value(defining).truthy() {
        return Err(i.error("Not defining kbd macro"));
    }
    let repeat = arg(&a, 0);
    let repeat = if repeat.is_nil() {
        1
    } else {
        crate::lisp::builtins::want_fixnum(i, &repeat)?
    };
    let last = end_kbd_macro_recording(i);
    i.message("Keyboard macro defined");
    // REPEAT counts the definition just completed as its first iteration.
    // Zero retains GNU's repeat-until-error/LOOPFUNC behavior.
    if repeat == 0 || repeat > 1 {
        let remaining = if repeat == 0 { 0 } else { repeat - 1 };
        f_execute_kbd_macro(i, vec![last, Value::Int(remaining), arg(&a, 1)].into())?;
    }
    Ok(Value::Nil)
}

fn end_kbd_macro_recording(i: &mut Interp) -> Value {
    let defining = kbd_var(i, "defining-kbd-macro");
    i.store_internal_value(defining, Value::Nil);
    // GNU's batch kboard yields [] regardless of stored events; keep
    // real events for interactive sessions.
    let mut events = std::mem::take(&mut i.kbd_macro_events);
    if i.noninteractive { events.clear(); }
    i.kbd_macro_mark = 0;
    let last = macro_events_value(&events);
    let lk = i.q("last-kbd-macro");
    i.store_internal_value(lk, last.clone());
    last
}

fn f_store_kbd_macro_event(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU `Fstore_kbd_macro_event'
    // calls `store_kbd_macro_char' unconditionally — no
    // "Not defining kbd macro" check; the event is appended even when
    // no macro is being defined.
    let defining = kbd_var(i, "defining-kbd-macro");
    if i.symbol_value(defining).truthy() {
        i.kbd_macro_events.push(a[0].clone());
    }
    Ok(Value::Nil)
}

fn f_cancel_kbd_macro_events(i: &mut Interp, _a: crate::lisp::Argv) -> EvalResult {
    // The current command's events are not yet committed to the macro.
    i.kbd_macro_events.truncate(i.kbd_macro_mark);
    Ok(Value::Nil)
}
/// `command-execute' routes keyboard-macro strings/vectors here
/// (GNU `command_execute' → `Fexecute_kbd_macro').
pub(crate) fn f_execute_kbd_macro_pub(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    f_execute_kbd_macro(i, a)
}
fn f_execute_kbd_macro(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // (execute-kbd-macro MACRO &optional COUNT LOOPFUNC)
    // GNU macros.c Fexecute_kbd_macro: MACRO is resolved through
    // indirect_function and must end at a string or vector; COUNT goes
    // through Fprefix_numeric_value (nil → 1, 0 → run until error).
    // LOOPFUNC is called before every iteration; nil return stops.
    // Before running, the current buffer switches to the buffer shown
    // in the selected window.
    let count = match f_prefix_numeric_value(i, vec![arg(&a, 1)].into())? {
        Value::Int(n) => n,
        _ => 1,
    };
    // `indirect_function' — resolve a symbol chain to its function cell.
    let final_v = i.indirect_function_value(&a[0]);
    if !matches!(final_v, Value::Str(_) | Value::Vec(_)) {
        return Err(i.error("Keyboard macros must be strings or vectors"));
    }
    let events = macro_event_codes(&final_v);
    let loopfunc = arg(&a, 2);
    // The buffer shown in the currently selected window is made current.
    let sw = i.q("selected-window");
    let wb = i.q("window-buffer");
    if let Ok(win) = i.apply(&Value::Sym(sw), vec![]) {
        if let Ok(buf) = i.apply(&Value::Sym(wb), vec![win]) {
            if !buf.is_nil() {
                let sb = i.q("set-buffer");
                let _ = i.apply(&Value::Sym(sb), vec![buf]);
            }
        }
    }
    let ek = i.q("executing-kbd-macro");
    let eki = i.q("executing-kbd-macro-index");
    let pa = i.q("prefix-arg");
    let rtc = i.q("real-this-command");
    let saved_strict = i.macro_strict;
    i.macro_strict = true;
    let mut repeat: i128 = count;
    let mut iterations: i128 = 0;
    // When this call itself runs during a macro drain, its events must go
    // *before* the outer call's still-queued events (GNU's macro stack).
    // `outer_rest' is the count of events that existed at call entry; our
    // insertions go ahead of them at index (len - outer_rest), and the
    // drain stops after our own events are consumed.
    let outer_rest = if i.macro_replaying {
        i.macro_replay.len()
    } else {
        0
    };
    loop {
        // GNU do/while: at least one iteration is attempted even when
        // COUNT is 0 (the "until error" case) or negative.
        let _ = i.set_symbol(ek, final_v.clone());
        let _ = i.set_symbol(eki, Value::Int(0));
        let _ = i.set_symbol(pa, Value::Nil);
        if !loopfunc.is_nil() {
            let cont = i.apply(&loopfunc, vec![]);
            match cont {
                Ok(v) if v.is_nil() => break,
                Ok(_) => {}
                Err(e) => {
                    i.macro_strict = saved_strict;
                    let _ = i.set_symbol(ek, Value::Nil);
                    return Err(e);
                }
            }
        }
        if i.macro_replaying {
            // Nested call inside a drain: insert ahead of the outer
            // events left in the queue.
            let pos = i.macro_replay.len() - outer_rest;
            for (off, k) in events.iter().enumerate() {
                i.macro_replay.insert(pos + off, *k);
            }
        } else {
            for k in &events {
                i.macro_replay.push_back(*k);
            }
        }
        let stop = if i.macro_replaying { outer_rest } else { 0 };
        crate::frontend::replay_macro_events(i, stop);
        if let Some(f) = i.macro_pending_flow.take() {
            i.macro_strict = saved_strict;
            let _ = i.set_symbol(ek, Value::Nil);
            let _ = i.set_symbol(rtc, i.symbol_value(ek));
            return Err(f);
        }
        if i.quit_editor {
            i.macro_strict = saved_strict;
            let _ = i.set_symbol(ek, Value::Nil);
            return Ok(Value::Nil);
        }
        iterations += 1;
        // Safety cap: GNU's repeat-0 loop is unbounded; cap at a level no
        // legitimate macro replay approaches so a non-terminating macro
        // cannot wedge the caller forever.
        if iterations > 4_000_000 {
            break;
        }
        repeat -= 1;
        // GNU: `while (--repeat && (STRINGP || VECTORP (executing-kbd-macro)))'.
        if repeat == 0 {
            break;
        }
        let cur = i.symbol_value(ek);
        if !matches!(cur, Value::Str(_) | Value::Vec(_)) {
            break;
        }
    }
    i.macro_strict = saved_strict;
    let _ = i.set_symbol(ek, Value::Nil);
    // GNU: `Vreal_this_command = Vexecuting_kbd_macro' (the final nil).
    let _ = i.set_symbol(rtc, i.symbol_value(ek));
    Ok(Value::Nil)
}

fn f_call_last_kbd_macro(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU macros.c Fcall_last_kbd_macro:
    //   Vthis_command = Vlast_command;
    //   Vreal_this_command = Vlast_kbd_macro;   // C-x z repeats the macro.
    //   if (defining_kbd_macro) error ("Can't execute anonymous macro while
    //     defining one");
    //   else if (!last_kbd_macro) error ("No kbd macro has been defined");
    //   else Fexecute_kbd_macro (last_kbd_macro, PREFIX, LOOPFUNC);
    let tc = i.q("this-command");
    let lc = i.q("last-command");
    let rtc = i.q("real-this-command");
    let dk = i.q("defining-kbd-macro");
    let lk = i.q("last-kbd-macro");
    let _ = i.set_symbol(tc, i.symbol_value(lc));
    let _ = i.set_symbol(rtc, i.symbol_value(lk));
    if i.symbol_value(dk).truthy() {
        return Err(i.error("Can't execute anonymous macro while defining one"));
    }
    let last = i.symbol_value(lk);
    if last.is_nil() {
        return Err(i.error("No kbd macro has been defined"));
    }
    let prefix = arg(&a, 0);
    let loopfunc = arg(&a, 1);
    f_execute_kbd_macro(i, vec![last, prefix, loopfunc].into())
}

fn f_kmacro(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // (kmacro KEYS &optional COUNTER FORMAT) — build a `kmacro'
    // oclosure.  The constructor body lives in lisp/kmacro.el as
    // `kmacro--impl'; lazily load the library (GNU autoloads it).
    let impl_id = i.q("kmacro--impl");
    if matches!(i.symbol_function(impl_id), Value::Sym(s) if s == sym::UNBOUND) {
        crate::lisp::load::load_library_opts_msg(i, "kmacro", false, false, true, true)?;
    }
    i.apply(&Value::Sym(impl_id), a)
}

/// GNU callint.c `Fprefix_numeric_value': nil -> 1, `- -> -1,
/// cons -> car when it's a fixnum, fixnum -> itself, else 1.
/// The cons arm does NOT recurse: `((4))' -> 1.
pub fn prefix_numeric_value(i: &Interp, v: &Value) -> i128 {
    match v {
        Value::Nil => 1,
        Value::Sym(s) if i.symbol_name(*s) == "-" => -1,
        Value::Cons(c) => match c.borrow().car {
            Value::Int(n) => n,
            _ => 1,
        },
        Value::Int(n) => *n,
        _ => 1,
    }
}

fn f_prefix_numeric_value(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    Ok(Value::Int(prefix_numeric_value(i, &a[0])))
}

fn f_universal_argument(i: &mut Interp, _a: crate::lisp::Argv) -> EvalResult {
    // GNU simple.el `universal-argument':
    //   (prefix-command-preserve-state)
    //   (setq prefix-arg (list 4))
    //   (universal-argument--mode)
    let preserve = i.q("prefix-command-preserve-state");
    i.apply(&Value::Sym(preserve), vec![])?;
    let pa = i.q("prefix-arg");
    i.obarray.symbol_mut(pa).value = Value::list(vec![Value::Int(4)]);
    let mode = i.q("universal-argument--mode");
    i.apply(&Value::Sym(mode), vec![])
}

fn f_digit_argument(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU simple.el:
    //   (prefix-command-preserve-state)
    //   (let* ((char (if (integerp last-command-event)
    //                    last-command-event
    //                  (get last-command-event 'ascii-character)))
    //          (digit (- (logand char ?\177) ?0))) ...)
    // A nil `last-command-event' makes `(logand nil ...)' signal
    // `(integer-or-marker-p nil)' before ARG is ever looked at.
    let preserve = i.q("prefix-command-preserve-state");
    i.apply(&Value::Sym(preserve), vec![])?;
    let ev = i.symbol_value(i.q_soft("last-command-event").unwrap_or(0));
    let ch = match ev {
        Value::Int(c) => c,
        other => {
            let ac = i.q("ascii-character");
            // `(get NONSYMBOL 'ascii-character)' → nil; `logand' then
            // signals `(integer-or-marker-p nil)'.
            let v = match other {
                Value::Sym(s) => i.get_prop(s, ac),
                _ => Value::Nil,
            };
            match v {
                Value::Int(c) => c,
                _ => return Err(i.wrong_type_mut("integer-or-marker-p", &v)),
            }
        }
    };
    let base = ch & 0x7f;
    // GNU: `prefix-arg' ← (cond ((integerp arg) (+ (* arg 10)
    //   (if (< arg 0) (- digit) digit)))
    //  ((eq arg '-) (if (zerop digit) '- (- digit)))
    //  (t digit))).
    let digit = base - 48; // ?0
    let next = match &a[0] {
        Value::Int(n) => Value::Int(n * 10 + if *n < 0 { -digit } else { digit }),
        Value::Sym(s) if i.symbol_name(*s) == "-" => {
            if digit == 0 {
                a[0].clone()
            } else {
                Value::Int(-digit)
            }
        }
        _ => Value::Int(digit),
    };
    let pa2 = i.q("prefix-arg");
    i.obarray.symbol_mut(pa2).value = next;
    let mode = i.q("universal-argument--mode");
    i.apply(&Value::Sym(mode), vec![])
}

fn f_negative_argument(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU simple.el `negative-argument':
    //   (prefix-command-preserve-state)
    //   (setq prefix-arg (cond ((integerp arg) (- arg))
    //                          ((eq arg '-) nil)
    //                          (t '-)))
    //   (universal-argument--mode)
    let preserve = i.q("prefix-command-preserve-state");
    i.apply(&Value::Sym(preserve), vec![])?;
    let pa = i.q("prefix-arg");
    let next = match &a[0] {
        Value::Int(n) => Value::Int(-*n),
        Value::Sym(s) if i.symbol_name(*s) == "-" => Value::Nil,
        _ => Value::Sym(i.q("-")),
    };
    i.obarray.symbol_mut(pa).value = next;
    let mode = i.q("universal-argument--mode");
    i.apply(&Value::Sym(mode), vec![])
}

// `beginning-of-defun', `end-of-defun', `mark-defun', `narrow-to-defun'
// are Lisp functions in GNU (lisp.el) — see the prelude.

fn f_count_words(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let len = cur(i).borrow().text.len();
    let (s, e) = region_bounds_strict(i, &a[0], &a[1], len)?;
    let text = {
        let b = cur(i);
        let bb = b.borrow();
        bb.text.substring(s.min(e), s.max(e))
    };
    let syn = syntax_table_entries(i);
    let mut count = 0i128;
    let mut previous = None;
    for c in text.chars() {
        if syntax_entry_code(syn.as_ref(), c) != b'w' {
            previous = None;
            continue;
        }
        if previous.is_none_or(|p| word_boundary_p(i, p, c)) {
            count += 1;
        }
        previous = Some(c);
    }
    Ok(Value::Int(count))
}

fn f_what_cursor_position(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU simple.el `what-cursor-position': at EOB
    // "point=N of TOTAL (EOB) column=C"; otherwise
    // "Char: KEYDESC (code, #oOctal, #xHex) point=N of TOTAL (PCT%)
    //  column=C" (with <BEG-END> markers when narrowed).
    // `message' returns the printed string.
    let _detail = a.get(0).map(|v| v.truthy()).unwrap_or(false);
    let (pos, total, c, beg, end, col) = {
        let b = cur(i);
        let bb = b.borrow();
        let p = bb.point();
        let c = if p < bb.text_len() {
            Some((bb.text.char_at(p)).code() as u32)
        } else {
            None
        };
        let line = bb.text.line_of_pos(p);
        let ls = bb.text.line_start(line);
        (
            p + 1,
            bb.text_len(),
            c,
            bb.begv + 1,
            bb.text_len() + 1,
            p - ls,
        )
    };
    let percent = (((pos.saturating_sub(1)) as f64 * 100.0) / (total.max(1)) as f64).round() as i64;
    let narrowed = beg != 1 || end != total + 1;
    let msg = match c {
        None if narrowed => format!(
            "point={} of {} ({}%) <{}-{}> column={}",
            pos, total, percent, beg, end, col
        ),
        None => format!("point={} of {} (EOB) column={}", pos, total, col),
        Some(ch) => {
            let name = if ch < 256 {
                describe_key(ch as i128)
            } else {
                (char::from_u32(ch).unwrap_or('\u{FFFD}').to_string()).into()
            };
            let enc = format!("({}, #o{:o}, #x{:x})", ch, ch, ch);
            if narrowed {
                format!(
                    "Char: {} {} point={} of {} ({}%) <{}-{}> column={}",
                    name, enc, pos, total, percent, beg, end, col
                )
            } else {
                format!(
                    "Char: {} {} point={} of {} ({}%) column={}",
                    name, enc, pos, total, percent, col
                )
            }
        }
    };
    i.message(&msg);
    Ok(Value::string(msg))
}

fn f_what_line(i: &mut Interp, _a: crate::lisp::Argv) -> EvalResult {
    // GNU simple.el `what-line': the message string is the return value;
    // narrowed buffers report "line N (narrowed line M)".
    let (line, begv) = {
        let b = cur(i);
        let bb = b.borrow();
        (bb.text.line_of_pos(bb.point()) + 1, bb.begv)
    };
    let s = if begv == 0 {
        format!("Line {}", line)
    } else {
        let abs = {
            let b = cur(i);
            let bb = b.borrow();
            bb.text.line_of_pos(0) + line
        };
        format!("line {} (narrowed line {})", abs, line)
    };
    i.message(&s);
    Ok(Value::string(s))
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
            let v = crate::lisp::builtins::misc::ct_raw_tag(self.tag, &vec.borrow(), c, false);
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
    syntax_table_snapshot(i, table)
}

/// GNU category.c `word_boundary_p': differing character scripts form a
/// boundary unless the current category sets select an exception from
/// `word-combining-categories'; equal scripts use
/// `word-separating-categories' instead.
pub(crate) fn word_boundary_p(i: &mut Interp, left: EmacsChar, right: EmacsChar) -> bool {
    let script_table = i
        .intern_soft("char-script-table")
        .map(|id| i.symbol_value(id))
        .unwrap_or(Value::Nil);
    let left_script = crate::lisp::builtins::misc::char_table_ref(
        i,
        &script_table,
        left.code() as usize,
    );
    let right_script =
        crate::lisp::builtins::misc::char_table_ref(i, &script_table, right.code() as usize);
    let same_script = crate::lisp::builtins::eq_values(&left_script, &right_script);
    let default = !same_script;
    let rules = i
        .intern_soft(if default {
            "word-combining-categories"
        } else {
            "word-separating-categories"
        })
        .map(|id| i.symbol_value(id))
        .unwrap_or(Value::Nil);
    let category_table = i
        .buffers
        .get(i.current_buffer)
        .and_then(|buffer| buffer.try_borrow().ok().and_then(|b| b.category_table.clone()))
        .unwrap_or_else(|| i.standard_category_table());
    let left_set = crate::lisp::builtins::misc::char_table_ref(
        i,
        &category_table,
        left.code() as usize,
    );
    let right_set = crate::lisp::builtins::misc::char_table_ref(
        i,
        &category_table,
        right.code() as usize,
    );
    if !crate::lisp::builtins::misc::is_bool_vector(i, &left_set)
        || !crate::lisp::builtins::misc::is_bool_vector(i, &right_set)
    {
        return default;
    }
    let left_bits = crate::lisp::builtins::misc::bool_vec_of(i, &left_set).unwrap_or_default();
    let right_bits = crate::lisp::builtins::misc::bool_vec_of(i, &right_set).unwrap_or_default();
    for rule in rules.list_to_vec().unwrap_or_default() {
        let Value::Cons(pair) = rule else { continue };
        let (left_category, right_category) = {
            let pair = pair.borrow();
            (pair.car.clone(), pair.cdr.clone())
        };
        let left_matches = left_category.is_nil()
            || matches!(left_category, Value::Int(c) if c >= 0 && (c as usize) < left_bits.len() && left_bits[c as usize] && !right_bits[c as usize]);
        let right_matches = right_category.is_nil()
            || matches!(right_category, Value::Int(c) if c >= 0 && (c as usize) < right_bits.len() && !left_bits[c as usize] && right_bits[c as usize]);
        if left_matches && right_matches {
            return !default;
        }
    }
    default
}

fn syntax_table_snapshot(i: &Interp, table: Value) -> Option<SynTable> {
    if !is_syntax_table(i, &table) {
        return None;
    }
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
        tag: i.q_soft("sub-char-table"),
        levels,
    })
}

/// Syntax class letter of C under TABLE; chars with no entry fall
/// back to the hardcoded standard table.
pub(crate) fn syntax_entry_code(t: Option<&SynTable>, c: impl Into<crate::lisp::value::EmacsChar>) -> u8 {
    let c = c.into();
    if let Some(t) = t {
        if let Value::Cons(cn) = t.get(crate::lisp::value::lisp_char_code(c) as u32) {
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
pub(crate) fn syntax_code_buf(i: &Interp, c: impl Into<crate::lisp::value::EmacsChar>) -> u8 {
    let c = c.into();
    match syntax_table_entries(i) {
        Some(t) => syntax_entry_code(Some(&t), c),
        None => crate::lisp::regexp::syntax_code(c),
    }
}

/// Regex-ready syntax lookup for the current buffer's `syntax-table'
/// (falls back to the standard table).  Construct BEFORE borrowing a
/// buffer — the lookup itself touches the buffer.
pub(crate) fn re_syntax(i: &Interp) -> RegexSyntax {
    use crate::lisp::builtins::{misc::CharTableSnapshot, strfn::case_extra};
    let down = i
        .current_buffer_ref()
        .and_then(|buffer| buffer.try_borrow().ok().and_then(|b| b.case_table.clone()))
        .or_else(|| i.standard_case_table.clone())
        .unwrap_or(Value::Nil);
    let snapshot_extra =
        |slot| case_extra(&down, slot).and_then(|table| CharTableSnapshot::new(i, &table));
    RegexSyntax {
        properties: Vec::new(),
        syntax: syntax_table_entries(i),
        down: CharTableSnapshot::new(i, &down),
        up: snapshot_extra(0),
        canon: snapshot_extra(1),
        equiv: snapshot_extra(2),
        category: i
            .intern_soft("unicode-category-table")
            .and_then(|id| CharTableSnapshot::new(i, &i.symbol_value(id))),
    }
}

enum RegexSyntaxProperty {
    Entry(u8),
    Table(SynTable),
}

pub(crate) struct RegexSyntax {
    properties: Vec<(usize, usize, RegexSyntaxProperty)>,
    syntax: Option<SynTable>,
    down: Option<crate::lisp::builtins::misc::CharTableSnapshot>,
    up: Option<crate::lisp::builtins::misc::CharTableSnapshot>,
    canon: Option<crate::lisp::builtins::misc::CharTableSnapshot>,
    equiv: Option<crate::lisp::builtins::misc::CharTableSnapshot>,
    category: Option<crate::lisp::builtins::misc::CharTableSnapshot>,
}

fn regex_syntax_properties(
    i: &Interp,
    intervals: &[(usize, usize, Vec<Value>)],
) -> Vec<(usize, usize, RegexSyntaxProperty)> {
    let enabled = i
        .intern_soft("parse-sexp-lookup-properties")
        .is_some_and(|id| i.symbol_value(id).truthy());
    if !enabled {
        return Vec::new();
    }
    let Some(syntax) = i.q_soft("syntax-table") else {
        return Vec::new();
    };
    let category = i.q_soft("category");
    intervals
        .iter()
        .filter_map(|(start, end, plist)| {
            let property = |id| {
                plist
                    .chunks_exact(2)
                    .find(|p| matches!(p[0], Value::Sym(s) if s == id))
                    .map(|p| p[1].clone())
            };
            let mut value = property(syntax).unwrap_or(Value::Nil);
            if value.is_nil() {
                if let Some(Value::Sym(symbol)) = category.and_then(property) {
                    value = i.get_prop(symbol, syntax);
                }
            }
            let entry = match &value {
                Value::Cons(cell) => match cell.borrow().car {
                    Value::Int(code) => SYNTAX_CLASS_CHARS
                        .get(code as usize & 0xf)
                        .map(|c| RegexSyntaxProperty::Entry(*c as u8)),
                    _ => None,
                },
                _ => syntax_table_snapshot(i, value).map(RegexSyntaxProperty::Table),
            }?;
            Some((*start, *end, entry))
        })
        .collect()
}

/// Regex source offsets are relative to ORIGIN, which can be BEGV or
/// the beginning of a buffer substring used by a higher-level command.
pub(crate) fn re_syntax_for_buffer(i: &Interp, origin: usize) -> RegexSyntax {
    let mut syntax = re_syntax(i);
    if let Some(buffer) = i.current_buffer_ref() {
        let b = buffer.borrow();
        let intervals = b.text.props.borrow().slice(origin, b.text.len().min(b.zv));
        syntax.properties = regex_syntax_properties(i, &intervals);
    }
    syntax
}

pub(crate) fn re_syntax_for_current_buffer(i: &Interp) -> RegexSyntax {
    let origin = i.current_buffer_ref().map_or(0, |b| b.borrow().begv);
    re_syntax_for_buffer(i, origin)
}

pub(crate) fn re_syntax_for_string(i: &Interp, value: &Value) -> RegexSyntax {
    let mut syntax = re_syntax(i);
    if let Value::Str(text) = value {
        syntax.properties = regex_syntax_properties(i, i.str_props(text));
    }
    syntax
}

impl RegexSyntax {
    fn mapped(table: &Option<crate::lisp::builtins::misc::CharTableSnapshot>, c: crate::lisp::value::EmacsChar) -> crate::lisp::value::EmacsChar {
        let code = crate::lisp::value::lisp_char_code(c) as u32;
        table
            .as_ref()
            .and_then(|t| match t.get(code) {
                Value::Int(n) if (0..=0x3f_ffff).contains(&n) => Some(
                    crate::lisp::value::lisp_char(n as u32)
                        .unwrap_or_else(|| crate::lisp::value::mule_seq_char(n as u32)),
                ),
                _ => None,
            })
            .unwrap_or(c)
    }

    fn raw_class(&self, name: &str, c: crate::lisp::value::EmacsChar) -> bool {
        use crate::lisp::regexp::Syntax;
        // GNU's regexp bitmap contains only ASCII members for these
        // extras.  Unicode Latin-1 characters take the multibyte path.
        if name == "unibyte" {
            return c.is_ascii();
        }
        if name == "multibyte" {
            return !c.is_ascii();
        }
        // Before Unicode loadup there is no category table yet.  Keep
        // standalone engine behavior until the GNU table is installed.
        if (self.category.is_none()
            && !c.is_ascii()
            && matches!(name, "alpha" | "alnum" | "blank" | "graph" | "print"))
            || (self.down.is_none() && matches!(name, "upper" | "lower"))
        {
            return Syntax::posix(&|ch: char| self.code(ch.into()), name, c, false);
        }
        let cp = crate::lisp::value::lisp_char_code(c) as u32;
        let upper = Self::mapped(&self.down, c) != c;
        let lower = !upper && Self::mapped(&self.up, c) != c;
        let category = self
            .category
            .as_ref()
            .and_then(|table| match table.get(cp) {
                Value::Int(n)
                    if (crate::lisp::value::FIXNUM_MIN..=crate::lisp::value::FIXNUM_MAX)
                        .contains(&n) =>
                {
                    Some(n)
                }
                _ => None,
            });
        crate::lisp::regexp::class_matches(name, c, self.code(c), category, upper, lower)
    }
}

impl crate::lisp::regexp::Syntax for RegexSyntax {
    fn case_eq(&self, a: crate::lisp::value::EmacsChar, b: crate::lisp::value::EmacsChar) -> bool {
        if self.canon.is_none() {
            // During bootstrap no installed GNU case table is available.
            return crate::lisp::regexp::Syntax::case_eq(&|c: char| self.code(c.into()), a, b);
        }
        Self::mapped(&self.canon, a) == Self::mapped(&self.canon, b)
    }

    fn case_match(&self, c: crate::lisp::value::EmacsChar, test: &mut dyn FnMut(crate::lisp::value::EmacsChar) -> bool) -> bool {
        if self.canon.is_none() || self.equiv.is_none() {
            return crate::lisp::regexp::Syntax::case_match(&|c: char| self.code(c.into()), c, test);
        }
        let mut next = Self::mapped(&self.canon, c);
        // Brent's detector also terminates malformed equivalence chains,
        // without allocating a set for every character-class candidate.
        let mut anchor = next;
        let (mut power, mut distance) = (1usize, 0usize);
        loop {
            if test(next) {
                return true;
            }
            next = Self::mapped(&self.equiv, next);
            distance += 1;
            if next == anchor {
                return false;
            }
            if distance == power {
                anchor = next;
                power = power.saturating_mul(2);
                distance = 0;
            }
        }
    }

    fn code(&self, c: crate::lisp::value::EmacsChar) -> u8 {
        syntax_entry_code(self.syntax.as_ref(), c)
    }

    fn code_at(&self, c: crate::lisp::value::EmacsChar, pos: usize) -> u8 {
        // Match the interval precedence of get-text-property, including
        // property literals whose source ranges are not sorted.
        if let Some((_, _, property)) = self
            .properties
            .iter()
            .find(|(start, end, _)| *start <= pos && pos < *end)
        {
            return match property {
                RegexSyntaxProperty::Entry(code) => *code,
                RegexSyntaxProperty::Table(table) => syntax_entry_code(Some(table), c),
            };
        }
        self.code(c)
    }

    fn posix(&self, name: &str, c: crate::lisp::value::EmacsChar, fold: bool) -> bool {
        // Eight-bit characters use the byte bitmap even in multibyte
        // text.  GNU's POSIX-class bitmap is populated only for ASCII.
        if crate::lisp::value::eight_bit_byte(c).is_some() {
            return false;
        }
        // GNU tests upper/lower against the original character, merging
        // those two classes when case folding is enabled.  Full Unicode
        // string expansions (e.g. sharp-s -> SS) are not character cases.
        if matches!(name, "upper" | "lower") {
            return self.raw_class(name, c)
                || (fold && self.raw_class(if name == "upper" { "lower" } else { "upper" }, c));
        }
        if !fold {
            return self.raw_class(name, c);
        }
        let canonical = Self::mapped(&self.canon, c);
        if !c.is_ascii() {
            return self.raw_class(name, canonical);
        }
        // GNU compiles ASCII class members into a bitmap after applying
        // translation.  Include equivalent ASCII members even when their
        // syntax differs, rather than folding their syntax after matching.
        if self.raw_class(name, canonical) || self.raw_class(name, c) {
            return true;
        }
        let mut seen = std::collections::HashSet::new();
        let mut next = canonical;
        while seen.insert(next) {
            if next.is_ascii() && self.raw_class(name, next) {
                return true;
            }
            next = Self::mapped(&self.equiv, next);
        }
        false
    }
}

/// Regex-ready category lookup for `\cX'/`\CX' classes: `(c, BIT)'
/// reports whether C's set in the current buffer's `category-table'
/// (falling back to `standard-category-table') has BIT set.
pub(crate) fn re_category(i: &Interp) -> impl Fn(crate::lisp::value::EmacsChar, u8) -> bool + 'static {
    // try_borrow: several regexp call sites invoke us while the buffer
    // is mutably borrowed (mid-edit); fall back to the standard table
    // rather than panic.
    let cb: Option<std::rc::Rc<std::cell::RefCell<Vec<Value>>>> = i
        .current_buffer_ref()
        .and_then(|b| b.try_borrow().ok().and_then(|bb| bb.category_table.clone()))
        .or_else(|| i.standard_category_table.clone())
        .and_then(|v| crate::lisp::builtins::misc::char_table_vec(&v));
    let tag = i.q_soft("sub-char-table");
    move |c, bit| {
        let Some(cb) = &cb else { return false };
        let Ok(cb) = cb.try_borrow() else {
            return false;
        };
        let v = crate::lisp::builtins::misc::ct_raw_tag(tag, &cb, c.code(), false);
        match v {
            Value::Record(r) => {
                let rr = r.borrow();
                match rr.get(1) {
                    Some(Value::Vec(b)) => b
                        .borrow()
                        .get(bit as usize)
                        .map(|x| matches!(x, Value::Int(n) if *n != 0))
                        .unwrap_or(false),
                    _ => false,
                }
            }
            _ => false,
        }
    }
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
                if let Some(sid) = i.q_soft("internal--syntax-propertize") {
                    if i.fbound_p(sid) {
                        let _ = i.apply(&Value::Sym(sid), vec![Value::Int(upto as i128)]);
                    }
                }
            }
            // Collect `syntax-table' property ranges, newest first
            // (last write wins like `prop_at').  GNU's
            // `lookup_char_property' falls back to the `category'
            // symbol's plist when the prop isn't set directly —
            // cc-mode marks template parens that way — so collect
            // category-derived `syntax-table' values into a second
            // list consulted only after the direct ones.
            let stid = i.q("syntax-table");
            let catid = i.q("category");
            let buf = cur(i);
            let bb = buf.borrow();
            let mut cat_ranges = Vec::new();
            for tp in bb.text.props.borrow().iter().rev() {
                if tp.prop == stid {
                    if let Value::Cons(_) = tp.value {
                        prop_ranges.push((tp.start, tp.end, tp.value.clone()));
                    }
                } else if tp.prop == catid {
                    if let Value::Sym(cs) = tp.value {
                        let v = i.get_prop(cs, stid);
                        if let Value::Cons(_) = v {
                            cat_ranges.push((tp.start, tp.end, v));
                        }
                    }
                }
            }
            prop_ranges.append(&mut cat_ranges);
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
    pub(crate) fn with_flags_at(&self, idx: usize, c: impl Into<crate::lisp::value::EmacsChar>) -> i128 {
        let c = c.into();
        self.prop_code(idx).unwrap_or_else(|| self.with_flags(c))
    }
    /// Syntax-class letter for the char at IDX (property-aware).
    pub(crate) fn code_at(&self, idx: usize, c: impl Into<crate::lisp::value::EmacsChar>) -> u8 {
        let c = c.into();
        if let Some(n) = self.prop_code(idx) {
            if let Some(l) = SYNTAX_CLASS_CHARS.get(n as usize & 0xf) {
                return *l as u8;
            }
        }
        self.code(c)
    }
    /// Matching delimiter for the char at IDX (property-aware).
    pub(crate) fn matching_at(&self, idx: usize, c: impl Into<crate::lisp::value::EmacsChar>) -> Option<i128> {
        let c = c.into();
        if let Some(m) = self.prop_match(idx) {
            return Some(m);
        }
        self.matching(c)
    }
    /// Raw (CLASS | FLAGS<<16) syntax value of C, or -1 when the char
    /// has no table entry (falls back to the standard class).
    fn raw(&self, c: impl Into<crate::lisp::value::EmacsChar>) -> i128 {
        let c = c.into();
        if let Some(t) = &self.entries {
            if let Value::Cons(cn) = t.get(c.code()) {
                if let Value::Int(n) = &cn.borrow().car {
                    return *n;
                }
            }
        }
        -1
    }
    /// GNU `SYNTAX_WITH_FLAGS' for C: (CLASS | FLAGS<<16).  Chars with
    /// no table entry yield the standard class and zero flags.
    pub(crate) fn with_flags(&self, c: impl Into<crate::lisp::value::EmacsChar>) -> i128 {
        let c = c.into();
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
    pub(crate) fn code(&self, c: impl Into<crate::lisp::value::EmacsChar>) -> u8 {
        let c = c.into();
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
    fn flag(&self, c: impl Into<crate::lisp::value::EmacsChar>, bit: u32) -> bool {
        let c = c.into();
        (self.with_flags(c) >> bit) & 1 == 1
    }
    /// `3' flag: first char of a two-character comment end.
    pub(crate) fn comend_first(&self, c: impl Into<crate::lisp::value::EmacsChar>) -> bool {
        let c = c.into();
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
    pub(crate) fn is_prefix_flag(&self, c: impl Into<crate::lisp::value::EmacsChar>) -> bool {
        let c = c.into();
        self.flag(c, 20)
    }
    /// The entry's matching-char (cdr) for C, if the table has one.
    pub(crate) fn matching(&self, c: impl Into<crate::lisp::value::EmacsChar>) -> Option<i128> {
        let c = c.into();
        if let Some(t) = &self.entries {
            if let Value::Cons(cn) = t.get(c.code()) {
                if let Value::Int(m) = &cn.borrow().cdr {
                    return Some(*m);
                }
            }
        }
        None
    }
    /// GNU numeric syntax class of C (0..15), honoring table entries.
    pub(crate) fn class(&self, c: impl Into<crate::lisp::value::EmacsChar>) -> i128 {
        let c = c.into();
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
    pub(crate) fn class_at(&self, idx: usize, c: impl Into<crate::lisp::value::EmacsChar>) -> i128 {
        let c = c.into();
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

fn f_char_syntax(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU CHECK_CHARACTER: signals `characterp' on non-chars
    // (strings, negative ints, >MAX_CHAR).
    let c = match &a[0] {
        Value::Int(n) if (0..=0x3f_ffff).contains(n) => *n as u32,
        other => return Err(i.wrong_type_mut("characterp", other)),
    };
    // Syntax tables cover all Emacs character codes, including surrogates
    // and the non-Unicode internal range.  Do not narrow through Rust char.
    let multibyte = i.q("enable-multibyte-characters");
    let c = if !i.symbol_value(multibyte).truthy() && (128..=255).contains(&c) {
        c + 0x3fff00
    } else {
        c
    };
    let table = f_syntax_table(i, vec![].into())?;
    let entry = crate::lisp::builtins::misc::char_table_ref(i, &table, c as usize);
    let code = match entry {
        Value::Cons(cell) => cell.borrow().car.int().unwrap_or(0) as usize & 0xff,
        _ => 0,
    };
    Ok(Value::Int(
        SYNTAX_CLASS_CHARS.get(code).copied().unwrap_or(' ') as i128,
    ))
}

/// `standard-syntax-table': GNU's exact contents — including all
/// Unicode (>255) entries — replayed from the printed trie of
/// Emacs 31's standard table (see `ctdata::STD_SYNTAX_OPS').
fn std_syntax_table(i: &mut Interp) -> Value {
    use crate::lisp::builtins::misc::{ct_replay, make_ct};
    let tag = Value::Sym(i.q("syntax-table"));
    let t = make_ct(i, tag, Value::cons(Value::Int(0), Value::Nil), vec![]);
    // GNU's exact contents — including all Unicode (>255) entries —
    // replayed from the printed trie of Emacs 31's standard table.
    ct_replay(i, &t, crate::lisp::ctdata::STD_SYNTAX_OPS);
    t
}

fn empty_syntax_table(i: &mut Interp) -> Value {
    let tag = Value::Sym(i.q("syntax-table"));
    crate::lisp::builtins::misc::make_ct(i, tag, Value::Nil, vec![])
}

fn new_syntax_table(i: &mut Interp) -> Value {
    // GNU `make-syntax-table': empty contents, parented to
    // `standard-syntax-table' — lookups inherit through the parent.
    let t = empty_syntax_table(i);
    let parent = f_standard_syntax_table(i, vec![].into()).unwrap_or(Value::Nil);
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

fn f_make_syntax_table(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU: (make-syntax-table &optional TABLE) — the new table is an
    // empty char-table whose parent is TABLE (or the standard table
    // when TABLE is nil/omitted).
    match a.first() {
        Some(v) if !v.is_nil() => {
            // GNU CHECK_CHAR_TABLE (not syntax-table-p): the parent
            // may be any char-table.
            if !crate::lisp::builtins::misc::is_char_table(i, v) {
                return Err(i.wrong_type_mut("char-table-p", v));
            }
            let t = empty_syntax_table(i);
            i.set_char_table_parent(&t, v.clone());
            Ok(t)
        }
        _ => Ok(new_syntax_table(i)),
    }
}

fn f_syntax_table_p(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    Ok(Value::from_bool(is_syntax_table(i, &a[0])))
}

/// Parse a syntax descriptor string like "w", "()" or "  4" into
/// (CLASS|FLAGS . MATCHING-CHAR); nil return means the generic "@".
fn parse_syntax_desc(i: &mut Interp, s: &EmacsStr) -> Result<Option<Value>, Flow> {
    let chars: Vec<EmacsChar> = s.chars().collect();
    if chars.is_empty() {
        // GNU: "Invalid syntax description letter: \0" (literal NUL
        // in the message) — same as `string-to-syntax ""'.
        return Err(i.error("Invalid syntax description letter: \0"));
    }
    let code: i128 = match chars[0].unicode().unwrap_or('\0') {
        // GNU syntax.c accepts '-' as a legacy synonym for whitespace.
        ' ' | '-' => 0,
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
            matching = Value::Int(m.code() as i128);
        }
    }
    // Remaining chars are flags: '1'..'4' → bits 16..19 (two-char
    // comment sequences), 'p' → bit 20 (prefix), 'b' → 21 (style b),
    // 'n' → 22 (nested), 'c' → 23 (style c).
    let mut flags: i128 = 0;
    for &f in chars.iter().skip(2) {
        flags |= match f.unicode().unwrap_or('\0') {
            '1'..='4' => 1i128 << (15 + f.code() as i128 - '0' as i128),
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
fn f_string_to_syntax(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let s = match &a[0] {
        Value::Str(s) => s.borrow().clone(),
        other => return Err(i.wrong_type_mut("stringp", other)),
    };
    if s.is_empty() {
        // GNU signals `error' with a NUL byte in the message
        // ("Invalid syntax description letter: \0"), not
        // args-out-of-range.
        return Err(i.error("Invalid syntax description letter: \0"));
    }
    Ok(parse_syntax_desc(i, &s)?.unwrap_or(Value::Nil))
}

/// GNU's syntax-class letters, indexed by class number.
const SYNTAX_CLASS_CHARS: [char; 16] = [
    ' ', '.', 'w', '_', '(', ')', '\'', '"', '$', '\\', '/', '<', '>', '@', '!', '|',
];

fn f_syntax_class_to_char(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU checks fixnump, then narrows to C int before the range test.
    let class = crate::lisp::builtins::want_fixnum(i, &a[0])? as libc::c_int;
    match SYNTAX_CLASS_CHARS.get(class as usize) {
        Some(c) if class >= 0 => Ok(Value::Int(*c as i128)),
        _ => Err(i.signal_data(
            sym::ARGS_OUT_OF_RANGE,
            vec![
                Value::Int(SYNTAX_CLASS_CHARS.len() as i128 - 1),
                a[0].clone(),
            ],
        )),
    }
}

fn f_matching_paren(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU CHECK_CHARACTER: `characterp' on non-chars
    // (non-integers, negatives, >MAX_CHAR).
    let c = match &a[0] {
        Value::Int(n) if (0..=0x3f_ffff).contains(n) => *n,
        other => return Err(i.wrong_type_mut("characterp", other)),
    };
    let table = f_syntax_table(i, vec![].into())?;
    let entry = crate::lisp::builtins::misc::char_table_ref(i, &table, c as usize);
    if let Value::Cons(cell) = entry {
        let cell = cell.borrow();
        if cell
            .car
            .int()
            .is_some_and(|code| matches!(code & 0xff, 4 | 5))
        {
            return Ok(cell.cdr.clone());
        }
    }
    Ok(Value::Nil)
}

fn f_standard_syntax_table(i: &mut Interp, _a: crate::lisp::Argv) -> EvalResult {
    let sid = i.q("remacs--standard-syntax-table");
    let cur = i.symbol_value(sid);
    if is_syntax_table(i, &cur) {
        return Ok(cur);
    }
    let t = std_syntax_table(i);
    let _ = i.set_symbol(sid, t.clone());
    Ok(t)
}

fn f_syntax_table(i: &mut Interp, _a: crate::lisp::Argv) -> EvalResult {
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
    f_standard_syntax_table(i, vec![].into())
}

fn f_set_syntax_table(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
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

fn f_copy_syntax_table(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU `copy-syntax-table': copies TABLE's raw contents; the copy's
    // parent is TABLE's parent, or `standard-syntax-table' when TABLE
    // has none.  The defalt slot is not copied.
    // (default TABLE = `standard-syntax-table').
    let src_t = match &arg(&a, 0) {
        Value::Nil => f_standard_syntax_table(i, vec![].into())?,
        v if is_syntax_table(i, v) => v.clone(),
        other => return Err(i.wrong_type_mut("syntax-table-p", other)),
    };
    let src_parent = i.char_table_parent(&src_t);
    // GNU `copy_char_table' deep-copies the trie; the defalt slot is
    // NOT copied and the parent becomes src's parent (or standard).
    let t = crate::lisp::builtins::misc::ct_copy(i, &src_t);
    i.set_char_table_defalt(&t, Value::Nil);
    let parent = match src_parent {
        Value::Nil => f_standard_syntax_table(i, vec![].into())?,
        p => p,
    };
    i.set_char_table_parent(&t, parent);
    Ok(t)
}

fn f_modify_syntax_entry(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // (modify-syntax-entry CHAR NEWENTRY &optional SYNTAX-TABLE)
    // GNU: CHAR may also be a (MIN . MAX) cons for a range of chars
    // (syntax.c SET_SYNTAX_ENTRY accepts ranges; `map-char-table'
    // over a fresh table yields them).
    let (lo, hi) = match &a[0] {
        Value::Cons(c) => {
            // GNU SET_SYNTAX_ENTRY bounds go through CHECK_CHARACTER on
            // each endpoint, not CHECK_NUMBER — a symbol bound errors
            // `characterp'.
            let (lo_v, hi_v) = {
                let b = c.borrow();
                (b.car.clone(), b.cdr.clone())
            };
            let chk = |i: &mut Interp, v: &Value| -> Result<u32, Flow> {
                match v {
                    Value::Int(n)
                        if (0..=crate::lisp::builtins::misc::CT_MAX_CHAR as i128).contains(n) =>
                    {
                        Ok(*n as u32)
                    }
                    _ => Err(i.wrong_type_mut("characterp", v)),
                }
            };
            (chk(i, &lo_v)?, chk(i, &hi_v)?)
        }
        v => {
            // GNU CHECK_CHARACTER: `characterp' on non-chars
            // (symbols, strings, negatives, >MAX_CHAR).
            let n = match v {
                Value::Int(n) if (0..=0x3f_ffff).contains(n) => *n,
                other => return Err(i.wrong_type_mut("characterp", other)),
            };
            (n as u32, n as u32)
        }
    };
    let desc = match &a[1] {
        Value::Str(s) => s.borrow().clone(),
        other => return Err(i.wrong_type_mut("stringp", other)),
    };
    let entry = match parse_syntax_desc(i, &desc)? {
        Some(e) => e,
        None => Value::Nil,
    };
    let table = match a.get(2) {
        Some(v) if !v.is_nil() => {
            if !is_syntax_table(i, v) {
                return Err(i.wrong_type_mut("syntax-table-p", v));
            }
            v.clone()
        }
        _ => f_syntax_table(i, vec![].into())?,
    };
    // GNU `SET_SYNTAX_ENTRY' uses `char_table_set_range' — one
    // block-collapsing descent, not per-char `char_table_set'.
    crate::lisp::builtins::misc::ct_set_range(i, &table, lo, hi, entry);
    Ok(Value::Nil)
}

/// `(parse-partial-sexp FROM TO &optional TARGETDEPTH STOPBEFORE
/// OLDSTATE COMMENTSTOP)' — GNU's `scan_sexps_forward' port driving the
/// active syntax table.
fn f_parse_partial_sexp(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let syn = Syn::current(
        i,
        a.get(1).and_then(|v| v.int()).unwrap_or(1).max(1) as usize,
    );
    let b = cur(i);
    let (stop_pos, state) = {
        let bb = b.borrow_mut();
        // GNU checks TARGETDEPTH's type first (CHECK_FIXNUM), then
        // coerces TO before FROM (CHECK_NUMBER_COERCE_MARKER order in
        // Fparse_partial_sexp), then validates both against [BEGV, ZV].
        let targetdepth = match a.get(2) {
            Some(v) if v.truthy() => match v.int() {
                Some(n) => n,
                None => return Err(i.wrong_type_mut("fixnump", v)),
            },
            _ => i128::MIN,
        };
        let to_l = want_int(i, &a[1])?;
        let from_l = want_int(i, &a[0])?;
        if to_l < from_l {
            return Err(i.error("End position is smaller than start position"));
        }
        let begv = bb.begv as i128 + 1;
        let zv = bb.zv as i128 + 1;
        if from_l < begv || to_l > zv {
            let s = i.q("args-out-of-range");
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
            &text,
            begv,
            from,
            to,
            &mut st,
            targetdepth,
            stopbefore,
            commentstop,
        );
        drop(text);
        (st.location, st.externalize(i))
    };
    b.borrow_mut().set_point(stop_pos);
    Ok(state)
}

fn f_syntax_ppss(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
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
    f_parse_partial_sexp(i, vec![beg, pos].into())
}

// ---------- modes ----------

fn f_fundamental_mode(i: &mut Interp, _a: crate::lisp::Argv) -> EvalResult {
    // GNU: (kill-all-local-variables) then set major-mode/mode-name and
    // run the hook.  Derived modes rely on the root doing the killing.
    crate::buffer::primitives::f_kill_all_local_variables(i, vec![].into())?;
    let mm = i.q("major-mode");
    let fmid = i.q("fundamental-mode");
    let mn = i.q("mode-name");
    {
        let b = cur(i);
        let mut bb = b.borrow_mut();
        bb.locals.insert(mm, Value::Sym(fmid));
        bb.locals.insert(mn, Value::string("Fundamental"));
    }
    let rmh = i.q("run-mode-hooks");
    // GNU simple.el calls `(run-mode-hooks)' with no HOOKS (there is no
    // `fundamental-mode-hook') and returns its nil.
    i.apply(&Value::Sym(rmh), vec![])?;
    Ok(Value::Nil)
}

// ---------- overlays ----------

fn overlay_target_buffer(
    i: &mut Interp,
    value: &Value,
    default: usize,
    message: &str,
) -> Result<crate::lisp::value::BufferRef, Flow> {
    let buffer = match value {
        Value::Nil => i.buffers.get(default).ok_or_else(|| i.error(message))?,
        Value::Buffer(buffer) => buffer.clone(),
        other => return Err(i.wrong_type_mut("bufferp", other)),
    };
    if !buffer.borrow().live {
        return Err(i.error(message));
    }
    Ok(buffer)
}

pub(crate) fn f_make_overlay(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let default = i.current_buffer;
    let b = overlay_target_buffer(
        i,
        &arg(&a, 2),
        default,
        "Attempt to create overlay in a dead buffer",
    )?;
    let (bid, len) = {
        let buffer = b.borrow();
        (buffer.id, buffer.text.len())
    };
    let mut s = want_int(i, &a[0])? - 1;
    let mut e = want_int(i, &a[1])? - 1;
    // GNU swaps BEG/END when reversed, then clamps into the buffer.
    if s > e {
        std::mem::swap(&mut s, &mut e);
    }
    let s = (s.max(0) as usize).min(len);
    let e = (e.max(0) as usize).min(len);
    let mut bb = b.borrow_mut();
    let idx = bb.overlays.len();
    // The Lisp handle is `[overlay BUFFER INDEX]' — kept on the
    // Overlay itself so queries return the identical object (`eq').
    let handle = Value::vector(vec![
        Value::Sym(i.q("overlay")),
        Value::Int(bid as i128),
        Value::Int(idx as i128),
    ]);
    i.register_overlay(&handle);
    bb.overlays.push(crate::buffer::Overlay {
        start: s,
        end: e,
        buffer: Some(bid),
        plist: Value::Nil,
        front_advance: arg(&a, 3).truthy(),
        rear_advance: arg(&a, 4).truthy(),
        evaporate: false,
        handle: handle.clone(),
    });
    Ok(handle)
}

fn overlay_of(i: &mut Interp, v: &Value) -> Result<(usize, usize), Flow> {
    if !i.is_overlay(v) {
        return Err(i.wrong_type_mut("overlayp", v));
    }
    if let Value::Vec(vec) = v {
        let vv = vec.borrow();
        if vv.len() == 3 || vv.len() == 7 {
            if let (Value::Sym(tag), Value::Int(bid), Value::Int(idx)) = (&vv[0], &vv[1], &vv[2]) {
                if i.sym_id(&Value::Sym(*tag)) == i.q_soft("overlay") {
                    return Ok((*bid as usize, *idx as usize));
                }
            }
        }
    }
    Err(i.wrong_type_mut("overlayp", v))
}

/// Look up the Overlay a handle points at, if its slot still exists.
fn overlay_slot(i: &Interp, handle: &Value) -> Option<crate::buffer::Overlay> {
    i.overlay_state(handle)
}

fn f_delete_overlay(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
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

fn f_delete_all_overlays(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let b = crate::buffer::primitives::buf_of(i, &arg(&a, 0))?;
    let mut bb = b.borrow_mut();
    for ov in &mut bb.overlays {
        ov.buffer = None;
    }
    Ok(Value::Nil)
}

fn f_move_overlay(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let (bid, idx) = overlay_of(i, &a[0])?;
    let state = overlay_slot(i, &a[0]);
    // Nil retains an attached overlay's buffer; detached overlays use current.
    let default = state
        .as_ref()
        .and_then(|overlay| overlay.buffer)
        .unwrap_or(i.current_buffer);
    let target = overlay_target_buffer(
        i,
        &arg(&a, 3),
        default,
        "Attempt to move overlay to a dead buffer",
    )?;
    let (nbid, len) = {
        let buffer = target.borrow();
        (buffer.id, buffer.text.len())
    };
    let mut s = want_int(i, &a[1])? - 1;
    let mut e = want_int(i, &a[2])? - 1;
    if s > e {
        std::mem::swap(&mut s, &mut e);
    }
    let s = (s.max(0) as usize).min(len);
    let e = (e.max(0) as usize).min(len);
    let Some(ov) = state else {
        return Ok(a[0].clone());
    };
    let detached = crate::buffer::Overlay::detached_state(&a[0]).is_some();
    if s == e {
        let property = Value::Sym(i.q("evaporate"));
        // overlay-get includes a category's inherited evaporate value.
        if f_overlay_get(i, vec![a[0].clone(), property].into())?.truthy() {
            f_delete_overlay(i, vec![a[0].clone()].into())?;
            return Ok(a[0].clone());
        }
    }
    if nbid != bid || detached {
        // Cross-buffer move: leave a dead stub behind and push a new
        // entry into the target buffer, then retarget the handle.
        if !detached {
            if let Some(ob) = i.buffers.get(bid) {
                if let Some(old) = ob.borrow_mut().overlays.get_mut(idx) {
                    old.buffer = None;
                }
            }
        }
        let nidx = {
            let mut nbb = target.borrow_mut();
            nbb.overlays.push(ov.clone());
            nbb.overlays.len() - 1
        };
        if let Value::Vec(v) = &a[0] {
            let mut vv = v.borrow_mut();
            vv[1] = Value::Int(nbid as i128);
            vv[2] = Value::Int(nidx as i128);
            vv.truncate(3);
        }
        let mut nbb = target.borrow_mut();
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

fn f_overlay_start(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    overlay_of(i, &a[0])?;
    Ok(match overlay_slot(i, &a[0]) {
        Some(ov) if ov.buffer.is_some() => Value::Int(ov.start as i128 + 1),
        _ => Value::Nil,
    })
}
fn f_overlay_end(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    overlay_of(i, &a[0])?;
    Ok(match overlay_slot(i, &a[0]) {
        Some(ov) if ov.buffer.is_some() => Value::Int(ov.end as i128 + 1),
        _ => Value::Nil,
    })
}
fn f_overlay_buffer(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    overlay_of(i, &a[0])?;
    Ok(match overlay_slot(i, &a[0]) {
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
fn f_overlay_put(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let (bid, idx) = overlay_of(i, &a[0])?;
    // GNU Foverlay_put accepts ANY Lisp object as the property name —
    // keys are compared with `eq' in the overlay plist (Foverlay_put's
    // CHECK_SYMBOL is on the OVERLAY arg, not PROP).
    let evap = i.q("evaporate");
    if let Some(mut ov) = overlay_slot(i, &a[0]) {
        if eq_values(&a[1], &Value::Sym(evap)) {
            // Cached flag so the deletion path needs no interner.
            ov.evaporate = a[2].truthy();
            // Foverlay_put immediately detaches an empty overlay
            // when its evaporate property becomes non-nil.
            if ov.evaporate && ov.start == ov.end {
                ov.buffer = None;
            }
        }
        // GNU Foverlay_put conses a NEW property onto the front of
        // the plist (existing keys update in place) — append-order
        // plist_put_v would print keys oldest-first, not GNU order.
        ov.plist = crate::lisp::eval::plist_put_front(&ov.plist, &a[1], a[2].clone());
        if crate::buffer::Overlay::detached_state(&a[0]).is_some() {
            ov.save_detached_state();
        } else if let Some(buffer) = i.buffers.get(bid) {
            if let Some(slot) = buffer.borrow_mut().overlays.get_mut(idx) {
                *slot = ov;
            }
        }
    }
    Ok(a[2].clone())
}
fn f_overlay_get(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    overlay_of(i, &a[0])?;
    Ok(match overlay_slot(i, &a[0]) {
        Some(ov) => ov.property(i, &a[1]),
        _ => Value::Nil,
    })
}
fn f_overlay_properties(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    overlay_of(i, &a[0])?;
    let plist = overlay_slot(i, &a[0]).map_or(Value::Nil, |ov| ov.plist);
    crate::lisp::builtins::seq::f_copy_sequence(i, vec![plist].into())
}
fn f_overlayp(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    Ok(Value::from_bool(i.is_overlay(&a[0])))
}

/// GNU's interval tree keeps overlays ordered by (start asc, newest
/// first).  `overlays-in'/`overlays-at' (SORTED nil) return that
/// order verbatim.
fn overlay_pos_order(hits: &mut Vec<(&crate::buffer::Overlay, usize)>) {
    hits.sort_by_key(|(ov, idx)| (ov.start, std::cmp::Reverse(*idx)));
}

/// `overlays-at POS SORTED' uses GNU priority/containment precedence,
/// highest first. Keep the existing slot order for exact ties.
fn overlay_sorted_order(i: &Interp, hits: &mut Vec<(&crate::buffer::Overlay, usize)>) {
    crate::buffer::sort_overlays_by_priority(i, hits);
    hits.reverse();
}

fn f_overlays_at(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let pos = want_int(i, &a[0])?.max(1) as usize - 1;
    let b = cur(i);
    let bb = b.borrow();
    let bid = bb.id;
    let mut hits: Vec<(&crate::buffer::Overlay, usize)> = bb
        .overlays
        .iter()
        .enumerate()
        .filter(|(_, ov)| ov.buffer == Some(bid) && pos >= ov.start && pos < ov.end)
        .map(|(idx, ov)| (ov, idx))
        .collect();
    // GNU: SORTED (any non-nil 2nd arg) sorts by descending priority;
    // the default is buffer position order.
    if arg(&a, 1).truthy() {
        overlay_sorted_order(i, &mut hits);
    } else {
        overlay_pos_order(&mut hits);
    }
    Ok(Value::list(
        hits.into_iter().map(|(ov, _)| ov.handle.clone()).collect(),
    ))
}
fn f_overlays_in(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let b = cur(i);
    let bb = b.borrow();
    let len = bb.text.len();
    // GNU Foverlays_in → overlays_in(BEG, END, empty=1) over the
    // interval tree (buffer.c + itree.c).  Working in GNU's 1-based
    // positions (B = beg, E = end, Z = len+1): the tree yields a node
    // when `ov.end > B && ov.begin < search_end' — search_end = Z+1
    // when END >= Z, else Z — plus empty nodes sitting exactly at B.
    // A node with `begin == E' qualifies only when empty AND (B == E
    // or END at the buffer end); `begin > E' never qualifies.  A
    // reversed range (B > E) yields nothing.
    let (s, e) = {
        // GNU overlays-in allows BEG > END (empty result); bounds are
        // decoded like region_bounds but swapped instead of signalled.
        let mut rd = |v: &Value| -> Result<Option<i128>, Flow> {
            match v {
                Value::Nil => Ok(None),
                Value::Int(n) => Ok(Some(*n)),
                Value::Marker(m) if m.borrow().buffer.is_some() => {
                    Ok(Some(m.borrow().position as i128 + 1))
                }
                Value::Marker(_) => Err(i.error("Marker does not point anywhere")),
                other => Err(i.wrong_type_mut("integer-or-marker-p", other)),
            }
        };
        // Clamp to the 1-based interval: BEG/END below 1 or above Z just
        // shrink the range (GNU overlays_in treats them as BEGV/ZV ends);
        // `sp as usize - 1` would underflow on BEG <= 0.
        let sp = rd(&a[0])?.unwrap_or(1).clamp(1, len as i128 + 1);
        let ep = rd(&a[1])?
            .unwrap_or(len as i128 + 1)
            .clamp(1, len as i128 + 1);
        if sp > ep {
            return Ok(Value::Nil);
        }
        (sp as usize - 1, ep as usize - 1)
    };
    let (big_b, big_e) = (s + 1, e + 1);
    let z = len + 1;
    let search_end = if big_e >= z { z + 1 } else { z };
    let bid = bb.id;
    let mut hits: Vec<(&crate::buffer::Overlay, usize)> = Vec::new();
    for (idx, ov) in bb.overlays.iter().enumerate() {
        if ov.buffer != Some(bid) {
            continue;
        }
        let (beg, end) = (ov.start + 1, ov.end + 1);
        let yielded = (end > big_b && beg < search_end) || (beg == end && beg == big_b);
        if !yielded || beg > big_e {
            continue;
        }
        if beg == big_e && !(beg == end && (big_b == big_e || big_e >= z)) {
            continue;
        }
        hits.push((ov, idx));
    }
    overlay_pos_order(&mut hits);
    Ok(Value::list(
        hits.into_iter().map(|(ov, _)| ov.handle.clone()).collect(),
    ))
}
fn f_overlays_at_point(i: &mut Interp, _a: crate::lisp::Argv) -> EvalResult {
    let pos = cur(i).borrow().point() as i128 + 1;
    f_overlays_at(i, vec![Value::Int(pos)].into())
}
fn f_next_overlay_change(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU Fnext_overlay_change: endpoints clamp into the accessible
    // region [BEGV+1, ZV+1] (narrowing adjusts them lazily); no
    // endpoint after POS → point-max.
    let pos = crate::lisp::builtins::want_int_or_mark(i, &a[0])?.max(1);
    let b = cur(i);
    let bb = b.borrow();
    Ok(Value::Int(
        crate::buffer::primitives::next_ov_change(&bb, pos),
    ))
}
fn f_prev_overlay_change(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU Fprevious_overlay_change: clamped endpoints before POS;
    // none → point-min (never nil).
    let pos = crate::lisp::builtins::want_int_or_mark(i, &a[0])?.max(1);
    let b = cur(i);
    let bb = b.borrow();
    Ok(Value::Int(
        crate::buffer::primitives::prev_ov_change(&bb, pos),
    ))
}
fn f_remove_overlays(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
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
    // NAME may be ANY Lisp object (overlay keys aren't symbol-only).
    let name = a.get(2).filter(|v| v.truthy());
    let val = arg(&a, 3);
    let mut bb = b.borrow_mut();
    for ov in &mut bb.overlays {
        if ov.buffer != Some(bid) {
            continue;
        }
        let inside = s <= ov.start && ov.end <= e;
        let prop_ok = match name {
            Some(ps) => eq_values(&crate::lisp::eval::plist_get_v(&ov.plist, ps), &val),
            None => true,
        };
        if inside && prop_ok {
            ov.buffer = None;
        }
    }
    Ok(Value::Nil)
}

// ---------- help/docs ----------

/// Resolve a DOC offset or an external .elc docstring reference. File
/// offsets are byte offsets, with the #@/unit-separator validation and
/// control-character escaping used by GNU get_doc_string.
fn documentation_reference(i: &mut Interp, reference: &Value) -> EvalResult {
    if let Value::Int(offset) = reference {
        if *offset == 0 {
            return Ok(Value::Nil);
        }
        return Ok(offset
            .checked_abs()
            .and_then(|n| usize::try_from(n).ok())
            .and_then(|n| crate::lisp::builtins::misc::doc_text_at(i, n))
            .map(Value::string)
            .unwrap_or(Value::Nil));
    }
    let Value::Cons(pair) = reference else {
        return Ok(Value::Nil);
    };
    let (file, offset) = {
        let p = pair.borrow();
        (p.car.clone(), p.cdr.int().unwrap_or(0))
    };
    let Value::Str(file) = file else {
        return Ok(Value::Nil);
    };
    let directory = i.q("lisp-directory");
    let Value::Str(directory) = i.symbol_value(directory) else {
        return Ok(Value::Nil);
    };
    let file = file.borrow().clone();
    let path = if native_path(i, &file)?.is_absolute() {
        file.clone()
    } else {
        format!("{}{}", directory.borrow(), file).into()
    };
    let bytes = match std::fs::read(native_path(i, &path)?) {
        Ok(bytes) => bytes,
        Err(error)
            if error.kind() == std::io::ErrorKind::NotFound
                || error.raw_os_error() == Some(libc::ENOTDIR) =>
        {
            return Ok(Value::string(format!(
                "Cannot open doc string file \"{}\"\n",
                file
            )));
        }
        Err(error) => {
            return Err(report_file_errno(
                i,
                "Read error on documentation file",
                vec![Value::string(file)],
                &error,
            ));
        }
    };
    let Some(offset) = offset.checked_abs().and_then(|n| usize::try_from(n).ok()) else {
        return Ok(Value::Nil);
    };
    if offset == 0 || offset > bytes.len() {
        return Ok(Value::Nil);
    }
    if bytes[offset - 1] != 31 {
        if bytes[offset - 1] != b' ' {
            return Ok(Value::Nil);
        }
        let mut at = offset - 1;
        while at > 0 && bytes[at - 1].is_ascii_digit() {
            at -= 1;
        }
        if at < 2 || &bytes[at - 2..at] != b"#@" {
            return Ok(Value::Nil);
        }
    }
    let end = bytes[offset..]
        .iter()
        .position(|b| *b == 31)
        .map(|n| offset + n)
        .unwrap_or(bytes.len());
    let mut decoded = Vec::new();
    let mut at = offset;
    while at < end {
        let byte = bytes[at];
        at += 1;
        if byte != 1 {
            decoded.push(byte);
            continue;
        }
        let escape = if at < end { bytes[at] } else { 0 };
        at += 1;
        decoded.push(match escape {
            1 => 1,
            b'0' => 0,
            b'_' => 31,
            value => {
                return Err(i.error(format!(
                    "Invalid data in documentation file -- \u{1} followed by code {:03o}",
                    value
                )));
            }
        });
    }
    // Docstrings use Emacs's internal string bytes: C0/C1 escape raw
    // eight-bit characters and surrogate codes are valid storage chars.
    let mut text = EmacsText::new();
    let mut at = 0;
    let mut start = 0;
    while at < decoded.len() {
        let raw = matches!(decoded[at], 0xc0 | 0xc1)
            && decoded.get(at + 1).is_some_and(|b| b & 0xc0 == 0x80);
        let surrogate = decoded[at] == 0xed
            && decoded
                .get(at + 1)
                .is_some_and(|b| (0xa0..=0xbf).contains(b))
            && decoded.get(at + 2).is_some_and(|b| b & 0xc0 == 0x80);
        if raw || surrogate {
            text.push_str(&(crate::lisp::builtins::coding_legacy::internal_decode(
                &decoded[start..at],
            )).to_unicode_lossy());
            let code = if raw {
                0x3fff00 + (((decoded[at] & 1) as u32) << 6 | decoded[at + 1] as u32)
            } else {
                (((decoded[at] & 15) as u32) << 12)
                    | (((decoded[at + 1] & 63) as u32) << 6)
                    | (decoded[at + 2] & 63) as u32
            };
            text.push(crate::lisp::value::lisp_char(code).unwrap());
            at += if raw { 2 } else { 3 };
            start = at;
        } else {
            at += 1;
        }
    }
    text.push_str(&(crate::lisp::builtins::coding_legacy::internal_decode(
        &decoded[start..],
    )).to_unicode_lossy());
    Ok(Value::string(text))
}

fn f_documentation_property(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let sid = want_sym(i, &a[0])?;
    // GNU doc.c uses Fget: only SYMBOL is type-checked; PROP is an
    // arbitrary eq-comparable key, just as with get and put.
    let prop = &a[1];
    let mut tem = crate::lisp::eval::plist_get_v(&i.obarray.symbol(sid).plist, prop);
    let vd = i.q("variable-documentation");
    if tem.is_nil() && i.sym_id(prop) == Some(vd) {
        // GNU doc.c: a variable with no doc of its own falls through
        // to its `variable-alias' target's property.
        let ind = i.var_alias_target(sid);
        if ind != sid {
            tem = i.get_prop(ind, vd);
        }
    }
    // GNU: fixnum 0 counts as no doc.
    if matches!(&tem, Value::Int(n) if *n == 0) {
        tem = Value::Nil;
    }
    // References share the same lookup as function values; other
    // property objects are evaluated as documentation forms.
    match &tem {
        Value::Int(_) if tem.fixnump() => {
            tem = documentation_reference(i, &tem)?;
        }
        Value::Cons(pair) if pair.borrow().cdr.fixnump() => {
            tem = documentation_reference(i, &tem)?;
        }
        // A non-string non-offset doc property is an eval form in GNU.
        Value::Nil | Value::Str(_) => {}
        other => {
            let form = other.clone();
            tem = i.eval(&form)?;
        }
    }
    // RAW nil passes strings through `substitute-command-keys'.
    let raw = a.get(2).map(|v| v.truthy()).unwrap_or(false);
    if !raw {
        if let Value::Str(s) = &tem {
            let sck = Value::Sym(i.q("substitute-command-keys"));
            if let Ok(v) = i.apply(&sck, vec![Value::Str(s.clone())]) {
                tem = v;
            }
        }
    }
    Ok(tem)
}

/// Raw lambda/autoload/keymap values and specialized function objects
/// use the same function-documentation protocol as GNU doc.c.
fn documentation_function_value(i: &mut Interp, function: &Value, raw: &Value) -> EvalResult {
    let getter = Value::Sym(i.q("function-documentation"));
    let doc = i.apply(&getter, vec![function.clone()])?;
    let doc = match &doc {
        Value::Int(_) if doc.fixnump() => documentation_reference(i, &doc)?,
        Value::Cons(pair) if pair.borrow().cdr.fixnump() => documentation_reference(i, &doc)?,
        _ => doc,
    };
    if raw.truthy() {
        Ok(doc)
    } else {
        let substitute = Value::Sym(i.q("substitute-command-keys"));
        i.apply(&substitute, vec![doc])
    }
}

fn f_documentation(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let f = match &a[0] {
        Value::Nil | Value::Sym(_) => {
            let id = want_sym(i, &a[0])?;
            // GNU Fdocumentation: a symbol's `function-documentation'
            // property is consulted even when the function cell is
            // void (void-function only fires when there is no prop).
            // The prop also wins over the function's own docstring —
            // this is how `defalias' with a DOC arg overrides the
            // aliased function's text.
            let fd = i.q("function-documentation");
            if !i.get_prop(id, fd).is_nil() {
                let mut dargs = vec![a[0].clone(), Value::Sym(fd)];
                if let Some(raw) = a.get(1) {
                    dargs.push(raw.clone());
                }
                return f_documentation_property(i, dargs.into());
            }
            // Resolve the complete alias chain using the native
            // indirect-function implementation. Only the original
            // symbol's DOC property overrides the resolved definition.
            let v = crate::lisp::builtins::data::f_indirect_function(i, vec![a[0].clone()].into())?;
            if v.is_nil() || matches!(v, Value::Sym(s) if s == crate::lisp::sym::UNBOUND) {
                return Err(i.signal_data(crate::lisp::sym::VOID_FUNCTION, vec![a[0].clone()]));
            }
            v
        }
        other => {
            if !matches!(
                other,
                Value::Lambda(_) | Value::Subr(_) | Value::Cons(_) | Value::Str(_) | Value::Vec(_)
            ) {
                return Err(i.signal_data(crate::lisp::sym::INVALID_FUNCTION, vec![other.clone()]));
            }
            other.clone()
        }
    };
    // GNU `documentation-function-or-1': `(macro . FUN)' steps into FUN —
    // a macro's docstring lives on the inner function.
    let f = crate::lisp::builtins::data::macro_inner(i, f);
    match f.as_lambda() {
        Some(l) => match &l.doc {
            Some(d) => {
                // RAW non-nil returns the docstring verbatim.  Otherwise
                // GNU runs the text through `substitute-command-keys'
                // (which also applies the quoting-style transform) and
                // appends the `(fn ARGLIST)' usage trailer.
                if !matches!(arg(&a, 1), Value::Nil) {
                    return Ok(Value::string(d.clone()));
                }
                let sck = Value::Sym(i.q("substitute-command-keys"));
                let sub = i.apply(&sck, vec![Value::string(d.clone())])?;
                let mut t = match &sub {
                    Value::Str(s) => s.borrow().clone(),
                    _ => d.clone(),
                };
                if l.dumped_doc {
                    if let Some(al) = &l.arglist {
                        let mut usage = String::from("\n\n(fn");
                        let mut cur = al.clone();
                        loop {
                            match cur {
                                Value::Cons(c) => {
                                    let (h, tl) = {
                                        let cc = c.borrow();
                                        (cc.car.clone(), cc.cdr.clone())
                                    };
                                    match &h {
                                        Value::Sym(s) => {
                                            let n = i.symbol_name(*s);
                                            if n.starts_with('&') {
                                                usage.push(' ');
                                                usage.push_str(&n);
                                            } else {
                                                usage.push(' ');
                                                usage.push_str(&n.to_uppercase());
                                            }
                                        }
                                        Value::Cons(cc) => {
                                            // (var init) or (var) optional
                                            // form: GNU prints just the var
                                            // name.
                                            if let Value::Sym(s) = &cc.borrow().car {
                                                usage.push(' ');
                                                usage.push_str(&i.symbol_name(*s).to_uppercase());
                                            }
                                        }
                                        _ => {}
                                    }
                                    cur = tl;
                                }
                                Value::Sym(s) => {
                                    // Dotted arglist tail (&rest var printed
                                    // as `. VAR' only in raw forms — fundoc
                                    // uses `&rest VAR').
                                    usage.push_str(" &rest ");
                                    usage.push_str(&i.symbol_name(s).to_uppercase());
                                    break;
                                }
                                _ => break,
                            }
                        }
                        usage.push(')');
                        t.push_str(&usage);
                    }
                }
                Ok(Value::string(t))
            }
            None => documentation_function_value(i, &f, &arg(&a, 1)),
        },
        None => match &f {
            Value::Subr(s) => {
                if let Some(documentation) = crate::lisp::builtins::comp::native_documentation(s) {
                    return Ok(documentation);
                }
                if let Some(t) = crate::lisp::builtins::misc::doc_text(i, &format!("F{}", s.name)) {
                    // `documentation' applies substitute-quotes:
                    // `...' -> ‘...’ unless text-quoting-style is
                    // 'grave/'straight.
                    // RAW non-nil returns the docstring verbatim.
                    if !matches!(arg(&a, 1), Value::Nil) {
                        return Ok(Value::string(t));
                    }
                    return Ok(Value::string(
                        crate::lisp::builtins::evalfn::translate_message_quotes(i, &t),
                    ));
                }
                if s.doc.is_empty() {
                    Ok(Value::Nil)
                } else {
                    Ok(Value::string(s.doc))
                }
            }
            _ => documentation_function_value(i, &f, &arg(&a, 1)),
        },
    }
}

fn f_apropos_internal(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let pat = want_str(i, &a[0])?;
    let re = crate::lisp::regexp::compile_case(&pat, false)
        .map_err(|e| err_sym(i, "invalid-regexp", vec![Value::string(e.0)]))?;
    let mut out = Vec::new();
    for id in i.obarray.all_ids() {
        // GNU collects every interned symbol matching REGEXP when
        // PREDICATE is nil — no boundp/fboundp/plist filter (verified:
        // GNU -Q lists an explicitly-interned unbound symbol).
        let name = i.symbol_name(id);
        let syn = crate::editor::re_syntax(i);
        let chars: Vec<char> = name.chars().collect();
        if crate::lisp::regexp::search(
            &re,
            &chars,
            0,
            &syn,
            &crate::editor::re_category(i),
            crate::lisp::regexp::NO_POINT,
        )
        .is_some()
        {
            out.push(i.sym(id));
        }
    }
    Ok(Value::list(out))
}

/// Look up a key sequence (as Int event codes) in the active maps:
/// local map, then the global map. Returns the bound value, or None.
pub(crate) fn lookup_command_in_maps(
    i: &mut Interp,
    keys: &[Value],
) -> Result<Option<Value>, Flow> {
    let codes: Vec<i128> = keys.iter().filter_map(|v| v.int()).collect();
    let local = {
        let Some(b) = i.current_buffer_ref() else {
            return Ok(None);
        };
        let lb = b.borrow();
        lb.locals
            .get(&i.q_soft("local-keymap").unwrap_or(u32::MAX))
            .cloned()
            .unwrap_or(Value::Nil)
    };
    // GNU's active-map order: minor modes, local map, global.
    let mut map_list = Vec::new();
    if let Ok(minors) = current_minor_maps(i) {
        map_list.extend(minors.into_iter().map(|(_, m)| m));
    }
    map_list.push(local);
    map_list.push(i.symbol_value(i.q_soft("global-map").unwrap_or(0)));
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
                Err(_) => return Ok(None),
            };
            let def = match keymap_def(i, raw.clone()) {
                Ok(v) => v,
                Err(_) => return Ok(None),
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
            return Ok(Some(km));
        }
        if last_def.truthy() {
            // GNU get_keyelt: unwrap menu-item bindings and apply
            // `:filter'; a nil filter result hides the binding so the
            // next map in the list is consulted.
            let resolved = keyelt_command(i, &last_def)?;
            if resolved.truthy() {
                return Ok(Some(resolved));
            }
        }
    }
    Ok(None)
}

// ---------- faces (minimal tty model) ----------

/// Face names GNU binds in a batch session (a subset of face-list;
/// enough for facep/face-list lookups).
pub(crate) const KNOWN_FACES: &[&str] = &[
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

pub(crate) fn builtin_face_name(i: &Interp, face: SymId) -> Option<&'static str> {
    KNOWN_FACES.iter().copied().find(|name| i.q_soft(name) == Some(face))
}

pub(crate) fn face_symbol(i: &Interp, value: &Value) -> Option<SymId> {
    if let Some(face) = i.sym_id(value) { return Some(face); }
    let Value::Str(name) = value else { return None; };
    let text = name.borrow();
    let key = if i.is_unibyte_str(name) {
        crate::lisp::obarray::InternKey::from_text_kind(&*text, false)
    } else if i.is_multibyte_str(name) {
        crate::lisp::obarray::InternKey::from_text_kind(&*text, true)
    } else { crate::lisp::obarray::InternKey::from_text(&*text) };
    i.obarray.lookup_key(&key)
}

pub(crate) fn face_known(i: &Interp, face: &SymId) -> bool {
    i.face_table.iter().any(|(key, _)| key == face) || builtin_face_name(i, *face).is_some()
}
/// GNU face attributes accepted by the internal setter. The legacy
/// :bold/:italic aliases are write-only; the getter rejects them.
pub(crate) fn face_known_attr(attr: &str) -> bool {
    const KNOWN_ATTRS: &[&str] = &[
        ":font", ":fontset", ":family", ":foundry", ":width", ":height",
        ":weight", ":slant", ":foreground", ":distant-foreground",
        ":background", ":underline", ":overline", ":strike-through",
        ":box", ":inverse-video", ":stipple", ":extend", ":inherit",
        ":bold", ":italic",
    ];
    KNOWN_ATTRS.contains(&attr)
}
/// GNU's `face--new-frame-defaults' entry for NAME: `(face-id . lface)'
/// — the face-id lives in the symbol's `face' property (assigned on
/// first definition like xfaces.c's next_lface_id), the lface is the
/// canonical 20-slot [face ATTR...] vector (slot order per GNU's
/// lface_attribute_index).
pub(crate) fn face_register_global(i: &mut Interp, name: &SymId) {
    let sym = *name;
    let face_prop = i.q("face");
    if i.get_prop(sym, face_prop).is_nil() {
        // IDs: built-ins keep GNU's fixed small ids; user faces grow
        // past them like GNU's next_lface_id.
        let id = if let Some(p) = KNOWN_FACES.iter().position(|n| i.q_soft(n) == Some(sym)) {
            p as i128
        } else {
            let taken: i128 = i
                .face_table
                .iter()
                .filter_map(|(n, _)| {
                    Some(i.get_prop(*n, face_prop))
                        .and_then(|v| match v {
                            Value::Int(n) => Some(n),
                            _ => None,
                        })
                })
                .max()
                .unwrap_or((KNOWN_FACES.len() - 1) as i128);
            taken + 1
        };
        i.put_prop(sym, face_prop, Value::Int(id));
    }
    let mut v = vec![Value::Sym(i.q("face"))];
    let un = Value::Sym(i.q("unspecified"));
    v.resize(20, un);
    let lface = Value::vector(v);
    let id = i.get_prop(sym, face_prop);
    if let Some(d) = i.q_soft("face--new-frame-defaults") {
        if let Value::Hash(h) = i.symbol_value(d) {
            let key = crate::lisp::builtins::hashfn::hash_key_for(
                i,
                &Value::Sym(sym),
                crate::lisp::HashTest::Eq,
            );
            h.borrow_mut()
                .map
                .insert(key.clone(), Value::cons(id, lface));
            h.borrow_mut().put_key(key, Value::Sym(sym));
        }
    }
}
/// Write ATTR=VAL into the global lface stored inside
/// `face--new-frame-defaults' for NAME (GNU writes the lface slots
/// directly; our canonical text attrs live in `face_t_table' — this
/// mirrors them into the vector so `frame-face-alist'/dumped-face
/// readers see the same values).
pub(crate) fn face_global_update(i: &mut Interp, name: &SymId, attr: &str, val: Value) {
    const SLOTS: &[&str] = &[
        ":family",
        ":foundry",
        ":width",
        ":height",
        ":weight",
        ":slant",
        ":underline",
        ":inverse-video",
        ":foreground",
        ":background",
        ":stipple",
        ":overline",
        ":strike-through",
        ":box",
        ":font",
        ":inherit",
        ":fontset",
        ":distant-foreground",
        ":extend",
    ];
    let idx = match SLOTS.iter().position(|a| *a == attr) {
        Some(p) => p + 1,
        None => return,
    };
    let sym = *name;
    let defaults = match i.q_soft("face--new-frame-defaults") {
        Some(d) => d,
        None => return,
    };
    let entry = {
        let hv = i.symbol_value(defaults);
        if let Value::Hash(h) = &hv {
            let key = crate::lisp::builtins::hashfn::hash_key_for(
                i,
                &Value::Sym(sym),
                crate::lisp::HashTest::Eq,
            );
            h.borrow().map.get(&key).cloned()
        } else {
            None
        }
    };
    if let Some(Value::Cons(c)) = entry {
        let lface = c.borrow().cdr.clone();
        if let Value::Vec(v) = lface {
            if idx < v.borrow().len() {
                v.borrow_mut()[idx] = val;
            }
        }
    }
}

/// GNU `face-alias' chains resolve to the target face's name
/// (nil when the name never resolves to a known face).
pub(crate) fn face_resolve(i: &Interp, face: &SymId) -> Option<SymId> {
    let mut face = *face;
    for _ in 0..16 {
        if face_known(i, &face) { return Some(face); }
        let alias = i.get_prop(face, i.q_soft("face-alias").unwrap_or(u32::MAX));
        face = i.sym_id(&alias)?;
    }
    None
}

fn face_name_of(i: &mut Interp, value: &Value) -> Result<SymId, Flow> {
    if let Some(face) = face_symbol(i, value).and_then(|face| face_resolve(i, &face)) {
        return Ok(face);
    }
    Err(invalid_face(i, value))
}

/// GNU `Fcheck_face' failure: the signal's data is
/// `("Invalid face" . FACE)' for cons/nil faces and
/// `("Invalid face" FACE)' for atoms; string args intern first.
pub(crate) fn invalid_face(i: &mut Interp, v: &Value) -> Flow {
    let face = match v {
        Value::Str(s) => Value::Sym(i.intern(&s.borrow())),
        _ => v.clone(),
    };
    let msg = Value::string("Invalid face");
    // GNU lface_from_face_name splices *proper* face lists into the
    // error data ((a b) → "Invalid face: a, b") but wraps atoms and
    // improper lists as a single datum ((a . b) → "(a . b)").
    let is_proper = match &face {
        Value::Cons(_) => face.list_to_vec().is_ok(),
        Value::Nil => true,
        _ => false,
    };
    let data = if is_proper {
        Value::cons(msg, face)
    } else {
        Value::cons(msg, Value::cons(face, Value::Nil))
    };
    let sym = i.q("error");
    i.signal(sym, data)
}

/// One attribute of a face, GNU-style (`unspecified' when unset).
/// GNU layers: FRAME=t reads the *new-frame defaults* lface — set only
/// by `(set-face-attribute FACE t ...)', NOT by the defface spec — so
/// it reports `unspecified' for spec-only faces; a real frame or nil
/// reads the realized value (defface specs land in `face_table').
fn face_attr(i: &mut Interp, name: &SymId, attr: &str, frame_is_t: bool) -> Value {
    if frame_is_t {
        // New-frame defaults: only `(set-face-attribute FACE t ...)'
        // writes here — defface specs don't.
        if let Some((_, Value::Cons(_))) = i.face_t_table.iter().find(|(n, _)| n == name) {
            let (_, plist) = i.face_t_table.iter().find(|(n, _)| n == name).unwrap();
            if let Some(v) = plist.list_to_vec().ok().and_then(|items| {
                items.chunks(2).find_map(|kv| match (&kv[0], kv.get(1)) {
                    (Value::Sym(k), Some(v)) if i.symbol_name(*k) == attr => Some(v.clone()),
                    _ => None,
                })
            }) {
                return v;
            }
        }
        return Value::Sym(i.q("unspecified"));
    }
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
    match (builtin_face_name(i, *name).unwrap_or(""), attr) {
        ("default", ":weight") | ("default", ":slant") | ("default", ":width") => {
            Value::Sym(i.q("normal"))
        }
        ("default", ":background") => Value::string("unspecified-bg"),
        ("default", ":foreground") => Value::string("unspecified-fg"),
        ("default", ":family") | ("default", ":foundry") => Value::string("default"),
        ("fixed-pitch", ":family") => Value::string("Monospace"),
        ("variable-pitch", ":family") => Value::string("Sans Serif"),
        ("default", ":height") => Value::Int(1),
        (
            "default",
            ":underline" | ":overline" | ":strike-through" | ":box" | ":inverse-video" | ":stipple"
            | ":extend" | ":inherit",
        ) => Value::Nil,
        ("bold", ":weight") | ("bold-italic", ":weight") => Value::Sym(i.q("bold")),
        ("italic", ":slant") | ("bold-italic", ":slant") => Value::Sym(i.q("italic")),
        ("underline", ":underline") => Value::t(),
        _ => Value::Sym(i.q("unspecified")),
    }
}

fn f_face_attribute(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU decode_live_frame on FRAME wins over every other arg check:
    // t → spec layer, nil/live frame → realized value, else frame-live-p.
    match &arg(&a, 2) {
        Value::Nil => {}
        Value::Sym(s) if *s == sym::T => {}
        Value::Frame(f) if !f.borrow().dead => {}
        other => return Err(i.wrong_type_mut("frame-live-p", other)),
    }
    let name = face_name_of(i, &a[0])?;
    // GNU resolves the name first, then requires the original FACE to be a symbol.
    let _ = want_sym(i, &a[0])?;
    let attr = match &a[1] {
        Value::Sym(s) => i.symbol_name(*s),
        other => return Err(i.wrong_type_mut("symbolp", other)),
    };
    // The setter's legacy aliases are not readable attributes.
    if !face_known_attr(&attr) || matches!(attr.as_str(), ":bold" | ":italic") {
        return Err(i.signal_data(
            sym::ERROR,
            vec![Value::string("Invalid face attribute name"), a[1].clone()],
        ));
    }
    // FRAME=t → stored spec layer (validated above).
    let spec = matches!(arg(&a, 2), Value::Sym(s) if s == sym::T);
    Ok(face_attr(i, &name, &attr, spec))
}

fn f_facep(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU `internal-lisp-face-p' decodes
    // FRAME first — `decode_live_frame' signals frame-live-p on any
    // non-frame arg even when FACE is invalid — then looks the face up
    // (unknown or non-symbol names just yield nil, so face-spec-set can
    // use it as a plain predicate during early startup).
    match arg(&a, 1) {
        Value::Nil => {}
        Value::Frame(f) if !f.borrow().dead => {}
        other => return Err(i.wrong_type_mut("frame-live-p", &other)),
    }
    if face_symbol(i, &a[0]).and_then(|face| face_resolve(i, &face)).is_none() {
        return Ok(Value::Nil);
    }
    let mut v = Vec::with_capacity(20);
    v.push(Value::Sym(i.q("face")));
    let un = Value::Sym(i.q("unspecified"));
    v.resize(20, un);
    Ok(Value::vector(v))
}

fn f_face_list(i: &mut Interp, _a: crate::lisp::Argv) -> EvalResult {
    let mut faces: Vec<SymId> = i.face_table.iter().rev().map(|(face, _)| *face)
        .filter(|face| builtin_face_name(i, *face).is_none()).collect();
    faces.extend(KNOWN_FACES.iter().rev().map(|name| i.q(name)));
    Ok(Value::list(faces.into_iter().map(Value::Sym).collect()))
}

fn f_face_id(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU faces.el `face-id': `(check-face face)' first — "Not a
    // face: FACE" on non-faces — then the symbol's `face' property
    // (the id assigned at registration); when that's nil GNU follows
    // `face-alias' and recurses.  A face registered by
    // `internal-make-lisp-face' (raw entry, no id) has no `face' prop,
    // so the id lookup must NOT fall back to the face-table index —
    // GNU returns whatever the prop holds.
    let mut face = a[0].clone();
    for _ in 0..16 {
        // check-face → internal-lisp-face-p → lface_from_face_name:
        // a `face-alias' chain resolves, so aliases pass check-face.
        let known = face_symbol(i, &face).and_then(|symbol| face_resolve(i, &symbol)).is_some();
        if !known {
            let sym = i.q("error");
            let shown = i.princ_to_string(&face);
            return Err(i.signal_data(sym, vec![Value::string(format!("Not a face: {shown}"))]));
        }
        // (get face 'face) — only meaningful for symbol faces.
        let sym_id = i.sym_id(&face);
        let face_kw = i.q("face");
        let alias_kw = i.q("face-alias");
        let fid = sym_id.map(|s| i.get_prop(s, face_kw)).unwrap_or(Value::Nil);
        if let Value::Int(_) = fid {
            return Ok(fid);
        }
        // (face-id (get face 'face-alias)) — recurse into the alias.
        let alias = sym_id
            .map(|s| i.get_prop(s, alias_kw))
            .unwrap_or(Value::Nil);
        match alias {
            Value::Nil => {
                // Registered face with no id and no alias: GNU's
                // recursion lands on nil and check-face's it.
                let sym = i.q("error");
                return Err(i.signal_data(sym, vec![Value::string("Not a face: nil".to_string())]));
            }
            _ => face = alias,
        }
    }
    let sym = i.q("error");
    Err(i.signal_data(sym, vec![Value::string("Not a face: nil".to_string())]))
}

fn f_face_equal(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU `internal-lisp-face-equal-p': true iff every attribute of
    // the two faces compares equal on the requested layer — FRAME=t
    // is the stored spec layer, nil/a real frame the realized one.
    // GNU validates FRAME (decode_live_frame) before the face names —
    // only nil, t (spec layer) or a live frame pass.
    face_frame_live(i, &arg(&a, 2))?;
    let n1 = face_name_of(i, &a[0])?;
    let n2 = face_name_of(i, &a[1])?;
    let spec = matches!(arg(&a, 2), Value::Sym(s) if s == sym::T);
    const ATTRS: &[&str] = &[
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
    for at in ATTRS {
        let v1 = face_attr(i, &n1, at, spec);
        let v2 = face_attr(i, &n2, at, spec);
        if !crate::lisp::builtins::equal_values(i, &v1, &v2)? {
            return Ok(Value::Nil);
        }
    }
    Ok(Value::t())
}

fn set_face_attr(i: &mut Interp, name: &SymId, attr: &str, val: Value) {
    let kw = i.intern(attr);
    // Built-in faces in KNOWN_FACES satisfy face_known without a
    // face_table row — a write must still land, so key the row on
    // the table itself, not on face_known.
    if !i.face_table.iter().any(|(n, _)| n == name) {
        i.face_table.push((*name, Value::Nil));
    }
    face_register_global(i, name);
    if let Some((_, plist)) = i.face_table.iter_mut().find(|(n, _)| n == name) {
        *plist = crate::lisp::eval::plist_put(plist, kw, val);
    }
}

fn f_make_face(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let name = match &a[0] {
        Value::Sym(s) => *s,
        other => return Err(i.wrong_type_mut("symbolp", other)),
    };
    if !face_known(i, &name) {
        i.face_table.push((name.clone(), Value::Nil));
    }
    // GNU make-face seeds the global definition + face id.
    face_register_global(i, &name);
    Ok(a[0].clone())
}

fn f_copy_face(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU Fcopy_face: both args are CHECK_SYMBOL, NEW_NAME
    // checked before OLD_FACE.
    let to = match &a[1] {
        Value::Sym(s) => *s,
        other => return Err(i.wrong_type_mut("symbolp", other)),
    };
    let from = match i.sym_id(&a[0]) {
        Some(s) => s,
        None => return Err(i.wrong_type_mut("symbolp", &a[0])),
    };
    // check_face: an unknown OLD_FACE signals `Invalid face'.
    if !face_known(i, &from) {
        return Err(invalid_face(i, &a[0]));
    }
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
        // GNU Fcopy_face copies each attribute's value as realized on
        // the default frame (booting font-selection), i.e. nil frame —
        // not the `unspecified'-padded spec layer.
        let v = face_attr(i, &from, attr, false);
        set_face_attr(i, &to, attr, v);
    }
    Ok(a[1].clone())
}

fn f_internal_set_lisp_face_attribute(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU checks FACE and ATTR as symbols before resolving FRAME.
    let mut face = match i.sym_id(&a[0]) {
        Some(s) => s,
        None => return Err(i.wrong_type_mut("symbolp", &a[0])),
    };
    let mut attr = match i.sym_id(&a[1]) {
        Some(s) => i.symbol_name(s),
        None => return Err(i.wrong_type_mut("symbolp", &a[1])),
    };
    // Resolve aliases before creating anything, including aliases whose
    // target has no face yet. Cycles signal without creating a face.
    let alias = i.q("face-alias");
    let mut seen = std::collections::HashSet::new();
    while face != sym::NIL {
        if !seen.insert(face) {
            let condition = i.q("circular-list");
            return Err(i.signal_data(condition, vec![a[0].clone()]));
        }
        let target = i.get_prop(face, alias);
        match i.sym_id(&target) {
            Some(s) if target.truthy() => face = s,
            _ => break,
        }
    }
    let name = face;
    face_frame_live(i, &arg(&a, 3))?;
    // FRAME=t (4th arg) → new-frame defaults (face_t_table), mirrored
    // into the global lface inside `face--new-frame-defaults' like
    // GNU (which stores the lface directly); `frame' 0 → the defaults
    // AND every existing frame — both layers in our flat model; a
    // real frame or nil → the spec/realized layer only.
    let to_t = matches!(arg(&a, 3), Value::Sym(s) if s == sym::T);
    let zero = matches!(arg(&a, 3), Value::Int(0));
    if !face_known(i, &name) {
        // FRAME=t needs an existing global definition. FRAME=0 visits
        // that layer first. A live frame creates the missing face even
        // when the following attribute/value validation will fail.
        if to_t || zero {
            return Err(invalid_face(i, &Value::Sym(face)));
        }
        i.face_table.push((name.clone(), Value::Nil));
        face_register_global(i, &name);
    }
    if !face_known_attr(&attr) {
        return Err(i.signal_data(
            sym::ERROR,
            vec![Value::string("Invalid face attribute name"), a[1].clone()],
        ));
    }
    // GNU FRAME=t stores :ignore-defface for `unspecified' so frame
    // defaults ignore the defface layer.
    let mut value = a[2].clone();
    if to_t {
        if let Value::Sym(s) = &value {
            if i.symbol_name(*s) == "unspecified" {
                value = Value::Sym(i.q(":ignore-defface"));
            }
        }
    }
    if matches!(attr.as_str(), ":bold" | ":italic") {
        let bold = attr == ":bold";
        let reset = matches!(&value, Value::Sym(s) if i.symbol_name(*s) == "reset");
        if !reset {
            value = Value::Sym(i.intern(if value.truthy() {
                if bold { "bold" } else { "italic" }
            } else {
                "normal"
            }));
        }
        attr = if bold { ":weight" } else { ":slant" }.to_string();
    }
    // GNU stores :box t as fixnum 1.
    if attr == ":box" && matches!(&value, Value::Sym(s) if *s == sym::T) {
        value = Value::Int(1);
    }
    check_face_value(i, &name, &attr, &value)?;
    if to_t || zero {
        face_register_global(i, &name);
        if !i.face_t_table.iter().any(|(n, _)| *n == name) {
            i.face_t_table.push((name.clone(), Value::Nil));
        }
        let kw = i.intern(&attr);
        if let Some((_, plist)) = i.face_t_table.iter_mut().find(|(n, _)| *n == name) {
            *plist = crate::lisp::eval::plist_put(plist, kw, value.clone());
        }
        face_global_update(i, &name, &attr, value.clone());
    }
    if !to_t {
        set_face_attr(i, &name, &attr, value);
    }
    if i.symbol_name(name) == "cursor" && attr == ":background" {
        if let Some(cursor) = i.intern_soft("cursor") {
            let override_property = i.intern("remacs--cursor-face-background-override");
            i.put_prop(cursor, override_property, Value::t());
        }
    }
    Ok(Value::Sym(face))
}

pub(crate) fn f_set_face_attribute(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU Fset_face_attribute1: with an empty ARGS list it returns
    // before FACE is validated at all — `(set-face-attribute "a")'
    // is nil.  With ARGS present, CHECK_SYMBOL on FACE first, then
    // FRAME (arg 2) decodes (check_lface_frame) before the
    // face-existence test.
    if a.len() <= 2 {
        return Ok(Value::Nil);
    }
    let name = match i.sym_id(&a[0]) {
        Some(s) => s,
        None => return Err(i.wrong_type_mut("symbolp", &a[0])),
    };
    // GNU Fset_face_attribute1 decodes FRAME (arg 2) after CHECK_SYMBOL
    // on FACE and before the face must exist.
    face_frame_live(i, &arg(&a, 1))?;
    if !face_known(i, &name) {
        let err = i.q("error");
        return Err(i.signal_data(
            err,
            vec![Value::string("Invalid face".to_string()), a[0].clone()],
        ));
    }
    // FRAME (2nd arg) t → new-frame defaults; 0 → defaults AND the
    // spec layer (all frames); anything else → spec only.
    let to_t = matches!(&a[1], Value::Sym(s) if *s == sym::T);
    let zero = matches!(&a[1], Value::Int(0));
    for kv in a[2..].chunks(2) {
        if let (Some(k), Some(v)) = (i.sym_id(&kv[0]), kv.get(1)) {
            let attr = i.symbol_name(k);
            // GNU set-face-attribute (faces.el) only dispatches
            // :family/:foundry when the value is a string or
            // `unspecified' — other values are silently dropped.
            if attr == ":family" || attr == ":foundry" {
                let is_unspec = matches!(v, Value::Sym(s) if {
                    i.symbol_name(*s) == "unspecified"
                });
                if !matches!(v, Value::Str(_)) && !is_unspec {
                    continue;
                }
            }
            // GNU stores :box t as fixnum 1 and, when FRAME=t,
            // `unspecified' as :ignore-defface.
            let mut v = v.clone();
            if to_t {
                if let Value::Sym(s) = &v {
                    if i.symbol_name(*s) == "unspecified" {
                        v = Value::Sym(i.q(":ignore-defface"));
                    }
                }
            }
            if attr == ":box" && matches!(&v, Value::Sym(s) if *s == sym::T) {
                v = Value::Int(1);
            }
            check_face_value(i, &name, &attr, &v)?;
            if to_t || zero {
                if !i.face_t_table.iter().any(|(n, _)| *n == name) {
                    i.face_t_table.push((name.clone(), Value::Nil));
                }
                if let Some((_, plist)) = i.face_t_table.iter_mut().find(|(n, _)| *n == name) {
                    *plist = crate::lisp::eval::plist_put(plist, k, v.clone());
                }
            }
            if !to_t {
                set_face_attr(i, &name, &attr, v.clone());
            }
        }
    }
    Ok(Value::Nil)
}

/// GNU xfaces.c `check_face_attr' — per-attribute value validation
/// performed by `set-face-attribute' / `internal-set-lisp-face-attribute'
/// when the pair is stored.  `unspecified' and `undefined' are
/// accepted for every attribute.
fn check_face_value(i: &mut Interp, name: &SymId, attr: &str, v: &Value) -> Result<(), Flow> {
    if let Value::Sym(s) = v {
        let n = i.symbol_name(*s);
        // GNU xfaces.c treats `unspecified'/`reset'/`:ignore-defface'
        // as per-attr pseudo-values legal everywhere; `undefined' is
        // NOT among them (GNU signals Invalid face <attr> for it).
        if n == "unspecified" || n == "reset" || n == ":ignore-defface" {
            return Ok(());
        }
    }
    match attr {
        ":foreground" | ":background" | ":distant-foreground" | ":distant-background" => {
            // GNU: nil → `unspecified' via HANDLE_INVALID_NIL_VALUE
            // (warns in *Messages*); a non-string is wta `stringp';
            // an empty string is "Empty <attr> color value".
            if v.is_nil() {
                return Ok(());
            }
            if !matches!(v, Value::Str(_)) {
                return Err(i.wrong_type_mut("stringp", v));
            }
            if matches!(v, Value::Str(s) if s.borrow().is_empty()) {
                let label = match attr {
                    ":foreground" => "Empty foreground color value",
                    ":background" => "Empty background color value",
                    ":distant-foreground" => "Empty distant-foreground color value",
                    _ => "Empty distant-background color value",
                };
                return Err(face_value_err(i, label, v));
            }
        }
        ":family" | ":foundry" => {
            // Reached from internal-set-lisp-face-attribute (GNU
            // CHECK_STRING); `set-face-attribute' skips non-string
            // values itself before dispatching here.
            if !matches!(v, Value::Str(_)) {
                return Err(i.wrong_type_mut("stringp", v));
            }
            if matches!(v, Value::Str(s) if s.borrow().is_empty()) {
                let label = if attr == ":family" {
                    "Invalid face family"
                } else {
                    "Invalid face foundry"
                };
                return Err(face_value_err(i, label, v));
            }
        }
        ":font" => {
            if !matches!(v, Value::Str(_) | Value::Vec(_) | Value::Nil) {
                return Err(face_value_err(i, "Invalid font or font-spec", v));
            }
        }
        ":weight" | ":slant" | ":width" => {
            if !matches!(v, Value::Sym(_)) {
                return Err(i.wrong_type_mut("symbolp", v));
            }
            let n = i.symbol_name(i.sym_id(v).unwrap());
            let ok = match attr {
                ":weight" => matches!(
                    n.as_str(),
                    "ultra-light" | "extra-light" | "light" | "semi-light"
                        | "normal" | "regular" | "medium" | "semi-bold"
                        | "bold" | "extra-bold" | "ultra-bold"
                ),
                ":slant" => matches!(
                    n.as_str(),
                    "italic" | "oblique" | "normal" | "reverse-italic"
                        | "reverse-oblique"
                ),
                _ => matches!(
                    n.as_str(),
                    "condensed" | "semi-condensed" | "normal"
                        | "semi-expanded" | "expanded"
                ),
            };
            if !ok {
                let label = match attr {
                    ":weight" => "Invalid face weight",
                    ":slant" => "Invalid face slant",
                    _ => "Invalid face width",
                };
                return Err(face_value_err(i, label, v));
            }
        }
        ":height" => {
            // GNU check_face_attr: on `default' the height must be an
            // absolute positive number; on other faces the value is
            // test-merged against 10 — ints/floats >0, a symbol naming
            // a real face, or a function/list producing a positive
            // number pass (merge_face_heights).
            let is_default = builtin_face_name(i, *name) == Some("default");
            // GNU merge_face_heights against a base of 10: fixnum
            // passes through absolute; float scales; FUNCTIONP is
            // safe_calln'd (symbol w/o function → invalid → error);
            // nil and non-function conses produce `invalid' → error.
            let ok = match v {
                Value::Int(n) => *n > 0,
                Value::Float(f) => **f > 0.0,
                Value::Sym(s) => {
                    !is_default && {
                        let fv = i.obarray.symbol(*s).function.clone();
                        !matches!(&fv, Value::Sym(u) if *u == sym::UNBOUND)
                    }
                }
                Value::Lambda(_) | Value::Subr(_) => !is_default,
                Value::Cons(c) => {
                    let is_lambda = matches!(
                        &c.borrow().car,
                        Value::Sym(s) if {
                            let n = i.symbol_name(*s);
                            n == "lambda" || n == "closure"
                        }
                    );
                    !is_default && is_lambda
                }
                _ => false,
            };
            if !ok {
                let label = if is_default {
                    "Default face height not absolute and positive"
                } else {
                    "Face height does not produce a positive integer"
                };
                return Err(face_value_err(i, label, v));
            }
        }
        ":inverse-video" | ":extend" => {
            // GNU CHECK_SYMBOL first — non-symbols are wta `symbolp'
            // (nil IS a symbol and passes); only t/nil survive.
            if !matches!(v, Value::Sym(_) | Value::Nil) {
                return Err(i.wrong_type_mut("symbolp", v));
            }
            let ok = v.is_nil() || matches!(v, Value::Sym(s) if *s == sym::T);
            if !ok {
                let label = if attr == ":inverse-video" {
                    "Invalid inverse-video face attribute value"
                } else {
                    "Invalid extend face attribute value"
                };
                return Err(face_value_err(i, label, v));
            }
        }
        ":overline" | ":strike-through" => {
            // GNU: symbol must be t/nil; a non-symbol must be a
            // nonempty string (color).
            let ok = match v {
                Value::Nil => true,
                Value::Sym(s) => *s == sym::T,
                Value::Str(s) => !s.borrow().is_empty(),
                _ => false,
            };
            if !ok {
                let label = if attr == ":overline" {
                    "Invalid face overline"
                } else {
                    "Invalid face strike-through"
                };
                return Err(face_value_err(i, label, v));
            }
        }
        ":underline" => {
            // GNU: t/nil, a nonempty color string, or a plist where
            // :color is a nonempty string or `foreground-color',
            // :style is line/double-line/wave/dots/dashes, nil values
            // allowed only for :position; other keys' values pass.
            let ok = match v {
                Value::Nil => true,
                Value::Sym(s) => *s == sym::T,
                Value::Str(s) => !s.borrow().is_empty(),
                Value::Cons(_) => face_underline_plist_ok(i, v),
                _ => false,
            };
            if !ok {
                return Err(face_value_err(i, "Invalid face underline", v));
            }
        }
        ":stipple" => {
            if !matches!(v, Value::Nil | Value::Str(_) | Value::Vec(_) | Value::Cons(_)) {
                return Err(face_value_err(i, "Invalid stipple attribute", v));
            }
        }
        ":box" => {
            // GNU xfaces.c merge_face_vectors :box arm: t is legal
            // (stored as width 1), plus `unspecified', `reset' and
            // `:ignore-defface' — other symbols are NOT.  Nonzero
            // fixnums and nonempty strings pass; a cons must be an
            // (INT . INT) pair or a plist of :line-width (nonzero
            // int or (INT . INT) pair), :color (nonempty string),
            // :style (nil/pressed-button/released-button/flat-button).
            let ok = match v {
                Value::Nil => true,
                Value::Sym(s) => {
                    *s == sym::T || {
                        let n = i.symbol_name(*s);
                        n == ":ignore-defface"
                    }
                }
                Value::Int(n) => *n != 0,
                Value::Str(s) => !s.borrow().is_empty(),
                Value::Cons(c) => {
                    let (car, cdr) = {
                        let cc = c.borrow();
                        (cc.car.clone(), cc.cdr.clone())
                    };
                    if matches!(car, Value::Int(_)) && matches!(cdr, Value::Int(_)) {
                        true
                    } else {
                        face_box_plist_ok(i, v)
                    }
                }
                _ => false,
            };
            if !ok {
                return Err(face_value_err(i, "Invalid face box", v));
            }
        }
        ":inherit" => {
            let ok = matches!(v, Value::Nil | Value::Sym(_) | Value::Str(_) | Value::Cons(_));
            if !ok {
                return Err(face_value_err(i, "Invalid face inheritance", v));
            }
        }
        _ => {}
    }
    Ok(())
}

/// GNU signal_error: `(error . (MSG . ARG))' where a proper-list ARG
/// splices into the data tail and any other value is wrapped in a
/// one-element list.
fn face_value_err(i: &mut Interp, label: &str, v: &Value) -> Flow {
    let tail = match v.list_to_vec() {
        Ok(items) => Value::list(items),
        Err(_) => Value::list(vec![v.clone()]),
    };
    i.signal_data(
        sym::ERROR,
        Value::cons(Value::string(label), tail).list_to_vec().unwrap_or_default(),
    )
}



/// GNU xfaces.c :box plist arm: pairwise `(:key val)' scan where
/// `:line-width' takes a nonzero fixnum or an (INT . INT) pair,
/// `:color' a nonempty string, `:style' nil or one of
/// pressed-button/released-button/flat-button — anything else
/// (including an odd/dotted tail) invalidates the whole value.
fn face_box_plist_ok(i: &mut Interp, v: &Value) -> bool {
    let mut rest = v.clone();
    loop {
        let (k, tail) = match &rest {
            Value::Cons(c) => {
                let cc = c.borrow();
                (cc.car.clone(), cc.cdr.clone())
            }
            _ => return rest.is_nil(),
        };
        let (val, next) = match &tail {
            Value::Cons(c) => {
                let cc = c.borrow();
                (cc.car.clone(), cc.cdr.clone())
            }
            _ => return false,
        };
        let key = i.sym_id(&k).map(|s| i.symbol_name(s));
        let good = match key.as_deref() {
            Some(":line-width") => match &val {
                Value::Int(n) => *n != 0,
                Value::Cons(p) => {
                    let pp = p.borrow();
                    matches!(pp.car, Value::Int(a) if a != 0)
                        && matches!(pp.cdr, Value::Int(b) if b != 0)
                }
                _ => false,
            },
            Some(":color") => match &val {
                Value::Nil => false,
                Value::Str(s) => !s.borrow().is_empty(),
                _ => false,
            },
            Some(":style") => match &val {
                Value::Nil => true,
                Value::Sym(s) => {
                    let n = i.symbol_name(*s);
                    matches!(n.as_str(), "pressed-button" | "released-button" | "flat-button")
                }
                _ => false,
            },
            _ => false,
        };
        if !good {
            return false;
        }
        rest = next;
    }
}

/// GNU xfaces.c :underline plist arm: every key must be a symbol,
/// nil values pass only for :position; :color must be a nonempty
/// string or `foreground-color'; :style must be one of
/// line/double-line/wave/dots/dashes.  Other keys pass unchecked.
fn face_underline_plist_ok(i: &mut Interp, v: &Value) -> bool {
    let mut rest = v.clone();
    loop {
        let (k, tail) = match &rest {
            Value::Cons(c) => {
                let cc = c.borrow();
                (cc.car.clone(), cc.cdr.clone())
            }
            _ => return rest.is_nil(),
        };
        let (val, next) = match &tail {
            Value::Cons(c) => {
                let cc = c.borrow();
                (cc.car.clone(), cc.cdr.clone())
            }
            _ => return true, // odd tail: CAR_SAFE nil val is OK for :position only; GNU treats missing val as nil
        };
        let key = i.sym_id(&k).map(|s| i.symbol_name(s));
        let good = match (key.as_deref(), &val) {
            (None, _) => false,
            (Some(":color"), Value::Sym(s)) => i.symbol_name(*s) == "foreground-color",
            (Some(":color"), Value::Str(s)) => !s.borrow().is_empty(),
            (Some(":color"), _) => false,
            (Some(":style"), Value::Sym(s)) => matches!(
                i.symbol_name(*s).as_str(),
                "line" | "double-line" | "wave" | "dots" | "dashes"
            ),
            (Some(":style"), _) => false,
            (Some(_), Value::Nil) => key.as_deref() == Some(":position"),
            _ => true,
        };
        if !good {
            return false;
        }
        rest = next;
    }
}



/// Route through the Lisp `face-attribute' (faces.el layering) so the
/// :inherit chain and the INHERIT arg merge like GNU's
/// `internal-face-attribute' → `merge_face_ref'.  `unspecified' → nil.
fn face_color_attr(i: &mut Interp, a: &[Value], attr: &str) -> EvalResult {
    // No face check here: GNU's face-attribute validates FRAME before
    // the face name, so `face-foreground'/`face-background' on a bad
    // frame must signal `frame-live-p' rather than `Invalid face'.
    let fa = i.q("face-attribute");
    let attr_sym = Value::Sym(i.intern(attr));
    let v = i.apply(
        &Value::Sym(fa),
        vec![
            a[0].clone(),
            attr_sym,
            a.get(1).cloned().unwrap_or(Value::Nil),
            a.get(2).cloned().unwrap_or(Value::Nil),
        ],
    )?;
    match v {
        Value::Sym(s) if i.symbol_name(s) == "unspecified" => Ok(Value::Nil),
        v => Ok(v),
    }
}

fn f_face_background(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    face_color_attr(i, &a, ":background")
}
fn f_face_foreground(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    face_color_attr(i, &a, ":foreground")
}

/// `set-face-background` — GNU Fset_face_attribute_path: CHECK_SYMBOL
/// on FACE before touching attributes.
fn f_set_face_background(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    if i.sym_id(&a[0]).is_none() {
        return Err(i.wrong_type_mut("symbolp", &a[0]));
    }
    // GNU Fset_face_attribute_path decodes FRAME (arg 3) before the
    // face-name check, so a bad frame signals `frame-live-p' even when
    // FACE is unknown.
    face_frame_live(i, &arg(&a, 2))?;
    let name = face_name_of(i, &a[0])?;
    set_face_attr(i, &name, ":background", a[1].clone());
    Ok(Value::Nil)
}

/// `face-extend-p` — t when FACE's :extend attribute is non-nil.
fn f_face_extend_p(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let _ = face_frame_live(i, &arg(&a, 1))?;
    let name = face_name_of(i, &a[0])?;
    match face_attr(i, &name, ":extend", false) {
        Value::Nil => Ok(Value::Nil),
        Value::Sym(s) if i.symbol_name(s) == "unspecified" => Ok(Value::Nil),
        _ => Ok(Value::t()),
    }
}

/// `set-face-extend` — set FACE's :extend attribute; nil.  GNU's
/// faces.el wrapper reaches `internal-set-lisp-face-attribute', whose
/// order is symbol → frame → face.
fn f_set_face_extend(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    if i.sym_id(&a[0]).is_none() {
        return Err(i.wrong_type_mut("symbolp", &a[0]));
    }
    face_frame_live(i, &arg(&a, 2))?;
    let name = face_name_of(i, &a[0])?;
    set_face_attr(i, &name, ":extend", a[1].clone());
    Ok(Value::Nil)
}

/// (memq WEIGHT '(semi-bold bold extra-bold ultra-bold)) like GNU.
fn f_face_bold_p(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU decodes FRAME (arg2) before FACE: non-live → frame-live-p.
    let _ = face_frame_live(i, &arg(&a, 1))?;
    let name = face_name_of(i, &a[0])?;
    let w = face_attr(i, &name, ":weight", false);
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
fn f_face_italic_p(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let _ = face_frame_live(i, &arg(&a, 1))?;
    let name = face_name_of(i, &a[0])?;
    let w = face_attr(i, &name, ":slant", false);
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
fn f_face_underline_p(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let _ = face_frame_live(i, &arg(&a, 1))?;
    let name = face_name_of(i, &a[0])?;
    Ok(match face_attr(i, &name, ":underline", false) {
        Value::Sym(s) if i.symbol_name(s) == "unspecified" => Value::Nil,
        v => v,
    })
}

// `face-all-attributes' is Lisp (prelude) on top of `face-attribute',
// per GNU faces.el — a Rust subr would bypass the :inherit merge.

// ---------- window resizing & layout ----------

const WINDOW_MIN_HEIGHT: usize = 4;
const WINDOW_MIN_WIDTH: usize = 10;

/// Refresh a normal dimension after an explicit geometry change.
pub(crate) fn update_window_normal_size(window: &WindowRef, horizontal: bool) {
    let normal = w_parent(window).map_or(1.0, |parent| {
        let parent = parent.borrow();
        let window = window.borrow();
        if parent.horiz != Some(horizontal) {
            1.0
        } else {
            let (size, total) = if horizontal {
                (window.width, parent.width)
            } else {
                (window.height, parent.height)
            };
            size as f64 / total.max(1) as f64
        }
    });
    let mut window = window.borrow_mut();
    if horizontal {
        window.normal_size.1 = normal;
    } else {
        window.normal_size.0 = normal;
    }
}

/// Adjust WINDOW's height (or width when HORIZ) by DELTA cells,
/// taking/giving space from another window in the frame.
fn window_resize(i: &mut Interp, w: &WindowRef, delta: i128, horiz: bool) -> Result<(), Flow> {
    let old_size = if horiz {
        w.borrow().width
    } else {
        w.borrow().height
    };
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
    if delta != 0 && others.is_empty() {
        return Err(i.error("Cannot resize the root window of a frame"));
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
    let new_size = if horiz {
        w.borrow().width
    } else {
        w.borrow().height
    };
    if old_size != new_size {
        update_window_normal_size(w, horiz);
        if let Some(other) = others.first() {
            update_window_normal_size(other, horiz);
        }
    }
    Ok(())
}

fn f_enlarge_window(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let w = sel_window(i).ok_or_else(|| i.error("No window"))?;
    let delta = want_num_or_mark(i, &a[0])?;
    let horiz = a.get(1).map(|v| v.truthy()).unwrap_or(false);
    window_resize(i, &w, delta, horiz)?;
    Ok(Value::t())
}
fn f_shrink_window(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let w = sel_window(i).ok_or_else(|| i.error("No window"))?;
    let delta = want_num_or_mark(i, &a[0])?;
    let horiz = a.get(1).map(|v| v.truthy()).unwrap_or(false);
    window_resize(i, &w, -delta, horiz)?;
    Ok(Value::t())
}
fn f_enlarge_window_horizontally(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    f_enlarge_window(i, vec![a[0].clone(), Value::t()].into())
}
fn f_shrink_window_horizontally(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    f_shrink_window(i, vec![a[0].clone(), Value::t()].into())
}
fn f_adjust_window_trailing_edge(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let w = win_err_valid(i, &a[0])?;
    // GNU CHECK_NUMBER: DELTA accepts floats — the predicate is
    // number-or-marker-p, not integer-or-marker-p.
    let delta = crate::lisp::builtins::want_num_or_mark(i, &a[1])?;
    let horiz = a.get(2).map(|v| v.truthy()).unwrap_or(false);
    // GNU adjust_window_trailing_edge (window.c): with no sibling
    // window on the trailing edge it user_errors BEFORE any resize —
    // "No window on the right of this one" (horiz) / "No window below
    // this one" (vertical).
    {
        let f = sel_frame(i).ok_or_else(|| i.error("No frame"))?;
        let wid = w.borrow().id;
        let has_sibling = f
            .borrow()
            .windows
            .iter()
            .any(|o| o.borrow().id != wid && !o.borrow().dead && !o.borrow().minibuffer);
        if !has_sibling {
            let ue = i.q("user-error");
            let msg = if horiz {
                "No window on the right of this one"
            } else {
                "No window below this one"
            };
            return Err(i.signal(ue, Value::cons(Value::string(msg), Value::Nil)));
        }
    }
    window_resize(i, &w, delta, horiz)?;
    Ok(Value::Nil)
}

fn f_balance_windows(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU
    // Fbalance_windows decodes WINDOW with decode_valid_window —
    // `(wrong-type-argument window-valid-p X)' on non-windows.
    let w = win_valid(i, &arg(&a, 0))?;
    let f = frame_of_window(i, &w)
        .or_else(|| sel_frame(i))
        .ok_or_else(|| i.error("No frame"))?;
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
            update_window_normal_size(w, false);
        }
    }
    Ok(Value::t())
}

fn f_maximize_window(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let w = win_err_valid(i, &arg(&a, 0))?;
    let f = sel_frame(i).ok_or_else(|| i.error("No frame"))?;
    let fh = f.borrow().height;
    let delta = fh as i128 - w.borrow().height as i128 - 1;
    if delta <= 0 {
        return Ok(Value::t());
    }
    window_resize(i, &w, delta, false)?;
    Ok(Value::t())
}
fn f_minimize_window(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU window.el
    // `minimize-window' → `window-resize': `window-normalize-window'
    // (plain \"N is not a valid window\") then the root window can't
    // shrink — `(error \"Cannot resize the root window of a frame\")'.
    let w = win_err_valid(i, &arg(&a, 0))?;
    let f = frame_of_window(i, &w);
    let is_root = f
        .and_then(|fr| fr.borrow().root.clone())
        .map(|r| r.borrow().id == w.borrow().id)
        .unwrap_or(false);
    if is_root {
        return Err(i.error("Cannot resize the root window of a frame"));
    }
    if w.borrow().minibuffer {
        return Err(i.error("Cannot resize minibuffer window horizontally"));
    }
    let cur = w.borrow().height;
    if cur <= WINDOW_MIN_HEIGHT {
        return Ok(Value::t());
    }
    window_resize(i, &w, WINDOW_MIN_HEIGHT as i128 - cur as i128, false)?;
    Ok(Value::t())
}

fn f_split_window_vertically(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU `split-window-below':
    // `(and size (< size 0))' fires `number-or-marker-p' on a
    // non-number SIZE before `split-window' decodes the window.
    if let Some(sz) = a.first() {
        if !sz.is_nil() && sz.int().is_none() {
            return Err(i.wrong_type_mut("number-or-marker-p", sz));
        }
    }
    f_split_window(i, vec![arg(&a, 1), arg(&a, 0)].into())
}
fn f_split_window_horizontally(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU `split-window-right':
    // same `(and size (< size 0))' gate — `number-or-marker-p' first.
    if let Some(sz) = a.first() {
        if !sz.is_nil() && sz.int().is_none() {
            return Err(i.wrong_type_mut("number-or-marker-p", sz));
        }
    }
    // GNU: (split-window-horizontally &optional SIZE WINDOW-TO-SPLIT)
    // puts a new window of SIZE columns on the right of
    // WINDOW-TO-SPLIT — i.e. `split-window' with side `right'.
    let right = Value::Sym(i.q("right"));
    f_split_window(i, vec![arg(&a, 1), arg(&a, 0), right].into())
}

fn f_switch_to_buffer_other_window(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU window.el: nil BUFFER-OR-NAME switches to `other-buffer'.
    let target = if a[0].is_nil() {
        match crate::buffer::primitives::other_buffer_visible(i, usize::MAX) {
            Some(id) => i.buffer_value(id).unwrap_or(Value::Nil),
            None => Value::Nil,
        }
    } else {
        a[0].clone()
    };
    let bid = match i.buffer_id_of(&target) {
        Some(id) => id,
        None => {
            // GNU `get-buffer-create' path: non-string names signal stringp.
            let name = match &target {
                Value::Str(s) => s.borrow().clone(),
                other => return Err(i.wrong_type_mut("stringp", other)),
            };
            i.create_buffer(&name)
        }
    };
    // GNU decode_live_buffer: a dead buffer object signals
    // "Invalid buffer" before any window work.
    if !i.buffer_live(bid) {
        return Err(i.error("Invalid buffer"));
    }
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
        match f_split_window(i, vec![Value::Nil, Value::Nil].into())? {
            Value::Window(w) => w,
            _ => sel.clone(),
        }
    };
    let buffer = i.buffer_value(bid).unwrap_or(Value::Nil);
    f_set_window_buffer(
        i,
        vec![Value::Window(target.clone()), buffer.clone()].into(),
    )?;
    f_select_window(i, vec![Value::Window(target), arg(&a, 1)].into())?;
    Ok(buffer)
}

fn f_switch_to_buffer_other_frame(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU validates the buffer arg (stringp on non-string non-buffer) before
    // the different-frame refusal.
    match &arg(&a, 0) {
        Value::Nil | Value::Str(_) | Value::Buffer(_) => {}
        other => return Err(i.wrong_type_mut("stringp", other)),
    }
    Err(i.error("Unknown terminal type"))
}
fn f_display_buffer_other_frame(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU: display-buffer requires an existing buffer — `nil' and
    // non-string values fail `stringp', a name with no live buffer
    // signals `Invalid buffer', and a real buffer reaches make-frame's
    // batch-terminal error `Unknown terminal type'.
    match &arg(&a, 0) {
        Value::Str(_) | Value::Buffer(_) => {}
        other => return Err(i.wrong_type_mut("stringp", other)),
    }
    let exists = match &arg(&a, 0) {
        Value::Str(s) => i.buffers.by_name(&s.borrow().clone()).is_some(),
        Value::Buffer(_) => true,
        _ => false,
    };
    if exists {
        Err(i.error("Unknown terminal type"))
    } else {
        Err(i.error("Invalid buffer"))
    }
}

fn f_quit_window(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let w = win_err_valid(i, &arg(&a, 1))?;
    let kill = arg(&a, 0).truthy();
    if kill {
        // GNU quit_window: errors from kill-buffer propagate — a vetoed
        // kill signals instead of silently keeping the window's buffer.
        let bid = w.borrow().buffer;
        crate::buffer::primitives::kill_buffer_keep_current(i, bid)?;
        // GNU kill_buffer → replace_buffer_in_windows also syncs the
        // selected window's *new* buffer into current-buffer even when
        // the killed buffer wasn't current (the temp-buffer case in
        // `exit-splash-screen': cur=temp, window showed scratch —
        // scratch killed, window picks *scratch*<2>/next, cur follows).
        let f0 = sel_frame(i);
        if let Some(f0) = f0 {
            if f0.borrow().selected.borrow().id == w.borrow().id {
                let now = w.borrow().buffer;
                if i.buffer_live(now) {
                    i.set_current_buffer(now);
                }
            }
        }
    }
    // The replacement policy may already have deleted a dedicated window.
    if w.borrow().dead { return Ok(Value::from_bool(kill)); }
    let f = sel_frame(i).ok_or_else(|| i.error("No frame"))?;
    // GNU counts only *ordinary* windows: the minibuffer window never
    // makes this "one of many", so a lone text window quits in place.
    let live = f
        .borrow()
        .windows
        .iter()
        .filter(|x| !x.borrow().dead && !x.borrow().minibuffer)
        .count();
    if live > 1 {
        f_delete_window(i, vec![a.get(1).cloned().unwrap_or(Value::Nil)].into())?;
    } else {
        // Sole window: GNU `switch_to_prev_or_next_buffer' — but ONLY on
        // the bury path.  When KILL ran, kill_buffer's
        // replace_buffer_in_windows already installed each window's own
        // prev-buffer/other-buffer pick and synced current_buffer.
        if !kill {
            let cur = w.borrow().buffer;
            let mut pick = crate::buffer::primitives::window_prev_live(i, &w, cur);
            if pick.is_none() {
                pick = crate::buffer::primitives::other_buffer_visible(i, cur);
            }
            if let Some(other) = pick {
                w.borrow_mut().buffer = other;
                let point = i.buffers.get(other).map_or(0, |buffer| buffer.borrow().point());
                set_window_old_point(i, &mut w.borrow_mut(), point);
                record_buffer_display(i, other);
                // GNU `set-window-buffer-start-and-point': sync current
                // only when the selected window was showing the current
                // buffer (`and selected current').  Blindly selecting on
                // selection alone stole `current-buffer' from a window
                // showing a different buffer (qa_window /
                // quit-window prev-buffer divergence).
                if f.borrow().selected.borrow().id == w.borrow().id && i.current_buffer == cur {
                    i.set_current_buffer(other);
                }
            }
        }
    }
    // GNU's return value: t when the buffer was killed, nil when the
    // window merely switched buffers or was deleted.
    Ok(Value::from_bool(kill))
}

fn f_quit_restore_window(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // (quit-restore-window &optional WINDOW BURY-OR-KILL)
    let w = win_err_live(i, &arg(&a, 0))?;
    let bok = arg(&a, 1);
    let bok_name = i.sym_id(&bok).map(|s| i.symbol_name(s)).unwrap_or_default();
    // `killing' means kill-buffer will perform teardown after this call.
    let kill = bok.truthy() && !matches!(bok_name.as_str(), "bury" | "append" | "killing");
    f_quit_window(i, vec![Value::from_bool(kill), Value::Window(w)].into())?;
    // GNU Fquit_restore_window (window.c) returns Qnil unconditionally —
    // unlike `quit-window', the kill/bury outcome isn't the return value.
    Ok(Value::Nil)
}

fn f_kill_buffer_and_window(i: &mut Interp, _a: crate::lisp::Argv) -> EvalResult {
    let f = sel_frame(i).ok_or_else(|| i.error("No frame"))?;
    let live = f
        .borrow()
        .windows
        .iter()
        .filter(|x| !x.borrow().dead && !x.borrow().minibuffer)
        .count();
    if live <= 1 {
        return Err(i.error("Attempt to delete minibuffer or sole ordinary window"));
    }
    let original_window = f.borrow().selected.clone();
    let bid = i.current_buffer;
    crate::buffer::primitives::kill_buffer_keep_current(i, bid)?;
    if !original_window.borrow().dead {
        f_delete_window(i, vec![Value::Window(original_window)].into())?;
    }
    let next = crate::buffer::primitives::other_buffer_visible(i, bid)
        .or_else(|| i.buffers.list().first().copied())
        .unwrap_or_else(|| i.create_buffer("*scratch*"));
    i.set_current_buffer(next);
    Ok(Value::Nil)
}

/// `window-tree' node: (VERTICAL-P EDGES CHILD...) for internal
/// windows — VERTICAL-P t means children are stacked — or the window
/// object itself for leaves.
fn window_tree_node(w: &WindowRef) -> Value {
    let (horiz, l, t, wd, h, kids) = {
        let wb = w.borrow();
        (
            wb.horiz,
            wb.left,
            wb.top,
            wb.width,
            wb.height,
            wb.children.clone(),
        )
    };
    match horiz {
        Some(hz) => {
            // GNU window-tree: VERTICAL-P is t for a `vc' (stacked)
            // parent — our `horiz' flag is the inverse (t = hc).
            let mut node = vec![
                Value::from_bool(!hz),
                Value::list(vec![
                    Value::Int(l as i128),
                    Value::Int(t as i128),
                    Value::Int((l + wd) as i128),
                    Value::Int((t + h) as i128),
                ]),
            ];
            for k in kids {
                node.push(window_tree_node(&k));
            }
            Value::list(node)
        }
        None => Value::Window(w.clone()),
    }
}

fn f_window_tree(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU: non-live-frame -> `(error "X is not a live frame")'.
    let f = match arg(&a, 0) {
        Value::Frame(f) if !f.borrow().dead => f.clone(),
        Value::Nil => sel_frame(i).ok_or_else(|| i.error("No frame"))?,
        other => return Err(i.error(format!("{} is not a live frame", i.princ_to_string(&other)))),
    };
    let ff = f.borrow();
    let mut top: Vec<Value> = Vec::new();
    if let Some(r) = &ff.root {
        top.push(window_tree_node(r));
    }
    if let Some(mb) = &ff.minibuffer {
        top.push(Value::Window(mb.clone()));
    }
    Ok(Value::list(top))
}

fn f_window_combination_limit(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU: valid only on internal windows; leaves → plain error.
    let w = match arg(&a, 0) {
        Value::Nil => sel_window(i).ok_or_else(|| i.error("No window"))?,
        Value::Window(w) => w,
        other => return Err(i.wrong_type_mut("window-valid-p", &other)),
    };
    let wb = w.borrow();
    if wb.horiz.is_some() && !wb.dead {
        Ok(Value::from_bool(wb.combination_limit))
    } else {
        Err(i.error("Combination limit is meaningful for internal windows only"))
    }
}

fn f_scroll_other_window_down(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU window.el
    // `scroll-other-window-down': scroll-down-command on the
    // other-window-for-scrolling window — see scroll_other_command.
    scroll_other_command(i, &arg(&a, 0), "scroll-down-command")
}

// ---------- overlays (missing pieces) ----------

fn f_copy_overlay(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let (bid, _) = overlay_of(i, &a[0])?;
    let mut ov = match overlay_slot(i, &a[0]) {
        Some(o) => o,
        None => return Err(i.wrong_type_mut("overlayp", &a[0])),
    };
    ov.plist = crate::lisp::builtins::seq::f_copy_sequence(i, vec![ov.plist.clone()].into())?;
    let owner = if crate::buffer::Overlay::detached_state(&a[0]).is_some() {
        i.current_buffer
    } else {
        bid
    };
    let b = i.buffers.get(owner).unwrap();
    let mut bb = b.borrow_mut();
    let new_idx = bb.overlays.len();
    let handle = Value::vector(vec![
        Value::Sym(i.q("overlay")),
        Value::Int(owner as i128),
        Value::Int(new_idx as i128),
    ]);
    i.register_overlay(&handle);
    bb.overlays.push(crate::buffer::Overlay {
        handle: handle.clone(),
        ..ov
    });
    Ok(handle)
}

fn f_overlay_lists(i: &mut Interp, _a: crate::lisp::Argv) -> EvalResult {
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

fn f_overlay_recenter(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU still validates POS as integer-or-marker even though the
    // recenter itself is a no-op for us.
    match &a[0] {
        Value::Int(_) | Value::Marker(_) => Ok(Value::Nil),
        other => Err(i.wrong_type_mut("integer-or-marker-p", other)),
    }
}

// ---------- readers & redisplay ----------

fn f_completing_read_default(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU `completing-read-default' (minibuffer.el): pick the base
    // keymap, compose the filename map on top when completing file
    // names, specbind the completion state, read via the minibuffer,
    let prompt = match &a[0] {
        Value::Str(s) => s.borrow().clone(),
        // GNU read_minibuf_noninteractive CHECK_STRINGs the prompt before
        // touching stdin — order precedes the reader guard.
        other => return Err(i.wrong_type_mut("stringp", other)),
    };
    // Batch must take the stdin line path too: `minibuf_reader' can be
    // installed while noninteractive (direct calls to this subr bypass
    // f_completing_read's `noninteractive' gate).
    // GNU read_minibuf checks INITIAL-INPUT is stringp-or-cons before
    // any stdin/minibuffer read; the batch reader narrows it to stringp.
    let init = arg(&a, 4);
    if i.minibuf_reader.is_none() || i.noninteractive {
        // Batch: GNU `read_minibuf_noninteractive' — read a raw stdin line;
        // empty input yields DEF.  INITIAL-INPUT (index 4) is stringp-
        // checked before stdin is touched.
        if !matches!(init, Value::Str(_) | Value::Nil) {
            return Err(i.wrong_type_mut("stringp", &init));
        }
        let input = i.batch_read_line(&prompt)?;

        return Ok(completing_read_result(input, arg(&a, 6)));
    }
    if !matches!(init, Value::Str(_) | Value::Nil | Value::Cons(_)) {
        return Err(i.wrong_type_mut("stringp", &init));
    }
    let mark = i.specbind_depth();
    let rm = arg(&a, 3);
    // GNU: base-keymap = must-match ? minibuffer-local-must-match-map
    // : minibuffer-local-completion-map; while completing a file name
    // (`minibuffer-completing-file-name' non-nil except `lambda' in a
    // nested read) layer minibuffer-local-filename-completion-map on
    // top so its nil SPC binding un-binds minibuffer-complete-word.
    let base_name = if rm.truthy() {
        "minibuffer-local-must-match-map"
    } else {
        "minibuffer-local-completion-map"
    };
    let mut keymap = i
        .intern_soft(base_name)
        .map(|id| i.symbol_value(id))
        .unwrap_or(Value::Nil);
    let cf = i.q("minibuffer-completing-file-name");
    let cfv = i.symbol_value(cf);
    let completing_file =
        cfv.truthy() && !matches!(&cfv, Value::Sym(s) if i.symbol_name(*s) == "lambda");
    if completing_file {
        let fm = i
            .intern_soft("minibuffer-local-filename-completion-map")
            .map(|id| i.symbol_value(id))
            .unwrap_or(Value::Nil);
        if is_keymap(i, &fm) {
            let mck = i.q("make-composed-keymap");
            if i.fbound_p(mck) {
                keymap = match i.apply(&Value::Sym(mck), vec![fm, keymap.clone()]) {
                    Ok(v) => v,
                    Err(_) => keymap,
                };
            }
        }
    }
    // GNU completing-read-default setq-locals in the minibuffer:
    // minibuffer-completion-confirm gets nil when require-match is t;
    // minibuffer--require-match records the raw arg.
    let mct = i.q("minibuffer-completion-table");
    let _ = i.specbind(mct, arg(&a, 1));
    let mcp = i.q("minibuffer-completion-predicate");
    let _ = i.specbind(mcp, arg(&a, 2));
    let mcc_val = match rm {
        Value::Sym(s) if i.symbol_name(s) == "t" => Value::Nil,
        _ => rm.clone(),
    };
    let mcc = i.q("minibuffer-completion-confirm");
    let _ = i.specbind(mcc, mcc_val);
    let mrm = i.q("minibuffer--require-match");
    let _ = i.specbind(mrm, rm);
    let mob = i.q("minibuffer--original-buffer");
    let orig = i.buffer_value(i.current_buffer).unwrap_or(Value::Nil);
    let _ = i.specbind(mob, orig);
    let args = crate::lisp::eval::MinibufArgs {
        hist: arg(&a, 5),
        defalt: arg(&a, 6),
        initial: arg(&a, 4),
        keymap,
    };
    let input_r = i.minibuf_read(&prompt, args);
    let _ = i.unbind_to(mark);
    let input = input_r?;
    Ok(completing_read_result(Value::string(input), arg(&a, 6)))
}

fn completing_read_result(input: Value, default: Value) -> Value {
    // minibuffer.el preserves the empty input when DEF is nil. A non-nil
    // list selects its car, even if that element itself is nil.
    if input.is_empty_string() && default.truthy() {
        match default {
            Value::Cons(cell) => cell.borrow().car.clone(),
            value => value,
        }
    } else {
        input
    }
}

fn f_read_coding_system(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let args = crate::lisp::eval::MinibufArgs {
        hist: Value::Sym(i.q("coding-system-history")),
        ..Default::default()
    };
    let input = match minibuf_or(i, &arg(&a, 0), args)? {
        Some(s) => s,
        None => Value::string(""),
    };
    let def = match a.get(1) {
        Some(Value::Sym(s)) => i.symbol_name(*s),
        _ => "undecided".to_string(),
    };
    if input.is_empty_string() { Ok(Value::Sym(i.intern(&def))) }
    else { crate::lisp::builtins::data::f_intern(i, vec![input].into()) }
}

fn f_read_color(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let args = crate::lisp::eval::MinibufArgs {
        hist: Value::Sym(i.q("color-history")),
        ..Default::default()
    };
    let input = match minibuf_or(i, &arg(&a, 0), args)? {
        Some(s) => s,
        None => Value::string(""),
    };
    if input.is_empty_string() && !a.get(2).map(|v| v.truthy()).unwrap_or(false) {
        return Ok(Value::string(""));
    }
    Ok(input)
}

fn f_read_passwd(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let prompt = match &a[0] {
        Value::Str(s) => s.borrow().clone(),
        _ => String::new().into(),
    };
    if i.minibuf_reader.is_none() && i.noninteractive {
        // GNU (auth-source.el) binds `read-hide-char' to ?* around the
        // minibuffer read; batch echoes one mask char per input char.
        let hc = i.q("read-hide-char");
        let depth = i.specbind_depth();
        i.specbind(hc, Value::Int('*' as i128))?;
        let line = i.batch_read_line(&prompt);
        i.unbind(i.specbind_depth() - depth)?;
        let line = line?;
        // Empty input yields DEFAULT (3rd arg).
        if line.is_empty_string() && !arg(&a, 2).is_nil() {
            return Ok(arg(&a, 2));
        }
        return Ok(line);
    }
    let input = if i.minibuf_reader.is_some() {
        i.minibuf_line(&prompt)?
    } else {
        return Err(batch_eof(i));
    };
    if input.is_empty() && !arg(&a, 2).is_nil() {
        return Ok(arg(&a, 2));
    }
    Ok(Value::string(input))
}

fn f_read_kbd_macro(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU's read-kbd-macro always returns a vector (unlike `kbd').
    let s = want_str(i, &a[0])?;
    let keys = parse_kbd(i, &(s).to_unicode_lossy())?;
    Ok(Value::vector(keys))
}

fn f_momentary_string_display(_i: &mut Interp, _a: crate::lisp::Argv) -> EvalResult {
    Ok(Value::Nil)
}

fn f_minibuffer_completion_help(_i: &mut Interp, _a: crate::lisp::Argv) -> EvalResult {
    Ok(Value::Nil)
}

fn f_display_message_or_buffer(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // Small messages go to the echo area; multi-line would pop to a
    // buffer — we always echo and return the message.
    // GNU Fdisplay_message_or_buffer validates MESSAGE via
    // CHECK_STRING_OR_NUMBER first (char-or-string-p).
    // GNU Fdisplay_message_or_buffer: string/number via
    // CHECK_STRING_OR_NUMBER, or a buffer to display (batch = no-op,
    // returns the buffer like shell-command-on-region relies on).
    match &a[0] {
        Value::Str(s) => {
            // Batch output does not occupy the interactive echo area.
            if !i.noninteractive {
                i.echo_message = s.borrow().clone();
            }
            Ok(a[0].clone())
        }
        Value::Int(_) => Ok(a[0].clone()),
        // A buffer arg means "pop to / display it": GNU returns t.
        Value::Buffer(_) => Ok(Value::t()),
        other => Err(i.wrong_type_mut("char-or-string-p", other)),
    }
}

fn f_redisplay(_i: &mut Interp, _a: crate::lisp::Argv) -> EvalResult {
    Ok(Value::t())
}

fn f_force_mode_line_update(_i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU:
    // nil/absent arg resolves to the current buffer internally but the
    // RETURN value is nil; explicit args echo back.
    Ok(match a.first() {
        None | Some(Value::Nil) => Value::Nil,
        Some(v) => v.clone(),
    })
}
fn f_tooltip_show(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // tty: no tooltip — GNU returns the text.
    let _ = i;
    Ok(a[0].clone())
}
fn f_tooltip_hide(_i: &mut Interp, _a: crate::lisp::Argv) -> EvalResult {
    Ok(Value::Nil)
}

fn f_timer_event_handler(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    Err(i.wrong_type_mut("timerp", &a[0]))
}

fn f_invisible_p(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU Finvisible_p treats only nonnegative fixnums and markers as
    // positions. Negative integers and bignums are property values.
    let prop = if matches!(&a[0], Value::Int(n) if (0..=crate::lisp::value::FIXNUM_MAX).contains(n))
        || matches!(&a[0], Value::Marker(_))
    {
        let inv = i.q("invisible");
        crate::buffer::primitives::f_get_char_property(
            i,
            vec![a[0].clone(), Value::Sym(inv)].into(),
        )?
    } else {
        a[0].clone()
    };
    let spec_id = i.q("buffer-invisibility-spec");
    let spec = i.symbol_value(spec_id);
    if matches!(&spec, Value::Sym(s) if *s == sym::T) {
        return Ok(Value::from_bool(!prop.is_nil()));
    }

    // GNU invisible_prop first matches the whole property by identity,
    // then each of its list elements. The first spec match decides
    // whether text is entirely hidden (t) or replaced by an ellipsis (2).
    // Dotted tails of either list are ignored, as in GNU's CONSP walks.
    fn in_spec(prop: &Value, spec: &Value) -> Option<Value> {
        let mut tail = spec.clone();
        while let Value::Cons(cell) = tail {
            let cell = cell.borrow();
            if eq_values(prop, &cell.car) {
                return Some(Value::t());
            }
            if let Value::Cons(entry) = &cell.car {
                let entry = entry.borrow();
                if eq_values(prop, &entry.car) {
                    return Some(if entry.cdr.is_nil() {
                        Value::t()
                    } else {
                        Value::Int(2)
                    });
                }
            }
            tail = cell.cdr.clone();
        }
        None
    }
    if let Some(invisible) = in_spec(&prop, &spec) {
        return Ok(invisible);
    }
    let mut tail = prop;
    while let Value::Cons(cell) = tail {
        let cell = cell.borrow();
        if let Some(invisible) = in_spec(&cell.car, &spec) {
            return Ok(invisible);
        }
        tail = cell.cdr.clone();
    }
    Ok(Value::Nil)
}

fn f_fringe_bitmaps_at_pos(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU fringe.el:
    // `(window-normalize-window window)' decodes arg1 FIRST — a
    // non-window is `windowp' wta — then `(or pos (point))' needs an
    // integer/marker; batch tty → nil.
    match arg(&a, 1) {
        Value::Nil | Value::Window(_) => {}
        ref other => return Err(i.wrong_type_mut("windowp", other)),
    }
    let pos = arg(&a, 0);
    if !matches!(pos, Value::Nil) {
        crate::lisp::builtins::want_int_or_mark(i, &pos)?;
    }
    Ok(Value::Nil)
}

// ---------- modes & jit/font-lock ----------

fn mode_toggle(i: &mut Interp, a: &[Value], var: &str) -> EvalResult {
    let sym = i.intern(var);
    let cur = i.symbol_value(sym).truthy();
    // GNU define-minor-mode setter:
    //   (cond ((eq ARG 'toggle) (not MODE))
    //         ((and ARG (numberp (prefix-numeric-value ARG)))
    //          (> (prefix-numeric-value ARG) 0))
    //         (t nil))
    // — 'sym, lists, vectors (arg that's neither 'toggle nor numeric)
    // DISABLE the mode; bare nil toggles.
    let togglem = i.q("toggle");
    let new = match a.first() {
        Some(Value::Sym(s)) if *s == togglem => !cur,
        Some(Value::Nil) | None => !cur,
        Some(Value::Int(n)) => *n > 0,
        // prefix-numeric-value sees floats too.
        Some(Value::Float(f)) => **f > 0.0,
        // prefix-numeric-value accepts '(N) — a 1-elem numeric list.
        Some(Value::Cons(_)) => {
            if let Value::Cons(c) = a[0].clone() {
                let b = c.borrow();
                if matches!(b.cdr, Value::Nil) {
                    match b.car.int() {
                        Some(n) => n > 0,
                        None => false,
                    }
                } else {
                    false
                }
            } else {
                false
            }
        }
        Some(_) => false,
    };
    i.obarray.symbol_mut(sym).value = Value::from_bool(new);
    Ok(Value::from_bool(new))
}

fn f_binary_overwrite_mode(i: &mut Interp, _a: crate::lisp::Argv) -> EvalResult {
    Ok(Value::Sym(i.q("overwrite-mode-binary")))
}
fn f_scroll_lock_mode(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    mode_toggle(i, &a, "scroll-lock-mode")
}
fn f_pixel_scroll_mode(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    mode_toggle(i, &a, "pixel-scroll-mode")
}
fn f_pixel_scroll_precision_mode(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    mode_toggle(i, &a, "pixel-scroll-precision-mode")
}
// font-lock-mode, font-lock-ensure/flush and jit-lock-register/unregister are
// implemented in Lisp (GNU font-core.el / font-lock.el / jit-lock.el ports in
// the prelude).

// ---------- command loop / terminal misc ----------

fn f_move_to_window_line_top_bottom(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // Cycle point through center → bottom → top window lines on repeat.
    let me = i.q("move-to-window-line-top-bottom");
    let this_id = i.q("this-command");
    let last_id = i.q("last-command");
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

fn f_recenter_other_window(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // Scroll other window so its point line is centered. GNU signals
    // "There is no other window" on a single-window frame.
    let sel = sel_window(i).unwrap();
    let other = window_cycle(i, 1, &sel).unwrap_or(sel.clone());
    if Rc::ptr_eq(&other, &sel) {
        return Err(i.error("There is no other window"));
    }
    let arg_n = a.first().and_then(|v| v.int()).unwrap_or(-1);
    let (buf, height, win_point) = {
        let wb = other.borrow();
        (wb.buffer, wb.height, wb.point)
    };
    if let Some(b) = i.buffers.get(buf) {
        let bb = b.borrow();
        // Center on the *other* window's point, not the current buffer's.
        let pl = bb.text.line_of_pos(win_point);
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

fn f_exit_minibuffer(i: &mut Interp, _a: crate::lisp::Argv) -> EvalResult {
    // GNU Fexit_minibuffer is `(throw 'exit nil)': the catch lives in
    // `read_minibuf'.  With no matching catch this signals `no-catch'
    // (f_throw semantics).
    let exit_sym = Value::Sym(i.q("exit"));
    if i.catch_tags
        .iter()
        .any(|t| crate::lisp::eq_values(t, &exit_sym))
    {
        Err(crate::lisp::error::Flow::Throw(exit_sym, Value::Nil))
    } else {
        let no_catch = i.q("no-catch");
        Err(i.signal_data(no_catch, vec![exit_sym, Value::Nil]))
    }
}

fn f_self_insert_and_exit(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // Insert last-command-event's char, then exit-minibuffer.
    let _ = a;
    f_exit_minibuffer(i, vec![].into())
}

fn f_save_buffers_kill_terminal(i: &mut Interp, _a: crate::lisp::Argv) -> EvalResult {
    i.quit_editor = true;
    Ok(Value::Nil)
}

fn f_open_dribble_file(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    match &a[0] {
        Value::Nil => {
            i.dribble_file = None;
        }
        Value::Str(s) => {
            let path = s.borrow().clone();
            if let Err(e) = std::fs::File::create(native_path(i, &path)?) {
                return Err(i.error(format!("Cannot open dribble file {path}: {e}")));
            }
            i.dribble_file = Some((path).to_unicode_lossy());
        }
        _ => return Err(i.wrong_type_mut("stringp", &a[0])),
    }
    Ok(Value::Nil)
}

fn f_suspend_emacs(i: &mut Interp, _a: crate::lisp::Argv) -> EvalResult {
    // GNU batch: suspending on a non-tty terminal signals a plain error.
    if i.noninteractive {
        return Err(i.error("Attempt to suspend a non-text terminal device"));
    }
    i.quit_editor = true;
    Ok(Value::Nil)
}

fn f_byteorder(_i: &mut Interp, _a: crate::lisp::Argv) -> EvalResult {
    // GNU returns ?l (108) on little-endian, ?B (66) on big-endian.
    #[cfg(target_endian = "little")]
    return Ok(Value::Int(108));
    #[cfg(target_endian = "big")]
    return Ok(Value::Int(66));
}

fn f_standard_display_european_internal(_i: &mut Interp, _a: crate::lisp::Argv) -> EvalResult {
    // GNU returns the previous glyph-display flag as a one-element vector.
    Ok(Value::vector(vec![Value::Int(39)]))
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
        .unwrap_or_else(|| i.create_buffer(" *Minibuf-0*"));
    let frame = Frame::new_tty(scratch, mb, 80, 25);
    let initial_windows: Vec<_> = {
        let frame = frame.borrow();
        frame.windows.iter().chain(frame.minibuffer.iter()).cloned().collect()
    };
    for window in initial_windows {
        set_window_old_point(i, &mut window.borrow_mut(), 0);
    }
    // GNU make_initial_frame records the initial root as selection 1;
    // ordinary newly allocated windows keep their initial zero use time.
    frame.borrow().selected.borrow_mut().use_time = next_use_time(i);
    record_buffer_display(i, scratch);
    // Wire the frame's window buffer linkage.
    i.selected_frame = Some(frame.clone());
    i.frames.push(frame);
    // GNU `Vminibuffer_list': index 0 is ` *Minibuf-0*', the
    // never-active null minibuffer.
    if i.minibuf_list.is_empty() {
        i.minibuf_list.push(mb);
    }
    // GNU C: `minibuffer-local-map' is a real (dense) keymap.
    let mlm = i.q("minibuffer-local-map");
    if i.symbol_value(mlm).is_nil() {
        let km = Value::cons(
            Value::Sym(i.q("keymap")),
            Value::cons(keymap_char_table(i), Value::Nil),
        );
        i.obarray.symbol_mut(mlm).value = km;
    }
    // global-map default: a dense keymap (char-table element), as in
    // GNU's `current-global-map'.
    let gm = i.q("global-map");
    if i.symbol_value(gm).is_nil() {
        let km = Value::cons(
            Value::Sym(i.q("keymap")),
            Value::cons(keymap_char_table(i), Value::Nil),
        );
        i.obarray.symbol_mut(gm).value = km;
    }
}
