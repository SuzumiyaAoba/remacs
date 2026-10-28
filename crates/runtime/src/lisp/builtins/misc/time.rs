//! Lisp timestamp representation, arithmetic, timezone conversion and clock primitives.

use crate::lisp::builtins::arg;
use crate::lisp::builtins::want_string;
use crate::lisp::eq_values;
use crate::lisp::{
    Interp,
    error::{EvalResult, Flow},
    obarray::sym,
    value::Value,
};
use num_bigint::BigInt;
use num_integer::Integer;
use num_traits::{ToPrimitive, Zero};

// ---------- time values ----------
//
// Lisp time is (HIGH LOW MICRO PICO) or an integer seconds/ticks. We
// convert to microseconds (i128) internally.

pub(crate) fn lisp_time_to_us(i: &mut Interp, v: &Value) -> Result<i128, Flow> {
    scaled_lisp_time(i, v, 1_000_000)
}

/// Like `lisp_time_to_us`, but nanosecond precision.
pub(crate) fn lisp_time_to_ns(i: &mut Interp, v: &Value) -> Result<i128, Flow> {
    scaled_lisp_time(i, v, 1_000_000_000)
}

fn scaled_lisp_time(i: &mut Interp, v: &Value, scale: i128) -> Result<i128, Flow> {
    let (ticks, hz) = decode_big_ticks_hz(i, v)?;
    (ticks * scale)
        .div_floor(&hz)
        .to_i128()
        .ok_or_else(|| i.error("Specified time is not representable"))
}

/// GNU CFORM_SECS_ONLY ignores fractional fields in old list timestamps.
pub(crate) fn lisp_time_seconds(i: &mut Interp, v: &Value) -> Result<i64, Flow> {
    let (ticks, hz) = decode_time_parts_big(i, v, true)?;
    ticks
        .div_floor(&hz)
        .to_i64()
        .ok_or_else(|| i.error("Specified time is not representable"))
}

pub(crate) fn us_to_lisp_time(us: i128) -> Value {
    let secs = us.div_euclid(1_000_000);
    let micro = us.rem_euclid(1_000_000);
    let hi = secs.div_euclid(65536);
    let lo = secs.rem_euclid(65536);
    Value::list(vec![
        Value::Int(hi as i128),
        Value::Int(lo as i128),
        Value::Int(micro as i128),
        Value::Int(0),
    ])
}

/// GNU (hi lo us ps) timestamp from nanoseconds since the epoch.
pub(crate) fn ns_to_lisp_time(ns: i128) -> Value {
    let secs = ns.div_euclid(1_000_000_000);
    let nano = ns.rem_euclid(1_000_000_000);
    let hi = secs.div_euclid(65536);
    let lo = secs.rem_euclid(65536);
    Value::list(vec![
        Value::Int(hi as i128),
        Value::Int(lo as i128),
        Value::Int(nano / 1000),
        Value::Int((nano % 1000) * 1000),
    ])
}

/// Like `ns_to_lisp_time` but honors `current-time-list' — GNU's
/// `make_lisp_time' renders (TICKS . HZ) with ns ticks when the
/// variable is nil.
pub(crate) fn ns_to_lisp_time_i(i: &mut Interp, ns: i128) -> Value {
    if !current_time_list(i) {
        return Value::cons(Value::Int(ns), Value::Int(1_000_000_000));
    }
    ns_to_lisp_time(ns)
}

/// Like `lisp_time_to_ns`, but picosecond precision — GNU's native
/// tick unit (hz = 10^12).
pub(crate) fn lisp_time_to_ps(i: &mut Interp, v: &Value) -> Result<i128, Flow> {
    scaled_lisp_time(i, v, 1_000_000_000_000)
}

/// Minimal POSIX tm for `localtime_r` (macOS/Linux layout).
#[repr(C)]
#[derive(Clone, Copy)]
pub(crate) struct Tm {
    pub tm_sec: i32,
    pub tm_min: i32,
    pub tm_hour: i32,
    pub tm_mday: i32,
    pub tm_mon: i32,
    pub tm_year: i32,
    pub tm_wday: i32,
    pub tm_yday: i32,
    pub tm_isdst: i32,
    pub tm_gmtoff: i64,
    pub tm_zone: *const u8,
}

unsafe extern "C" {
    pub(crate) fn localtime_r(timep: *const i64, result: *mut Tm) -> *mut Tm;
    pub(crate) fn gmtime_r(timep: *const i64, result: *mut Tm) -> *mut Tm;
    fn tzset();
    fn mktime(tm: *mut Tm) -> i64;
}

/// Process-global TZ lock: `tz_local_tm' and `mktime_secs' flip TZ +
/// call `tzset', and ambient readers (`local_tm', `mktime' with no
/// zone) depend on the CURRENT TZ.  GNU Emacs is effectively
/// single-threaded here; parallel Rust test threads race, so
/// serialize every TZ-sensitive section (holders call
/// `local_tm_locked' instead of `local_tm' — no recursion).
/// `set-time-zone-rule' itself never touches process state: it stores
/// the rule on the Interp (`tz_rule') and `interp_local_tm' applies it.
pub(crate) fn tz_lock() -> std::sync::MutexGuard<'static, ()> {
    static L: std::sync::Mutex<()> = std::sync::Mutex::new(());
    L.lock().unwrap_or_else(|p| p.into_inner())
}

/// Run a time conversion with TZ temporarily set.  The caller must hold
/// `tz_lock` while using this helper.
fn with_tz<R>(zone: Option<&str>, f: impl FnOnce() -> R) -> R {
    if let Some(zone) = zone {
        // NUL inside the spec would panic std::env::set_var; a NUL
        // string never names a real zone — fall back to UTC like tzset.
        let zone = if zone.contains('\0') { "UTC0" } else { zone };
        let saved = std::env::var_os("TZ");
        unsafe {
            std::env::set_var("TZ", zone);
            tzset();
        }
        let result = f();
        unsafe {
            match saved {
                Some(value) => std::env::set_var("TZ", value),
                None => std::env::remove_var("TZ"),
            }
            tzset();
        }
        result
    } else {
        f()
    }
}

/// Local-time breakdown for a unix-second timestamp.
pub(crate) fn local_tm(secs: i64) -> Tm {
    let _g = tz_lock();
    let mut tm = unsafe { std::mem::zeroed::<Tm>() };
    unsafe { localtime_r(&secs, &mut tm) };
    tm
}

/// Same as `local_tm' but for callers already holding `tz_lock'.
fn local_tm_locked(secs: i64) -> Tm {
    let mut tm = unsafe { std::mem::zeroed::<Tm>() };
    unsafe { localtime_r(&secs, &mut tm) };
    tm
}

/// Local-time breakdown honoring the interpreter's `tz_rule'
/// (set by `set-time-zone-rule'); `None' falls back to the ambient
/// zone.  This is the single entry point every "local zone" reader
/// should use so one `Interp's rule can't leak into another's.
pub(crate) fn interp_local_tm(i: &Interp, secs: i64) -> Tm {
    interp_local_tm_named(i, secs).0
}

/// `interp_local_tm' plus the resolved zone abbreviation — under a
/// rule the breakdown comes from `tz_local_tm', whose restore
/// `tzset()' invalidates `tm_zone', so the name must come from the
/// pre-restore copy, not the returned `Tm' (which reads as garbage
/// like "LMT" on macOS).
pub(crate) fn interp_local_tm_named(i: &Interp, secs: i64) -> (Tm, String) {
    match &i.tz_rule {
        Some(z) => tz_local_tm(secs, z),
        None => {
            let t = local_tm(secs);
            let name = if t.tm_zone.is_null() {
                String::new()
            } else {
                unsafe { std::ffi::CStr::from_ptr(t.tm_zone as *const i8) }
                    .to_string_lossy()
                    .into_owned()
            };
            (t, name)
        }
    }
}

