//! NextStep (macOS Cocoa) primitives — GNU `nsfns.m'/`nsselect.m'/
//! `nsmenu.m' ports.
//!
//! Native NS functions that require an AppKit terminal continue to
//! report "Window system is not in use or not initialized" when called
//! without one. The GUI front-end supplies clipboard, drag, and AppleScript
//! backends; PRIMARY and SECONDARY remain process-local.
//! Functions that work without a display connection in GNU (font-name
//! conversion, color lists, dock tile operations, sleep assertions,
//! accessibility check) are implemented via dlopen'd AppKit runtime.

use std::collections::HashMap;
use std::sync::OnceLock;

use super::{Interp, S, Subr};
use crate::lisp::EvalResult;
use crate::lisp::error::Flow;
use crate::lisp::value::{Cons, SymId, Value};

// ---------------------------------------------------------------------------
// Objective-C runtime bridge (libobjc + AppKit/Foundation via dynlib)
// ---------------------------------------------------------------------------

type GetClass = unsafe extern "C" fn(*const core::ffi::c_char) -> usize;
type SelName = unsafe extern "C" fn(*const core::ffi::c_char) -> usize;
type MsgId0 = unsafe extern "C" fn(usize, usize) -> usize;
type MsgId1 = unsafe extern "C" fn(usize, usize, usize) -> usize;
type MsgInt0 = unsafe extern "C" fn(usize, usize) -> i64;
type MsgInt1 = unsafe extern "C" fn(usize, usize, usize) -> i64;
type MsgU64_2 = unsafe extern "C" fn(usize, usize, u64, usize) -> usize;
type MsgVoid0 = unsafe extern "C" fn(usize, usize);
type MsgVoid1 = unsafe extern "C" fn(usize, usize, usize);
type MsgVoid2 = unsafe extern "C" fn(usize, usize, usize, usize);
type MsgDbl1 = unsafe extern "C" fn(usize, usize, f64);
type MsgRange1 = unsafe extern "C" fn(usize, usize, usize) -> NsRange;
type PoolPush = unsafe extern "C" fn() -> usize;
type PoolPop = unsafe extern "C" fn(usize);

#[repr(C)]
#[derive(Clone, Copy)]
struct NsRange {
    location: usize,
    length: usize,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct NsSize {
    width: f64,
    height: f64,
}

type MsgSize0 = unsafe extern "C" fn(usize, usize) -> NsSize;
type MsgRect1 = unsafe extern "C" fn(usize, usize, NsRect) -> usize;

#[repr(C)]
#[derive(Clone, Copy)]
struct NsRect {
    x: f64,
    y: f64,
    w: f64,
    h: f64,
}

pub(crate) struct Objc {
    _lib: libloading::Library,
    get_class: GetClass,
    sel_name: SelName,
    id0: MsgId0,
    id1: MsgId1,
    int0: MsgInt0,
    int1: MsgInt1,
    u64_2: MsgU64_2,
    void0: MsgVoid0,
    void1: MsgVoid1,
    void2: MsgVoid2,
    dbl1: MsgDbl1,
    range1: MsgRange1,
    size0: MsgSize0,
    rect1: MsgRect1,
    pool_push: PoolPush,
    pool_pop: PoolPop,
}

unsafe impl Send for Objc {}
unsafe impl Sync for Objc {}

static OBJC: OnceLock<Option<Objc>> = OnceLock::new();

fn objc() -> Option<&'static Objc> {
    OBJC.get_or_init(|| {
        let lib = crate::lisp::dynlib::open_library(
            "REMACS_OBJC_LIBRARY",
            &[
                "/usr/lib/libobjc.A.dylib",
                "libobjc.A.dylib",
                "libobjc.dylib",
            ],
            "",
            "",
        )?;
        unsafe {
            Some(Objc {
                id0: crate::lisp::dynlib::sym(&lib, b"objc_msgSend\0")?,
                id1: crate::lisp::dynlib::sym(&lib, b"objc_msgSend\0")?,
                int0: crate::lisp::dynlib::sym(&lib, b"objc_msgSend\0")?,
                int1: crate::lisp::dynlib::sym(&lib, b"objc_msgSend\0")?,
                u64_2: crate::lisp::dynlib::sym(&lib, b"objc_msgSend\0")?,
                void0: crate::lisp::dynlib::sym(&lib, b"objc_msgSend\0")?,
                void1: crate::lisp::dynlib::sym(&lib, b"objc_msgSend\0")?,
                void2: crate::lisp::dynlib::sym(&lib, b"objc_msgSend\0")?,
                dbl1: crate::lisp::dynlib::sym(&lib, b"objc_msgSend\0")?,
                range1: crate::lisp::dynlib::sym(&lib, b"objc_msgSend\0")?,
                size0: crate::lisp::dynlib::sym(&lib, b"objc_msgSend\0")?,
                rect1: crate::lisp::dynlib::sym(&lib, b"objc_msgSend\0")?,
                pool_push: crate::lisp::dynlib::sym(&lib, b"objc_autoreleasePoolPush\0")?,
                pool_pop: crate::lisp::dynlib::sym(&lib, b"objc_autoreleasePoolPop\0")?,
                get_class: crate::lisp::dynlib::sym(&lib, b"objc_getClass\0")?,
                sel_name: crate::lisp::dynlib::sym(&lib, b"sel_registerName\0")?,
                _lib: lib,
            })
        }
    })
    .as_ref()
}

fn cls(o: &Objc, name: &str) -> usize {
    let c = std::ffi::CString::new(name).unwrap();
    unsafe { (o.get_class)(c.as_ptr()) }
}

