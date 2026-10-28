//! Face, font, colour and fringe compatibility primitives.

use super::display::decode_display;

use super::compat::desc_rgb3;

use super::display::want_opt_frame;

use crate::lisp::builtins::{arg, want_string, want_sym};
use crate::lisp::{
    Interp,
    error::{EvalResult, Flow},
    obarray::sym,
    value::{SymId, Value},
};

/// Whether V names a face (symbol or string), like GNU's face lookup.
pub(super) fn face_exists(i: &Interp, value: &Value) -> bool {
    crate::editor::face_symbol(i, value)
        .and_then(|face| crate::editor::face_resolve(i, &face))
        .is_some()
}

/// `face-name` — GNU faces.el returns the face's name as a *string*
/// (`symbol-name`), rejects string args with `wrong-type-argument
/// (symbolp ...)', and errors "Not a face: X" on anything else that
/// doesn't name an existing face.
pub(super) fn f_face_name(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU: strings (and other non-symbols) signal "Not a face: X".
    if let Some(s) = i.sym_id(&a[0]) {
        if face_exists(i, &a[0]) {
            return crate::lisp::builtins::data::f_symbol_name(i, vec![Value::Sym(s)].into());
        }
    }
    Err(i.error(format!("Not a face: {}", i.princ_to_string(&a[0]))))
}

pub(super) fn f_face_font(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let frame_value = a.get(1).cloned().unwrap_or(Value::Nil);
    want_opt_frame(i, &frame_value)?;
    if !face_exists(i, &a[0]) {
        return Err(crate::editor::invalid_face(i, &a[0]));
    }
    let frame = match &frame_value {
        Value::Nil => crate::editor::sel_frame(i),
        Value::Frame(frame) => Some(frame.clone()),
        _ => None,
    };
    if let Some(frame) = frame.filter(|frame| crate::editor::frame_is_graphical(i, frame)) {
        let font = i.intern_soft("font");
        let font = font
            .map(|font| {
                crate::lisp::eval::plist_get_v(&frame.borrow().params, &Value::Sym(font))
            })
            .filter(|font| !font.is_nil());
        return Ok(font.unwrap_or_else(|| Value::string("Menlo")));
    }
    // Text terminals have no scalable font objects.
    Ok(Value::Nil)
}

pub(super) fn f_invert_face(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    want_opt_frame(i, a.get(1).unwrap_or(&Value::Nil))?;
    if !face_exists(i, &a[0]) {
        return Err(crate::editor::invalid_face(i, &a[0]));
    }
    // GNU internal-set-lisp-face-attribute path: swap the stored
    // :foreground/:background spec values (`unspecified' is legal and
    // stays unspecified on both sides).
    let name = crate::editor::face_symbol(i, &a[0])
        .and_then(|face| crate::editor::face_resolve(i, &face))
        .unwrap();
    let fg_kw = i.q(":foreground");
    let bg_kw = i.q(":background");
    let _unsp = i.q("unspecified");
    let getv = |t: &Vec<(SymId, Value)>, n: &SymId, kw, miss: &str| -> Value {
        t.iter()
            .find(|(f, _)| f == n)
            .map(|(_, p)| crate::lisp::eval::plist_get(p, kw))
            .map(|v| if v.is_nil() { Value::string(miss) } else { v })
            .unwrap_or_else(|| Value::string(miss))
    };
    // GNU's tty realizes unset fg/bg as the "unspecified-fg"/
    // "unspecified-bg" pseudo-strings; unset slots swap those.
    let fg = getv(&i.face_table, &name, fg_kw, "unspecified-fg");
    let bg = getv(&i.face_table, &name, bg_kw, "unspecified-bg");
    if let Some((_, plist)) = i.face_table.iter_mut().find(|(n, _)| *n == name) {
        *plist = crate::lisp::eval::plist_put(plist, fg_kw, bg);
        *plist = crate::lisp::eval::plist_put(plist, bg_kw, fg);
    }
    Ok(a[0].clone())
}

/// `window-font-width' / `window-font-height' — use the window's
/// default frame font metrics (the GPUI font is shared by all windows).
fn window_font_metric(
    i: &mut Interp,
    a: crate::lisp::Argv,
    horizontal: bool,
) -> EvalResult {
    let window = match arg(&a, 0) {
        Value::Nil => crate::editor::sel_window(i),
        Value::Window(window) => Some(window),
        other => {
            let shown = i.princ_to_string(&other);
            return Err(i.error(format!("{shown} is not a live window")));
        }
    };
    let frame = window
        .as_ref()
        .and_then(|window| crate::editor::frame_of_window(i, window))
        .or_else(|| crate::editor::sel_frame(i));
    let Some(frame) = frame else {
        return Ok(Value::Int(1));
    };
    let metric = if horizontal {
        crate::editor::frame_char_width(i, &frame)
    } else {
        crate::editor::frame_char_height(i, &frame)
    };
    Ok(Value::Int(metric as i128))
}

