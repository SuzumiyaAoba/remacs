//! Additional window/frame primitives over the flat window model.
//! Many are stubs returning sensible tty defaults — the window tree
//! (internal windows, splits) is a later milestone.

use std::cell::RefCell;
use std::rc::Rc;

use crate::editor::{
    Window, WindowRef, f_window_list, frame_of, is_terminal, sel_frame, sel_window,
    terminal_token, win_of,
};
use crate::lisp::Interp;
use crate::lisp::builtins::{S, arg, want_int};
use crate::lisp::error::{EvalResult, Flow};
use crate::lisp::sym;
use crate::lisp::value::{Subr, Value};

pub(crate) static SUBRS: &[Subr] = &[
    // --- tree/shape stubs ---
    S!(
        "window-valid-p",
        1,
        1,
        f_window_valid_p,
        "t if WINDOW is a live window object."
    ),
    S!(
        "window-parent",
        0,
        1,
        f_nil_win,
        "Parent window (flat model: nil)."
    ),
    S!(
        "window-top-child",
        0,
        1,
        f_nil_win,
        "First child (flat: nil)."
    ),
    S!("window-left-child", 0, 1, f_window_valid_nil, ""),
    S!(
        "window-next-sibling",
        0,
        1,
        f_next_sibling,
        "Next window in frame order."
    ),
    S!(
        "window-prev-sibling",
        0,
        1,
        f_prev_sibling,
        "Previous window in frame order."
    ),
    S!(
        "window-next-buffers",
        0,
        1,
        f_window_next_buffers,
        "List of buffers recently re-shown in WINDOW."
    ),
    S!(
        "window-prev-buffers",
        0,
        1,
        f_window_prev_buffers,
        "List of buffers previously shown in WINDOW."
    ),
    S!("window-normal-size", 0, 3, f_one_f, "Normal size (1.0)."),
    S!(
        "window-new-total",
        0,
        2,
        f_window_height2,
        "Total lines after resize."
    ),
    S!("window-new-normal", 0, 2, f_one_f, ""),
    S!("window-new-pixel", 0, 2, f_window_height_px, ""),
    S!(
        "window-old-point",
        0,
        1,
        f_window_point1,
        "Old point of WINDOW."
    ),
    S!(
        "window-old-buffer",
        0,
        1,
        f_window_buffer1,
        "Buffer last shown."
    ),
    S!("window-old-pixel-width", 0, 1, f_window_width_px, ""),
    S!("window-old-pixel-height", 0, 1, f_window_height_px, ""),
    S!("window-old-body-pixel-width", 0, 1, f_window_width_px, ""),
    S!("window-old-body-pixel-height", 0, 1, f_window_height_px, ""),
    S!(
        "window-combination-p",
        1,
        2,
        f_false,
        "t if WINDOW is internal (no)."
    ),
    S!(
        "window-has-parameters",
        0,
        1,
        f_window_has_params,
        "t if WINDOW has parameters."
    ),
    S!(
        "window-deletable-p",
        0,
        1,
        f_window_live_t,
        "t if WINDOW can be deleted."
    ),
    // `window-splittable-p' is Lisp (GNU window.el) — see prelude.
    S!(
        "window-min-size",
        0,
        4,
        f_win_min_size,
        "Minimum window size."
    ),
    S!("window-size", 0, 4, f_window_size, "Size of WINDOW."),
    S!(
        "window-safe-min-size",
        0,
        3,
        f_window_safe_min_size,
        "Absolute minimum window size."
    ),
    S!("window-max-delta", 0, 5, f_zero, ""),
    S!("window-min-delta", 0, 5, f_zero, ""),
    S!("window-sizable-p", 1, 4, f_window_sizable_p, ""),
    S!("window-size-fixed-p", 0, 2, f_window_size_fixed_p, ""),
    S!(
        "window-resize",
        3,
        5,
        f_window_resize,
        "Resize WINDOW by DELTA lines."
    ),
    S!("window-resize-apply", 2, 3, f_true2, ""),
    S!("window-resize-apply-total", 2, 3, f_true2, ""),
    S!("window-resize-no-error", 3, 5, f_window_resize, ""),
    S!(
        "window-list-1",
        0,
        4,
        f_window_list1,
        "Windows in cyclic order."
    ),
    S!("window-bump-use-time", 0, 1, f_window_live_nil, ""),
    S!("window-discard-buffer-from-window", 2, 3, f_nil, ""),
    S!(
        "split-window-internal",
        4,
        4,
        f_split_window_internal,
        "Split WINDOW."
    ),
    S!(
        "delete-window-internal",
        1,
        1,
        f_delete_window_internal,
        "Remove WINDOW."
    ),
    S!("uncombine-window", 1, 1, f_uncombine_window, ""),
    S!("combine-windows", 2, 2, f_combine_windows, ""),
    S!(
        "get-lru-window",
        0,
        3,
        f_get_lru_window,
        "Least recently used window."
    ),
    S!(
        "get-mru-window",
        0,
        3,
        f_get_mru_window,
        "Most recently used window."
    ),
    S!(
        "get-largest-window",
        0,
        3,
        f_get_largest_window,
        "Largest window by pixel area."
    ),
    S!(
        "other-window-for-scrolling",
        0,
        0,
        f_other_window_for_scrolling,
        "Window to scroll."
    ),
    S!("coordinates-in-window-p", 2, 2, f_coordinates_in_window_p, ""),
    S!(
        "posn-at-point",
        0,
        2,
        f_posn_at_point,
        "Position info at POINT."
    ),
    S!("posn-at-x-y", 2, 4, f_posn_at_xy, "Position info at X Y."),
    S!(
        "window-vscroll",
        0,
        2,
        f_zero,
        "Vertical scroll amount (0)."
    ),
    S!("set-window-vscroll", 2, 4, f_set_window_vscroll, ""),
    S!(
        "scroll-left",
        0,
        2,
        f_scroll_left,
        "Scroll left COUNT columns."
    ),
    S!(
        "scroll-right",
        0,
        2,
        f_scroll_right,
        "Scroll right COUNT columns."
    ),
    S!(
        "window-fringes",
        0,
        2,
        f_zero4,
        "(l r w out) fringe widths."
    ),
    S!("set-window-fringes", 2, 5, f_set_window_fringes, ""),
    S!(
        "window-margins",
        0,
        1,
        f_window_margins,
        "(left . right) margin widths."
    ),
    S!("set-window-margins", 2, 3, f_set_window_margins, ""),
    S!("window-scroll-bars", 0, 1, f_window_scroll_bars, ""),
    S!("set-window-scroll-bars", 1, 6, f_set_window_scroll_bars, ""),
    S!("window-current-scroll-bars", 0, 1, f_window_current_scroll_bars, ""),
    S!("window-mode-line-height", 0, 1, f_window_mode_line_height, ""),
    S!("window-header-line-height", 0, 1, f_zero, ""),
    S!("window-tab-line-height", 0, 1, f_zero, ""),
    S!("window-bottom-divider-width", 0, 1, f_zero, ""),
    S!("window-right-divider-width", 0, 1, f_zero, ""),
    S!("window-divider-width-valid-p", 1, 1, f_false, ""),
    S!("window-lines-pixel-dimensions", 0, 6, f_window_live_nil, ""),
    S!("window-text-pixel-size", 0, 7, f_window_text_pixel_size, ""),
    S!("window-absolute-pixel-position", 2, 2, f_posn_pair, ""),
    S!(
        "window-screen-lines",
        0,
        1,
        f_window_screen_lines,
        "Lines visible."
    ),
    S!("truncated-partial-width-window-p", 0, 1, f_truncated_partial_width, ""),
    // --- pixel measurements (tty: 1 char = 1 col) ---
    S!(
        "window-text-height",
        0,
        2,
        f_window_height2,
        "Height in pixels (==lines)."
    ),
    S!(
        "window-pixel-width",
        0,
        1,
        f_window_width_px,
        "Width in pixels (==cols)."
    ),
    S!("window-pixel-height", 0, 1, f_window_height_px, ""),
    S!("window-pixel-left", 0, 1, f_window_left_px, ""),
    S!("window-pixel-top", 0, 1, f_window_top_px, ""),
    S!(
        "window-pixel-edges",
        0,
        1,
        f_window_edges4,
        "(l t r b) in pixels."
    ),
    S!("window-inside-pixel-edges", 0, 1, f_window_edges4, ""),
    S!("window-absolute-pixel-edges", 0, 1, f_window_edges4, ""),
    S!(
        "window-inside-absolute-pixel-edges",
        0,
        1,
        f_window_edges4,
        ""
    ),
    S!("window-body-pixel-edges", 0, 1, f_window_body_edges4, ""),
    S!(
        "window-absolute-body-pixel-edges",
        0,
        1,
        f_window_body_edges4,
        ""
    ),
    S!(
        "window-inside-absolute-body-pixel-edges",
        0,
        1,
        f_window_body_edges4,
        ""
    ),
    // `window-safe-min-height'/`window-safe-min-width' are GNU variables.
    // --- frames ---
    S!(
        "frame-root-window",
        0,
        2,
        f_frame_root_window,
        "Root window of FRAME."
    ),
    S!(
        "frame-selected-window",
        0,
        2,
        f_frame_sel_window,
        "Selected window of FRAME."
    ),
    S!("frame-first-window", 0, 2, f_frame_sel_window, ""),
    S!(
        "minibuffer-window",
        0,
        1,
        f_minibuffer_window,
        "The minibuffer window."
    ),
    S!("frame-parent", 0, 1, f_frame_parent, ""),
    S!("frame-ancestor-p", 2, 2, f_frame_ancestor_p, ""),
    S!("frame-old-selected-window", 0, 1, f_frame_sel_window, ""),
    S!("frame-root-frame", 0, 1, f_frame_self, ""),
    S!(
        "frame-initial-p",
        0,
        1,
        f_frame_initial_p,
        "t if FRAME is the initial frame."
    ),
    S!("frame-focus", 0, 1, f_frame_self, "Frame with input focus."),
    S!(
        "frame-pointer-visible-p",
        0,
        1,
        f_frame_pointer_visible_p,
        ""
    ),
    S!("frame-id", 0, 1, f_frame_id, "Opaque frame id."),
    S!("frame-native-width", 0, 1, f_frame_width, ""),
    S!("frame-native-height", 0, 1, f_frame_height, ""),
    S!("frame-text-width", 0, 1, f_frame_width, ""),
    S!("frame-text-height", 0, 1, f_frame_height, ""),
    S!("frame-total-cols", 0, 1, f_frame_width, ""),
    S!("frame-total-lines", 0, 1, f_frame_height, ""),
    S!("frame-text-cols", 0, 1, f_frame_width, ""),
    S!("frame-text-lines", 0, 1, f_frame_height, ""),
    S!("frame-char-width", 0, 1, f_one, ""),
    S!("frame-char-height", 0, 1, f_one, ""),
    S!("frame-fringe-width", 0, 1, f_zero, ""),
    S!("frame-internal-border-width", 0, 1, f_zero, ""),
    S!("frame-right-divider-width", 0, 1, f_zero, ""),
    S!("frame-bottom-divider-width", 0, 1, f_zero, ""),
    S!("frame-scroll-bar-width", 0, 1, f_zero, ""),
    S!("frame-scroll-bar-height", 0, 1, f_zero, ""),
    S!("frame-child-frame-border-width", 0, 1, f_zero, ""),
    S!("frame-scale-factor", 0, 1, f_one_f, ""),
    S!(
        "set-frame-height",
        2,
        4,
        f_set_frame_height,
        "Set FRAME height."
    ),
    S!(
        "set-frame-width",
        2,
        4,
        f_set_frame_width,
        "Set FRAME width."
    ),
    S!("set-frame-size", 3, 4, f_set_frame_size, "Set FRAME size."),
    S!("set-frame-position", 3, 3, f_set_frame_position, ""),
    S!("set-frame-size-and-position-pixelwise", 5, 6, f_frame_live_arg_nil, ""),
    S!("set-frame-window-state-change", 0, 2, f_frame_live_arg_nil, ""),
    S!("frame-window-state-change", 0, 1, f_frame_live_arg_nil, ""),
    S!("frame-after-make-frame", 2, 2, f_frame_after_make_frame, ""),
    S!("frame--set-was-invisible", 2, 2, f_frame_set_was_invisible, ""),
    S!("frame--z-order-lessp", 2, 3, f_true2, ""),
    S!("frame--face-hash-table", 0, 1, f_frame_face_hash_table, ""),
    S!("frame-font-cache", 0, 1, f_nil, ""),
    S!("next-frame", 0, 2, f_frame_self, "Next frame (only one)."),
    S!("previous-frame", 0, 2, f_frame_self, ""),
    S!("old-selected-frame", 0, 0, f_frame_self, ""),
    S!("old-selected-window", 0, 0, f_sel_window, ""),
    S!("raise-frame", 0, 1, f_frame_live_nil, ""),
    S!("lower-frame", 0, 1, f_frame_live_nil, ""),
    S!("make-frame-visible", 0, 1, f_make_frame_visible, ""),
    S!("make-frame-invisible", 0, 2, f_make_frame_invisible, ""),
    S!("iconify-frame", 0, 1, f_frame_live_nil, ""),
    S!("x-focus-frame", 1, 2, f_x_focus_frame, ""),
    S!("redirect-frame-focus", 1, 2, f_redirect_frame_focus, ""),
    S!("reconsider-frame-fonts", 1, 1, f_reconsider_frame_fonts, ""),
    S!("frame--list-z-order", 0, 1, f_frame_list, ""),
    S!("tty-frame-list-z-order", 0, 1, f_frame_list, ""),
    S!("tty-frame-restack", 2, 2, f_tty_frame_restack, ""),
    S!("tty-frame-at", 2, 2, f_frame_self, ""),
    S!("tty-display-color-p", 0, 3, f_nil, ""),
    S!(
        "tty-display-color-cells",
        0,
        2,
        f_tty_colors,
        "Number of tty colors."
    ),
    S!("tty-display-pixel-width", 0, 1, f_frame_width, ""),
    S!("tty-display-pixel-height", 0, 1, f_frame_height, ""),
    S!("tty-type", 0, 1, f_tty_type, "Terminal type name."),
    S!("tty-top-frame", 0, 1, f_frame_self, ""),
    S!("tty-no-underline", 0, 1, f_false, ""),
    S!(
        "tty-suppress-bold-inverse-default-colors",
        1,
        1,
        f_arg0,
        ""
    ),
    S!("controlling-tty-p", 0, 1, f_controlling_tty_p, ""),
    S!("terminal-live-p", 1, 1, f_terminal_live_p, ""),
    S!("terminal-list", 0, 0, f_terminal_list, ""),
    S!("terminal-name", 0, 1, f_terminal_name, ""),
    S!("terminal-parameter", 2, 2, f_terminal_parameter, ""),
    S!("terminal-parameters", 0, 1, f_terminal_parameters, ""),
    S!("set-terminal-parameter", 3, 3, f_set_terminal_parameter, ""),
    // `terminal-id' does not exist in GNU.
    S!("delete-terminal", 0, 2, f_delete_terminal, ""),
    S!("suspend-tty", 0, 1, f_suspend_tty, ""),
    S!("resume-tty", 0, 1, f_resume_tty, ""),
    S!("tty--output-buffer-size", 0, 1, f_not_tty, ""),
    S!("tty--set-output-buffer-size", 1, 2, f_not_tty, ""),
    S!("tty-find-type", 2, 2, f_tty_find_type, ""),
    S!("handle-select-window", 1, 1, f_t, ""),
    S!("innermost-minibuffer-p", 0, 1, f_false, ""),
    S!("minibuffer-innermost-command-loop-p", 0, 1, f_false, ""),
    S!("display-supports-face-attributes-p", 1, 2, f_nil, ""),
    S!("run-window-configuration-change-hook", 0, 1, f_nil, ""),
    S!("run-window-scroll-functions", 0, 1, f_nil, ""),
    S!("set-window-new-total", 2, 3, f_set_window_new_total, ""),
    S!("set-window-new-normal", 1, 2, f_set_window_new_normal, ""),
    S!("set-window-new-pixel", 2, 3, f_set_window_new_pixel, ""),
    S!("set-window-combination-limit", 2, 2, f_set_window_combination_limit, ""),
    S!("set-window-next-buffers", 2, 2, f_set_window_next_buffers, ""),
    S!("set-window-prev-buffers", 2, 2, f_set_window_prev_buffers, ""),
    S!("force-window-update", 0, 1, f_force_window_update, ""),
    S!("resize-mini-window-internal", 1, 1, f_resize_mini_window_internal, ""),
];