fn sel(o: &Objc, name: &str) -> usize {
    let c = std::ffi::CString::new(name).unwrap();
    unsafe { (o.sel_name)(c.as_ptr()) }
}

/// `[NSString stringWithUTF8String: S]' → autoreleased id.
fn ns_str(o: &Objc, s: &str) -> usize {
    let c = std::ffi::CString::new(s).unwrap_or_default();
    unsafe {
        (o.id1)(
            cls(o, "NSString"),
            sel(o, "stringWithUTF8String:"),
            c.as_ptr() as usize,
        )
    }
}

/// `NSString'/`NSObject' id → Rust String via `UTF8String'.
fn utf8(o: &Objc, obj: usize) -> String {
    if obj == 0 {
        return String::new();
    }
    unsafe {
        let p = (o.id0)(obj, sel(o, "UTF8String"));
        if p == 0 {
            String::new()
        } else {
            std::ffi::CStr::from_ptr(p as *const core::ffi::c_char)
                .to_string_lossy()
                .into_owned()
        }
    }
}

/// Wrap BODY in an autorelease pool (GNU creates temporaries the same
/// way when these are callable during dumping).
fn with_pool<R>(o: &Objc, f: impl FnOnce(&Objc) -> R) -> R {
    unsafe {
        let pool = (o.pool_push)();
        let r = f(o);
        (o.pool_pop)(pool);
        r
    }
}

/// `[NSApplication sharedApplication]' — creates NSApp on first use,
/// like GNU's `init_process_emacs'.
fn ns_app(o: &Objc) -> usize {
    unsafe { (o.id0)(cls(o, "NSApplication"), sel(o, "sharedApplication")) }
}

// ---------------------------------------------------------------------------
// Accessibility check (pure C — ApplicationServices/HIServices)
// ---------------------------------------------------------------------------

type AxTrusted = unsafe extern "C" fn() -> u8;

static AX: OnceLock<Option<AxTrusted>> = OnceLock::new();

fn ax_trusted() -> Option<AxTrusted> {
    *AX.get_or_init(|| {
        let lib = crate::lisp::dynlib::open_library(
            "REMACS_APPSERVICES_LIBRARY",
            &[
                "/System/Library/Frameworks/ApplicationServices.framework/ApplicationServices",
                "ApplicationServices.framework/ApplicationServices",
            ],
            "",
            "",
        )?;
        let f: Option<AxTrusted> =
            unsafe { crate::lisp::dynlib::sym(&lib, b"AXIsProcessTrusted\0") };
        // Keep the library mapped so the pointer stays valid.
        core::mem::forget(lib);
        f
    })
}

fn f_ns_process_is_accessibility_trusted(_i: &mut Interp, _a: crate::lisp::Argv) -> EvalResult {
    let ok = ax_trusted().map(|f| unsafe { f() } != 0).unwrap_or(false);
    Ok(Value::from_bool(ok))
}

// ---------------------------------------------------------------------------
// Pure-string helpers
// ---------------------------------------------------------------------------

/// GNU `Fns_font_name' + `ns_xlfd_to_fontname': non-XLFD names (and
/// fontset names other than `fontset-startup') pass through; otherwise
/// the XLFD family field is extracted, `$'→`-', `_'→` ', and
/// first/after-separator letters are uppercased.  Empty → "Monaco".
fn f_ns_font_name(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let name = match &a[0] {
        Value::Str(s) => s.borrow().clone(),
        _ => return Err(i.wrong_type_mut("stringp", &a[0])),
    };
    if !name.starts_with('-') {
        return Ok(a[0].clone());
    }
    if name.contains("fontset") && !name.contains("fontset-startup") {
        return Ok(a[0].clone());
    }
    // `-%*[^-]-%179[^-]-' (or `--' prefixed): family = 2nd field.
    let body = name.trim_start_matches('-');
    let family = body.split('-').nth(1).unwrap_or_default();
    let mut fam: String = if family.is_empty() {
        "Monaco".to_string()
    } else {
        family.chars().take(179).filter_map(|c| c.unicode()).collect()
    };
    // Uppercase the first char.
    fam = {
        let mut it = fam.chars();
        match it.next() {
            Some(c0) => c0.to_uppercase().collect::<String>() + it.as_str(),
            None => fam,
        }
    };
    // `$'→`-', `_'→` ', uppercasing the following char.
    let bytes = fam.as_bytes().to_vec();
    let mut out = String::with_capacity(fam.len());
    let mut upcase_next = false;
    for (idx, &b) in bytes.iter().enumerate() {
        let ch = b as char;
        if ch == '$' || ch == '_' {
            out.push(if ch == '$' { '-' } else { ' ' });
            upcase_next = true;
            continue;
        }
        if upcase_next && idx > 0 {
            for u in ch.to_uppercase() {
                out.push(u);
            }
            upcase_next = false;
        } else {
            out.push(ch);
        }
    }
    Ok(Value::string(out))
}

// ---------------------------------------------------------------------------
// Dock-tile / process activity (real implementations)
// ---------------------------------------------------------------------------

