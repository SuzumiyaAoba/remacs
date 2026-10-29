//! Frame/window/display compatibility primitives.

use crate::editor::{FrameRef, WindowRef, sel_frame, sel_window, w_parent};
use crate::lisp::eq_values;

use crate::lisp::builtins::{arg, want_int, want_list};
use crate::lisp::{
    Interp,
    error::{EvalResult, Flow},
    obarray::sym,
    value::Value,
};

pub(super) fn f_display_screens(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU batch: one screen (the initial terminal).
    let v = a.get(0).cloned().unwrap_or(Value::Nil);
    let _ = decode_display(i, &v)?;
    Ok(Value::Int(1))
}

/// GNU's optional FRAME argument check for face/window readers:
/// nil ok, live frame ok, else `wrong-type-argument frame-live-p'
/// (decode_live_frame).
pub(super) fn want_opt_frame(i: &mut Interp, v: &Value) -> Result<(), Flow> {
    match v {
        Value::Nil => Ok(()),
        Value::Frame(f) if !f.borrow().dead => Ok(()),
        other => Err(i.wrong_type_mut("frame-live-p", other)),
    }
}

/// GNU Fframe_or_buffer_changed_p (dispnew.c): the state snapshot
/// records every frame (object + name) and every LIVE buffer whose
/// name doesn't start with a space (object + read_only +
/// `buffer-modified-p' flag — NOT the modtick, so edits to an
/// already-dirty buffer don't count).  Space-named scratch buffers
/// like the driver's " *temp*" are skipped entirely.
pub(super) fn frame_buffer_state(i: &mut Interp) -> Vec<Value> {
    let mut v: Vec<Value> = Vec::new();
    for f in &i.frames {
        let name = f.borrow().name.clone();
        v.push(Value::Frame(f.clone()));
        v.push(Value::Str(name));
    }
    for id in i.buffers.list() {
        let Some(b) = i.buffers.get(id) else {
            continue;
        };
        let (name, ro, modified) = {
            let bb = b.borrow();
            let ro = bb
                .locals
                .get(&i.q("buffer-read-only"))
                .cloned()
                .unwrap_or(Value::Nil);
            (bb.name.clone(), ro, bb.is_modified())
        };
        if name.starts_with(' ') {
            continue;
        }
        v.push(i.buffer_value(id).unwrap_or(Value::Nil));
        v.push(ro);
        v.push(Value::from_bool(modified));
    }
    v
}

pub(super) fn f_frame_or_buffer_changed_p(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU: VARIABLE is a symbol naming a state vector.
    match a.first() {
        None | Some(Value::Nil) | Some(Value::Sym(_)) => {}
        Some(other) => return Err(i.wrong_type_mut("symbolp", other)),
    }
    let cur = frame_buffer_state(i);
    let (old, var) = match a.first() {
        Some(Value::Sym(id)) => {
            let id = *id;
            if id != sym::NIL {
                // GNU Fsymbol_value: unbound VARIABLE is void-variable.
                if !i.bound_p(id) {
                    return Err(i.signal_data(sym::VOID_VARIABLE, vec![Value::Sym(id)]));
                }
                let old = match i.symbol_value(id) {
                    Value::Vec(v) => Some(v.borrow().clone()),
                    _ => None,
                };
                (old, Some(id))
            } else {
                (i.frame_state_seen.clone(), None)
            }
        }
        _ => (i.frame_state_seen.clone(), None),
    };
    let changed = match &old {
        Some(o) => o.len() != cur.len() || !o.iter().zip(cur.iter()).all(|(x, y)| eq_values(x, y)),
        None => true,
    };
    if changed {
        if let Some(id) = var {
            i.set_symbol(id, Value::vector(cur))?;
        } else {
            i.frame_state_seen = Some(cur);
        }
    }
    Ok(Value::from_bool(changed))
}

pub(super) fn f_set_buffer_redisplay(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU xdisp.c: marks the current buffer for redisplay and returns
    // nil — the (SYMBOL NEWVAL OPERATION WHERE) args are unused, never
    // type-checked.
    let _ = (i, a);
    Ok(Value::Nil)
}

pub(super) fn f_tab_bar_height(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU tab-bar.el:
    // `(frame-parameter frame 'tab-bar-height)' — decode_any_frame:
    // wta `framep' on non-frames; a tty frame reports 0.
    let _ = crate::editor::frame_any(i, &arg(&a, 0))?;
    Ok(Value::Int(0))
}

/// `window-at-side-p' — GNU's plain "N is not a valid window" error
/// for a bad window, then SIDE nil/left/top/right/bottom selects the
/// edge; anything else fails GNU's side decode with
/// `wrong-type-argument integerp nil'.
pub(super) fn f_window_at_side_p(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU window.el: WINDOW's SIDE edge equals the same edge of the
    // frame's root window.  Our flat model has no root window object;
    // the root extent is (0,0,frame-width,minibuffer-top) — i.e. the
    // frame minus the echo area.
    let w = match arg(&a, 0) {
        Value::Nil => crate::editor::sel_window(i).ok_or_else(|| i.error("no selected window"))?,
        Value::Window(w) if !w.borrow().dead => w,
        other => {
            let shown = i.princ_to_string(&other);
            return Err(i.error(format!("{shown} is not a valid window")));
        }
    };
    let side = match arg(&a, 1) {
        Value::Nil => 3,
        Value::Sym(s) => match i.symbol_name(s).as_str() {
            "left" => 0,
            "top" => 1,
            "right" => 2,
            "bottom" => 3,
            _ => return Err(i.wrong_type_mut("integerp", &Value::Nil)),
        },
        _ => return Err(i.wrong_type_mut("integerp", &Value::Nil)),
    };
    let wid = w.borrow().id;
    let frame = i.frames.iter().find(|f| {
        f.borrow().windows.iter().any(|w2| w2.borrow().id == wid)
            || f.borrow()
                .minibuffer
                .as_ref()
                .map(|m| m.borrow().id == wid)
                .unwrap_or(false)
    });
    let Some(frame) = frame.cloned() else {
        return Ok(Value::Nil);
    };
    let (root_l, root_t, root_r, root_b) = {
        let f = frame.borrow();
        let mini_top = f
            .minibuffer
            .as_ref()
            .map(|m| m.borrow().top as i128)
            .unwrap_or(f.height as i128);
        // Root top = topmost content edge (0 unsplit, 1 after GNU's
        // menu-bar row materializes on the first split).
        let top = f
            .windows
            .iter()
            .map(|w| w.borrow().top as i128)
            .min()
            .unwrap_or(0);
        (0i128, top, f.width as i128, mini_top)
    };
    let (wl, wt, wr, wb) = {
        let b = w.borrow();
        (
            b.left as i128,
            b.top as i128,
            (b.left + b.width) as i128,
            (b.top + b.height) as i128,
        )
    };
    let at = match side {
        0 => wl == root_l,
        1 => wt == root_t,
        2 => wr == root_r,
        _ => wb == root_b,
    };
    Ok(Value::from_bool(at))
}