/// UTC breakdown for a unix-second timestamp.
pub(crate) fn gmt_tm(secs: i64) -> Tm {
    let mut tm = unsafe { std::mem::zeroed::<Tm>() };
    unsafe { gmtime_r(&secs, &mut tm) };
    tm
}

/// Run one `strftime` conversion against a broken-down time.  Returns the
/// locale-formatted text (e.g. `%a` -> weekday abbrev, `%p` -> AM/PM).
/// The C library's locale is initialized once from the environment, like
/// GNU does at startup; an explicit LOCALE (`system-time-locale')
/// temporarily overrides it — GNU's Fformat_time_string does exactly
/// this pair of setlocale calls, falling back to "C" for unknown
/// locale names.
pub(crate) fn strftime_spec(fmt: &str, tm: &Tm, locale: Option<&str>) -> Option<String> {
    // setlocale/strftime share process-global C state — serialize with
    // the TZ lock so parallel threads can't observe a half-switched
    // locale (or vice versa).
    let _g = tz_lock();
    static INIT: std::sync::Once = std::sync::Once::new();
    INIT.call_once(|| unsafe {
        libc::setlocale(libc::LC_TIME, c"".as_ptr());
    });
    // Only portable, unadorned locale directives reach libc. Field widths,
    // flags, numeric conversions and GNU extensions live in timefmt.rs.
    // Match gnulib's underlying_strftime buffer and sentinel: locale
    // composites can include a long caller-supplied zone abbreviation, and
    // a leading space distinguishes valid empty locale output from failure.
    let mut cfmt = format!(" {fmt}").into_bytes();
    cfmt.push(0);
    let mut buf = vec![0u8; 1024];
    // Our `Tm' mirrors the POSIX `struct tm' layout used by strftime.
    let n = if let Some(loc) = locale {
        unsafe {
            let cloc = std::ffi::CString::new(loc).unwrap_or_default();
            if libc::setlocale(libc::LC_TIME, cloc.as_ptr()).is_null() {
                // GNU: an unusable locale name falls back to "C".
                libc::setlocale(libc::LC_TIME, c"C".as_ptr());
            }
            let n = libc::strftime(
                buf.as_mut_ptr() as *mut i8,
                buf.len(),
                cfmt.as_ptr() as *const i8,
                tm as *const Tm as *const libc::tm,
            );
            libc::setlocale(libc::LC_TIME, c"".as_ptr());
            n
        }
    } else {
        unsafe {
            libc::strftime(
                buf.as_mut_ptr() as *mut i8,
                buf.len(),
                cfmt.as_ptr() as *const i8,
                tm as *const Tm as *const libc::tm,
            )
        }
    };
    if n == 0 {
        None
    } else {
        Some(String::from_utf8_lossy(&buf[1..n]).into_owned())
    }
}

/// Local-time breakdown under an explicit TZ spec (e.g. "Europe/Paris",
/// "UTC"), restoring the process TZ afterwards.  Also returns the
/// zone's abbreviation (`tm_zone`) — the pointer is copied BEFORE the
/// restore `tzset()` because on macOS it aliases libc-internal zone
/// state that `tzset` invalidates (reads as stale "LMT" garbage).
pub(crate) fn tz_local_tm(secs: i64, zone: &str) -> (Tm, String) {
    // GNU passes the TZ rule as a C string: ignore the suffix after NUL.
    let zone = zone.split('\0').next().unwrap_or("");
    let _g = tz_lock();
    let saved = std::env::var_os("TZ");
    unsafe {
        std::env::set_var("TZ", zone);
        tzset();
    }
    let tm = local_tm_locked(secs);
    let name = if tm.tm_zone.is_null() {
        String::new()
    } else {
        unsafe { std::ffi::CStr::from_ptr(tm.tm_zone as *const i8) }
            .to_string_lossy()
            .into_owned()
    };
    unsafe {
        match saved {
            Some(v) => std::env::set_var("TZ", v),
            None => std::env::remove_var("TZ"),
        }
        tzset();
    }
    (tm, name)
}

/// Days since 1970-01-01 for (y, m, d) — Howard Hinnant's algorithm.
pub(super) fn days_from_civil(y: i128, m: i128, d: i128) -> i128 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146097 + doe - 719468
}

/// Resolve a local wall-clock time by iterating the UTC offset around
/// its naive UTC equivalent.  Caller must hold `tz_lock`.
fn fallback_local_mktime(
    result: i64,
    sec: i32,
    min: i32,
    hour: i32,
    day: i32,
    mon: i32,
    year: i32,
    isdst: i32,
) -> Option<i64> {
    let total_m = (year as i128 + 1900) * 12 + mon as i128;
    let (year, month) = (total_m.div_euclid(12), total_m.rem_euclid(12) + 1);
    let wall_seconds = days_from_civil(year, month, day as i128)
        .checked_mul(86400)?
        .checked_add(hour as i128 * 3600)?
        .checked_add(min as i128 * 60)?
        .checked_add(sec as i128)?;
    let wall_seconds = i64::try_from(wall_seconds).ok()?;
    let expected = gmt_tm(wall_seconds);
    let matches = |tm: &Tm| {
        tm.tm_year == expected.tm_year
            && tm.tm_mon == expected.tm_mon
            && tm.tm_mday == expected.tm_mday
            && tm.tm_hour == expected.tm_hour
            && tm.tm_min == expected.tm_min
            && tm.tm_sec == expected.tm_sec
            && (isdst < 0 || (tm.tm_isdst > 0) == (isdst > 0))
    };
    if matches(&local_tm_locked(result)) {
        return None;
    }

    let mut candidate = wall_seconds;
    for _ in 0..4 {
        let offset = local_tm_locked(candidate).tm_gmtoff;
        let adjusted = wall_seconds.checked_sub(offset)?;
        if adjusted == candidate {
            break;
        }
        candidate = adjusted;
    }
    matches(&local_tm_locked(candidate)).then_some(candidate)
}

fn time_tm_field(i: &mut Interp, value: i128) -> Result<i32, Flow> {
    i32::try_from(value).map_err(|_| i.error("Specified time is not representable"))
}