fn f_nil(_i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    Ok(Value::Nil)
}

fn f_other_window_for_scrolling(i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    // GNU: with a single window, other-window scrolling errors.
    let f = i
        .selected_frame
        .clone()
        .ok_or_else(|| i.error("No frame"))?;
    let fb = f.borrow();
    if fb.windows.len() <= 1 {
        return Err(i.error("There is no other window"));
    }
    // Return the first non-selected window.
    let sel = fb.selected.clone();
    for w in &fb.windows {
        if w.borrow().id != sel.borrow().id {
            return Ok(Value::Window(w.clone()));
        }
    }
    Err(i.error("There is no other window"))
}

fn f_suspend_tty(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    // GNU batch: the initial terminal is not suspendable.
    let _ = want_terminal_live(i, &arg(&a, 0))?;
    Err(i.error("Attempt to suspend a non-text terminal device"))
}

fn f_resume_tty(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let _ = want_terminal_live(i, &arg(&a, 0))?;
    Err(i.error("Attempt to resume a non-text terminal device"))
}

fn f_delete_terminal(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    // GNU batch: deleting the sole live terminal is an error; a
    // non-terminal arg is quietly ignored (nil).
    match a.first() {
        None | Some(Value::Nil) => {
            Err(i.error("Attempt to delete the sole active display terminal"))
        }
        Some(v) if is_terminal(i, v) => {
            Err(i.error("Attempt to delete the sole active display terminal"))
        }
        _ => Ok(Value::Nil),
    }
}

