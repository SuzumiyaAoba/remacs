//! Small platform/process compatibility adapters and unsupported-operation surfaces.

use super::err_sym;

use super::display::geometry_offsets;

use crate::lisp::builtins::{arg, want_list, want_string};
use crate::lisp::{Interp, error::EvalResult, obarray::sym, value::Value};

pub(super) fn f_command_modes(_i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    let _ = args;
    Ok(Value::Nil)
}

pub(super) fn f_send_string_to_terminal(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    let Value::Str(text) = &args[0] else {
        return Err(i.wrong_type_mut("stringp", &args[0]));
    };
    crate::stdio::write(&crate::lisp::builtins::strfn::gnu_sbytes(i, text));
    crate::stdio::flush();
    Ok(Value::Nil)
}

pub(super) fn f_flush_stdout(_i: &mut Interp, _a: crate::lisp::Argv) -> EvalResult {
    crate::stdio::flush();
    Ok(Value::Nil)
}

pub(super) fn f_command_line(i: &mut Interp, _a: crate::lisp::Argv) -> EvalResult {
    // GNU recurses into the top-level loop; in batch that ends in
    // excessive-lisp-nesting. Signal a plain error instead.
    Err(i.error("command-line is for interactive use"))
}

pub(super) fn f_set_output_flow_control(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // (FLOW &optional TERMINAL). No terminal objects in this port;
    // GNU signals terminal-live-p when TERMINAL is given and not a
    // live terminal.
    if let Some(term) = a.get(1) {
        let pred = i.q("terminal-live-p");
        return Err(i.signal_data(
            sym::WRONG_TYPE_ARGUMENT,
            vec![Value::Sym(pred), term.clone()],
        ));
    }
    Ok(Value::Nil)
}

pub(super) fn f_format_mode_line(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU Fformat_mode_line:
    // WINDOW (arg3) is CHECK_WINDOW (windowp) when non-nil; batch GNU
    // yields "" for everything else.
    match a.get(2) {
        Some(v) if v.truthy() && !matches!(v, Value::Window(_)) => {
            return Err(i.wrong_type_mut("windowp", v));
        }
        _ => {}
    }
    Ok(Value::string(""))
}

pub(super) fn f_byte_to_string(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let n = crate::lisp::builtins::want_fixnum(i, &a[0])?;
    if !(0..=255).contains(&n) {
        return Err(i.error("Invalid byte"));
    }
    crate::lisp::builtins::call_canonical_builtin(i, "unibyte-string", vec![a[0].clone()])
}

pub(super) fn f_get_load_suffixes(_i: &mut Interp, _a: crate::lisp::Argv) -> EvalResult {
    // macOS module suffixes first, like GNU.
    Ok(Value::list(
        [".so", ".dylib", ".elc", ".elc.gz", ".el", ".el.gz"]
            .iter()
            .map(|s| Value::string(*s))
            .collect(),
    ))
}

pub(super) fn f_daemon_initialized(i: &mut Interp, _a: crate::lisp::Argv) -> EvalResult {
    Err(i.error("This function can only be called if emacs is run as a daemon"))
}

pub(super) fn f_signal_names(_i: &mut Interp, _a: crate::lisp::Argv) -> EvalResult {
    // POSIX names in reverse signal-number order, as GNU's `signal-names'.
    const NAMES: &[&str] = &[
        "USR2", "USR1", "INFO", "WINCH", "PROF", "VTALRM", "XFSZ", "XCPU", "IO", "TTOU", "TTIN",
        "CHLD", "CONT", "TSTP", "STOP", "URG", "TERM", "ALRM", "PIPE", "SYS", "SEGV", "BUS",
        "KILL", "FPE", "EMT", "ABRT", "TRAP", "ILL", "QUIT", "INT", "HUP", "EXIT",
    ];
    Ok(Value::list(
        NAMES.iter().map(|s| Value::string(*s)).collect(),
    ))
}

/// `gui-get-selection' — GNU's Lisp definition calls
/// `gui-selection-exists-p', which is void in batch; the
/// void-function error propagates.
pub(super) fn f_gui_get_selection(i: &mut Interp, _a: crate::lisp::Argv) -> EvalResult {
    let f = Value::Sym(i.q("gui-selection-exists-p"));
    i.call_function(&f, &Value::Nil, None)
}