/// POSIX mktime: interpret a (possibly out-of-range) broken-down time
/// in the given TZ spec — None means the ambient local zone — honoring
/// tm_isdst (-1 guess / 0 standard / >0 DST).  Returns epoch seconds.
fn mktime_secs(
    sec: i32,
    min: i32,
    hour: i32,
    day: i32,
    mon: i32,
    year: i32,
    isdst: i32,
    tz: Option<&str>,
) -> i64 {
    let _g = tz_lock();
    let mut tm: Tm = unsafe { std::mem::zeroed() };
    tm.tm_sec = sec;
    tm.tm_min = min;
    tm.tm_hour = hour;
    tm.tm_mday = day;
    tm.tm_mon = mon;
    tm.tm_year = year;
    tm.tm_isdst = isdst;
    // GNU's gnulib mktime normalizes a forced tm_isdst to the zone's
    // real flag: an isdst=1 request in a non-DST zone (e.g. JST) must
    // produce the standard-offset instant, not the blindly-shifted
    // result libc mktime returns.  Pass 1 asks with the requested flag;
    // pass 2 re-runs with the flag mktime resolved to, which GNU
    // guarantees matches the zone's rule.
    // Run `f' with TZ=Z (restored after).  Named-zone mktime and the
    // roundtrip's localtime must share the same zone, so the TZ flip
    // brackets both passes.
    // GNU's gnulib mktime (lib/mktime.c): libc mktime on a forced
    // isdst=1 blind-shifts -1h even when the zone never observes DST
    // (e.g. Asia/Tokyo).  GNU resolves the request by probing
    // localtime around the result: when the resolved isdst differs
    // from the request, it scans ±1yr for an instant whose resolved
    // flag matches and extrapolates back; none found → keep the
    // zone's resolved interpretation.  Emulate: first pass asks libc,
    // then a localtime roundtrip normalizes fields under the resolved
    // flag; when the request is still unmet and the zone never shows
    // DST at the probed instants, the resolved result stands.
    let r = with_tz(tz, || unsafe { mktime(&mut tm) });
    if isdst >= 0 && (tm.tm_isdst > 0) != (isdst > 0) {
        // Resolved flag disagrees with the request.  Find a nearby
        // instant (gnulib probes ±1yr, stride 601200s) whose resolved
        // flag matches the request; extrapolating its offset back is
        // exactly what libc already did on pass 1, so KEEP r when a
        // matching probe exists — the blind shift is the honored
        // interpretation.  Otherwise re-run under the resolved flag.
        let mut found = false;
        for delta in (601200i64..366 * 86400 + 601200).step_by(601200) {
            for dir in [-1i64, 1] {
                let ot = r.saturating_add(delta.saturating_mul(dir));
                let resolved = with_tz(tz, || {
                    let mut otm: Tm = unsafe { std::mem::zeroed() };
                    unsafe { localtime_r(&ot, &mut otm) };
                    otm.tm_isdst as i64
                });
                if (resolved > 0) == (isdst > 0) {
                    found = true;
                    break;
                }
            }
            if found {
                break;
            }
        }
        if !found {
            // Zone never shows the requested flag: return the
            // zone-resolved interpretation — re-run with the flag
            // libc resolved (fields were normalized in place, so
            // rebuild the tm from the request first).
            let mut tm2: Tm = unsafe { std::mem::zeroed() };
            tm2.tm_sec = sec;
            tm2.tm_min = min;
            tm2.tm_hour = hour;
            tm2.tm_mday = day;
            tm2.tm_mon = mon;
            tm2.tm_year = year;
            tm2.tm_isdst = if tm.tm_isdst > 0 { 1 } else { 0 };
            return with_tz(tz, || unsafe { mktime(&mut tm2) });
        }
    }
    if let Some(fallback) = with_tz(tz, || {
        fallback_local_mktime(r, sec, min, hour, day, mon, year, isdst)
    }) {
        return fallback;
    }
    r
}

/// How `encode-time' interprets ZONE: a fixed seconds-east-of-UTC
/// offset (t, "UTC", integer) uses pure calendar arithmetic; nil uses
/// the interpreter's zone rule (or ambient when unset), `wall' is the
/// startup wall clock (GNU's `wall_tz', immune to `set-time-zone-rule'),
/// other strings name a TZ spec.
enum ZoneKind {
    Fixed(i128),
    /// nil: the interpreter's current rule, or ambient zone.
    Local,
    /// `wall': ambient system zone regardless of `set-time-zone-rule'.
    Wall,
    Named(String),
}

pub(crate) const MAX_FIXED_ZONE_OFFSET: i128 = i64::MAX as i128;

pub(crate) fn valid_fixed_zone_offset(offset: i128) -> bool {
    (i64::MIN as i128..=MAX_FIXED_ZONE_OFFSET).contains(&offset)
}

pub(crate) fn invalid_time_zone(i: &mut Interp, zone: &Value) -> Flow {
    i.signal_data(
        sym::ERROR,
        vec![
            Value::string("Invalid time zone specification"),
            zone.clone(),
        ],
    )
}

pub(crate) fn checked_time_zone_cons(i: &mut Interp, zone: &Value) -> Result<(i128, String), Flow> {
    let Value::Cons(pair) = zone else {
        return Err(invalid_time_zone(i, zone));
    };
    let (offset, tail) = {
        let pair = pair.borrow();
        (pair.car.clone(), pair.cdr.clone())
    };
    let Value::Int(offset) = offset else {
        return Err(invalid_time_zone(i, zone));
    };
    let Value::Cons(name_cell) = tail else {
        return Err(invalid_time_zone(i, zone));
    };
    let (name, end) = {
        let name_cell = name_cell.borrow();
        (name_cell.car.clone(), name_cell.cdr.clone())
    };
    let Value::Str(name) = name else {
        return Err(invalid_time_zone(i, zone));
    };
    if !end.is_nil() || !valid_fixed_zone_offset(offset) {
        return Err(invalid_time_zone(i, zone));
    }
    Ok((offset, (name.borrow().clone()).to_unicode_lossy()))
}

fn decode_zone(i: &mut Interp, zone: &Value) -> Result<ZoneKind, Flow> {
    let t_sym = i.q("t");
    let wall_sym = i.q("wall");
    // GNU decode_time_zone: t = UTC, "UTC"/"t" = UTC, integer = fixed
    // offset, a cons uses the car as a fixed offset, nil and `wall'
    // mean local; anything else signals "Invalid time zone
    // specification".
    Ok(match zone {
        Value::Int(off) if zone.fixnump() => ZoneKind::Fixed(if valid_fixed_zone_offset(*off) {
            *off
        } else {
            0
        }),
        Value::Sym(s) if *s == t_sym => ZoneKind::Fixed(0),
        Value::Sym(s) if *s == wall_sym => ZoneKind::Wall,
        Value::Str(z) => {
            let z = z.borrow().clone();
            if z == "UTC" || z == "t" {
                ZoneKind::Fixed(0)
            } else {
                ZoneKind::Named((z).to_unicode_lossy())
            }
        }
        Value::Cons(_) => ZoneKind::Fixed(checked_time_zone_cons(i, zone)?.0),
        Value::Nil => ZoneKind::Local,
        v => return Err(invalid_time_zone(i, v)),
    })
}

pub(crate) fn time_tm(i: &mut Interp, secs: i64, zone: &Value) -> Result<Tm, Flow> {
    Ok(match decode_zone(i, zone)? {
        ZoneKind::Fixed(offset) => {
            let adjusted = (secs as i128)
                .checked_add(offset)
                .and_then(|n| i64::try_from(n).ok())
                .ok_or_else(|| i.error("Specified time is not representable"))?;
            gmt_tm(adjusted)
        }
        ZoneKind::Local => interp_local_tm(i, secs),
        ZoneKind::Wall => local_tm(secs),
        ZoneKind::Named(zone) => tz_local_tm(secs, &zone).0,
    })
}