fn f_force_window_update(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    // GNU: t when OBJECT designates something updateable — nil (all
    // windows), a window, a buffer, or a live buffer name.
    match a.first() {
        None | Some(Value::Nil) => Ok(Value::t()),
        Some(Value::Window(_)) | Some(Value::Buffer(_)) => Ok(Value::t()),
        Some(Value::Str(s)) => {
            let name = s.borrow().clone();
            Ok(Value::from_bool(i.buffers.by_name(&name).is_some()))
        }
        _ => Ok(Value::Nil),
    }
}

fn f_coordinates_in_window_p(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    // GNU: COORDINATES is (X . Y) relative to the frame; returns it when
    // inside WINDOW's body. Our single batch window spans the frame.
    let (x, y) = match &a[0] {
        Value::Cons(c) => {
            let cc = c.borrow();
            (cc.car.clone(), cc.cdr.clone())
        }
        other => return Err(i.wrong_type_mut("consp", other)),
    };
    match (x, y) {
        (Value::Int(_), Value::Int(_)) => Ok(a[0].clone()),
        _ => Ok(Value::Nil),
    }
}

fn f_false(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    f_nil(i, a)
}

fn f_t(_i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    Ok(Value::t())
}

fn f_true2(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    f_t(i, a)
}

fn f_zero(_i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    Ok(Value::Int(0))
}

fn f_one(_i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    Ok(Value::Int(1))
}

fn f_one_f(_i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    Ok(Value::float(1.0))
}

fn f_zero4(_i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    Ok(Value::list(vec![
        Value::Int(0),
        Value::Int(0),
        Value::Int(0),
        Value::Nil,
    ]))
}

/// `window-scroll-bars' — window-live-p check; batch GNU reports
/// (WIDTH COLS VT HEIGHT LINES HT PERSISTENT) = (nil 0 t nil 0 t nil)
/// on a tty frame with no scroll bars.
fn f_window_scroll_bars(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    match arg(&a, 0) {
        Value::Nil | Value::Window(_) => Ok(Value::list(vec![
            Value::Nil,
            Value::Int(0),
            Value::t(),
            Value::Nil,
            Value::Int(0),
            Value::t(),
            Value::Nil,
        ])),
        other => Err(i.wrong_type_mut("window-live-p", &other)),
    }
}

/// `window-current-scroll-bars' — window-live-p check; batch GNU
/// reports the cons (WIDTH . HEIGHT) = (nil . nil) on a tty frame.
fn f_window_current_scroll_bars(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    match arg(&a, 0) {
        Value::Nil | Value::Window(_) => Ok(Value::cons(Value::Nil, Value::Nil)),
        other => Err(i.wrong_type_mut("window-live-p", &other)),
    }
}

fn f_nil_win(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let _ = win_of(i, &arg(&a, 0))?;
    Ok(Value::Nil)
}

