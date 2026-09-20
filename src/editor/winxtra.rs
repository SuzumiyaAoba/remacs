//! Additional window/frame primitives over the flat window model.
//! Many are stubs returning sensible tty defaults — the window tree
//! (internal windows, splits) is a later milestone.

use crate::editor::{frame_of, sel_frame, sel_window, win_of};
use crate::lisp::Interp;
use crate::lisp::builtins::{S, arg, want_int};
use crate::lisp::error::EvalResult;
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
    S!("window-left-child", 0, 1, f_nil_win, ""),
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
        f_nil,
        "Recently shown buffers (nil)."
    ),
    S!("window-prev-buffers", 0, 1, f_nil, ""),
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
    S!("window-combination-limit", 0, 2, f_nil, ""),
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
    S!(
        "window-splittable-p",
        0,
        2,
        f_t,
        "t if WINDOW is splittable."
    ),
    S!(
        "window-min-size",
        0,
        4,
        f_win_min_size,
        "Minimum window size."
    ),
    S!("window-max-delta", 0, 5, f_zero, ""),
    S!("window-min-delta", 0, 5, f_zero, ""),
    S!("window-sizable-p", 1, 4, f_t, ""),
    S!("window-size-fixed-p", 0, 2, f_false, ""),
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
    S!("window-bump-use-time", 1, 1, f_nil, ""),
    S!("window-discard-buffer-from-window", 2, 2, f_nil, ""),
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
    S!("uncombine-window", 1, 2, f_false, ""),
    S!("combine-windows", 1, 3, f_false, ""),
    S!(
        "get-lru-window",
        0,
        3,
        f_get_lru_window,
        "Least recently used window."
    ),
    S!("get-largest-window", 0, 3, f_get_lru_window, ""),
    S!(
        "other-window-for-scrolling",
        0,
        0,
        f_other_window,
        "Window to scroll."
    ),
    S!("coordinates-in-window-p", 2, 2, f_false, ""),
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
    S!("set-window-fringes", 2, 5, f_nil, ""),
    S!(
        "window-margins",
        0,
        1,
        f_window_margins,
        "(left . right) margin widths."
    ),
    S!("set-window-margins", 2, 3, f_nil, ""),
    S!("window-scroll-bars", 0, 1, f_zero4, ""),
    S!("set-window-scroll-bars", 2, 5, f_nil, ""),
    S!("window-current-scroll-bars", 0, 1, f_zero4, ""),
    S!("window-mode-line-height", 0, 1, f_one, ""),
    S!("window-header-line-height", 0, 1, f_zero, ""),
    S!("window-tab-line-height", 0, 1, f_zero, ""),
    S!("window-bottom-divider-width", 0, 1, f_zero, ""),
    S!("window-right-divider-width", 0, 1, f_zero, ""),
    S!("window-divider-width-valid-p", 1, 1, f_false, ""),
    S!("window-lines-pixel-dimensions", 0, 7, f_zero, ""),
    S!("window-text-pixel-size", 0, 8, f_zero, ""),
    S!("window-absolute-pixel-position", 2, 2, f_posn_pair, ""),
    S!(
        "window-screen-lines",
        0,
        1,
        f_window_screen_lines,
        "Lines visible."
    ),
    S!("truncated-partial-width-window-p", 0, 1, f_false, ""),
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
    S!("window-pixel-left", 0, 1, f_zero, ""),
    S!("window-pixel-top", 0, 1, f_zero, ""),
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
    S!("window-safe-min-height", 0, 0, f_one, ""),
    S!("window-safe-min-width", 0, 0, f_two, ""),
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
    S!("frame-parent", 0, 1, f_nil, ""),
    S!("frame-ancestor-p", 2, 2, f_false, ""),
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
    S!("set-frame-position", 3, 3, f_nil, ""),
    S!("set-frame-size-and-position-pixelwise", 0, 2, f_nil, ""),
    S!("set-frame-window-state-change", 0, 2, f_nil, ""),
    S!("frame-window-state-change", 0, 1, f_nil, ""),
    S!("frame-after-make-frame", 2, 2, f_nil, ""),
    S!("frame--set-was-invisible", 1, 1, f_nil, ""),
    S!("frame--z-order-lessp", 2, 3, f_true2, ""),
    S!("frame--face-hash-table", 0, 1, f_nil, ""),
    S!("frame-font-cache", 0, 1, f_nil, ""),
    S!("next-frame", 0, 2, f_frame_self, "Next frame (only one)."),
    S!("previous-frame", 0, 2, f_frame_self, ""),
    S!("old-selected-frame", 0, 0, f_frame_self, ""),
    S!("old-selected-window", 0, 0, f_sel_window, ""),
    S!("raise-frame", 0, 1, f_nil, ""),
    S!("lower-frame", 0, 1, f_nil, ""),
    S!("make-frame-visible", 0, 1, f_make_frame_visible, ""),
    S!("make-frame-invisible", 0, 2, f_nil, ""),
    S!("iconify-frame", 0, 1, f_nil, ""),
    S!("x-focus-frame", 1, 2, f_x_focus_frame, ""),
    S!("redirect-frame-focus", 1, 2, f_nil, ""),
    S!("reconsider-frame-fonts", 0, 1, f_nil, ""),
    S!("frame--list-z-order", 0, 1, f_frame_list, ""),
    S!("tty-frame-list-z-order", 0, 1, f_frame_list, ""),
    S!("tty-frame-restack", 3, 3, f_nil, ""),
    S!("tty-frame-at", 2, 2, f_frame_self, ""),
    S!("tty-display-color-p", 0, 3, f_t, ""),
    S!(
        "tty-display-color-cells",
        0,
        2,
        f_tty_colors,
        "Number of tty colors."
    ),
    S!("tty-display-pixel-width", 0, 1, f_frame_width, ""),
    S!("tty-display-pixel-height", 0, 1, f_frame_height, ""),
    S!("tty-type", 0, 3, f_tty_type, "Terminal type name."),
    S!("tty-top-frame", 0, 1, f_frame_self, ""),
    S!("tty-no-underline", 0, 1, f_false, ""),
    S!("tty-suppress-bold-inverse-default-colors", 1, 1, f_nil, ""),
    S!("controlling-tty-p", 0, 1, f_nil, ""),
    S!("terminal-live-p", 1, 1, f_terminal_live_p, ""),
    S!("terminal-list", 0, 0, f_terminal_list, ""),
    S!("terminal-name", 0, 1, f_terminal_name, ""),
    S!("terminal-parameter", 2, 2, f_nil, ""),
    S!("terminal-parameters", 0, 1, f_nil, ""),
    S!("set-terminal-parameter", 3, 3, f_nil, ""),
    S!("terminal-id", 1, 1, f_terminal_id, ""),
    S!("delete-terminal", 1, 2, f_nil, ""),
    S!("suspend-tty", 0, 1, f_nil, ""),
    S!("resume-tty", 0, 1, f_nil, ""),
    S!("tty--output-buffer-size", 0, 1, f_zero, ""),
    S!("tty--set-output-buffer-size", 1, 1, f_nil, ""),
    S!("tty-find-type", 3, 3, f_nil, ""),
    S!("handle-select-window", 1, 1, f_nil, ""),
    S!("innermost-minibuffer-p", 0, 0, f_false, ""),
    S!("minibuffer-innermost-command-loop-p", 0, 1, f_false, ""),
    S!("display-supports-face-attributes-p", 1, 2, f_t, ""),
    S!("compute-motion-hints", 1, 1, f_nil, ""),
    S!("run-window-configuration-change-hook", 0, 1, f_nil, ""),
    S!("run-window-scroll-functions", 0, 1, f_nil, ""),
    S!("set-window-new-total", 2, 3, f_set_window_new_total, ""),
    S!("set-window-new-normal", 2, 3, f_nil, ""),
    S!("set-window-new-pixel", 2, 3, f_nil, ""),
    S!("set-window-combination-limit", 2, 2, f_nil, ""),
    S!("set-window-next-buffers", 2, 2, f_nil, ""),
    S!("set-window-prev-buffers", 2, 2, f_nil, ""),
    S!("force-window-update", 0, 1, f_nil, ""),
    S!("resize-mini-window-internal", 1, 1, f_nil, ""),
];