pub(super) fn f_window_font_width(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    window_font_metric(i, a, true)
}

pub(super) fn f_window_font_height(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    window_font_metric(i, a, false)
}

/// `x-select-font' on an NS build: GNU decodes the optional FRAME
/// with decode_live_frame first (frame-live-p), then the NS font
/// panel requires a window-system frame, so batch signals
/// `window-system-frame'.
pub(super) fn f_select_font_err(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let _ = crate::editor::frame_live(i, &arg(&a, 0))?;
    Err(i.error("Window system frame should be used"))
}

/// `face-documentation' — GNU reads the symbol's `face-documentation'
/// property; nil for unknown faces (no facep check).
pub(super) fn f_face_documentation(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let id = want_sym(i, &a[0])?;
    let prop = i.q("face-documentation");
    Ok(i.get_prop(id, prop))
}

/// `xw-color-defined-p' — errors until an X connection was tried,
/// then answers from the standard color table.
pub(super) fn f_xw_color_defined_p(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    if !i.x_display_attempted {
        return Err(i.error("Window system is not in use or not initialized"));
    }
    match &a[0] {
        Value::Str(_) => Ok(Value::from_bool(parse_color_16(&a[0]).is_some())),
        _ => Ok(Value::Nil),
    }
}

/// `internal-make-lisp-face' — creates the face (GNU registers it in
/// the face table) and returns the fresh face vector
/// `[face unspecified ...]' with 19 attribute slots.
pub(super) fn f_internal_make_lisp_face(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let id = want_sym(i, &a[0])?;
    let name = id;
    if !i.face_table.iter().any(|(n, _)| *n == name) {
        // GNU registers the NAME but creates no lface — so
        // `internal-lisp-face-empty-p' (lface_from_face_name) still
        // errors "Invalid face" on it, unlike `make-face' faces.
        // The `remacs--raw-face' sentinel distinguishes the two.
        let sentinel = Value::Sym(i.q("remacs--raw-face"));
        i.face_table.push((name.clone(), sentinel));
    }
    crate::editor::face_register_global(i, &name);
    let mut v = vec![Value::Sym(i.q("face"))];
    let unspec = Value::Sym(i.q("unspecified"));
    for _ in 0..19 {
        v.push(unspec.clone());
    }
    Ok(Value::vector(v))
}

/// `internal-copy-lisp-face' — GNU Finternal_copy_lisp_face order:
/// CHECK_SYMBOL(FROM), CHECK_SYMBOL(TO), then FRAME: `t' copies the
/// global definition; otherwise CHECK_LIVE_FRAME(FRAME) and
/// CHECK_LIVE_FRAME(NEW-FRAME, defaulting to FRAME) — nil is NOT a live
/// frame here.  Unknown FROM signals (error "Invalid face" FROM)
/// before TO is created.
pub(super) fn f_internal_copy_lisp_face(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let from = want_sym(i, &a[0])?;
    let to = want_sym(i, &a[1])?;
    let frame = arg(&a, 2);
    let global = matches!(frame, Value::Sym(s) if s == sym::T);
    if !global {
        let nf = arg(&a, 3);
        let nf = if nf.is_nil() { frame.clone() } else { nf };
        match &frame {
            Value::Frame(f) if !f.borrow().dead => {}
            other => return Err(i.wrong_type_mut("frame-live-p", other)),
        }
        match &nf {
            Value::Frame(f) if !f.borrow().dead => {}
            other => return Err(i.wrong_type_mut("frame-live-p", other)),
        }
    }
    let from_name = from;
    if !crate::editor::face_known(i, &from_name) {
        return Err(i.signal_data(
            sym::ERROR,
            vec![Value::string("Invalid face"), a[0].clone()],
        ));
    }
    let to_name = to;
    if !i.face_table.iter().any(|(n, _)| *n == to_name) {
        i.face_table.push((to_name.clone(), Value::Nil));
    }
    // GNU Finternal_copy_lisp_face: TO's frame face receives FROM's
    // attributes (our spec/realized layer = face_table plist).
    crate::editor::face_register_global(i, &to_name);
    let src = i
        .face_table
        .iter()
        .find(|(n, _)| *n == from_name)
        .map(|(_, p)| p.clone())
        .unwrap_or(Value::Nil);
    if let Some((_, dst)) = i.face_table.iter_mut().find(|(n, _)| *n == to_name) {
        *dst = src;
    }
    Ok(a[1].clone())
}