pub(super) fn f_encode_time(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    // (encode-time TIME) where TIME is a list, or the obsolete
    // (encode-time SECOND MINUTE HOUR DAY MONTH YEAR &rest) form.
    // GNU signals `wrong-type-argument (consp X)' for a lone
    // non-list argument.
    // GNU: arities 2..5 signal `wrong-number-of-arguments (encode-time N)'
    // — only 1-arg (list) or ≥6-arg calls are valid.
    if args.len() >= 2 && args.len() <= 5 {
        let f = Value::Sym(i.q("encode-time"));
        return Err(i.wrong_number_of_args(&f, args.len() as i128));
    }
    let items: Vec<Value> = if args.len() == 1 {
        let mut items = Vec::new();
        let mut tail = args[0].clone();
        for _ in 0..6 {
            let Value::Cons(cell) = tail else {
                return Err(i.wrong_type_mut("consp", &tail));
            };
            let cell = cell.borrow();
            items.push(cell.car.clone());
            tail = cell.cdr.clone();
        }
        if !tail.is_nil() {
            for _ in 0..3 {
                let Value::Cons(cell) = tail else {
                    return Err(i.wrong_type_mut("consp", &tail));
                };
                let cell = cell.borrow();
                items.push(cell.car.clone());
                tail = cell.cdr.clone();
            }
        }
        items
    } else {
        args.as_slice().to_vec()
    };
    // GNU's Fencode_time CHECK_FIXNUMs the five fields after SEC —
    // nil (missing) elements signal `wrong-type-argument (fixnump nil)'.
    let get = |i: &mut Interp, k: usize| -> Result<i128, Flow> {
        match items.get(k) {
            Some(Value::Int(n)) => Ok(*n),
            other => Err(i.wrong_type_mut("fixnump", &other.cloned().unwrap_or(Value::Nil))),
        }
    };
    // SEC may be any Lisp time value — a fixnum, (HI LO), or
    // (TICKS . HZ); whole seconds add to the wall time and the
    // fraction becomes the sub-second part of the result.
    let (sec_whole, sub_ps) = match items.first() {
        Some(v @ (Value::Cons(_) | Value::Int(_) | Value::Float(_))) => {
            let (ticks, hz) = decode_ticks_hz(i, v)?;
            if hz <= 0 {
                return Err(i.error("Invalid time specification"));
            }
            (
                ticks.div_euclid(hz),
                ticks.rem_euclid(hz) * 1_000_000_000_000 / hz,
            )
        }
        other => return Err(i.wrong_type_mut("fixnump", &other.cloned().unwrap_or(Value::Nil))),
    };
    let (min, hour, day, mon, year) = (get(i, 1)?, get(i, 2)?, get(i, 3)?, get(i, 4)?, get(i, 5)?);
    // GNU's obsolescent calling convention (the call wasn't the
    // single-list form): when more than 6 args are given the LAST
    // argument is ZONE — middle extras are ignored — and DST is -1.
    // The one-list form reads ZONE at index 8 and DST at index 7.
    let (zone, isdst) = if args.len() > 1 {
        let zone = if items.len() > 6 {
            items.last().cloned().unwrap_or(Value::Nil)
        } else {
            Value::Nil
        };
        (zone, -1i32)
    } else {
        let zone = items.get(8).cloned().unwrap_or(Value::Nil);
        let t_sym = i.q("t");
        let isdst = match items.get(7) {
            Some(Value::Int(n)) => (*n).clamp(-1, 1) as i32,
            Some(Value::Sym(s)) if *s == t_sym => 1,
            // GNU list form: DST nil means "not in effect" (0), -1 asks
            // mktime to guess; absent fields guess too.
            Some(Value::Nil) => 0,
            _ => -1,
        };
        (zone, isdst)
    };
    // Every broken-down field is ultimately stored in a C `struct tm`.
    // Reject values that cannot be represented there before either the
    // libc or fixed-zone path can wrap or normalize a truncated value.
    let tm_sec = time_tm_field(i, sec_whole)?;
    let tm_min = time_tm_field(i, min)?;
    let tm_hour = time_tm_field(i, hour)?;
    let tm_mday = time_tm_field(i, day)?;
    let tm_mon = time_tm_field(i, mon - 1)?;
    let tm_year = time_tm_field(i, year - 1900)?;
    let secs = match decode_zone(i, &zone)? {
        ZoneKind::Fixed(off) => {
            // Arithmetic path: normalize overflowing fields like
            // mktime does (Jan 32 -> Feb 1, hour 24 -> next day).
            let total_m = year * 12 + (mon - 1);
            let (ny, nm0) = (total_m.div_euclid(12), total_m.rem_euclid(12) + 1);
            let days = days_from_civil(ny, nm0, day);
            let seconds = days
                .checked_mul(86400)
                .and_then(|n| n.checked_add(hour.checked_mul(3600)?))
                .and_then(|n| n.checked_add(min.checked_mul(60)?))
                .and_then(|n| n.checked_add(sec_whole))
                .and_then(|n| n.checked_sub(off))
                .ok_or_else(|| i.error("Specified time is not representable"))?;
            i64::try_from(seconds)
                .map(i128::from)
                .map_err(|_| i.error("Specified time is not representable"))?
        }
        ZoneKind::Local => mktime_secs(
            tm_sec,
            tm_min,
            tm_hour,
            tm_mday,
            tm_mon,
            tm_year,
            isdst,
            // The interpreter's `set-time-zone-rule', when set, IS the
            // local zone — don't read the process `TZ'.
            i.tz_rule.as_deref(),
        ) as i128,
        ZoneKind::Named(z) => mktime_secs(
            tm_sec,
            tm_min,
            tm_hour,
            tm_mday,
            tm_mon,
            tm_year,
            isdst,
            Some(&z),
        ) as i128,
        ZoneKind::Wall => mktime_secs(
            tm_sec, tm_min, tm_hour, tm_mday, tm_mon, tm_year, isdst, None,
        ) as i128,
    };
    // Emacs returns (HIGH LOW), plus (HIGH LOW USEC PSEC) when the
    // SEC argument carried a sub-second fraction.
    let hi = secs.div_euclid(65536);
    let lo = secs.rem_euclid(65536);
    if sub_ps == 0 {
        Ok(Value::list(vec![Value::Int(hi), Value::Int(lo)]))
    } else {
        Ok(Value::list(vec![
            Value::Int(hi),
            Value::Int(lo),
            Value::Int(sub_ps / 1_000_000),
            Value::Int(sub_ps % 1_000_000),
        ]))
    }
}

pub(super) fn f_decode_time(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    let (ticks, hz) = if matches!(arg(&args, 2), Value::Sym(sym::T)) {
        decode_ticks_hz(i, &arg(&args, 0))?
    } else {
        (lisp_time_seconds(i, &arg(&args, 0))? as i128, 1)
    };
    let secs = i64::try_from(ticks.div_euclid(hz))
        .map_err(|_| i.error("Specified time is not representable"))?;
    // GNU decode_time_zone: nil/`wall' = local wall clock, t and
    // "UTC"/"Universal" = UTC, integer = fixed seconds-east offset,
    // other strings name a TZ spec, a (OFFSET . NAME) cons uses car.
    // Anything else signals "Invalid time zone specification".
    let t_sym = i.q("t");
    let wall_sym = i.q("wall");
    let (tm, zone) = match args.get(1) {
        Some(Value::Int(off)) if valid_fixed_zone_offset(*off) => {
            let adjusted = secs
                .checked_add(*off as i64)
                .ok_or_else(|| i.error("Specified time is not representable"))?;
            (gmt_tm(adjusted), *off)
        }
        Some(Value::Int(_)) => (gmt_tm(secs), 0),
        Some(Value::Sym(s)) if *s == t_sym => (gmt_tm(secs), 0),
        Some(Value::Sym(s)) if *s == wall_sym => {
            // `wall' is the system wall clock — GNU keeps a separate
            // `wall_tz' allocated at startup, so `set-time-zone-rule'
            // does NOT affect it; use the ambient zone, not tz_rule.
            let t = local_tm(secs);
            let off = t.tm_gmtoff as i128;
            (t, off)
        }
        Some(Value::Str(z)) => {
            let z = z.borrow().clone();
            if z == "UTC" || z == "t" {
                (gmt_tm(secs), 0)
            } else {
                let (t, _zone_name) = tz_local_tm(secs, &(z).to_unicode_lossy());
                let off = t.tm_gmtoff as i128;
                (t, off)
            }
        }
        Some(zone @ Value::Cons(_)) => {
            let (offset, _) = checked_time_zone_cons(i, zone)?;
            let adjusted = secs
                .checked_add(offset as i64)
                .ok_or_else(|| i.error("Specified time is not representable"))?;
            (gmt_tm(adjusted), offset)
        }
        Some(Value::Nil) | None => {
            let t = interp_local_tm(i, secs);
            let off = t.tm_gmtoff as i128;
            (t, off)
        }
        Some(v) => {
            return Err(i.signal_data(
                sym::ERROR,
                vec![Value::string("Invalid time zone specification"), v.clone()],
            ));
        }
    };
    Ok(Value::list(vec![
        if hz == 1 {
            Value::Int(tm.tm_sec as i128)
        } else {
            let seconds = (tm.tm_sec as i128)
                .checked_mul(hz)
                .and_then(|n| n.checked_add(ticks.rem_euclid(hz)))
                .ok_or_else(|| i.error("Specified time is not representable"))?;
            Value::cons(Value::Int(seconds), Value::Int(hz))
        },
        Value::Int(tm.tm_min as i128),
        Value::Int(tm.tm_hour as i128),
        Value::Int(tm.tm_mday as i128),
        Value::Int(tm.tm_mon as i128 + 1),
        Value::Int(tm.tm_year as i128 + 1900),
        Value::Int(tm.tm_wday as i128),
        Value::from_bool(tm.tm_isdst > 0),
        Value::Int(zone),
    ]))
}