/// `window-cursor-info' — nil on tty; a typed window-live-p check.
pub(super) fn f_window_cursor_info(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    match arg(&a, 0) {
        Value::Nil | Value::Window(_) => Ok(Value::Nil),
        other => Err(i.wrong_type_mut("window-live-p", &other)),
    }
}

/// `x-create-frame' on an NS build: GNU's nsterm reports that
/// Nextstep windows are unavailable.
pub(super) fn f_ns_frame_stub_err(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU `x-create-frame-with-faces' CHECK_LIST(PARMS) before the
    // NS window-system check fires, so a non-list PARMS reports
    // `wrong-type-argument listp' rather than the Nextstep error.
    let _ = want_list(i, &a[0])?;
    Err(i.error("Nextstep windows are not in use or not initialized"))
}

/// `delete-other-windows-internal' — GNU decodes WINDOW with
/// decode_valid_window (`window-valid-p'); our single window is
/// never deleted.
pub(super) fn f_delete_other_windows_internal(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let _ = crate::editor::win_valid(i, &arg(&a, 0))?;
    Ok(Value::Nil)
}

/// `fit-window-to-buffer' — GNU window.el: `(window-normalize-window
/// WINDOW t)' gives `(error "%s is not a live window")' for
/// non-window args.
pub(super) fn f_fit_window_to_buffer(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    match arg(&a, 0) {
        Value::Nil => Ok(Value::Nil),
        Value::Window(w) if !w.borrow().dead => Ok(Value::Nil),
        other => Err(i.error(format!(
            "{} is not a live window",
            i.princ_to_string(&other)
        ))),
    }
}

/// `delete-windows-on' — resolves BUFFER-OR-NAME (GNU's "No such
/// buffer" error for the unresolvable); our single window is never
/// deleted, so nil.
pub(super) fn f_delete_windows_on(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU
    // window.el `window-normalize-buffer': a name naming no live
    // buffer is "No such live buffer %s"; other non-buffer objects
    // are "No such buffer %s".  Our single window is never deleted.
    match arg(&a, 0) {
        Value::Nil => Ok(Value::Nil),
        Value::Buffer(b) => {
            if !i.buffer_live(b.borrow().id) {
                return Err(i.error(format!("No such live buffer {}", i.princ_to_string(&a[0]))));
            }
            Ok(Value::Nil)
        }
        Value::Str(s) => {
            let name = s.borrow().clone();
            match i.buffers.by_name(&name) {
                Some(_) => Ok(Value::Nil),
                None => Err(i.error(format!("No such live buffer {name}"))),
            }
        }
        other => {
            let shown = i.princ_to_string(&other);
            Err(i.error(format!("No such buffer {shown}")))
        }
    }
}

/// `window--display-buffer' — GNU's Lisp defun (nativecomp build:
/// `subrp' t), replayed in Rust.  `(when (and (buffer-live-p BUFFER)
/// (window-live-p WINDOW)) ...)': a dead buffer or a dead/minibuffer
/// window quietly returns nil — NO type check on either arg (a number
/// `buffer' arg is nil-live too).  On success: when the window isn't
/// already showing BUFFER, `dedicated' clears and the buffer slot is
/// set (the shown buffer remembered in `old_buffer'); a non-nil ALIST
/// `bump-use-time' bumps use time; the dedicated-entry policy applies
/// only when not already dedicated and TYPE isn't plain `reuse' on a
/// non-side window; TYPE ∈ (window frame tab) drops prev-buffers.
pub(super) fn f_window_display_buffer(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let bid = match &a[0] {
        Value::Buffer(b) => {
            let id = b.borrow().id;
            if i.buffer_live(id) { Some(id) } else { None }
        }
        // GNU's `(and (buffer-live-p buffer) (window-live-p window))':
        // `buffer-live-p' is `(bufferp x) && live' — a name STRING is
        // not a buffer object, so `"a" → nil, no lookup.  (`display-buffer'
        // callers normalize names via `window-normalize-buffer' first.)
        _ => None, // (buffer-live-p 42)/"a" → nil, no signal
    };
    let Some(bid) = bid else {
        return Ok(Value::Nil);
    };
    let w = match &a[1] {
        Value::Window(w) => w.clone(),
        _ => return Ok(Value::Nil), // (window-live-p x) → nil
    };
    {
        let wb = w.borrow();
        if wb.dead || wb.minibuffer {
            return Ok(Value::Nil);
        }
    }
    if w.borrow().buffer != bid {
        // GNU set-window-buffer: swap `buffer'; `old_buffer' is only
        // written when a buffer leaves a window (unshow_window), so this
        // path must NOT touch it.
        let point = i
            .buffers
            .get(bid)
            .map_or(0, |buffer| buffer.borrow().point());
        let mut wb = w.borrow_mut();
        wb.dedicated = Value::Nil;
        wb.buffer = bid;
        crate::editor::set_window_old_point(i, &mut wb, point);
        // A brand-new window's `next-buffers' entry.
        if !wb.next_buffers.is_nil() {
            wb.next_buffers = Value::Nil;
        }
        drop(wb);
        crate::editor::record_buffer_display(i, bid);
    }
    // (window-bump-use-time WINDOW) when ALIST has `bump-use-time'.
    {
        let alist = arg(&a, 3);
        // NOTE: collect the interesting ALIST entries eagerly so the
        // closure borrow of `i' doesn't fight the mutable intern below.
        let bump = {
            let items = alist.list_to_vec().unwrap_or_default();
            items.iter().any(|e| {
                if let Value::Cons(c) = e {
                    let cb = c.borrow();
                    matches!(&cb.car, Value::Sym(s) if i.symbol_name(*s) == "bump-use-time")
                        && cb.cdr.truthy()
                } else {
                    false
                }
            })
        };
        let dedicated_entry = {
            let items = alist.list_to_vec().unwrap_or_default();
            items.iter().find_map(|e| {
                if let Value::Cons(c) = e {
                    let cb = c.borrow();
                    if matches!(&cb.car, Value::Sym(s) if i.symbol_name(*s) == "dedicated") {
                        return Some(cb.cdr.clone());
                    }
                }
                None
            })
        };
        if bump {
            let bump = Value::Sym(i.q("window-bump-use-time"));
            i.apply(&bump, vec![Value::Window(w.clone())])?;
        }
        // Dedicated policy: skip if already dedicated, or TYPE is `reuse'
        // on a non-side window; else ALIST's `dedicated' wins, else
        // `display-buffer-mark-dedicated'.
        let ty_name = i.sym_id(&arg(&a, 2)).map(|s| i.symbol_name(s));
        let win_side = {
            let side_sym = i.q("window-side");
            let wb = w.borrow();
            crate::lisp::eval::plist_get(&wb.params, side_sym).truthy()
        };
        let dedicated = w.borrow().dedicated.truthy();
        if dedicated || (ty_name.as_deref() == Some("reuse") && !win_side) {
            // leave alone
        } else if let Some(v) = dedicated_entry {
            w.borrow_mut().dedicated = v;
        } else {
            let flag = i
                .obarray
                .intern_soft("display-buffer-mark-dedicated")
                .map(|id| i.symbol_value(id))
                .unwrap_or(Value::Nil);
            w.borrow_mut().dedicated = flag;
        }
        // (memq type '(window frame tab)) → (set-window-prev-buffers w nil)
        if matches!(
            ty_name.as_deref(),
            Some("window") | Some("frame") | Some("tab")
        ) {
            w.borrow_mut().prev_buffers = Value::Nil;
        }
    }
    Ok(Value::Window(w))
}

