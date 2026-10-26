//! Buffer/region compatibility adapters.

use super::chartables::char_table_ref;

use super::chartables::is_char_table;

use crate::lisp::builtins::{arg, want_sym};
use crate::lisp::{Interp, error::EvalResult, obarray::sym, value::Value};

pub(super) fn f_abort_minibuffers(i: &mut Interp, _a: crate::lisp::Argv) -> EvalResult {
    // GNU Fabort_minibuffers: not in a minibuffer → error; otherwise
    // `minibuffer-quit-recursive-edit' throws `exit' (a function that
    // signals `minibuffer-quit' after the read unwinds).
    if i.minibuf_level <= 0 {
        return Err(i.error("Not in a minibuffer"));
    }
    let qre = Value::Sym(i.q("minibuffer-quit-recursive-edit"));
    i.apply(&qre, vec![])
}

pub(super) fn f_newline_cache_check(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU Fnewline_cache_check:
    // optional BUFFER is CHECK_BUFFER; a buffer with no newline cache
    // (cache-long-scans nil — the default, and we never maintain one)
    // answers nil.
    match arg(&a, 0) {
        Value::Nil => {}
        Value::Buffer(_) => {}
        other => return Err(i.wrong_type_mut("bufferp", &other)),
    }
    Ok(Value::Nil)
}

pub(super) fn f_get_truename_buffer(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU: nil for any arg (truename cache unused in batch).
    let _ = (i, &a[0]);
    Ok(Value::Nil)
}

pub(super) fn f_find_buffer(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let sid = want_sym(i, &a[0])?;
    for id in i.buffers.list() {
        let v = match i.buffers.get(id) {
            Some(b) => match b.borrow().locals.get(&sid) {
                Some(v) => v.clone(),
                None => i.obarray.symbol(sid).value.clone(),
            },
            None => continue,
        };
        if matches!(v, Value::Sym(s) if s == sym::UNBOUND) {
            return Err(i.signal_data(sym::VOID_VARIABLE, vec![a[0].clone()]));
        }
        if crate::lisp::builtins::equal_values(i, &v, &a[1])? {
            if let Some(b) = i.buffers.get(id) {
                return Ok(Value::Buffer(b));
            }
        }
    }
    Ok(Value::Nil)
}