/// `font-at' — GNU checks that WINDOW (default selected) displays
/// the current buffer, then bounds-checks POSITION against the
/// buffer (1-based) or STRING (0-based).  tty text has no font.
pub(super) fn f_font_at(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU font.c: decode_live_window(WINDOW) first (wta window-live-p),
    // then the STRING branch: CHECK_FIXNUM(position) + CHECK_STRING;
    // else the window's buffer must BE the current buffer, and
    // fix_position(pos) accepts ints/markers (integer-or-marker-p).
    let w = crate::editor::win_live(i, &arg(&a, 1))?;
    let aoor = |i: &mut Interp, v: Vec<Value>| {
        let s = i.q("args-out-of-range");
        i.signal_data(s, v)
    };
    if !arg(&a, 2).is_nil() {
        let pos = crate::lisp::builtins::want_fixnum(i, &a[0])?;
        let s = want_string(i, &a[2])?;
        let len = s.chars().count() as i128;
        if pos < 0 || pos >= len {
            return Err(aoor(i, vec![Value::string(s.clone()), Value::Int(pos)]));
        }
        return Ok(Value::Nil);
    }
    let wid = w.borrow().buffer;
    let cur = i.buffers.get(i.current_buffer);
    if cur.as_ref().map(|b| b.borrow().id) != Some(wid) {
        return Err(i.error("Specified window is not displaying the current buffer"));
    }
    let pos = crate::lisp::builtins::want_int_or_mark(i, &a[0])?;
    let (begv, zv) = match &cur {
        Some(b) => {
            let bb = b.borrow();
            (bb.begv as i128 + 1, bb.text_len() as i128 + 1)
        }
        None => (1, 1),
    };
    if pos < begv || pos >= zv {
        return Err(aoor(
            i,
            vec![a[0].clone(), Value::Int(begv), Value::Int(zv)],
        ));
    }
    Ok(Value::Nil)
}

/// `font-info' — needs a window-system frame; GNU always errors on
/// a tty.
pub(super) fn f_font_info(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU font.c Ffont_info:
    // CHECK_STRING(NAME) first, then the optional FRAME decodes via
    // frame-live-p — both before the batch winsys error.
    let _ = want_string(i, &a[0])?;
    if let Some(v) = a.get(1) {
        match v {
            Value::Nil => {}
            Value::Frame(f) if !f.borrow().dead => {}
            other => return Err(i.wrong_type_mut("frame-live-p", other)),
        }
    }
    Err(i.error("Window system frame should be used"))
}

/// `font-match-p' — both args are font-specs in GNU (typed check);
/// matching two arbitrary specs is trivially true for us.
pub(super) fn f_font_match_p(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    for v in &a[..2] {
        if v.record_kind() != Some(crate::lisp::value::RecordKind::FontSpec) {
            return Err(i.wrong_type_mut("font-spec", v));
        }
    }
    Ok(Value::t())
}

/// `font-face-attributes' — GNU font.c accepts a font name string
/// (mapped to `(:family NAME)'), a font-spec (its core properties are
/// reported, :size → :height ×10), and other objects signal
/// "Invalid font object OBJ".
pub(super) fn invalid_font_object(i: &mut Interp, v: &Value) -> Flow {
    // GNU font.c: (error "Invalid font object" OBJ) — the object is a
    // separate signal datum, not part of the format string.
    let err = i.q("error");
    i.signal_data(err, vec![Value::string("Invalid font object"), v.clone()])
}