fn f_sel_window(i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    match sel_window(i) {
        Some(w) => Ok(Value::Window(w)),
        None => Ok(Value::Nil),
    }
}

fn f_window_valid_p(_i: &mut Interp, a: Vec<Value>) -> EvalResult {
    Ok(Value::from_bool(matches!(&a[0], Value::Window(_))))
}

fn f_window_live_t(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let w = win_of(i, &arg(&a, 0))?;
    Ok(Value::from_bool(!w.borrow().dead))
}

fn siblings(i: &mut Interp, w: &Value, next: bool) -> EvalResult {
    let win = win_of(i, w)?;
    let wid = win.borrow().id;
    for f in &i.frames {
        let fb = f.borrow();
        let n = fb.windows.len();
        for (k, w2) in fb.windows.iter().enumerate() {
            if w2.borrow().id == wid {
                let idx = if next {
                    (k + 1) % n.max(1)
                } else {
                    (k + n.saturating_sub(1)) % n.max(1)
                };
                if let Some(target) = fb.windows.get(idx) {
                    return Ok(Value::Window(target.clone()));
                }
            }
        }
    }
    Ok(Value::Nil)
}

fn f_next_sibling(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    siblings(i, &arg(&a, 0), true)
}

fn f_prev_sibling(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    siblings(i, &arg(&a, 0), false)
}

fn f_window_height2(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let w = win_of(i, &arg(&a, 0))?;
    Ok(Value::Int(w.borrow().height as i128))
}

fn f_window_height_px(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    f_window_height2(i, a)
}

fn f_window_width_px(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let w = win_of(i, &arg(&a, 0))?;
    Ok(Value::Int(w.borrow().width as i128))
}

fn f_window_left_px(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let w = win_of(i, &arg(&a, 0))?;
    Ok(Value::Int(w.borrow().left as i128))
}

fn f_window_top_px(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let w = win_of(i, &arg(&a, 0))?;
    Ok(Value::Int(w.borrow().top as i128))
}

fn f_window_point1(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let w = win_of(i, &arg(&a, 0))?;
    Ok(Value::Int(w.borrow().point as i128 + 1))
}

fn f_window_buffer1(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let w = win_of(i, &arg(&a, 0))?;
    Ok(i.buffer_value(w.borrow().buffer).unwrap_or(Value::Nil))
}

fn f_window_has_params(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let w = win_of(i, &arg(&a, 0))?;
    let p = w.borrow().params.clone();
    Ok(Value::from_bool(!p.is_nil()))
}

/// `window-min-size' — GNU's decode_any_window convention: nil →
/// selected, non-window → plain "N is not a valid window" error.
/// Safe minimum plus decorations, clamped to `window-min-height'/
/// `window-min-width' unless IGNORE suppresses them.
fn f_win_min_size(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let wv = match &arg(&a, 0) {
        Value::Nil => sel_window(i).map(Value::Window).unwrap_or(Value::Nil),
        v @ Value::Window(_) => v.clone(),
        other => {
            let shown = i.princ_to_string(other);
            return Err(i.error(format!("{shown} is not a valid window")));
        }
    };
    let horiz = arg(&a, 1).truthy();
    let ignore = arg(&a, 2);
    Ok(Value::Int(crate::lisp::builtins::misc::win_min_size(
        i, &wv, horiz, &ignore,
    )))
}

/// `window-size' — GNU's subr uses CHECK_VALID_WINDOW (typed
/// `window-valid-p' error) and returns the total size.
fn f_window_size(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let w = match arg(&a, 0) {
        Value::Nil => sel_window(i),
        Value::Window(w) => Some(w),
        other => return Err(i.wrong_type_mut("window-valid-p", &other)),
    };
    let horiz = arg(&a, 1).truthy();
    let (wd, ht) = w
        .map(|w| {
            let w = w.borrow();
            (w.width, w.height)
        })
        .unwrap_or((80, 24));
    Ok(Value::Int(if horiz { wd } else { ht } as i128))
}

/// `window-safe-min-size' — `window-safe-min-height'/`-width' (1/2).
fn f_window_safe_min_size(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    match arg(&a, 0) {
        Value::Nil | Value::Window(_) => {}
        other => return Err(i.wrong_type_mut("window-valid-p", &other)),
    }
    let horiz = arg(&a, 1).truthy();
    Ok(Value::Int(if horiz { 2 } else { 1 }))
}

fn f_window_resize(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let w = win_of(i, &arg(&a, 0))?;
    let delta = want_int(i, &a[1])?;
    let horizontal = arg(&a, 2).truthy();
    let mut wb = w.borrow_mut();
    if horizontal {
        wb.width = (wb.width as i128 + delta).max(1) as usize;
    } else {
        wb.height = (wb.height as i128 + delta).max(1) as usize;
    }
    Ok(Value::Nil)
}

fn f_window_list1(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    // (window-list-1 WINDOW MINIBUF ALL-FRAMES): flat model — same
    // filtering as `window-list' on WINDOW's frame (the selected one).
    f_window_list(i, vec![Value::Nil, arg(&a, 1)])
}

/// Candidate windows for `get-lru-window'/`get-mru-window'/
/// `get-largest-window': the live, non-minibuffer windows of the
/// selected frame.  DEDICATED (arg 1) non-nil includes windows
/// dedicated to their buffers; NOT-SELECTED (arg 2) non-nil excludes
/// the selected window.
fn some_window_candidates(i: &mut Interp, a: &[Value]) -> Vec<WindowRef> {
    let dedicated_ok = a.get(1).is_some_and(|v| v.truthy());
    let not_selected = a.get(2).is_some_and(|v| v.truthy());
    let sel = sel_frame(i).map(|f| f.borrow().selected.clone());
    match sel_frame(i) {
        Some(f) => f
            .borrow()
            .windows
            .iter()
            .filter(|w| {
                let wb = w.borrow();
                !wb.minibuffer
                    && (dedicated_ok || !wb.dedicated)
                    && !(not_selected
                        && sel.as_ref().map_or(false, |s| {
                            s.borrow().id == wb.id
                        }))
            })
            .cloned()
            .collect(),
        None => Vec::new(),
    }
}

fn f_split_window_internal(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    // (split-window-internal WINDOW SIZE SIDE PIXELWISE) → new window.
    let w = win_of(i, &a[0])?;
    let size = want_int(i, &a[1])?;
    let buf = w.borrow().buffer;
    let new_id: usize = i.frames.iter().map(|f| f.borrow().windows.len()).sum();
    let mut wb = w.borrow_mut();
    let horizontal = i
        .sym_id(&a[2])
        .map(|id| i.symbol_name(id) == "right" || i.symbol_name(id) == "left")
        .unwrap_or(false);
    let new_win = crate::editor::Window {
        id: wb.id + 1000 + new_id,
        buffer: buf,
        point: wb.point,
        start: wb.start,
        hscroll: wb.hscroll,
        top: if horizontal {
            wb.top
        } else {
            wb.top + size.max(0) as usize
        },
        height: if horizontal {
            wb.height
        } else {
            (wb.height as i128 - size).max(1) as usize
        },
        left: if horizontal {
            wb.left + size.max(0) as usize
        } else {
            wb.left
        },
        width: if horizontal {
            (wb.width as i128 - size).max(1) as usize
        } else {
            wb.width
        },
        dedicated: false,
        minibuffer: false,
        params: Value::Nil,
        margins: (0, 0),
        use_time: 0,
        prev_buffers: Value::Nil,
        next_buffers: Value::Nil,
        dead: false,
    };
    if horizontal {
        wb.width = size.max(1) as usize;
    } else {
        wb.height = size.max(1) as usize;
    }
    drop(wb);
    let wr = std::rc::Rc::new(std::cell::RefCell::new(new_win));
    if let Some(f) = sel_frame(i) {
        f.borrow_mut().windows.push(wr.clone());
    }
    Ok(Value::Window(wr))
}