/// `current-time-list': GNU defaults it to t; when the variable is
/// unbound (or bound non-nil) timestamps render in (HI LO US PS)
/// list form rather than (TICKS . HZ).
pub(crate) fn current_time_list(i: &mut Interp) -> bool {
    let id = i.q("current-time-list");
    !i.bound_p(id) || i.symbol_value(id).truthy()
}

/// Current timestamp for C-style callers such as window display.
/// GNU Fcurrent_time honors current-time-list and retains nanoseconds.
pub(crate) fn current_time_value(i: &mut Interp) -> Value {
    let ticks = match std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH) {
        Ok(duration) => duration.as_nanos() as i128,
        Err(error) => -(error.duration().as_nanos() as i128),
    };
    if current_time_list(i) {
        ticks_hz_list4(ticks, 1_000_000_000)
    } else {
        Value::cons(Value::Int(ticks), Value::Int(1_000_000_000))
    }
}

/// Decode a Lisp time value to GNU's (TICKS . HZ) pair, per
/// decode_lisp_time in timefns.c (CFORM_TICKS_HZ):
///   int N      -> (N . 1)
///   float F    -> (F * 1e12 . 1e12)
///   (T . HZ)   -> (T . HZ), HZ > 0 required
///   (HI LO)    -> (HI*65536 + LO . 1)
///   (HI LO US) / (HI LO . US)  -> (*1e6 . 1e6)
///   (HI LO US PS ...)          -> (*1e12 . 1e12)
pub(crate) fn decode_ticks_hz(i: &mut Interp, v: &Value) -> Result<(i128, i128), Flow> {
    decode_time_parts(i, v, false)
}

/// Exact integer tick representation used by the Lisp time operations.
pub(crate) fn decode_big_ticks_hz(i: &mut Interp, value: &Value) -> Result<(BigInt, BigInt), Flow> {
    decode_time_parts_big(i, value, false)
}
fn decode_time_parts_big(
    i: &mut Interp,
    value: &Value,
    seconds_only: bool,
) -> Result<(BigInt, BigInt), Flow> {
    if let Some(n) = value.as_bigint() {
        return Ok((n, BigInt::from(1)));
    }
    match value {
        Value::Nil => {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default();
            Ok((BigInt::from(now.as_nanos()), BigInt::from(1_000_000_000)))
        }
        Value::Float(n) => {
            let n = **n;
            if n.is_nan() {
                return Err(i.error("Invalid time specification"));
            }
            if !n.is_finite() {
                return Err(i.error("Specified time is not representable"));
            }
            if n == 0.0 {
                return Ok((BigInt::from(0), BigInt::from(1)));
            }
            // GNU double_integer_scale retains trailing mantissa zeros.
            let bits = n.to_bits();
            let exponent = ((bits >> 52) & 0x7ff) as i32;
            let fraction = bits & ((1u64 << 52) - 1);
            let (mantissa, exponent) = if exponent == 0 {
                (fraction, -1074)
            } else {
                (fraction | (1u64 << 52), exponent - 1075)
            };
            let mut ticks = BigInt::from(mantissa);
            let hz = if exponent < 0 {
                BigInt::from(1) << (-exponent) as usize
            } else {
                ticks <<= exponent as usize;
                BigInt::from(1)
            };
            if n.is_sign_negative() {
                ticks = -ticks;
            }
            Ok((ticks, hz))
        }
        Value::Cons(first) => {
            let (high, tail) = {
                let first = first.borrow();
                (first.car.clone(), first.cdr.clone())
            };
            let high = high
                .as_bigint()
                .ok_or_else(|| i.error("Invalid time specification"))?;
            let Value::Cons(second) = tail else {
                let hz = tail
                    .as_bigint()
                    .filter(|n| n.sign() == num_bigint::Sign::Plus)
                    .ok_or_else(|| i.error("Invalid time specification"))?;
                return Ok((high, hz));
            };
            let (low, tail) = {
                let second = second.borrow();
                (second.car.clone(), second.cdr.clone())
            };
            let low = low
                .as_bigint()
                .ok_or_else(|| i.error("Invalid time specification"))?;
            let seconds = high * 65536 + low;
            if seconds_only || tail.is_nil() {
                return Ok((seconds, BigInt::from(1)));
            }
            let (usec, psec, hz) = match tail {
                Value::Cons(third) => {
                    let third = third.borrow();
                    match &third.cdr {
                        Value::Cons(fourth) => (
                            third.car.clone(),
                            fourth.borrow().car.clone(),
                            1_000_000_000_000i128,
                        ),
                        _ => (third.car.clone(), Value::Int(0), 1_000_000),
                    }
                }
                usec => (usec, Value::Int(0), 1_000_000),
            };
            if !usec.fixnump() || !psec.fixnump() {
                return Err(i.error("Invalid time specification"));
            }
            let ticks = seconds * hz + usec.int().unwrap() * (hz / 1_000_000) + psec.int().unwrap();
            Ok((ticks, BigInt::from(hz)))
        }
        _ => Err(i.error("Invalid time specification")),
    }
}

/// Bounded projection for platform date/time interfaces. Keep the exact
/// frequency when it fits; otherwise retain the picoseconds they can use.
fn decode_time_parts(
    i: &mut Interp,
    value: &Value,
    seconds_only: bool,
) -> Result<(i128, i128), Flow> {
    let (ticks, hz) = decode_time_parts_big(i, value, seconds_only)?;
    if let (Some(ticks), Some(hz)) = (ticks.to_i128(), hz.to_i128()) {
        return Ok((ticks, hz));
    }
    let common = ticks.gcd(&hz);
    let (ticks, hz) = (ticks / &common, hz / common);
    if let (Some(ticks), Some(hz)) = (ticks.to_i128(), hz.to_i128()) {
        return Ok((ticks, hz));
    }
    let ps: BigInt = (ticks * 1_000_000_000_000i128).div_floor(&hz);
    ps.to_i128()
        .map(|ps| (ps, 1_000_000_000_000))
        .ok_or_else(|| i.error("Specified time is not representable"))
}
fn ticks_hz_list4_big(ticks: BigInt, hz: BigInt) -> Value {
    let trillion = BigInt::from(1_000_000_000_000i128);
    let ps = (ticks * &trillion).div_floor(&hz);
    let (seconds, fraction) = ps.div_mod_floor(&trillion);
    let (high, low) = seconds.div_mod_floor(&BigInt::from(65536));
    let (micro, pico) = fraction.div_mod_floor(&BigInt::from(1_000_000));
    Value::list(vec![
        Value::from_bigint(high),
        Value::from_bigint(low),
        Value::from_bigint(micro),
        Value::from_bigint(pico),
    ])
}
fn ticks_hz_list4(ticks: i128, hz: i128) -> Value {
    ticks_hz_list4_big(ticks.into(), hz.into())
}

/// True for a (TICKS . HZ) cons — a cons whose cdr is not a cons.
fn is_ticks_hz_form(v: &Value) -> bool {
    match v {
        Value::Cons(c) => !matches!(c.borrow().cdr, Value::Cons(_)),
        _ => false,
    }
}