pub(super) fn f_font_face_attributes(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU Ffont_face_attributes decodes FRAME (arg2) via
    // decode_live_frame before CHECK_FONTing arg1.
    if a.len() > 1 {
        match &a[1] {
            Value::Nil => {}
            Value::Frame(f) if !f.borrow().dead => {}
            other => return Err(i.wrong_type_mut("frame-live-p", &other)),
        }
    }
    match &a[0] {
        Value::Str(_) => {
            return Ok(Value::list(vec![Value::Sym(i.q(":family")), a[0].clone()]));
        }
        Value::Record(r) => {
            let fields = r.borrow().clone();
            // Our font-spec stores its property plist in slot 1.
            let is_spec = a[0].record_kind() == Some(crate::lisp::value::RecordKind::FontSpec);
            if !is_spec {
                return Err(invalid_font_object(i, &a[0]));
            }
            let plist = fields.get(1).cloned().unwrap_or(Value::Nil);
            let mut out = Vec::new();
            let mut cur = plist;
            while let Value::Cons(c) = cur {
                let (k, rest) = {
                    let b = c.borrow();
                    (b.car.clone(), b.cdr.clone())
                };
                if let Value::Cons(c2) = rest {
                    let (v, next) = {
                        let b = c2.borrow();
                        (b.car.clone(), b.cdr.clone())
                    };
                    let kname = match &k {
                        Value::Sym(s) => i.symbol_name(*s).to_string(),
                        _ => String::new(),
                    };
                    let mapped = match kname.as_str() {
                        ":family" => Some((":family", v)),
                        ":weight" => Some((":weight", v)),
                        ":slant" => Some((":slant", v)),
                        ":width" => Some((":width", v)),
                        ":size" => match &v {
                            Value::Int(n) => Some((":height", Value::Int(n * 10))),
                            Value::Float(f) => Some((":height", Value::float(**f * 10.0))),
                            _ => None,
                        },
                        ":foundry" => Some((":foundry", v)),
                        ":registry" => Some((":registry", v)),
                        _ => None,
                    };
                    if let Some((k2, v2)) = mapped {
                        // GNU interns string values of the name slots into
                        // symbols... no: :family stays a string, as in
                        // `(:family "a")'.  Symbols pass through.
                        out.push(Value::Sym(i.intern(k2)));
                        out.push(v2);
                    }
                    cur = next;
                } else {
                    break;
                }
            }
            return Ok(Value::list(out));
        }
        _ => {}
    }
    Err(invalid_font_object(i, &a[0]))
}

/// `font-has-char-p' — typed `font' check, then nil (no fonts on a
/// tty).
pub(super) fn f_font_has_char_p(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    if !matches!(a[0], Value::Record(_)) {
        return Err(i.wrong_type_mut("font", &a[0]));
    }
    Ok(Value::Nil)
}

/// `font-get-glyphs' / `font-variation-glyphs' — GNU requires a real
/// font object (font-spec does not qualify).
pub(super) fn f_font_object_stub(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let fo = i.q("font-object");
    let ok = match &a[0] {
        Value::Record(r) => i.sym_is(&r.borrow()[0], fo),
        _ => false,
    };
    if !ok {
        return Err(i.wrong_type_mut("font-object", &a[0]));
    }
    Ok(Value::Nil)
}

/// `font-shape-gstring' — GNU checks a glyph-string (vector).
pub(super) fn f_font_shape_gstring(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU font.c requires a real gstring — a vector of LGLYPH records
    // produced by composition-get-gstring.  At -Q no lgstring can be
    // built, so every arg is invalid (a plain vector errors too).
    Err(i.signal_data(
        sym::ERROR,
        vec![
            Value::string("Invalid glyph-string: ".to_string()),
            a[0].clone(),
        ],
    ))
}

/// `set-frame-font' — GNU's third argument (FRAMES) is a list.
pub(super) fn f_set_frame_font(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    match arg(&a, 2) {
        Value::Nil | Value::Cons(_) => Ok(Value::Nil),
        other => Err(i.wrong_type_mut("listp", &other)),
    }
}

/// `destroy-fringe-bitmap' — the name must be a symbol.
pub(super) fn f_destroy_fringe_bitmap(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    match &a[0] {
        Value::Sym(_) => Ok(Value::Nil),
        other => Err(i.wrong_type_mut("symbolp", other)),
    }
}

/// `set-fringe-bitmap-face' — symbolp name, then GNU fails because
/// no fringe bitmaps are defined.
pub(super) fn f_set_fringe_bitmap_face(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    match &a[0] {
        Value::Sym(_) => Err(i.error("Undefined fringe bitmap")),
        other => Err(i.wrong_type_mut("symbolp", other)),
    }
}

/// `internal-set-alternative-font-family-alist' — each element's car
/// must be a string (GNU stores family-name → alternative list).
pub(super) fn f_set_alt_font_family_alist(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU CHECK_LIST first — non-nil non-cons signals listp.
    if !matches!(a[0], Value::Nil | Value::Cons(_)) {
        return Err(i.wrong_type_mut("listp", &a[0]));
    }
    let mut cur = a[0].clone();
    while let Value::Cons(c) = cur {
        let (elt, rest) = (c.borrow().car.clone(), c.borrow().cdr.clone());
        let car = match &elt {
            Value::Cons(e) => e.borrow().car.clone(),
            other => other.clone(),
        };
        if !matches!(car, Value::Str(_)) {
            return Err(i.wrong_type_mut("stringp", &car));
        }
        cur = rest;
    }
    Ok(Value::Nil)
}