fn f_delete_window_internal(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let w = win_of(i, &a[0])?;
    let wid = w.borrow().id;
    for f in &i.frames {
        let mut fb = f.borrow_mut();
        if fb.windows.len() <= 1 {
            continue;
        }
        if let Some(pos) = fb.windows.iter().position(|x| x.borrow().id == wid) {
            let removed = fb.windows.remove(pos);
            removed.borrow_mut().dead = true;
            if fb.selected.borrow().id == wid {
                if let Some(first) = fb.windows.first() {
                    fb.selected = first.clone();
                }
            }
            return Ok(Value::Nil);
        }
    }
    Ok(Value::Nil)
}

fn f_get_lru_window(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    // GNU Fget_lru_window: lowest use-time among the candidates.
    match some_window_candidates(i, &a)
        .iter()
        .min_by_key(|w| w.borrow().use_time)
    {
        Some(w) => Ok(Value::Window(w.clone())),
        None => Ok(Value::Nil),
    }
}

fn f_get_mru_window(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    // GNU Fget_mru_window: highest use-time among the candidates
    // (rev() so ties keep the first, matching GNU's strict-> scan).
    match some_window_candidates(i, &a)
        .iter()
        .rev()
        .max_by_key(|w| w.borrow().use_time)
    {
        Some(w) => Ok(Value::Window(w.clone())),
        None => Ok(Value::Nil),
    }
}

fn f_get_largest_window(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    // GNU Fget_largest_window: maximum pixel area (ties keep the
    // first, i.e. the least recently used among equals).
    match some_window_candidates(i, &a)
        .iter()
        .rev()
        .max_by_key(|w| {
            let wb = w.borrow();
            wb.width * wb.height
        }) {
        Some(w) => Ok(Value::Window(w.clone())),
        None => Ok(Value::Nil),
    }
}

fn f_posn_at_point(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let pos = a.get(0).and_then(|v| v.int()).unwrap_or_else(|| {
        i.current_buffer_ref()
            .map(|b| b.borrow().point() as i128 + 1)
            .unwrap_or(1)
    });
    // (x y) → col/line in window.
    let _ = pos;
    Ok(Value::list(vec![
        Value::Int(0),
        Value::Int(0),
        Value::Nil,
        Value::Nil,
        Value::Int(0),
        Value::Int(0),
        Value::Nil,
        Value::Nil,
        Value::Int(pos),
        Value::list(vec![Value::Int(0), Value::Int(0)]),
        Value::Nil,
        Value::Int(0),
        Value::Nil,
        Value::list(vec![Value::Int(0), Value::Int(0)]),
        Value::Nil,
        Value::list(vec![Value::Int(0), Value::Int(0)]),
        Value::Int(0),
        Value::Int(0),
    ]))
}

fn f_posn_at_xy(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let x = want_int(i, &a[0])?;
    let y = want_int(i, &a[1])?;
    // Approximate position: line y, column x of the current buffer.
    let b = i
        .current_buffer_ref()
        .ok_or_else(|| i.error("No current buffer"))?;
    let bb = b.borrow();
    let line = bb.text.line_start(y.max(0) as usize);
    let pos = (line + x.max(0) as usize).min(bb.text_len());
    Ok(Value::list(vec![
        Value::Window(sel_window(i).unwrap()),
        Value::Int(pos as i128 + 1),
        Value::Int(x),
        Value::Int(y),
        Value::Nil,
    ]))
}

fn f_posn_pair(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let x = want_int(i, &a[0])?;
    let y = want_int(i, &a[1])?;
    Ok(Value::cons(Value::Int(x), Value::Int(y)))
}

fn f_set_window_vscroll(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let _ = win_of(i, &arg(&a, 0))?;
    // A tty window has no vscroll; GNU reports 0.
    Ok(Value::Int(0))
}

fn f_scroll_left(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let n = arg(&a, 0).int().unwrap_or(1);
    let w = win_of(i, &arg(&a, 1))?;
    w.borrow_mut().hscroll += n.max(0) as usize;
    Ok(Value::Nil)
}

fn f_scroll_right(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let n = arg(&a, 0).int().unwrap_or(1);
    let w = win_of(i, &arg(&a, 1))?;
    let mut wb = w.borrow_mut();
    wb.hscroll = wb.hscroll.saturating_sub(n.max(0) as usize);
    Ok(Value::Nil)
}

fn f_window_margins(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let w = match &arg(&a, 0) {
        Value::Nil => sel_window(i).ok_or_else(|| i.error("No window"))?,
        Value::Window(w) if !w.borrow().dead => w.clone(),
        other => return Err(i.wrong_type_mut("window-live-p", other)),
    };
    let m = w.borrow().margins;
    // GNU always returns (LEFT . RIGHT), nil sides for zero margins.
    let lv = if m.0 == 0 { Value::Nil } else { Value::Int(m.0 as i128) };
    let rv = if m.1 == 0 { Value::Nil } else { Value::Int(m.1 as i128) };
    Ok(Value::cons(lv, rv))
}

fn f_window_edges4(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let w = win_of(i, &arg(&a, 0))?;
    let wb = w.borrow();
    Ok(Value::list(vec![
        Value::Int(wb.left as i128),
        Value::Int(wb.top as i128),
        Value::Int((wb.left + wb.width) as i128),
        Value::Int((wb.top + wb.height) as i128),
    ]))
}

fn f_window_body_edges4(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    // Body excludes mode line (1 line at bottom).
    let w = win_of(i, &arg(&a, 0))?;
    let wb = w.borrow();
    Ok(Value::list(vec![
        Value::Int(wb.left as i128),
        Value::Int(wb.top as i128),
        Value::Int((wb.left + wb.width) as i128),
        Value::Int((wb.top + wb.height.saturating_sub(1)) as i128),
    ]))
}

fn f_window_screen_lines(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let w = win_of(i, &arg(&a, 0))?;
    Ok(Value::float(w.borrow().height as f64))
}

// ---------- frames ----------

