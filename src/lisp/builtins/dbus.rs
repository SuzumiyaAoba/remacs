//! D-Bus primitives — a port of GNU `src/dbusbind.c' against
//! `libdbus-1' loaded at runtime (like `gnutls.rs').
//!
//! Implemented subrs: `dbus--init-bus', `dbus-get-unique-name',
//! `dbus-message-internal', `dbus--fd-open', `dbus--fd-close',
//! `dbus--registered-fds' — plus `read-event', which remacs lacked
//! and which `dbus-call-method' busy-waits on.  Incoming messages
//! are drained into `dbus-event' special events (dispatched through
//! `special-event-map' the way GNU's `read_char' does) from
//! `drain', called by the `sleep_firing_timers' pump and `read-event'.

use std::os::raw::{c_char, c_void};
use std::sync::OnceLock;

use super::hashfn::hash_key_for;
use super::{arg, want_int, want_string, S};
use crate::lisp::error::{EvalResult, Flow};
use crate::lisp::obarray::sym;
use crate::lisp::value::{HashTest, SymId, Value};

type Interp = crate::lisp::eval::Interp;

// ---------------------------------------------------------------------------
// D-Bus constants (dbus-protocol.h)
// ---------------------------------------------------------------------------

const DBUS_BUS_SESSION: i32 = 0;
const DBUS_BUS_SYSTEM: i32 = 1;

const DBUS_MESSAGE_TYPE_INVALID: i32 = 0;
const DBUS_MESSAGE_TYPE_METHOD_CALL: i32 = 1;
const DBUS_MESSAGE_TYPE_METHOD_RETURN: i32 = 2;
const DBUS_MESSAGE_TYPE_ERROR: i32 = 3;
const DBUS_MESSAGE_TYPE_SIGNAL: i32 = 4;

const DBUS_TYPE_INVALID: i32 = 0;
const DBUS_TYPE_ARRAY: i32 = b'a' as i32;
const DBUS_TYPE_VARIANT: i32 = b'v' as i32;
const DBUS_TYPE_STRUCT: i32 = b'r' as i32;
const DBUS_TYPE_DICT_ENTRY: i32 = b'e' as i32;
const DBUS_TYPE_BYTE: i32 = b'y' as i32;
const DBUS_TYPE_BOOLEAN: i32 = b'b' as i32;
const DBUS_TYPE_INT16: i32 = b'n' as i32;
const DBUS_TYPE_UINT16: i32 = b'q' as i32;
const DBUS_TYPE_INT32: i32 = b'i' as i32;
const DBUS_TYPE_UINT32: i32 = b'u' as i32;
const DBUS_TYPE_INT64: i32 = b'x' as i32;
const DBUS_TYPE_UINT64: i32 = b't' as i32;
const DBUS_TYPE_DOUBLE: i32 = b'd' as i32;
const DBUS_TYPE_STRING: i32 = b's' as i32;
const DBUS_TYPE_OBJECT_PATH: i32 = b'o' as i32;
const DBUS_TYPE_SIGNATURE: i32 = b'g' as i32;
const DBUS_TYPE_UNIX_FD: i32 = b'h' as i32;

const DBUS_STRUCT_BEGIN_CHAR: u8 = b'(';
const DBUS_STRUCT_END_CHAR: u8 = b')';
const DBUS_DICT_ENTRY_BEGIN_CHAR: u8 = b'{';
const DBUS_DICT_ENTRY_END_CHAR: u8 = b'}';
const DBUS_MAXIMUM_SIGNATURE_LENGTH: usize = 255;

const DBUS_DISPATCH_COMPLETE: i32 = 1;

fn is_basic_dtype(dtype: i32) -> bool {
    matches!(
        dtype,
        DBUS_TYPE_BYTE
            | DBUS_TYPE_BOOLEAN
            | DBUS_TYPE_INT16
            | DBUS_TYPE_UINT16
            | DBUS_TYPE_INT32
            | DBUS_TYPE_UINT32
            | DBUS_TYPE_INT64
            | DBUS_TYPE_UINT64
            | DBUS_TYPE_DOUBLE
            | DBUS_TYPE_STRING
            | DBUS_TYPE_OBJECT_PATH
            | DBUS_TYPE_SIGNATURE
            | DBUS_TYPE_UNIX_FD
    )
}

// ---------------------------------------------------------------------------
// FFI — dynamically loaded libdbus-1
// ---------------------------------------------------------------------------

/// Public `DBusError' layout (dbus/dbus-errors.h).
#[repr(C)]
pub struct DBusError {
    name: *const c_char,
    message: *const c_char,
    /// bitfield dummy1..dummy5 packed into one unsigned int.
    dummy: u32,
    padding1: *mut c_void,
}

/// Opaque `DBusMessageIter' — the public struct is 72 bytes on
/// LP64 (2 pointers + u32 + 8 ints + int + 2 pointers); give it
/// generous headroom, libdbus only writes into it.
#[repr(C)]
pub struct DBusMessageIter {
    opaque: [usize; 32],
}

impl DBusMessageIter {
    fn new() -> Self {
        DBusMessageIter { opaque: [0; 32] }
    }
}

macro_rules! dbus_fns {
    ($($name:ident : $ty:ty),* $(,)?) => {
        pub struct Dbus {
            _lib: libloading::Library,
            $($name: $ty,)*
        }
        fn load_dbus() -> Option<Dbus> {
            let lib = crate::lisp::dynlib::open_library(
                "REMACS_DBUS_LIBRARY",
                &["libdbus-1.dylib", "libdbus-1.so.3", "libdbus-1.so"],
                "dbus",
                "libdbus-1.dylib",
            )?;
            unsafe {
                Some(Dbus {
                    $($name: crate::lisp::dynlib::sym(&lib, concat!(stringify!($name), "\0").as_bytes())?,)*
                    _lib: lib,
                })
            }
        }
    };
}

dbus_fns! {
    dbus_error_init: unsafe extern "C" fn(*mut DBusError),
    dbus_error_free: unsafe extern "C" fn(*mut DBusError),
    dbus_error_is_set: unsafe extern "C" fn(*const DBusError) -> u32,
    dbus_parse_address: unsafe extern "C" fn(*const c_char, *mut *mut c_void, *mut i32, *mut DBusError) -> u32,
    dbus_address_entries_free: unsafe extern "C" fn(*mut c_void),
    dbus_bus_get: unsafe extern "C" fn(i32, *mut DBusError) -> *mut c_void,
    dbus_bus_get_private: unsafe extern "C" fn(i32, *mut DBusError) -> *mut c_void,
    dbus_connection_open: unsafe extern "C" fn(*const c_char, *mut DBusError) -> *mut c_void,
    dbus_connection_open_private: unsafe extern "C" fn(*const c_char, *mut DBusError) -> *mut c_void,
    dbus_bus_register: unsafe extern "C" fn(*mut c_void, *mut DBusError) -> u32,
    dbus_connection_set_exit_on_disconnect: unsafe extern "C" fn(*mut c_void, u32),
    dbus_connection_ref: unsafe extern "C" fn(*mut c_void) -> *mut c_void,
    dbus_connection_unref: unsafe extern "C" fn(*mut c_void),
    dbus_connection_close: unsafe extern "C" fn(*mut c_void),
    dbus_connection_get_is_connected: unsafe extern "C" fn(*mut c_void) -> u32,
    dbus_connection_flush: unsafe extern "C" fn(*mut c_void),
    dbus_bus_get_unique_name: unsafe extern "C" fn(*mut c_void) -> *const c_char,
    dbus_bus_name_has_owner: unsafe extern "C" fn(*mut c_void, *const c_char, *mut DBusError) -> u32,
    dbus_validate_bus_name: unsafe extern "C" fn(*const c_char, *mut DBusError) -> u32,
    dbus_validate_path: unsafe extern "C" fn(*const c_char, *mut DBusError) -> u32,
    dbus_validate_interface: unsafe extern "C" fn(*const c_char, *mut DBusError) -> u32,
    dbus_validate_member: unsafe extern "C" fn(*const c_char, *mut DBusError) -> u32,
    dbus_get_version: unsafe extern "C" fn(*mut i32, *mut i32, *mut i32),
    dbus_message_new: unsafe extern "C" fn(i32) -> *mut c_void,
    dbus_message_unref: unsafe extern "C" fn(*mut c_void),
    dbus_message_set_destination: unsafe extern "C" fn(*mut c_void, *const c_char) -> u32,
    dbus_message_set_path: unsafe extern "C" fn(*mut c_void, *const c_char) -> u32,
    dbus_message_set_interface: unsafe extern "C" fn(*mut c_void, *const c_char) -> u32,
    dbus_message_set_member: unsafe extern "C" fn(*mut c_void, *const c_char) -> u32,
    dbus_message_set_error_name: unsafe extern "C" fn(*mut c_void, *const c_char) -> u32,
    dbus_message_set_reply_serial: unsafe extern "C" fn(*mut c_void, u32) -> u32,
    dbus_message_set_allow_interactive_authorization: unsafe extern "C" fn(*mut c_void, u32),
    dbus_message_get_type: unsafe extern "C" fn(*mut c_void) -> i32,
    dbus_message_get_serial: unsafe extern "C" fn(*mut c_void) -> u32,
    dbus_message_get_reply_serial: unsafe extern "C" fn(*mut c_void) -> u32,
    dbus_message_get_sender: unsafe extern "C" fn(*mut c_void) -> *const c_char,
    dbus_message_get_destination: unsafe extern "C" fn(*mut c_void) -> *const c_char,
    dbus_message_get_path: unsafe extern "C" fn(*mut c_void) -> *const c_char,
    dbus_message_get_interface: unsafe extern "C" fn(*mut c_void) -> *const c_char,
    dbus_message_get_member: unsafe extern "C" fn(*mut c_void) -> *const c_char,
    dbus_message_get_error_name: unsafe extern "C" fn(*mut c_void) -> *const c_char,
    dbus_message_iter_init: unsafe extern "C" fn(*mut c_void, *mut DBusMessageIter) -> u32,
    dbus_message_iter_get_arg_type: unsafe extern "C" fn(*mut DBusMessageIter) -> i32,
    dbus_message_iter_get_basic: unsafe extern "C" fn(*mut DBusMessageIter, *mut c_void),
    dbus_message_iter_recurse: unsafe extern "C" fn(*mut DBusMessageIter, *mut DBusMessageIter),
    dbus_message_iter_next: unsafe extern "C" fn(*mut DBusMessageIter) -> u32,
    dbus_message_iter_init_append: unsafe extern "C" fn(*mut c_void, *mut DBusMessageIter),
    dbus_message_iter_append_basic: unsafe extern "C" fn(*mut DBusMessageIter, i32, *const c_void) -> u32,
    dbus_message_iter_open_container: unsafe extern "C" fn(*mut DBusMessageIter, i32, *const c_char, *mut DBusMessageIter) -> u32,
    dbus_message_iter_close_container: unsafe extern "C" fn(*mut DBusMessageIter, *mut DBusMessageIter) -> u32,
    dbus_connection_send: unsafe extern "C" fn(*mut c_void, *mut c_void, *mut u32) -> u32,
    dbus_connection_send_with_reply: unsafe extern "C" fn(*mut c_void, *mut c_void, *mut *mut c_void, i32) -> u32,
    dbus_connection_read_write: unsafe extern "C" fn(*mut c_void, i32) -> u32,
    dbus_connection_pop_message: unsafe extern "C" fn(*mut c_void) -> *mut c_void,
    dbus_connection_get_dispatch_status: unsafe extern "C" fn(*mut c_void) -> i32,
}