fn f_ns_list_colors(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU checks FRAMEP (a live frame) when the arg is non-nil.
    if let Some(v) = a.first() {
        if !v.is_nil() && !matches!(v, Value::Frame(_)) {
            return Err(i.wrong_type_mut("framep", v));
        }
    }
    let Some(o) = objc() else {
        return Err(i.error("Window system is not in use or not initialized"));
    };
    Ok(with_pool(o, |o| {
        let mut list = Value::Nil;
        unsafe {
            let lists = (o.id0)(cls(o, "NSColorList"), sel(o, "availableColorLists"));
            let n = (o.int0)(lists, sel(o, "count"));
            for k in 0..n {
                let clist = (o.id1)(lists, sel(o, "objectAtIndex:"), k as usize);
                if clist == 0 {
                    continue;
                }
                let lname = (o.id0)(clist, sel(o, "name"));
                let len = (o.int0)(lname, sel(o, "length"));
                // GNU: include when name < 7 chars or starts "PANTONE".
                let pantone = ns_str(o, "PANTONE");
                let range = (o.range1)(lname, sel(o, "rangeOfString:"), pantone);
                if !(len < 7 || range.location == 0) {
                    continue;
                }
                let keys = (o.id0)(clist, sel(o, "allKeys"));
                let nk = (o.int0)(keys, sel(o, "count"));
                // GNU pushes `reverseObjectEnumerator' order → result is
                // forward allKeys order.
                for j in (0..nk).rev() {
                    let key = (o.id1)(keys, sel(o, "objectAtIndex:"), j as usize);
                    list = Value::cons(Value::string(utf8(o, key)), list);
                }
            }
        }
        list
    }))
}

fn f_ns_badge(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    if !a[0].is_nil() {
        if !matches!(a[0], Value::Str(_)) {
            return Err(i.wrong_type_mut("stringp", &a[0]));
        }
    }
    let Some(o) = objc() else {
        return Err(i.error("Window system is not in use or not initialized"));
    };
    with_pool(o, |o| unsafe {
        let dock = (o.id0)(ns_app(o), sel(o, "dockTile"));
        let label = match &a[0] {
            Value::Str(s) => ns_str(o, &(s.borrow()).to_unicode_lossy()),
            _ => 0,
        };
        (o.void1)(dock, sel(o, "setBadgeLabel:"), label);
    });
    Ok(Value::Nil)
}

static ATTENTION_ID: std::sync::Mutex<i64> = std::sync::Mutex::new(-1);

fn f_ns_request_user_attention(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let Some(o) = objc() else {
        return Err(i.error("Window system is not in use or not initialized"));
    };
    let (is_sym, name) = match &a[0] {
        Value::Sym(s) => (true, i.symbol_name(*s).to_string()),
        _ => (false, String::new()),
    };
    with_pool(o, |o| unsafe {
        let app = ns_app(o);
        let mut guard = ATTENTION_ID.lock().unwrap();
        if *guard != -1 {
            (o.void1)(app, sel(o, "cancelUserAttentionRequest:"), *guard as usize);
            *guard = -1;
        }
        if is_sym {
            let kind: i64 = match name.as_str() {
                // NSCriticalRequest = 0, NSInformationalRequest = 10.
                "critical" => 0,
                "informational" => 10,
                _ => return,
            };
            *guard = (o.int1)(app, sel(o, "requestUserAttention:"), kind as usize);
        }
    });
    Ok(Value::Nil)
}

/// Sleep-block tokens → NSProcessInfo activity objects.
static SLEEP_MAP: OnceLock<std::sync::Mutex<HashMap<u64, usize>>> = OnceLock::new();
static SLEEP_ID: std::sync::Mutex<u64> = std::sync::Mutex::new(0);

fn sleep_map() -> &'static std::sync::Mutex<HashMap<u64, usize>> {
    SLEEP_MAP.get_or_init(|| std::sync::Mutex::new(HashMap::new()))
}

fn f_ns_block_system_sleep(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let why = match &a[0] {
        Value::Nil => None,
        Value::Str(s) => Some(s.borrow().clone()),
        _ => return Err(i.wrong_type_mut("stringp", &a[0])),
    };
    let Some(o) = objc() else {
        return Ok(Value::Nil);
    };
    let token = with_pool(o, |o| unsafe {
        let pi = (o.id0)(cls(o, "NSProcessInfo"), sel(o, "processInfo"));
        if pi == 0 {
            return 0usize;
        }
        // NSActivityUserInitiated | NSActivityIdleSystemSleepDisabled,
        // | NSActivityIdleDisplaySleepDisabled when arg is nil.
        let mut opts: u64 = (1u64 << 40) | 0x00FF_FFFF;
        if a[1].is_nil() {
            opts |= 1u64 << 46;
        }
        let reason = ns_str(o, &why.as_deref().map(|s| s.to_unicode_lossy()).unwrap_or_else(|| "Emacs".into()));
        let act = (o.u64_2)(pi, sel(o, "beginActivityWithOptions:reason:"), opts, reason);
        // The activity object is autoreleased; keep it alive past the
        // pool pop so `endActivity:' can use it later.
        if act != 0 {
            (o.id0)(act, sel(o, "retain"))
        } else {
            0
        }
    });
    if token == 0 {
        return Ok(Value::Nil);
    }
    let mut guard = SLEEP_ID.lock().unwrap();
    *guard += 1;
    sleep_map().lock().unwrap().insert(*guard, token);
    Ok(Value::Int(*guard as i128))
}

fn f_ns_unblock_system_sleep(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let tok = match &a[0] {
        Value::Int(n) if *n >= 0 => *n as u64,
        _ => return Err(i.wrong_type_mut("wholenump", &a[0])),
    };
    let Some(o) = objc() else {
        return Ok(Value::Nil);
    };
    let act = sleep_map().lock().unwrap().remove(&tok);
    match act {
        Some(id) => {
            with_pool(o, |o| unsafe {
                let pi = (o.id0)(cls(o, "NSProcessInfo"), sel(o, "processInfo"));
                (o.void1)(pi, sel(o, "endActivity:"), id);
                (o.void0)(id, sel(o, "release"));
            });
            Ok(Value::t())
        }
        None => Ok(Value::Nil),
    }
}