/// `internal-set-alternative-font-registry-alist' — each element's
/// car is char-or-string-p.
pub(super) fn f_set_alt_font_registry_alist(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    if !matches!(a[0], Value::Nil | Value::Cons(_)) {
        return Err(i.wrong_type_mut("listp", &a[0]));
    }
    let mut cur = a[0].clone();
    while let Value::Cons(c) = cur {
        let (elt, rest) = (c.borrow().car.clone(), c.borrow().cdr.clone());
        let car = match &elt {
            Value::Cons(e) => e.borrow().car.clone(),
            other => other.clone(),
        };
        if !matches!(car, Value::Str(_) | Value::Int(_)) {
            return Err(i.wrong_type_mut("char-or-string-p", &car));
        }
        cur = rest;
    }
    Ok(Value::Nil)
}

/// `internal-set-lisp-face-attribute-from-resource' — GNU xfaces.c
/// arg order: CHECK_SYMBOL(FACE), CHECK_SYMBOL(ATTR), CHECK_STRING(VALUE),
/// then the resource value is decoded per ATTR name (unknown attrs pass
/// straight through to `internal-set-lisp-face-attribute', which is
/// where "Invalid face attribute name" comes from).
pub(super) fn f_set_lisp_face_attr_resource(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    const ATTRS: &[&str] = &[
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
        ":bold",
        ":italic",
    ];
    if i.sym_id(&a[0]).is_none() {
        return Err(i.wrong_type_mut("symbolp", &a[0]));
    }
    // GNU CHECK_SYMBOLs ATTR, then CHECK_STRINGs VALUE — both before
    // the attribute-name test inside internal-set-lisp-face-attribute.
    if i.sym_id(&a[1]).is_none() {
        return Err(i.wrong_type_mut("symbolp", &a[1]));
    }
    let _v = want_string(i, &a[2])?;
    let attr_id = i.sym_id(&a[1]).unwrap();
    if !ATTRS.contains(&i.symbol_name(attr_id).as_str()) {
        // GNU signals (error "Invalid face attribute name" ATTR) —
        // structured signal data, not a message-formatted string.
        return Err(i.signal_data(
            sym::ERROR,
            vec![Value::string("Invalid face attribute name"), a[1].clone()],
        ));
    }
    Ok(a[0].clone())
}

pub(super) fn f_display_color_cells(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let _ = decode_display(i, &arg(&a, 0))?;
    Ok(Value::Int(0))
}

/// `(tty-color-desc COLOR FRAME)' — the prelude Lisp entry; returns
/// (NAME INDEX R G B) or nil.  `frame' is the already-validated frame
/// arg (nil = selected), forwarded verbatim like GNU's C callers do.
pub(crate) fn tty_color_desc(i: &mut Interp, color: &Value, frame: &Value) -> Result<Value, Flow> {
    let f = Value::Sym(i.q("tty-color-desc"));
    i.apply(&f, vec![color.clone(), frame.clone()])
}

/// GNU xfaces.c `face_color_supported_p' on a tty frame:
/// `tty_defined_color' — "" succeeds vacuously, the unspecified-fg/-bg
/// names map to the terminal defaults, everything else needs a
/// fixnum INDEX from `tty-color-desc'.
pub(super) fn tty_color_supported(
    i: &mut Interp,
    color: &str,
    frame: &Value,
) -> Result<bool, Flow> {
    // GNU tty_defined_color checks `*color_name' — the C string's
    // first byte, so a Lisp "\0xxx" reads as empty: the default
    // black pixel is installed and status comes back true.
    if color.is_empty()
        || color.starts_with('\0')
        || color == "unspecified-fg"
        || color == "unspecified-bg"
    {
        return Ok(true);
    }
    let desc = tty_color_desc(i, &Value::string(color), frame)?;
    let idx = match crate::lisp::builtins::listfn::nthcdr_of(&desc, 1) {
        Value::Cons(c) => c.borrow().car.clone(),
        _ => Value::Nil,
    };
    Ok(matches!(idx, Value::Int(_)))
}

/// `color-supported-p' — GNU xfaces.c: CHECK_STRING (COLOR), then
/// decode_any_frame (FRAME), then the tty lookup.
pub(super) fn f_color_defined_p(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let s = match &a[0] {
        Value::Str(s) => s.borrow().clone(),
        other => return Err(i.wrong_type_mut("stringp", other)),
    };
    let frame_v = arg(&a, 1);
    let _ = crate::editor::frame_any(i, &frame_v)?;
    Ok(Value::from_bool(tty_color_supported(
        i,
        &(s).to_unicode_lossy(),
        &frame_v,
    )?))
}