fn f_nil(_i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    Ok(Value::Nil)
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

fn f_two(_i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    Ok(Value::Int(2))
}

fn f_one_f(_i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    Ok(Value::Float(1.0))
}

fn f_zero4(_i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    Ok(Value::list(vec![
        Value::Int(0),
        Value::Int(0),
        Value::Int(0),
        Value::Nil,
    ]))
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

fn f_win_min_size(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let _ = win_of(i, &arg(&a, 0))?;
    let horiz = arg(&a, 2).truthy();
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
    let _ = a;
    match sel_frame(i) {
        Some(f) => {
            let ws: Vec<Value> = f
                .borrow()
                .windows
                .iter()
                .map(|w| Value::Window(w.clone()))
                .collect();
            Ok(Value::list(ws))
        }
        None => Ok(Value::Nil),
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

fn f_get_lru_window(i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    f_sel_window(i, vec![])
}

fn f_other_window(i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    // Next window after selected, or selected itself.
    match sel_frame(i) {
        Some(f) => {
            let fb = f.borrow();
            let sel_id = fb.selected.borrow().id;
            let n = fb.windows.len();
            for (k, w) in fb.windows.iter().enumerate() {
                if w.borrow().id == sel_id {
                    let target = &fb.windows[(k + 1) % n.max(1)];
                    return Ok(Value::Window(target.clone()));
                }
            }
            Ok(Value::Window(fb.selected.clone()))
        }
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
    Ok(Value::Nil)
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
    let w = win_of(i, &arg(&a, 0))?;
    let m = w.borrow().margins;
    Ok(Value::cons(
        Value::Int(m.0 as i128),
        Value::Int(m.1 as i128),
    ))
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
    Ok(Value::Float(w.borrow().height as f64))
}

// ---------- frames ----------

fn f_frame_root_window(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let f = frame_of(i, &arg(&a, 0))?;
    let fb = f.borrow();
    match fb.windows.first() {
        Some(w) => Ok(Value::Window(w.clone())),
        None => Ok(Value::Window(fb.selected.clone())),
    }
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

fn f_tty_type(i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    let _ = i;
    Ok(Value::Sym(i.intern("remacs-tty")))
}

fn f_terminal_live_p(_i: &mut Interp, a: Vec<Value>) -> EvalResult {
    Ok(Value::from_bool(matches!(
        &a[0],
        Value::Sym(_) | Value::Nil
    )))
}

fn f_terminal_list(i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    let tid = i.intern("tty");
    Ok(Value::list(vec![Value::Sym(tid)]))
}

fn f_terminal_name(_i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    Ok(Value::string("initial_terminal"))
}

fn f_terminal_id(i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    Ok(Value::Sym(i.intern("tty")))
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