static DBUS: OnceLock<Option<Dbus>> = OnceLock::new();

fn dbus() -> Option<&'static Dbus> {
    DBUS.get_or_init(load_dbus).as_ref()
}

// ---------------------------------------------------------------------------
// Interp-side state
// ---------------------------------------------------------------------------

/// A registered bus connection (one entry of GNU's
/// `xd_registered_buses' alist — the conn pointer itself stays
/// Rust-side; only `equal'-compared bus keys matter to Lisp).
pub struct DbusReg {
    pub bus: Value,
    pub conn: *mut c_void,
}

/// Interp-owned D-Bus registries.
pub struct DbusState {
    /// (bus . connection) — GNU `xd_registered_buses'.
    pub buses: Vec<DbusReg>,
    /// (fd . object-path-or-filename) — GNU `xd_registered_fds'.
    pub fds: Vec<(i64, String)>,
}

impl DbusState {
    pub fn new() -> Self {
        DbusState {
            buses: Vec::new(),
            fds: Vec::new(),
        }
    }
}

// ---------------------------------------------------------------------------
// Small Lisp helpers
// ---------------------------------------------------------------------------

fn intern(i: &mut Interp, s: &str) -> SymId {
    i.intern(s)
}

fn symv(i: &mut Interp, s: &str) -> Value {
    Value::Sym(intern(i, s))
}

fn kw(i: &mut Interp, s: &str) -> SymId {
    intern(i, &format!(":{}", s))
}

fn dbus_error(i: &mut Interp, msg: impl Into<String>) -> Flow {
    let s = intern(i, "dbus-error");
    i.signal_data(s, vec![Value::string(msg.into())])
}

fn dbus_error_val(i: &mut Interp, msg: &str, v: &Value) -> Flow {
    let s = intern(i, "dbus-error");
    i.signal_data(s, vec![Value::string(msg), v.clone()])
}

fn car_safe(v: &Value) -> Value {
    match v {
        Value::Cons(c) => c.borrow().car.clone(),
        _ => Value::Nil,
    }
}

fn cdr_safe(v: &Value) -> Value {
    match v {
        Value::Cons(c) => c.borrow().cdr.clone(),
        _ => Value::Nil,
    }
}

fn is_t(i: &mut Interp, v: &Value) -> bool {
    i.sym_id(v) == Some(sym::T)
}

fn is_keyword(i: &Interp, v: &Value) -> bool {
    match v {
        Value::Sym(s) => i.symbol_name(*s).starts_with(':'),
        _ => false,
    }
}

fn str_of(v: &Value) -> Option<String> {
    match v {
        Value::Str(s) => Some(s.borrow().clone()),
        _ => None,
    }
}

fn cstr(s: &str) -> std::ffi::CString {
    std::ffi::CString::new(s).unwrap_or_else(|_| std::ffi::CString::new("").unwrap())
}

fn call_lisp(i: &mut Interp, f: &str, args: Vec<Value>) -> EvalResult {
    let fun = Value::Sym(i.intern(f));
    i.apply(&fun, args)
}

fn fboundp(i: &mut Interp, name: &str) -> bool {
    match i.intern_soft(name) {
        Some(s) => {
            let f = i.symbol_function(s);
            !(f.is_nil() || matches!(f, Value::Sym(x) if x == sym::UNBOUND))
        }
        None => false,
    }
}

/// Get an `equal'-hash-table's value for KEY (GNU `Fgethash' with
/// dflt nil).  The registered-objects table is always `equal'.
fn gethash(i: &mut Interp, key: &Value, table: &Value) -> Value {
    match table {
        Value::Hash(h) => {
            let k = hash_key_for(i, key, HashTest::Equal);
            h.borrow().map.get(&k).cloned().unwrap_or(Value::Nil)
        }
        _ => Value::Nil,
    }
}

fn puthash(i: &mut Interp, key: &Value, val: &Value, table: &Value) {
    if let Value::Hash(h) = table {
        let k = hash_key_for(i, key, HashTest::Equal);
        let mut hh = h.borrow_mut();
        hh.map.insert(k.clone(), val.clone());
        hh.put_key(k, key.clone());
    }
}

fn remhash(i: &mut Interp, key: &Value, table: &Value) {
    if let Value::Hash(h) = table {
        let k = hash_key_for(i, key, HashTest::Equal);
        let mut hh = h.borrow_mut();
        hh.map.remove(&k);
        hh.remove_key(&k);
    }
}

fn reg_table(i: &mut Interp) -> Value {
    let s = intern(i, "dbus-registered-objects-table");
    i.symbol_value(s)
}

/// Variable value is nil or void (unbound) — i.e. `install' may
/// populate it.
fn var_needs_set(i: &mut Interp, s: SymId) -> bool {
    let v = i.symbol_value(s);
    v.is_nil() || matches!(v, Value::Sym(x) if x == sym::UNBOUND)
}

// ---------------------------------------------------------------------------
// Type-symbol ↔ D-Bus type (GNU `xd_symbol_to_dbus_type' etc.)
// ---------------------------------------------------------------------------

fn symbol_to_dbus_type(i: &mut Interp, s: SymId) -> i32 {
    for (name, ty) in [
        ("byte", DBUS_TYPE_BYTE),
        ("boolean", DBUS_TYPE_BOOLEAN),
        ("int16", DBUS_TYPE_INT16),
        ("uint16", DBUS_TYPE_UINT16),
        ("int32", DBUS_TYPE_INT32),
        ("uint32", DBUS_TYPE_UINT32),
        ("int64", DBUS_TYPE_INT64),
        ("uint64", DBUS_TYPE_UINT64),
        ("double", DBUS_TYPE_DOUBLE),
        ("string", DBUS_TYPE_STRING),
        ("object-path", DBUS_TYPE_OBJECT_PATH),
        ("signature", DBUS_TYPE_SIGNATURE),
        ("unix-fd", DBUS_TYPE_UNIX_FD),
        ("array", DBUS_TYPE_ARRAY),
        ("variant", DBUS_TYPE_VARIANT),
        ("struct", DBUS_TYPE_STRUCT),
        ("dict-entry", DBUS_TYPE_DICT_ENTRY),
    ] {
        if s == kw(i, name) {
            return ty;
        }
    }
    DBUS_TYPE_INVALID
}

fn dbus_type_to_symbol(i: &mut Interp, ty: i32) -> Value {
    let name = match ty {
        DBUS_TYPE_BYTE => "byte",
        DBUS_TYPE_BOOLEAN => "boolean",
        DBUS_TYPE_INT16 => "int16",
        DBUS_TYPE_UINT16 => "uint16",
        DBUS_TYPE_INT32 => "int32",
        DBUS_TYPE_UINT32 => "uint32",
        DBUS_TYPE_INT64 => "int64",
        DBUS_TYPE_UINT64 => "uint64",
        DBUS_TYPE_DOUBLE => "double",
        DBUS_TYPE_STRING => "string",
        DBUS_TYPE_OBJECT_PATH => "object-path",
        DBUS_TYPE_SIGNATURE => "signature",
        DBUS_TYPE_UNIX_FD => "unix-fd",
        DBUS_TYPE_ARRAY => "array",
        DBUS_TYPE_VARIANT => "variant",
        DBUS_TYPE_STRUCT => "struct",
        DBUS_TYPE_DICT_ENTRY => "dict-entry",
        _ => return Value::Nil,
    };
    Value::Sym(kw(i, name))
}

/// `XD_DBUS_TYPE_P' — a keyword naming one of the type symbols.
fn is_dbus_type_sym(i: &mut Interp, v: &Value) -> bool {
    is_keyword(i, v)
        && match v {
            Value::Sym(s) => symbol_to_dbus_type(i, *s) != DBUS_TYPE_INVALID,
            _ => false,
        }
}

