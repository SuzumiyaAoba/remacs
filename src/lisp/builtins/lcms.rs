//! LCMS2 colorimetry primitives (`lcms-*`), a port of GNU `src/lcms.c`.
//!
//! Real `liblcms2' is loaded dynamically (see `dynlib'); the JCh<->Jab
//! transforms are pure math ported literally from GNU.

use std::rc::Rc;
use std::sync::OnceLock;

use super::{arg, want_num, S};
use crate::lisp::error::{EvalResult, Flow};
use crate::lisp::value::Value;

// ---- lcms2 C types ---------------------------------------------------------

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct CmsXYZ {
    x: f64,
    y: f64,
    z: f64,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct CmsxyY {
    x: f64,
    y: f64,
    cap_y: f64,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct CmsLab {
    l: f64,
    a: f64,
    b: f64,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct CmsJCh {
    j: f64,
    c: f64,
    h: f64,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct CmsViewingConditions {
    white_point: CmsXYZ,
    yb: f64,
    la: f64,
    surround: u32,
    d_value: f64,
}

type CmsHandle = *mut core::ffi::c_void;
type CmsContext = *mut core::ffi::c_void;

type Cie2000 = unsafe extern "C" fn(*const CmsLab, *const CmsLab, f64, f64, f64) -> f64;
type Cam02Init = unsafe extern "C" fn(CmsContext, *const CmsViewingConditions) -> CmsHandle;
type Cam02Fwd = unsafe extern "C" fn(CmsHandle, *const CmsXYZ, *mut CmsJCh);
type Cam02Rev = unsafe extern "C" fn(CmsHandle, *const CmsJCh, *mut CmsXYZ);
type Cam02Done = unsafe extern "C" fn(CmsHandle);
type WhitePointFromTemp = unsafe extern "C" fn(*mut CmsxyY, f64) -> i32;
type XyY2Xyz = unsafe extern "C" fn(*mut CmsXYZ, *const CmsxyY);

struct LcmsFns {
    _lib: libloading::Library,
    cie2000: Cie2000,
    cam02_init: Cam02Init,
    cam02_fwd: Cam02Fwd,
    cam02_rev: Cam02Rev,
    cam02_done: Cam02Done,
    white_point_from_temp: WhitePointFromTemp,
    xyy2xyz: XyY2Xyz,
}

unsafe impl Send for LcmsFns {}
unsafe impl Sync for LcmsFns {}

static LCMS: OnceLock<Option<LcmsFns>> = OnceLock::new();

fn lcms() -> Option<&'static LcmsFns> {
    LCMS
        .get_or_init(|| {
            let lib = crate::lisp::dynlib::open_library(
                "REMACS_LCMS2_LIBRARY",
                &[
                    "liblcms2.2.dylib",
                    "liblcms2.so.2",
                    "liblcms2.so",
                    "liblcms2.dylib",
                ],
                "lcms2",
                "liblcms2.dylib",
            )?;
            let f = unsafe {
                LcmsFns {
                    cie2000: crate::lisp::dynlib::sym(&lib, b"cmsCIE2000DeltaE\0")?,
                    cam02_init: crate::lisp::dynlib::sym(&lib, b"cmsCIECAM02Init\0")?,
                    cam02_fwd: crate::lisp::dynlib::sym(&lib, b"cmsCIECAM02Forward\0")?,
                    cam02_rev: crate::lisp::dynlib::sym(&lib, b"cmsCIECAM02Reverse\0")?,
                    cam02_done: crate::lisp::dynlib::sym(&lib, b"cmsCIECAM02Done\0")?,
                    white_point_from_temp: crate::lisp::dynlib::sym(
                        &lib,
                        b"cmsWhitePointFromTemp\0",
                    )?,
                    xyy2xyz: crate::lisp::dynlib::sym(&lib, b"cmsxyY2XYZ\0")?,
                    _lib: lib,
                }
            };
            Some(f)
        })
        .as_ref()
}

// ---- list parsing (GNU parse_*_list ports) ----------------------------------

/// GNU `parse_lab_list': three leading cons cells, each NUMBERP.
/// Trailing elements are ignored.
fn parse_lab(v: &Value) -> Option<CmsLab> {
    let mut out = [0f64; 3];
    if !parse_num_fields(v, &mut out) {
        return None;
    }
    Some(CmsLab {
        l: out[0],
        a: out[1],
        b: out[2],
    })
}

/// GNU `parse_xyz_list': three leading numbers scaled by 100.
fn parse_xyz(v: &Value) -> Option<CmsXYZ> {
    let mut out = [0f64; 3];
    if !parse_num_fields(v, &mut out) {
        return None;
    }
    Some(CmsXYZ {
        x: out[0] * 100.0,
        y: out[1] * 100.0,
        z: out[2] * 100.0,
    })
}

/// GNU `parse_jch_list': three numbers, list must end there.
fn parse_jch(v: &Value) -> Option<CmsJCh> {
    let mut out = [0f64; 3];
    if !parse_num_fields(v, &mut out) {
        return None;
    }
    if let Value::Cons(_) = v_list_rest(v, 3) {
        return None;
    }
    Some(CmsJCh {
        j: out[0],
        c: out[1],
        h: out[2],
    })
}

fn parse_jab(v: &Value) -> Option<CmsJCh> {
    parse_jch(v)
}

fn v_list_rest(v: &Value, n: usize) -> Value {
    let mut cur = v.clone();
    for _ in 0..n {
        cur = match cur {
            Value::Cons(c) => c.borrow().cdr.clone(),
            other => return other,
        };
    }
    cur
}

/// Parse the first `out.len()` cons cells; each car must be a number.
fn parse_num_fields(v: &Value, out: &mut [f64]) -> bool {
    let mut cur = v.clone();
    for slot in out.iter_mut() {
        match cur {
            Value::Cons(c) => {
                let (car, cdr) = {
                    let b = c.borrow();
                    (b.car.clone(), b.cdr.clone())
                };
                *slot = match car {
                    Value::Int(n) => n as f64,
                    Value::Float(f) => *f,
                    _ => return false,
                };
                cur = cdr;
            }
            _ => return false,
        }
    }
    true
}

/// GNU `parse_viewing_conditions' (Yb La surround D_value), strict end.
fn parse_viewing(v: &Value, wp: &CmsXYZ) -> Option<CmsViewingConditions> {
    let mut vc = CmsViewingConditions {
        white_point: *wp,
        ..Default::default()
    };
    let mut cur = v.clone();
    let mut idx = 0usize;
    while let Value::Cons(c) = cur.clone() {
        let (car, cdr) = {
            let b = c.borrow();
            (b.car.clone(), b.cdr.clone())
        };
        match idx {
            0 => {
                vc.yb = num_or_none(&car)?;
            }
            1 => {
                vc.la = num_or_none(&car)?;
            }
            2 => {
                // FIXNATP then range 1..=4.
                let n = match car {
                    Value::Int(n) if n >= 0 => n,
                    _ => return None,
                };
                if !(1..=4).contains(&n) {
                    return None;
                }
                vc.surround = n as u32;
            }
            3 => {
                vc.d_value = num_or_none(&car)?;
            }
            _ => return None,
        }
        idx += 1;
        cur = cdr;
    }
    if idx != 4 || !cur.is_nil() {
        return None;
    }
    Some(vc)
}

fn num_or_none(v: &Value) -> Option<f64> {
    match v {
        Value::Int(n) => Some(*n as f64),
        Value::Float(f) => Some(**f),
        _ => None,
    }
}

/// GNU's illuminant_d65.
const D65: CmsXYZ = CmsXYZ {
    x: 95.0455,
    y: 100.0,
    z: 108.8753,
};

fn default_viewing(wp: &CmsXYZ) -> CmsViewingConditions {
    CmsViewingConditions {
        white_point: *wp,
        yb: 20.0,
        la: 100.0,
        surround: 1, // AVG_SURROUND
        d_value: 1.0,
    }
}

// ---- math (GNU helper ports) ------------------------------------------------

fn deg2rad(d: f64) -> f64 {
    std::f64::consts::PI * d / 180.0
}
fn rad2deg(r: f64) -> f64 {
    180.0 * r / std::f64::consts::PI
}

fn xyz_to_jch(xyz: &CmsXYZ, vc: &CmsViewingConditions) -> CmsJCh {
    let g = lcms().expect("lcms");
    unsafe {
        let h = (g.cam02_init)(std::ptr::null_mut(), vc);
        let mut jch = CmsJCh::default();
        (g.cam02_fwd)(h, xyz, &mut jch);
        (g.cam02_done)(h);
        jch
    }
}

fn jch_to_xyz(jch: &CmsJCh, vc: &CmsViewingConditions) -> CmsXYZ {
    let g = lcms().expect("lcms");
    unsafe {
        let h = (g.cam02_init)(std::ptr::null_mut(), vc);
        let mut xyz = CmsXYZ::default();
        (g.cam02_rev)(h, jch, &mut xyz);
        (g.cam02_done)(h);
        xyz
    }
}

#[derive(Clone, Copy)]
struct Jab {
    j: f64,
    a: f64,
    b: f64,
}

fn jch_to_jab(jch: &CmsJCh, fl: f64, c1: f64, c2: f64) -> Jab {
    let mp = 43.86 * (1.0 + c2 * (jch.c * fl.sqrt().sqrt())).ln();
    Jab {
        j: 1.7 * jch.j / (1.0 + (c1 * jch.j)),
        a: mp * deg2rad(jch.h).cos(),
        b: mp * deg2rad(jch.h).sin(),
    }
}

fn jab_to_jch(jab: &Jab, fl: f64, c1: f64, c2: f64) -> CmsJCh {
    let mut h = jab.b.atan2(jab.a);
    let mp = jab.a.hypot(jab.b);
    h = rad2deg(h);
    if h < 0.0 {
        h += 360.0;
    }
    CmsJCh {
        j: jab.j / (1.0 + c1 * (100.0 - jab.j)),
        c: ((c2 * mp).exp() - 1.0) / (c2 * fl.sqrt().sqrt()),
        h,
    }
}

fn fl_factor(la: f64) -> f64 {
    let k = 1.0 / (1.0 + (5.0 * la));
    let k4 = k * k * k * k;
    la * k4 + 0.1 * (1.0 - k4) * (1.0 - k4) * (5.0 * la).cbrt()
}

// ---- Lisp plumbing ----------------------------------------------------------

fn fl(x: f64) -> Value {
    Value::float(x)
}

fn list3(a: Value, b: Value, c: Value) -> Value {
    Value::list(vec![a, b, c])
}

fn need(i: &mut crate::lisp::eval::Interp) -> Result<&'static LcmsFns, Flow> {
    match lcms() {
        Some(g) => Ok(g),
        None => Err(i.error("lcms2 library not found")),
    }
}

fn want_wp_and_vc(
    i: &mut crate::lisp::eval::Interp,
    whitepoint: &Value,
    view: &Value,
) -> Result<CmsViewingConditions, Flow> {
    let xyzw = if whitepoint.is_nil() {
        D65
    } else {
        match parse_xyz(whitepoint) {
            Some(w) => w,
            None => return Err(i.signal_data(crate::lisp::obarray::sym::ERROR, vec![Value::string("Invalid white point"), whitepoint.clone()])),
        }
    };
    let vc = if view.is_nil() {
        default_viewing(&xyzw)
    } else {
        match parse_viewing(view, &xyzw) {
            Some(v) => v,
            None => return Err(i.signal_data(crate::lisp::obarray::sym::ERROR, vec![Value::string("Invalid viewing conditions"), view.clone()])),
        }
    };
    Ok(vc)
}

fn f_lcms_cie_de2000(i: &mut crate::lisp::eval::Interp, a: Vec<Value>) -> EvalResult {
    let g = need(i)?;
    let lab1 = match parse_lab(&arg(&a, 0)) {
        Some(v) => v,
        None => return Err(i.signal_data(crate::lisp::obarray::sym::ERROR, vec![Value::string("Invalid color"), a[0].clone()])),
    };
    let lab2 = match parse_lab(&arg(&a, 1)) {
        Some(v) => v,
        None => return Err(i.signal_data(crate::lisp::obarray::sym::ERROR, vec![Value::string("Invalid color"), a[1].clone()])),
    };
    let mut opt_k = |v: &Value| -> Result<f64, Flow> {
        if v.is_nil() {
            Ok(1.0)
        } else {
            want_num(i, v)
        }
    };
    let kl = opt_k(&arg(&a, 2))?;
    let kc = opt_k(&arg(&a, 3))?;
    let kh = opt_k(&arg(&a, 4))?;
    Ok(fl(unsafe { (g.cie2000)(&lab1, &lab2, kl, kc, kh) }))
}

fn f_lcms_xyz_to_jch(i: &mut crate::lisp::eval::Interp, a: Vec<Value>) -> EvalResult {
    need(i)?;
    let xyz = match parse_xyz(&arg(&a, 0)) {
        Some(v) => v,
        None => return Err(i.signal_data(crate::lisp::obarray::sym::ERROR, vec![Value::string("Invalid color"), a[0].clone()])),
    };
    let vc = want_wp_and_vc(i, &arg(&a, 1), &arg(&a, 2))?;
    let jch = xyz_to_jch(&xyz, &vc);
    Ok(list3(fl(jch.j), fl(jch.c), fl(jch.h)))
}

fn f_lcms_jch_to_xyz(i: &mut crate::lisp::eval::Interp, a: Vec<Value>) -> EvalResult {
    need(i)?;
    let jch = match parse_jch(&arg(&a, 0)) {
        Some(v) => v,
        None => return Err(i.signal_data(crate::lisp::obarray::sym::ERROR, vec![Value::string("Invalid color"), a[0].clone()])),
    };
    let vc = want_wp_and_vc(i, &arg(&a, 1), &arg(&a, 2))?;
    let xyz = jch_to_xyz(&jch, &vc);
    Ok(list3(fl(xyz.x / 100.0), fl(xyz.y / 100.0), fl(xyz.z / 100.0)))
}

fn f_lcms_jch_to_jab(i: &mut crate::lisp::eval::Interp, a: Vec<Value>) -> EvalResult {
    need(i)?;
    let jch = match parse_jch(&arg(&a, 0)) {
        Some(v) => v,
        None => return Err(i.signal_data(crate::lisp::obarray::sym::ERROR, vec![Value::string("Invalid color"), a[0].clone()])),
    };
    let vc = want_wp_and_vc(i, &arg(&a, 1), &arg(&a, 2))?;
    let fl_ = fl_factor(vc.la);
    let jab = jch_to_jab(&jch, fl_, 0.007, 0.0228);
    Ok(list3(fl(jab.j), fl(jab.a), fl(jab.b)))
}

fn f_lcms_jab_to_jch(i: &mut crate::lisp::eval::Interp, a: Vec<Value>) -> EvalResult {
    need(i)?;
    let jabv = match parse_jab(&arg(&a, 0)) {
        Some(v) => v,
        None => return Err(i.signal_data(crate::lisp::obarray::sym::ERROR, vec![Value::string("Invalid color"), a[0].clone()])),
    };
    let vc = want_wp_and_vc(i, &arg(&a, 1), &arg(&a, 2))?;
    let fl_ = fl_factor(vc.la);
    let jab = Jab {
        j: jabv.j,
        a: jabv.c,
        b: jabv.h,
    };
    let jch = jab_to_jch(&jab, fl_, 0.007, 0.0228);
    Ok(list3(fl(jch.j), fl(jch.c), fl(jch.h)))
}

fn f_lcms_cam02_ucs(i: &mut crate::lisp::eval::Interp, a: Vec<Value>) -> EvalResult {
    need(i)?;
    let xyz1 = match parse_xyz(&arg(&a, 0)) {
        Some(v) => v,
        None => return Err(i.signal_data(crate::lisp::obarray::sym::ERROR, vec![Value::string("Invalid color"), a[0].clone()])),
    };
    let xyz2 = match parse_xyz(&arg(&a, 1)) {
        Some(v) => v,
        None => return Err(i.signal_data(crate::lisp::obarray::sym::ERROR, vec![Value::string("Invalid color"), a[1].clone()])),
    };
    let vc = want_wp_and_vc(i, &arg(&a, 2), &arg(&a, 3))?;
    let jch1 = xyz_to_jch(&xyz1, &vc);
    let jch2 = xyz_to_jch(&xyz2, &vc);
    let fl_ = fl_factor(vc.la);
    let jab1 = jch_to_jab(&jch1, fl_, 0.007, 0.0228);
    let jab2 = jch_to_jab(&jch2, fl_, 0.007, 0.0228);
    Ok(fl((jab2.j - jab1.j).hypot((jab2.a - jab1.a).hypot(jab2.b - jab1.b))))
}

fn f_lcms_temp_to_white_point(i: &mut crate::lisp::eval::Interp, a: Vec<Value>) -> EvalResult {
    let g = need(i)?;
    let t = want_num(i, &arg(&a, 0))?;
    let mut wp = CmsxyY::default();
    let ok = unsafe { (g.white_point_from_temp)(&mut wp, t) };
    if ok == 0 {
        return Err(i.signal_data(crate::lisp::obarray::sym::ERROR, vec![Value::string("Invalid temperature"), a[0].clone()]));
    }
    let mut xyz = CmsXYZ::default();
    unsafe { (g.xyy2xyz)(&mut xyz, &wp) };
    Ok(list3(fl(xyz.x), fl(xyz.y), fl(xyz.z)))
}

fn f_lcms2_available_p(_i: &mut crate::lisp::eval::Interp, _a: Vec<Value>) -> EvalResult {
    if lcms().is_some() {
        Ok(Value::t())
    } else {
        Ok(Value::Nil)
    }
}

pub(crate) static SUBRS: &[crate::lisp::value::Subr] = &[
    S!("lcms-cie-de2000", 2, 5, f_lcms_cie_de2000, "Compute CIEDE2000 metric distance between COLOR1 and COLOR2."),
    S!("lcms-xyz->jch", 1, 3, f_lcms_xyz_to_jch, "Convert CIE XYZ to CIE CAM02 JCh."),
    S!("lcms-jch->xyz", 1, 3, f_lcms_jch_to_xyz, "Convert CIE CAM02 JCh to CIE XYZ."),
    S!("lcms-jch->jab", 1, 3, f_lcms_jch_to_jab, "Convert CIE CAM02 JCh to CAM02-UCS J'a'b'."),
    S!("lcms-jab->jch", 1, 3, f_lcms_jab_to_jch, "Convert CAM02-UCS J'a'b' to CIE CAM02 JCh."),
    S!("lcms-cam02-ucs", 2, 4, f_lcms_cam02_ucs, "Compute CAM02-UCS metric distance between COLOR1 and COLOR2."),
    S!("lcms-temp->white-point", 1, 1, f_lcms_temp_to_white_point, "Return XYZ black body chromaticity from TEMPERATURE in K."),
    S!("lcms2-available-p", 0, 0, f_lcms2_available_p, "Return t if lcms2 color calculations are available."),
];

/// Register the `lcms2' feature when the library is usable (GNU's
/// `Fprovide (intern_c_string ("lcms2"))' when built with HAVE_LCMS2).
pub(crate) fn install(i: &mut crate::lisp::eval::Interp) {
    if lcms().is_some() {
        let f = i.intern("lcms2");
        if !i.features.contains(&f) {
            i.features.push(f);
        }
    }
}

// silence unused Rc import
#[allow(dead_code)]
fn _unused(_: Rc<f64>) {}