/// `color-gray-p' — GNU xfaces.c `face_color_gray_p': CHECK_STRING,
/// decode_any_frame, then the defined-color's RGB distance from the
/// gray diagonal (any near-black color counts too).
pub(super) fn f_color_gray_p(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let s = match &a[0] {
        Value::Str(s) => s.borrow().clone(),
        other => return Err(i.wrong_type_mut("stringp", other)),
    };
    let frame_v = arg(&a, 1);
    let _ = crate::editor::frame_any(i, &frame_v)?;
    // tty_defined_color on "" leaves {0,0,0} → gray; otherwise the
    // description's RGB comes from tty-color-desc (nil → not gray).
    let rgb = if s.is_empty() || s.starts_with('\0') {
        // GNU tty_defined_color reads `*color_name' — a leading NUL
        // truncates the C view, so "\0xxx" lands on the {0,0,0}
        // default like "" does.
        Some((0, 0, 0))
    } else {
        let desc = tty_color_desc(i, &a[0], &frame_v)?;
        desc_rgb3(&desc)
    };
    let ok = match rgb {
        Some((r, g, b)) => {
            (r < 5000 && g < 5000 && b < 5000)
                || ((r - g).abs() < std::cmp::max(r, g) / 20
                    && (g - b).abs() < std::cmp::max(g, b) / 20
                    && (b - r).abs() < std::cmp::max(b, r) / 20)
        }
        None => false,
    };
    Ok(Value::from_bool(ok))
}

/// Parse a color to 16-bit (0-65535) RGB like GNU: "#rgb"/"#rrggbb"
/// strings, the 8 standard color names, or a (R G B) list.
pub(super) fn color_hex_value(text: &crate::lisp::text::EmacsStr) -> Option<i64> {
    if text.is_empty() {
        return None;
    }
    text.chars().try_fold(0i64, |value, character| {
        if !character.is_ascii_hexdigit() {
            return None;
        }
        value
            .checked_mul(16)?
            .checked_add(character.to_digit(16)? as i64)
    })
}

pub(crate) fn parse_color_16(v: &Value) -> Option<(i64, i64, i64)> {
    if let Value::Str(s) = v {
        let t = s.borrow().clone();
        let h = t.trim_start_matches('#');
        if t.starts_with('#') && h.len() == 6 {
            let r = color_hex_value(&h[0..2])? * 257;
            let g = color_hex_value(&h[2..4])? * 257;
            let b = color_hex_value(&h[4..6])? * 257;
            return Some((r, g, b));
        }
        if t.starts_with('#') && h.len() == 3 {
            let mut it = h.chars().filter_map(|c| c.to_digit(16));
            let (r, g, b) = (it.next()?, it.next()?, it.next()?);
            return Some((
                (r * 65535 / 15) as i64,
                (g * 65535 / 15) as i64,
                (b * 65535 / 15) as i64,
            ));
        }
        // Standard color names (tty-color-standard-values).
        // tty-defined colors: like tty_defined_color, a leading NUL
        // truncates the C view of the name to "" — the default
        // black is returned, never "Invalid color".
        if t.starts_with('\0') {
            return Some((0, 0, 0));
        }
        let rgb = match () {
            _ if t == "black" => (0, 0, 0),
            _ if t == "red" => (65535, 0, 0),
            _ if t == "green" => (0, 65535, 0),
            _ if t == "yellow" => (65535, 65535, 0),
            _ if t == "blue" => (0, 0, 65535),
            _ if t == "magenta" => (65535, 0, 65535),
            _ if t == "cyan" => (0, 65535, 65535),
            _ if t == "white" => (65535, 65535, 65535),
            _ => return None,
        };
        return Some(rgb);
    }
    if let Value::Cons(_) = v {
        let items = v.list_to_vec().ok()?;
        if items.len() == 3 {
            let mut out = [0i64; 3];
            for (k, item) in items.iter().enumerate() {
                out[k] = match item {
                    Value::Int(n) => (*n).clamp(0, 65535) as i64,
                    _ => return None,
                };
            }
            return Some((out[0], out[1], out[2]));
        }
    }
    None
}