fn f_ns_progress_indicator(_i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let Some(o) = objc() else {
        return Ok(Value::Nil);
    };
    with_pool(o, |o| unsafe {
        let dock = (o.id0)(ns_app(o), sel(o, "dockTile"));
        if dock == 0 {
            return;
        }
        let content = (o.id0)(dock, sel(o, "contentView"));
        let mut indicator = 0usize;
        if content != 0 {
            let subs = (o.id0)(content, sel(o, "subviews"));
            let n = (o.int0)(subs, sel(o, "count"));
            if n > 0 {
                let last = (o.id0)(subs, sel(o, "lastObject"));
                let lcls = (o.id0)(last, sel(o, "class"));
                let lname = utf8(o, (o.id0)(lcls, sel(o, "description")));
                if lname == "NSLevelIndicator" {
                    indicator = last;
                }
            }
        }
        if indicator == 0 {
            let mut cv = content;
            if cv == 0 {
                let iv = (o.id0)(
                    (o.id0)(cls(o, "NSImageView"), sel(o, "alloc")),
                    sel(o, "init"),
                );
                let icon = (o.id0)(ns_app(o), sel(o, "applicationIconImage"));
                (o.void1)(iv, sel(o, "setImage:"), icon);
                (o.void1)(dock, sel(o, "setContentView:"), iv);
                cv = iv;
            }
            let size = (o.size0)(
                (o.id0)(ns_app(o), sel(o, "applicationIconImage")),
                sel(o, "size"),
            );
            let li_cls = cls(o, "NSLevelIndicator");
            let li = (o.rect1)(
                (o.id0)(li_cls, sel(o, "alloc")),
                sel(o, "initWithFrame:"),
                NsRect {
                    x: 0.0,
                    y: 0.0,
                    w: size.width,
                    h: 0.10 * size.height,
                },
            );
            (o.void1)(li, sel(o, "setWantsLayer:"), 1usize);
            (o.void1)(li, sel(o, "setEnabled:"), 0usize);
            // NSLevelIndicatorStyleContinuousCapacity = 1.
            (o.void1)(li, sel(o, "setLevelIndicatorStyle:"), 1usize);
            let accent = (o.id0)(cls(o, "NSColor"), sel(o, "controlAccentColor"));
            (o.void1)(li, sel(o, "setFillColor:"), accent);
            (o.dbl1)(li, sel(o, "setMinValue:"), 0.0);
            (o.dbl1)(li, sel(o, "setMaxValue:"), 1.0);
            (o.void1)(cv, sel(o, "addSubview:"), li);
            indicator = li;
        }
        let hide = match &a[0] {
            Value::Nil => true,
            Value::Float(f) => !(0.0..=1.0).contains(&**f),
            _ => true,
        };
        let v = match &a[0] {
            Value::Float(f) => **f,
            _ => 0.0,
        };
        if hide {
            (o.dbl1)(indicator, sel(o, "setDoubleValue:"), 0.0);
            (o.void1)(indicator, sel(o, "setHidden:"), 1usize);
        } else {
            (o.dbl1)(indicator, sel(o, "setDoubleValue:"), v);
            (o.void1)(indicator, sel(o, "setHidden:"), 0usize);
        }
        (o.void0)(dock, sel(o, "display"));
    });
    Ok(Value::Nil)
}

// ---------------------------------------------------------------------------
// Window-system-gated functions (GNU `check_window_system' parity)
// ---------------------------------------------------------------------------

/// GNU `check_window_system (NULL)' with no ns terminal:
/// `error ("Window system is not in use or not initialized")'.
fn winsys(i: &mut Interp) -> Flow {
    i.error("Window system is not in use or not initialized")
}

macro_rules! winsys_fn {
    ($f:ident) => {
        fn $f(i: &mut Interp, _a: crate::lisp::Argv) -> EvalResult {
            Err(winsys(i))
        }
    };
}

fn ns_window_system_in_use(i: &Interp) -> bool {
    cfg!(target_os = "macos")
        && i.selected_frame
            .as_ref()
            .is_some_and(|frame| {
                !frame.borrow().dead && crate::editor::frame_is_graphical(i, frame)
            })
}

fn f_ns_get_resource(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    if !ns_window_system_in_use(i) {
        return Err(winsys(i));
    }
    let Value::Str(name) = &a[1] else {
        return Err(i.wrong_type_mut("stringp", &a[1]));
    };
    let Some(o) = objc() else {
        return Err(winsys(i));
    };
    let name = name.borrow().to_unicode_lossy();
    let value = with_pool(o, |o| unsafe {
        let defaults = (o.id0)(cls(o, "NSUserDefaults"), sel(o, "standardUserDefaults"));
        if defaults == 0 {
            return None;
        }
        let key = ns_str(o, &name);
        let object = (o.id1)(defaults, sel(o, "objectForKey:"), key);
        if object == 0 {
            None
        } else {
            let description = (o.id0)(object, sel(o, "description"));
            Some(utf8(o, description))
        }
    });
    Ok(value.map(Value::string).unwrap_or(Value::Nil))
}

