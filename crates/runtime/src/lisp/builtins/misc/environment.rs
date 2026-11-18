//! Process environment and operating-system user/group identity.

use crate::lisp::builtins::{arg, want_string};
use crate::lisp::{Interp, error::EvalResult, text::EmacsText, value::Value};
use rustc_hash::FxHashSet;
use std::rc::Rc;

// ---------- environment / user ----------

pub(super) fn f_setenv(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    let mut variable = args[0].clone();
    let mut name = want_string(i, &variable)?;
    let mut value = match args.get(1) {
        Some(Value::Nil) | None => None,
        Some(value) => {
            want_string(i, value)?;
            Some(value.clone())
        }
    };
    let locale_coding_system = i.q("locale-coding-system");
    let locale = if i.bound_p(locale_coding_system) {
        i.symbol_value(locale_coding_system)
    } else {
        Value::Nil
    };
    if !locale.is_nil() && string_is_multibyte(i, &variable) {
        let concat = Value::Sym(i.q("concat"));
        let combined = i.apply(
            &concat,
            vec![variable.clone(), value.clone().unwrap_or(Value::Nil)],
        )?;
        let find_codings = Value::Sym(i.q("find-coding-systems-string"));
        let codings = i.apply(&find_codings, vec![combined])?;
        let codings = match codings.list_to_vec() {
            Ok(codings) => codings,
            Err(_) => return Err(i.wrong_type_mut("listp", &codings)),
        };
        let base_function = Value::Sym(i.q("coding-system-base"));
        let base = i.apply(&base_function, vec![locale.clone()])?;
        let undecided = Value::Sym(i.q("undecided"));
        if !codings
            .first()
            .is_some_and(|coding| crate::lisp::builtins::eq_values(coding, &undecided))
            && !codings
                .iter()
                .any(|coding| crate::lisp::builtins::eq_values(coding, &base))
        {
            let value_text = match &value {
                Some(value) => want_string(i, value)?,
                None => EmacsText::new(),
            };
            return Err(i.error_q(
                "Can't encode `%s=%s' with `locale-coding-system'",
                &[&name, &value_text],
            ));
        }
    }
    // GNU validates the original assignment against locale-coding-system
    // before substituting references from process-environment.
    if args.get(2).is_some_and(|value| value.truthy()) && value.is_some() {
        let substitute = Value::Sym(i.q("substitute-env-vars"));
        let original = value.as_ref().unwrap().clone();
        let substituted = i.apply(&substitute, vec![original])?;
        want_string(i, &substituted)?;
        value = Some(substituted);
    }
    if !locale.is_nil() && string_is_multibyte(i, &variable) {
        variable = super::coding::f_encode_coding_string(i, vec![variable, locale.clone()].into())?;
        name = want_string(i, &variable)?;
    }
    if !locale.is_nil()
        && value
            .as_ref()
            .is_some_and(|value| string_is_multibyte(i, value))
    {
        let original = value.unwrap();
        value = Some(super::coding::f_encode_coding_string(
            i,
            vec![original, locale.clone()].into(),
        )?);
    }
    if name.contains('=') {
        return Err(i.error_q("Environment variable name `%s' contains `='", &[&name]));
    }
    if name == "TZ" {
        let set_time_zone = Value::Sym(i.q("set-time-zone-rule"));
        i.apply(&set_time_zone, vec![value.clone().unwrap_or(Value::Nil)])?;
    }
    // GNU `setenv' only modifies `process-environment', not the OS
    // environment.
    let pe = i.q("process-environment");
    let environment = i.symbol_value(pe);
    let environment = f_setenv_internal(
        i,
        vec![
            environment,
            variable,
            value.clone().unwrap_or(Value::Nil),
            Value::t(),
        ]
        .into(),
    )?;
    i.set_symbol(pe, environment)?;
    Ok(value.unwrap_or(Value::Nil))
}