/// `XD_OBJECT_TO_DBUS_TYPE'.
fn object_to_dbus_type(i: &mut Interp, v: &Value) -> i32 {
    if v.is_nil() || is_t(i, v) || matches!(v, Value::Sym(s) if *s == 0) {
        return DBUS_TYPE_BOOLEAN;
    }
    match v {
        Value::Int(n) if *n >= 0 => return DBUS_TYPE_UINT32,
        Value::Int(_) => return DBUS_TYPE_INT32,
        Value::Float(_) => return DBUS_TYPE_DOUBLE,
        Value::Str(_) => return DBUS_TYPE_STRING,
        Value::Cons(_) => {
            let car = car_safe(v);
            if is_dbus_type_sym(i, &car) {
                if let Value::Sym(s) = car {
                    let ty = symbol_to_dbus_type(i, s);
                    return if is_basic_dtype(ty) { DBUS_TYPE_ARRAY } else { ty };
                }
            }
            return DBUS_TYPE_ARRAY;
        }
        Value::Sym(s) => {
            if is_keyword(i, v) {
                let ty = symbol_to_dbus_type(i, *s);
                if ty != DBUS_TYPE_INVALID {
                    return ty;
                }
            }
            return DBUS_TYPE_INVALID;
        }
        _ => DBUS_TYPE_INVALID,
    }
}

/// `XD_NEXT_VALUE' — skip a leading type symbol.
fn next_value(i: &mut Interp, v: &Value) -> Value {
    if is_dbus_type_sym(i, &car_safe(v)) {
        cdr_safe(v)
    } else {
        v.clone()
    }
}

// ---------------------------------------------------------------------------
// xd_signature — build + validate a D-Bus signature for a Lisp value
// ---------------------------------------------------------------------------

fn sig_err(i: &mut Interp, v: &Value) -> Flow {
    i.wrong_type_mut("D-Bus", v)
}

fn check_fixnat(i: &mut Interp, v: &Value) -> Result<(), Flow> {
    match v {
        Value::Int(n) if *n >= 0 => Ok(()),
        _ => Err(i.wrong_type_mut("wholenump", v)),
    }
}

fn check_fixnum(i: &mut Interp, v: &Value) -> Result<(), Flow> {
    match v {
        Value::Int(_) => Ok(()),
        _ => Err(i.wrong_type_mut("integerp", v)),
    }
}

fn check_number(i: &mut Interp, v: &Value) -> Result<(), Flow> {
    match v {
        Value::Int(_) | Value::Float(_) => Ok(()),
        _ => Err(i.wrong_type_mut("numberp", v)),
    }
}

fn check_string(i: &mut Interp, v: &Value) -> Result<(), Flow> {
    match v {
        Value::Str(_) => Ok(()),
        _ => Err(i.wrong_type_mut("stringp", v)),
    }
}

fn check_cons(i: &mut Interp, v: &Value) -> Result<(), Flow> {
    match v {
        Value::Cons(_) => Ok(()),
        _ => Err(i.wrong_type_mut("consp", v)),
    }
}

fn validate_path(i: &mut Interp, v: &Value) -> Result<(), Flow> {
    check_string(i, v)?;
    if let Some(d) = dbus() {
        let s = str_of(v).unwrap_or_default();
        let cs = cstr(&s);
        let mut err = DBusError {
            name: std::ptr::null(),
            message: std::ptr::null(),
            dummy: 0,
            padding1: std::ptr::null_mut(),
        };
        unsafe {
            (d.dbus_error_init)(&mut err);
            let ok = (d.dbus_validate_path)(cs.as_ptr(), &mut err);
            if ok == 0 {
                return Err(xd_error(i, &mut err));
            }
            (d.dbus_error_free)(&mut err);
        }
    }
    Ok(())
}

fn validate_bus_name(i: &mut Interp, v: &Value) -> Result<(), Flow> {
    if v.is_nil() {
        return Ok(());
    }
    check_string(i, v)?;
    if let Some(d) = dbus() {
        let s = str_of(v).unwrap_or_default();
        let cs = cstr(&s);
        let mut err = DBusError {
            name: std::ptr::null(),
            message: std::ptr::null(),
            dummy: 0,
            padding1: std::ptr::null_mut(),
        };
        unsafe {
            (d.dbus_error_init)(&mut err);
            let ok = (d.dbus_validate_bus_name)(cs.as_ptr(), &mut err);
            if ok == 0 {
                return Err(xd_error(i, &mut err));
            }
            (d.dbus_error_free)(&mut err);
        }
    }
    Ok(())
}

fn validate_interface_member(
    i: &mut Interp,
    v: &Value,
    iface: bool,
) -> Result<(), Flow> {
    if v.is_nil() {
        return Ok(());
    }
    check_string(i, v)?;
    if let Some(d) = dbus() {
        let s = str_of(v).unwrap_or_default();
        let cs = cstr(&s);
        let mut err = DBusError {
            name: std::ptr::null(),
            message: std::ptr::null(),
            dummy: 0,
            padding1: std::ptr::null_mut(),
        };
        unsafe {
            (d.dbus_error_init)(&mut err);
            let f = if iface {
                d.dbus_validate_interface
            } else {
                d.dbus_validate_member
            };
            let ok = f(cs.as_ptr(), &mut err);
            if ok == 0 {
                return Err(xd_error(i, &mut err));
            }
            (d.dbus_error_free)(&mut err);
        }
    }
    Ok(())
}

/// `XD_ERROR' — build a `dbus-error' signal from a DBusError and
/// free it.  The caller returns the Flow immediately.
fn xd_error(i: &mut Interp, err: *mut DBusError) -> Flow {
    let d = dbus().expect("dbus lib loaded");
    let mess = unsafe {
        let mess = if !(*err).message.is_null() {
            std::ffi::CStr::from_ptr((*err).message)
                .to_string_lossy()
                .into_owned()
        } else {
            "D-Bus error".to_string()
        };
        mess.trim_end_matches('\n').to_string()
    };
    unsafe {
        (d.dbus_error_free)(err);
    }
    dbus_error(i, mess)
}