/// `x-open-connection' — never connects on a tty batch (nil), but
/// `xw-*' color functions consult the color database instead of
/// erroring.
pub(super) fn f_x_open_connection(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU xfns.c Fx_open_connection: CHECK_STRING(DISPLAY) runs before
    // the connection attempt — a bad arg does NOT count as "attempted"
    // for the xw-* color functions.
    if !matches!(&a[0], Value::Str(_)) {
        return Err(i.wrong_type_mut("stringp", &a[0]));
    }
    i.x_display_attempted = true;
    Ok(Value::Nil)
}

pub(super) fn f_x_close_connection(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU nsfns.m: check_ns_display_info(terminal) — a string looks up
    // the display name and fails (no multi-display NS), nil reads the
    // selected frame's terminal (errors in batch), a live frame/terminal
    // that's not NS reports "not a Nextstep display", and other atoms
    // need frame-live-p.
    match a.first() {
        Some(Value::Str(_)) => {
            Err(i.error("Emacs for Nextstep does not yet support multi-display"))
        }
        Some(Value::Nil) | None => {
            Err(i.error("Nextstep windows are not in use or not initialized"))
        }
        Some(v) if matches!(v, Value::Frame(_)) || crate::editor::is_terminal(i, v) => {
            Err(i.error("Terminal 0 is not a Nextstep display"))
        }
        Some(other) => Err(i.wrong_type_mut("frame-live-p", other)),
    }
}

/// `x-show-tip' — GNU xfns.c Fx_show_tip: CHECK_STRING(STRING),
/// then decode_live_frame(FRAME), only then the window-system check
/// ("Window system frame should be used" on a tty frame).
pub(super) fn f_x_show_tip(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    if !matches!(&a[0], Value::Str(_)) {
        return Err(i.wrong_type_mut("stringp", &a[0]));
    }
    let frame = crate::editor::frame_live(i, &arg(&a, 1))?;
    if !crate::editor::frame_is_graphical(i, &frame) {
        return Err(i.error("Window system frame should be used"));
    }
    let Value::Str(text) = &a[0] else {
        unreachable!();
    };
    i.gui_tip = Some(text.borrow().to_unicode_lossy());
    Ok(Value::Nil)
}

pub(super) fn f_x_hide_tip(i: &mut Interp, _a: crate::lisp::Argv) -> EvalResult {
    Ok(Value::from_bool(i.gui_tip.take().is_some()))
}

/// `gnutls-available-p' — GNU returns the GnuTLS capability list.
pub(super) fn f_gnutls_available_p(i: &mut Interp, _a: crate::lisp::Argv) -> EvalResult {
    const CAPS: &[&str] = &[
        "Key Share",
        "Post Handshake Auth",
        "PSK Key Exchange Modes",
        "Cookie",
        "Supported Versions",
        "Early Data",
        "Pre Shared Key",
        "Session Ticket",
        "Record Size Limit",
        "Compress Certificate",
        "Extended Master Secret",
        "Encrypt-then-MAC",
        "Server Certificate Type",
        "Client Certificate Type",
        "ALPN",
        "SRTP",
        "Signature Algorithms",
        "Supported EC Point Formats",
        "Supported Groups",
        "OCSP Status Request",
        "Maximum Record Size",
        "Server Name Indication",
        "macs",
        "AEAD-ciphers",
        "ciphers",
        "digests",
        "gnutls3",
        "ClientHello Padding",
        "gnutls",
    ];
    let vals: Vec<Value> = CAPS.iter().map(|c| Value::Sym(i.intern(c))).collect();
    Ok(Value::list(vals))
}

/// `x-get-resource' / `x-list-fonts' — no X display in batch.
pub(super) fn f_x_get_resource(i: &mut Interp, _a: crate::lisp::Argv) -> EvalResult {
    Err(i.error("Window system is not in use or not initialized"))
}

/// window-scroll-bar-*: decode_live_window → `window-live-p', 0 in batch.
pub(super) fn f_zero_winlive(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let _ = crate::editor::win_live(i, &arg(&a, 0))?;
    Ok(Value::Int(0))
}

/// The three ints in (cddr (tty-color-desc …)), or None.
pub(super) fn desc_rgb3(desc: &Value) -> Option<(i128, i128, i128)> {
    let tail = crate::lisp::builtins::listfn::nthcdr_of(desc, 2);
    let v = tail.list_to_vec().ok()?;
    match v.as_slice() {
        [Value::Int(r), Value::Int(g), Value::Int(b), ..] => Some((*r, *g, *b)),
        _ => None,
    }
}