/// GNU `setenv-internal': (ENV VARIABLE VALUE KEEP-EMPTY) — set
/// VARIABLE to VALUE in the list ENV by side effect; the matching
/// element ("VAR=..." or a bare "VAR") is replaced in place, else
/// "VAR=VALUE" is prepended.  Returns the new list head.
pub(super) fn f_setenv_internal(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    let env = arg(&args, 0);
    let variable = args[1].clone();
    let var = want_string(i, &variable)?;
    let var_multibyte = string_is_multibyte(i, &variable);
    let value_object = args[2].clone();
    let val = match &args[2] {
        Value::Nil => None,
        value => Some(want_string(i, value)?),
    };
    let value_multibyte = string_is_multibyte(i, &value_object);
    let keep_empty = args[3].truthy();
    let prefix = format!("{}=", var);
    let mut cur = env.clone();
    let mut previous = None;
    let mut seen = FxHashSet::default();
    loop {
        if cur.is_nil() {
            break;
        }
        let Value::Cons(cell) = &cur else {
            return Err(i.wrong_type_mut("listp", &cur));
        };
        if !seen.insert(Rc::as_ptr(cell) as usize) {
            return Err(i.signal_data(crate::lisp::obarray::sym::CIRCULAR_LIST, vec![cur.clone()]));
        }
        let (entry, next) = {
            let cell = cell.borrow();
            (cell.car.clone(), cell.cdr.clone())
        };
        let Value::Str(entry_string) = &entry else {
            break;
        };
        let hit = {
            let entry_string = entry_string.borrow();
            *entry_string == var || entry_string.starts_with(&prefix)
        };
        if hit {
            match &val {
                Some(value) => {
                    cell.borrow_mut().car =
                        environment_entry(i, &var, var_multibyte, value, value_multibyte);
                }
                None if keep_empty => {
                    cell.borrow_mut().car = variable.clone();
                }
                None => {
                    if let Some(Value::Cons(previous)) = previous {
                        previous.borrow_mut().cdr = next.clone();
                    } else {
                        return Ok(next);
                    }
                }
            }
            return Ok(env);
        }
        previous = Some(Value::Cons(cell.clone()));
        cur = next;
    }
    match val {
        Some(value) => Ok(Value::cons(
            environment_entry(i, &var, var_multibyte, &value, value_multibyte),
            env,
        )),
        None if keep_empty => Ok(Value::cons(variable, env)),
        None => Ok(env),
    }
}

fn string_is_multibyte(i: &Interp, value: &Value) -> bool {
    matches!(value, Value::Str(string) if !i.is_unibyte_str(string))
}

fn environment_entry(
    i: &mut Interp,
    variable: &EmacsText,
    variable_multibyte: bool,
    value: &EmacsText,
    value_multibyte: bool,
) -> Value {
    let multibyte = variable_multibyte || value_multibyte;
    let mut entry = variable.clone();
    entry.push('=');
    entry.push_str(value);
    let entry = Value::string_with_kind(entry, multibyte);
    if let Value::Str(string) = &entry {
        if multibyte {
            i.mark_multibyte(string);
        } else {
            i.mark_unibyte(string);
        }
    }
    entry
}

pub(super) fn f_user_login_name(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU Fuser_login_name:
    //   if (NILP (uid)) return Vuser_login_name;
    //   CONS_TO_INTEGER (uid, uid_t, id); pw = getpwuid (id);
    if arg(&a, 0).is_nil() {
        let uln = i.q("user-login-name");
        return Ok(i.symbol_value(uln));
    }
    let uid = match cons_to_uid(&a[0]) {
        Some(u) => u,
        None => {
            return Err(i.error("Not an in-range integer, integral float, or cons of integers"));
        }
    };
    Ok(match pw_login_name(uid) {
        Some(n) => Value::string(n),
        None => Value::Nil,
    })
}