/// `x-display-*'/`x-server-*'/`xw-*' on a non-Nextstep display: GNU
/// checks the arg (frame-live-p), then signals the NS error — with a
/// terminal object the message names the terminal.
pub(super) fn f_ns_display(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    match a.first() {
        None | Some(Value::Nil) => {
            Err(i.error("Nextstep windows are not in use or not initialized"))
        }
        // GNU (nsfns.m): a string is a display name — the NS backend is
        // single-display, so it always errors this way.
        Some(Value::Str(_)) => {
            Err(i.error("Emacs for Nextstep does not yet support multi-display"))
        }
        Some(v) if crate::editor::is_terminal(i, v) => {
            Err(i.error("Terminal 0 is not a Nextstep display"))
        }
        Some(Value::Frame(_)) => Err(i.error("Terminal 0 is not a Nextstep display")),
        Some(other) => Err(i.wrong_type_mut("frame-live-p", other)),
    }
}

/// GPUI's graphical frames expose the usual color display predicate
/// through their `display-type' frame parameter.
pub(super) fn f_xw_display_color_p(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let display = a.first().cloned().unwrap_or(Value::Nil);
    let frame = decode_display(i, &display)?;
    Ok(Value::from_bool(
        crate::editor::frame_is_graphical(i, &frame),
    ))
}

/// `scroll-bar-scale' — first arg is a cons (LISTP check), returns 0
/// when there are no scroll bars.
pub(super) fn f_scroll_bar_scale(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    match &a[0] {
        Value::Cons(_) | Value::Nil => Ok(Value::Int(0)),
        other => Err(i.wrong_type_mut("listp", other)),
    }
}

/// `get-display-property' — GNU Fget_display_property checks that
/// POSITION is inside OBJECT's text (signal `args-out-of-range' with
/// `(POS POS)') and that OBJECT satisfies `buffer-or-string-p'.
/// Buffer positions are 1-based [BEGV, ZV]; string positions are
/// 0-based [0, len].  The lookup always finds no display prop in
/// batch, so a valid call returns nil.
pub(super) fn f_get_display_property(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let pos = match &a[0] {
        Value::Int(n) => *n,
        // GNU marker_position: 0-based bufpos + 1.
        Value::Marker(m) => match m.borrow().buffer {
            Some(_) => m.borrow().position as i128 + 1,
            None => return Err(i.error("Marker does not point anywhere")),
        },
        other => return Err(i.wrong_type_mut("integer-or-marker-p", other)),
    };
    // Internal begv/zv are 0-based; add 1 for the Emacs position.
    let (lo, hi) = match a.get(2) {
        None | Some(Value::Nil) => match i.current_buffer_ref() {
            Some(b) => {
                let bb = b.borrow();
                (bb.begv as i128 + 1, bb.zv as i128 + 1)
            }
            None => return Ok(Value::Nil),
        },
        Some(Value::Buffer(b)) => {
            let bb = b.borrow();
            (bb.begv as i128 + 1, bb.zv as i128 + 1)
        }
        Some(Value::Str(s)) => {
            let len = s.borrow().chars().count() as i128;
            (0, len)
        }
        Some(other) => return Err(i.wrong_type_mut("buffer-or-string-p", other)),
    };
    if pos < lo || pos > hi {
        return Err(i.signal(
            sym::ARGS_OUT_OF_RANGE,
            Value::list(vec![a[0].clone(), a[0].clone()]),
        ));
    }
    Ok(Value::Nil)
}

pub(crate) fn mouse_coordinate(i: &mut Interp, value: &Value) -> Result<i128, Flow> {
    let coordinate = want_int(i, value)?;
    if coordinate < i32::MIN as i128 || coordinate > i32::MAX as i128 {
        let condition = i.q("args-out-of-range");
        return Err(i.signal_data(
            condition,
            vec![
                value.clone(),
                Value::Int(i32::MIN as i128),
                Value::Int(i32::MAX as i128),
            ],
        ));
    }
    Ok(coordinate)
}

pub(crate) fn warp_mouse_absolute(
    i: &mut Interp,
    frame: &FrameRef,
    x: i128,
    y: i128,
) -> Result<(), Flow> {
    if !cfg!(target_os = "macos") || !crate::editor::frame_is_graphical(i, frame) {
        return Ok(());
    }
    let backend = i
        .frame_window_backend
        .clone()
        .ok_or_else(|| i.error("GUI pointer backend is unavailable"))?;
    backend
        .warp_pointer_absolute(frame.borrow().id, x, y)
        .map_err(|message| i.error(message))?;
    i.gui_mouse_absolute_position = Some((x, y));
    Ok(())
}

fn warp_mouse_in_frame(i: &mut Interp, frame: &FrameRef, x: i128, y: i128) -> EvalResult {
    if !cfg!(target_os = "macos") || !crate::editor::frame_is_graphical(i, frame) {
        return Ok(Value::Nil);
    }
    let backend = i
        .frame_window_backend
        .clone()
        .ok_or_else(|| i.error("GUI pointer backend is unavailable"))?;
    backend
        .warp_pointer_in_frame(frame.borrow().id, x, y)
        .map_err(|message| i.error(message))?;
    Ok(Value::Nil)
}

pub(super) fn f_set_mouse_position(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let frame = crate::editor::frame_live(i, &arg(&a, 0))?;
    let x = mouse_coordinate(i, &a[1])?;
    let y = mouse_coordinate(i, &a[2])?;
    let char_width = crate::editor::frame_char_width(i, &frame) as i128;
    let char_height = crate::editor::frame_char_height(i, &frame) as i128;
    let border = crate::editor::frame_internal_border_width(i, &frame) as i128;
    let top_margin = frame.borrow().text_margin as i128;
    let pixel_x = border
        .saturating_add(x.saturating_mul(char_width))
        .saturating_add(char_width / 2)
        .clamp(0, crate::editor::frame_pixel_width(i, &frame) as i128);
    let pixel_y = border
        .saturating_add(top_margin.saturating_mul(char_height))
        .saturating_add(y.saturating_mul(char_height))
        .saturating_add(char_height / 2)
        .clamp(0, crate::editor::frame_pixel_height(i, &frame) as i128);
    warp_mouse_in_frame(i, &frame, pixel_x, pixel_y)
}

pub(super) fn f_set_mouse_pixel_position(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let frame = crate::editor::frame_live(i, &arg(&a, 0))?;
    let x = mouse_coordinate(i, &a[1])?;
    let y = mouse_coordinate(i, &a[2])?;
    warp_mouse_in_frame(i, &frame, x, y)
}