/// `xd_signature' — validate OBJECT against DTYPE and write the
/// signature into a String.  PARENT is the containing type
/// (DBUS_TYPE_INVALID at top level) — needed for dict-entry checks.
fn xd_signature(
    i: &mut Interp,
    signature: &mut String,
    dtype: i32,
    parent_type: i32,
    object: &Value,
) -> Result<(), Flow> {
    let mut elt = object.clone();
    match dtype {
        DBUS_TYPE_BYTE | DBUS_TYPE_UINT16 => {
            check_fixnat(i, object)?;
            signature.push(dtype as u8 as char);
        }
        DBUS_TYPE_BOOLEAN => {
            if matches!(object, Value::Sym(s) if *s == kw(i, "boolean")) {
                return Err(i.wrong_type_mut("booleanp", object));
            }
            signature.push(dtype as u8 as char);
        }
        DBUS_TYPE_INT16 => {
            check_fixnum(i, object)?;
            signature.push(dtype as u8 as char);
        }
        DBUS_TYPE_UINT32
        | DBUS_TYPE_UINT64
        | DBUS_TYPE_UNIX_FD
        | DBUS_TYPE_INT32
        | DBUS_TYPE_INT64
        | DBUS_TYPE_DOUBLE => {
            check_number(i, object)?;
            signature.push(dtype as u8 as char);
        }
        DBUS_TYPE_STRING | DBUS_TYPE_OBJECT_PATH | DBUS_TYPE_SIGNATURE => {
            if dtype == DBUS_TYPE_OBJECT_PATH {
                validate_path(i, object)?;
            } else {
                check_string(i, object)?;
            }
            signature.push(dtype as u8 as char);
        }
        DBUS_TYPE_ARRAY => {
            check_cons(i, object)?;
            if matches!(&elt, Value::Cons(c) if i.sym_id(&c.borrow().car) == Some(kw(i, "array")))
            {
                elt = next_value(i, &elt);
            }
            let subtype;
            let mut subsig: String;
            if elt.is_nil() {
                subtype = DBUS_TYPE_STRING;
                subsig = "s".to_string();
            } else {
                subtype = object_to_dbus_type(i, &car_safe(&elt));
                let nv = next_value(i, &elt);
                let nv_car = car_safe(&nv);
                let mut x = String::new();
                xd_signature(i, &mut x, subtype, dtype, &nv_car)?;
                subsig = x;
            }
            if subtype == DBUS_TYPE_SIGNATURE {
                let elt1 = next_value(i, &elt);
                if matches!(&elt1, Value::Cons(c) if matches!(c.borrow().car, Value::Str(_))
                    && c.borrow().cdr.is_nil())
                {
                    subsig = str_of(&car_safe(&elt1)).unwrap_or_default();
                    elt = Value::Nil;
                }
            }
            while !elt.is_nil() {
                let mut x = String::new();
                let st = object_to_dbus_type(i, &car_safe(&elt));
                let nv = next_value(i, &elt);
                let nv_car = car_safe(&nv);
                xd_signature(i, &mut x, st, dtype, &nv_car)?;
                if subsig != x {
                    return Err(sig_err(i, &car_safe(&elt)));
                }
                elt = cdr_safe(&nv);
            }
            signature.push(dtype as u8 as char);
            signature.push_str(&subsig);
        }
        DBUS_TYPE_VARIANT => {
            check_cons(i, object)?;
            elt = next_value(i, &elt);
            check_cons(i, &elt)?;
            let subtype = object_to_dbus_type(i, &car_safe(&elt));
            let nv = next_value(i, &elt);
            let nv_car = car_safe(&nv);
            let mut x = String::new();
            xd_signature(i, &mut x, subtype, dtype, &nv_car)?;
            if !cdr_safe(&nv).is_nil() {
                let bad = car_safe(&cdr_safe(&nv));
                return Err(sig_err(i, &bad));
            }
            signature.push(dtype as u8 as char);
        }
        DBUS_TYPE_STRUCT => {
            check_cons(i, object)?;
            elt = next_value(i, &elt);
            check_cons(i, &elt)?;
            signature.push(DBUS_STRUCT_BEGIN_CHAR as char);
            while !elt.is_nil() {
                let st = object_to_dbus_type(i, &car_safe(&elt));
                let nv = next_value(i, &elt);
                let nv_car = car_safe(&nv);
                let mut x = String::new();
                xd_signature(i, &mut x, st, dtype, &nv_car)?;
                signature.push_str(&x);
                elt = cdr_safe(&nv);
            }
            signature.push(DBUS_STRUCT_END_CHAR as char);
        }
        DBUS_TYPE_DICT_ENTRY => {
            check_cons(i, object)?;
            if parent_type != DBUS_TYPE_ARRAY {
                return Err(sig_err(i, object));
            }
            signature.push(DBUS_DICT_ENTRY_BEGIN_CHAR as char);
            elt = next_value(i, &elt);
            check_cons(i, &elt)?;
            let subtype = object_to_dbus_type(i, &car_safe(&elt));
            let nv = next_value(i, &elt);
            let nv_car = car_safe(&nv);
            let mut x = String::new();
            xd_signature(i, &mut x, subtype, dtype, &nv_car)?;
            signature.push_str(&x);
            if !is_basic_dtype(subtype) {
                return Err(sig_err(i, &nv_car));
            }
            elt = cdr_safe(&nv);
            check_cons(i, &elt)?;
            let st2 = object_to_dbus_type(i, &car_safe(&elt));
            let nv2 = next_value(i, &elt);
            let nv2_car = car_safe(&nv2);
            let mut x2 = String::new();
            xd_signature(i, &mut x2, st2, dtype, &nv2_car)?;
            signature.push_str(&x2);
            if !cdr_safe(&nv2).is_nil() {
                let bad = car_safe(&cdr_safe(&nv2));
                return Err(sig_err(i, &bad));
            }
            signature.push(DBUS_DICT_ENTRY_END_CHAR as char);
        }
        _ => return Err(sig_err(i, object)),
    }
    if signature.len() > DBUS_MAXIMUM_SIGNATURE_LENGTH {
        return Err(sig_err(i, object));
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// xd_append_arg / xd_retrieve_arg
// ---------------------------------------------------------------------------

fn extract_signed(i: &mut Interp, v: &Value, lo: i64, hi: i64) -> Result<i64, Flow> {
    match v {
        Value::Int(n) if *n >= lo as i128 && *n <= hi as i128 => Ok(*n as i64),
        _ => Err(i.wrong_type_mut("integerp", v)),
    }
}

fn extract_unsigned(i: &mut Interp, v: &Value, hi: u64) -> Result<u64, Flow> {
    match v {
        Value::Int(n) if *n >= 0 && *n <= hi as i128 => Ok(*n as u64),
        _ => Err(i.wrong_type_mut("integerp", v)),
    }
}

/// `xd_append_arg' — serialize one typed Lisp arg into the iter.
fn xd_append_arg(
    i: &mut Interp,
    dtype: i32,
    object: &Value,
    iter: *mut DBusMessageIter,
) -> Result<(), Flow> {
    let d = dbus().ok_or_else(|| dbus_error(i, "No D-Bus library"))?;
    unsafe {
        if is_basic_dtype(dtype) {
            let fail = |i: &mut Interp| dbus_error_val(i, "Unable to append argument", object);
            match dtype {
                DBUS_TYPE_BYTE => {
                    check_fixnat(i, object)?;
                    let val = match object {
                        Value::Int(n) => (*n as u64 & 0xFF) as u8,
                        _ => 0,
                    };
                    if (d.dbus_message_iter_append_basic)(
                        iter,
                        dtype,
                        &val as *const u8 as *const c_void,
                    ) == 0
                    {
                        return Err(fail(i));
                    }
                }
                DBUS_TYPE_BOOLEAN => {
                    if matches!(object, Value::Sym(s) if *s == kw(i, "boolean")) {
                        return Err(i.wrong_type_mut("booleanp", object));
                    }
                    let val: u32 = if object.is_nil() { 0 } else { 1 };
                    if (d.dbus_message_iter_append_basic)(
                        iter,
                        dtype,
                        &val as *const u32 as *const c_void,
                    ) == 0
                    {
                        return Err(fail(i));
                    }
                }
                DBUS_TYPE_INT16 => {
                    let val = extract_signed(i, object, i16::MIN as i64, i16::MAX as i64)? as i16;
                    if (d.dbus_message_iter_append_basic)(
                        iter,
                        dtype,
                        &val as *const i16 as *const c_void,
                    ) == 0
                    {
                        return Err(fail(i));
                    }
                }
                DBUS_TYPE_UINT16 => {
                    let val = extract_unsigned(i, object, u16::MAX as u64)? as u16;
                    if (d.dbus_message_iter_append_basic)(
                        iter,
                        dtype,
                        &val as *const u16 as *const c_void,
                    ) == 0
                    {
                        return Err(fail(i));
                    }
                }
                DBUS_TYPE_INT32 => {
                    let val = extract_signed(i, object, i32::MIN as i64, i32::MAX as i64)? as i32;
                    if (d.dbus_message_iter_append_basic)(
                        iter,
                        dtype,
                        &val as *const i32 as *const c_void,
                    ) == 0
                    {
                        return Err(fail(i));
                    }
                }
                DBUS_TYPE_UINT32 | DBUS_TYPE_UNIX_FD => {
                    let val = extract_unsigned(i, object, u32::MAX as u64)? as u32;
                    if (d.dbus_message_iter_append_basic)(
                        iter,
                        dtype,
                        &val as *const u32 as *const c_void,
                    ) == 0
                    {
                        return Err(fail(i));
                    }
                }
                DBUS_TYPE_INT64 => {
                    let val = extract_signed(i, object, i64::MIN, i64::MAX)? as i64;
                    if (d.dbus_message_iter_append_basic)(
                        iter,
                        dtype,
                        &val as *const i64 as *const c_void,
                    ) == 0
                    {
                        return Err(fail(i));
                    }
                }
                DBUS_TYPE_UINT64 => {
                    let val = extract_unsigned(i, object, u64::MAX)? as u64;
                    if (d.dbus_message_iter_append_basic)(
                        iter,
                        dtype,
                        &val as *const u64 as *const c_void,
                    ) == 0
                    {
                        return Err(fail(i));
                    }
                }
                DBUS_TYPE_DOUBLE => {
                    let val = match object {
                        Value::Int(n) => *n as f64,
                        Value::Float(f) => **f,
                        _ => return Err(i.wrong_type_mut("numberp", object)),
                    };
                    if (d.dbus_message_iter_append_basic)(
                        iter,
                        dtype,
                        &val as *const f64 as *const c_void,
                    ) == 0
                    {
                        return Err(fail(i));
                    }
                }
                DBUS_TYPE_STRING | DBUS_TYPE_OBJECT_PATH | DBUS_TYPE_SIGNATURE => {
                    if dtype == DBUS_TYPE_OBJECT_PATH {
                        validate_path(i, object)?;
                    } else {
                        check_string(i, object)?;
                    }
                    let s = str_of(object).unwrap_or_default();
                    let cs = cstr(&s);
                    let p = cs.as_ptr();
                    if (d.dbus_message_iter_append_basic)(
                        iter,
                        dtype,
                        &p as *const *const c_char as *const c_void,
                    ) == 0
                    {
                        return Err(fail(i));
                    }
                }
                _ => {}
            }
            return Ok(());
        }
    }

    /* Compound types.  All except array carry a type symbol — skip it
       (array's is optional). */
    let mut object = object.clone();
    if !is_basic_dtype(object_to_dbus_type(i, &car_safe(&object))) {
        object = next_value(i, &object);
    }
    let d = dbus().unwrap();
    let mut subiter = DBusMessageIter::new();
    unsafe {
        match dtype {
            DBUS_TYPE_ARRAY => {
                let signature: String;
                if object.is_nil() {
                    signature = "s".to_string();
                } else {
                    let mut sig = String::new();
                    // `:signature' sole element supplies the element
                    // signature (GNU quirk).
                    let first = car_safe(&object);
                    if object_to_dbus_type(i, &first) == DBUS_TYPE_SIGNATURE {
                        let val = next_value(i, &object);
                        if matches!(&val, Value::Cons(c)
                            if matches!(c.borrow().car, Value::Str(_))
                                && c.borrow().cdr.is_nil()
                                && str_of(&c.borrow().car).unwrap_or_default().len()
                                    < DBUS_MAXIMUM_SIGNATURE_LENGTH)
                        {
                            sig = str_of(&car_safe(&val)).unwrap_or_default();
                            object = Value::Nil;
                        }
                    }
                    if !object.is_nil() {
                        let nv = next_value(i, &object);
                        let nv_car = car_safe(&nv);
                        let st = object_to_dbus_type(i, &car_safe(&object));
                        xd_signature(i, &mut sig, st, dtype, &nv_car)?;
                    }
                    signature = sig;
                }
                let cs = cstr(&signature);
                if (d.dbus_message_iter_open_container)(
                    iter,
                    dtype,
                    cs.as_ptr(),
                    &mut subiter,
                ) == 0
                {
                    return Err(dbus_error_val(
                        i,
                        "Cannot open container",
                        &Value::string(signature),
                    ));
                }
            }
            DBUS_TYPE_VARIANT => {
                let mut sig = String::new();
                let nv = next_value(i, &object);
                let nv_car = car_safe(&nv);
                let st = object_to_dbus_type(i, &car_safe(&object));
                xd_signature(i, &mut sig, st, dtype, &nv_car)?;
                let cs = cstr(&sig);
                if (d.dbus_message_iter_open_container)(
                    iter,
                    dtype,
                    cs.as_ptr(),
                    &mut subiter,
                ) == 0
                {
                    return Err(dbus_error_val(
                        i,
                        "Cannot open container",
                        &Value::string(sig),
                    ));
                }
            }
            DBUS_TYPE_STRUCT | DBUS_TYPE_DICT_ENTRY => {
                if (d.dbus_message_iter_open_container)(
                    iter,
                    dtype,
                    std::ptr::null(),
                    &mut subiter,
                ) == 0
                {
                    return Err(dbus_error(i, "Cannot open container"));
                }
            }
            _ => return Err(sig_err(i, &object)),
        }

        /* Loop over list elements. */
        while !object.is_nil() {
            let dt = object_to_dbus_type(i, &car_safe(&object));
            object = next_value(i, &object);
            xd_append_arg(i, dt, &car_safe(&object), &mut subiter)?;
            object = cdr_safe(&object);
        }

        if (d.dbus_message_iter_close_container)(iter, &mut subiter) == 0 {
            return Err(dbus_error(i, "Cannot close container"));
        }
    }
    Ok(())
}

/// `xd_retrieve_arg' — deserialize one argument from a message iter.
fn xd_retrieve_arg(i: &mut Interp, dtype: i32, iter: *mut DBusMessageIter) -> Value {
    let d = match dbus() {
        Some(d) => d,
        None => return Value::Nil,
    };
    unsafe {
        let sym = dbus_type_to_symbol(i, dtype);
        match dtype {
            DBUS_TYPE_BYTE => {
                let mut val: u8 = 0;
                (d.dbus_message_iter_get_basic)(iter, &mut val as *mut u8 as *mut c_void);
                Value::list(vec![sym, Value::Int(val as i128)])
            }
            DBUS_TYPE_BOOLEAN => {
                let mut val: u32 = 0;
                (d.dbus_message_iter_get_basic)(iter, &mut val as *mut u32 as *mut c_void);
                Value::list(vec![sym, if val == 0 { Value::Nil } else { Value::t() }])
            }
            DBUS_TYPE_INT16 => {
                let mut val: i16 = 0;
                (d.dbus_message_iter_get_basic)(iter, &mut val as *mut i16 as *mut c_void);
                Value::list(vec![sym, Value::Int(val as i128)])
            }
            DBUS_TYPE_UINT16 => {
                let mut val: u16 = 0;
                (d.dbus_message_iter_get_basic)(iter, &mut val as *mut u16 as *mut c_void);
                Value::list(vec![sym, Value::Int(val as i128)])
            }
            DBUS_TYPE_INT32 => {
                let mut val: i32 = 0;
                (d.dbus_message_iter_get_basic)(iter, &mut val as *mut i32 as *mut c_void);
                Value::list(vec![sym, Value::Int(val as i128)])
            }
            DBUS_TYPE_UINT32 | DBUS_TYPE_UNIX_FD => {
                let mut val: u32 = 0;
                (d.dbus_message_iter_get_basic)(iter, &mut val as *mut u32 as *mut c_void);
                Value::list(vec![sym, Value::Int(val as i128)])
            }
            DBUS_TYPE_INT64 => {
                let mut val: i64 = 0;
                (d.dbus_message_iter_get_basic)(iter, &mut val as *mut i64 as *mut c_void);
                Value::list(vec![sym, Value::Int(val as i128)])
            }
            DBUS_TYPE_UINT64 => {
                let mut val: u64 = 0;
                (d.dbus_message_iter_get_basic)(iter, &mut val as *mut u64 as *mut c_void);
                Value::list(vec![sym, Value::Int(val as i128)])
            }
            DBUS_TYPE_DOUBLE => {
                let mut val: f64 = 0.0;
                (d.dbus_message_iter_get_basic)(iter, &mut val as *mut f64 as *mut c_void);
                Value::list(vec![sym, Value::float(val)])
            }
            DBUS_TYPE_STRING | DBUS_TYPE_OBJECT_PATH | DBUS_TYPE_SIGNATURE => {
                let mut val: *mut c_char = std::ptr::null_mut();
                (d.dbus_message_iter_get_basic)(
                    iter,
                    &mut val as *mut *mut c_char as *mut c_void,
                );
                let s = if val.is_null() {
                    String::new()
                } else {
                    std::ffi::CStr::from_ptr(val).to_string_lossy().into_owned()
                };
                Value::list(vec![sym, Value::string(s)])
            }
            DBUS_TYPE_ARRAY | DBUS_TYPE_VARIANT | DBUS_TYPE_STRUCT | DBUS_TYPE_DICT_ENTRY => {
                let mut items: Vec<Value> = Vec::new();
                let mut subiter = DBusMessageIter::new();
                (d.dbus_message_iter_recurse)(iter, &mut subiter);
                loop {
                    let subtype = (d.dbus_message_iter_get_arg_type)(&mut subiter);
                    if subtype == DBUS_TYPE_INVALID {
                        break;
                    }
                    items.push(xd_retrieve_arg(i, subtype, &mut subiter));
                    (d.dbus_message_iter_next)(&mut subiter);
                }
                items.reverse();
                let tail = Value::list(items);
                Value::cons(sym, tail)
            }
            _ => Value::Nil,
        }
    }
}

// ---------------------------------------------------------------------------
// Bus registry (GNU `xd_registered_buses' / `xd_get_connection_address')
// ---------------------------------------------------------------------------

/// `equal' comparison of bus keys (keyword or address string).
fn bus_key_eq(i: &Interp, a: &Value, b: &Value) -> bool {
    super::equal_values(i, a, b)
}

fn find_bus_pos(i: &Interp, bus: &Value) -> Option<usize> {
    i.dbus.buses.iter().position(|r| bus_key_eq(i, &r.bus, bus))
}

/// `xd_get_connection_references' — the refcount is the first field
/// of DBusConnection (DBusAtomic = volatile i32).
fn connection_refs(conn: *mut c_void) -> i32 {
    if conn.is_null() {
        return 0;
    }
    unsafe { *(conn as *const i32) }
}

/// `XD_DBUS_VALIDATE_BUS_ADDRESS'; may rewrite `bus' to `:session'
/// when the address string equals DBUS_SESSION_BUS_ADDRESS.
fn validate_bus_address(i: &mut Interp, bus: &mut Value) -> Result<(), Flow> {
    let session_addr = std::env::var("DBUS_SESSION_BUS_ADDRESS").ok();
    if let Value::Str(s) = bus {
        let addr = s.borrow().clone();
        if let Some(d) = dbus() {
            let cs = cstr(&addr);
            unsafe {
                let mut err = DBusError {
                    name: std::ptr::null(),
                    message: std::ptr::null(),
                    dummy: 0,
                    padding1: std::ptr::null_mut(),
                };
                let mut entries: *mut c_void = std::ptr::null_mut();
                let mut len: i32 = 0;
                (d.dbus_error_init)(&mut err);
                let ok = (d.dbus_parse_address)(cs.as_ptr(), &mut entries, &mut len, &mut err);
                if ok == 0 {
                    return Err(xd_error(i, &mut err));
                }
                if !entries.is_null() {
                    (d.dbus_address_entries_free)(entries);
                }
                (d.dbus_error_free)(&mut err);
            }
        }
        if session_addr.as_deref() == Some(addr.as_str()) {
            *bus = symv(i, ":session");
        }
        return Ok(());
    }
    // Must be one of the four bus keywords.
    let id = i
        .sym_id(bus)
        .ok_or_else(|| i.wrong_type_mut("symbolp", bus))?;
    let known = [
        kw(i, "system"),
        kw(i, "session"),
        kw(i, "system-private"),
        kw(i, "session-private"),
    ];
    if !known.contains(&id) {
        return Err(dbus_error_val(i, "Wrong bus name", bus));
    }
    // No autolaunch for the session bus.
    if (id == kw(i, "session") || id == kw(i, "session-private"))
        && session_addr.is_none()
    {
        return Err(dbus_error_val(i, "No connection to bus", bus));
    }
    Ok(())
}

/// `xd_get_connection_address'.
fn get_connection(i: &mut Interp, bus: &Value) -> Result<*mut c_void, Flow> {
    let conn = match find_bus_pos(i, bus) {
        Some(p) => i.dbus.buses[p].conn,
        None => return Err(dbus_error_val(i, "No connection to bus", bus)),
    };
    let d = dbus().ok_or_else(|| dbus_error_val(i, "No connection to bus", bus))?;
    unsafe {
        if conn.is_null() || (d.dbus_connection_get_is_connected)(conn) == 0 {
            return Err(dbus_error_val(i, "No connection to bus", bus));
        }
    }
    Ok(conn)
}

/// `xd_close_bus' — close-or-unref an existing registration.
fn close_bus(i: &mut Interp, bus: &Value) {
    let Some(p) = find_bus_pos(i, bus) else {
        return;
    };
    let conn = i.dbus.buses[p].conn;
    if connection_refs(conn) <= 1 {
        if let Some(d) = dbus() {
            unsafe {
                (d.dbus_connection_close)(conn);
            }
        }
        i.dbus.buses.remove(p);
    } else if let Some(d) = dbus() {
        unsafe {
            (d.dbus_connection_unref)(conn);
        }
    }
}

// ---------------------------------------------------------------------------
// DEFUNs — connection side
// ---------------------------------------------------------------------------

fn f_init_bus(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let mut bus = arg(&a, 0);
    let private = arg(&a, 1);
    if !private.is_nil() {
        if i.sym_id(&bus) == Some(kw(i, "system")) {
            bus = symv(i, ":system-private");
        } else if i.sym_id(&bus) == Some(kw(i, "session")) {
            bus = symv(i, ":session-private");
        }
    }
    validate_bus_address(i, &mut bus)?;

    // Close existing registration, then re-check whether still connected.
    close_bus(i, &bus);
    if find_bus_pos(i, &bus).is_some() {
        let conn = get_connection(i, &bus)?;
        let d = dbus().unwrap();
        unsafe {
            (d.dbus_connection_ref)(conn);
        }
        return Ok(Value::Int(connection_refs(conn) as i128));
    }

    let d = match dbus() {
        Some(d) => d,
        None => return Err(dbus_error_val(i, "No connection to bus", &bus)),
    };

    let conn = unsafe {
        let mut err = DBusError {
            name: std::ptr::null(),
            message: std::ptr::null(),
            dummy: 0,
            padding1: std::ptr::null_mut(),
        };
        (d.dbus_error_init)(&mut err);
        let c = if let Value::Str(s) = &bus {
            let addr = s.borrow().clone();
            let cs = cstr(&addr);
            if private.is_nil() {
                (d.dbus_connection_open)(cs.as_ptr(), &mut err)
            } else {
                (d.dbus_connection_open_private)(cs.as_ptr(), &mut err)
            }
        } else {
            let bustype = if i.sym_id(&bus) == Some(kw(i, "system"))
                || i.sym_id(&bus) == Some(kw(i, "system-private"))
            {
                DBUS_BUS_SYSTEM
            } else {
                DBUS_BUS_SESSION
            };
            if private.is_nil() {
                (d.dbus_bus_get)(bustype, &mut err)
            } else {
                (d.dbus_bus_get_private)(bustype, &mut err)
            }
        };
        if (d.dbus_error_is_set)(&err) != 0 {
            return Err(xd_error(i, &mut err));
        }
        if c.is_null() {
            (d.dbus_error_free)(&mut err);
            return Err(dbus_error_val(i, "No connection to bus", &bus));
        }
        if matches!(bus, Value::Str(_)) {
            (d.dbus_bus_register)(c, &mut err);
        } else {
            (d.dbus_connection_set_exit_on_disconnect)(c, 0);
        }
        if (d.dbus_error_is_set)(&err) != 0 {
            return Err(xd_error(i, &mut err));
        }
        (d.dbus_error_free)(&mut err);
        c
    };

    i.dbus.buses.push(DbusReg {
        bus: bus.clone(),
        conn,
    });
    Ok(Value::Int(connection_refs(conn) as i128))
}

fn f_get_unique_name(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let mut bus = a[0].clone();
    validate_bus_address(i, &mut bus)?;
    let conn = get_connection(i, &bus)?;
    let d = dbus().unwrap();
    unsafe {
        let name = (d.dbus_bus_get_unique_name)(conn);
        if name.is_null() {
            return Err(dbus_error(i, "No unique name available"));
        }
        Ok(Value::string(
            std::ffi::CStr::from_ptr(name).to_string_lossy().into_owned(),
        ))
    }
}

// ---------------------------------------------------------------------------
// DEFUN — dbus-message-internal
// ---------------------------------------------------------------------------

fn f_message_internal(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let nargs = args.len();
    let message_type = arg(&args, 0);
    let bus = arg(&args, 1);
    let service = arg(&args, 2);
    let mut handler = Value::Nil;
    let mut path = Value::Nil;
    let mut interface = Value::Nil;
    let mut member = Value::Nil;
    let mut error_name = Value::Nil;
    let mut serial: u32 = 0;
    let count: usize;

    let mtype = match &message_type {
        Value::Int(n) if *n >= DBUS_MESSAGE_TYPE_INVALID as i128
            && *n < 5_i128 =>
        {
            *n as i32
        }
        _ => {
            check_fixnat(i, &message_type)?;
            return Err(dbus_error_val(i, "Invalid message type", &message_type));
        }
    };

    if mtype == DBUS_MESSAGE_TYPE_METHOD_CALL || mtype == DBUS_MESSAGE_TYPE_SIGNAL {
        path = arg(&args, 3);
        interface = arg(&args, 4);
        member = arg(&args, 5);
        if mtype == DBUS_MESSAGE_TYPE_METHOD_CALL {
            handler = arg(&args, 6);
        }
        count = if mtype == DBUS_MESSAGE_TYPE_METHOD_CALL {
            7
        } else {
            6
        };
    } else if mtype == DBUS_MESSAGE_TYPE_METHOD_RETURN || mtype == DBUS_MESSAGE_TYPE_ERROR {
        serial = extract_unsigned(i, &arg(&args, 3), u32::MAX as u64)? as u32;
        if mtype == DBUS_MESSAGE_TYPE_ERROR {
            error_name = arg(&args, 4);
        }
        count = if mtype == DBUS_MESSAGE_TYPE_ERROR { 5 } else { 4 };
    } else {
        count = 3;
    }

    let mut bus = bus;
    validate_bus_address(i, &mut bus)?;
    validate_bus_name(i, &service)?;

    if nargs < count {
        let f = symv(i, "dbus-message-internal");
        return Err(i.wrong_number_of_args(&f, nargs as i128));
    }

    if mtype == DBUS_MESSAGE_TYPE_METHOD_CALL || mtype == DBUS_MESSAGE_TYPE_SIGNAL {
        validate_path(i, &path)?;
        validate_interface_member(i, &interface, true)?;
        validate_interface_member(i, &member, false)?;
        if !handler.is_nil() {
            let fp = call_lisp(i, "functionp", vec![handler.clone()])?;
            if fp.is_nil() {
                return Err(i.wrong_type_mut("invalid-function", &handler));
            }
        }
    }

    let conn = get_connection(i, &bus)?;
    let d = dbus().unwrap();

    let dmsg = unsafe {
        (d.dbus_message_new)(if mtype == DBUS_MESSAGE_TYPE_INVALID {
            DBUS_MESSAGE_TYPE_SIGNAL
        } else {
            mtype
        })
    };
    if dmsg.is_null() {
        return Err(dbus_error(i, "Unable to create a new message"));
    }

    let r = f_message_internal_2(
        i,
        d,
        conn,
        dmsg,
        mtype,
        &bus,
        &service,
        &path,
        &interface,
        &member,
        &error_name,
        serial,
        &handler,
        &args,
        count,
    );
    unsafe {
        (d.dbus_message_unref)(dmsg);
    }
    r
}

fn f_message_internal_2(
    i: &mut Interp,
    d: &'static Dbus,
    conn: *mut c_void,
    dmsg: *mut c_void,
    mtype: i32,
    bus: &Value,
    service: &Value,
    path: &Value,
    interface: &Value,
    member: &Value,
    error_name: &Value,
    serial: u32,
    handler: &Value,
    args: &[Value],
    count0: usize,
) -> EvalResult {
    unsafe {
        if matches!(service, Value::Str(_)) && mtype != DBUS_MESSAGE_TYPE_INVALID {
            let svc = str_of(service).unwrap_or_default();
            let cs = cstr(&svc);
            if mtype != DBUS_MESSAGE_TYPE_SIGNAL {
                if (d.dbus_message_set_destination)(dmsg, cs.as_ptr()) == 0 {
                    return Err(dbus_error_val(i, "Unable to set the destination", service));
                }
            } else {
                // Unicast signal — see whether the name has an owner
                // that isn't ourselves.
                let mut uname = Value::Nil;
                if (d.dbus_bus_name_has_owner)(conn, cs.as_ptr(), std::ptr::null_mut()) != 0
                    && fboundp(i, "dbus-get-name-owner")
                {
                    uname = call_lisp(i, "dbus-get-name-owner", vec![bus.clone(), service.clone()])
                        .unwrap_or(Value::Nil);
                }
                if let Value::Str(u) = &uname {
                    let us = u.borrow().clone();
                    let mine = (d.dbus_bus_get_unique_name)(conn);
                    let mine_s = if mine.is_null() {
                        String::new()
                    } else {
                        std::ffi::CStr::from_ptr(mine).to_string_lossy().into_owned()
                    };
                    if us != mine_s && (d.dbus_message_set_destination)(dmsg, cs.as_ptr()) == 0 {
                        return Err(dbus_error_val(
                            i,
                            "Unable to set signal destination",
                            service,
                        ));
                    }
                }
            }
        }

        if mtype == DBUS_MESSAGE_TYPE_METHOD_CALL || mtype == DBUS_MESSAGE_TYPE_SIGNAL {
            let cp = cstr(&str_of(path).unwrap_or_default());
            let ci = cstr(&str_of(interface).unwrap_or_default());
            let cm = cstr(&str_of(member).unwrap_or_default());
            if (d.dbus_message_set_path)(dmsg, cp.as_ptr()) == 0
                || (d.dbus_message_set_interface)(dmsg, ci.as_ptr()) == 0
                || (d.dbus_message_set_member)(dmsg, cm.as_ptr()) == 0
            {
                return Err(dbus_error(i, "Unable to set the message parameter"));
            }
        } else if mtype == DBUS_MESSAGE_TYPE_METHOD_RETURN || mtype == DBUS_MESSAGE_TYPE_ERROR {
            if (d.dbus_message_set_reply_serial)(dmsg, serial) == 0 {
                return Err(dbus_error(i, "Unable to create a return message"));
            }
            if mtype == DBUS_MESSAGE_TYPE_ERROR {
                let ce = cstr(&str_of(error_name).unwrap_or_default());
                if (d.dbus_message_set_error_name)(dmsg, ce.as_ptr()) == 0 {
                    return Err(dbus_error(i, "Unable to create an error message"));
                }
            }
        }
    }

    // Keyword parameters: :timeout N, :authorizable B, :keep-fd.
    let mut count = count0;
    let nargs = args.len();
    let mut timeout: i64 = -1;
    let mut keepfd = false;
    while count + 2 <= nargs {
        let k = &args[count];
        if i.sym_id(k) == Some(kw(i, "timeout")) {
            if mtype != DBUS_MESSAGE_TYPE_METHOD_CALL {
                return Err(dbus_error(i, ":timeout is only supported on method calls"));
            }
            check_fixnat(i, &args[count + 1])?;
            if let Value::Int(n) = &args[count + 1] {
                timeout = (*n).min(i32::MAX as i128) as i64;
            }
            count += 2;
        } else if i.sym_id(k) == Some(kw(i, "authorizable")) {
            if mtype != DBUS_MESSAGE_TYPE_METHOD_CALL {
                return Err(dbus_error(
                    i,
                    ":authorizable is only supported on method calls",
                ));
            }
            unsafe {
                (d.dbus_message_set_allow_interactive_authorization)(
                    dmsg,
                    if args[count + 1].is_nil() { 0 } else { 1 },
                );
            }
            count += 2;
        } else if i.sym_id(k) == Some(kw(i, "keep-fd")) {
            if mtype != DBUS_MESSAGE_TYPE_METHOD_CALL {
                return Err(dbus_error(i, ":keep-fd is only supported on method calls"));
            }
            keepfd = true;
            count += 1;
        } else {
            break;
        }
    }

    // Serialize the remaining args.
    unsafe {
        let mut iter = DBusMessageIter::new();
        (d.dbus_message_iter_init_append)(dmsg, &mut iter);
        while count < nargs {
            let dtype = object_to_dbus_type(i, &args[count]);
            if count + 1 < nargs && is_dbus_type_sym(i, &args[count]) {
                count += 1;
            }
            let mut sig = String::new();
            xd_signature(i, &mut sig, dtype, DBUS_TYPE_INVALID, &args[count])?;
            xd_append_arg(i, dtype, &args[count], &mut iter)?;
            count += 1;
        }
    }

    if mtype == DBUS_MESSAGE_TYPE_INVALID {
        return Ok(Value::t());
    }

    if !handler.is_nil() {
        let ok = unsafe {
            (d.dbus_connection_send_with_reply)(
                conn,
                dmsg,
                std::ptr::null_mut(),
                timeout as i32,
            )
        };
        if ok == 0 {
            return Err(dbus_error(i, "Cannot send message"));
        }
        let serial = unsafe { (d.dbus_message_get_serial)(dmsg) };
        let key = Value::list(vec![
            Value::Sym(kw(i, "serial")),
            bus.clone(),
            Value::Int(serial as i128),
        ]);
        let value = if keepfd {
            Value::cons(handler.clone(), path.clone())
        } else {
            handler.clone()
        };
        let tbl = reg_table(i);
        puthash(i, &key, &value, &tbl);
        unsafe {
            (d.dbus_connection_flush)(conn);
        }
        return Ok(key);
    }

    let ok = unsafe { (d.dbus_connection_send)(conn, dmsg, std::ptr::null_mut()) };
    if ok == 0 {
        return Err(dbus_error(i, "Cannot send message"));
    }
    unsafe {
        (d.dbus_connection_flush)(conn);
    }
    Ok(Value::Nil)
}

// ---------------------------------------------------------------------------
// DEFUNs — fd helpers (GNU `xd_registered_fds')
// ---------------------------------------------------------------------------

fn f_fd_open(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let filename = want_string(i, &a[0])?;
    let filename = crate::editor::expand_file_name_str(i, &filename);
    if let Some((fd, _)) = i.dbus.fds.iter().find(|(_, f)| f == &filename) {
        return Ok(Value::Int(*fd as i128));
    }
    let cs = cstr(&filename);
    let fd = unsafe { libc::open(cs.as_ptr(), libc::O_RDONLY) };
    if fd <= 0 {
        return Err(dbus_error_val(i, "Cannot open file", &Value::string(filename)));
    }
    i.dbus.fds.push((fd as i64, filename));
    Ok(Value::Int(fd as i128))
}

fn f_fd_close(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let fd = want_int(i, &a[0])?;
    let Some(p) = i.dbus.fds.iter().position(|(f, _)| *f == fd as i64) else {
        return Ok(Value::Nil);
    };
    i.dbus.fds.remove(p);
    let ok = unsafe { libc::close(fd as i32) };
    Ok(if ok == 0 { Value::t() } else { Value::Nil })
}

fn f_registered_fds(i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    let mut out = Vec::new();
    for (fd, f) in &i.dbus.fds {
        out.push(Value::cons(Value::Int(*fd as i128), Value::string(f.clone())));
    }
    Ok(Value::list(out))
}

// ---------------------------------------------------------------------------
// Incoming message → `dbus-event' (GNU `xd_read_message_1' + `xd_store_event')
// ---------------------------------------------------------------------------

fn msg_str(f: unsafe extern "C" fn(*mut c_void) -> *const c_char, m: *mut c_void) -> Option<String> {
    unsafe {
        let p = f(m);
        if p.is_null() {
            None
        } else {
            Some(std::ffi::CStr::from_ptr(p).to_string_lossy().into_owned())
        }
    }
}

/// Store one event: append event_args + (handler . args), prepend
/// `dbus-event', set `last-input-event', dispatch via
/// `special-event-map' — the remacs equivalent of
/// `kbd_buffer_store_event' + `read_char' dispatch.
fn store_event(
    i: &mut Interp,
    handler: &Value,
    args: &Value,
    event_args: Vec<Value>,
) -> Result<(), Flow> {
    let mut ev = event_args;
    ev.push(handler.clone());
    if let Ok(rest) = args.list_to_vec() {
        ev.extend(rest);
    }
    let event = Value::cons(symv(i, "dbus-event"), Value::list(ev));
    // GNU's read_char sets last-input-event before command-execute.
    let lie = intern(i, "last-input-event");
    let _ = i.set_symbol(lie, event.clone());
    super::filenotify::dispatch_special_event(i, &event).map(|_| ())
}

/// `xd_read_message_1' — pop one queued message, translate, dispatch.
fn read_message_1(i: &mut Interp, conn: *mut c_void, bus: &Value) -> Result<(), Flow> {
    let d = dbus().unwrap();
    let dmsg = unsafe { (d.dbus_connection_pop_message)(conn) };
    if dmsg.is_null() {
        return Ok(());
    }
    let r = read_message_1_inner(i, d, conn, dmsg, bus);
    unsafe {
        (d.dbus_message_unref)(dmsg);
    }
    r
}

fn read_message_1_inner(
    i: &mut Interp,
    d: &'static Dbus,
    _conn: *mut c_void,
    dmsg: *mut c_void,
    bus: &Value,
) -> Result<(), Flow> {
    // Arguments (typed).
    let mut arglist: Vec<Value> = Vec::new();
    unsafe {
        let mut iter = DBusMessageIter::new();
        if (d.dbus_message_iter_init)(dmsg, &mut iter) != 0 {
            loop {
                let dtype = (d.dbus_message_iter_get_arg_type)(&mut iter);
                if dtype == DBUS_TYPE_INVALID {
                    break;
                }
                arglist.push(xd_retrieve_arg(i, dtype, &mut iter));
                (d.dbus_message_iter_next)(&mut iter);
            }
        }
    }
    let args = Value::list(arglist);

    let mtype = unsafe { (d.dbus_message_get_type)(dmsg) };
    let serial = unsafe {
        if mtype == DBUS_MESSAGE_TYPE_METHOD_RETURN || mtype == DBUS_MESSAGE_TYPE_ERROR {
            (d.dbus_message_get_reply_serial)(dmsg)
        } else {
            (d.dbus_message_get_serial)(dmsg)
        }
    };
    let uname = msg_str(d.dbus_message_get_sender, dmsg);
    let destination = msg_str(d.dbus_message_get_destination, dmsg);
    let path = msg_str(d.dbus_message_get_path, dmsg);
    let interface = msg_str(d.dbus_message_get_interface, dmsg);
    let member = msg_str(d.dbus_message_get_member, dmsg);
    let error_name = msg_str(d.dbus_message_get_error_name, dmsg);

    // event_args = (bus mtype serial uname destination path interface
    // member-or-error-name)
    let member_ev = if mtype == DBUS_MESSAGE_TYPE_ERROR {
        error_name.clone()
    } else {
        member.clone()
    };
    let event_args: Vec<Value> = vec![
        bus.clone(),
        Value::Int(mtype as i128),
        Value::Int(serial as i128),
        uname.clone().map(Value::string).unwrap_or(Value::Nil),
        destination.map(Value::string).unwrap_or(Value::Nil),
        path.clone().map(Value::string).unwrap_or(Value::Nil),
        interface.clone().map(Value::string).unwrap_or(Value::Nil),
        member_ev.map(Value::string).unwrap_or(Value::Nil),
    ];

    let tbl = reg_table(i);

    if mtype == DBUS_MESSAGE_TYPE_INVALID {
        return Ok(());
    }

    if mtype == DBUS_MESSAGE_TYPE_METHOD_RETURN || mtype == DBUS_MESSAGE_TYPE_ERROR {
        let key = Value::list(vec![
            Value::Sym(kw(i, "serial")),
            bus.clone(),
            Value::Int(serial as i128),
        ]);
        let value = gethash(i, &key, &tbl);
        if value.is_nil() {
            return monitor(i, &tbl, bus, &args, event_args);
        }
        remhash(i, &key, &tbl);
        let handler = if matches!(value, Value::Cons(_)) {
            car_safe(&value)
        } else {
            value.clone()
        };
        store_event(i, &handler, &args, event_args)?;

        // :keep-fd — register the received fd against the stored path.
        if matches!(value, Value::Cons(_)) {
            let first = car_safe(&args);
            if matches!(&first, Value::Cons(c) if i.sym_id(&c.borrow().car) == Some(kw(i, "unix-fd")))
            {
                let fdv = car_safe(&cdr_safe(&first));
                if let Value::Int(fd) = fdv {
                    let pth = cdr_safe(&value);
                    if let Value::Str(s) = &pth {
                        i.dbus.fds.push((fd as i64, s.borrow().clone()));
                    }
                }
            }
        }
        return Ok(());
    }

    // METHOD_CALL / SIGNAL.
    let (Some(iface), Some(memb)) = (interface.clone(), member.clone()) else {
        return monitor(i, &tbl, bus, &args, event_args);
    };

    let key_type = if mtype == DBUS_MESSAGE_TYPE_METHOD_CALL {
        "method"
    } else {
        "signal"
    };
    let key = Value::list(vec![
        Value::Sym(kw(i, key_type)),
        bus.clone(),
        Value::string(iface.clone()),
        Value::string(memb.clone()),
    ]);
    let mut value = gethash(i, &key, &tbl);

    if mtype == DBUS_MESSAGE_TYPE_SIGNAL {
        for (iv, mv) in [
            (Value::Nil, Value::string(memb.clone())),
            (Value::string(iface.clone()), Value::Nil),
            (Value::Nil, Value::Nil),
        ] {
            let k = Value::list(vec![
                Value::Sym(kw(i, "signal")),
                bus.clone(),
                iv,
                mv,
            ]);
            let v = gethash(i, &k, &tbl);
            if !v.is_nil() {
                let mut merged: Vec<Value> = Vec::new();
                if let Ok(l) = value.list_to_vec() {
                    merged.extend(l);
                }
                if let Ok(l) = v.list_to_vec() {
                    merged.extend(l);
                }
                value = Value::list(merged);
            }
        }
    }

    let mut called: Vec<Value> = Vec::new();
    if let Ok(entries) = value.list_to_vec() {
        for key in entries {
            // key = (UNAME SERVICE PATH HANDLER [RULE])
            let key_uname = car_safe(&key);
            if let (Some(u), Value::Str(ku)) = (&uname, &key_uname) {
                if *u != *ku.borrow() {
                    continue;
                }
            }
            let key_path = car_safe(&cdr_safe(&cdr_safe(&key)));
            if let (Some(p), Value::Str(kp)) = (&path, &key_path) {
                if *p != *kp.borrow() {
                    continue;
                }
            }
            let handler = car_safe(&cdr_safe(&cdr_safe(&cdr_safe(&key))));
            if handler.is_nil() {
                continue;
            }
            if called
                .iter()
                .any(|h| super::eq_values(h, &handler))
            {
                continue;
            }
            called.push(handler.clone());
            store_event(i, &handler, &args, event_args.clone())?;
        }
    }

    monitor(i, &tbl, bus, &args, event_args)
}

/// `monitor:' label — deliver to a registered monitor handler.
fn monitor(
    i: &mut Interp,
    tbl: &Value,
    bus: &Value,
    args: &Value,
    event_args: Vec<Value>,
) -> Result<(), Flow> {
    let key = Value::list(vec![Value::Sym(kw(i, "monitor")), bus.clone()]);
    let value = gethash(i, &key, tbl);
    if value.is_nil() {
        return Ok(());
    }
    let first = car_safe(&value);
    let handler = car_safe(&cdr_safe(&cdr_safe(&cdr_safe(&cdr_safe(&first)))));
    store_event(i, &handler, args, event_args)
}

/// `xd_read_message' for one bus — read_write(0) + pop until the
/// dispatch queue is empty.  GNU catches `dbus-error' around this
/// (internal_catch); do the same.
fn read_bus(i: &mut Interp, bus: &Value) -> Result<(), Flow> {
    let conn = get_connection(i, bus)?;
    let d = dbus().unwrap();
    unsafe {
        (d.dbus_connection_read_write)(conn, 0);
        loop {
            if (d.dbus_connection_get_dispatch_status)(conn) == DBUS_DISPATCH_COMPLETE {
                break;
            }
            read_message_1(i, conn, bus)?;
        }
    }
    Ok(())
}

/// `xd_read_queued_messages' — drain every registered bus, dispatching
/// `dbus-event's synchronously.  Called from the
/// `sleep_firing_timers' wait pump and `read-event'.
pub fn drain(i: &mut Interp) -> EvalResult {
    if i.dbus.buses.is_empty() || dbus().is_none() {
        return Ok(Value::Nil);
    }
    let buses: Vec<Value> = i.dbus.buses.iter().map(|r| r.bus.clone()).collect();
    let dbe = intern(i, "dbus-error");
    for bus in buses {
        if let Err(f) = read_bus(i, &bus) {
            // GNU: internal_catch (Qdbus_error, ...) — only D-Bus
            // errors are swallowed; everything else propagates.
            match f {
                Flow::Signal(ref sig, _, _)
                    if matches!(sig, Value::Sym(s) if *s == dbe) => {}
                _ => return Err(f),
            }
        }
    }
    Ok(Value::Nil)
}

// ---------------------------------------------------------------------------
// Registration
// ---------------------------------------------------------------------------

pub(crate) static SUBRS: &[crate::lisp::value::Subr] = &[
    S!("dbus--init-bus", 1, 2, f_init_bus, "Establish the connection to D-Bus BUS."),
    S!("dbus-get-unique-name", 1, 1, f_get_unique_name, "Return the unique name of Emacs registered at D-Bus BUS."),
    S!("dbus-message-internal", many 3, f_message_internal, "Send a D-Bus message.\nThis is an internal function, it shall not be used outside dbus.el."),
    S!("dbus--fd-open", 1, 1, f_fd_open, "Open FILENAME and return the respective read-only file descriptor."),
    S!("dbus--fd-close", 1, 1, f_fd_close, "Close file descriptor FD."),
    S!("dbus--registered-fds", 0, 0, f_registered_fds, "Return registered file descriptors, an alist."),
];

/// `(provide 'dbusbind)` + `dbus-error' conditions +
/// `dbus-registered-objects-table' + version/message-type vars —
/// the remacs equivalent of `syms_of_dbusbind'.  Called from
/// `builtins::install' and `register_extlib_features'.
pub(crate) fn install(i: &mut Interp) {
    // `dbus-error' condition properties (GNU Fput in syms_of_dbusbind).
    let ec = intern(i, "error-conditions");
    let de = intern(i, "dbus-error");
    let es = intern(i, "error");
    i.put_prop(de, ec, Value::list(vec![Value::Sym(de), Value::Sym(es)]));
    let em = intern(i, "error-message");
    i.put_prop(de, em, Value::string("D-Bus error"));

    // Registered-objects hash table (`equal' test), created in C by
    // syms_of_dbusbind; dbus.el only `defvar's it.
    let rot = intern(i, "dbus-registered-objects-table");
    if var_needs_set(i, rot) {
        let mk = Value::Sym(intern(i, "make-hash-table"));
        let test_kw = symv(i, ":test");
        let eq_s = symv(i, "equal");
        if let Ok(tbl) = i.apply(&mk, vec![test_kw, eq_s]) {
            let _ = i.set_symbol(rot, tbl);
        }
    }
    let rvt = intern(i, "dbus-return-values-table");
    if var_needs_set(i, rvt) {
        let mk = Value::Sym(intern(i, "make-hash-table"));
        let test_kw = symv(i, ":test");
        let eq_s = symv(i, "equal");
        if let Ok(tbl) = i.apply(&mk, vec![test_kw, eq_s]) {
            let _ = i.set_symbol(rvt, tbl);
        }
    }

    // Message-type variables GNU sets from the constants.
    for (name, val) in [
        ("dbus-message-type-invalid", DBUS_MESSAGE_TYPE_INVALID),
        ("dbus-message-type-method-call", DBUS_MESSAGE_TYPE_METHOD_CALL),
        ("dbus-message-type-method-return", DBUS_MESSAGE_TYPE_METHOD_RETURN),
        ("dbus-message-type-error", DBUS_MESSAGE_TYPE_ERROR),
        ("dbus-message-type-signal", DBUS_MESSAGE_TYPE_SIGNAL),
    ] {
        let s = intern(i, name);
        if var_needs_set(i, s) {
            let _ = i.set_symbol(s, Value::Int(val as i128));
        }
    }

    // Runtime + compiled version strings (we dlopen, so "compiled
    // against" == the loaded library's version).
    for name in ["dbus-runtime-version", "dbus-compiled-version"] {
        let v = intern(i, name);
        if var_needs_set(i, v) {
            if let Some(d) = dbus() {
                unsafe {
                    let (mut a, mut b, mut c) = (0, 0, 0);
                    (d.dbus_get_version)(&mut a, &mut b, &mut c);
                    let _ =
                        i.set_symbol(v, Value::string(format!("{}.{}.{}", a, b, c)));
                }
            }
        }
    }

    // Feature — only when the library actually loaded (GNU provides
    // `dbusbind' iff compiled with D-Bus).
    if dbus().is_some() {
        let f = intern(i, "dbusbind");
        if !i.features.contains(&f) {
            i.features.push(f);
        }
    }
}