pub(super) fn f_buffer_text_pixel_size(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU xdisp.c Fbuffer_text_pixel_size (BUFFER-OR-NAME WINDOW
    // X-LIMIT Y-LIMIT): decode_live_window(WINDOW) FIRST — wta
    // `window-live-p' on non-windows — then Fget_buffer on
    // BUFFER-OR-NAME (wta `stringp' on non-buffer non-string; a missing
    // name aborts in GNU — we signal cleanly).  X-LIMIT/Y-LIMIT are
    // never type-checked.
    let w = crate::editor::win_live(i, &arg(&a, 1))?;
    let buf = match &arg(&a, 0) {
        Value::Nil => i.current_buffer_ref(),
        Value::Buffer(b) => Some(b.clone()),
        Value::Str(name) => {
            let n = name.borrow().clone();
            match i.buffers.by_name(&n).and_then(|id| i.buffers.get(id)) {
                Some(b) => Some(b),
                None => return Err(i.error(format!("No such buffer: ‘{}’", n))),
            }
        }
        other => return Err(i.wrong_type_mut("stringp", other)),
    };
    // GNU window_text_pixel_size on the batch tty (a column and a line
    // are one pixel each): an X-LIMIT fixnum in [0, INT_MAX] wraps
    // screen lines at that column, any other non-nil X-LIMIT means no
    // limit, and nil wraps at the window's text width (its last column
    // holds the continuation glyph).  Y-LIMIT nil is unlimited, a
    // fixnum in [0, INT_MAX] caps the height, and any other value
    // leaves GNU's max_y at 0.  With a non-nil X-LIMIT the iterator
    // also moves across the screen line starting at max_y, so that
    // line still counts toward the width.
    const INT_MAX: i128 = i32::MAX as i128;
    let wrap = match arg(&a, 2) {
        Value::Int(n) if (0..=INT_MAX).contains(&n) => Some(n as usize),
        Value::Nil => Some(w.borrow().width.saturating_sub(1).max(1)),
        _ => None,
    };
    let max_y = match arg(&a, 3) {
        Value::Nil => usize::MAX,
        Value::Int(n) if (0..=INT_MAX).contains(&n) => n as usize,
        _ => 0,
    };
    let measure_y = max_y.saturating_add(usize::from(arg(&a, 2).truthy()));
    let Some(b) = buf else {
        return Ok(Value::cons(Value::Int(0), Value::Int(0)));
    };
    let (text, tab_width, ctl_arrow) = {
        let bb = b.borrow();
        let local = |name: &str| {
            let id = i.intern_soft(name)?;
            match bb.locals.get(&id) {
                Some(v) if !matches!(v, Value::Sym(s) if *s == sym::UNBOUND) => Some(v.clone()),
                _ => Some(i.obarray.symbol(id).value.clone()),
            }
        };
        // GNU SANE_TAB_WIDTH: a fixnum in 1..=1000, else 8.
        let tab_width = match local("tab-width") {
            Some(Value::Int(n)) if (1..=1000).contains(&n) => n as usize,
            _ => 8,
        };
        let ctl_arrow = local("ctl-arrow").map_or(true, |v| v.truthy());
        (
            bb.text.substring(bb.begv, bb.text_len()),
            tab_width,
            ctl_arrow,
        )
    };
    // Zero-width chars with a `glyphless-char-display' method draw as
    // that method on the tty: `thin-space' (the default for format
    // controls and variation selectors) takes one column.  A cons
    // entry is (GRAPHICAL . TEXT).
    let glyphless = i
        .intern_soft("glyphless-char-display")
        .map(|id| i.obarray.symbol(id).value.clone())
        .filter(|t| is_char_table(i, t));
    let thin_space = i.q_soft("thin-space");
    let glyph_width = |c: crate::lisp::text::EmacsChar| -> usize {
        let u = c.code();
        if crate::lisp::value::eight_bit_byte(c).is_some() || (0x80..=0x9F).contains(&u) {
            return 4;
        }
        if u < 0x20 || u == 0x7F {
            return if ctl_arrow { 2 } else { 4 };
        }
        let cw = crate::buffer::primitives::char_width(c).max(0) as usize;
        if cw == 0 {
            if let Some(t) = &glyphless {
                let method = match char_table_ref(i, t, u as usize) {
                    Value::Cons(m) => m.borrow().cdr.clone(),
                    m => m,
                };
                if matches!(method, Value::Sym(s) if Some(s) == thin_space) {
                    return 1;
                }
            }
        }
        cw
    };
    let (mut width, mut lines) = (0usize, 0usize);
    let segs: Vec<&crate::lisp::text::EmacsStr> = text.split('\n').collect();
    // The empty line after a final newline takes no screen line.
    let n_logical = segs.len() - usize::from(segs.last().map_or(true, |s| s.is_empty()));
    'lines: for seg in &segs[..n_logical] {
        let (mut x, mut line_x) = (0usize, 0usize);
        for c in seg.chars() {
            let cw = |x: usize| {
                if c == '\t' {
                    tab_width - x % tab_width
                } else {
                    glyph_width(c)
                }
            };
            let mut adv = cw(x);
            if let Some(wr) = wrap {
                if x > 0 && x + adv > wr {
                    // Continued screen line: it spans the full wrap width.
                    if lines >= measure_y {
                        break 'lines;
                    }
                    lines += 1;
                    width = width.max(wr);
                    x = 0;
                    line_x = 0;
                    adv = cw(0);
                }
                if x == 0 && c == '\t' && adv > wr && wr > 0 {
                    // A tab wider than the limit at a line start runs
                    // past the edge; the next glyph lands at its
                    // column modulo the limit, on the same screen line.
                    line_x = line_x.max(adv);
                    x = adv % wr;
                    continue;
                }
            }
            x += adv;
            line_x = line_x.max(x);
        }
        if lines >= measure_y {
            break;
        }
        lines += 1;
        width = width.max(wrap.map_or(line_x, |wr| line_x.min(wr)));
    }
    Ok(Value::cons(
        Value::Int(width as i128),
        Value::Int(lines.min(max_y) as i128),
    ))
}

