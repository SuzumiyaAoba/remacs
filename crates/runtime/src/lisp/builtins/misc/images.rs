//! Image specifications, metadata and buffer display adapters.

use crate::lisp::builtins::arg;
use crate::lisp::{Interp, error::EvalResult, value::Value};

pub(super) fn f_image_transforms_p(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    match a.first() {
        None | Some(Value::Nil) => Ok(Value::Nil),
        Some(Value::Frame(f)) if !f.borrow().dead => Ok(Value::Nil),
        Some(other) => Err(i.wrong_type_mut("frame-live-p", other)),
    }
}

/// `on_hot_spot_p' (xdisp.c) — does AREA contain pixel (X, Y)?
/// AREA is (rect . ((x0 . y0) . (x1 . y1))), (circle . ((x0 . y0) . r))
/// or (poly . [x0 y0 x1 y1 ...]).
pub(super) fn on_hot_spot_p(i: &Interp, area: &Value, x: i128, y: i128) -> bool {
    let Value::Cons(hs) = area else {
        return false;
    };
    let (tag, body) = {
        let b = hs.borrow();
        (b.car.clone(), b.cdr.clone())
    };
    let Value::Sym(tag_id) = tag else {
        return false;
    };
    let cons_parts = |v: &Value| -> Option<(Value, Value)> {
        if let Value::Cons(c) = v {
            let b = c.borrow();
            Some((b.car.clone(), b.cdr.clone()))
        } else {
            None
        }
    };
    let fixnum = |v: &Value| -> Option<i128> {
        if let Value::Int(n) = v {
            Some(*n)
        } else {
            None
        }
    };
    match i.symbol_name(tag_id).as_str() {
        "rect" => {
            // ((x0 . y0) . (x1 . y1))
            let Some((tl, br)) = cons_parts(&body) else {
                return false;
            };
            let Some((x0v, y0v)) = cons_parts(&tl) else {
                return false;
            };
            let Some((x1v, y1v)) = cons_parts(&br) else {
                return false;
            };
            let (Some(x0), Some(y0), Some(x1), Some(y1)) =
                (fixnum(&x0v), fixnum(&y0v), fixnum(&x1v), fixnum(&y1v))
            else {
                return false;
            };
            x >= x0 && y >= y0 && x <= x1 && y <= y1
        }
        "circle" => {
            // ((x0 . y0) . r)
            let Some((ctr, rv)) = cons_parts(&body) else {
                return false;
            };
            let Some((x0v, y0v)) = cons_parts(&ctr) else {
                return false;
            };
            let (Some(x0), Some(y0)) = (fixnum(&x0v), fixnum(&y0v)) else {
                return false;
            };
            let r = match &rv {
                Value::Int(n) => *n as f64,
                Value::Float(f) => **f,
                _ => return false,
            };
            let dx = (x0 - x) as f64;
            let dy = (y0 - y) as f64;
            dx * dx + dy * dy <= r * r
        }
        "poly" => {
            // [x0 y0 x1 y1 x2 y2 ...]
            let Value::Vec(v) = &body else {
                return false;
            };
            let poly = v.borrow();
            let n = poly.len();
            if n < 6 || n % 2 != 0 {
                return false;
            }
            let mut inside = false;
            let (Some(mut x0), Some(mut y0)) = (fixnum(&poly[n - 2]), fixnum(&poly[n - 1])) else {
                return false;
            };
            for k in (0..n).step_by(2) {
                let x1 = x0;
                let y1 = y0;
                let (Some(nx), Some(ny)) = (fixnum(&poly[k]), fixnum(&poly[k + 1])) else {
                    return false;
                };
                x0 = nx;
                y0 = ny;
                // Does this segment cross the X line?
                if x0 >= x {
                    if x1 >= x {
                        continue;
                    }
                } else if x1 < x {
                    continue;
                }
                if y > y0 && y > y1 {
                    continue;
                }
                if y < y0 + ((y1 - y0) * (x - x0)) / (x1 - x0) {
                    inside = !inside;
                }
            }
            inside
        }
        _ => false,
    }
}