fn f_frame_root_window(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let f = frame_of(i, &arg(&a, 0))?;
    // GNU: with one live window that window IS the root; otherwise a
    // covering internal window spanning the frame minus the echo area.
    let live: Vec<WindowRef> = f
        .borrow()
        .windows
        .iter()
        .filter(|w| !w.borrow().dead)
        .cloned()
        .collect();
    if live.len() == 1 {
        return Ok(Value::Window(live[0].clone()));
    }
    let mut fb = f.borrow_mut();
    if fb.root.is_none() {
        let r = Window::new(usize::MAX);
        {
            let mini_top = fb
                .minibuffer
                .as_ref()
                .map(|m| m.borrow().top)
                .unwrap_or(fb.height);
            // The root covers the content area: from the topmost
            // child edge (0 before the first split, 1 once GNU's
            // menu-bar row materialized) down to the echo area.
            let top = fb
                .windows
                .iter()
                .map(|w| w.borrow().top)
                .min()
                .unwrap_or(0);
            let mut rb = r.borrow_mut();
            rb.left = 0;
            rb.top = top;
            rb.width = fb.width;
            rb.height = mini_top.saturating_sub(top);
        }
        fb.root = Some(r);
    }
    Ok(Value::Window(fb.root.as_ref().unwrap().clone()))
}

fn f_frame_sel_window(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let f = frame_of(i, &arg(&a, 0))?;
    Ok(Value::Window(f.borrow().selected.clone()))
}

fn f_minibuffer_window(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let f = frame_of(i, &arg(&a, 0))?;
    match &f.borrow().minibuffer {
        Some(w) => Ok(Value::Window(w.clone())),
        None => Ok(Value::Nil),
    }
}

fn f_frame_self(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    match frame_of(i, &arg(&a, 0)) {
        Ok(f) => Ok(Value::Frame(f)),
        Err(_) => Ok(Value::Nil),
    }
}

fn f_frame_initial_p(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    match frame_of(i, &arg(&a, 0)) {
        Ok(f) => Ok(Value::from_bool(f.borrow().id == 0)),
        Err(_) => Ok(Value::Nil),
    }
}

fn f_frame_id(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let f = frame_of(i, &arg(&a, 0))?;
    Ok(Value::Int(f.borrow().id as i128))
}

fn f_frame_width(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let f = frame_of(i, &arg(&a, 0))?;
    Ok(Value::Int(f.borrow().width as i128))
}

fn f_frame_height(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let f = frame_of(i, &arg(&a, 0))?;
    Ok(Value::Int(f.borrow().height as i128))
}

fn f_set_frame_height(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let f = frame_of(i, &arg(&a, 0))?;
    let h = want_int(i, &a[1])?;
    f.borrow_mut().height = h.max(1) as usize;
    Ok(Value::Nil)
}

fn f_set_frame_width(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let f = frame_of(i, &arg(&a, 0))?;
    let w = want_int(i, &a[1])?;
    f.borrow_mut().width = w.max(1) as usize;
    Ok(Value::Nil)
}

fn f_set_frame_size(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let f = frame_of(i, &arg(&a, 0))?;
    let w = want_int(i, &a[1])?;
    let h = want_int(i, &a[2])?;
    let mut fb = f.borrow_mut();
    fb.width = w.max(1) as usize;
    fb.height = h.max(1) as usize;
    Ok(Value::Nil)
}

fn f_frame_list(i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    let fs: Vec<Value> = i.frames.iter().map(|f| Value::Frame(f.clone())).collect();
    Ok(Value::list(fs))
}

fn f_tty_colors(_i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    Ok(Value::Int(256))
}

fn f_arg0(_i: &mut Interp, a: Vec<Value>) -> EvalResult {
    Ok(a[0].clone())
}

fn f_tty_type(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    // GNU: nil on a tty batch terminal; TERMINAL must be terminal-live-p.
    let _ = want_terminal_live(i, &arg(&a, 0))?;
    Ok(Value::Nil)
}

/// nil or the terminal record; anything else is a terminal-live-p error.
fn want_terminal_live(i: &mut Interp, v: &Value) -> EvalResult {
    match v {
        Value::Nil => Ok(Value::Nil),
        w if is_terminal(i, w) => Ok(Value::Nil),
        other => Err(i.wrong_type_mut("terminal-live-p", other)),
    }
}

/// `window-live-p' check on an optional arg, then nil (all our
/// windows are live).
fn f_window_live_nil(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    match arg(&a, 0) {
        Value::Nil | Value::Window(_) => Ok(Value::Nil),
        other => Err(i.wrong_type_mut("window-live-p", &other)),
    }
}

/// `window-valid-p' check on an optional arg, then nil.
fn f_window_valid_nil(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    match arg(&a, 0) {
        Value::Nil | Value::Window(_) => Ok(Value::Nil),
        other => Err(i.wrong_type_mut("window-valid-p", &other)),
    }
}

/// GNU's plain `error "N is not a valid window"' (window.c signals a
/// bare `error', not a typed one, for these predicates).
fn err_not_valid_window(i: &mut Interp, v: &Value) -> Flow {
    let shown = i.princ_to_string(v);
    i.error(format!("{shown} is not a valid window"))
}

/// Same pattern for live-window checks ("N is not a live window").
fn err_not_live_window(i: &mut Interp, v: &Value) -> Flow {
    let shown = i.princ_to_string(v);
    i.error(format!("{shown} is not a live window"))
}

/// `window-sizable-p' — GNU: normalize WINDOW, then compare
/// `window-sizable' against DELTA.
fn f_window_sizable_p(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    match arg(&a, 0) {
        Value::Nil | Value::Window(_) => {}
        ref other => {
            let shown = i.princ_to_string(other);
            return Err(i.error(format!("{shown} is not a valid window")));
        }
    }
    let delta = match &arg(&a, 1) {
        Value::Int(n) => *n,
        other => return Err(i.wrong_type_mut("number-or-marker-p", other)),
    };
    let wv = arg(&a, 0);
    let horiz = arg(&a, 2).truthy();
    let ignore = arg(&a, 3);
    let actual = if delta < 0 {
        let min = crate::lisp::builtins::misc::win_min_size(i, &wv, horiz, &ignore);
        let size = match &wv {
            Value::Window(w) => {
                let w = w.borrow();
                if horiz {
                    w.width
                } else {
                    w.height
                }
            }
            _ => {
                if horiz {
                    80
                } else {
                    24
                }
            }
        } as i128;
        if size <= min {
            0
        } else {
            (min - size).max(delta)
        }
    } else if delta > 0 {
        delta
    } else {
        0
    };
    Ok(Value::from_bool(if delta > 0 {
        actual >= delta
    } else {
        actual <= delta
    }))
}

/// `set-frame-position' — frame-live-p check, then t (GNU reports
/// success even though a tty has no real positioning).
fn f_set_frame_position(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    match arg(&a, 0) {
        Value::Nil | Value::Frame(_) => Ok(Value::t()),
        other => Err(i.wrong_type_mut("frame-live-p", &other)),
    }
}

/// `set-frame-size-and-position-pixelwise' /
/// `set-frame-window-state-change' / `frame-window-state-change' —
/// frame-live-p on the first/optional FRAME argument, then nil.
fn f_frame_live_arg_nil(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    match arg(&a, 0) {
        Value::Nil | Value::Frame(_) => Ok(Value::Nil),
        other => Err(i.wrong_type_mut("frame-live-p", &other)),
    }
}