/// GNU `CONS_TO_INTEGER' into `uid_t': an integer, an integral
/// float, or a bignum-shaped cons `(HIGH . LOW16)' / `(HIGH MID24
/// . LOW16)'.  Plain integers/floats are range-checked against
/// uid_t; the cons form folds parts then truncates into uid_t.
fn cons_to_uid(v: &Value) -> Option<u32> {
    let n: i128 = match v {
        Value::Int(n) => *n,
        Value::Float(f) => {
            let d = **f;
            if d.fract() != 0.0 || !d.is_finite() {
                return None;
            }
            d as i128
        }
        Value::Cons(c) => {
            let (car, cdr) = {
                let b = c.borrow();
                (b.car.clone(), b.cdr.clone())
            };
            let hi = match car {
                Value::Int(n) if n >= 0 => n,
                _ => return None,
            };
            let n = match cdr {
                Value::Int(lo) if (0..=0xFFFF).contains(&lo) => (hi << 16) | lo,
                Value::Cons(r) => {
                    // Bignum literal: proper 2-list (HI MID LOW).
                    let (mid, midrest) = {
                        let rb = r.borrow();
                        (rb.car.clone(), rb.cdr.clone())
                    };
                    let low = match &midrest {
                        Value::Cons(r2) => {
                            let rb2 = r2.borrow();
                            match (&rb2.car, &rb2.cdr) {
                                (l @ Value::Int(_), Value::Nil) => l.clone(),
                                _ => return None,
                            }
                        }
                        _ => return None,
                    };
                    match (mid, low) {
                        (Value::Int(m), Value::Int(l))
                            if (0..=0xFF_FFFF).contains(&m) && (0..=0xFFFF).contains(&l) =>
                        {
                            (hi << 40) | (m << 16) | l
                        }
                        _ => return None,
                    }
                }
                _ => return None,
            };
            return Some(n as u32);
        }
        _ => return None,
    };
    if !(0..=u32::MAX as i128).contains(&n) {
        return None;
    }
    Some(n as u32)
}

/// `getpwuid (uid)' -> `pw_name'.
fn pw_login_name(uid: u32) -> Option<String> {
    #[cfg(unix)]
    unsafe {
        let pw = libc::getpwuid(uid);
        if !pw.is_null() {
            return Some(
                std::ffi::CStr::from_ptr((*pw).pw_name)
                    .to_string_lossy()
                    .into_owned(),
            );
        }
    }
    let _ = uid;
    None
}

/// The GECOS full-name field (before the first comma) of a uid.
fn gecos_full_name(uid: u32) -> Option<String> {
    #[cfg(unix)]
    unsafe {
        let pw = libc::getpwuid(uid);
        if !pw.is_null() {
            let gecos = std::ffi::CStr::from_ptr((*pw).pw_gecos)
                .to_string_lossy()
                .to_string();
            let name = gecos.split(',').next().unwrap_or("").to_string();
            if !name.is_empty() {
                return Some(name);
            }
            let login = std::ffi::CStr::from_ptr((*pw).pw_name)
                .to_string_lossy()
                .to_string();
            return if login.is_empty() { None } else { Some(login) };
        }
    }
    let _ = uid;
    None
}
pub(super) fn f_user_real_login_name(_i: &mut Interp, _a: crate::lisp::Argv) -> EvalResult {
    // GNU Fuser_real_login_name:
    // `getpwuid (getuid ())' — ignores USER/LOGNAME.
    #[cfg(unix)]
    {
        let uid = unsafe { libc::getuid() };
        return Ok(match pw_login_name(uid) {
            Some(n) => Value::string(n),
            None => Value::Nil,
        });
    }
    #[allow(unreachable_code)]
    Ok(Value::Nil)
}

pub(super) fn f_user_real_uid(_i: &mut Interp, _a: crate::lisp::Argv) -> EvalResult {
    // GNU Fuser_real_uid: getuid ().
    #[cfg(unix)]
    {
        return Ok(Value::Int(unsafe { libc::getuid() } as i128));
    }
    #[allow(unreachable_code)]
    Ok(Value::Int(0))
}