pub(super) fn f_set_mouse_absolute_pixel_position(
    i: &mut Interp,
    a: crate::lisp::Argv,
) -> EvalResult {
    let frame = crate::editor::frame_live(i, &Value::Nil)?;
    if !cfg!(target_os = "macos") || !crate::editor::frame_is_graphical(i, &frame) {
        return Ok(Value::Nil);
    }
    let x = mouse_coordinate(i, &a[0])?;
    let y = mouse_coordinate(i, &a[1])?;
    warp_mouse_absolute(i, &frame, x, y)?;
    Ok(Value::Nil)
}

/// `window-preserve-size' — GNU window.el: `window-normalize-window
/// window t' gates with the plain "N is not a live window" error,
/// then reports (BUFFER HSIZE WSIZE) — nil sizes in batch.
pub(super) fn f_window_preserve_size(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let w = match arg(&a, 0) {
        Value::Nil => crate::editor::sel_window(i),
        Value::Window(w) if !w.borrow().dead => Some(w),
        other => {
            let shown = i.princ_to_string(&other);
            return Err(i.error(format!("{shown} is not a live window")));
        }
    };
    let buf = match w {
        Some(w) => i.buffer_value(w.borrow().buffer).unwrap_or(Value::Nil),
        None => Value::Nil,
    };
    Ok(Value::list(vec![buf, Value::Nil, Value::Nil]))
}

pub(super) fn f_frame_configuration_p(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let fc = i.q("frame-configuration");
    Ok(Value::from_bool(match &a[0] {
        Value::Cons(c) => i.sym_is(&c.borrow().car, fc),
        _ => false,
    }))
}

/// `current-frame-configuration' — GNU returns a list headed
/// `frame-configuration' of (FRAME PARAMS WINDOW-CONFIG) triples.
pub(super) fn f_current_frame_configuration(i: &mut Interp, _a: crate::lisp::Argv) -> EvalResult {
    let fr = match sel_frame(i) {
        Some(f) => Value::Frame(f),
        None => {
            return Ok(Value::list(vec![Value::Sym(i.q("frame-configuration"))]));
        }
    };
    let params = crate::editor::f_frame_parameters(i, vec![fr.clone()].into())?;
    let wc = crate::editor::f_current_window_configuration(i, vec![fr.clone()].into())?;
    let fc = i.q("frame-configuration");
    Ok(Value::list(vec![
        Value::Sym(fc),
        Value::list(vec![fr, params, wc]),
    ]))
}

pub(super) fn f_display_pixel_width(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let v = a.get(0).cloned().unwrap_or(Value::Nil);
    let frame = decode_display(i, &v)?;
    let (width, _) = monitor_pixel_size(i, &frame).unwrap_or_else(|| {
        (
            crate::editor::frame_pixel_width(i, &frame) as i128,
            crate::editor::frame_pixel_height(i, &frame) as i128,
        )
    });
    Ok(Value::Int(width))
}

pub(super) fn f_display_pixel_height(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let v = a.get(0).cloned().unwrap_or(Value::Nil);
    let frame = decode_display(i, &v)?;
    let (_, height) = monitor_pixel_size(i, &frame).unwrap_or_else(|| {
        (
            crate::editor::frame_pixel_width(i, &frame) as i128,
            crate::editor::frame_pixel_height(i, &frame) as i128,
        )
    });
    Ok(Value::Int(height))
}

pub(super) fn f_display_false(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let v = a.get(0).cloned().unwrap_or(Value::Nil);
    let _ = decode_display(i, &v)?;
    Ok(Value::Nil)
}

pub(super) fn f_display_pred_false(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let v = a.get(0).cloned().unwrap_or(Value::Nil);
    let _ = decode_display(i, &v)?;
    Ok(Value::Nil)
}

pub(super) fn f_display_selections_p(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let v = a.get(0).cloned().unwrap_or(Value::Nil);
    let frame = decode_display(i, &v)?;
    Ok(Value::from_bool(
        i.clipboard_backend.is_some() && crate::editor::frame_is_graphical(i, &frame),
    ))
}

/// GNU `decode_display' (xfns.c): nil → selected frame, a live frame
/// → it, a string must name an open display (only ":0"/""-style TTY
/// names resolve) — anything else signals
/// `(error "Display %s does not exist")'.
pub(crate) fn decode_display(i: &mut Interp, v: &Value) -> Result<crate::editor::FrameRef, Flow> {
    match v {
        // GNU get-device-terminal: nil → selected frame's terminal;
        // live frame → its terminal.
        Value::Nil | Value::Frame(_) => crate::editor::frame_live(i, v),
        Value::Str(s) => Err(i.error(format!("Display {} does not exist", s.borrow()))),
        // Any other object: "Invalid argument X in ‘get-device-terminal’"
        // (GNU keeps the curly-quote style even in batch).
        other => Err(i.error(format!(
            "Invalid argument {} in ‘get-device-terminal’",
            i.princ_to_string(other)
        ))),
    }
}

pub(super) fn f_display_mm(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let _ = decode_display(i, &arg(&a, 0))?;
    Ok(Value::Nil)
}

pub(super) fn f_display_visual_class(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let _ = decode_display(i, &arg(&a, 0))?;
    Ok(Value::Sym(i.q("static-gray")))
}

pub(super) fn f_display_planes(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let _ = decode_display(i, &arg(&a, 0))?;
    Ok(Value::Int(3))
}

/// Window dims helper: nil/omitted → selected window; a non-window
/// value is a `wrong-type-argument' under GNU's CHECK_WINDOW
/// convention — callers needing GNU's "not a valid window" plain
/// error use `win_any' instead.
pub(super) fn win_dims(i: &Interp, v: &Value) -> (usize, usize) {
    let w = match v {
        Value::Window(w) => Some(w.clone()),
        _ => sel_window(i),
    };
    w.map(|w| {
        let w = w.borrow();
        (w.width, w.height)
    })
    .unwrap_or((80, 24))
}

/// Character-unit minimum used by resizing and sizable queries.
/// Keep these callers consistent with state serialization and window-min-size.
pub(crate) fn win_min_size(i: &mut Interp, wv: &Value, horiz: bool, ignore: &Value) -> i128 {
    let window = match wv {
        Value::Window(w) => Some(w.clone()),
        _ => crate::editor::sel_window(i),
    };
    window.map_or(0, |w| {
        crate::editor::window_minimum(i, &w, horiz, ignore, false)
    })
}

pub(crate) fn window_is_size_fixed(
    i: &mut Interp,
    window: &Value,
    horizontal: bool,
    ignore: &Value,
) -> Result<bool, Flow> {
    let function = Value::Sym(i.q("window-size-fixed-p"));
    Ok(i
        .apply(
            &function,
            vec![window.clone(), Value::from_bool(horizontal), ignore.clone()],
        )?
        .truthy())
}