/// `tty-frame-restack' — GNU's tty version always signals that the
/// operation is not implemented.
fn f_tty_frame_restack(i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    Err(i.error("tty-frame-restack is not implemented"))
}

/// `resize-mini-window-internal' — GNU requires a minibuffer window.
fn f_resize_mini_window_internal(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    match arg(&a, 0) {
        Value::Window(w) => {
            let is_mini = w.borrow().minibuffer;
            if is_mini {
                Ok(Value::Nil)
            } else {
                Err(i.error("Not a valid minibuffer window"))
            }
        }
        other => Err(err_not_valid_window(i, &other)),
    }
}

/// `frame-parent' — frame-live-p check on the optional FRAME; tty
/// frames have no parent so nil.
fn f_frame_parent(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    match arg(&a, 0) {
        Value::Nil | Value::Frame(_) => Ok(Value::Nil),
        other => Err(i.wrong_type_mut("frame-live-p", &other)),
    }
}

/// `window-size-fixed-p' — nil on our model (nothing is size-fixed).
fn f_window_size_fixed_p(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    match arg(&a, 0) {
        Value::Nil | Value::Window(_) => Ok(Value::Nil),
        other => Err(err_not_valid_window(i, &other)),
    }
}

/// `truncated-partial-width-window-p' — nil; GNU checks the window
/// is live with the plain "not a live window" error.
fn f_truncated_partial_width(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    match arg(&a, 0) {
        Value::Nil | Value::Window(_) => Ok(Value::Nil),
        other => Err(err_not_live_window(i, &other)),
    }
}

/// `window-mode-line-height' — 1 on tty; GNU uses a typed
/// `window-live-p' check here (unlike the predicates above).
fn f_window_mode_line_height(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    match arg(&a, 0) {
        Value::Nil | Value::Window(_) => Ok(Value::Int(1)),
        other => Err(i.wrong_type_mut("window-live-p", &other)),
    }
}

/// `frame-ancestor-p' — both args must be live frames; our single
/// frame is no one's ancestor.
fn f_frame_ancestor_p(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    for v in &a[..2] {
        match v {
            Value::Frame(_) => {}
            other => return Err(i.wrong_type_mut("frame-live-p", other)),
        }
    }
    Ok(Value::Nil)
}

/// `set-window-new-normal' — window-valid-p check (nil means the
/// selected window); GNU returns the SIZE argument (nil when omitted).
fn f_set_window_new_normal(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    match &a[0] {
        Value::Nil | Value::Window(_) => Ok(arg(&a, 1)),
        other => Err(i.wrong_type_mut("window-valid-p", other)),
    }
}

/// `set-window-new-pixel' — window-valid-p check, then nil.
fn f_set_window_new_pixel(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    match &a[0] {
        Value::Nil | Value::Window(_) => Ok(Value::Nil),
        other => Err(i.wrong_type_mut("window-valid-p", other)),
    }
}

/// `set-window-combination-limit' — a leaf window cannot hold a
/// combination limit; GNU signals a plain error.
fn f_set_window_combination_limit(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    match &a[0] {
        Value::Nil | Value::Window(_) => {
            Err(i.error("Combination limit is meaningful for internal windows only"))
        }
        other => Err(i.wrong_type_mut("window-valid-p", other)),
    }
}

/// `window-next-buffers' — list of buffers recorded by
/// `unrecord-window-buffer'/quit-restore; nil selects the selected
/// window.
fn f_window_next_buffers(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let w = match arg(&a, 0) {
        Value::Nil => sel_window(i).ok_or_else(|| i.error("No window"))?,
        other => win_of(i, &other)?,
    };
    Ok(w.borrow().next_buffers.clone())
}

/// `window-prev-buffers' — list of `(buffer start point)' entries
/// for buffers previously shown in WINDOW.
fn f_window_prev_buffers(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let w = match arg(&a, 0) {
        Value::Nil => sel_window(i).ok_or_else(|| i.error("No window"))?,
        other => win_of(i, &other)?,
    };
    Ok(w.borrow().prev_buffers.clone())
}

/// `set-window-next-buffers' — replace WINDOW's next-buffers list.
fn f_set_window_next_buffers(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let w = match &a[0] {
        Value::Nil => sel_window(i).ok_or_else(|| i.error("No window"))?,
        other => win_of(i, other)?,
    };
    w.borrow_mut().next_buffers = arg(&a, 1);
    Ok(Value::Nil)
}

/// `set-window-prev-buffers' — replace WINDOW's prev-buffers list.
fn f_set_window_prev_buffers(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let w = match &a[0] {
        Value::Nil => sel_window(i).ok_or_else(|| i.error("No window"))?,
        other => win_of(i, other)?,
    };
    w.borrow_mut().prev_buffers = arg(&a, 1);
    Ok(Value::Nil)
}

/// `set-window-margins' — window-live-p check (nil = selected
/// window), stores the margin widths on the window; GNU returns t.
fn f_set_window_margins(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let w = match &a[0] {
        Value::Nil => sel_window(i).ok_or_else(|| i.error("No window"))?,
        Value::Window(w) => w.clone(),
        other => return Err(i.wrong_type_mut("window-live-p", other)),
    };
    let l = arg(&a, 1).int().unwrap_or(0).max(0) as usize;
    let r = arg(&a, 2).int().unwrap_or(0).max(0) as usize;
    w.borrow_mut().margins = (l, r);
    Ok(Value::t())
}

/// `set-window-fringes' — window-live-p check; GNU returns t.
fn f_set_window_fringes(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    match &a[0] {
        Value::Nil | Value::Window(_) => Ok(Value::t()),
        other => Err(i.wrong_type_mut("window-live-p", &other)),
    }
}

/// `set-window-scroll-bars' — window-live-p check; GNU returns nil.
fn f_set_window_scroll_bars(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    match &a[0] {
        Value::Nil | Value::Window(_) => Ok(Value::Nil),
        other => Err(i.wrong_type_mut("window-live-p", &other)),
    }
}

/// `raise-frame'/`lower-frame'/`iconify-frame' — frame-live-p check,
/// then nil (no window manager to talk to).
fn f_frame_live_nil(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    match arg(&a, 0) {
        Value::Nil | Value::Frame(_) => Ok(Value::Nil),
        other => Err(i.wrong_type_mut("frame-live-p", &other)),
    }
}

/// `frame-after-make-frame' — GNU checks FRAME with frame-live-p
/// and returns the VALUE flag.
fn f_frame_after_make_frame(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    match &a[0] {
        Value::Frame(_) => Ok(arg(&a, 1)),
        other => Err(i.wrong_type_mut("frame-live-p", other)),
    }
}

/// `frame--set-was-invisible' — frame-live-p check on FRAME; GNU
/// returns WAS-INVISIBLE.
fn f_frame_set_was_invisible(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    match &a[0] {
        Value::Frame(_) => Ok(arg(&a, 1)),
        other => Err(i.wrong_type_mut("frame-live-p", other)),
    }
}