/// `shrink-window-if-larger-than-buffer' — GNU window.el: live-window
/// gate via `window-normalize-window w t' (plain "N is not a live
/// window" error), then only shrinks when the window is taller than
/// its buffer — nil in batch.
pub(super) fn f_shrink_if_larger(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let _ = crate::editor::win_err_live(i, &arg(&a, 0))?;
    Ok(Value::Nil)
}

pub(super) fn f_minibuffer_depth(i: &mut Interp, _a: crate::lisp::Argv) -> EvalResult {
    // GNU: make_fixnum (minibuf_level).
    Ok(Value::Int(i.minibuf_level.max(0) as i128))
}

pub(super) fn f_bufferpos_to_filepos(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU's Lisp (mule-util.el)
    // runs `coding-system-eol-type'/`coding-system-type'/`coding-system-base'
    // on CODING-SYSTEM before POSITION is coerced — each wants a symbol.
    if let Some(q) = a.get(2) {
        if !matches!(q, Value::Sym(_)) {
            return Err(i.wrong_type_mut("symbolp", q));
        }
    }
    if let Some(q) = a.get(1) {
        if !matches!(q, Value::Sym(_)) {
            return Err(i.wrong_type_mut("symbolp", q));
        }
    }
    let pos = match &a[0] {
        // GNU CHECK_NUMBER_COERCE_MARKER accepts only ints and markers —
        // floats land on the same integer-or-marker-p error.
        Value::Int(n) => *n,
        Value::Marker(m) => m.borrow().position as i128,
        other => return Err(i.wrong_type_mut("integer-or-marker-p", other)),
    };
    // Unibyte buffers: file byte = (clamped position) - 1.
    let zv = i
        .current_buffer_ref()
        .map(|b| b.borrow().text_len())
        .unwrap_or(0) as i128
        + 1;
    Ok(Value::Int((pos.clamp(1, zv)) - 1))
}

pub(super) fn f_filepos_to_bufferpos(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let pos = match &a[0] {
        // GNU CHECK_NUMBER_COERCE_MARKER: marker → position; other
        // non-numbers signal number-or-marker-p.
        Value::Int(n) => *n,
        Value::Marker(m) => m.borrow().position as i128,
        other => return Err(i.wrong_type_mut("number-or-marker-p", other)),
    };
    let zv = i
        .current_buffer_ref()
        .map(|b| b.borrow().text_len())
        .unwrap_or(0) as i128
        + 1;
    Ok(if pos + 1 <= zv {
        Value::Int(pos + 1)
    } else {
        Value::Nil
    })
}

pub(super) fn f_set_buffer_multibyte(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU Fset_buffer_multibyte: forbidden on indirect buffers — their
    // multibyte flag belongs to the shared text's owner (base).
    if let Some(b) = i.current_buffer_ref() {
        if b.borrow().base_buffer.is_some() {
            return Err(i.error_q::<str>(
                "Cannot do `set-buffer-multibyte' on an indirect buffer",
                &[],
            ));
        }
    }
    // GNU Fset_buffer_multibyte flips the current buffer's
    // `enable-multibyte-characters' local and returns the flag.
    let sym = i.q("enable-multibyte-characters");
    i.set_symbol(sym, a[0].clone())?;
    if let Some(buffer) = i.current_buffer_ref() {
        buffer.borrow().text.set_multibyte(a[0].truthy());
    }
    Ok(a[0].clone())
}

/// `window-safely-shrinkable-p` — batch tty: always safe → t.
pub(super) fn f_safely_shrinkable(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    match arg(&a, 0) {
        Value::Nil | Value::Window(_) => Ok(Value::t()),
        other => Err(i.wrong_type_mut("window-valid-p", &other)),
    }
}