pub(super) fn f_color_distance(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // Riemersma's "colour metric" on 16-bit components, as in GNU.
    // GNU checks FRAME with `frame-live-p' FIRST (Fcolor_distance calls
    // get_frame which errors on dead/non-frame args), then parses colors.
    if a.len() > 2 && !a[2].is_nil() {
        let live = match &a[2] {
            Value::Frame(f) => !f.borrow().dead,
            other => return Err(i.wrong_type_mut("frame-live-p", other)),
        };
        if !live {
            return Err(i.wrong_type_mut("frame-live-p", &a[2]));
        }
    }
    let (c1, c2) = match (parse_color_16(&a[0]), parse_color_16(&a[1])) {
        (Some(x), Some(y)) => (x, y),
        _ => {
            // GNU reports the first INVALID color (COLOR1 before COLOR2).
            let bad = if parse_color_16(&a[0]).is_none() {
                &a[0]
            } else {
                &a[1]
            };
            let err = i.q("error");
            return Err(i.signal_data(err, vec![Value::string("Invalid color"), bad.clone()]));
        }
    };
    let r = c1.0 - c2.0;
    let g = c1.1 - c2.1;
    let b = c1.2 - c2.2;
    let r_mean = (c1.0 + c2.0) >> 1;
    let d = ((((2 * 65536 + r_mean) * r * r) >> 16)
        + 4 * g * g
        + (((2 * 65536 + 65535 - r_mean) * b * b) >> 16))
        >> 16;
    Ok(Value::Int(d as i128))
}

pub(super) fn f_glyph_face(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    match &a[0] {
        Value::Int(_) => Ok(Value::Nil),
        Value::Cons(c) => Ok(c.borrow().cdr.clone()),
        other => Err(i.wrong_type_mut("numberp", other)),
    }
}

/// `font-spec` — build a font-spec record `#s(font-spec PROPS)'.
/// GNU validates the restricted properties: :weight/:slant/:width
/// must name a member of their enum, :size/:dpi a number.
pub(super) fn f_font_spec(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    const WEIGHTS: &[&str] = &[
        "thin",
        "ultra-light",
        "extra-light",
        "light",
        "semi-light",
        "book",
        "normal",
        "medium",
        "semi-bold",
        "demi-bold",
        "bold",
        "extra-bold",
        "ultra-bold",
    ];
    const SLANTS: &[&str] = &[
        "normal",
        "roman",
        "italic",
        "oblique",
        "reverse-italic",
        "reverse-oblique",
    ];
    const WIDTHS: &[&str] = &[
        "ultra-condensed",
        "extra-condensed",
        "condensed",
        "semi-condensed",
        "narrow",
        "normal",
        "regular",
        "medium",
        "semi-expanded",
        "expanded",
        "extra-expanded",
        "ultra-expanded",
    ];
    let bad = |i: &mut Interp, p: &Value, v: &Value| -> Flow {
        let err = i.q("error");
        let msg = Value::string("invalid font property");
        let datum = Value::cons(p.clone(), v.clone());
        i.signal_data(err, vec![msg, datum])
    };
    let mut idx = 0;
    while idx < a.len() {
        let prop = &a[idx];
        // GNU: CHECK_SYMBOL(key) — non-symbol keys signal `symbolp'
        // (the key itself is the datum), checked before the missing-value
        // arity error for the LAST key.
        let Some(id) = i.sym_id(prop) else {
            return Err(i.wrong_type_mut("symbolp", prop));
        };
        if idx + 1 >= a.len() {
            return Err(i.error(&format!("No value for key ‘{}’", i.symbol_name(id))));
        }
        let val = &a[idx + 1];
        let name = i.symbol_name(id).to_string();
        let invalid = match name.as_str() {
            ":weight" => {
                !i.sym_id(val)
                    .map(|s| WEIGHTS.contains(&i.symbol_name(s).as_str()))
                    .unwrap_or(false)
                    && !matches!(val, Value::Int(_))
            }
            ":slant" => !i
                .sym_id(val)
                .map(|s| SLANTS.contains(&i.symbol_name(s).as_str()))
                .unwrap_or(false),
            ":width" => {
                !i.sym_id(val)
                    .map(|s| WIDTHS.contains(&i.symbol_name(s).as_str()))
                    .unwrap_or(false)
                    && !matches!(val, Value::Int(_))
            }
            ":size" | ":dpi" => !matches!(val, Value::Int(_) | Value::Float(_)),
            _ => false,
        };
        if invalid {
            return Err(bad(i, prop, val));
        }
        idx += 2;
    }
    Ok(Value::record_with_kind(
        vec![
            Value::Sym(i.q("font-spec")),
            Value::list(a.as_slice().to_vec()),
        ],
        crate::lisp::value::RecordKind::FontSpec,
    ))
}

pub(super) fn f_set_font_selection_order(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU requires a proper list of font-driver symbols.
    // GNU: CHECK_LIST first — non-lists signal `listp'; then each member
    // must be a font-driver symbol else "Invalid font sort order".
    if !matches!(a[0], Value::Nil | Value::Cons(_)) {
        return Err(i.wrong_type_mut("listp", &a[0]));
    }
    let ok = match a[0].list_to_vec() {
        Ok(items) => !items.is_empty() && items.iter().all(|v| matches!(v, Value::Sym(_))),
        Err(_) => false,
    };
    if ok {
        Ok(Value::Nil)
    } else {
        Err(i.error("Invalid font sort order"))
    }
}