/// GNU timefns.c `time_arith': decode both args to (TICKS . HZ), add or
/// subtract ticks (normalizing via lcm when the frequencies differ, and
/// never returning a resolution coarser than either input), then render:
/// hz == 1 yields an integer; a (TICKS . HZ) input, `current-time-list'
/// nil, or an hz that does not divide 1e12 yields (TICKS . HZ);
/// otherwise the (HI LO US PS) list form.
fn time_arith(i: &mut Interp, a: &[Value], sub: bool) -> EvalResult {
    let (ta, ha) = decode_big_ticks_hz(i, &a[0])?;
    let (tb, hb) = decode_big_ticks_hz(i, &a[1])?;
    let minimum = ha.clone().min(hb.clone());
    let same_frequency = ha == hb;
    let common = ha.gcd(&hb);
    let ma = &hb / &common;
    let mb = &ha / &common;
    let mut hz = ha * &ma;
    let mut ticks = if sub {
        ta * ma - tb * mb
    } else {
        ta * ma + tb * mb
    };
    if !same_frequency {
        // GNU reduces the result, then rescales to at least the smaller
        // input frequency. Keeping the raw lcm can change the output form.
        let reduction = ticks.gcd(&hz);
        if reduction > BigInt::from(1) {
            ticks /= &reduction;
            hz /= reduction;
            if hz < minimum {
                let scale = minimum.div_ceil(&hz);
                ticks *= &scale;
                hz *= scale;
            }
        }
    }
    if hz == BigInt::from(1) {
        return Ok(Value::from_bigint(ticks));
    }
    if is_ticks_hz_form(&a[0])
        || is_ticks_hz_form(&a[1])
        || !current_time_list(i)
        || !(BigInt::from(1_000_000_000_000i128) % &hz).is_zero()
    {
        Ok(Value::cons(
            Value::from_bigint(ticks),
            Value::from_bigint(hz),
        ))
    } else {
        Ok(ticks_hz_list4_big(ticks, hz))
    }
}

pub(super) fn f_time_add(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    time_arith(i, &args, false)
}

pub(super) fn f_time_subtract(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    // GNU: (time-subtract X X) takes the BASE_EQ shortcut and returns
    // zero in make_lisp_time form — (0 0 0 0) under current-time-list.
    if crate::lisp::builtins::eq_values(&args[0], &args[1]) {
        return Ok(if current_time_list(i) {
            Value::list(vec![
                Value::Int(0),
                Value::Int(0),
                Value::Int(0),
                Value::Int(0),
            ])
        } else {
            Value::cons(Value::Int(0), Value::Int(1_000_000_000))
        });
    }
    time_arith(i, &args, true)
}

pub(super) fn f_time_less_p(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    let (a, b) = (&args[0], &args[1]);
    if eq_values(a, b) {
        return Ok(Value::from_bool(false));
    }
    if let (Value::Cons(a), Value::Cons(b)) = (a, b) {
        let (a, b) = (a.borrow(), b.borrow());
        if eq_values(&a.cdr, &b.cdr) {
            if let (Some(a), Some(b)) = (a.car.as_bigint(), b.car.as_bigint()) {
                return Ok(Value::from_bool(a < b));
            }
        }
    }
    let (a, ah) = decode_big_ticks_hz(i, &args[0])?;
    let (b, bh) = decode_big_ticks_hz(i, &args[1])?;
    Ok(Value::from_bool(a * bh < b * ah))
}

pub(super) fn f_time_equal_p(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    let (a, b) = (&args[0], &args[1]);
    if eq_values(a, b) {
        return Ok(Value::from_bool(true));
    }
    if let (Value::Cons(a), Value::Cons(b)) = (a, b) {
        let (a, b) = (a.borrow(), b.borrow());
        if eq_values(&a.cdr, &b.cdr) {
            if let (Some(a), Some(b)) = (a.car.as_bigint(), b.car.as_bigint()) {
                return Ok(Value::from_bool(a == b));
            }
        }
    }
    let (a, ah) = decode_big_ticks_hz(i, &args[0])?;
    let (b, bh) = decode_big_ticks_hz(i, &args[1])?;
    Ok(Value::from_bool(a * bh == b * ah))
}

pub(super) fn f_time_convert(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    let (ticks, hz) = decode_big_ticks_hz(i, &args[0])?;
    let form = args.get(1);
    let name = form
        .and_then(|v| i.sym_id(v))
        .map(|id| i.symbol_name(id))
        .unwrap_or_default();
    let default = form.is_none_or(Value::is_nil);
    if name == "list" || (default && current_time_list(i)) {
        return Ok(ticks_hz_list4_big(ticks, hz));
    }
    if name == "integer" {
        if args[0].integerp() {
            return Ok(args[0].clone());
        }
        return Ok(Value::from_bigint(ticks.div_floor(&hz)));
    }
    if name == "t" || default {
        if let Value::Cons(pair) = &args[0] {
            let pair = pair.borrow();
            if pair.car.integerp() && pair.cdr.integerp() {
                return Ok(args[0].clone());
            }
        }
        if args[0].integerp() {
            return Ok(Value::cons(args[0].clone(), Value::Int(1)));
        }
        return Ok(Value::cons(
            Value::from_bigint(ticks),
            Value::from_bigint(hz),
        ));
    }
    let form = form.unwrap();
    if let Some(frequency) = form
        .as_bigint()
        .filter(|n| n.sign() == num_bigint::Sign::Plus)
    {
        if let Value::Cons(pair) = &args[0] {
            if eq_values(form, &pair.borrow().cdr) {
                return Ok(args[0].clone());
            }
        }
        let converted = (ticks * frequency).div_floor(&hz);
        return Ok(Value::cons(Value::from_bigint(converted), form.clone()));
    }
    Err(i.signal_data(
        sym::ERROR,
        vec![Value::string("Invalid time frequency"), form.clone()],
    ))
}

pub(super) fn f_load_average(_i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // getloadavg(3): 1/5/15-minute averages; GNU USE-FLOATS t
    // returns floats, otherwise scaled integers (*100).
    unsafe extern "C" {
        fn getloadavg(loadavg: *mut f64, nelem: i32) -> i32;
    }
    let use_floats = a.get(0).map(|v| v.truthy()).unwrap_or(false);
    let mut v = [0.0f64; 3];
    let n = unsafe { getloadavg(v.as_mut_ptr(), 3) };
    let items: Vec<Value> = (0..n.max(0) as usize)
        .map(|k| {
            if use_floats {
                Value::float(v[k])
            } else {
                Value::Int((v[k] * 100.0) as i128)
            }
        })
        .collect();
    Ok(Value::list(items))
}

pub(super) fn f_invocation_dir(i: &mut Interp, _a: crate::lisp::Argv) -> EvalResult {
    let dir = std::env::args()
        .next()
        .and_then(|p| {
            std::path::Path::new(&p)
                .parent()
                .map(|s| s.to_string_lossy().into_owned())
        })
        .unwrap_or_default();
    let _ = i;
    Ok(Value::string(dir))
}

pub(super) fn f_build_binding(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // (VAR VALUE &optional BUFFER) — GNU's printed binding shape is
    // (VAR (and VALUE VAR)).  GNU specbinds VAR from a proper-list or
    // symbol binding cell; other atoms signal `(listp ARG)'.
    match &a[0] {
        Value::Sym(_) => {}
        Value::Nil | Value::Cons(_) => {
            // Cycle-safe proper-list check via `list_to_vec' (Dotted
            // carries the offending tail for the error data).
            if let Err(e) = a[0].list_to_vec() {
                let tail = match e {
                    crate::lisp::value::ListError::Dotted(t) => t,
                    crate::lisp::value::ListError::Circular(_) => Value::Nil,
                };
                return Err(i.wrong_type_mut("listp", if tail.is_nil() { &a[0] } else { &tail }));
            }
        }
        other => return Err(i.wrong_type_mut("listp", other)),
    }
    let and = Value::Sym(i.q("and"));
    Ok(Value::list(vec![
        a[0].clone(),
        Value::list(vec![and, a[1].clone(), a[0].clone()]),
    ]))
}