pub(super) fn f_version_list_not_zero(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU: listp arg, first non-zero elt or 0.
    let items = match &a[0] {
        Value::Nil => Vec::new(),
        Value::Cons(_) => want_list(i, &a[0])?,
        other => return Err(i.wrong_type_mut("listp", other)),
    };
    Ok(items
        .into_iter()
        .find(|v| !matches!(v, Value::Int(0)))
        .unwrap_or(Value::Int(0)))
}

pub(super) fn f_message_box(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    crate::lisp::builtins::evalfn::f_message(i, a)
}

fn parse_geometry_size(text: &crate::lisp::text::EmacsStr) -> Option<u32> {
    if text.is_empty() {
        return None;
    }
    let mut size = 0u32;
    for c in text.chars() {
        if !c.is_ascii_digit() {
            return None;
        }
        // GNU parses through strtoul and clips the result to UINT_MAX.
        size = size
            .saturating_mul(10)
            .saturating_add(c.code() - b'0' as u32);
    }
    Some(size)
}

#[cfg(target_os = "macos")]
fn parse_nextstep_geometry(i: &mut Interp, text: &crate::lisp::text::EmacsStr) -> Value {
    let chars: Vec<_> = text.chars().collect();
    let Some(mut at) = chars.iter().position(|c| c.is_ascii_digit()) else {
        return Value::Nil;
    };
    let mut values = Vec::new();
    while values.len() < 4 {
        let start = at;
        while at < chars.len() && chars[at].is_ascii_digit() {
            at += 1;
        }
        let digits: Vec<u8> = chars[start..at].iter().map(|c| c.code() as u8).collect();
        let Some(number) = num_bigint::BigInt::parse_bytes(&digits, 10) else {
            break;
        };
        values.push(Value::from_bigint(number));
        if at + 1 >= chars.len()
            || chars[at].code() != b' ' as u32
            || !chars[at + 1].is_ascii_digit()
        {
            break;
        }
        at += 1;
    }
    Value::list(
        ["top", "left", "height", "width"]
            .into_iter()
            .zip(values)
            .map(|(name, value)| Value::cons(Value::Sym(i.intern(name)), value))
            .collect(),
    )
}

pub(super) fn f_x_parse_geometry(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU returns an alist ((height . H) (width . W) (top . Y) (left . X)).
    let s = match &a[0] {
        Value::Str(s) => s.borrow().clone(),
        other => return Err(i.wrong_type_mut("stringp", other)),
    };
    #[cfg(target_os = "macos")]
    if s.chars().any(|c| c == ' ') {
        return Ok(parse_nextstep_geometry(i, s.as_str()));
    }
    let mut w = None;
    let mut h = None;
    // The first sign starts the position part; sizes are digits separated
    // by either case of `x'.  XParseGeometry also ignores a leading `='.
    let split_at = s
        .chars()
        .position(|c| c == '+' || c == '-')
        .unwrap_or(s.len());
    let (size, pos) = s.split_at(split_at);
    let size = size.strip_prefix("=").unwrap_or(size);
    if !size.is_empty() {
        if let Some((width, height)) = size.split_once('x').or_else(|| size.split_once('X')) {
            if !width.is_empty() {
                let Some(width) = parse_geometry_size(width) else {
                    return Ok(Value::Nil);
                };
                w = Some(width);
            }
            let Some(height) = parse_geometry_size(height) else {
                return Ok(Value::Nil);
            };
            h = Some(height);
        } else {
            let Some(width) = parse_geometry_size(size) else {
                return Ok(Value::Nil);
            };
            w = Some(width);
        }
    }
    let Some([parsed_left, parsed_top]) = geometry_offsets(pos) else {
        return Ok(Value::Nil);
    };
    let left = parsed_left;
    let top = parsed_top;
    let mut items: Vec<Value> = Vec::new();
    for (name, v) in [("height", h), ("width", w)] {
        if let Some(n) = v {
            items.push(Value::cons(
                Value::Sym(i.intern(name)),
                Value::Int(i128::from(n)),
            ));
        }
    }
    for (name, v) in [("top", top), ("left", left)] {
        if let Some((n, negative_zero)) = v {
            let value = if negative_zero {
                Value::list(vec![Value::Sym(i.intern("-")), Value::Int(0)])
            } else {
                Value::Int(i128::from(n))
            };
            items.push(Value::cons(Value::Sym(i.intern(name)), value));
        }
    }
    Ok(Value::list(items))
}