/// GECOS full name for a login *name* (getpwnam path).
fn gecos_full_name_by_login(name: &str) -> Option<String> {
    #[cfg(unix)]
    unsafe {
        let c = std::ffi::CString::new(name).ok()?;
        let pw = libc::getpwnam(c.as_ptr());
        if !pw.is_null() {
            let gecos = std::ffi::CStr::from_ptr((*pw).pw_gecos)
                .to_string_lossy()
                .to_string();
            let name2 = gecos.split(',').next().unwrap_or("").to_string();
            if !name2.is_empty() {
                return Some(name2);
            }
            let login = std::ffi::CStr::from_ptr((*pw).pw_name)
                .to_string_lossy()
                .into_owned();
            return Some(login);
        }
    }
    let _ = name;
    None
}

/// Boot-time `user-login-name' (GNU init_editfns): LOGNAME, then
/// USER, then getpwuid(geteuid()).
pub fn boot_user_login_name() -> String {
    if let Ok(n) = std::env::var("LOGNAME") {
        if !n.is_empty() {
            return n;
        }
    }
    if let Ok(n) = std::env::var("USER") {
        if !n.is_empty() {
            return n;
        }
    }
    #[cfg(unix)]
    unsafe {
        let pw = libc::getpwuid(libc::geteuid());
        if !pw.is_null() {
            return std::ffi::CStr::from_ptr((*pw).pw_name)
                .to_string_lossy()
                .into_owned();
        }
    }
    "unknown".to_string()
}

/// Boot-time `user-real-login-name': getpwuid(getuid()).
pub fn boot_user_real_login_name() -> String {
    #[cfg(unix)]
    unsafe {
        let pw = libc::getpwuid(libc::getuid());
        if !pw.is_null() {
            return std::ffi::CStr::from_ptr((*pw).pw_name)
                .to_string_lossy()
                .into_owned();
        }
    }
    "unknown".to_string()
}

/// Boot-time `user-full-name' (GNU init_editfns): NAME env, else
/// Fuser_full_name(login-name-or-euid), else "unknown".
pub fn boot_user_full_name() -> String {
    if let Ok(n) = std::env::var("NAME") {
        return n;
    }
    // `tem = Fuser_login_name (euid)' when the claimed env name
    // differs from the real login name, else the claimed name.
    let login = boot_user_login_name();
    let real = boot_user_real_login_name();
    let full = if login == real {
        gecos_full_name_by_login(&login)
    } else {
        #[cfg(unix)]
        {
            gecos_full_name(unsafe { libc::geteuid() })
        }
        #[cfg(not(unix))]
        {
            None
        }
    };
    full.unwrap_or_else(|| "unknown".to_string())
}

/// Effective uid for the boot `user-uid' binding.
pub fn boot_geteuid() -> i128 {
    #[cfg(unix)]
    {
        unsafe { libc::geteuid() as i128 }
    }
    #[cfg(not(unix))]
    {
        0
    }
}

/// Real uid for the boot `user-real-uid' binding.
pub fn boot_getuid() -> i128 {
    #[cfg(unix)]
    {
        unsafe { libc::getuid() as i128 }
    }
    #[cfg(not(unix))]
    {
        0
    }
}