fn f_ns_set_resource(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    if !ns_window_system_in_use(i) {
        return Err(winsys(i));
    }
    let Value::Str(name) = &a[1] else {
        return Err(i.wrong_type_mut("stringp", &a[1]));
    };
    let name = name.borrow().to_unicode_lossy();
    let value = match &a[2] {
        Value::Nil => None,
        Value::Str(value) => Some(value.borrow().to_unicode_lossy()),
        other => return Err(i.wrong_type_mut("stringp", other)),
    };
    let Some(o) = objc() else {
        return Err(winsys(i));
    };
    let result = with_pool(o, |o| unsafe {
        let defaults = (o.id0)(cls(o, "NSUserDefaults"), sel(o, "standardUserDefaults"));
        if defaults == 0 {
            return Err("NSUserDefaults is unavailable".to_owned());
        }
        let key = ns_str(o, &name);
        if let Some(value) = &value {
            let value = ns_str(o, value);
            (o.void2)(defaults, sel(o, "setObject:forKey:"), value, key);
        } else {
            (o.void1)(defaults, sel(o, "removeObjectForKey:"), key);
        }
        Ok(())
    });
    result.map_err(|message| i.error(message))?;
    Ok(Value::Nil)
}

fn f_ns_emacs_info_panel(i: &mut Interp, _a: crate::lisp::Argv) -> EvalResult {
    let Some(backend) = i.about_panel_backend.clone() else {
        return Err(winsys(i));
    };
    let Some(frame) = i
        .selected_frame
        .clone()
        .filter(|frame| !frame.borrow().dead)
    else {
        return Err(winsys(i));
    };
    let frame_id = frame.borrow().id;
    let version = i
        .q_soft("emacs-version")
        .filter(|symbol| i.bound_p(*symbol))
        .map(|symbol| {
            i.princ_to_string(&i.symbol_value(symbol))
                .to_unicode_lossy()
        })
        .unwrap_or_else(|| "31.1".to_owned());
    backend
        .show(frame_id, &version)
        .map_err(|error| i.error(format!("Native About panel failed: {error}")))?;
    Ok(Value::Nil)
}

winsys_fn!(f_ns_frame_edges);
winsys_fn!(f_ns_frame_geometry);

fn f_ns_mouse_absolute_pixel_position(i: &mut Interp, _a: crate::lisp::Argv) -> EvalResult {
    let Some(frame) = i
        .selected_frame
        .clone()
        .filter(|frame| !frame.borrow().dead)
    else {
        return Err(winsys(i));
    };
    if !cfg!(target_os = "macos") || !crate::editor::frame_is_graphical(i, &frame) {
        return Err(winsys(i));
    }
    let (x, y) = i.gui_mouse_absolute_position.unwrap_or((0, 0));
    Ok(Value::Cons(Cons::new(Value::Int(x), Value::Int(y))))
}

fn f_ns_set_mouse_absolute_pixel_position(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let Some(frame) = i
        .selected_frame
        .clone()
        .filter(|frame| !frame.borrow().dead)
    else {
        return Err(winsys(i));
    };
    if !cfg!(target_os = "macos") {
        return Err(winsys(i));
    }
    if !crate::editor::frame_is_graphical(i, &frame) {
        return Ok(Value::Nil);
    }
    let x = super::misc::mouse_coordinate(i, &a[0])?;
    let y = super::misc::mouse_coordinate(i, &a[1])?;
    super::misc::warp_mouse_absolute(i, &frame, x, y)?;
    Ok(Value::Nil)
}

fn f_ns_hide_emacs(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let Some(backend) = i.application_visibility_backend.clone() else {
        return Err(winsys(i));
    };
    let result = if a[0].truthy() {
        backend.hide()
    } else {
        backend.activate()
    };
    result.map_err(|error| i.error(format!("Application visibility failed: {error}")))?;
    Ok(Value::Nil)
}

fn f_ns_hide_others(i: &mut Interp, _a: crate::lisp::Argv) -> EvalResult {
    let Some(backend) = i.application_visibility_backend.clone() else {
        return Err(winsys(i));
    };
    backend
        .hide_other_apps()
        .map_err(|error| i.error(format!("Hiding other applications failed: {error}")))?;
    Ok(Value::Nil)
}

fn f_ns_read_file_name(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    fn optional_string(i: &mut Interp, value: Option<&Value>) -> Result<Option<String>, Flow> {
        match value {
            None | Some(Value::Nil) => Ok(None),
            Some(value) => super::want_string(i, value)
                .map(|text| Some(text.to_unicode_lossy())),
        }
    }

    let prompt = super::want_string(i, &a[0])?.to_unicode_lossy();
    let directory = optional_string(i, a.get(1))?;
    let must_match = a.get(2).is_some_and(Value::truthy);
    let initial = optional_string(i, a.get(3))?;
    let directory_only = a.get(4).is_some_and(Value::truthy);
    let Some(backend) = i.file_dialog_backend.clone() else {
        return Err(winsys(i));
    };
    let selected = backend
        .read_file_name(
            &prompt,
            directory.as_deref(),
            must_match,
            initial.as_deref(),
            directory_only,
        )
        .map_err(|error| i.error(format!("File dialog failed: {error}")))?;
    Ok(selected.map(Value::string).unwrap_or(Value::Nil))
}

fn f_ns_popup_color_panel(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let frame = crate::editor::frame_live(i, &a.first().cloned().unwrap_or(Value::Nil))?;
    if !crate::editor::frame_is_graphical(i, &frame) {
        return Err(winsys(i));
    }
    let Some(backend) = i.color_panel_backend.clone() else {
        return Err(winsys(i));
    };
    backend
        .show(frame.borrow().id)
        .map_err(|error| i.error(format!("Color panel failed: {error}")))?;
    Ok(Value::Nil)
}

