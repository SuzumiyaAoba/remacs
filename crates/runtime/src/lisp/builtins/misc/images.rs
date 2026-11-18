//! Image specifications, metadata and buffer display adapters.

use crate::lisp::builtins::arg;
use crate::lisp::{Interp, error::EvalResult, obarray::sym, value::Value};

pub(super) fn f_display_images_p(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let display = a.get(0).cloned().unwrap_or(Value::Nil);
    let frame = crate::lisp::builtins::misc::decode_display(i, &display)?;
    Ok(Value::from_bool(
        i.image_backend.is_some() && crate::editor::frame_is_graphical(i, &frame),
    ))
}

pub(super) fn f_image_transforms_p(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let frame = crate::editor::frame_live(i, &arg(&a, 0))?;
    if crate::editor::frame_is_graphical(i, &frame) {
        // GPUI applies image width/height descriptors, but has no image
        // rotation transform.
        Ok(Value::list(vec![Value::Sym(i.intern("scale"))]))
    } else {
        Ok(Value::Nil)
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

fn image_spec_info(
    i: &mut Interp,
    spec: &Value,
    frame: &crate::editor::FrameRef,
) -> Result<crate::frontend::ImageSpecInfo, crate::lisp::error::Flow> {
    let font_height = crate::editor::frame_char_height(i, frame)
        .max(1)
        .min(u16::MAX as usize) as u16;
    crate::frontend::image_spec_info(i, spec, i.current_buffer, font_height)
        .ok_or_else(|| i.error("Invalid image specification"))
}

fn image_info(
    i: &mut Interp,
    frame: &crate::editor::FrameRef,
    spec: &crate::frontend::ImageSpecInfo,
) -> Result<Option<crate::frontend::ImageInfo>, crate::lisp::error::Flow> {
    let Some(backend) = i.image_backend.clone() else {
        return Ok(None);
    };
    let frame_id = frame.borrow().id;
    backend
        .image_info(frame_id, spec.source.clone(), spec.index)
        .map_err(|error| i.error(error))
}

fn image_display_size(
    spec: &crate::frontend::ImageSpecInfo,
    info: &crate::frontend::ImageInfo,
) -> (usize, usize) {
    match (spec.width, spec.height) {
        (Some(width), Some(height)) => (width, height),
        (Some(width), None) => (
            width,
            if info.width == 0 {
                0
            } else {
                (info.height as f64 * width as f64 / info.width as f64).round() as usize
            },
        ),
        (None, Some(height)) => (
            if info.height == 0 {
                0
            } else {
                (info.width as f64 * height as f64 / info.height as f64).round() as usize
            },
            height,
        ),
        (None, None) => (info.width, info.height),
    }
}

/// `image-size' — load a valid image descriptor on the requested frame.
pub(super) fn f_image_size(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    if !valid_image_spec(i, &a[0]) {
        return Err(i.error("Invalid image specification"));
    }
    let frame = crate::editor::frame_live(i, &arg(&a, 2))?;
    if !crate::editor::frame_is_graphical(i, &frame) {
        return Err(i.error("Window system frame should be used"));
    }
    let spec = image_spec_info(i, &a[0], &frame)?;
    let Some(info) = image_info(i, &frame, &spec)? else {
        return Ok(Value::Nil);
    };
    let (width, height) = image_display_size(&spec, &info);
    if arg(&a, 1).truthy() {
        Ok(Value::cons(
            Value::Int(width as i128),
            Value::Int(height as i128),
        ))
    } else {
        let char_width = crate::editor::frame_char_width(i, &frame).max(1) as f64;
        let char_height = crate::editor::frame_char_height(i, &frame).max(1) as f64;
        Ok(Value::cons(
            Value::float(width as f64 / char_width),
            Value::float(height as f64 / char_height),
        ))
    }
}

/// `image-mask-p' — report transparency or a mask requested by the spec.
pub(super) fn f_image_mask_p(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    if !valid_image_spec(i, &a[0]) {
        return Err(i.error("Invalid image specification"));
    }
    let frame = crate::editor::frame_live(i, &arg(&a, 1))?;
    if !crate::editor::frame_is_graphical(i, &frame) {
        return Err(i.error("Window system frame should be used"));
    }
    let spec = image_spec_info(i, &a[0], &frame)?;
    let masked =
        spec.mask_requested || image_info(i, &frame, &spec)?.is_some_and(|info| info.has_mask);
    Ok(Value::from_bool(masked))
}

/// `image-metadata' — expose frame count and delay for animated images.
pub(super) fn f_image_metadata(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    if !valid_image_spec(i, &a[0]) {
        return Ok(Value::Nil);
    }
    let frame = crate::editor::frame_live(i, &arg(&a, 1))?;
    if !crate::editor::frame_is_graphical(i, &frame) {
        return Err(i.error("Window system frame should be used"));
    }
    let spec = image_spec_info(i, &a[0], &frame)?;
    let Some(info) = image_info(i, &frame, &spec)? else {
        return Ok(Value::Nil);
    };
    let mut metadata = Vec::new();
    if let Some(delay) = info.frame_delay.filter(|delay| *delay > 0.0) {
        metadata.extend([Value::Sym(i.intern("delay")), Value::float(delay)]);
    }
    if info.frame_count > 1 {
        metadata.extend([
            Value::Sym(i.intern("count")),
            Value::Int(info.frame_count as i128),
        ]);
    }
    Ok(Value::list(metadata))
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

/// Image descriptor types registered by Remacs.  Actual availability is
/// filtered by the frontend's decoder support below.
pub(super) fn known_image_type(i: &Interp, v: &Value) -> bool {
    let Value::Sym(s) = v else {
        return false;
    };
    [
        "png",
        "gif",
        "jpeg",
        "webp",
        "bmp",
        "xpm",
        "pbm",
        "xbm",
        "postscript",
        "tiff",
        "svg",
        "heic",
    ]
    .iter()
    .any(|name| i.intern_soft(name) == Some(*s))
}

/// `image-type-available-p' — the image formats the GUI renderer can decode.
pub(super) fn f_image_type_available_p(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    Ok(Value::from_bool(renderer_supports_image_type(i, &a[0])))
}

/// `init-image-library' — only initialize image types the GUI decoder supports.
pub(super) fn f_init_image_library(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    Ok(Value::from_bool(renderer_supports_image_type(i, &a[0])))
}

/// `imagep' — GNU's valid_image_p: a list headed `image' whose plist
/// has a known :type plus a :file or :data source.
pub(super) fn valid_image_spec(i: &Interp, v: &Value) -> bool {
    let Value::Cons(c) = v else {
        return false;
    };
    if !matches!(&c.borrow().car, Value::Sym(s) if i.intern_soft("image") == Some(*s)) {
        return false;
    }
    let type_key = i.intern_soft(":type");
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
                ":type" if type_key == Some(*id) => {
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

/// `image-flush' — remove the image from the renderer cache.
pub(super) fn f_image_flush(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    if !valid_image_spec(i, &a[0]) {
        return Err(i.error("Invalid image specification"));
    }
    let frame_arg = arg(&a, 1);
    let (frame, frame_ids) = if i.sym_is(&frame_arg, sym::T) {
        let frames: Vec<_> = i
            .frames
            .iter()
            .filter(|frame| !frame.borrow().dead && crate::editor::frame_is_graphical(i, frame))
            .cloned()
            .collect();
        let Some(frame) = frames.first().cloned() else {
            return Ok(Value::Nil);
        };
        let frame_ids = frames.iter().map(|frame| frame.borrow().id).collect();
        (frame, frame_ids)
    } else {
        let frame = crate::editor::frame_live(i, &frame_arg)?;
        if !crate::editor::frame_is_graphical(i, &frame) {
            return Err(i.error("Window system frame should be used"));
        }
        let frame_ids = vec![frame.borrow().id];
        (frame, frame_ids)
    };
    let spec = image_spec_info(i, &a[0], &frame)?;
    if let Some(backend) = i.image_backend.clone() {
        backend
            .flush_image(frame_ids, spec.source)
            .map_err(|error| i.error(error))?;
    }
    Ok(Value::Nil)
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
    let filter = arg(&a, 0);
    let (frame_ids, filename) = if i.sym_is(&filter, sym::T) {
        (None, None)
    } else if matches!(filter, Value::Nil | Value::Frame(_)) {
        let frame = crate::editor::frame_live(i, &filter)?;
        if !crate::editor::frame_is_graphical(i, &frame) {
            return Err(i.error("Window system frame should be used"));
        }
        (Some(vec![frame.borrow().id]), None)
    } else if let Value::Str(filename) = filter {
        (None, Some(filename.borrow().to_unicode_lossy()))
    } else {
        return Ok(Value::Nil);
    };
    if let Some(backend) = i.image_backend.clone() {
        backend
            .clear_image_cache(frame_ids, filename)
            .map_err(|error| i.error(error))?;
    }
    Ok(Value::Nil)
}

pub(super) fn f_image_cache_size(i: &mut Interp, _a: crate::lisp::Argv) -> EvalResult {
    Ok(Value::Int(
        i.gui_image_cache_size
            .load(std::sync::atomic::Ordering::Acquire) as i128,
    ))
}

fn renderer_supports_image_type(i: &Interp, value: &Value) -> bool {
    let Value::Sym(symbol) = value else {
        return false;
    };
    ["png", "gif", "jpeg", "webp", "bmp", "pbm", "tiff", "svg"]
        .iter()
        .any(|name| i.intern_soft(name) == Some(*symbol))
}

pub(super) fn f_bitmap_spec_p(_i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU treats a string as a bitmap spec (file name / data form).
    Ok(Value::from_bool(matches!(&a[0], Value::Str(_))))
}