pub(super) fn f_emacs_uptime(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let start = *START;
    let secs = start.elapsed().as_secs() as i128;
    if let Some(fmt) = a.get(0) {
        if fmt.truthy() {
            // GNU applies `format-seconds' to the uptime in seconds
            // (autoloaded from time-date).
            let fs = i.q("format-seconds");
            if !i.fbound_p(fs) {
                let q = |v: Value| Value::list(vec![Value::Sym(sym::QUOTE), v]);
                let req = Value::list(vec![
                    Value::Sym(i.q("require")),
                    q(Value::Sym(i.q("time-date"))),
                ]);
                let _ = i.eval(&req);
            }
            if i.fbound_p(fs) {
                let q = |v: &Value| Value::list(vec![Value::Sym(sym::QUOTE), v.clone()]);
                let args = Value::list(vec![q(fmt), q(&Value::Int(secs))]);
                return i.call_function(&Value::Sym(fs), &args, None);
            }
        }
    }
    Ok(Value::string(uptime_text(secs)))
}

/// `emacs-uptime` default format: "N seconds" / "N days, HH:MM:SS".
pub(crate) fn uptime_text(secs: i128) -> String {
    let days = secs / 86400;
    if days <= 0 {
        format!("{} second{}", secs, if secs == 1 { "" } else { "s" })
    } else {
        let rem = secs % 86400;
        format!(
            "{} day{}, {:02}:{:02}:{:02}",
            days,
            if days == 1 { "" } else { "s" },
            rem / 3600,
            (rem % 3600) / 60,
            rem % 60
        )
    }
}

pub(super) fn f_current_cpu_time(_i: &mut Interp, _a: crate::lisp::Argv) -> EvalResult {
    // GNU sysdep.c `get_current_time' for CPU time: on POSIX it is
    // `clock_gettime(CLOCK_PROCESS_CPUTIME_ID)' with microsecond ticks;
    // 31.1 returns (TICKS . HZ) with HZ = 1000000.
    let us = unsafe {
        let mut ts: libc::timespec = std::mem::zeroed();
        if libc::clock_gettime(libc::CLOCK_PROCESS_CPUTIME_ID, &mut ts) == 0 {
            (ts.tv_sec as i128) * 1_000_000 + (ts.tv_nsec as i128) / 1_000
        } else {
            // getrusage fallback (user + sys).
            let mut ru: libc::rusage = std::mem::zeroed();
            if libc::getrusage(libc::RUSAGE_SELF, &mut ru) == 0 {
                (ru.ru_utime.tv_sec as i128 + ru.ru_stime.tv_sec as i128) * 1_000_000
                    + (ru.ru_utime.tv_usec as i128 + ru.ru_stime.tv_usec as i128)
            } else {
                0
            }
        }
    };
    Ok(Value::cons(Value::Int(us), Value::Int(1_000_000)))
}

static START: std::sync::LazyLock<std::time::Instant> =
    std::sync::LazyLock::new(std::time::Instant::now);

/// `set-time-zone-rule' — GNU's set_time_zone_rule: adopt TZ as the
/// process-local zone (like `(setenv "TZ" TZ)' minus the
/// `process-environment' entry).  nil/`wall' restores system wall
/// clock (TZ unset); t = "UTC0"; an integer offset or a (OFFSET NAME)
/// cons uses a fabricated POSIX spec.  Returns nil.
pub(super) fn f_set_time_zone(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let _g = tz_lock();
    let t_sym = i.q("t");
    let wall_sym = i.q("wall");
    /// GNU's name for an integer zone: sign plus HH, then MM/SS only
    /// when non-zero ("+01", "+0130", "+010101").
    fn offset_name(off: i64) -> String {
        let sign = if off < 0 { '-' } else { '+' };
        let a = off.abs();
        let (h, m, s) = (a / 3600, (a % 3600) / 60, a % 60);
        if s != 0 {
            format!("{}{:02}{:02}{:02}", sign, h, m, s)
        } else if m != 0 {
            format!("{}{:02}{:02}", sign, h, m)
        } else {
            format!("{}{:02}", sign, h)
        }
    }
    /// POSIX TZ field for a fixed offset: POSIX inverts the sign
    /// (the field is what to subtract from local time to get UTC).
    fn posix_off(off: i64) -> String {
        let sign = if off > 0 { '-' } else { '+' };
        let a = off.abs();
        let (h, m, s) = (a / 3600, (a % 3600) / 60, a % 60);
        if s != 0 {
            format!("{}{}:{:02}:{:02}", sign, h, m, s)
        } else if m != 0 {
            format!("{}{}:{:02}", sign, h, m)
        } else {
            format!("{}{}", sign, h)
        }
    }
    /// Fabricate "NAME±H[:M[:S]]"; POSIX needs >=3 chars or <...>
    /// quoting for NAME, and macOS honors "<+01>" but not "<X>", so
    /// short/empty names use the offset's own "+HH" rendering.
    fn fixed_spec(name: &str, off: i64) -> String {
        let name = if name.len() >= 3 {
            name.to_string()
        } else {
            offset_name(off)
        };
        format!("<{}>{}", name, posix_off(off))
    }
    let spec: Option<String> = match &a[0] {
        Value::Str(z) => Some((z.borrow().clone()).to_unicode_lossy()),
        Value::Sym(s) if *s == t_sym => Some("UTC0".to_string()),
        Value::Sym(s) if *s == wall_sym => None,
        Value::Int(off) => Some(fixed_spec("", *off as i64)),
        Value::Cons(c) => {
            let (off, name) = {
                let b = c.borrow();
                let off = match &b.car {
                    Value::Int(n) => *n as i64,
                    _ => 0,
                };
                let name = match &b.cdr {
                    Value::Cons(c2) => match &c2.borrow().car {
                        Value::Str(s) => s.borrow().clone(),
                        _ => String::new().into(),
                    },
                    Value::Str(s) => s.borrow().clone(),
                    _ => (String::new()).into(),
                };
                (off, name)
            };
            Some(fixed_spec(&(name).to_unicode_lossy(), off))
        }
        Value::Nil => None,
        v => {
            return Err(i.signal_data(
                sym::ERROR,
                vec![Value::string("Invalid time zone specification"), v.clone()],
            ));
        }
    };
    // Unlike GNU we do NOT set the process `TZ'/call `tzset': the rule
    // lives on the Interp so parallel interpreters never see each
    // other's zone (libtest runs them in the same process).
    i.tz_rule = spec;
    Ok(Value::Nil)
}

fn parse_date_ymd(s: &str) -> Option<(i64, i64, i64)> {
    // Accept "YYYY-MM-DD" or "YYYY/MM/DD" (with optional trailing time).
    let date = s.split([' ', 'T']).next()?;
    let parts: Vec<&str> = date.split(['-', '/']).collect();
    if parts.len() != 3 {
        return None;
    }
    Some((
        parts[0].parse().ok()?,
        parts[1].parse().ok()?,
        parts[2].parse().ok()?,
    ))
}

pub(super) fn f_days_between(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let s1 = want_string(i, &a[0])?;
    let s2 = want_string(i, &a[1])?;
    match (
        parse_date_ymd(&(s1).to_unicode_lossy()),
        parse_date_ymd(&(s2).to_unicode_lossy()),
    ) {
        (Some((y1, m1, d1)), Some((y2, m2, d2))) => Ok(Value::Int(
            (days_from_civil(y1 as i128, m1 as i128, d1 as i128)
                - days_from_civil(y2 as i128, m2 as i128, d2 as i128)) as i128,
        )),
        _ => Err(i.signal_data(sym::ERROR, vec![Value::string("Invalid date")])),
    }
}