fn f_ns_show_character_palette(i: &mut Interp, _a: crate::lisp::Argv) -> EvalResult {
    let Some(frame) = i.selected_frame.clone() else {
        return Err(winsys(i));
    };
    if !crate::editor::frame_is_graphical(i, &frame) {
        return Err(winsys(i));
    }
    let Some(backend) = i.color_panel_backend.clone() else {
        return Err(winsys(i));
    };
    backend
        .show_character_palette(frame.borrow().id)
        .map_err(|error| i.error(format!("Character palette failed: {error}")))?;
    Ok(Value::Nil)
}

fn selection_symbol(i: &mut Interp, value: &Value) -> Result<SymId, Flow> {
    match value {
        Value::Nil => Ok(i.q("PRIMARY")),
        Value::Sym(symbol) => Ok(*symbol),
        other => Err(i.wrong_type_mut("symbolp", other)),
    }
}

fn selection_data_text(value: &Value) -> Option<String> {
    match value {
        Value::Str(text) => Some(text.borrow().to_unicode_lossy()),
        _ => None,
    }
}

fn require_selection_backend(
    i: &mut Interp,
) -> Result<std::rc::Rc<dyn crate::frontend::ClipboardBackend>, Flow> {
    i.clipboard_backend
        .clone()
        .ok_or_else(|| winsys(i))
}

fn f_ns_own_selection_internal(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let selection = selection_symbol(i, &a[0])?;
    let clipboard = i.q("CLIPBOARD");
    let value = a[1].clone();
    let backend = require_selection_backend(i)?;
    if selection == clipboard
        && let Some(text) = selection_data_text(&value)
    {
        backend
            .write_text(text)
            .map_err(|error| i.error(format!("Clipboard write failed: {error}")))?;
    }
    i.nsgui_selections.insert(selection, value.clone());
    Ok(value)
}

fn f_ns_disown_selection_internal(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let selection = selection_symbol(i, &a[0])?;
    let clipboard = i.q("CLIPBOARD");
    let backend = require_selection_backend(i)?;
    if selection == clipboard
        && let Some(owned_text) = i
            .nsgui_selections
            .get(&selection)
            .and_then(selection_data_text)
    {
        let current = backend
            .read_text()
            .map_err(|error| i.error(format!("Clipboard read failed: {error}")))?;
        if current.as_deref() == Some(owned_text.as_str()) {
            backend
                .write_text(String::new())
                .map_err(|error| i.error(format!("Clipboard write failed: {error}")))?;
        }
    }
    i.nsgui_selections.remove(&selection);
    Ok(Value::Nil)
}

fn f_ns_get_selection(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let selection = selection_symbol(i, &a[0])?;
    let target = match &a[1] {
        Value::Sym(target) => *target,
        other => return Err(i.wrong_type_mut("symbolp", other)),
    };
    let backend = require_selection_backend(i)?;
    let clipboard = i.q("CLIPBOARD");
    let target_name = i.symbol_name(target);
    if target_name == "TIMESTAMP" {
        return Ok(Value::Nil);
    }
    if target_name == "TARGETS" {
        let string = i.q("STRING");
        return Ok(Value::vector(vec![Value::Sym(string)]));
    }
    if !matches!(target_name.as_str(), "STRING" | "TEXT" | "UTF8_STRING") {
        return Ok(Value::Nil);
    }
    if selection == clipboard {
        let text = backend
            .read_text()
            .map_err(|error| i.error(format!("Clipboard read failed: {error}")))?;
        return Ok(text.map(Value::string).unwrap_or(Value::Nil));
    }
    Ok(i.nsgui_selections
        .get(&selection)
        .and_then(selection_data_text)
        .map(Value::string)
        .unwrap_or(Value::Nil))
}

fn f_ns_selection_exists_p(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let selection = selection_symbol(i, a.first().unwrap_or(&Value::Nil))?;
    let clipboard = i.q("CLIPBOARD");
    let Some(backend) = i.clipboard_backend.clone() else {
        return Ok(Value::Nil);
    };
    let exists = if selection == clipboard {
        backend
            .read_text()
            .map_err(|error| i.error(format!("Clipboard read failed: {error}")))?
            .is_some()
    } else {
        i.nsgui_selections.contains_key(&selection)
    };
    Ok(Value::from_bool(exists))
}

fn f_ns_selection_owner_p(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let selection = selection_symbol(i, a.first().unwrap_or(&Value::Nil))?;
    let clipboard = i.q("CLIPBOARD");
    let backend = require_selection_backend(i)?;
    let owns = if selection == clipboard {
        let selected = i
            .nsgui_selections
            .get(&selection)
            .and_then(selection_data_text);
        match selected {
            Some(selected) => backend
                .read_text()
                .map_err(|error| i.error(format!("Clipboard read failed: {error}")))?
                .is_some_and(|current| current == selected),
            None => false,
        }
    } else {
        i.nsgui_selections.contains_key(&selection)
    };
    Ok(Value::from_bool(owns))
}

/// `ns-send-items': GNU nsselect.m CHECK_LISTs ITEMS and checks each
/// element is a string ("Element not a string") before the
/// window-system check is reached.
fn f_ns_send_items(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let mut cur = a[0].clone();
    loop {
        match cur {
            Value::Nil => break,
            Value::Cons(c) => {
                let (car, cdr) = {
                    let cb = c.borrow();
                    (cb.car.clone(), cb.cdr.clone())
                };
                if !matches!(car, Value::Str(_)) {
                    let err = i.q("error");
                    return Err(
                        i.signal_data(err, vec![Value::string("Element not a string"), car])
                    );
                }
                cur = cdr;
            }
            other => return Err(i.wrong_type_mut("listp", &other)),
        }
    }
    Err(winsys(i))
}