/// `window-sizable' — GNU: normalize WINDOW (plain "N is not a valid
/// window" error), DELTA must satisfy number-or-marker-p.  Negative
/// DELTA clamps at the window's minimum size; positive DELTA returns
/// DELTA unless the window is size-fixed.
pub(super) fn f_window_sizable(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let wv = Value::Window(crate::editor::win_err_valid(i, &a[0])?);
    let delta = a[1].clone();
    let horiz = !arg(&a, 2).is_nil();
    let ignore = arg(&a, 3);
    let less_than_zero = {
        let less = Value::Sym(i.q("<"));
        i.apply(&less, vec![delta.clone(), Value::Int(0)])?.truthy()
    };
    if less_than_zero {
        let min = win_min_size(i, &wv, horiz, &ignore);
        let (w, h) = win_dims(i, &a[0]);
        let size = if horiz { w } else { h } as i128;
        if size <= min {
            Ok(Value::Int(0))
        } else {
            let max = Value::Sym(i.q("max"));
            i.apply(&max, vec![Value::Int(min - size), delta])
        }
    } else {
        let greater_than_zero = {
            let greater = Value::Sym(i.q(">"));
            i.apply(&greater, vec![delta.clone(), Value::Int(0)])?.truthy()
        };
        if !greater_than_zero {
            return Ok(Value::Int(0));
        }
        if window_is_size_fixed(i, &wv, horiz, &ignore)? {
            Ok(Value::Int(0))
        } else {
            Ok(delta)
        }
    }
}

pub(super) fn f_window_max_chars(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU `window-max-chars-per-line'
    // is Lisp (window.el): `window-normalize-window window t' raises the
    // plain "N is not a live window" error on non-live args; a live tty
    // window reserves the rightmost column for the continuation glyph
    // (80-col → 79).
    let w = crate::editor::win_err_live(i, &arg(&a, 0))?;
    let width = w.borrow().width;
    Ok(Value::Int(width.saturating_sub(1) as i128))
}

/// `window-left-column' — GNU decodes a VALID window (returns `left').
pub(super) fn f_window_left_column(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let w = crate::editor::win_valid(i, &arg(&a, 0))?;
    Ok(Value::Int(w.borrow().left as i128))
}

pub(super) fn f_pos_visible(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU Fpos_visible_in_window_group_p: POS is
    // CHECK_NUMBER_COERCE_MARKER, WINDOW is CHECK_LIVE_WINDOW.
    // POS omitted/nil → point in WINDOW (the Lisp wrapper defaults it).
    let w = match arg(&a, 1) {
        Value::Nil => sel_window(i),
        Value::Window(w) => Some(w),
        other => return Err(i.wrong_type_mut("window-live-p", &other)),
    };
    // POS omitted/nil → point in WINDOW.
    let pos = match arg(&a, 0) {
        Value::Nil => w.as_ref().map(|w| w.borrow().point).unwrap_or(1),
        Value::Int(n) => n as usize,
        Value::Marker(m) => m.borrow().position,
        ref other => return Err(i.wrong_type_mut("integer-or-marker-p", other)),
    };
    let ok = match w {
        Some(w) => {
            // GNU computes visibility from the window's glyph
            // matrix — in batch there is no display, so positions
            // are never visible.
            if i.noninteractive {
                false
            } else {
                let w = w.borrow();
                pos >= w.start
            }
        }
        None => true,
    };
    Ok(Value::from_bool(ok))
}

/// `window-line' — display-line index of point in WINDOW (0-based
/// line count from window-start to point).
pub(super) fn f_window_line(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let w = match arg(&a, 0) {
        Value::Window(w) => w,
        _ => match sel_window(i) {
            Some(w) => w,
            None => return Ok(Value::Nil),
        },
    };
    let (start, bid) = {
        let b = w.borrow();
        (b.start, b.buffer)
    };
    let buf = match i.buffers.get(bid) {
        Some(b) => b,
        None => return Ok(Value::Nil),
    };
    let bb = buf.borrow();
    let point = bb.point();
    let s0 = start.min(bb.text.len());
    let e0 = point.min(bb.text.len());
    let (lo, hi) = if s0 <= e0 { (s0, e0) } else { (e0, s0) };
    let mut n = 0i128;
    for k in lo..hi {
        if bb.text.char_at(k) == '\n' {
            n += 1;
        }
    }
    Ok(Value::Int(if s0 <= e0 { n } else { -n }))
}

pub(super) fn f_selected_window(i: &mut Interp, _a: crate::lisp::Argv) -> EvalResult {
    match sel_window(i) {
        Some(w) => Ok(Value::Window(w)),
        None => Ok(Value::Nil),
    }
}

pub(super) fn f_line_number_display_width(
    i: &mut Interp,
    a: crate::lisp::Argv,
) -> EvalResult {
    let Some(window) = sel_window(i) else {
        return Ok(Value::Int(0));
    };
    let buffer = window.borrow().buffer;
    if !crate::frontend::buffer_setting(i, "display-line-numbers", buffer).truthy() {
        return Ok(Value::Int(0));
    }

    let (columns, _) = crate::frontend::line_number_display_settings(i, buffer);
    let frame = crate::editor::frame_of_window(i, &window).or_else(|| sel_frame(i));
    let character_width = frame
        .as_ref()
        .map(|frame| crate::editor::frame_char_width(i, frame))
        .unwrap_or(1)
        .max(1);
    let pixel_width = (columns as f64
        * character_width as f64
        * crate::editor::winxtra::window_pixel_scale_x(i, &window))
    .round();

    if matches!(arg(&a, 0), Value::Sym(symbol) if i.symbol_name(symbol) == "columns") {
        Ok(Value::float(pixel_width / character_width as f64))
    } else if arg(&a, 0).truthy() {
        Ok(Value::Int(pixel_width as i128))
    } else {
        Ok(Value::Int(columns as i128))
    }
}

