//! Additional window/frame primitives over the flat window model.
//! Many are stubs returning sensible tty defaults — the window tree
//! (internal windows, splits) is a later milestone.

use std::cell::RefCell;
use std::rc::Rc;

use crate::editor::{
    FrameRef, WindowRef, f_window_list, fmenu_shift, frame_any, frame_live, frame_of,
    frame_of_window, is_terminal, relay_children, sel_frame, sel_window, terminal_token, w_parent,
    w_siblings, win_any, win_err_live, win_err_valid, win_live, win_of, win_valid,
};
use crate::lisp::builtins::{S, arg, want_int};
use crate::lisp::error::{EvalResult, Flow};
use crate::lisp::sym;
use crate::lisp::value::{Subr, Value};
use crate::lisp::{Interp, SymId};

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
        f_window_parent,
        "Parent internal window."
    ),
    S!(
        "window-top-child",
        0,
        1,
        f_window_top_child,
        "First child of a vc window."
    ),
    S!("window-left-child", 0, 1, f_window_left_child, ""),
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
    S!(
        "window-normal-size",
        0,
        2,
        f_window_normal_size,
        "Normal size (1.0)."
    ),
    S!(
        "window-new-total",
        0,
        1,
        f_window_new_total,
        "Total lines after resize."
    ),
    S!("window-new-normal", 0, 1, f_window_new_normal, ""),
    S!("window-new-pixel", 0, 1, f_window_new_pixel, ""),
    S!(
        "window-old-point",
        0,
        1,
        f_window_old_point,
        "Old point of WINDOW."
    ),
    S!(
        "window-old-buffer",
        0,
        1,
        f_window_buffer1,
        "Buffer last shown."
    ),
    // GNU's `window-old-pixel-*'/`window-old-body-pixel-*' decode a
    // LIVE window (CHECK_LIVE_WINDOW) then report the pre-redisplay
    // sizes — 0 in batch.
    S!("window-old-pixel-width", 0, 1, f_zero_winvalid, ""),
    S!("window-old-pixel-height", 0, 1, f_zero_winvalid, ""),
    S!("window-old-body-pixel-width", 0, 1, f_zero_winlive, ""),
    S!("window-old-body-pixel-height", 0, 1, f_zero_winlive, ""),
    S!(
        "window-combination-p",
        0,
        2,
        f_window_combination_p,
        "t if WINDOW is internal."
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
        2,
        f_window_deletable_p,
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
    S!("window-max-delta", 0, 7, f_window_max_delta, ""),
    S!("window-min-delta", 0, 7, f_window_min_delta, ""),
    S!("window-sizable-p", 2, 5, f_window_sizable_p, ""),
    S!("window-size-fixed-p", 0, 3, f_window_size_fixed_p, ""),
    S!(
        "window-resize",
        2,
        5,
        f_window_resize,
        "Resize WINDOW by DELTA lines."
    ),
    // GNU Fwindow_resize_apply/-total: FRAME is decode_live_frame'd;
    // batch resize is a no-op returning nil.
    S!("window-resize-apply", 0, 2, f_window_resize_apply, ""),
    S!(
        "window-resize-apply-total",
        0,
        2,
        f_window_resize_apply_total,
        ""
    ),
    S!("window-resize-no-error", 2, 5, f_window_resize_no_error, ""),
    // GNU window.c Fwindow_list_1: WINDOW is decode_live_window'd.
    S!(
        "window-list-1",
        0,
        3,
        f_window_list1,
        "Windows in cyclic order."
    ),
    S!("window-bump-use-time", 0, 1, f_window_bump_use_time, ""),
    S!(
        "window-discard-buffer-from-window",
        2,
        3,
        f_window_discard_buffer_from_window,
        ""
    ),
    S!(
        "split-window-internal",
        4,
        5,
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
        4,
        f_get_lru_window,
        "Least recently used window."
    ),
    S!(
        "get-mru-window",
        0,
        4,
        f_get_mru_window,
        "Most recently used window."
    ),
    S!(
        "get-largest-window",
        0,
        4,
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
    S!(
        "coordinates-in-window-p",
        2,
        2,
        f_coordinates_in_window_p,
        ""
    ),
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
        f_zero_winlive,
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
        1,
        f_window_fringes,
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
    S!(
        "window-current-scroll-bars",
        0,
        1,
        f_window_current_scroll_bars,
        ""
    ),
    S!(
        "window-mode-line-height",
        0,
        1,
        f_window_mode_line_height,
        ""
    ),
    S!("window-header-line-height", 0, 1, f_window_header_line_height, ""),
    S!("window-tab-line-height", 0, 1, f_window_tab_line_height, ""),
    S!("window-bottom-divider-width", 0, 1, f_zero_winlive, ""),
    S!("window-right-divider-width", 0, 1, f_zero_winlive, ""),
    S!("window-divider-width-valid-p", 1, 1, f_false, ""),
    S!("window-lines-pixel-dimensions", 0, 6, f_window_live_nil, ""),
    S!("window-text-pixel-size", 0, 7, f_window_text_pixel_size, ""),
    S!("window-absolute-pixel-position", 0, 2, f_posn_pair, ""),
    S!(
        "window-screen-lines",
        0,
        0,
        f_window_screen_lines,
        "Lines visible."
    ),
    S!(
        "truncated-partial-width-window-p",
        0,
        1,
        f_truncated_partial_width,
        ""
    ),
    // --- pixel measurements (tty: 1 char = 1 col) ---
    S!(
        "window-text-height",
        0,
        2,
        f_window_text_height,
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
        f_window_pixel_edges4,
        "(l t r b) in pixels."
    ),
    S!("window-inside-pixel-edges", 0, 1, f_window_edges4, ""),
    S!(
        "window-absolute-pixel-edges",
        0,
        1,
        f_window_absolute_edges4,
        ""
    ),
    S!(
        "window-inside-absolute-pixel-edges",
        0,
        1,
        f_window_absolute_edges4,
        ""
    ),
    S!("window-body-pixel-edges", 0, 1, f_window_body_edges4, ""),
    S!(
        "window-absolute-body-pixel-edges",
        0,
        1,
        f_window_absolute_body_edges4,
        ""
    ),
    // GNU 31.1 has no `window-inside-absolute-body-pixel-edges'
    // (unbound upstream); keep it off the registry.
    // `window-safe-min-height'/`window-safe-min-width' are GNU variables.
    // --- frames ---
    S!(
        "frame-root-window",
        0,
        1,
        f_frame_root_window,
        "Root window of FRAME."
    ),
    S!(
        "frame-selected-window",
        0,
        1,
        f_frame_sel_window,
        "Selected window of FRAME."
    ),
    S!("frame-first-window", 0, 1, f_frame_first_window, ""),
    S!(
        "minibuffer-window",
        0,
        1,
        f_minibuffer_window,
        "The minibuffer window."
    ),
    S!("frame-parent", 0, 1, f_frame_parent, ""),
    S!("frame-ancestor-p", 2, 2, f_frame_ancestor_p, ""),
    S!(
        "frame-old-selected-window",
        0,
        1,
        f_frame_sel_window_live,
        ""
    ),
    S!("frame-root-frame", 0, 1, f_frame_root, ""),
    S!(
        "frame-initial-p",
        0,
        1,
        f_frame_initial_p,
        "t if FRAME is the initial frame."
    ),
    S!(
        "frame-focus",
        0,
        1,
        f_frame_focus,
        "Frame with input focus."
    ),
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
    S!("frame-char-width", 0, 1, f_one_framep, ""),
    S!("frame-char-height", 0, 1, f_one_framep, ""),
    S!("frame-fringe-width", 0, 1, f_zero_framep, ""),
    S!("frame-internal-border-width", 0, 1, f_zero_framep, ""),
    S!("frame-right-divider-width", 0, 1, f_zero_framep, ""),
    S!("frame-bottom-divider-width", 0, 1, f_zero_framep, ""),
    S!("frame-scroll-bar-width", 0, 1, f_zero_framep, ""),
    S!("frame-scroll-bar-height", 0, 1, f_zero_framep, ""),
    S!("frame-child-frame-border-width", 0, 1, f_zero_framep, ""),
    S!("frame-scale-factor", 0, 1, f_frame_scale_factor, ""),
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
    S!(
        "set-frame-size-and-position-pixelwise",
        5,
        6,
        f_set_frame_size_and_position_pixelwise,
        ""
    ),
    S!(
        "set-frame-window-state-change",
        0,
        2,
        f_set_frame_window_state_change,
        ""
    ),
    S!("frame-window-state-change", 0, 1, f_frame_window_state_change, ""),
    S!("frame-after-make-frame", 2, 2, f_frame_after_make_frame, ""),
    S!(
        "frame--set-was-invisible",
        2,
        2,
        f_frame_set_was_invisible,
        ""
    ),
    S!("frame--z-order-lessp", 2, 2, f_true2, ""),
    S!("frame--face-hash-table", 0, 1, f_frame_face_hash_table, ""),
    S!("frame-font-cache", 0, 1, f_nil_checked, ""),
    S!("next-frame", 0, 2, f_frame_self, "Next frame (only one)."),
    S!("previous-frame", 0, 2, f_frame_self, ""),
    S!("old-selected-frame", 0, 0, f_old_selected_frame, ""),
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
    S!("tty-frame-list-z-order", 0, 1, f_tty_frame_list_z_order, ""),
    S!("tty-frame-restack", 2, 3, f_tty_frame_restack, ""),
    S!("tty-frame-at", 2, 2, f_tty_frame_at, ""),
    S!("tty-frame-edges", 0, 2, f_tty_frame_edges, ""),
    S!("tty-frame-geometry", 0, 1, f_tty_frame_geometry, ""),
    S!("make-terminal-frame", 1, 1, f_make_terminal_frame, ""),
    S!(
        "redirect-debugging-output",
        1,
        2,
        f_redirect_debugging_output,
        ""
    ),
    S!("search-emacs-glossary", 0, 0, f_search_emacs_glossary, ""),
    S!("tty-display-color-p", 0, 1, f_tty_nil, ""),
    S!(
        "tty-display-color-cells",
        0,
        1,
        f_tty_colors,
        "Number of tty colors."
    ),
    // GNU tty_display_*: no arg decoding — always pixel width/height.
    S!("tty-display-pixel-width", 0, 1, f_tty_px_width, ""),
    S!("tty-display-pixel-height", 0, 1, f_tty_px_height, ""),
    S!("tty-type", 0, 1, f_tty_type, "Terminal type name."),
    S!("tty-top-frame", 0, 1, f_tty_top_frame, ""),
    S!("tty-no-underline", 0, 1, f_tty_false, ""),
    S!("tty-suppress-bold-inverse-default-colors", 1, 1, f_arg0, ""),
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
    S!(
        "tty--set-output-buffer-size",
        1,
        2,
        f_tty_set_output_buffer_size,
        ""
    ),
    S!("tty-find-type", 2, 2, f_tty_find_type, ""),
    S!("handle-select-window", 1, 1, f_t, ""),
    S!(
        "innermost-minibuffer-p",
        0,
        1,
        f_innermost_minibuffer_p,
        "t if BUFFER is the most nested active minibuffer."
    ),
    S!(
        "minibuffer-innermost-command-loop-p",
        0,
        1,
        f_minibuffer_innermost_command_loop_p,
        "t if BUFFER is a minibuffer at the current command loop level."
    ),
    S!("display-supports-face-attributes-p", 1, 2, f_nil, ""),
    S!(
        "run-window-configuration-change-hook",
        0,
        1,
        f_frame_live_nil_arg,
        ""
    ),
    S!(
        "run-window-scroll-functions",
        0,
        1,
        f_run_window_scroll_functions,
        ""
    ),
    S!("set-window-new-total", 2, 3, f_set_window_new_total, ""),
    S!("set-window-new-normal", 1, 2, f_set_window_new_normal, ""),
    S!("set-window-new-pixel", 2, 3, f_set_window_new_pixel, ""),
    S!(
        "set-window-combination-limit",
        2,
        2,
        f_set_window_combination_limit,
        ""
    ),
    S!(
        "set-window-next-buffers",
        2,
        2,
        f_set_window_next_buffers,
        ""
    ),
    S!(
        "set-window-prev-buffers",
        2,
        2,
        f_set_window_prev_buffers,
        ""
    ),
    S!("force-window-update", 0, 1, f_force_window_update, ""),
    S!(
        "resize-mini-window-internal",
        1,
        1,
        f_resize_mini_window_internal,
        ""
    ),
];

fn f_nil(_i: &mut Interp, _a: crate::lisp::Argv) -> EvalResult {
    Ok(Value::Nil)
}
/// nil result, but the optional arg is still decode_live_frame-checked
/// like GNU (font-family-list, frame-font-cache, ...).
fn f_nil_checked(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let _ = crate::editor::frame_live(i, &arg(&a, 0))?;
    Ok(Value::Nil)
}

fn f_other_window_for_scrolling(i: &mut Interp, _a: crate::lisp::Argv) -> EvalResult {
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

fn f_suspend_tty(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU batch: the initial terminal is not suspendable.
    let _ = want_terminal_live(i, &arg(&a, 0))?;
    Err(i.error("Attempt to suspend a non-text terminal device"))
}

fn f_resume_tty(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let _ = want_terminal_live(i, &arg(&a, 0))?;
    Err(i.error("Attempt to resume a non-text terminal device"))
}

fn f_delete_terminal(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
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

fn f_force_window_update(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
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

fn f_coordinates_in_window_p(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU Fcoordinates_in_window_p:
    // CHECK_LIVE_WINDOW (window arg 1) runs BEFORE the coordinates cons
    // shape check — `wta window-live-p' on any non-live window.
    let _w = win_live(i, &arg(&a, 1))?;
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

/// Shared buffer-arg resolution for the minibuffer predicates: nil →
/// current buffer; a string name resolves via `get_buffer' (nil when
/// absent — both predicates return nil for it); anything else is
/// CHECK_BUFFER'd.
fn minibuf_arg_id(i: &mut Interp, a: &crate::lisp::Argv) -> Result<usize, Flow> {
    match a.get(0) {
        None | Some(Value::Nil) => Ok(i.current_buffer),
        Some(v) => match i.buffer_id_of(v) {
            Some(id) => Ok(id),
            None if matches!(v, Value::Str(_)) => Ok(usize::MAX),
            None => Err(i.wrong_type_mut("bufferp", v)),
        },
    }
}

fn f_innermost_minibuffer_p(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU Finnermost_minibuffer_p: EQ (buffer, car (nthcdr
    // (minibuf_level, Vminibuffer_list))).
    let id = minibuf_arg_id(i, &a)?;
    let depth = i.minibuf_level.max(0) as usize;
    Ok(Value::from_bool(
        id != usize::MAX && i.minibuf_list.get(depth) == Some(&id),
    ))
}

fn f_minibuffer_innermost_command_loop_p(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU Fminibuffer_innermost_command_loop_p: an active minibuffer
    // (depth 1..=minibuf_level) whose Vcommand_loop_level_list entry
    // equals the current command_loop_level (remacs `recursion_depth').
    let id = minibuf_arg_id(i, &a)?;
    let hi = i.minibuf_level.max(0) as usize;
    for d in 1..=hi {
        if i.minibuf_list.get(d) == Some(&id) {
            let cll = i.minibuf_cll.get(d).copied().unwrap_or(0);
            return Ok(Value::from_bool(cll == i.recursion_depth));
        }
    }
    Ok(Value::Nil)
}

fn f_false(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    f_nil(i, a)
}

fn f_t(_i: &mut Interp, _a: crate::lisp::Argv) -> EvalResult {
    Ok(Value::t())
}

fn f_true2(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    f_t(i, a)
}

// frame-char-width/-height: GNU `decode_any_frame' on the optional
// arg — a non-frame signals framep before the constant.
fn f_one_framep(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    match arg(&a, 0) {
        Value::Nil | Value::Frame(_) => Ok(Value::Int(1)),
        other => Err(i.wrong_type_mut("framep", &other)),
    }
}

/// `window-scroll-bars' — window-live-p check; batch GNU reports
/// (WIDTH COLS VT HEIGHT LINES HT PERSISTENT) = (nil 0 t nil 0 t nil)
/// on a tty frame with no scroll bars.
fn f_window_scroll_bars(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
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

/// `window-current-scroll-bars' — GNU window.el `window-normalize-window
/// window t' → plain "N is not a live window"; tty batch is (nil . nil).
fn f_window_current_scroll_bars(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    match arg(&a, 0) {
        Value::Nil | Value::Window(_) => Ok(Value::cons(Value::Nil, Value::Nil)),
        ref other => Err(err_not_live_window(i, other)),
    }
}

fn f_sel_window(i: &mut Interp, _a: crate::lisp::Argv) -> EvalResult {
    match sel_window(i) {
        Some(w) => Ok(Value::Window(w)),
        None => Ok(Value::Nil),
    }
}

/// `window-valid-p': any window object — leaf or internal — that
/// hasn't been deleted (GNU's WINDOW_VALID_P).
fn f_window_valid_p(_i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    Ok(Value::from_bool(
        matches!(&a[0], Value::Window(w) if !w.borrow().dead),
    ))
}

/// `window-new-normal' — the `new_normal' slot stored by
/// `set-window-new-normal' (0 until first set in batch).
fn f_window_new_normal(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let w = win_of(i, &arg(&a, 0))?;
    Ok(w.borrow().new_normal.clone())
}

/// `window-fringes' — GNU returns (LEFT RIGHT OUTSIDE-MARGINS
/// PERSISTENT) with the live-window check; OUTSIDE-MARGINS is a
/// boolean, so nil (not 0) on tty where fringes don't exist.
fn f_window_fringes(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let _ = win_live(i, &arg(&a, 0))?;
    Ok(Value::list(vec![
        Value::Int(0),
        Value::Int(0),
        Value::Nil,
        Value::Nil,
    ]))
}

/// `window-deletable-p' — mirrors GNU window.el's decision tree on the
/// flat tty window model: a tab may absorb the deletion, the frame's
/// root is deletable only when the frame itself is, the minibuffer
/// window never is, and other leaves defer to
/// `window-deletable-functions' unless NO-RUN.
fn window_param(i: &Interp, w: &WindowRef, name: SymId) -> Value {
    let _ = i;
    crate::lisp::eval::plist_get(&w.borrow().params, name)
}

fn frame_deletable(i: &Interp, f: &FrameRef) -> bool {
    // GNU frame.el `frame-deletable-p', batch-terminal subset: a frame
    // is deletable when another live frame shares its terminal and no
    // other frame uses this frame's minibuffer.  Our frames all live
    // on one implicit tty terminal and `delete-before'/child frames
    // are absent, so the checks collapse to "a second live frame".
    let fid = f.borrow().id;
    let mut ok = false;
    for other in i.frames.iter() {
        let ob = other.borrow();
        if ob.id != fid && !ob.dead {
            ok = true;
            if let Some(mw) = &ob.minibuffer {
                let mid = mw.borrow().id;
                if frame_of_window(i, mw)
                    .map(|mf| mf.borrow().id == fid)
                    .unwrap_or(false)
                    || mid == fid
                {
                    return false;
                }
            }
        }
    }
    ok
}

fn f_window_deletable_p(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU's `decode_any_window' convention: "N is not a valid window".
    let w = win_err_valid(i, &arg(&a, 0))?;
    let no_run = arg(&a, 1).truthy();
    let frame = frame_of_window(i, &w).or_else(|| sel_frame(i));
    let Some(frame) = frame else {
        return Ok(Value::Nil);
    };
    let is_root = frame
        .borrow()
        .root
        .as_ref()
        .map(|r| Rc::ptr_eq(r, &w))
        .unwrap_or(false);
    // Tab path (first GNU clause, applies to the root too — bug#71386):
    // `(> (frame-parameter frame 'tab-bar-lines) 0)'.
    {
        let tbl_name = i.q("tab-bar-lines");
        let tbl = crate::lisp::eval::plist_get(&frame.borrow().params, tbl_name);
        let tbl = if matches!(tbl, Value::Int(n) if n > 0) {
            tbl
        } else {
            Value::Nil
        };
        if tbl.truthy() {
            // GNU calls `tab-bar-tabs-function' unconditionally; an
            // unset function there signals void-variable.
            let tf_sym = i.q("tab-bar-tabs-function");
            let tf = i.symbol_value(tf_sym);
            if matches!(tf, Value::Sym(s) if s == sym::UNBOUND) {
                return Err(i.signal_data(sym::VOID_VARIABLE, vec![Value::Sym(tf_sym)]));
            }
            let tabs = i.apply(&tf, vec![Value::Frame(frame.clone())])?;
            let ntabs = tabs.list_to_vec().map(|v| v.len()).unwrap_or(0);
            if ntabs > 1 {
                let qr = i.q("quit-restore");
                let tab = i.q("tab");
                let is_tab_qr = |win: &WindowRef| {
                    let v = window_param(i, win, qr);
                    matches!(&v, Value::Cons(c) if c.borrow().cdr.list_to_vec().map(|x| matches!(x.first(), Some(v2) if i.sym_is(v2, tab))).unwrap_or(false))
                        || i.sym_is(&v, tab)
                };
                if is_root || is_tab_qr(&w) {
                    let qwindow = i.q("window");
                    let qframe = i.q("frame");
                    let qsame = i.q("same");
                    let counted = frame
                        .borrow()
                        .windows
                        .iter()
                        .filter(|ww| {
                            let v = window_param(i, ww, qr);
                            if let Value::Cons(c) = &v {
                                let car = c.borrow().car.clone();
                                i.sym_is(&car, qwindow)
                                    || i.sym_is(&car, tab)
                                    || i.sym_is(&car, qframe)
                                    || i.sym_is(&car, qsame)
                            } else {
                                false
                            }
                        })
                        .count();
                    if counted < 2 {
                        return Ok(Value::Sym(tab));
                    }
                }
            }
        }
    }
    if !is_root && w.borrow().minibuffer {
        return Ok(Value::Nil);
    }
    if is_root {
        // Root: deletable only if the frame itself is and the hook
        // (with arg t) does not veto.
        if !frame_deletable(i, &frame) {
            return Ok(Value::Nil);
        }
        if !no_run {
            let ok = run_deletable_hook(i, &w, true)?;
            if !ok {
                return Ok(Value::Nil);
            }
        }
        return Ok(Value::Sym(i.q("frame")));
    }
    // Ordinary leaf: GNU requires non-main-window (our model has no
    // side windows, main == root, so this is always satisfied here).
    if !no_run {
        let ok = run_deletable_hook(i, &w, false)?;
        if !ok {
            return Ok(Value::Nil);
        }
    }
    Ok(Value::t())
}

/// `(run-hook-with-args-until-failure 'window-deletable-functions w flag)'
/// run in the window's buffer when the window is live, as GNU does.
fn run_deletable_hook(i: &mut Interp, w: &WindowRef, arg2: bool) -> Result<bool, Flow> {
    let hook_sym = i.q("window-deletable-functions");
    let saved = i.current_buffer;
    let live = !w.borrow().dead && w.borrow().horiz.is_none();
    if live {
        let bid = w.borrow().buffer;
        if i.buffers.get(bid).map(|b| b.borrow().live).unwrap_or(false) {
            i.set_current_buffer(bid);
        }
    }
    let hook_fn = Value::Sym(i.q("run-hook-with-args-until-failure"));
    let res = i.apply(
        &hook_fn,
        vec![
            Value::Sym(hook_sym),
            Value::Window(w.clone()),
            if arg2 { Value::t() } else { Value::Nil },
        ],
    );
    if live {
        i.set_current_buffer(saved);
    }
    res.map(|v| v.truthy())
}

/// GNU `decode_valid_window' for the `window-*-child'/sibling/
/// parent family — `wrong-type-argument window-valid-p' on
/// non-windows and dead windows (window.c, 31.x).
fn valid_window_arg(i: &mut Interp, v: &Value) -> Result<WindowRef, Flow> {
    win_valid(i, v)
}

/// `window-parent' — the internal combination window above WINDOW,
/// nil for the root and minibuffer windows.
fn f_window_parent(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let w = valid_window_arg(i, &arg(&a, 0))?;
    Ok(w_parent(&w).map(Value::Window).unwrap_or(Value::Nil))
}

/// `window-top-child': first child of a vc combination window;
/// nil on leaves and hc internals.
fn f_window_top_child(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let w = valid_window_arg(i, &arg(&a, 0))?;
    let wb = w.borrow();
    Ok(if wb.horiz == Some(false) {
        wb.children
            .first()
            .map(|k| Value::Window(k.clone()))
            .unwrap_or(Value::Nil)
    } else {
        Value::Nil
    })
}

/// `window-left-child': first child of an hc combination window.
fn f_window_left_child(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let w = valid_window_arg(i, &arg(&a, 0))?;
    let wb = w.borrow();
    Ok(if wb.horiz == Some(true) {
        wb.children
            .first()
            .map(|k| Value::Window(k.clone()))
            .unwrap_or(Value::Nil)
    } else {
        Value::Nil
    })
}

/// `window-combination-p': internal windows only.  GNU decodes with
/// decode_any_window — plain "N is not a valid window" error.
fn f_window_combination_p(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let w = win_err_valid(i, &arg(&a, 0))?;
    Ok(Value::from_bool(w.borrow().horiz.is_some()))
}

/// Sibling navigation: children of the same parent, or the frame's
/// parentless chain (root … minibuffer).  Never wraps.
pub(crate) fn siblings(i: &mut Interp, w: &Value, next: bool) -> EvalResult {
    let win = valid_window_arg(i, w)?;
    let Some(f) = frame_of_window(i, &win).or_else(|| sel_frame(i)) else {
        return Ok(Value::Nil);
    };
    let (group, pos) = w_siblings(&f.borrow(), &win);
    if pos == usize::MAX {
        return Ok(Value::Nil);
    }
    let idx = if next {
        pos.checked_add(1)
    } else {
        pos.checked_sub(1)
    };
    match idx.and_then(|j| group.get(j)) {
        Some(t) => Ok(Value::Window(t.clone())),
        _ => Ok(Value::Nil),
    }
}

fn f_next_sibling(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    siblings(i, &arg(&a, 0), true)
}

fn f_prev_sibling(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    siblings(i, &arg(&a, 0), false)
}

fn f_window_height2(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let w = win_of(i, &arg(&a, 0))?;
    Ok(Value::Int(w.borrow().height as i128))
}

/// GNU window.el: `window-text-height' = `(window-edges W t)' height —
/// the body, i.e. total minus the mode-line row (none on
/// minibuffer/internal windows).
fn f_window_text_height(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let w = win_of(i, &arg(&a, 0))?;
    let wb = w.borrow();
    let ml = usize::from(!wb.minibuffer && wb.horiz.is_none());
    Ok(Value::Int(wb.height.saturating_sub(ml) as i128))
}

fn f_window_new_total(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let w = win_of(i, &arg(&a, 0))?;
    Ok(Value::Int(w.borrow().new_total))
}

fn f_window_new_pixel(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let w = win_of(i, &arg(&a, 0))?;
    Ok(Value::Int(w.borrow().new_pixel))
}

fn f_window_height_px(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    f_window_height2(i, a)
}

fn f_window_width_px(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let w = win_of(i, &arg(&a, 0))?;
    Ok(Value::Int(w.borrow().width as i128))
}

fn f_window_left_px(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let w = win_of(i, &arg(&a, 0))?;
    Ok(Value::Int(w.borrow().left as i128))
}

fn f_window_top_px(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let w = win_of(i, &arg(&a, 0))?;
    Ok(Value::Int(w.borrow().top as i128))
}

/// `window-old-point' — GNU decodes a LIVE window (old_pointm).
fn f_window_old_point(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let w = win_live(i, &arg(&a, 0))?;
    Ok(Value::Int(w.borrow().old_point_position() as i128 + 1))
}

fn f_window_buffer1(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let w = win_any(i, &arg(&a, 0))?;
    let data = w.borrow();
    if data.horiz.is_some() || data.old_buffer.is_nil() {
        return Ok(Value::Nil);
    }
    let frame = data.change_frame.and_then(|id| i.frames.iter().find(|f| f.borrow().id == id));
    let stamp = data.change_clock.as_ref().map(|clock| clock.get())
        .or_else(|| frame.map(|frame| frame.borrow().change_stamp.get()));
    if stamp.is_some_and(|stamp| stamp != data.change_stamp) {
        Ok(Value::t())
    } else {
        Ok(data.old_buffer.clone())
    }
}

fn f_window_has_params(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let w = win_of(i, &arg(&a, 0))?;
    let p = w.borrow().params.clone();
    Ok(Value::from_bool(!p.is_nil()))
}

/// `window-min-size' — GNU's decode_any_window convention: nil →
/// selected, non-window → plain "N is not a valid window" error.
/// Safe minimum plus decorations, clamped to `window-min-height'/
/// `window-min-width' unless IGNORE suppresses them.
fn f_win_min_size(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
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
    let Value::Window(window) = wv else {
        return Err(err_not_valid_window(i, &wv));
    };
    Ok(Value::Int(crate::editor::window_minimum(
        i,
        &window,
        horiz,
        &ignore,
        arg(&a, 3).truthy(),
    )))
}

/// `window-size' — GNU's subr uses CHECK_VALID_WINDOW (typed
/// `window-valid-p' error) and returns the total size.
fn f_window_size(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
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
fn f_window_safe_min_size(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    match arg(&a, 0) {
        Value::Nil | Value::Window(_) => {}
        ref other => return Err(err_not_valid_window(i, other)),
    }
    let horiz = arg(&a, 1).truthy();
    let symbol = i.intern(if horiz {
        "window-safe-min-width"
    } else {
        "window-safe-min-height"
    });
    Ok(i.symbol_value(symbol))
}

fn f_window_resize(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    window_resize_impl(i, a, false)
}

fn f_window_resize_no_error(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    window_resize_impl(i, a, true)
}

/// Window's size on the split axis its parent combines along, or
/// None when the window can't be resized that way (root leaf,
/// minibuffer, or parent splits the other axis — GNU: the parent's
/// combination direction must match the resize direction).
fn resize_axis(w: &WindowRef, horizontal: bool) -> Option<(WindowRef, Vec<WindowRef>)> {
    let p = w_parent(w)?;
    let pb = p.borrow();
    if pb.horiz != Some(horizontal) {
        return None;
    }
    Some((p.clone(), pb.children.clone()))
}

/// Maximum grow capacity: total slack the siblings above their
/// minimum sizes.  Shrink capacity: W's own slack above its minimum.
fn resize_caps(i: &mut Interp, w: &WindowRef, horizontal: bool) -> (i128, i128) {
    let Some((_, kids)) = resize_axis(w, horizontal) else {
        return (0, 0);
    };
    let wmin = crate::lisp::builtins::misc::win_min_size(
        i,
        &Value::Window(w.clone()),
        horizontal,
        &Value::Nil,
    );
    let wsize = {
        let wb = w.borrow();
        (if horizontal { wb.width } else { wb.height }) as i128
    };
    let shrink = (wsize - wmin).max(0);
    let grow: i128 = kids
        .iter()
        .filter(|k| !Rc::ptr_eq(k, w) && !k.borrow().dead)
        .map(|k| {
            let ks = {
                let kb = k.borrow();
                (if horizontal { kb.width } else { kb.height }) as i128
            };
            let kmin = crate::lisp::builtins::misc::win_min_size(
                i,
                &Value::Window(k.clone()),
                horizontal,
                &Value::Nil,
            );
            (ks - kmin).max(0)
        })
        .sum();
    (grow, shrink)
}

/// `window-max-delta' — how far WINDOW can grow (batch: siblings'
/// slack; 0 for root/minibuffer).
fn f_window_max_delta(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU signature has a leading DELTA arg; window.el callers omit
    // it (window WINDOW HORIZONTAL ...).  Detect the shape.
    let (wv, horiz) = delta_args(i, &a)?;
    let w = match &wv {
        Value::Window(w) if !w.borrow().dead => w.clone(),
        _ => win_err_valid(i, &wv)?,
    };
    if w.borrow().minibuffer || w.borrow().horiz.is_some() {
        return Ok(Value::Int(0));
    }
    Ok(Value::Int(resize_caps(i, &w, horiz).0))
}

/// `window-min-delta' — how far WINDOW can shrink (positive amount).
fn f_window_min_delta(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let (wv, horiz) = delta_args(i, &a)?;
    let w = match &wv {
        Value::Window(w) if !w.borrow().dead => w.clone(),
        _ => win_err_valid(i, &wv)?,
    };
    if w.borrow().minibuffer || w.borrow().horiz.is_some() {
        return Ok(Value::Int(0));
    }
    Ok(Value::Int(resize_caps(i, &w, horiz).1))
}

/// Parse the ambiguous (WINDOW DELTA ...)/(DELTA WINDOW ...) arg
/// shapes `window-max-delta' sees from window.el vs direct calls.
fn delta_args(_i: &mut Interp, a: &[Value]) -> Result<(Value, bool), Flow> {
    let first = arg(a, 0);
    if matches!(first, Value::Int(_)) {
        let w = arg(a, 1);
        let horiz = a.get(3).map(|v| v.truthy()).unwrap_or(false);
        Ok((w, horiz))
    } else {
        let horiz = a.get(1).map(|v| v.truthy()).unwrap_or(false);
        Ok((first, horiz))
    }
}

fn window_resize_impl(i: &mut Interp, a: crate::lisp::Argv, no_error: bool) -> EvalResult {
    // GNU Fwindow_resize uses decode_any_window: bogus objects and
    // dead windows get the plain "N is not a valid window" error.
    let w = win_err_valid(i, &arg(&a, 0))?;
    let delta = want_int(i, &a[1])?;
    let horizontal = arg(&a, 2).truthy();
    let fail = |i: &mut Interp, msg: &str| -> EvalResult {
        if no_error {
            Ok(Value::Nil)
        } else {
            Err(i.error(msg))
        }
    };
    {
        let wb = w.borrow();
        if wb.minibuffer {
            return fail(i, "Cannot resize the minibuffer window");
        }
    }
    // GNU: resizing a frame's root window is invalid.
    let is_root = w_parent(&w).is_none();
    if is_root {
        return fail(i, "Cannot resize the root window of a frame");
    }
    if delta == 0 {
        return fail(i, "Invalid window size");
    }
    let Some((p, kids)) = resize_axis(&w, horizontal) else {
        return fail(i, "Invalid window size");
    };
    let (grow, shrink) = resize_caps(i, &w, horizontal);
    let ok = if delta > 0 {
        delta <= grow
    } else {
        -delta <= shrink
    };
    if !ok {
        return fail(i, "Invalid window size");
    }
    // Apply: W takes DELTA; siblings give up (grow) or receive
    // (shrink) space proportional to their current sizes.
    {
        let mut wb = w.borrow_mut();
        if horizontal {
            wb.width = (wb.width as i128 + delta).max(1) as usize;
        } else {
            wb.height = (wb.height as i128 + delta).max(1) as usize;
        }
    }
    let pool = if delta > 0 {
        grow
    } else {
        // Shrinking W releases space siblings soak up — proportional
        // to their current sizes (a sole sibling takes it all).
        kids.iter()
            .filter(|k| !Rc::ptr_eq(k, &w) && !k.borrow().dead)
            .map(|k| {
                let kb = k.borrow();
                (if horizontal { kb.width } else { kb.height }) as i128
            })
            .sum()
    };
    let mut need = delta.abs();
    let last = kids
        .iter()
        .filter(|k| !Rc::ptr_eq(k, &w) && !k.borrow().dead)
        .last()
        .cloned();
    for k in &kids {
        if Rc::ptr_eq(k, &w) || k.borrow().dead {
            continue;
        }
        let ksize = {
            let kb = k.borrow();
            (if horizontal { kb.width } else { kb.height }) as i128
        };
        let mut delta_k = if pool > 0 {
            if delta > 0 {
                // W grows: shrink K by its share of NEED (≤ its slack).
                let kmin = crate::lisp::builtins::misc::win_min_size(
                    i,
                    &Value::Window(k.clone()),
                    horizontal,
                    &Value::Nil,
                );
                let slack = (ksize - kmin).max(0);
                -(need * slack / pool).min(slack).min(need)
            } else {
                // W shrinks: grow K by its share of NEED.
                (need * ksize / pool).min(need)
            }
        } else {
            0
        };
        // The last sibling absorbs any rounding remainder so the
        // parent combination stays exactly tiled.
        if last.as_ref().is_some_and(|l| Rc::ptr_eq(l, k)) {
            delta_k = if delta > 0 { -need } else { need };
        }
        need -= delta_k.abs();
        {
            let mut kb = k.borrow_mut();
            if horizontal {
                kb.width = (kb.width as i128 + delta_k).max(1) as usize;
            } else {
                kb.height = (kb.height as i128 + delta_k).max(1) as usize;
            }
        }
    }
    // GNU updates normal sizes after a successful resize.  Keep each
    // child's share relative to its parent's normal size, using the new
    // geometry so later space redistribution honors this resize.
    let (parent_extent, parent_normal) = {
        let pb = p.borrow();
        if horizontal {
            (pb.width as f64, pb.normal_size.1)
        } else {
            (pb.height as f64, pb.normal_size.0)
        }
    };
    if parent_extent > 0.0 {
        for child in &kids {
            let mut child = child.borrow_mut();
            if child.dead {
                continue;
            }
            let extent = if horizontal {
                child.width as f64
            } else {
                child.height as f64
            };
            if horizontal {
                child.normal_size.1 = parent_normal * extent / parent_extent;
            } else {
                child.normal_size.0 = parent_normal * extent / parent_extent;
            }
        }
    }
    relay_children(&p);
    if no_error {
        return Ok(Value::Nil);
    }
    Ok(Value::t())
}

fn f_window_list1(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // (window-list-1 WINDOW MINIBUF ALL-FRAMES): GNU
    // decode_live_window's WINDOW before building the cyclic list.
    let _ = win_live(i, &arg(&a, 0))?;
    f_window_list(i, vec![Value::Nil, arg(&a, 1)].into())
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
                    && (dedicated_ok || !wb.dedicated.truthy())
                    && !(not_selected && sel.as_ref().map_or(false, |s| s.borrow().id == wb.id))
            })
            .cloned()
            .collect(),
        None => Vec::new(),
    }
}

fn f_split_window_internal(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU Fsplit_window_internal decodes
    // WINDOW via decode_valid_window FIRST — nil → selected window,
    // non-window → `wta window-valid-p'.  Then SIZE must be a fixnum
    // (nil/non-fixnum → `fixnump' wta) and on a tty frame every
    // explicit SIZE fails — positive in `wset_new_size' ("Resizing old
    // window failed"), non-positive in the minimum-size check ("Size
    // of new window too small (after split)") — verified on GNU
    // -Q --batch for right/below and 0/±N.  All remacs frames are
    // tty-equivalent, so the tty branch applies unconditionally.
    // (This strictness is only on the -internal worker: the public
    // `split-window' computes its own size before delegating.)
    let _w = win_valid(i, &a[0])?;
    match &a[1] {
        Value::Int(s) => {
            if *s > 0 {
                return Err(i.error("Resizing old window failed".to_string()));
            }
            return Err(i.error("Size of new window too small (after split)".to_string()));
        }
        other => return Err(i.wrong_type_mut("fixnump", other)),
    }
}

fn f_delete_window_internal(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU Fdelete_window_internal uses decode_window —
    // plain `windowp', not valid-p.
    let w = match &a[0] {
        Value::Window(w) => w.clone(),
        other => return Err(i.wrong_type_mut("windowp", other)),
    };
    let wid = w.borrow().id;
    for f in &i.frames {
        let mut fb = f.borrow_mut();
        if fb.windows.len() <= 1 {
            continue;
        }
        if let Some(pos) = fb.windows.iter().position(|x| x.borrow().id == wid) {
            let removed = fb.windows.remove(pos);
            removed.borrow_mut().dead = true;
            // Unlink from the combination parent.
            if let Some(p) = w_parent(&removed) {
                p.borrow_mut().children.retain(|k| !Rc::ptr_eq(k, &removed));
                removed.borrow_mut().parent = None;
            }
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

fn f_get_lru_window(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU Fget_lru_window: lowest use-time among the candidates.
    match some_window_candidates(i, &a)
        .iter()
        .min_by_key(|w| w.borrow().use_time)
    {
        Some(w) => Ok(Value::Window(w.clone())),
        None => Ok(Value::Nil),
    }
}

fn f_get_mru_window(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
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

fn f_get_largest_window(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU Fget_largest_window: maximum pixel area (ties keep the
    // first, i.e. the least recently used among equals).
    match some_window_candidates(i, &a).iter().rev().max_by_key(|w| {
        let wb = w.borrow();
        wb.width * wb.height
    }) {
        Some(w) => Ok(Value::Window(w.clone())),
        None => Ok(Value::Nil),
    }
}

fn f_posn_at_point(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU decodes WINDOW (arg2)
    // via decode_live_window even when the batch result is nil.
    if a.len() > 1 {
        let _ = win_live(i, &a[1])?;
    }
    // GNU Fposn_at_point CHECK_NUMBER_COERCE_MARKERs POS before
    // consulting the display — a cons errors even in batch.
    let pos = match a.get(0) {
        None | Some(Value::Nil) => i
            .current_buffer_ref()
            .map(|b| b.borrow().point() as i128 + 1)
            .unwrap_or(1),
        Some(Value::Int(n)) => *n,
        Some(Value::Marker(m)) => m.borrow().position as i128 + 1,
        Some(other) => return Err(i.wrong_type_mut("integer-or-marker-p", other)),
    };
    if i.noninteractive {
        // GNU: no window exists in batch mode, so the position isn't
        // visible anywhere and the whole posn is nil.
        return Ok(Value::Nil);
    }
    // GNU: point not in any window's text area → the minimal posn
    // `(WINDOW POS (0 . 0) TIMESTAMP)' that its C `event-start'/`event-end'
    // fallback also yields in batch — car MUST be the live window, else
    // callers like `describe-key' trip `posn-set-point''s
    // "Position not in text area of window" check.
    let win = match sel_window(i) {
        Some(w) => Value::Window(w),
        None => return Ok(Value::Nil),
    };
    Ok(Value::list(vec![
        win,
        Value::Int(pos),
        Value::cons(Value::Int(0), Value::Int(0)),
        Value::Int(0),
    ]))
}

fn f_posn_at_xy(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU Fposn_at_x_y: CHECK_FIXNUM X, CHECK_FIXNUM Y
    // (`fixnump' — floats and markers reject), then the optional FRAME
    // via `frame-live-p'; WHOLE is unused in batch.
    let x = crate::lisp::builtins::want_fixnum(i, &a[0])?;
    let y = crate::lisp::builtins::want_fixnum(i, &a[1])?;
    let frame = match a.get(2) {
        None | Some(Value::Nil) => sel_frame(i),
        Some(v) => Some(crate::editor::frame_live(i, v)?),
    };
    let f = match frame {
        Some(f) => f,
        None => return Ok(Value::Nil),
    };
    // Outside the frame's text area GNU reports the short form
    // `(FRAME nil (X . Y) 0)'.
    let (fw, fh) = {
        let fb = f.borrow();
        (fb.width as i128, fb.height as i128)
    };
    if x < 0 || y < 0 || x >= fw || y >= fh {
        return Ok(Value::list(vec![
            Value::Frame(f),
            Value::Nil,
            Value::cons(Value::Int(x), Value::Int(y)),
            Value::Int(0),
        ]));
    }
    // Find the window containing (x, y).
    let fb = f.borrow();
    let mut found: Option<WindowRef> = None;
    for w in &fb.windows {
        let ww = w.borrow();
        if ww.minibuffer {
            continue;
        }
        if x >= ww.left as i128
            && x < (ww.left + ww.width) as i128
            && y >= ww.top as i128
            && y < (ww.top + ww.height) as i128
        {
            found = Some(w.clone());
            break;
        }
    }
    // Minibuffer / echo row: GNU reports the minibuffer window with
    // position 1.
    let w = match found {
        Some(w) => w,
        None => {
            let mb = fb
                .minibuffer
                .clone()
                .or_else(|| fb.windows.iter().find(|w| w.borrow().minibuffer).cloned())
                .or_else(|| fb.windows.first().cloned());
            let w = match mb {
                Some(w) => w,
                None => return Ok(Value::Nil),
            };
            return Ok(Value::list(vec![
                Value::Window(w),
                Value::Int(1),
                Value::cons(Value::Int(x), Value::Int(0)),
                Value::Int(0),
                Value::Nil,
                Value::Int(1),
                Value::cons(Value::Int(x), Value::Int(0)),
                Value::Nil,
                Value::cons(Value::Int(x), Value::Int(0)),
                Value::cons(Value::Int(0), Value::Int(0)),
            ]));
        }
    };
    drop(fb);
    let (wtop, wh) = {
        let ww = w.borrow();
        (ww.top as i128, ww.height as i128)
    };
    // The window's last row is its mode line: GNU reports the
    // `mode-line' area symbol with nil position fields.
    if y == wtop + wh - 1 {
        let ml = i.q("mode-line");
        return Ok(Value::list(vec![
            Value::Window(w),
            Value::Sym(ml),
            Value::cons(Value::Int(x), Value::Int(y)),
            Value::Int(0),
            Value::Nil,
            Value::Nil,
            Value::cons(Value::Int(x), Value::Int(y)),
            Value::Nil,
            Value::cons(Value::Int(x), Value::Int(y)),
            Value::cons(Value::Int(0), Value::Int(0)),
        ]));
    }
    let row = (y - wtop).max(0);
    let buf = w.borrow().buffer;
    let b = i
        .buffers
        .get(buf)
        .ok_or_else(|| i.error("No current buffer"))?;
    let bb = b.borrow();
    let nlines = bb.text.line_of_pos(bb.text.len()) + 1;
    let row_c = (row as usize).min(nlines - 1);
    let y_oob = row as usize >= nlines;
    let bol = bb.text.line_start(row_c);
    let eol = bb.text.line_end(bol);
    let len = eol - bol;
    let xu = x.max(0) as usize;
    // GNU's tty iterator stops at the first char at or right of X —
    // past EOL it lands on the newline/EOL char, and a Y beyond the
    // buffer's last line lands at the last line's EOL.
    let pos0 = if y_oob || xu >= len {
        bol + len
    } else {
        bol + xu
    };
    let col_row_x = if y_oob && xu < len {
        len as i128
    } else if xu > len && len > 0 {
        x - 1
    } else {
        x
    };
    let dxx = if xu > len {
        if len == 0 { x } else { x - len as i128 }
    } else if y_oob && xu < len {
        x - len as i128
    } else {
        0
    };
    let dyy = if y_oob { y - row_c as i128 } else { 0 };
    drop(bb);
    Ok(Value::list(vec![
        Value::Window(w),
        Value::Int(pos0 as i128 + 1),
        Value::cons(Value::Int(x), Value::Int(y)),
        Value::Int(0),
        Value::Nil,
        Value::Int(pos0 as i128 + 1),
        Value::cons(Value::Int(col_row_x), Value::Int(row_c as i128)),
        Value::Nil,
        Value::cons(Value::Int(dxx), Value::Int(dyy)),
        Value::cons(Value::Int(0), Value::Int(0)),
    ]))
}

fn f_posn_pair(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // `window-absolute-pixel-position' (POSITION WINDOW): GNU batch
    // returns nil (no usable glyph matrix); the WINDOW arg decodes with
    // `decode_any_window' ("N is not a live window"), POSITION is
    // `integer-or-marker-p'.
    let wv = match arg(&a, 1) {
        Value::Nil => sel_window(i).map(Value::Window).unwrap_or(Value::Nil),
        v => v.clone(),
    };
    let _ = win_err_live(i, &wv)?;
    if let Some(v) = a.first() {
        if !v.is_nil() {
            let _ = crate::lisp::builtins::arith::want_int_or_marker(i, v)?;
        }
    }
    Ok(Value::Nil)
}

fn f_set_window_vscroll(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let _ = win_live(i, &arg(&a, 0))?;
    // A tty window has no vscroll; GNU reports 0.
    Ok(Value::Int(0))
}

fn f_scroll_left(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU Fscroll_left: changes hscroll only — point does NOT move;
    // `suspend_auto_hscroll' keeps auto-hscroll from undoing it until
    // point changes (xdisp.c window-point vs old_pointm check).
    let w = sel_window(i).ok_or_else(|| i.error("No window"))?;
    let default = w.borrow().width.saturating_sub(2) as i128;
    let n = crate::editor::scroll_arg(i, &arg(&a, 0), default)?;
    let min = arg(&a, 1).truthy();
    let mut wb = w.borrow_mut();
    // GNU Fscroll_left: set_window_hscroll(w, w->hscroll + arg) with
    // clip_to_bounds(0, ·, MOST_POSITIVE_FIXNUM) — negative arg scrolls
    // right.
    let new = (wb.hscroll as i128 + n).clamp(0, (1 << 61) - 1) as usize;
    wb.hscroll = new;
    if min {
        wb.min_hscroll = new;
    }
    wb.suspend_hscroll = true;
    Ok(Value::Int(new as i128))
}

fn f_scroll_right(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let w = sel_window(i).ok_or_else(|| i.error("No window"))?;
    let default = w.borrow().width.saturating_sub(2) as i128;
    let n = crate::editor::scroll_arg(i, &arg(&a, 0), default)?;
    let min = arg(&a, 1).truthy();
    let mut wb = w.borrow_mut();
    // GNU Fscroll_right: set_window_hscroll(w, w->hscroll - arg) —
    // negative arg scrolls LEFT (hscroll grows); clip to fixnum.
    let new = (wb.hscroll as i128 - n).clamp(0, (1 << 61) - 1) as usize;
    wb.hscroll = new;
    if min {
        wb.min_hscroll = new;
    }
    wb.suspend_hscroll = true;
    Ok(Value::Int(new as i128))
}

fn f_window_margins(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let w = match &arg(&a, 0) {
        Value::Nil => sel_window(i).ok_or_else(|| i.error("No window"))?,
        Value::Window(w) if !w.borrow().dead => w.clone(),
        other => return Err(i.wrong_type_mut("window-live-p", other)),
    };
    let m = w.borrow().margins;
    // GNU always returns (LEFT . RIGHT), nil sides for zero margins.
    let lv = if m.0 == 0 {
        Value::Nil
    } else {
        Value::Int(m.0 as i128)
    };
    let rv = if m.1 == 0 {
        Value::Nil
    } else {
        Value::Int(m.1 as i128)
    };
    Ok(Value::cons(lv, rv))
}

/// Pixel-edge helpers shared by `window-inside-pixel-edges' /
/// `window-absolute-pixel-edges' — GNU's batch inside geometry is the
/// pre-split row space: subtract the menu-bar shift and the mode
/// line (leaves only).
fn inside_edges_of(i: &mut Interp, v: &Value) -> Result<(i128, i128, i128, i128), Flow> {
    let w = win_of(i, v)?;
    let shift = frame_of_window(i, &w)
        .map(|f| fmenu_shift(&f.borrow()))
        .unwrap_or(0);
    let wb = w.borrow();
    let ml = usize::from(!wb.minibuffer && wb.horiz.is_none());
    Ok((
        wb.left as i128,
        wb.top.saturating_sub(shift) as i128,
        (wb.left + wb.width) as i128,
        (wb.top + wb.height).saturating_sub(shift + ml) as i128,
    ))
}
/// `window-pixel-edges': GNU's pixel edges are the *pre-split* row
/// space (menu-bar shift undone) but still include the mode line —
/// top equals `window-edges' minus the shift, bottom likewise.
/// Verified on GNU -Q post-split: sel=(0 0 80 12) vs edges (0 1 80 13).
fn pixel_edges_of(i: &mut Interp, v: &Value) -> Result<(i128, i128, i128, i128), Flow> {
    let w = win_of(i, v)?;
    let shift = frame_of_window(i, &w)
        .map(|f| fmenu_shift(&f.borrow()))
        .unwrap_or(0);
    let wb = w.borrow();
    Ok((
        wb.left as i128,
        wb.top.saturating_sub(shift) as i128,
        (wb.left + wb.width) as i128,
        (wb.top + wb.height).saturating_sub(shift) as i128,
    ))
}

fn f_window_pixel_edges4(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let _ = crate::editor::win_err_valid(i, &arg(&a, 0))?;
    let (l, t, r, b) = pixel_edges_of(i, &arg(&a, 0))?;
    Ok(Value::list(vec![
        Value::Int(l),
        Value::Int(t),
        Value::Int(r),
        Value::Int(b),
    ]))
}

fn f_window_edges4(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU `window-pixel-edges'/`window-inside-pixel-edges': the arg is
    // decode_live_window — plain "X is not a live window" error.
    let w = crate::editor::win_err_live(i, &arg(&a, 0))?;
    let (l, t, r, b) = inside_edges_of(i, &Value::Window(w))?;
    Ok(Value::list(vec![
        Value::Int(l),
        Value::Int(t),
        Value::Int(r),
        Value::Int(b),
    ]))
}

/// `window-absolute-pixel-edges'/`window-inside-absolute-pixel-edges':
/// GNU decodes a valid window ("X is not a valid window") then calls
/// `frame-edges', which is nil on a tty — the list arithmetic dies
/// with `wrong-type-argument number-or-marker-p nil'.
fn f_window_absolute_edges4(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let _ = crate::editor::win_err_valid(i, &arg(&a, 0))?;
    Err(i.wrong_type_mut("number-or-marker-p", &Value::Nil))
}

/// `window-absolute-body-pixel-edges': live-window decode ("X is not
/// a live window"), then the same `frame-edges' → nil → wta path.
fn f_window_absolute_body_edges4(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let _ = crate::editor::win_err_live(i, &arg(&a, 0))?;
    Err(i.wrong_type_mut("number-or-marker-p", &Value::Nil))
}

fn f_window_body_edges4(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // Body excludes the mode line.
    // GNU: `window-inside-edges' DEFALIASES to `window-body-edges'
    // (window.el:3920) — same pre-shift row space and the same
    // mode-line exclusion as `inside_edges_of', just through the
    // live-window decode ("X is not a live window").
    let w = crate::editor::win_err_live(i, &arg(&a, 0))?;
    let (l, t, r, b) = inside_edges_of(i, &Value::Window(w))?;
    Ok(Value::list(vec![
        Value::Int(l),
        Value::Int(t),
        Value::Int(r),
        Value::Int(b),
    ]))
}

fn f_window_screen_lines(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let w = win_of(i, &arg(&a, 0))?;
    // GNU Fwindow_screen_lines excludes the mode line (1 row on
    // non-minibuffer windows) from the total.
    let wb = w.borrow();
    let h = if wb.minibuffer {
        wb.height
    } else {
        wb.height.saturating_sub(1)
    };
    Ok(Value::float(h as f64))
}

// ---------- frames ----------

fn f_frame_root_window(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let f = crate::editor::frame_or_win(i, &arg(&a, 0))?;
    // GNU: the lone live window IS the root; once split the root is
    // the internal combination window maintained by split/delete.
    match &f.borrow().root {
        Some(r) => Ok(Value::Window(r.clone())),
        None => Ok(Value::Nil),
    }
}

fn f_frame_sel_window(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let f = crate::editor::frame_or_win(i, &arg(&a, 0))?;
    Ok(Value::Window(f.borrow().selected.clone()))
}
/// GNU `frame-old-selected-window' — `old_selected_window' slot:
/// the window selected before the last `do_switch_frame'; nil for
/// a frame that has never been switched away from.
fn f_frame_sel_window_live(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let f = crate::editor::frame_live(i, &arg(&a, 0))?;
    let fb = f.borrow();
    Ok(fb
        .old_selected
        .as_ref()
        .map(|w| Value::Window(w.clone()))
        .unwrap_or(Value::Nil))
}

/// Leftmost/topmost live leaf of W's subtree.
fn first_leaf(w: &WindowRef) -> WindowRef {
    let first = {
        let wb = w.borrow();
        if wb.horiz.is_some() {
            wb.children.first().cloned()
        } else {
            None
        }
    };
    match first {
        Some(k) => first_leaf(&k),
        None => w.clone(),
    }
}

fn f_frame_first_window(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU `frame_first_window' walks the window tree to the leftmost/
    // topmost leaf.
    let f = crate::editor::frame_or_win(i, &arg(&a, 0))?;
    match f.borrow().root.as_ref().map(first_leaf) {
        Some(w) => Ok(Value::Window(w)),
        None => Ok(Value::Nil),
    }
}

fn f_minibuffer_window(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let f = crate::editor::frame_live(i, &arg(&a, 0))?;
    match &f.borrow().minibuffer {
        Some(w) => Ok(Value::Window(w.clone())),
        None => Ok(Value::Nil),
    }
}

fn f_frame_self(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU next-frame/previous-frame:
    // decode_live_frame — non-frame args signal frame-live-p.
    let f = crate::editor::frame_live(i, &arg(&a, 0))?;
    Ok(Value::Frame(f))
}
/// GNU Fframe_focus: FRAME_FOCUS_FRAME — the redirect target; nil when
/// focus isn't redirected (always nil for a non-GUI frame).
fn f_frame_focus(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let _ = crate::editor::frame_live(i, &arg(&a, 0))?;
    Ok(Value::Nil)
}
/// run-window-configuration-change-hook + friends: the optional
/// frame decodes via decode_live_frame; result is nil (no hook
/// machinery needed in batch).
fn f_frame_live_nil_arg(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let _ = crate::editor::frame_live(i, &arg(&a, 0))?;
    Ok(Value::Nil)
}
/// tty-frame-at: GNU returns nil (no tty frames in batch); coords
/// are numbers but a non-live terminal arg errors on
/// terminal-live-p — args here are row/col integers, harmless.
fn f_tty_frame_at(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let _ = (i, a);
    Ok(Value::Nil)
}
/// tty-top-frame: terminal-live-p check on the optional TERMINAL.
fn f_tty_top_frame(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    match arg(&a, 0) {
        Value::Nil | Value::Frame(_) => Ok(Value::Nil),
        w if is_terminal(i, &w) => Ok(Value::Nil),
        other => Err(i.wrong_type_mut("terminal-live-p", &other)),
    }
}
/// old-selected-frame: no args, selected frame.
fn f_old_selected_frame(i: &mut Interp, _a: crate::lisp::Argv) -> EvalResult {
    match sel_frame(i) {
        Some(f) => Ok(Value::Frame(f)),
        None => Ok(Value::Nil),
    }
}
/// GNU frame-root-frame: decode_live_frame, propagating errors.
fn f_frame_root(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let f = crate::editor::frame_live(i, &arg(&a, 0))?;
    Ok(Value::Frame(f))
}

fn f_frame_initial_p(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    match frame_of(i, &arg(&a, 0)) {
        // GNU compares against Vframe_initial_frame — the frame created
        // at startup, i.frames[0] in our model.  A non-frame arg just
        // compares false -> nil (no error).
        Ok(f) => Ok(Value::from_bool(
            i.frames
                .first()
                .map(|sf| Rc::ptr_eq(sf, &f))
                .unwrap_or(false),
        )),
        Err(_) => Ok(Value::Nil),
    }
}

fn f_frame_id(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let f = crate::editor::frame_live(i, &arg(&a, 0))?;
    Ok(Value::Int(f.borrow().id as i128))
}

fn f_frame_width(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let f = crate::editor::frame_of(i, &arg(&a, 0))?;
    Ok(Value::Int(f.borrow().width as i128))
}

fn f_frame_height(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let f = crate::editor::frame_of(i, &arg(&a, 0))?;
    Ok(Value::Int(f.borrow().height as i128))
}

fn f_set_frame_height(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let f = crate::editor::frame_live(i, &arg(&a, 0))?;
    let h = want_int(i, &a[1])?;
    f.borrow_mut().height = h.max(1) as usize;
    Ok(Value::Nil)
}

fn f_set_frame_width(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let f = crate::editor::frame_live(i, &arg(&a, 0))?;
    let w = want_int(i, &a[1])?;
    f.borrow_mut().width = w.max(1) as usize;
    Ok(Value::Nil)
}

fn f_set_frame_size(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let f = crate::editor::frame_live(i, &arg(&a, 0))?;
    let w = want_int(i, &a[1])?;
    let h = want_int(i, &a[2])?;
    let mut fb = f.borrow_mut();
    fb.width = w.max(1) as usize;
    fb.height = h.max(1) as usize;
    Ok(Value::Nil)
}

fn f_frame_list(i: &mut Interp, _a: crate::lisp::Argv) -> EvalResult {
    let fs: Vec<Value> = i.frames.iter().map(|f| Value::Frame(f.clone())).collect();
    Ok(Value::list(fs))
}

fn f_tty_colors(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU batch terminal: no color support → 0 cells.
    let _ = want_terminal_live(i, &arg(&a, 0))?;
    Ok(Value::Int(0))
}

fn f_arg0(_i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    Ok(a[0].clone())
}

// GNU tty_display_*: NO terminal arg decoding at all — any arg is
// ignored and the pixel size constants are returned.
fn f_tty_px_width(_i: &mut Interp, _a: crate::lisp::Argv) -> EvalResult {
    Ok(Value::Int(80))
}
fn f_tty_px_height(_i: &mut Interp, _a: crate::lisp::Argv) -> EvalResult {
    Ok(Value::Int(25))
}

fn f_tty_type(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU: nil on a tty batch terminal; TERMINAL must be terminal-live-p.
    let _ = want_terminal_live(i, &arg(&a, 0))?;
    Ok(Value::Nil)
}

/// nil or the terminal record; anything else is a terminal-live-p error.
fn want_terminal_live(i: &mut Interp, v: &Value) -> EvalResult {
    match v {
        Value::Nil => Ok(Value::Nil),
        // GNU accepts a live frame where a terminal is expected (the
        // frame's terminal is used).
        Value::Frame(_) => Ok(Value::Nil),
        w if is_terminal(i, w) => Ok(Value::Nil),
        other => Err(i.wrong_type_mut("terminal-live-p", other)),
    }
}

/// GNU `tty-*' variants that take an optional TERMINAL and return a
/// constant on our dumb terminal — same shape as `f_nil' but with
/// the terminal-live-p arg check GNU does.
fn f_tty_nil(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let _ = want_terminal_live(i, &arg(&a, 0))?;
    Ok(Value::Nil)
}
fn f_tty_false(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    f_tty_nil(i, a)
}

/// `window-live-p' check on an optional arg, then nil (all our
/// windows are live).
fn f_window_live_nil(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    match arg(&a, 0) {
        Value::Nil | Value::Window(_) => Ok(Value::Nil),
        other => Err(i.wrong_type_mut("window-live-p", &other)),
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
fn f_window_sizable_p(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
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
                if horiz { w.width } else { w.height }
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
fn f_set_frame_position(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    match arg(&a, 0) {
        Value::Nil | Value::Frame(_) => Ok(Value::t()),
        other => Err(i.wrong_type_mut("frame-live-p", &other)),
    }
}

/// Mark a frame for state-change notification at the next redisplay.
fn f_set_frame_window_state_change(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let frame = frame_live(i, &arg(&a, 0))?;
    let changed = arg(&a, 1).truthy();
    frame.borrow_mut().window_state_change = changed;
    Ok(Value::from_bool(changed))
}

fn f_frame_window_state_change(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let frame = frame_live(i, &arg(&a, 0))?;
    Ok(Value::from_bool(frame.borrow().window_state_change))
}


/// `set-frame-size-and-position-pixelwise' — GNU Fset_frame_size_and_
/// position_pixelwise on a tty: decode_live_frame, nil gravity → 1,
/// then `Fset_frame_size` (integerp width) and `check_integer_range` on
/// X/Y — nil X is `wrong-type-argument integerp'.
fn f_set_frame_size_and_position_pixelwise(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let _ = frame_live(i, &arg(&a, 0))?;
    let w = &a[1];
    if !matches!(w, Value::Nil) {
        want_int(i, w)?;
    }
    let h = &a[2];
    if !matches!(h, Value::Nil) {
        want_int(i, h)?;
    }
    for v in [&a[3], &a[4]] {
        if !matches!(v, Value::Int(_)) {
            return Err(i.wrong_type_mut("integerp", v));
        }
    }
    Ok(Value::Nil)
}

/// `tty-frame-restack' — GNU's tty version always signals that the
/// operation is not implemented.
fn f_tty_frame_restack(i: &mut Interp, _a: crate::lisp::Argv) -> EvalResult {
    Err(i.error("tty-frame-restack is not implemented"))
}

/// `resize-mini-window-internal' — GNU CHECK_LIVE_WINDOW first, then
/// the minibuffer-window check.
fn f_resize_mini_window_internal(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let w = win_live(i, &arg(&a, 0))?;
    if w.borrow().minibuffer {
        Ok(Value::Nil)
    } else {
        Err(i.error("Not a valid minibuffer window"))
    }
}

/// `frame-parent' — frame-live-p check on the optional FRAME; tty
/// frames have no parent so nil.
fn f_frame_parent(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    match arg(&a, 0) {
        Value::Nil | Value::Frame(_) => Ok(Value::Nil),
        other => Err(i.wrong_type_mut("frame-live-p", &other)),
    }
}

fn f_window_size_fixed_p(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU calls the Lisp helper after accepting only a window, nil, or
    // the `preserved' sentinel for IGNORE.
    let ignore = arg(&a, 2);
    let windowp = Value::Sym(i.q("windowp"));
    let ignore_window = i.apply(&windowp, vec![ignore.clone()])?.truthy();
    let preserved = i.q("preserved");
    if !(ignore_window || ignore.is_nil() || matches!(ignore, Value::Sym(s) if s == preserved)) {
        return Ok(Value::Nil);
    }
    let normalize = Value::Sym(i.q("window-normalize-window"));
    let window = i.apply(&normalize, vec![arg(&a, 0)])?;
    let fixed = Value::Sym(i.q("window--size-fixed-1"));
    i.apply(&fixed, vec![window, arg(&a, 1), ignore])
}

fn f_truncated_partial_width(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // Match window.el: normalize with LIVE-ONLY, then check the width
    // against the buffer-local truncate-partial-width-windows setting.
    let normalize = Value::Sym(i.q("window-normalize-window"));
    let window = i.apply(&normalize, vec![arg(&a, 0), Value::t()])?;
    let full_width = Value::Sym(i.q("window-full-width-p"));
    if i.apply(&full_width, vec![window.clone()])?.truthy() {
        return Ok(Value::Nil);
    }
    let window_buffer = Value::Sym(i.q("window-buffer"));
    let buffer = i.apply(&window_buffer, vec![window.clone()])?;
    let option = Value::Sym(i.q("truncate-partial-width-windows"));
    let setting = Value::Sym(i.q("buffer-local-value"));
    let setting = i.apply(&setting, vec![option, buffer])?;
    match setting {
        integer @ (Value::Int(_) | Value::BigInt(_)) => {
            let total_width = Value::Sym(i.q("window-total-width"));
            let width = i.apply(&total_width, vec![window])?;
            let less = Value::Sym(i.q("<"));
            i.apply(&less, vec![width, integer])
        }
        other => Ok(other),
    }
}

/// `window-mode-line-height' follows the window buffer's mode-line-format;
/// GNU uses a typed `window-live-p' check here.
fn f_window_mode_line_height(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let window = win_live(i, &arg(&a, 0))?;
    Ok(Value::Int(window_mode_line_height(i, &window)? as i128))
}

pub(crate) fn window_format_line_height(
    i: &mut Interp,
    window: &WindowRef,
    variable: &str,
) -> Result<usize, Flow> {
    let buffer = i
        .buffer_value(window.borrow().buffer)
        .unwrap_or(Value::Nil);
    let variable = Value::Sym(i.q(variable));
    let buffer_local_value = Value::Sym(i.q("buffer-local-value"));
    let value = i.apply(&buffer_local_value, vec![variable, buffer])?;
    Ok(usize::from(value.truthy()))
}

pub(crate) fn window_mode_line_height(
    i: &mut Interp,
    window: &WindowRef,
) -> Result<usize, Flow> {
    let has_mode_line = {
        let window = window.borrow();
        !window.minibuffer && window.horiz.is_none()
    };
    if has_mode_line {
        window_format_line_height(i, window, "mode-line-format")
    } else {
        Ok(0)
    }
}

fn f_window_header_line_height(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let window = win_live(i, &arg(&a, 0))?;
    Ok(Value::Int(
        window_format_line_height(i, &window, "header-line-format")? as i128,
    ))
}

fn f_window_tab_line_height(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let window = win_live(i, &arg(&a, 0))?;
    Ok(Value::Int(
        window_format_line_height(i, &window, "tab-line-format")? as i128,
    ))
}

/// `frame-ancestor-p' — both args must be live frames; our single
/// frame is no one's ancestor.
fn f_frame_ancestor_p(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU: both args default to the selected frame when nil; the check is
    // CHECK_LIVE_FRAME on each (so a stray arg signals frame-live-p with
    // the BAD value, not nil).
    let anc = if arg(&a, 0).is_nil() {
        sel_frame(i).map(Value::Frame).unwrap_or(Value::Nil)
    } else {
        a[0].clone()
    };
    let desc = if arg(&a, 1).is_nil() {
        sel_frame(i).map(Value::Frame).unwrap_or(Value::Nil)
    } else {
        a[1].clone()
    };
    for v in [&anc, &desc] {
        match v {
            Value::Frame(f) if !f.borrow().dead => {}
            other => return Err(i.wrong_type_mut("frame-live-p", other)),
        }
    }
    // A frame with no parents is no one's ancestor; our batch frames
    // never link, so the walk always ends in nil.
    Ok(Value::Nil)
}

/// `set-window-new-normal' — decode_valid_window; stores the SIZE
/// value in `new_normal' and returns it (nil when omitted).
fn f_set_window_new_normal(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let w = win_valid(i, &arg(&a, 0))?;
    let size = arg(&a, 1);
    w.borrow_mut().new_normal = size.clone();
    Ok(size)
}

/// `set-window-new-pixel' — decode_valid_window + CHECK_FIXNUM;
/// stores SIZE in `new_pixel' and returns SIZE.
fn f_set_window_new_pixel(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let w = win_valid(i, &arg(&a, 0))?;
    let n = want_int(i, &a[1])?;
    w.borrow_mut().new_pixel = n;
    Ok(a[1].clone())
}

/// `set-window-combination-limit' — sets the flag on an internal
/// window; GNU signals a plain error on leaves.
fn f_set_window_combination_limit(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    match &a[0] {
        Value::Window(w) if !w.borrow().dead => {
            if w.borrow().horiz.is_some() {
                w.borrow_mut().combination_limit = arg(&a, 1).truthy();
                Ok(arg(&a, 1))
            } else {
                Err(i.error("Combination limit is meaningful for internal windows only"))
            }
        }
        _ => Err(i.error("Combination limit is meaningful for internal windows only")),
    }
}

/// `window-next-buffers' — list of buffers recorded by
/// `unrecord-window-buffer'/quit-restore; nil selects the selected
/// window.
fn f_window_next_buffers(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let w = match arg(&a, 0) {
        Value::Nil => sel_window(i).ok_or_else(|| i.error("No window"))?,
        other => win_live(i, &other)?,
    };
    Ok(w.borrow().next_buffers.clone())
}

/// `window-prev-buffers' — list of `(buffer start point)' entries
/// for buffers previously shown in WINDOW.
fn f_window_prev_buffers(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let w = match arg(&a, 0) {
        Value::Nil => sel_window(i).ok_or_else(|| i.error("No window"))?,
        other => win_live(i, &other)?,
    };
    Ok(w.borrow().prev_buffers.clone())
}

/// `window-discard-buffer-from-window' — GNU window.el: remove BUFFER
/// from WINDOW's prev-buffers/next-buffers lists and from the
/// `quit-restore'/`quit-restore-prev' parameters (list entries whose
/// car is BUFFER are dropped; a bare BUFFER parameter resets to nil
/// only when it names the discarded buffer).
fn f_window_discard_buffer_from_window(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let bufv = arg(&a, 0);
    let bid = match &bufv {
        Value::Buffer(b) => Some(b.borrow().id),
        _ => None,
    };
    let w = match &a[1] {
        Value::Nil => sel_window(i).ok_or_else(|| i.error("No window"))?,
        other => win_live(i, other)?,
    };
    let same_buf = |v: &Value| -> bool {
        match (v, bid) {
            (Value::Buffer(b), Some(id)) => b.borrow().id == id,
            _ => false,
        }
    };
    // prev-buffers entries are (BUFFER START MARKER POINT MARKER).
    let mut wb = w.borrow_mut();
    if let Ok(items) = wb.prev_buffers.clone().list_to_vec() {
        let kept: Vec<Value> = items
            .into_iter()
            .filter(|e| {
                if let Value::Cons(c) = e {
                    !same_buf(&c.borrow().car)
                } else {
                    true
                }
            })
            .collect();
        wb.prev_buffers = Value::list(kept);
    }
    if let Ok(items) = wb.next_buffers.clone().list_to_vec() {
        let kept: Vec<Value> = items.into_iter().filter(|e| !same_buf(e)).collect();
        wb.next_buffers = Value::list(kept);
    }
    drop(wb);
    // GNU also clears quit-restore entries that reference the buffer;
    // parameter values are (window|frame . buffer) conses or bare
    // buffer objects.
    for pname in ["quit-restore", "quit-restore-prev"] {
        let pid = i.intern(pname);
        let v = crate::lisp::eval::plist_get(&w.borrow().params, pid);
        let stale = match &v {
            Value::Cons(c) => same_buf(&c.borrow().cdr),
            v2 => same_buf(v2),
        };
        if stale {
            let new = crate::lisp::eval::plist_put(&w.borrow().params, pid, Value::Nil);
            w.borrow_mut().params = new;
        }
    }
    Ok(Value::Nil)
}

/// `set-window-next-buffers' — replace WINDOW's next-buffers list.
fn f_set_window_next_buffers(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let w = match &a[0] {
        Value::Nil => sel_window(i).ok_or_else(|| i.error("No window"))?,
        other => win_live(i, other)?,
    };
    w.borrow_mut().next_buffers = arg(&a, 1);
    Ok(Value::Nil)
}

/// `set-window-prev-buffers' — replace WINDOW's prev-buffers list.
fn f_set_window_prev_buffers(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let w = match &a[0] {
        Value::Nil => sel_window(i).ok_or_else(|| i.error("No window"))?,
        other => win_live(i, other)?,
    };
    w.borrow_mut().prev_buffers = arg(&a, 1);
    Ok(Value::Nil)
}

/// `set-window-margins' — window-live-p check (nil = selected
/// window), stores the margin widths on the window; GNU returns t.
fn f_set_window_margins(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
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
fn f_set_window_fringes(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    match &a[0] {
        Value::Nil | Value::Window(_) => Ok(Value::t()),
        other => Err(i.wrong_type_mut("window-live-p", &other)),
    }
}

/// `set-window-scroll-bars' — window-live-p check; GNU returns nil.
fn f_set_window_scroll_bars(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    match &a[0] {
        Value::Nil | Value::Window(_) => Ok(Value::Nil),
        other => Err(i.wrong_type_mut("window-live-p", &other)),
    }
}

/// `raise-frame'/`lower-frame'/`iconify-frame' — frame-live-p check,
/// then nil (no window manager to talk to).
fn f_frame_live_nil(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    match arg(&a, 0) {
        Value::Nil | Value::Frame(_) => Ok(Value::Nil),
        other => Err(i.wrong_type_mut("frame-live-p", &other)),
    }
}

/// `frame-after-make-frame' — GNU checks FRAME with frame-live-p
/// and returns the VALUE flag.
fn f_frame_after_make_frame(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    match &a[0] {
        Value::Frame(_) => Ok(arg(&a, 1)),
        other => Err(i.wrong_type_mut("frame-live-p", other)),
    }
}

/// `frame--set-was-invisible' — GNU decode_live_frame: nil means the
/// selected frame; return value is WAS-INVISIBLE normalized to t/nil.
fn f_frame_set_was_invisible(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    match &a[0] {
        Value::Nil => Ok(arg(&a, 1)),
        Value::Frame(f) if !f.borrow().dead => Ok(arg(&a, 1)),
        other => Err(i.wrong_type_mut("frame-live-p", other)),
    }
}

/// `redirect-frame-focus' — GNU checks both args with framep; the
/// redirect itself is a display detail we don't model.
fn f_redirect_frame_focus(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU: FRAME via decode_any_frame (framep); a non-nil FOCUS-FRAME
    // must be live (frame-live-p).  FOCUS-FRAME is optional.
    crate::editor::frame_any(i, &a[0])?;
    match a.get(1) {
        None | Some(Value::Nil) => {}
        Some(Value::Frame(f)) if !f.borrow().dead => {}
        Some(other) => return Err(i.wrong_type_mut("frame-live-p", other)),
    }
    Ok(Value::Nil)
}

/// `tty-find-type' (GNU faces.el): return the longest prefix of TYPE
/// to which PRED returns non-nil; only prefixes followed by a
/// dash/underscore in the original name are tried (last "-_."-delimited
/// suffix is stripped each round).  A non-callable PRED signals
/// `invalid-function' with it as datum; TYPE must be a string.
fn f_tty_find_type(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let callable = match &a[0] {
        Value::Sym(s) => i.fbound_p(*s),
        Value::Lambda(_) | Value::Subr(_) => true,
        _ => false,
    };
    if !callable {
        let s = i.q("invalid-function");
        return Err(i.signal_data(s, vec![a[0].clone()]));
    }
    let mut typ = a[1].clone();
    loop {
        // GNU's while: nil short-circuits to nil; PRED sees the raw value.
        if typ.is_nil() {
            return Ok(Value::Nil);
        }
        if i.apply(&a[0], vec![typ.clone()])?.truthy() {
            return Ok(typ);
        }
        let s = match &typ {
            Value::Str(s) => s.borrow().clone(),
            other => return Err(i.wrong_type_mut("stringp", other)),
        };
        let cut = s
            .chars().enumerate().filter(|(_, c)| *c == '-' || *c == '_' || *c == '.').map(|(index, _)| index).last()
            .map(|p| &s[..p])
            .unwrap_or_default();
        if cut.is_empty() {
            return Ok(Value::Nil);
        }
        typ = Value::string(cut.to_string());
    }
}

/// The terminal parameter alist: GNU's tty defaults, seeded lazily so
/// `set-terminal-parameter' can overwrite them.
fn terminal_params(i: &mut Interp) -> &Vec<(Value, Value)> {
    if i.terminal_params.is_empty() {
        i.terminal_params = vec![
            (
                Value::Sym(i.q("normal-erase-is-backspace")),
                Value::Int(0),
            ),
            (
                Value::Sym(i.q("keyboard-coding-saved-meta-mode")),
                // GNU stores a one-element list here (the saved
                // meta-mode flag), not bare t.
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

fn f_terminal_parameters(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let _ = want_terminal_live(i, &arg(&a, 0))?;
    let entries = terminal_params(i)
        .iter()
        .map(|(k, v)| Value::cons(k.clone(), v.clone()))
        .collect();
    Ok(Value::list(entries))
}

fn f_terminal_parameter(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU checks PARAMETER (symbolp) before
    // TERMINAL (terminal-live-p).
    if !matches!(a[1], Value::Sym(_)) {
        return Err(i.wrong_type_mut("symbolp", &a[1]));
    }
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
fn f_set_terminal_parameter(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
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

fn f_frame_face_hash_table(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // Validate FRAME like GNU (frame-live-p), then hand back a hash
    // table of the frame's faces: symbol → lface vector
    // [face ATTR...] — `frame-face-alist' maphashes over this.
    if let Some(v) = a.first() {
        match v {
            Value::Nil | Value::Frame(_) => {}
            other => return Err(i.wrong_type_mut("frame-live-p", other)),
        }
    }
    use crate::lisp::value::{HashTest, LispHash};
    let mut h = LispHash::new(HashTest::Eq);
    let un = Value::Sym(i.q("unspecified"));
    // GNU stores an lface vector per face — populate its attribute slots
    // from the face's stored plist (face_table, seeded with spec attrs;
    // face_t_table overrides for new-frame defaults written via isla t).
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
    let mut names: Vec<_> = crate::editor::KNOWN_FACES.iter().map(|name| i.q(name)).collect();
    names.extend(i.face_table.iter().map(|(face, _)| *face));
    for n in names {
        let sym = Value::Sym(n);
        let key = crate::lisp::builtins::hashfn::hash_key_for(i, &sym, HashTest::Eq);
        let mut v = Vec::with_capacity(20);
        v.push(Value::Sym(i.q("face")));
        v.resize(20, un.clone());
        // Apply spec attrs from face_table then face_t_table overrides.
        for src_plist in [
            i.face_table
                .iter()
                .find(|(m, _)| *m == n)
                .map(|(_, p)| p.clone()),
            i.face_t_table
                .iter()
                .find(|(m, _)| *m == n)
                .map(|(_, p)| p.clone()),
        ]
        .into_iter()
        .flatten()
        {
            for (si, attr) in SLOTS.iter().enumerate() {
                let kw = i.intern(attr);
                let val = crate::lisp::eval::plist_get(&src_plist, kw);
                if !val.is_nil() || crate::lisp::eval::plist_lookup(&src_plist, kw).is_some() {
                    v[si + 1] = val;
                }
            }
        }
        h.map.insert(key.clone(), Value::vector(v));
        h.put_key(key, sym);
    }
    Ok(Value::Hash(Rc::new(RefCell::new(h))).tracked())
}

/// `controlling-tty-p' — GNU validates the optional TERMINAL with
/// terminal-live-p; in batch we are never the controlling tty → nil.
fn f_controlling_tty_p(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    match arg(&a, 0) {
        Value::Nil => Ok(Value::Nil),
        v if is_terminal(i, &v) => Ok(Value::Nil),
        other => Err(i.wrong_type_mut("terminal-live-p", &other)),
    }
}

fn f_terminal_live_p(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // Our only terminal is the singleton record (nil = default
    // terminal); a live frame means its terminal is live.
    Ok(Value::from_bool(match &a[0] {
        Value::Nil => true,
        Value::Frame(f) => !f.borrow().dead,
        v => is_terminal(i, v),
    }))
}

fn f_uncombine_window(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // nil = selected window; nothing to uncombine in batch → nil.
    match &a[0] {
        Value::Nil | Value::Window(_) => Ok(Value::Nil),
        other => Err(i.wrong_type_mut("window-valid-p", other)),
    }
}

fn f_combine_windows(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    match &a[0] {
        Value::Window(_) => Ok(Value::Nil),
        other => Err(i.wrong_type_mut("window-valid-p", other)),
    }
}

fn f_terminal_list(i: &mut Interp, _a: crate::lisp::Argv) -> EvalResult {
    Ok(Value::list(vec![terminal_token(i)]))
}

/// Batch text has no displayed glyph dimensions; requested mode,
/// header and tab lines still contribute their terminal cell heights.
fn f_window_text_pixel_size(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let w = win_live(i, &arg(&a, 0))?;
    let (buffer, height, minibuffer, params) = {
        let w = w.borrow();
        (w.buffer, w.height, w.minibuffer, w.params.clone())
    };
    let (beg, end) = i
        .buffers
        .get(buffer)
        .map(|b| {
            let b = b.borrow();
            (b.begv as i128 + 1, b.text_len() as i128 + 1)
        })
        .unwrap_or((1, 1));
    let from = arg(&a, 1);
    let (start, offset) = match &from {
        Value::Nil => (beg, 0),
        Value::Sym(s) if *s == sym::T => (beg, 0),
        Value::Cons(c) => {
            let (position, offset) = {
                let c = c.borrow();
                (c.car.clone(), c.cdr.clone())
            };
            let start = crate::lisp::builtins::want_int_or_mark(i, &position)?.clamp(beg, end);
            let offset = crate::lisp::builtins::want_fixnum(i, &offset)? as libc::c_int;
            (start, offset)
        }
        _ => (
            crate::lisp::builtins::want_int_or_mark(i, &from)?.clamp(beg, end),
            0,
        ),
    };
    let to = arg(&a, 2);
    if !to.is_nil() && i.sym_id(&to) != Some(sym::T) {
        crate::lisp::builtins::want_int_or_mark(i, &to)?;
    }

    // Read formats in WINDOW's buffer, which can differ from the
    // caller's current buffer. Window parameters override those values.
    let saved = i.current_buffer;
    i.set_current_buffer(buffer);
    let mut formats = Vec::new();
    for name in ["mode-line-format", "header-line-format", "tab-line-format"] {
        let symbol = i.intern(name);
        let parameter = crate::lisp::eval::plist_get(&params, symbol);
        let enabled = if i
            .sym_id(&parameter)
            .is_some_and(|s| i.symbol_name(s) == "none")
        {
            false
        } else {
            parameter.truthy() || i.symbol_value(symbol).truthy()
        };
        formats.push(enabled);
    }
    i.set_current_buffer(saved);
    let mode = !minibuffer && formats[0] && height > 1;
    let header = !minibuffer && formats[1] && height > 1 + usize::from(mode);
    let tab = !minibuffer && formats[2] && height > 1 + usize::from(mode) + usize::from(header);
    let lines = arg(&a, 5);
    let all = i.sym_id(&lines) == Some(sym::T);
    let mut measured_height = 0;
    for (name, wanted) in [
        ("mode-line", mode),
        ("header-line", header),
        ("tab-line", tab),
    ] {
        if wanted && (all || i.sym_id(&lines).is_some_and(|s| i.symbol_name(s) == name)) {
            measured_height += 1;
        }
    }
    if offset != 0 {
        Ok(Value::list(vec![
            Value::Int(0),
            Value::Int(measured_height),
            Value::Int(start),
        ]))
    } else {
        Ok(Value::cons(Value::Int(0), Value::Int(measured_height)))
    }
}

/// tty output-buffer helpers on a non-tty terminal → GNU error.
fn f_not_tty(i: &mut Interp, _a: crate::lisp::Argv) -> EvalResult {
    Err(i.error("Not a tty terminal"))
}

/// `tty--set-output-buffer-size' — GNU term.c checks the SIZE first
/// ("Invalid output buffer size" on non-positive-integer), then the
/// TERMINAL arg (`terminal-live-p'), then the tty-ness of the device
/// ("Attempt to suspend a non-text terminal device" in batch).
fn f_tty_set_output_buffer_size(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    match &a[0] {
        Value::Int(n) if *n > 0 => {}
        _ => return Err(i.error("Invalid output buffer size")),
    }
    let _ = want_terminal_live(i, &arg(&a, 1))?;
    Err(i.error("Attempt to suspend a non-text terminal device"))
}

/// `make-frame-invisible' — our batch frame is always the sole
/// visible frame → GNU's error.
fn f_make_frame_invisible(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    if let Some(v) = a.first() {
        match v {
            Value::Nil | Value::Frame(_) => {}
            other => return Err(i.wrong_type_mut("frame-live-p", other)),
        }
    }
    Err(i.error("Attempt to make invisible the sole visible or iconified frame"))
}

/// `reconsider-frame-fonts' — frame check, then GNU's non-wsi error.
fn f_reconsider_frame_fonts(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    match &a[0] {
        Value::Nil | Value::Frame(_) => {}
        other => return Err(i.wrong_type_mut("frame-live-p", other)),
    }
    Err(i.error("Window system frame should be used"))
}

fn f_terminal_name(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let _ = want_terminal_live(i, &arg(&a, 0))?;
    Ok(Value::string("initial_terminal"))
}

fn f_set_window_new_total(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU:
    // decode_valid_window + CHECK_FIXNUM; returns SIZE (or SIZE+old when
    // ADD is non-nil).
    let w = win_valid(i, &arg(&a, 0))?;
    let n = want_int(i, &a[1])?;
    if arg(&a, 2).truthy() {
        let sum = n + w.borrow().new_total;
        w.borrow_mut().new_total = sum;
        Ok(Value::Int(sum))
    } else {
        w.borrow_mut().new_total = n;
        Ok(a[1].clone())
    }
}

fn f_frame_pointer_visible_p(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU: FRAME is a plain
    // framep arg (dead frames ok); batch tty always shows it.
    let _ = frame_any(i, &arg(&a, 0))?;
    Ok(Value::t())
}
/// Mark WINDOW second most recently used only while the selected
/// window still carries the latest selection count.
fn f_window_bump_use_time(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let window = win_live(i, &arg(&a, 0))?;
    let selected = sel_window(i).ok_or_else(|| i.error("No window"))?;
    if Rc::ptr_eq(&window, &selected) || selected.borrow().use_time != i.window_select_count {
        return Ok(Value::Nil);
    }
    let previous = i.window_select_count;
    window.borrow_mut().use_time = previous;
    selected.borrow_mut().use_time = crate::editor::next_use_time(i);
    Ok(Value::Int(previous as i128))
}

/// `window-normal-size' — GNU Fwindow_normal_size decodes WINDOW with
/// decode_valid_window (wrong-type `window-valid-p').
fn f_window_normal_size(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let window = win_valid(i, &arg(&a, 0))?;
    let window = window.borrow();
    let size = if arg(&a, 1).truthy() {
        window.normal_size.1
    } else {
        window.normal_size.0
    };
    Ok(Value::float(size))
}

/// `window-resize-apply' — GNU Fwindow_resize_apply: decode_live_frame
/// (wta `frame-live-p'), then `window_resize_check` fails on the batch
/// frame (new_pixel unset ≠ pixel height) → nil.
fn f_window_resize_apply(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let _ = frame_live(i, &arg(&a, 0))?;
    Ok(Value::Nil)
}

/// `window-resize-apply-total' — GNU Fwindow_resize_apply_total:
/// decode_live_frame, applies totals unconditionally → t.
fn f_window_resize_apply_total(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let _ = frame_live(i, &arg(&a, 0))?;
    Ok(Value::t())
}

/// `run-window-scroll-functions' — GNU Frun_window_scroll_functions
/// decodes WINDOW with decode_live_window (wrong-type `window-live-p').
pub(crate) fn f_run_window_scroll_functions(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let argument = arg(&a, 0);
    let window = win_live(i, &argument)?;
    let buffer = window.borrow().buffer;
    let caller = i.current_buffer;
    i.set_current_buffer(buffer);
    let hook = i.q("window-scroll-functions");
    let start = Value::Int(window.borrow().start as i128 + 1);
    // GNU passes the original WINDOW argument: nil stays nil even
    // though decode_live_window has resolved the selected window.
    let result = crate::lisp::builtins::evalfn::f_run_hook_with_args(
        i,
        vec![Value::Sym(hook), argument, start].into(),
    );
    if i.buffer_live(caller) {
        i.set_current_buffer(caller);
    }
    result
}

/// `frame-scale-factor' — GNU Fframe_scale_factor decodes FRAME with
/// decode_live_frame (wrong-type `frame-live-p'); batch scale is 1.0.
fn f_frame_scale_factor(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let _ = frame_live(i, &arg(&a, 0))?;
    Ok(Value::float(1.0))
}

/// `tty-frame-list-z-order' — GNU Ftty_frame_list_z_order decodes
/// FRAME with decode_live_frame (wrong-type `frame-live-p').
fn f_tty_frame_list_z_order(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let _ = frame_live(i, &arg(&a, 0))?;
    f_frame_list(i, a)
}

fn f_x_focus_frame(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU errors "Cannot switch to an invisible frame" etc. in batch;
    // on a lone tty frame any arg raises `error'.
    let _ = a;
    let sym = i.q("error");
    Err(i.signal_data(
        sym,
        vec![Value::string("Window system frame should be used")],
    ))
}

fn f_make_frame_visible(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU
    // decode_live_frame: nil → selected frame; any non-frame (or a
    // dead frame) is `(wrong-type-argument frame-live-p X)'.
    match &arg(&a, 0) {
        Value::Frame(f) if !f.borrow().dead => Ok(a[0].clone()),
        Value::Nil => match crate::editor::sel_frame(i) {
            Some(f) => Ok(Value::Frame(f)),
            None => Ok(Value::Nil),
        },
        other => Err(i.wrong_type_mut("frame-live-p", other)),
    }
}

/// frame-*-width/height parameter getters: arg must satisfy framep
/// (any frame); value is 0 on our single batch frame.
fn f_zero_framep(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    match arg(&a, 0) {
        Value::Nil | Value::Frame(_) => Ok(Value::Int(0)),
        other => Err(i.wrong_type_mut("framep", &other)),
    }
}

/// window-*-height/divider-width getters: decode_live_window —
/// `window-live-p' on non-windows, 0 on the single batch window.
fn f_zero_winlive(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let _ = win_live(i, &arg(&a, 0))?;
    Ok(Value::Int(0))
}

/// `window-old-pixel-width'/`window-old-pixel-height' — GNU's
/// decode_valid_window (`window-valid-p'), 0 in batch.
fn f_zero_winvalid(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let _ = win_valid(i, &arg(&a, 0))?;
    Ok(Value::Int(0))
}

/// `tty-frame-edges' / `tty-frame-geometry' — GNU term.c subs:
/// the optional FRAME is decoded with `frame-live-p' (arg 1), then
/// batch ttys return nil (no tracked edge/geometry list).
fn f_tty_frame_edges(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    match a.first() {
        None | Some(Value::Nil) => {}
        Some(v) => {
            let _ = crate::editor::frame_live(i, v)?;
        }
    }
    Ok(Value::Nil)
}
fn f_tty_frame_geometry(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    f_tty_frame_edges(i, a)
}

/// `make-terminal-frame' — GNU frame.c: PARMS is a listp-checked
/// alist; without a matching `tty' entry naming a live terminal the
/// fallback signals plain `error "Unknown terminal type"' — what
/// every batch invocation hits (no tty device lookup succeeds).
fn f_make_terminal_frame(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    match &a[0] {
        Value::Nil => {}
        Value::Cons(_) => {}
        other => return Err(i.wrong_type_mut("listp", other)),
    }
    Err(i.error("Unknown terminal type"))
}

/// `redirect-debugging-output' — GNU print.c: FILE is expanded via
/// `expand-file-name', opened O_WRONLY|O_CREAT with O_APPEND only when
/// APPEND is non-nil (else O_TRUNC), then dup2'd onto stderr.  A nil
/// FILE resets stderr to the saved original dup.  `file-error'-family
/// on fopen failure: ("Cannot open debugging output stream" strerror
/// EXPANDED-FILE).
fn f_redirect_debugging_output(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // Saved copy of the original stderr (GNU's static stderr_dup);
    // -1 until the first successful redirect.
    static STDERR_DUP: std::sync::atomic::AtomicI32 = std::sync::atomic::AtomicI32::new(-1);
    if a[0].is_nil() {
        #[cfg(unix)]
        unsafe {
            let orig = STDERR_DUP.swap(-1, std::sync::atomic::Ordering::SeqCst);
            if orig >= 0 {
                libc::dup2(orig, libc::STDERR_FILENO);
                libc::close(orig);
            }
        }
        return Ok(Value::Nil);
    }
    let raw = crate::lisp::builtins::want_string(i, &a[0])?;
    let path = crate::editor::expand_file_name_str(i, &raw);
    #[cfg(unix)]
    unsafe {
        if STDERR_DUP.load(std::sync::atomic::Ordering::SeqCst) < 0 {
            let n = libc::fcntl(libc::STDERR_FILENO, libc::F_DUPFD_CLOEXEC, 3);
            if n >= 0 {
                STDERR_DUP.store(n, std::sync::atomic::Ordering::SeqCst);
            }
        }
    }
    let append = a.get(1).map(|v| v.truthy()).unwrap_or(false);
    match std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .append(append)
        .truncate(!append)
        .open(crate::editor::native_path(i, &path)?)
    {
        Ok(f) => {
            #[cfg(unix)]
            unsafe {
                use std::os::unix::io::IntoRawFd;
                let fd = f.into_raw_fd();
                libc::dup2(fd, libc::STDERR_FILENO);
                if fd != libc::STDERR_FILENO {
                    libc::close(fd);
                }
            }
            let _ = f;
            Ok(Value::Nil)
        }
        Err(e) => {
            let s = match e.kind() {
                std::io::ErrorKind::NotFound => sym::FILE_MISSING,
                std::io::ErrorKind::PermissionDenied => i.q("permission-denied"),
                _ => sym::FILE_ERROR,
            };
            Err(i.signal_data(
                s,
                vec![
                    Value::string("Cannot open debugging output stream"),
                    Value::string(crate::editor::file_error_string(&e)),
                    Value::string(path),
                ],
            ))
        }
    }
}

/// `search-emacs-glossary' — GNU's menu-bar defun is a subr in the
/// native-compiled dump (0 args); body is `(info "(emacs)Glossary")'.
fn f_search_emacs_glossary(i: &mut Interp, _a: crate::lisp::Argv) -> EvalResult {
    let info = Value::Sym(i.q("info"));
    i.apply(&info, vec![Value::string("(emacs)Glossary")])
}