/// `lookup-image-map' — first map entry whose AREA contains (X, Y).
pub(super) fn f_lookup_image_map(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    if a[0].is_nil() {
        return Ok(Value::Nil);
    }
    let (x, y) = match (&a[1], &a[2]) {
        (Value::Int(x), Value::Int(y)) => (*x, *y),
        (other, _) if !matches!(other, Value::Int(_)) => {
            return Err(i.wrong_type_mut("fixnump", other));
        }
        (_, other) => return Err(i.wrong_type_mut("fixnump", other)),
    };
    let mut cur = a[0].clone();
    while let Value::Cons(c) = cur {
        let (entry, rest) = {
            let b = c.borrow();
            (b.car.clone(), b.cdr.clone())
        };
        cur = rest;
        if let Value::Cons(e) = &entry {
            let area = e.borrow().car.clone();
            if on_hot_spot_p(i, &area, x, y) {
                return Ok(entry);
            }
        }
    }
    Ok(Value::Nil)
}

pub(super) fn f_remove_images(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    // GNU checks START/END as integer-or-marker (BUFFER arg unused
    // here since image display props are ignored) and returns nil.
    let _ = crate::lisp::builtins::want_int_or_mark(i, &args[0])?;
    let _ = crate::lisp::builtins::want_int_or_mark(i, &args[1])?;
    Ok(Value::Nil)
}

/// `image-size' / `image-mask-p' — GNU validates the spec first and
/// signals "Invalid image specification" for anything else; a valid
/// spec then hits `decode_window_system_frame', which on a tty is the
/// "Window system frame should be used" error.
pub(super) fn f_image_spec_check(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    if !valid_image_spec(i, &a[0]) {
        return Err(i.error("Invalid image specification"));
    }
    Err(i.error("Window system frame should be used"))
}

/// `image-metadata' — nil for an invalid spec; a valid spec hits the
/// same window-system error as `image-size'.
pub(super) fn f_image_metadata(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    if !valid_image_spec(i, &a[0]) {
        return Ok(Value::Nil);
    }
    Err(i.error("Window system frame should be used"))
}

/// `image-type' — GNU maps a file name's extension via
/// `image-type-file-name-regexps'; unknown extension signals
/// `unknown-image-type', a non-string signals a plain error.
pub(super) fn f_image_type(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let Value::Str(s) = &a[0] else {
        let shown = i.princ_to_string(&a[0]);
        return Err(i.error_q("Invalid image file name `%s'", &[&shown]));
    };
    let name = s.borrow().clone();
    let ext = name.rsplit('.').next().unwrap_or_default().to_lowercase();
    let has_dot = name.contains('.');
    let ty = if has_dot {
        match ext.to_unicode_lossy().as_str() {
            "png" => Some("png"),
            "gif" => Some("gif"),
            "jpg" | "jpeg" => Some("jpeg"),
            "webp" => Some("webp"),
            "bmp" => Some("bmp"),
            "xpm" => Some("xpm"),
            "pbm" => Some("pbm"),
            "xbm" => Some("xbm"),
            "ps" => Some("postscript"),
            "tif" | "tiff" => Some("tiff"),
            "svg" | "svgz" => Some("svg"),
            "heic" | "heif" | "heics" => Some("heic"),
            _ => None,
        }
    } else {
        None
    };
    match ty {
        Some(t) => Ok(Value::Sym(i.intern(t))),
        None => {
            let unk = i.q("unknown-image-type");
            Err(i.signal_data(unk, vec![Value::string("Cannot determine image type")]))
        }
    }
}

/// The image types remacs claims (GNU `image_types[]' parity):
/// `init-image-library'/`image-type-available-p' share the set.
pub(super) fn known_image_type(i: &Interp, v: &Value) -> bool {
    let Value::Sym(s) = v else {
        return false;
    };
    matches!(
        i.symbol_name(*s).as_str(),
        "png"
            | "gif"
            | "jpeg"
            | "webp"
            | "bmp"
            | "xpm"
            | "pbm"
            | "xbm"
            | "postscript"
            | "tiff"
            | "svg"
            | "heic"
    )
}