/// `user-full-name' — GNU Fuser_full_name: nil → Vuser_full_name;
/// NUMBERP → getpwuid(uid)'s GECOS; STRINGP → getpwnam's GECOS;
/// else "Invalid UID specification".
pub(super) fn f_user_full_name(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let arg0 = arg(&a, 0);
    if arg0.is_nil() {
        let ufn = i.q("user-full-name");
        return Ok(i.symbol_value(ufn));
    }
    match &arg0 {
        // GNU's NUMBERP test: a cons UID fails it, hitting
        // "Invalid UID specification".
        Value::Int(_) | Value::Float(_) => match cons_to_uid(&arg0) {
            Some(uid) => match gecos_full_name(uid) {
                Some(n) => return Ok(Value::string(n)),
                None => return Ok(Value::Nil),
            },
            None => {
                return Err(i.error("Not an in-range integer, integral float, or cons of integers"));
            }
        },
        Value::Str(s) => {
            let name = s.borrow().clone();
            match gecos_full_name_by_login(&(name).to_unicode_lossy()) {
                Some(n) => return Ok(Value::string(n)),
                None => return Ok(Value::Nil),
            }
        }
        _ => return Err(i.error("Invalid UID specification")),
    }
}

pub(super) fn f_user_uid(_i: &mut Interp, _a: crate::lisp::Argv) -> EvalResult {
    // GNU Fuser_uid: geteuid ().
    #[cfg(unix)]
    {
        return Ok(Value::Int(unsafe { libc::geteuid() } as i128));
    }
    #[allow(unreachable_code)]
    Ok(Value::Int(0))
}

pub(super) fn f_system_groups(_i: &mut Interp, _a: crate::lisp::Argv) -> EvalResult {
    // GNU enumerates the whole group database (getgrent), not just
    // the groups the current user belongs to.
    #[cfg(unix)]
    unsafe {
        let mut out = Vec::new();
        libc::setgrent();
        loop {
            let grp = libc::getgrent();
            if grp.is_null() {
                break;
            }
            out.push(Value::string(
                std::ffi::CStr::from_ptr((*grp).gr_name)
                    .to_string_lossy()
                    .into_owned(),
            ));
        }
        libc::endgrent();
        return Ok(Value::list(out));
    }
    #[allow(unreachable_code)]
    Ok(Value::Nil)
}

pub(super) fn f_group_name(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU editfns.c Fgroup_name: non-number
    // non-cons → bare (error "Invalid GID specification"); CONS_TO_INTEGER
    // range/shape failures → the "Not an in-range integer, integral
    // float, or cons of integers" error.
    match &a[0] {
        Value::Int(_) | Value::Float(_) | Value::Cons(_) => {}
        _ => return Err(i.error("Invalid GID specification")),
    }
    let gid = match cons_to_uid(&a[0]) {
        Some(g) => g,
        None => {
            return Err(i.error("Not an in-range integer, integral float, or cons of integers"));
        }
    };
    #[cfg(unix)]
    unsafe {
        let grp = libc::getgrgid(gid as libc::gid_t);
        if !grp.is_null() {
            let name = std::ffi::CStr::from_ptr((*grp).gr_name).to_string_lossy();
            return Ok(Value::string(name.into_owned()));
        }
    }
    Ok(Value::Nil)
}

pub(super) fn f_invocation_name(_i: &mut Interp, _a: crate::lisp::Argv) -> EvalResult {
    let name = std::env::args()
        .next()
        .map(|p| {
            std::path::Path::new(&p)
                .file_name()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or(p)
        })
        .unwrap_or_else(|| "remacs".to_string());
    Ok(Value::string(name))
}

pub(super) fn f_getenv_internal(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    let name = want_string(i, &args[0])?;
    // GNU consults `process-environment' only (it is initialized from
    // the OS environment at startup); shared with filename expansion.
    match crate::editor::lisp_getenv(i, &name) {
        Some(v) => Ok(Value::string(v)),
        None => Ok(Value::Nil),
    }
}

pub(super) fn f_num_processors(_i: &mut Interp, _a: crate::lisp::Argv) -> EvalResult {
    let n = std::thread::available_parallelism()
        .map(|n| n.get() as i128)
        .unwrap_or(1);
    Ok(Value::Int(n))
}

pub(super) fn f_group_gid(_i: &mut Interp, _a: crate::lisp::Argv) -> EvalResult {
    unsafe extern "C" {
        fn getegid() -> u32;
    }
    Ok(Value::Int(unsafe { getegid() } as i128))
}