pub(super) fn f_window_in_direction(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU window.el
    // `window-in-direction': `(window-normalize-window window t)' runs
    // FIRST — the plain "N is not a live window" check precedes the
    // `Wrong direction' error.
    enum Dir {
        Above,
        Below,
        Left,
        Right,
    }
    let window = match arg(&a, 1) {
        Value::Nil => crate::editor::sel_window(i).ok_or_else(|| i.error("no selected window"))?,
        Value::Window(w) if !w.borrow().dead => w.clone(),
        other => {
            return Err(i.error(format!(
                "{} is not a live window",
                i.princ_to_string(&other)
            )));
        }
    };
    let dir = match i.sym_id(&a[0]).map(|s| i.symbol_name(s)).as_deref() {
        Some("up") | Some("above") => Dir::Above,
        Some("down") | Some("below") => Dir::Below,
        Some("left") => Dir::Left,
        Some("right") => Dir::Right,
        _ => return Err(i.error(format!("Wrong direction {}", i.princ_to_string(&a[0])))),
    };
    let ignore = arg(&a, 2).truthy();
    let sign = arg(&a, 3).int().unwrap_or(0);
    let wrap = arg(&a, 4).truthy();
    let minibuf = arg(&a, 5);

    let wid = window.borrow().id;
    let frame = i
        .frames
        .iter()
        .find(|f| {
            f.borrow().windows.iter().any(|w| w.borrow().id == wid)
                || f.borrow()
                    .minibuffer
                    .as_ref()
                    .map(|m| m.borrow().id == wid)
                    .unwrap_or(false)
        })
        .cloned();
    let Some(frame) = frame else {
        return Ok(Value::Nil);
    };

    let (wtop, wleft, wwid, whgt, wpt) = {
        let w = window.borrow();
        (
            w.top as i128,
            w.left as i128,
            w.width as i128,
            w.height as i128,
            w.point,
        )
    };
    let hor = matches!(dir, Dir::Left | Dir::Right);
    let first = if hor { wleft } else { wtop };
    let last = first + if hor { wwid } else { whgt };
    let posn = if sign < 0 {
        if hor {
            wtop + whgt - 1
        } else {
            wleft + wwid - 1
        }
    } else if sign > 0 {
        if hor { wtop } else { wleft }
    } else {
        // GNU takes (nth 2 (posn-at-point ...)) = (COL . ROW); nil in
        // batch → the (or .. 1) fallbacks.
        let pap_sym = i.q("posn-at-point");
        let pap = i.apply(
            &Value::Sym(pap_sym),
            vec![Value::Int(wpt as i128 + 1), Value::Window(window.clone())],
        )?;
        let posn_cons = pap
            .list_to_vec()
            .ok()
            .and_then(|v| v.get(2).cloned())
            .unwrap_or(Value::Nil);
        let (pcar, pcdr) = match &posn_cons {
            Value::Cons(c) => {
                let b = c.borrow();
                (b.car.int(), b.cdr.int())
            }
            _ => (None, None),
        };
        if hor {
            pcdr.unwrap_or(1) + wtop
        } else {
            pcar.unwrap_or(1) + wleft
        }
    };

    let (fwidth, frame_bot, root_top, mini_top) = {
        let f = frame.borrow();
        let (mini_t, mini_b) = f
            .minibuffer
            .as_ref()
            .map(|m| {
                let b = m.borrow();
                (b.top as i128, (b.top + b.height) as i128)
            })
            .unwrap_or((f.height as i128, f.height as i128));
        // `frame-pixel-height' = frame bottom including the echo
        // area; `root_top' = topmost content edge (0 unsplit, 1
        // once the menu-bar row materialized).
        (
            f.width as i128,
            mini_b,
            f.windows
                .iter()
                .map(|w| w.borrow().top as i128)
                .min()
                .unwrap_or(0),
            mini_t,
        )
    };
    let mut best_edge = match dir {
        Dir::Below => frame_bot,
        Dir::Right => fwidth,
        _ => -1,
    };
    let mut best_edge_2 = best_edge;
    let mut best_diff_2 = if hor { frame_bot } else { fwidth };
    let mut best: Option<WindowRef> = None;
    let mut best_2: Option<WindowRef> = None;

    // walk-window-tree's minibuf arg: t → always include FRAME's
    // minibuffer window; nil → only when active; other → never.
    let mut cands: Vec<WindowRef> = frame.borrow().windows.clone();
    if let Some(mini) = frame.borrow().minibuffer.clone() {
        let include = match &minibuf {
            Value::Sym(s) if i.symbol_name(*s) == "t" => true,
            Value::Nil => i.minibuf_level > 0,
            _ => false,
        };
        if include && !mini.borrow().dead {
            cands.push(mini);
        }
    }
    let iwp = i
        .symbol_value(i.q_soft("ignore-window-parameters").unwrap_or(0))
        .truthy();
    let no_ow = i.q("no-other-window");
    let active_mini = i.minibuf_level > 0;

    // window-at-side-p against the root extent (0,root_top,fwidth,
    // mini_top).
    let at_side = |w: &WindowRef, s: usize| -> bool {
        let b = w.borrow();
        let (l, t, r, bt) = (
            b.left as i128,
            b.top as i128,
            (b.left + b.width) as i128,
            (b.top + b.height) as i128,
        );
        match s {
            0 => l == 0,
            1 => t == root_top,
            2 => r == fwidth,
            _ => bt == mini_top,
        }
    };

    for w in cands {
        if w.borrow().id == wid || w.borrow().dead {
            continue;
        }
        if !ignore && !iwp {
            let no_other = crate::lisp::eval::plist_get(&w.borrow().params, no_ow);
            if no_other.truthy() {
                continue;
            }
        }
        let (wt, wl, ww, wh) = {
            let b = w.borrow();
            (
                b.top as i128,
                b.left as i128,
                b.width as i128,
                b.height as i128,
            )
        };
        if hor {
            if wt <= posn && posn < wt + wh {
                // W covers the reference row.
                let ok = match dir {
                    Dir::Left => {
                        (wl <= first && wl > best_edge)
                            || (wrap && at_side(&window, 0) && at_side(&w, 2))
                    }
                    Dir::Right => {
                        (wl >= last && wl < best_edge)
                            || (wrap && at_side(&window, 2) && at_side(&w, 0))
                    }
                    _ => false,
                };
                if ok {
                    best_edge = wl;
                    best = Some(w.clone());
                }
            } else if (matches!(dir, Dir::Left) && wl + ww <= first)
                || (matches!(dir, Dir::Right) && last <= wl)
            {
                // W is on the right side axis but doesn't cover posn.
                let diff = if wt > posn { wt - posn } else { posn - wt - wh };
                if diff < best_diff_2
                    || (diff == best_diff_2
                        && match dir {
                            Dir::Left => wl > best_edge_2,
                            _ => wl < best_edge_2,
                        })
                {
                    best_edge_2 = wl;
                    best_diff_2 = diff;
                    best_2 = Some(w.clone());
                }
            }
        } else {
            if wl <= posn && posn < wl + ww {
                // W covers the reference column.
                let ok = match dir {
                    Dir::Above => {
                        (wt <= first && wt > best_edge)
                            || (wrap
                                && at_side(&window, 1)
                                && if active_mini {
                                    w.borrow().minibuffer && i.minibuf_level > 0
                                } else {
                                    at_side(&w, 3)
                                })
                    }
                    Dir::Below => {
                        (wt >= first && wt < best_edge)
                            || (wrap
                                && if active_mini {
                                    window.borrow().minibuffer && i.minibuf_level > 0
                                } else {
                                    at_side(&window, 3)
                                }
                                && at_side(&w, 1))
                    }
                    _ => false,
                };
                if ok {
                    best_edge = wt;
                    best = Some(w.clone());
                }
            } else if (matches!(dir, Dir::Above) && wt + wh <= first)
                || (matches!(dir, Dir::Below) && last <= wt)
            {
                let diff = if wl > posn { wl - posn } else { posn - wl - ww };
                if diff < best_diff_2
                    || (diff == best_diff_2
                        && match dir {
                            Dir::Above => wt > best_edge_2,
                            _ => wt < best_edge_2,
                        })
                {
                    best_edge_2 = wt;
                    best_diff_2 = diff;
                    best_2 = Some(w.clone());
                }
            }
        }
    }
    Ok(best.or(best_2).map(Value::Window).unwrap_or(Value::Nil))
}