/// `image-type-available-p' — the types our (fake) image support
/// claims: GNU batch reports all built-ins available.
pub(super) fn f_image_type_available_p(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    Ok(Value::from_bool(known_image_type(i, &a[0])))
}

/// `init-image-library' — GNU's `lookup_image_type': t iff TYPE is a
/// registered (built-in) image type symbol.
pub(super) fn f_init_image_library(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    Ok(Value::from_bool(known_image_type(i, &a[0])))
}

/// `imagep' — GNU's valid_image_p: a list headed `image' whose plist
/// has a known :type plus a :file or :data source.
pub(super) fn valid_image_spec(i: &Interp, v: &Value) -> bool {
    let Value::Cons(c) = v else {
        return false;
    };
    if !matches!(&c.borrow().car, Value::Sym(s) if i.symbol_name(*s) == "image") {
        return false;
    }
    let mut ty = false;
    let mut src = false;
    let mut cur = c.borrow().cdr.clone();
    while let Value::Cons(p) = cur {
        let (k, rest) = (p.borrow().car.clone(), p.borrow().cdr.clone());
        if let Value::Sym(id) = &k {
            let n = i.symbol_name(*id).to_string();
            let val = match &rest {
                Value::Cons(vc) => vc.borrow().car.clone(),
                _ => Value::Nil,
            };
            match n.as_str() {
                ":type" => {
                    if known_image_type(i, &val) {
                        ty = true;
                    }
                }
                ":file" | ":data" => {
                    if matches!(val, Value::Str(_)) {
                        src = true;
                    }
                }
                _ => {}
            }
        }
        match rest {
            Value::Cons(r) => cur = r.borrow().cdr.clone(),
            _ => break,
        }
    }
    ty && src
}

pub(super) fn f_imagep(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    Ok(Value::from_bool(valid_image_spec(i, &a[0])))
}

/// `image-flush' — invalid spec signals a plain error; a valid spec
/// then hits the no-window-system error like GNU on a tty.
pub(super) fn f_image_flush(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    if !valid_image_spec(i, &a[0]) {
        return Err(i.error("Invalid image specification"));
    }
    Err(i.error("Window system frame should be used"))
}

/// `put-image` — GNU requires an image spec (list headed `image') and
/// returns a fresh overlay at POS.
pub(super) fn f_put_image(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let image_sym = i.q("image");
    let ok = match &a[0] {
        Value::Cons(c) => i.sym_is(&c.borrow().car, image_sym),
        _ => false,
    };
    if !ok {
        let shown = i.prin1_to_string(&a[0]);
        return Err(i.error(format!("Not an image: {shown}")));
    }
    let pos = a[1].clone();
    crate::editor::f_make_overlay(i, vec![pos.clone(), pos, Value::Nil].into())
}

pub(super) fn f_clear_image_cache(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU image.c
    // Fclear_image_cache: non-nil ANIMATION-FILTER is CHECK_CONS'd FIRST
    // (wta `consp') and returns nil early; only nil/frame FILTER reaches
    // decode_window_system_frame → "Window system frame should be used".
    // A non-nil non-frame FILTER (e.g. a filename string) just clears
    // matching cache entries — no error even without a window system.
    if arg(&a, 1).truthy() {
        if !matches!(arg(&a, 1), Value::Cons(_)) {
            return Err(i.wrong_type_mut("consp", &arg(&a, 1)));
        }
        return Ok(Value::Nil);
    }
    match &arg(&a, 0) {
        Value::Nil | Value::Frame(_) => Err(i.error("Window system frame should be used")),
        _ => Ok(Value::Nil),
    }
}

pub(super) fn f_bitmap_spec_p(_i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU treats a string as a bitmap spec (file name / data form).
    Ok(Value::from_bool(matches!(&a[0], Value::Str(_))))
}