pub(super) fn f_current_bidi_paragraph_direction(
    i: &mut Interp,
    a: crate::lisp::Argv,
) -> EvalResult {
    // GNU Fcurrent_bidi_paragraph_direction: nil →
    // current buffer; a non-buffer arg is `bufferp'.
    match a.first() {
        None | Some(Value::Nil) | Some(Value::Buffer(_)) => {}
        Some(other) => return Err(i.wrong_type_mut("bufferp", other)),
    }
    let buffer = match a.first() {
        Some(Value::Buffer(buffer)) => buffer.clone(),
        _ => match i.current_buffer_ref() {
            Some(buffer) => buffer,
            None => return Ok(Value::Sym(i.q("left-to-right"))),
        },
    };
    let buffer_id = buffer.borrow().id;
    let configured_direction = crate::frontend::buffer_setting(
        i,
        "bidi-paragraph-direction",
        buffer_id,
    );
    if !configured_direction.is_nil() {
        return Ok(configured_direction);
    }

    let paragraph_text = {
        let bb = buffer.borrow();
        let point = bb.point().clamp(bb.begv, bb.zv);
        let first_line = bb.text.line_of_pos(bb.begv);
        let last_line = bb.text.line_of_pos(bb.zv);
        let mut point_line = bb.text.line_of_pos(point);
        let line_text = |line: usize| {
            let start = bb.text.line_start(line).max(bb.begv);
            let end = bb.text.line_end(start).min(bb.zv);
            bb.text.substring(start, end).to_unicode_lossy()
        };
        let line_is_blank = |line: usize| line_text(line).chars().all(char::is_whitespace);
        if line_is_blank(point_line) {
            let mut previous = point_line;
            while previous > first_line && line_is_blank(previous) {
                previous -= 1;
            }
            if !line_is_blank(previous) {
                point_line = previous;
            } else if let Some(next) = (point_line.saturating_add(1)..=last_line)
                .find(|line| !line_is_blank(*line))
            {
                point_line = next;
            }
        }
        let mut start_line = point_line;
        while start_line > first_line && !line_is_blank(start_line - 1) {
            start_line -= 1;
        }
        let mut end_line = point_line;
        while end_line < last_line && !line_is_blank(end_line + 1) {
            end_line += 1;
        }
        let mut text = String::new();
        for line in start_line..=end_line {
            if line != start_line {
                text.push(' ');
            }
            text.push_str(&line_text(line));
        }
        text
    };
    let is_rtl = unicode_bidi::BidiInfo::new(&paragraph_text, None)
        .paragraphs
        .first()
        .is_some_and(|paragraph| paragraph.level.is_rtl());
    let direction = if is_rtl {
        i.q("right-to-left")
    } else {
        i.q("left-to-right")
    };
    Ok(Value::Sym(direction))
}

pub(super) fn f_bidi_string_mark_left_to_right(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let s = want_string(i, &a[0])?;
    // GNU's bidi.c: if STR contains a character of strong R or AL
    // bidi type, return STR followed by U+200E LEFT-TO-RIGHT MARK so
    // subsequent text resolves LTR; otherwise return STR unchanged.
    let rtl = s.chars().any(|ch| {
        let u = (ch).code() as u32;
        crate::lisp::builtins::bidi_table::BIDI_CLASS_TABLE
            .binary_search_by(|&(lo, hi, _)| {
                if u < lo {
                    std::cmp::Ordering::Greater
                } else if u > hi {
                    std::cmp::Ordering::Less
                } else {
                    std::cmp::Ordering::Equal
                }
            })
            .map(|idx| {
                matches!(
                    crate::lisp::builtins::bidi_table::BIDI_CLASS_TABLE[idx].2,
                    "R" | "AL"
                )
            })
            .unwrap_or(false)
    });
    Ok(if rtl {
        let mut marked = s;
        marked.push('\u{200e}');
        Value::string(marked)
    } else {
        a[0].clone()
    })
}

/// `glyph-char`/`glyph-face` — GNU Lisp accessors over glyph codes:
/// a plain char maps to itself / no face.
pub(super) fn f_glyph_char(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    match &a[0] {
        v @ Value::Int(_) => Ok(v.clone()),
        Value::Cons(c) => Ok(c.borrow().car.clone()),
        other => Err(i.wrong_type_mut("numberp", other)),
    }
}