fn month_from_name(tok: &str) -> Option<i64> {
    const MONTHS: [&str; 12] = [
        "jan", "feb", "mar", "apr", "may", "jun", "jul", "aug", "sep", "oct", "nov", "dec",
    ];
    let mut t = tok.to_ascii_lowercase();
    t.truncate(3);
    MONTHS.iter().position(|m| t == *m).map(|p| p as i64 + 1)
}

/// Parse a numeric zone token like `+0900`, `-05:30` into offset seconds.
fn parse_zone_offset(tok: &str) -> Option<i64> {
    let (sign, rest) = match tok.as_bytes().first()? {
        b'+' => (1i64, &tok[1..]),
        b'-' => (-1i64, &tok[1..]),
        _ => return None,
    };
    let digits: String = rest.chars().filter(|c| c.is_ascii_digit()).collect();
    let n: i64 = digits.parse().ok()?;
    if rest.contains(':') || digits.len() <= 2 {
        Some(sign * (n * 3600 + 0))
    } else {
        Some(sign * ((n / 100) * 3600 + (n % 100) * 60))
    }
}

/// Parse the date+time forms GNU `parse-time-string' accepts in practice:
/// ISO `YYYY-MM-DD[T]HH:MM[:SS][Z|±ZZZZ]`, ctime `[Www] Mmm DD HH:MM:SS
/// YYYY`, RFC-822 `DD Mmm YYYY HH:MM:SS ZZZZ`.  Returns the broken-down
/// fields and an explicit zone offset in seconds when the string carried
/// one.
fn parse_date_hms(s: &str) -> Option<(i64, i64, i64, i64, i64, i64, Option<i64>)> {
    let mut year = None;
    let mut month = None;
    let mut day = None;
    let (mut hh, mut mm, mut ss) = (0i64, 0i64, 0i64);
    let mut zone = None;
    for raw in s.split(|c: char| c.is_whitespace() || c == ',') {
        let mut tok = raw;
        if tok.is_empty() {
            continue;
        }
        // ISO date `YYYY-MM-DD' or `YYYY/MM/DD', possibly glued to the
        // time via `T'.
        if tok.len() >= 8
            && tok[..4].chars().all(|c| c.is_ascii_digit())
            && matches!(tok.as_bytes()[4], b'-' | b'/')
        {
            let (dtok, rest) = match tok.find('T') {
                Some(p) => (&tok[..p], &tok[p + 1..]),
                None => (tok, ""),
            };
            let dpart: Vec<&str> = dtok.split(['-', '/']).collect();
            if dpart.len() >= 3 {
                year = dpart[0].parse().ok();
                month = dpart[1].parse().ok();
                day = dpart[2].parse().ok();
            }
            if rest.is_empty() {
                continue;
            }
            tok = rest;
        }
        // HH:MM[:SS][.frac][Z|±ZZZZ]
        if tok.contains(':') && tok.as_bytes()[0].is_ascii_digit() {
            let (hmain, zpart) = match tok.find(|c| c == '+' || c == 'Z' || c == 'z') {
                Some(p) => (&tok[..p], Some(&tok[p..])),
                None => (tok, None),
            };
            let p: Vec<&str> = hmain.split(':').collect();
            if p.len() >= 2 && p[0].chars().all(|c| c.is_ascii_digit()) {
                hh = p[0].parse().ok()?;
                mm = p[1].parse().ok()?;
                if p.len() >= 3 {
                    ss = p[2].split('.').next().unwrap_or("0").parse().unwrap_or(0);
                }
            }
            if let Some(z) = zpart {
                zone = Some(if z.eq_ignore_ascii_case("z") {
                    0
                } else {
                    parse_zone_offset(z).unwrap_or(0)
                });
            }
            continue;
        }
        if let Some(z) = parse_zone_offset(tok) {
            zone = Some(z);
            continue;
        }
        if matches!(tok, "Z" | "z" | "UT" | "UTC" | "GMT" | "ut" | "utc" | "gmt") {
            zone = Some(0);
            continue;
        }
        if let Some(m) = month_from_name(tok) {
            month = Some(m);
            continue;
        }
        if tok.chars().all(|c| c.is_ascii_digit()) {
            match tok.len() {
                4 => year = year.or(tok.parse().ok()),
                1 | 2 => day = day.or(tok.parse().ok()),
                _ => {}
            }
        }
    }
    Some((year?, month?, day?, hh, mm, ss, zone))
}

pub(super) fn f_date_to_time(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let s = want_string(i, &a[0])?;
    if let Some((y, m, d, hh, mm, ss, zone)) = parse_date_hms(&(s).to_unicode_lossy()) {
        let mut secs = days_from_civil(y as i128, m as i128, d as i128) * 86400
            + hh as i128 * 3600
            + mm as i128 * 60
            + ss as i128;
        // Broken-down fields are local wall time unless a zone was given.
        secs -= match zone {
            Some(z) => z as i128,
            None => interp_local_tm(i, secs as i64).tm_gmtoff as i128,
        };
        let hi = secs.div_euclid(65536);
        let lo = secs.rem_euclid(65536);
        return Ok(Value::list(vec![
            Value::Int(hi as i128),
            Value::Int(lo as i128),
        ]));
    }
    // GNU signals (error "Invalid date: DATE") on unparseable input;
    // `safe-date-to-time' relies on catching it.
    Err(i.error(format!("Invalid date: {}", s)))
}

pub(super) fn f_seconds_to_time(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let us = lisp_time_to_us(i, &a[0])?;
    Ok(us_to_lisp_time(us))
}

pub(super) fn f_time_since(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let from = lisp_time_to_us(i, &a[0])?;
    let now = lisp_time_to_us(i, &Value::Nil)?;
    // GNU's nil second arg defers to `current-time-list' (t => `list');
    // `list' yields the (HI LO US PS) form, otherwise (TICKS . HZ).
    if !current_time_list(i) {
        return Ok(Value::cons(
            Value::Int((now - from) * 1000),
            Value::Int(1_000_000_000),
        ));
    }
    Ok(us_to_lisp_time(now - from))
}

pub(super) fn f_time_to_days(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let secs = lisp_time_seconds(i, &a[0])?;
    let tm = interp_local_tm(i, secs);
    // Use floor division for the pseudo-Gregorian year offsets so
    // negative years count leap days before the year-zero boundary.
    let previous_year = tm.tm_year as i128 + 1899;
    Ok(Value::Int(
        tm.tm_yday as i128
            + 1
            + 365 * previous_year
            + previous_year.div_euclid(4)
            - previous_year.div_euclid(100)
            + previous_year.div_euclid(400),
    ))
}

pub(super) fn f_time_to_day_in_year(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let secs = lisp_time_seconds(i, &a[0])?;
    let tm = interp_local_tm(i, secs);
    Ok(Value::Int(tm.tm_yday as i128 + 1))
}

pub(super) fn f_days_to_time(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let days = match &a[0] {
        Value::Int(n) => *n,
        Value::Float(f) => **f as i128,
        other => return Err(i.wrong_type_mut("numberp", other)),
    };
    // GNU returns the (HIGH LOW) seconds form, not a full time value.
    let secs = days * 86400;
    Ok(Value::list(vec![
        Value::Int(secs.div_euclid(65536)),
        Value::Int(secs.rem_euclid(65536)),
    ]))
}

pub(super) fn f_date_leap_year_p(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let y = match &a[0] {
        Value::Int(n) => *n,
        other => return Err(i.wrong_type_mut("integerp", other)),
    };
    Ok(Value::from_bool(
        y % 4 == 0 && (y % 100 != 0 || y % 400 == 0),
    ))
}