pub(super) fn f_window_normalize(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU window.el defun: nil → selected-window; LIVE-ONLY →
    // `window-live-p' gate; otherwise `window-valid-p' (a *dead* window
    // object is invalid, internal windows are fine).  The prelude defun
    // matches GNU textually; this subr shadows it for GNU's nativecomp
    // `subrp' surface, so it must carry the same 1+1 arity.
    match &a[0] {
        Value::Nil => f_selected_window(i, vec![].into()),
        Value::Window(w) => {
            let w = w.borrow();
            let live_only = arg(&a, 1).truthy();
            let ok = if live_only {
                !w.dead && w.horiz.is_none()
            } else {
                !w.dead
            };
            if ok {
                drop(w);
                Ok(a[0].clone())
            } else {
                Err(i.error(if live_only {
                    format!("{} is not a live window", i.princ_to_string(&a[0]))
                } else {
                    format!("{} is not a valid window", i.princ_to_string(&a[0]))
                }))
            }
        }
        other => {
            let shown = i.princ_to_string(other);
            Err(i.error(if arg(&a, 1).truthy() {
                format!("{shown} is not a live window")
            } else {
                format!("{shown} is not a valid window")
            }))
        }
    }
}

pub(super) fn f_window_norm_buffer(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU window.el `window-normalize-buffer': nil →
    // current buffer; a live buffer object returns it; a string must name
    // a live buffer — anything else is `(error "No such live buffer %s")';
    // non-string atoms pass through for the caller's bufferp check.
    match &a[0] {
        Value::Nil => Ok(i
            .current_buffer_ref()
            .map(Value::Buffer)
            .unwrap_or(Value::Nil)),
        Value::Buffer(b) => {
            if i.buffer_live(b.borrow().id) {
                Ok(a[0].clone())
            } else {
                Err(i.error(format!("No such live buffer {}", i.princ_to_string(&a[0]))))
            }
        }
        Value::Str(s) => {
            let name = s.borrow().clone();
            match i.buffers.by_name(&name) {
                Some(id) if i.buffer_live(id) => match i.buffers.get(id) {
                    Some(b) => Ok(Value::Buffer(b)),
                    None => Err(i.error(format!("No such live buffer {name}"))),
                },
                _ => Err(i.error(format!("No such live buffer {name}"))),
            }
        }
        // Non-buffer non-string atoms: GNU falls to "No such buffer %s".
        other => Err(i.error(format!("No such buffer {}", i.princ_to_string(other)))),
    }
}

pub(super) fn f_window_norm_frame(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU window.el defun: nil -> selected frame; non-frame and
    // dead frames signal `(error "X is not a live frame")'.
    match &a[0] {
        Value::Frame(f) if !f.borrow().dead => Ok(Value::Frame(f.clone())),
        Value::Nil => match sel_frame(i) {
            Some(f) => Ok(Value::Frame(f)),
            None => Ok(Value::Nil),
        },
        other => {
            let shown = i.princ_to_string(other);
            Err(i.error(format!("{shown} is not a live frame")))
        }
    }
}

pub(super) fn f_get_window_pred(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let pred = a[0].clone();
    if let Some(f) = sel_frame(i) {
        let wins: Vec<WindowRef> = f.borrow().windows.clone();
        for w in wins {
            let wv = Value::Window(w.clone());
            // GNU funcall semantics: a special-form name signals
            // `invalid-function' (call_function would *call* it).
            let r = i.apply(&pred, vec![wv.clone()])?;
            if !r.is_nil() {
                return Ok(wv);
            }
        }
    }
    Ok(Value::Nil)
}

pub(super) fn f_window_with_parameter(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU window.el:
    // `walk-window-tree' decodes FRAME (arg 2) via `frame-root-window' —
    // nil -> selected, non-frame -> `frame-live-p' wta.
    let _ = crate::editor::frame_live(i, &arg(&a, 2))?;
    // Windows carry no `window-side' etc. in this build → nil.
    Ok(Value::Nil)
}

/// Monitor attributes are captured by the GUI front-end alongside frame
/// geometry. Text-only interpreters retain the single synthetic monitor.
fn monitor_attributes_list(i: &mut Interp, frame: &FrameRef) -> Value {
    let monitor_list = i.q("display-monitor-attributes-list");
    let stored = crate::lisp::eval::plist_get_v(&frame.borrow().params, &Value::Sym(monitor_list));
    if !stored.is_nil() {
        return stored;
    }

    let width = crate::editor::frame_pixel_width(i, frame) as i128;
    let height = crate::editor::frame_pixel_height(i, frame) as i128;
    let geometry = i.q("geometry");
    let workarea = i.q("workarea");
    let mm_size = i.q("mm-size");
    let frames = i.q("frames");
    let rect = |key, width, height| {
        Value::list(vec![
            Value::Sym(key),
            Value::Int(0),
            Value::Int(0),
            Value::Int(width),
            Value::Int(height),
        ])
    };
    let attributes = Value::list(vec![
        rect(geometry, width, height),
        rect(workarea, width, height),
        Value::list(vec![Value::Sym(mm_size), Value::Nil, Value::Nil]),
        Value::list(vec![
            Value::Sym(frames),
            Value::list(vec![Value::Frame(frame.clone())]),
        ]),
    ]);
    Value::list(vec![attributes])
}

/// Return the attributes for the monitor that currently contains FRAME.
/// GNU's NS backend keeps the primary monitor first, so use that as the
/// fallback if the frame has not yet been assigned to a native display.
pub(super) fn monitor_attributes(i: &mut Interp, frame: &FrameRef) -> Value {
    let attributes = monitor_attributes_list(i, frame)
        .list_to_vec()
        .unwrap_or_default();
    let frames_key = i.q("frames");
    let frame_value = Value::Frame(frame.clone());
    for attribute in &attributes {
        let Ok(entries) = attribute.list_to_vec() else {
            continue;
        };
        let monitor_frames = entries.iter().find_map(|entry| {
            let fields = entry.list_to_vec().ok()?;
            if matches!(fields.first(), Some(Value::Sym(symbol)) if *symbol == frames_key) {
                fields.get(1).cloned()
            } else {
                None
            }
        });
        if monitor_frames
            .and_then(|frames| frames.list_to_vec().ok())
            .is_some_and(|frames| {
                frames
                    .iter()
                    .any(|candidate| eq_values(candidate, &frame_value))
            })
        {
            return attribute.clone();
        }
    }
    attributes.into_iter().next().unwrap_or(Value::Nil)
}

fn monitor_pixel_size(i: &mut Interp, frame: &FrameRef) -> Option<(i128, i128)> {
    let geometry = i.q("geometry");
    let attributes = monitor_attributes_list(i, frame).list_to_vec().ok()?;
    let mut bounds: Option<(i128, i128, i128, i128)> = None;
    for attribute in attributes {
        let entries = attribute.list_to_vec().ok()?;
        let rectangle = entries.iter().find_map(|entry| {
            let fields = entry.list_to_vec().ok()?;
            if !matches!(fields.first(), Some(Value::Sym(symbol)) if *symbol == geometry) {
                return None;
            }
            let x = fields.get(1)?.int()?;
            let y = fields.get(2)?.int()?;
            let width = fields.get(3)?.int()?;
            let height = fields.get(4)?.int()?;
            (width >= 0 && height >= 0).then_some((
                x,
                y,
                x.saturating_add(width),
                y.saturating_add(height),
            ))
        });
        if let Some((left, top, right, bottom)) = rectangle {
            bounds = Some(match bounds {
                Some((min_x, min_y, max_x, max_y)) => (
                    min_x.min(left),
                    min_y.min(top),
                    max_x.max(right),
                    max_y.max(bottom),
                ),
                None => (left, top, right, bottom),
            });
        }
    }
    let (left, top, right, bottom) = bounds?;
    Some((right.saturating_sub(left), bottom.saturating_sub(top)))
}