/// `make-glyph-code` — GNU returns CHAR alone for no face, else a
/// (CHAR . FACE-ID) cons where FACE-ID is the face's index in the
/// face table (GNU's `lookup_named_face' result).
pub(super) fn f_make_glyph_code(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    match &a[0] {
        Value::Int(_) => {}
        other => return Err(i.wrong_type_mut("characterp", other)),
    }
    match a.get(1) {
        None | Some(Value::Nil) => Ok(a[0].clone()),
        Some(face) => {
            let name = match face {
                Value::Sym(s) => *s,
                _ => return Err(i.wrong_type_mut("symbolp", face)),
            };
            match i.face_table.iter().position(|(n, _)| *n == name) {
                Some(id) => Ok(Value::cons(a[0].clone(), Value::Int(id as i128))),
                // Unknown face: GNU falls back to the bare char.
                None => Ok(a[0].clone()),
            }
        }
    }
}

pub(super) fn f_process_arg_err(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let v = a.first().cloned().unwrap_or(Value::Nil);
    Err(i.wrong_type_mut("processp", &v))
}

pub(super) fn f_internal_handle_focus_in(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU validates EVENT is a (focus-in/out LIVE-FRAME) cons.
    let ok = match &a[0] {
        Value::Cons(c) => {
            let (car, cadr) = {
                let cc = c.borrow();
                let cadr = match &cc.cdr {
                    Value::Cons(d) => d.borrow().car.clone(),
                    _ => Value::Nil,
                };
                (cc.car.clone(), cadr)
            };
            let head_ok = match &car {
                Value::Sym(s) => {
                    let n = i.symbol_name(*s);
                    n == "focus-in" || n == "focus-out"
                }
                _ => false,
            };
            head_ok && matches!(cadr, Value::Frame(_))
        }
        _ => false,
    };
    if ok {
        Ok(Value::Nil)
    } else {
        Err(i.error("Invalid focus-in event"))
    }
}

pub(super) fn f_re_describe_compiled(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    match &a[0] {
        Value::Str(_) | Value::Nil => Ok(Value::Nil),
        other => Err(i.wrong_type_mut("stringp", other)),
    }
}

/// `network-lookup-address-info` — real getaddrinfo; GNU shape:
/// IPv4 → [A B C D 0], IPv6 → [S0..S7 0]; nil on lookup failure.
pub(super) fn f_network_lookup_address_info(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let host = want_string(i, &a[0])?;
    let mut want4 = true;
    let mut want6 = true;
    if let Some(fam) = a.get(1) {
        if !fam.is_nil() {
            match i
                .sym_id(fam)
                .map(|s| i.symbol_name(s).to_string())
                .as_deref()
            {
                Some("ipv4") => want6 = false,
                Some("ipv6") => want4 = false,
                _ => {
                    let e = i.q("error");
                    return Err(i.signal_data(e, vec![Value::string("Unsupported family")]));
                }
            }
        }
    }
    let hint = arg(&a, 2);
    let numeric_symbol = i.q("numeric");
    let numeric = i.sym_id(&hint) == Some(numeric_symbol);
    if hint.truthy() && !numeric {
        return Err(i.error("Unsupported hints value"));
    }
    crate::lisp::process::validate_hostname(i, &a[0])?;
    let resolved = if numeric {
        crate::lisp::process::address::numeric(&host, 0)
    } else {
        crate::lisp::process::address::resolve(&host, 0)
    };
    let addrs: Vec<std::net::SocketAddr> = resolved.map(Iterator::collect).unwrap_or_default();
    // GNU dedupes; keep first occurrences in order.
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::new();
    for sa in addrs {
        if !seen.insert(sa) {
            continue;
        }
        match sa {
            std::net::SocketAddr::V4(v4) if want4 => {
                let o = v4.ip().octets();
                out.push(Value::vector(vec![
                    Value::Int(o[0] as i128),
                    Value::Int(o[1] as i128),
                    Value::Int(o[2] as i128),
                    Value::Int(o[3] as i128),
                    Value::Int(0),
                ]));
            }
            std::net::SocketAddr::V6(v6) if want6 => {
                let mut elts: Vec<Value> = v6
                    .ip()
                    .segments()
                    .iter()
                    .map(|s| Value::Int(*s as i128))
                    .collect();
                elts.push(Value::Int(0));
                out.push(Value::vector(elts));
            }
            _ => {}
        }
    }
    Ok(Value::list(out))
}