/// `ns-frame-restack': GNU nsfns.m decodes FRAME1 then FRAME2 with
/// decode_live_frame, then hits check_window_system.
fn f_ns_frame_restack(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let _ = crate::editor::frame_live(i, &a[0])?;
    let _ = crate::editor::frame_live(i, &a[1])?;
    Err(winsys(i))
}

/// `ns-begin-drag': GNU nsselect.m rejects a nil PBOARD first, then decodes
/// the frame and waits for the host drag session to complete.
fn f_ns_begin_drag(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    if a[1].is_nil() {
        return Err(i.error("Empty pasteboard"));
    }
    let frame = crate::editor::frame_live(i, &a[0])?;
    if !crate::editor::frame_is_graphical(i, &frame) {
        return Err(i.error("Window system frame should be used"));
    }
    let Some(backend) = i.drag_backend.clone() else {
        return Err(winsys(i));
    };

    let entries = a[1]
        .list_to_vec()
        .map_err(|_| i.wrong_type_mut("listp", &a[1]))?;
    let mut text = None;
    for entry in entries {
        let Value::Cons(entry) = entry else {
            continue;
        };
        let (kind, value) = {
            let entry = entry.borrow();
            (entry.car.clone(), entry.cdr.clone())
        };
        if matches!(&kind, Value::Sym(symbol) if i.symbol_name(*symbol).as_str() == "string") {
            text = Some(super::want_string(i, &value)?.to_unicode_lossy());
            break;
        }
    }
    let Some(text) = text else {
        return Ok(Value::Nil);
    };
    let action = match a.get(2) {
        None | Some(Value::Nil) => crate::frontend::DragAction::Copy,
        Some(Value::Sym(symbol)) => match i.symbol_name(*symbol).as_str() {
            "XdndActionCopy" => crate::frontend::DragAction::Copy,
            "XdndActionMove" => crate::frontend::DragAction::Move,
            "XdndActionLink" => crate::frontend::DragAction::Link,
            "XdndActionPrivate" => crate::frontend::DragAction::Private,
            _ => return Err(i.error("Invalid drag action")),
        },
        Some(value) => return Err(i.wrong_type_mut("symbolp", value)),
    };
    let result = backend
        .begin_text_drag(frame.borrow().id, &text, action)
        .map_err(|error| i.error(format!("Drag and drop failed: {error}")))?;
    let result = match result {
        None => Value::Nil,
        Some(crate::frontend::DragAction::Copy) => Value::Sym(i.intern("XdndActionCopy")),
        Some(crate::frontend::DragAction::Move) => Value::Sym(i.intern("XdndActionMove")),
        Some(crate::frontend::DragAction::Link) => Value::Sym(i.intern("XdndActionLink")),
        Some(crate::frontend::DragAction::Private) => Value::Sym(i.intern("XdndActionPrivate")),
    };
    Ok(result)
}
// f_ns_do_applescript defined below — GNU CHECK_STRINGs SCRIPT first.
winsys_fn!(f_x_apply_session_resources);

/// `ns-perform-service': GNU checks SERVICE is a string *before*
/// `check_window_system'.
fn f_ns_perform_service(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    if !matches!(a[0], Value::Str(_)) {
        return Err(i.wrong_type_mut("stringp", &a[0]));
    }
    Err(winsys(i))
}

/// `ns-do-applescript': GNU CHECK_STRINGs SCRIPT *before*
/// `check_window_system' (nsfns.m order), same as ns-perform-service.
fn f_ns_do_applescript(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    if !matches!(a[0], Value::Str(_)) {
        return Err(i.wrong_type_mut("stringp", &a[0]));
    }
    let Some(backend) = i.apple_script_backend.clone() else {
        return Err(winsys(i));
    };
    let Value::Str(script) = &a[0] else {
        unreachable!();
    };
    let script = script.borrow().to_unicode_lossy();
    let result = backend
        .execute(&script)
        .map_err(|error| i.error(format!("AppleScript failed: {error}")))?;
    Ok(match result {
        crate::frontend::AppleScriptResult::Nil => Value::Nil,
        crate::frontend::AppleScriptResult::String(value) => Value::string(value),
        crate::frontend::AppleScriptResult::Integer(value) => Value::Int(value),
        crate::frontend::AppleScriptResult::Other => Value::t(),
    })
}

// ---------------------------------------------------------------------------
// nil-returning functions (GNU batch behavior)
// ---------------------------------------------------------------------------

fn f_ns_frame_list_z_order(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU: nil when no ns frames exist.
    let _ = (i, a);
    Ok(Value::Nil)
}

fn f_ns_display_monitor_attributes_list(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU: nil on a terminal-only session;
    // the optional TERMINAL arg is terminal-live_p-checked.
    match a.get(0) {
        None | Some(Value::Nil) | Some(Value::Frame(_)) => {}
        Some(w) if crate::editor::is_terminal(i, w) => {}
        Some(other) => return Err(i.wrong_type_mut("terminal-live-p", other)),
    }
    Ok(Value::Nil)
}

fn f_ns_list_services(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU's NS_IMPL_COCOA branch returns nil unconditionally.
    let _ = (i, a);
    Ok(Value::Nil)
}

fn f_ns_reset_menu(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let _ = (i, a);
    Ok(Value::Nil)
}

// ---------------------------------------------------------------------------
// Registration
// ---------------------------------------------------------------------------