pub(super) fn f_frame_monitor_attributes(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let v = a.get(0).cloned().unwrap_or(Value::Nil);
    let frame = decode_display(i, &v)?;
    Ok(monitor_attributes(i, &frame))
}

pub(super) fn f_display_monitor_attributes_list(
    i: &mut Interp,
    a: crate::lisp::Argv,
) -> EvalResult {
    let v = a.get(0).cloned().unwrap_or(Value::Nil);
    let frame = decode_display(i, &v)?;
    Ok(monitor_attributes_list(i, &frame))
}

/// `frame-inner-width'/`frame-inner-height' — GNU's decode errors
/// plainly (`"X is not a live frame"') for non-frames, then the tty
/// text geometry is the frame's own width/height.
/// `frame-geometry' — GNU frame.el: `window-normalize-frame' gates
/// with the plain "X is not a live frame" error, then tty → nil.
pub(super) fn f_frame_geometry(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let _ = crate::editor::frame_live_plain(i, &arg(&a, 0))?;
    Ok(Value::Nil)
}

pub(super) fn f_frame_width_val(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let frame = crate::editor::frame_live_plain(i, &arg(&a, 0))?;
    Ok(Value::Int(crate::editor::frame_pixel_width(i, &frame) as i128))
}

pub(super) fn f_frame_height_val(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let frame = crate::editor::frame_live_plain(i, &arg(&a, 0))?;
    Ok(Value::Int(crate::editor::frame_pixel_height(i, &frame) as i128))
}

/// `frame-outer-width'/`frame-outer-height' — GNU consults
/// `frame-edges', which is nil on a tty batch frame; the arithmetic
/// then signals `wrong-type-argument number-or-marker-p nil'.
pub(super) fn f_frame_outer_dim(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let _ = crate::editor::frame_live_plain(i, &arg(&a, 0))?;
    Err(i.wrong_type_mut("number-or-marker-p", &Value::Nil))
}

/// Parse coordinate fields with guaranteed progress, rejecting malformed tails.
pub(super) fn geometry_offsets(pos: &crate::lisp::text::EmacsStr) -> Option<[Option<i32>; 2]> {
    let chars: Vec<_> = pos.chars().collect();
    let mut result = [None, None];
    let mut at = 0;
    let mut field = 0;
    while at < chars.len() {
        if field == result.len() {
            return None;
        }
        let negative = match chars[at].code() {
            43 => false,
            45 => true,
            _ => return None,
        };
        at += 1;
        let start = at;
        let limit = if negative {
            i32::MAX as u64 + 1
        } else {
            i32::MAX as u64
        };
        let mut magnitude = 0u64;
        while at < chars.len() && chars[at].is_ascii_digit() {
            magnitude = (magnitude * 10 + (chars[at].code() - 48) as u64).min(limit);
            at += 1;
        }
        if at == start {
            return None;
        }
        result[field] = Some(if negative {
            -(magnitude as i64) as i32
        } else {
            magnitude as i32
        });
        field += 1;
    }
    Some(result)
}

/// `move-to-window-line` — with no window system GNU returns 0 without
/// even type-checking ARG — but only when the selected window shows
/// the current buffer; a stray buffer context signals "called from
/// unrelated buffer" (GNU Lisp signal text, isearch.el relies on it).
pub(super) fn f_move_to_window_line(i: &mut Interp, _a: crate::lisp::Argv) -> EvalResult {
    let w = sel_window(i);
    let wb_buf = w.and_then(|w| w.borrow().buffer.into());
    let cur_id = crate::editor::cur(i).borrow().id;
    match wb_buf {
        Some(id) if id == cur_id => Ok(Value::Int(0)),
        _ => Err(i.error("move-to-window-line called from unrelated buffer")),
    }
}

/// `window-child' — first child of an internal window; nil on leaves.
pub(super) fn f_window_child(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let window = crate::editor::win_valid(i, &arg(&a, 0))?;
    Ok(window
        .borrow()
        .children
        .first()
        .map(|child| Value::Window(child.clone()))
        .unwrap_or(Value::Nil))
}

/// `window-child-count' — GNU window.el: `(and (windowp window)
/// (window-child window))' walk — non-window args yield 0, not an
/// error.  Children only exist on internal windows.
pub(super) fn f_window_child_count(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    match arg(&a, 0) {
        Value::Window(_) => {
            let window = crate::editor::win_valid(i, &arg(&a, 0))?;
            Ok(Value::Int(window.borrow().children.len() as i128))
        }
        _ => Ok(Value::Int(0)),
    }
}

/// `window-combined-p` — WINDOW is part of a combination in direction
/// HORIZONTAL (nil = vertical): returns the parent's first child (the
/// top/left window of the combination), which for the first child is
/// WINDOW itself; nil when the parent splits the other axis.
/// GNU signals a plain `error' for non-windows.
pub(super) fn f_window_combined_p(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let w = match arg(&a, 0) {
        Value::Nil => sel_window(i).ok_or_else(|| i.error("No window"))?,
        Value::Window(w) if !w.borrow().dead => w.clone(),
        other => {
            let shown = i.princ_to_string(&other);
            return Err(i.error(format!("{shown} is not a valid window")));
        }
    };
    let horiz = arg(&a, 1).truthy();
    let Some(p) = w_parent(&w) else {
        return Ok(Value::Nil);
    };
    let pb = p.borrow();
    if pb.horiz != Some(horiz) {
        return Ok(Value::Nil);
    }
    Ok(pb
        .children
        .first()
        .map(|k| Value::Window(k.clone()))
        .unwrap_or(Value::Nil))
}

/// `window-main-window` — GNU returns the frame's root window object
/// (a leaf while unsplit, the internal vc/hc window after splits).
pub(super) fn f_window_main_window(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let f = match arg(&a, 0) {
        Value::Nil => sel_frame(i),
        Value::Frame(fr) => Some(fr),
        other => {
            let shown = i.princ_to_string(&other);
            return Err(i.error(format!("{shown} is not a live frame")));
        }
    };
    let Some(f) = f else {
        return f_selected_window(i, vec![].into());
    };
    let fb = f.borrow();
    Ok(fb
        .root
        .as_ref()
        .map(|r| Value::Window(r.clone()))
        .unwrap_or(Value::Nil))
}

/// `window-fixed-size-p` — GNU windowp-checks WINDOW; nil (nothing
/// fixed in our model).
pub(super) fn f_windowp_nil(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    match arg(&a, 0) {
        Value::Nil | Value::Window(_) => Ok(Value::Nil),
        other => Err(i.wrong_type_mut("windowp", &other)),
    }
}