pub(super) fn f_query_fontset(i: &mut Interp, _a: crate::lisp::Argv) -> EvalResult {
    // GNU batch: fontsets need a window system.
    Err(i.error("Window system is not in use or not initialized"))
}

pub(super) fn f_close_font(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // No font objects exist; GNU type-checks arg0 as font-object.
    Err(i.wrong_type_mut("font-object", &a[0]))
}

pub(super) fn f_define_fringe_bitmap(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU fringe.c: CHECK_SYMBOL(BITMAP) then CHECK_ARRAY(BITS,
    // Qstringp) — a string bits arg is allowed, nil is not.
    match &a[0] {
        Value::Sym(_) => {}
        other => return Err(i.wrong_type_mut("symbolp", other)),
    }
    match &a[1] {
        Value::Str(_) | Value::Vec(_) => {}
        other => return Err(i.wrong_type_mut("arrayp", other)),
    }
    Ok(a[0].clone())
}

/// `color-values-from-color-spec` — parse #RGB/#RRGGBB/#RRRGGGBBB/
/// #RRRRGGGGBBBB into 16-bit channel values; named colors need a
/// display (nil in batch), non-strings get stringp.
pub(super) fn f_color_values_from_color_spec(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let s = want_string(i, &a[0])?;
    let hex = s.strip_prefix('#').unwrap_or_default();
    if !s.starts_with('#')
        || ![3, 4, 6, 9, 12].contains(&hex.len())
        || !hex.chars().all(|c| c.is_ascii_hexdigit())
    {
        return Ok(Value::Nil);
    }
    let n = hex.len() / 3;
    let scale = |chunk: &crate::lisp::text::EmacsStr| -> i128 {
        let v = color_hex_value(chunk).unwrap_or(0) as i128;
        // Scale N-digit channel to 16 bits: v * 65535 / (16^n - 1).
        v * 65535 / ((1i128 << (4 * n)) - 1)
    };
    Ok(Value::list(vec![
        Value::Int(scale(&hex[0..n])),
        Value::Int(scale(&hex[n..2 * n])),
        Value::Int(scale(&hex[2 * n..3 * n])),
    ]))
}

/// `face-attributes-as-vector` — GNU maps a plist of `:attr value'
/// onto the 20-slot lface vector; anything else → all unspecified.
pub(super) fn f_face_attributes_as_vector(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    const ATTRS: [&str; 20] = [
        "face",
        "family",
        "foundry",
        "width",
        "height",
        "weight",
        "slant",
        "underline",
        "inverse-video",
        "foreground",
        "background",
        "stipple",
        "overline",
        "strike-through",
        "box",
        "font",
        "inherit",
        "fontset",
        "distant-foreground",
        "extend",
    ];
    let unspec = i.q("unspecified");
    let mut slots = vec![Value::Sym(unspec); 20];
    // Iterate plist pairs: (KEY VAL KEY VAL ...).
    let mut cur = a[0].clone();
    while let Value::Cons(c) = cur {
        let (k, rest) = {
            let b = c.borrow();
            (b.car.clone(), b.cdr.clone())
        };
        let (v, next) = match rest {
            Value::Cons(c2) => {
                let b = c2.borrow();
                (b.car.clone(), b.cdr.clone())
            }
            _ => break,
        };
        if let Value::Sym(s) = &k {
            let name = i.symbol_name(*s);
            let bare = name.strip_prefix(':').unwrap_or(&name);
            if let Some(idx) = ATTRS.iter().position(|x| *x == bare) {
                if idx > 0 {
                    slots[idx] = if idx == 7 && v.truthy() && !v.is_nil() {
                        // GNU normalizes :underline to t/nil/plist.
                        Value::t()
                    } else {
                        v
                    };
                }
            }
        }
        cur = next;
    }
    Ok(Value::vector(slots))
}

#[cfg(test)]
mod wide_color_hex_tests {
    use super::*;
    use crate::lisp::text::{EmacsChar, EmacsText};
    #[test]
    fn ascii_hex_is_checked_without_narrowing_character_codes() {
        assert_eq!(color_hex_value(&EmacsText::from("aF09")), Some(0xaf09));
        assert_eq!(color_hex_value(&EmacsText::from("")), None);
        for code in [0xf0080, 0xd800, 0x1a0000, 0x3fff80, 0xff11] {
            let character = EmacsChar::new(code).unwrap();
            assert_eq!(color_hex_value(&character.to_string()), None);
        }
        assert_eq!(
            color_hex_value(&EmacsText::from("ffffffffffffffffffff")),
            None
        );
    }
}