pub(crate) static SUBRS: &[Subr] = &[
    S!(
        "ns-font-name",
        1,
        1,
        f_ns_font_name,
        "Determine font PostScript or family name for font NAME."
    ),
    S!(
        "ns-process-is-accessibility-trusted",
        0,
        0,
        f_ns_process_is_accessibility_trusted,
        "Return non-nil if Emacs is trusted by macOS Accessibility."
    ),
    S!(
        "ns-list-colors",
        0,
        1,
        f_ns_list_colors,
        "Return a list of all available colors."
    ),
    S!(
        "ns-badge",
        1,
        1,
        f_ns_badge,
        "Set the app icon badge to BADGE."
    ),
    S!(
        "ns-request-user-attention",
        1,
        1,
        f_ns_request_user_attention,
        "Bounce the app dock icon to request user attention."
    ),
    S!(
        "ns-progress-indicator",
        1,
        1,
        f_ns_progress_indicator,
        "Show a progress indicator on the app dock icon."
    ),
    S!(
        "ns-block-system-sleep",
        2,
        2,
        f_ns_block_system_sleep,
        "Block system idle sleep."
    ),
    S!(
        "ns-unblock-system-sleep",
        1,
        1,
        f_ns_unblock_system_sleep,
        "Unblock system idle sleep."
    ),
    S!(
        "ns-get-resource",
        2,
        2,
        f_ns_get_resource,
        "Return the value of the property NAME of OWNER from the defaults database."
    ),
    S!(
        "ns-set-resource",
        3,
        3,
        f_ns_set_resource,
        "Set property NAME of OWNER to VALUE, from the defaults database."
    ),
    S!(
        "ns-hide-emacs",
        1,
        1,
        f_ns_hide_emacs,
        "Hide Emacs (TYPE says whether to unhide)."
    ),
    S!(
        "ns-hide-others",
        0,
        0,
        f_ns_hide_others,
        "Hide all applications except Emacs."
    ),
    S!(
        "ns-emacs-info-panel",
        0,
        0,
        f_ns_emacs_info_panel,
        "Show the Emacs info panel."
    ),
    S!(
        "ns-popup-color-panel",
        0,
        1,
        f_ns_popup_color_panel,
        "Pop up the NS color panel."
    ),
    S!(
        "ns-read-file-name",
        1,
        5,
        f_ns_read_file_name,
        "Use a graphical panel to read a file name."
    ),
    S!(
        "ns-perform-service",
        2,
        2,
        f_ns_perform_service,
        "Perform Nextstep SERVICE on SEND."
    ),
    S!(
        "ns-send-items",
        1,
        1,
        f_ns_send_items,
        "Send ITEMS to the pasteboard for a service."
    ),
    S!(
        "ns-frame-restack",
        2,
        3,
        f_ns_frame_restack,
        "Restack FRAME1 above FRAME2."
    ),
    S!(
        "ns-frame-edges",
        0,
        2,
        f_ns_frame_edges,
        "Return edge coordinates of FRAME."
    ),
    S!(
        "ns-frame-geometry",
        0,
        1,
        f_ns_frame_geometry,
        "Return geometric attributes of FRAME."
    ),
    S!(
        "ns-frame-list-z-order",
        0,
        1,
        f_ns_frame_list_z_order,
        "Return list of Emacs' frames, in Z (stacking) order."
    ),
    S!(
        "ns-mouse-absolute-pixel-position",
        0,
        0,
        f_ns_mouse_absolute_pixel_position,
        "Return absolute mouse position."
    ),
    S!(
        "ns-set-mouse-absolute-pixel-position",
        2,
        2,
        f_ns_set_mouse_absolute_pixel_position,
        "Move mouse pointer to absolute pixel position."
    ),
    S!(
        "ns-display-monitor-attributes-list",
        0,
        1,
        f_ns_display_monitor_attributes_list,
        "Return list of monitor attributes."
    ),
    S!(
        "ns-show-character-palette",
        0,
        0,
        f_ns_show_character_palette,
        "Show the character palette."
    ),
    S!(
        "ns-own-selection-internal",
        2,
        2,
        f_ns_own_selection_internal,
        "Assert an NS selection of type SELECTION and value VALUE."
    ),
    S!(
        "ns-disown-selection-internal",
        1,
        1,
        f_ns_disown_selection_internal,
        "If we own the selection SELECTION, disown it."
    ),
    S!(
        "ns-get-selection",
        2,
        2,
        f_ns_get_selection,
        "Return text selected from some Nextstep window."
    ),
    S!(
        "ns-selection-exists-p",
        0,
        1,
        f_ns_selection_exists_p,
        "Whether there is an owner for the given X selection."
    ),
    S!(
        "ns-selection-owner-p",
        0,
        1,
        f_ns_selection_owner_p,
        "Whether the current Emacs process owns the given selection."
    ),
    S!(
        "ns-begin-drag",
        3,
        6,
        f_ns_begin_drag,
        "Drag and drop an item described by TARGETS."
    ),
    S!(
        "ns-list-services",
        0,
        0,
        f_ns_list_services,
        "List available Nextstep services."
    ),
    S!(
        "ns-reset-menu",
        0,
        0,
        f_ns_reset_menu,
        "Reset the menu bar."
    ),
    S!(
        "ns-do-applescript",
        1,
        1,
        f_ns_do_applescript,
        "Compile and execute AppleScript SCRIPT."
    ),
    S!(
        "x-apply-session-resources",
        0,
        0,
        f_x_apply_session_resources,
        "Apply session resources."
    ),
];