/// `redirect-frame-focus' — GNU checks both args with framep; the
/// redirect itself is a display detail we don't model.
fn f_redirect_frame_focus(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    for v in &a[..2] {
        match v {
            Value::Nil | Value::Frame(_) => {}
            other => return Err(i.wrong_type_mut("framep", other)),
        }
    }
    Ok(Value::Nil)
}

/// `tty-find-type' — GNU funcalls the first argument; a non-callable
/// one raises `invalid-function'.
fn f_tty_find_type(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let callable = match &a[0] {
        Value::Sym(s) => i.fbound_p(*s),
        Value::Lambda(_) | Value::Subr(_) => true,
        _ => false,
    };
    if !callable {
        let s = i.intern("invalid-function");
        return Err(i.signal_data(s, vec![a[0].clone()]));
    }
    i.apply(&a[0], vec![a[1].clone()])
}

/// The terminal parameter alist: GNU's tty defaults, seeded lazily so
/// `set-terminal-parameter' can overwrite them.
fn terminal_params(i: &mut Interp) -> &Vec<(Value, Value)> {
    if i.terminal_params.is_empty() {
        i.terminal_params = vec![
            (
                Value::Sym(i.intern("normal-erase-is-backspace")),
                Value::Int(0),
            ),
            (
                Value::Sym(i.intern("keyboard-coding-saved-meta-mode")),
                Value::list(vec![Value::Sym(sym::T)]),
            ),
        ];
    }
    &i.terminal_params
}

fn param_eq(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Sym(x), Value::Sym(y)) => x == y,
        (Value::Int(x), Value::Int(y)) => x == y,
        (Value::Str(x), Value::Str(y)) => x == y,
        _ => false,
    }
}

fn f_terminal_parameters(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let _ = want_terminal_live(i, &arg(&a, 0))?;
    let entries = terminal_params(i)
        .iter()
        .map(|(k, v)| Value::cons(k.clone(), v.clone()))
        .collect();
    Ok(Value::list(entries))
}

fn f_terminal_parameter(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let _ = want_terminal_live(i, &a[0])?;
    // assq over the alist.
    for (k, v) in terminal_params(i) {
        if param_eq(k, &a[1]) {
            return Ok(v.clone());
        }
    }
    Ok(Value::Nil)
}

/// `set-terminal-parameter` — set/overwrite a parameter on the
/// terminal's alist.
fn f_set_terminal_parameter(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let _ = want_terminal_live(i, &a[0])?;
    let _ = terminal_params(i);
    for entry in i.terminal_params.iter_mut() {
        if param_eq(&entry.0, &a[1]) {
            entry.1 = a[2].clone();
            return Ok(Value::Nil);
        }
    }
    i.terminal_params.push((a[1].clone(), a[2].clone()));
    Ok(Value::Nil)
}

fn f_frame_face_hash_table(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    // Validate FRAME like GNU (frame-live-p), then hand back a hash
    // table of the known faces.
    if let Some(v) = a.first() {
        match v {
            Value::Nil | Value::Frame(_) => {}
            other => return Err(i.wrong_type_mut("frame-live-p", other)),
        }
    }
    use crate::lisp::value::{HashTest, LispHash};
    let h = Value::Hash(Rc::new(RefCell::new(LispHash::new(HashTest::Eq))));
    Ok(h)
}

/// `controlling-tty-p' — GNU validates the optional TERMINAL with
/// terminal-live-p; in batch we are never the controlling tty → nil.
fn f_controlling_tty_p(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    match arg(&a, 0) {
        Value::Nil => Ok(Value::Nil),
        v if is_terminal(i, &v) => Ok(Value::Nil),
        other => Err(i.wrong_type_mut("terminal-live-p", &other)),
    }
}

fn f_terminal_live_p(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    // Our only terminal is the singleton record (nil = default
    // terminal); a live frame means its terminal is live.
    Ok(Value::from_bool(match &a[0] {
        Value::Nil => true,
        Value::Frame(f) => !f.borrow().dead,
        v => is_terminal(i, v),
    }))
}

fn f_uncombine_window(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    // nil = selected window; nothing to uncombine in batch → nil.
    match &a[0] {
        Value::Nil | Value::Window(_) => Ok(Value::Nil),
        other => Err(i.wrong_type_mut("window-valid-p", other)),
    }
}

fn f_combine_windows(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    match &a[0] {
        Value::Window(_) => Ok(Value::Nil),
        other => Err(i.wrong_type_mut("window-valid-p", other)),
    }
}

fn f_terminal_list(i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    Ok(Value::list(vec![terminal_token(i)]))
}

/// `window-text-pixel-size' — fixed-pitch model: (WIDTH . HEIGHT)
/// in pixels; MODE-AND-HEADER-LINE arg adds the header rows (1).
fn f_window_text_pixel_size(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    if let Some(v) = a.first() {
        match v {
            Value::Nil | Value::Window(_) => {}
            other => return Err(i.wrong_type_mut("window-live-p", other)),
        }
    }
    let h = if arg(&a, 5).truthy() { 1 } else { 0 };
    Ok(Value::cons(Value::Int(0), Value::Int(h)))
}

/// tty output-buffer helpers on a non-tty terminal → GNU error.
fn f_not_tty(i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    Err(i.error("Not a tty terminal"))
}

/// `make-frame-invisible' — our batch frame is always the sole
/// visible frame → GNU's error.
fn f_make_frame_invisible(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    if let Some(v) = a.first() {
        match v {
            Value::Nil | Value::Frame(_) => {}
            other => return Err(i.wrong_type_mut("frame-live-p", other)),
        }
    }
    Err(i.error("Attempt to make invisible the sole visible or iconified frame"))
}

/// `reconsider-frame-fonts' — frame check, then GNU's non-wsi error.
fn f_reconsider_frame_fonts(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    match &a[0] {
        Value::Nil | Value::Frame(_) => {}
        other => return Err(i.wrong_type_mut("frame-live-p", other)),
    }
    Err(i.error("Window system frame should be used"))
}

fn f_terminal_name(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let _ = want_terminal_live(i, &arg(&a, 0))?;
    Ok(Value::string("initial_terminal"))
}

fn f_set_window_new_total(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let w = win_of(i, &arg(&a, 0))?;
    let n = want_int(i, &a[1])?;
    w.borrow_mut().height = n.max(1) as usize;
    Ok(Value::Nil)
}

fn f_frame_pointer_visible_p(_i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    Ok(Value::t())
}

fn f_x_focus_frame(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    // GNU errors "Cannot switch to an invisible frame" etc. in batch;
    // on a lone tty frame any arg raises `error'.
    let _ = a;
    let sym = i.intern("error");
    Err(i.signal_data(sym, Vec::new()))
}

fn f_make_frame_visible(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    match &arg(&a, 0) {
        Value::Frame(_) => Ok(a[0].clone()),
        _ => match crate::editor::sel_frame(i) {
            Some(f) => Ok(Value::Frame(f)),
            None => Ok(Value::Nil),
        },
    }
}