pub(super) fn f_group_real_gid(_i: &mut Interp, _a: crate::lisp::Argv) -> EvalResult {
    unsafe extern "C" {
        fn getgid() -> u32;
    }
    Ok(Value::Int(unsafe { getgid() } as i128))
}

pub(super) fn f_system_users(_i: &mut Interp, _a: crate::lisp::Argv) -> EvalResult {
    // User names from the passwd database (like GNU's getpwent walk).
    let names: Vec<Value> = std::fs::read_to_string("/etc/passwd")
        .map(|txt| {
            txt.lines()
                .filter_map(|l| l.split(':').next())
                .filter(|n| !n.is_empty() && !n.starts_with('#'))
                .map(Value::string)
                .collect()
        })
        .unwrap_or_default();
    Ok(Value::list(names))
}

pub(super) fn f_locale_info(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let item = match &a[0] {
        Value::Sym(s) => i.symbol_name(*s),
        _ => return Ok(Value::Nil),
    };
    let lang = |n: i32| -> Option<String> {
        unsafe extern "C" {
            fn nl_langinfo(item: i32) -> *const std::ffi::c_char;
            fn setlocale(category: i32, locale: *const std::ffi::c_char)
            -> *const std::ffi::c_char;
        }
        // GNU calls setlocale(LC_ALL, "") at startup; do it lazily here.
        // LC_ALL is 6 on glibc, 0 on BSD/macOS.
        static ONCE: std::sync::Once = std::sync::Once::new();
        let lc_all: i32 = if cfg!(target_os = "linux") { 6 } else { 0 };
        ONCE.call_once(|| unsafe {
            setlocale(lc_all, c"".as_ptr());
        });
        let p = unsafe { nl_langinfo(n) };
        if p.is_null() {
            return None;
        }
        let s = unsafe { std::ffi::CStr::from_ptr(p) }
            .to_string_lossy()
            .into_owned();
        if s.is_empty() { None } else { Some(s) }
    };
    // nl_item constants differ between glibc and BSD/macOS.
    let codeset: i32 = if cfg!(target_os = "linux") { 14 } else { 0 };
    let day1: i32 = if cfg!(target_os = "linux") {
        0x20007
    } else {
        7
    };
    let mon1: i32 = if cfg!(target_os = "linux") {
        0x2000e
    } else {
        21
    };
    match item.as_str() {
        "codeset" => Ok(lang(codeset).map(Value::string).unwrap_or(Value::Nil)),
        "days" => Ok(Value::vector(
            (0..7)
                .map(|d| lang(day1 + d).map(Value::string).unwrap_or(Value::Nil))
                .collect(),
        )),
        "months" => Ok(Value::vector(
            (0..12)
                .map(|m| lang(mon1 + m).map(Value::string).unwrap_or(Value::Nil))
                .collect(),
        )),
        _ => Ok(Value::Nil),
    }
}

#[cfg(test)]
mod circular_environment_tests {
    use super::*;
    use crate::lisp::{error::Flow, obarray::sym};

    #[test]
    fn setenv_internal_rejects_an_unmatched_circular_environment() {
        std::thread::Builder::new()
            .stack_size(64 * 1024 * 1024)
            .spawn(|| {
                let mut interp = Interp::new();
                let environment = Value::cons(Value::string("QA_OTHER=first"), Value::Nil);
                if let Value::Cons(cell) = &environment {
                    cell.borrow_mut().cdr = environment.clone();
                }
                let result = f_setenv_internal(
                    &mut interp,
                    vec![
                        environment,
                        Value::string("QA_TARGET"),
                        Value::string("value"),
                        Value::Nil,
                    ]
                    .into(),
                );
                assert!(matches!(
                    result,
                    Err(Flow::Signal(ref signal)) if signal.is_symbol(sym::CIRCULAR_LIST)
                ));
            })
            .unwrap()
            .join()
            .unwrap();
    }
}