/// `bidi-find-overridden-directionality` (FROM TO OBJECT &optional BASE-DIR):
/// GNU decodes OBJECT first — buffer/window accepted, anything else
/// non-nil is CHECK_STRING — then validate_interval_range/s u b a r r a y
/// bounds-checks FROM/TO as positions.  No bidi → nil.
pub(super) fn f_bidi_find_overridden(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let object_is_str = match &a[2] {
        Value::Nil | Value::Buffer(_) | Value::Window(_) => false,
        Value::Str(_) => true,
        other => return Err(i.wrong_type_mut("stringp", other)),
    };
    if object_is_str {
        let Value::Str(s) = &a[2] else { unreachable!() };
        // GNU bidi.c: unibyte strings are always strong LTR — nil is
        // returned BEFORE validate_subarray, so bad FROM/TO never
        // get type-checked on a unibyte object.
        if !i.is_multibyte_str(s) {
            return Ok(Value::Nil);
        }
        // validate_subarray: FROM/TO 0-based positions in the string.
        let len = s.borrow().chars().count() as i128;
        let from = crate::lisp::builtins::want_int_or_mark(i, &a[0])?;
        let to = crate::lisp::builtins::want_int_or_mark(i, &a[1])?;
        if from < 0 || to < from || to > len {
            return Err(err_sym(
                i,
                "args-out-of-range",
                vec![a[0].clone(), a[1].clone()],
            ));
        }
        return Ok(Value::Nil);
    }
    // Buffer path: FROM/TO are 1-based integer-or-marker positions.
    let from = crate::lisp::builtins::want_int_or_mark(i, &a[0])?;
    let to = crate::lisp::builtins::want_int_or_mark(i, &a[1])?;
    let (begv, zv) = {
        let b = match &a[2] {
            Value::Buffer(buf) => buf.clone(),
            _ => crate::buffer::primitives::cur(i),
        };
        let bb = b.borrow();
        (bb.begv as i128, bb.zv as i128)
    };
    if from < begv + 1 || from > zv + 1 || to < from || to > zv + 1 {
        return Err(err_sym(
            i,
            "args-out-of-range",
            vec![a[0].clone(), a[1].clone()],
        ));
    }
    Ok(Value::Nil)
}

/// `bidi-resolved-levels` — GNU fixnump-checks its arg; nil without bidi.
pub(super) fn f_bidi_resolved_levels(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    if let Some(v) = a.first() {
        if !v.is_nil() && !matches!(v, Value::Int(_)) {
            return Err(i.wrong_type_mut("fixnump", v));
        }
    }
    Ok(Value::Nil)
}

/// `module-load` — no dynamic-module support; GNU signals error with
/// the dlopen message as data.
pub(super) fn f_module_load(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let f = want_string(i, &a[0])?;
    let e = i.q("error");
    Err(i.signal_data(
        e,
        vec![
            Value::string(f.clone()),
            Value::string(format!("dlopen({}): module support not in this build", f)),
        ],
    ))
}

/// `completion--flex-cost-gotoh` — GNU's affine-gap flex cost:
/// (COST POS1 POS2 ...) or nil when NEEDLE isn't a subsequence of
/// STRING. Empirical model: 5 if the match doesn't start at 0, plus
/// 9 + gap-len per interior gap; trailing text is free.
pub(super) fn f_flex_cost_gotoh(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let needle = want_string(i, &a[0])?;
    let hay = want_string(i, &a[1])?;
    if needle.is_empty() || hay.is_empty() {
        return Ok(Value::Nil);
    }
    let hc: Vec<crate::lisp::value::EmacsChar> = hay.chars().collect();
    let mut pos: Vec<usize> = Vec::new();
    let mut from = 0usize;
    for nc in needle.chars() {
        match hc[from..].iter().position(|c| *c == nc) {
            Some(off) => {
                pos.push(from + off);
                from += off + 1;
            }
            None => return Ok(Value::Nil),
        }
    }
    let mut cost: i128 = if pos[0] > 0 { 5 } else { 0 };
    for w in pos.windows(2) {
        let gap = w[1] - w[0] - 1;
        if gap > 0 {
            cost += 9 + gap as i128;
        }
    }
    let mut out = vec![Value::Int(cost)];
    out.extend(pos.iter().map(|p| Value::Int(*p as i128)));
    Ok(Value::list(out))
}

pub(super) fn f_profiler_cpu_running_p(i: &mut Interp, _a: crate::lisp::Argv) -> EvalResult {
    Ok(if i.cpu_profiler {
        Value::t()
    } else {
        Value::Nil
    })
}
