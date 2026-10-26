//! Native compilation (comp.c / comp.el workalike).
//!
//! Remacs compiles Emacs Lisp to C source and drives the system C
//! compiler (`cc`) to produce real machine code in `.eln' shared
//! objects — our own analogue of GNU's libgccjit pipeline.  The
//! generated code calls back into the interpreter through a small
//! opaque-handle API; generic Lisp operations remain runtime calls,
//! just like the generic paths in GNU's generated code.
//!
//! Pipeline: `native-compile' → `emit_unit' (Lambda → C) → `run_cc'
//! (.eln) → `native-elisp-load' → dlopen → `remacs_eln_init' →
//! `defsubr' per function (installed via the trampoline pool).
//!
//! Forms that need real unwind semantics (catch/throw,
//! condition-case, unwind-protect, save-*-excursion) or closures over
//! captured environments compile as a trampoline re-evaluating a
//! `(make-interpreted-closure ...)' reconstructed from the original
//! definition — a real native subr wrapper with intact semantics.

#![allow(unsafe_op_in_unsafe_fn)]

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::ffi::{c_char, c_void};
use std::io::Write;
use std::rc::Rc;
use std::sync::Mutex;

use super::S;
use crate::lisp::value::{Arity, OptParam, Subr, SubrFn, Value};
use crate::lisp::{EvalResult, Flow, Interp, SymId, sym};

// ---------------------------------------------------------------------------
// em_obj handle space shared with generated code
// ---------------------------------------------------------------------------

/// Opaque object reference across the FFI boundary: index+1 into
/// `OBJTAB` (0 is the error sentinel `EM_INVALID`).
type EmObj = u64;
type EmEnv = usize;
type EmEntry = unsafe extern "C-unwind" fn(*const EmApi, EmEnv, *const EmObj, i64) -> EmObj;

const EM_INVALID: EmObj = 0;

thread_local! {
    static OBJTAB: RefCell<Vec<Value>> = const { RefCell::new(Vec::new()) };
    static PENDING: RefCell<Option<Flow>> = const { RefCell::new(None) };
    static NATIVES: RefCell<Vec<EmEntry>> = const { RefCell::new(Vec::new()) };
    static ACTIVE_NATIVE: Cell<Option<usize>> = const { Cell::new(None) };
    static META: RefCell<Vec<NativeMeta>> = RefCell::new(Vec::new());
    static UNITS: RefCell<Vec<Unit>> = RefCell::new(Vec::new());
    static CTXT: RefCell<Option<String>> = RefCell::new(None);
}

/// Call-local handles belong to a Lisp stack: truncate(mark) in one native
/// call must never discard another, suspended call's handles.
#[derive(Default)]
pub(crate) struct ThreadState {
    objects: Vec<Value>,
    pending: Option<Flow>,
    active: Option<usize>,
    context: Option<String>,
}

pub(crate) fn take_thread_state() -> ThreadState {
    ThreadState {
        objects: OBJTAB.with(|v| std::mem::take(&mut *v.borrow_mut())),
        pending: PENDING.with(|v| v.borrow_mut().take()),
        active: ACTIVE_NATIVE.replace(None),
        context: CTXT.with(|v| v.borrow_mut().take()),
    }
}

pub(crate) fn restore_thread_state(state: ThreadState) {
    OBJTAB.with(|v| *v.borrow_mut() = state.objects);
    PENDING.with(|v| *v.borrow_mut() = state.pending);
    ACTIVE_NATIVE.set(state.active);
    CTXT.with(|v| *v.borrow_mut() = state.context);
}

fn reg(v: Value) -> EmObj {
    OBJTAB.with(|t| {
        let mut t = t.borrow_mut();
        t.push(v);
        t.len() as EmObj
    })
}

fn obj(h: EmObj) -> Value {
    if h == EM_INVALID {
        // A failed inner api call leaked its sentinel into an outer
        // call argument — treat as nil rather than panic so the
        // original PENDING error can surface.
        return Value::Nil;
    }
    OBJTAB.with(|t| t.borrow()[(h - 1) as usize].clone())
}

// ---------------------------------------------------------------------------
// Host API table — every callback runs inside a live Interp
// ---------------------------------------------------------------------------

unsafe fn host(e: EmEnv) -> &'static mut Interp {
    &mut *(e as *mut Interp)
}

unsafe fn outcome(e: EmEnv, r: Result<Value, Flow>) -> EmObj {
    let r = host(e).offer_flow(r);
    match r {
        Ok(v) => reg(v),
        Err(flow) => {
            // Keep non-local exits in Rust until the C frame has returned.
            PENDING.with(|p| *p.borrow_mut() = Some(flow));
            EM_INVALID
        }
    }
}

#[repr(C)]
struct EmApi {
    mk_nil: unsafe extern "C-unwind" fn(EmEnv) -> EmObj,
    mk_t: unsafe extern "C-unwind" fn(EmEnv) -> EmObj,
    mk_int: unsafe extern "C-unwind" fn(EmEnv, i64) -> EmObj,
    mk_float: unsafe extern "C-unwind" fn(EmEnv, f64) -> EmObj,
    mk_str: unsafe extern "C-unwind" fn(EmEnv, *const c_char, usize) -> EmObj,
    intern: unsafe extern "C-unwind" fn(EmEnv, *const c_char, usize) -> EmObj,
    /// Parse a printed literal (`read-from-string').
    read: unsafe extern "C-unwind" fn(EmEnv, *const c_char, usize) -> EmObj,
    /// Truth test (non-nil → 1).
    truthy: unsafe extern "C-unwind" fn(EmEnv, EmObj) -> i64,
    /// `funcall' a function object or symbol.
    call: unsafe extern "C-unwind" fn(EmEnv, EmObj, *const EmObj, i64) -> EmObj,
    /// `eval' a form.
    eval: unsafe extern "C-unwind" fn(EmEnv, EmObj) -> EmObj,
    /// Call a lambda/subr object with argv (fallback path).
    call_lambda: unsafe extern "C-unwind" fn(EmEnv, EmObj, *const EmObj, i64) -> EmObj,
    /// `set' a (dynamic/global) variable symbol.
    sym_set: unsafe extern "C-unwind" fn(EmEnv, EmObj, EmObj) -> EmObj,
    /// Record a non-local exit; always returns EM_INVALID.
    signal: unsafe extern "C-unwind" fn(EmEnv, EmObj, EmObj) -> EmObj,
    car: unsafe extern "C-unwind" fn(EmEnv, EmObj) -> EmObj,
    cdr: unsafe extern "C-unwind" fn(EmEnv, EmObj) -> EmObj,
    cons: unsafe extern "C-unwind" fn(EmEnv, EmObj, EmObj) -> EmObj,
    listn: unsafe extern "C-unwind" fn(EmEnv, *const EmObj, i64) -> EmObj,
    /// Registration entry used by generated init code.
    defsubr: unsafe extern "C-unwind" fn(*const EmApi, EmEnv, i64, *const c_void) -> i64,
    /// Read a function constant once, retaining its Value rather than a
    /// temporary OBJTAB handle. Appended to preserve the old unit ABI.
    read_constant: unsafe extern "C-unwind" fn(EmEnv, *const c_char, usize) -> EmObj,
    string_constant: unsafe extern "C-unwind" fn(EmEnv, *const c_char, usize) -> EmObj,
    float_constant: unsafe extern "C-unwind" fn(EmEnv, f64) -> EmObj,
    /// Preserve the source string's byte-kind flags; appended for old units.
    string_constant_kind: unsafe extern "C-unwind" fn(EmEnv, *const c_char, usize, i64) -> EmObj,
    binding_mark: unsafe extern "C-unwind" fn(EmEnv) -> usize,
    bind: unsafe extern "C-unwind" fn(EmEnv, EmObj, EmObj) -> EmObj,
    unbind: unsafe extern "C-unwind" fn(EmEnv, usize) -> EmObj,
    sym_value: unsafe extern "C-unwind" fn(EmEnv, EmObj) -> EmObj,
    /// Appended for symbol-name representation fidelity without shifting old ABI fields.
    intern_kind: unsafe extern "C-unwind" fn(EmEnv, *const c_char, usize, i64) -> EmObj,
}

unsafe extern "C-unwind" fn api_nil(_e: EmEnv) -> EmObj {
    reg(Value::Nil)
}
unsafe extern "C-unwind" fn api_t(_e: EmEnv) -> EmObj {
    reg(Value::t())
}
unsafe extern "C-unwind" fn api_int(_e: EmEnv, n: i64) -> EmObj {
    reg(Value::integer(n as i128))
}
unsafe extern "C-unwind" fn api_float(_e: EmEnv, f: f64) -> EmObj {
    reg(Value::float(f))
}
unsafe extern "C-unwind" fn api_str(_e: EmEnv, p: *const c_char, n: usize) -> EmObj {
    let b = unsafe { std::slice::from_raw_parts(p as *const u8, n) };
    reg(Value::string(String::from_utf8_lossy(b)))
}
unsafe extern "C-unwind" fn api_intern(e: EmEnv, p: *const c_char, n: usize) -> EmObj {
    api_intern_kind(e, p, n, 0)
}
unsafe extern "C-unwind" fn api_intern_kind(e: EmEnv, p: *const c_char, n: usize, kind: i64) -> EmObj {
    let bytes = unsafe { std::slice::from_raw_parts(p as *const u8, n) };
    let i = host(e);
    let text = match crate::lisp::text::EmacsText::from_internal_bytes(bytes, true) {
        Ok(text) => text,
        Err(_) => return outcome(e, Err(i.error("Invalid native symbol name encoding"))),
    };
    let key = if kind & 1 != 0 {
        crate::lisp::obarray::InternKey::from_text_kind(&text, false)
    } else if kind & 2 != 0 {
        crate::lisp::obarray::InternKey::from_text_kind(&text, true)
    } else {
        crate::lisp::obarray::InternKey::from_text(&text)
    };
    let existing = i.obarray.lookup_key(&key).is_some();
    let symbol = i.obarray.intern_with_key(&text, key);
    if !existing && kind != 0 {
        let reference = Value::string_ref(&text);
        if kind & 1 != 0 { i.mark_unibyte(&reference); }
        if kind & 2 != 0 { i.mark_multibyte(&reference); }
        i.obarray.symbol_mut(symbol).name_string = Some(reference);
    }
    reg(i.sym(symbol))
}
unsafe extern "C-unwind" fn api_read(e: EmEnv, p: *const c_char, n: usize) -> EmObj {
    let b = unsafe { std::slice::from_raw_parts(p as *const u8, n) };
    let i = host(e);
    let s = match crate::lisp::text::EmacsText::from_internal_bytes(b, true) {
        Ok(text) => text,
        Err(_) => return outcome(e, Err(i.error("Invalid native Lisp constant encoding"))),
    };
    outcome(e, i.read_from_string(&s, 0).map(|(v, _)| v))
}
#[derive(Hash, PartialEq, Eq)]
enum ConstantKey {
    Read(usize, usize),
    String(usize, usize),
    StringKind(usize, usize, i64),
    Float(u64),
}

fn native_constant(key: ConstantKey, create: impl FnOnce() -> EmObj) -> EmObj {
    let Some(native) = ACTIVE_NATIVE.get() else {
        return create();
    };
    // Static literals can share addresses, so distinguish read syntax from
    // raw string bytes. Retain Values, never call-local OBJTAB handles.
    if let Some(value) = META.with(|m| m.borrow()[native].constants.get(&key).cloned()) {
        return reg(value);
    }
    let handle = create();
    if handle != EM_INVALID {
        let value = obj(handle);
        META.with(|m| m.borrow_mut()[native].constants.insert(key, value));
    }
    handle
}

unsafe extern "C-unwind" fn api_read_constant(e: EmEnv, p: *const c_char, n: usize) -> EmObj {
    native_constant(ConstantKey::Read(p as usize, n), || api_read(e, p, n))
}
unsafe extern "C-unwind" fn api_string_constant(e: EmEnv, p: *const c_char, n: usize) -> EmObj {
    native_constant(ConstantKey::String(p as usize, n), || api_str(e, p, n))
}
unsafe extern "C-unwind" fn api_string_constant_kind(
    e: EmEnv,
    p: *const c_char,
    n: usize,
    kind: i64,
) -> EmObj {
    native_constant(ConstantKey::StringKind(p as usize, n, kind), || {
        let bytes = unsafe { std::slice::from_raw_parts(p as *const u8, n) };
        let text = match crate::lisp::text::EmacsText::from_internal_bytes(bytes, true) {
            Ok(text) => text,
            Err(_) => {
                let i = host(e);
                return outcome(e, Err(i.error("Invalid native string constant encoding")));
            }
        };
        let handle = reg(Value::string_with_kind(text, kind & 2 != 0));
        if let Value::Str(string) = obj(handle) {
            let i = host(e);
            if kind & 1 != 0 {
                i.mark_unibyte(&string);
            }
            if kind & 2 != 0 {
                i.mark_multibyte(&string);
            }
        }
        handle
    })
}
unsafe extern "C-unwind" fn api_float_constant(e: EmEnv, f: f64) -> EmObj {
    native_constant(ConstantKey::Float(f.to_bits()), || api_float(e, f))
}
unsafe extern "C-unwind" fn api_truthy(_e: EmEnv, h: EmObj) -> i64 {
    (!obj(h).is_nil()) as i64
}
unsafe extern "C-unwind" fn api_call(e: EmEnv, f: EmObj, argv: *const EmObj, argc: i64) -> EmObj {
    let i = host(e);
    // `call' takes already-evaluated values → `apply', not
    // `call_function' (which evals each arg as a form).
    let args: Vec<Value> = (0..argc as usize)
        .map(|k| obj(unsafe { *argv.add(k) }))
        .collect();
    let f = obj(f);
    outcome(e, i.apply(&f, args))
}
unsafe extern "C-unwind" fn api_eval(e: EmEnv, form: EmObj) -> EmObj {
    let i = host(e);
    let form = obj(form);
    outcome(e, i.eval(&form))
}
unsafe extern "C-unwind" fn api_sym_set(e: EmEnv, s: EmObj, v: EmObj) -> EmObj {
    let i = host(e);
    let (s, v) = (obj(s), obj(v));
    match s {
        Value::Sym(id) => {
            let v2 = v.clone();
            outcome(e, i.set_symbol(id, v).map(|()| v2))
        }
        other => outcome(e, Err(i.wrong_type_mut("symbolp", &other))),
    }
}
unsafe extern "C-unwind" fn api_binding_mark(e: EmEnv) -> usize {
    host(e).specbind_depth()
}
unsafe extern "C-unwind" fn api_bind(e: EmEnv, symbol: EmObj, value: EmObj) -> EmObj {
    let i = host(e);
    let symbol = obj(symbol);
    let result = match i.sym_id(&symbol) {
        Some(id) => i.specbind(id, obj(value)).map(|_| Value::Nil),
        None => Err(i.wrong_type_mut("symbolp", &symbol)),
    };
    outcome(e, result)
}
unsafe extern "C-unwind" fn api_unbind(e: EmEnv, depth: usize) -> EmObj {
    let result = host(e).unbind_to(depth).map(|_| Value::Nil);
    outcome(e, result)
}
unsafe extern "C-unwind" fn api_sym_value(e: EmEnv, symbol: EmObj) -> EmObj {
    let i = host(e);
    let name = i.q("symbol-value");
    let symbol = obj(symbol);
    let original = super::data::SUBRS.iter().find(|subr| subr.name == "symbol-value").unwrap();
    let result = if i.native_trampolines.contains("symbol-value") {
        // GNU's subr trampoline still observes a replacement function cell.
        i.apply(&Value::Sym(name), vec![symbol])
    } else {
        (original.func)(i, vec![symbol].into())
    };
    outcome(e, result)
}
unsafe extern "C-unwind" fn api_signal(e: EmEnv, s: EmObj, d: EmObj) -> EmObj {
    let result = super::evalfn::f_signal(host(e), vec![obj(s), obj(d)].into());
    outcome(e, result)
}
unsafe extern "C-unwind" fn api_car(e: EmEnv, h: EmObj) -> EmObj {
    let v = obj(h);
    match &v {
        Value::Cons(c) => reg(c.borrow().car.clone()),
        Value::Nil => reg(Value::Nil),
        _ => {
            let i = host(e);
            outcome(e, Err(i.wrong_type_mut("consp", &v)))
        }
    }
}
unsafe extern "C-unwind" fn api_cdr(e: EmEnv, h: EmObj) -> EmObj {
    let v = obj(h);
    match &v {
        Value::Cons(c) => reg(c.borrow().cdr.clone()),
        Value::Nil => reg(Value::Nil),
        _ => {
            let i = host(e);
            outcome(e, Err(i.wrong_type_mut("consp", &v)))
        }
    }
}
unsafe extern "C-unwind" fn api_cons(_e: EmEnv, a: EmObj, d: EmObj) -> EmObj {
    reg(Value::cons(obj(a), obj(d)))
}
unsafe extern "C-unwind" fn api_listn(_e: EmEnv, argv: *const EmObj, n: i64) -> EmObj {
    let items: Vec<Value> = (0..n.max(0) as usize)
        .map(|k| obj(unsafe { *argv.add(k) }))
        .collect();
    reg(Value::list(items))
}
unsafe extern "C-unwind" fn api_defsubr(
    _api: *const EmApi,
    e: EmEnv,
    unit: i64,
    desc: *const c_void,
) -> i64 {
    let d = unsafe { &*(desc as *const EmFnDesc) };
    let i = host(e);
    let name_bytes = unsafe { std::slice::from_raw_parts(d.name as *const u8, d.name_len) };
    let name = match crate::lisp::text::EmacsText::from_internal_bytes(name_bytes, true) {
        Ok(name) => name,
        Err(_) => {
            PENDING.with(|pending| *pending.borrow_mut() = Some(i.error("Invalid native function name encoding")));
            return 1;
        }
    };
    let doc = if d.doc.is_null() {
        crate::lisp::text::EmacsText::default()
    } else {
        let bytes = unsafe { std::slice::from_raw_parts(d.doc as *const u8, d.doc_len) };
        match crate::lisp::text::EmacsText::from_internal_bytes(bytes, true) {
            Ok(text) => text,
            Err(_) => {
                PENDING.with(|pending| *pending.borrow_mut() = Some(i.error("Invalid native documentation encoding")));
                return 1;
            }
        }
    };
    // `defmacro': evaluate the recorded `(defmacro …)' form — a
    // macro's meaning is its expander, not a callable subr.
    if !d.sexp.is_null() && d.sexp_len > 0 {
        return match unsafe { read_native_descriptor(i, d.sexp, d.sexp_len) } {
            Ok(form) => match i.eval(&form) {
                Ok(_) => 0,
                Err(Flow::Signal(signal)) => {
                    if std::env::var_os("REMACS_ELN_DBG").is_some() {
                        eprintln!(
                            "ELN defsubr sexp eval failed for {}: {} {}",
                            name,
                            i.prin1_to_string(&signal.symbol()),
                            i.prin1_to_string(&signal.data())
                        );
                    }
                    PENDING.with(|p| *p.borrow_mut() = Some(Flow::Signal(signal)));
                    1
                }
                Err(flow) => {
                    PENDING.with(|p| *p.borrow_mut() = Some(flow));
                    1
                }
            },
            Err(_) => {
                if std::env::var_os("REMACS_ELN_DBG").is_some() {
                    eprintln!("ELN defsubr sexp READ failed for {}", name);
                }
                1
            }
        };
    }
    let metadata = unsafe { read_native_descriptor(i, d.arglist, d.arglist_len) }
        .and_then(|arglist| unsafe { read_native_descriptor(i, d.iform, d.iform_len) }
            .map(|iform| (arglist, iform)));
    let (arglist, iform) = match metadata {
        Ok(metadata) => metadata,
        Err(flow) => {
            PENDING.with(|pending| *pending.borrow_mut() = Some(flow));
            return 1;
        }
    };
    match install_native_subr(i, unit, &name, d.min, d.max, d.entry, &doc) {
        Ok(v) => {
            META.with(|m| {
                if let Some(m) = m.borrow_mut().last_mut() {
                    m.arglist = arglist;
                    m.iform = iform;
                }
            });
            let _ = v;
            0
        }
        Err(()) => 1,
    }
}

unsafe fn read_native_descriptor(i: &mut Interp, pointer: *const c_char, length: usize) -> Result<Value, Flow> {
    if pointer.is_null() || length == 0 { return Ok(Value::Nil); }
    let bytes = unsafe { std::slice::from_raw_parts(pointer as *const u8, length) };
    let text = crate::lisp::text::EmacsText::from_internal_bytes(bytes, true)
        .map_err(|_| i.error("Invalid native descriptor text encoding"))?;
    i.read_from_string(&text, 0).map(|(value, _)| value)
}

unsafe fn cstr(p: *const c_char, n: usize) -> String {
    let b = unsafe { std::slice::from_raw_parts(p as *const u8, n) };
    String::from_utf8_lossy(b).into_owned()
}

static API: EmApi = EmApi {
    mk_nil: api_nil,
    mk_t: api_t,
    mk_int: api_int,
    mk_float: api_float,
    mk_str: api_str,
    intern: api_intern,
    read: api_read,
    truthy: api_truthy,
    call: api_call,
    eval: api_eval,
    call_lambda: api_call,
    sym_set: api_sym_set,
    signal: api_signal,
    car: api_car,
    cdr: api_cdr,
    cons: api_cons,
    listn: api_listn,
    defsubr: api_defsubr,
    read_constant: api_read_constant,
    string_constant: api_string_constant,
    float_constant: api_float_constant,
    string_constant_kind: api_string_constant_kind,
    binding_mark: api_binding_mark,
    bind: api_bind,
    unbind: api_unbind,
    sym_value: api_sym_value,
    intern_kind: api_intern_kind,
};

/// Mirrors the generated descriptor (repr(C), must match codegen).
#[repr(C)]
struct EmFnDesc {
    name: *const c_char,
    name_len: usize,
    min: u32,
    /// 0xFFFF_FFFF means MANY (`&rest` present).
    max: u32,
    entry: EmEntry,
    doc: *const c_char,
    doc_len: usize,
    arglist: *const c_char,
    arglist_len: usize,
    iform: *const c_char,
    iform_len: usize,
    /// For `is_macro' functions: a `(defmacro …)' form to evaluate
    /// instead of installing a subr (macros expand at read time of
    /// the caller — they can't be trampolined through apply).
    sexp: *const c_char,
    sexp_len: usize,
}

// ---------------------------------------------------------------------------
// Native subr registration — trampoline pool
// ---------------------------------------------------------------------------

struct NativeMeta {
    name: crate::lisp::text::EmacsText,
    documentation: crate::lisp::text::EmacsText,
    subr: Option<&'static Subr>,
    unit: i64,
    arglist: Value,
    iform: Value,
    constants: HashMap<ConstantKey, Value>,
}

fn tramp<const K: usize>(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let entry = NATIVES.with(|n| n.borrow()[K]);
    let env = i as *mut Interp as EmEnv;
    let mark = OBJTAB.with(|t| t.borrow().len());
    let args: Vec<EmObj> = a.iter().map(|v| reg(v.clone())).collect();
    let previous_native = ACTIVE_NATIVE.replace(Some(K));
    let bindings = i.specbind_depth();
    let r = unsafe { entry(&API, env, args.as_ptr(), args.len() as i64) };
    ACTIVE_NATIVE.set(previous_native);
    // A pending signal always wins, including for previously compiled units
    // that may return a valid handle after a failed callback.
    let out = match PENDING.with(|p| p.borrow_mut().take()) {
        Some(flow) => Err(flow),
        None if r == EM_INVALID => Err(i.error("native function failed")),
        None => Ok(obj(r)),
    };
    let out = i.offer_flow(out);
    let out = i.unbind_to(bindings).and(out);
    // Handles registered during this call are disposable — the result
    // was cloned out; constant caches retain Values, never these handles.
    OBJTAB.with(|t| t.borrow_mut().truncate(mark));
    out
}

macro_rules! tramps {
    ($($k:ident => $n:literal),+) => {
        $(fn $k(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
            tramp::<$n>(i, a)
        })*
    };
}

// ---------------------------------------------------------------------------
// Compilation units
// ---------------------------------------------------------------------------

struct Unit {
    path: crate::lisp::text::EmacsText,
    /// Kept loaded for the life of the process (like GNU's units).
    _lib: libloading::Library,
}

type UnitInit = unsafe extern "C-unwind" fn(*const EmApi, EmEnv, i64) -> i32;

enum LoadError {
    Open(String),
    Lisp(Flow),
}

impl From<String> for LoadError {
    fn from(message: String) -> Self {
        Self::Open(message)
    }
}

/// dlopen an .eln and run its ordered top-level initialization.
fn load_eln<P: crate::lisp::text::TextSource + ?Sized>(i: &mut Interp, path: &P) -> Result<usize, LoadError> {
    let path = path.with_text(|text| crate::editor::expand_file_name_str(i, text));
    let native = crate::editor::native_path(i, &path).map_err(LoadError::Lisp)?;
    // libloading's error is the raw dlopen(3) message — GNU stores
    // exactly that string as the second datum of
    // `native-lisp-load-failed'.
    // GNU dynlib_load passes plain RTLD_LAZY (0x0001) — no RTLD_LOCAL —
    // and dlopen(3)'s error string (which embeds the flag bits) is what
    // `native-lisp-load-failed' reports, so match the flags exactly.
    let lib = unsafe {
        libloading::os::unix::Library::open(
            Some(&native),
            libloading::os::unix::RTLD_LAZY,
        )
        .map(libloading::Library::from)
    }
    .map_err(|e| e.to_string())?;
    let init: UnitInit = unsafe {
        *lib.get(b"remacs_eln_init_v2\0")
            .or_else(|_| lib.get(b"remacs_eln_init\0"))
            .map_err(|_| "Not a remacs .eln (missing remacs_eln_init)".to_string())?
    };
    // New file units carry their source's binding mode. Old units and
    // single-function units keep their existing ABI and calling context.
    let lexical = unsafe {
        lib.get::<*const i32>(b"remacs_eln_lexical\0")
            .ok()
            .map(|flag| **flag != 0)
    };
    // Reserve the index before executing Lisp (which may load another unit).
    // Keep the library even if initialization signals: definitions preceding
    // the failing form remain callable and must not point into unloaded code.
    let idx = UNITS.with(|u| {
        let mut units = u.borrow_mut();
        let idx = units.len();
        units.push(Unit {
            path: path.to_string(),
            _lib: lib,
        });
        idx
    });
    let saved_lexenv = i.lexenv.clone();
    if let Some(lexical) = lexical {
        // The file controls binding semantics, not the caller-visible value
        // of lexical-binding (native-elisp-load does not bind that variable).
        i.lexenv = if lexical {
            crate::lisp::eval::lexenv_root()
        } else {
            None
        };
    }
    let handles = OBJTAB.with(|t| t.borrow().len());
    let env = i as *mut Interp as EmEnv;
    let rc = unsafe { init(&API, env, idx as i64) };
    let pending = PENDING.with(|p| p.borrow_mut().take());
    OBJTAB.with(|t| t.borrow_mut().truncate(handles));
    i.lexenv = saved_lexenv;
    if let Some(flow) = pending {
        return Err(LoadError::Lisp(flow));
    }
    if rc != 0 {
        return Err(LoadError::Open("remacs_eln_init failed".into()));
    }
    Ok(idx)
}

fn unit_tag(i: &mut Interp) -> SymId {
    i.q("native-comp-unit")
}

fn unit_value(i: &mut Interp, idx: usize) -> Value {
    let tag = unit_tag(i);
    Value::vector(vec![
        Value::Sym(tag),
        Value::Int(idx as i128),
    ])
}

fn unit_index(i: &mut Interp, v: &Value) -> Result<usize, Flow> {
    if let Value::Vec(rc) = v {
        let vv = rc.borrow();
        if vv.len() == 2 {
            if let (Value::Sym(s), Value::Int(k)) = (&vv[0], &vv[1]) {
                if *s == unit_tag(i) {
                    return Ok((*k).max(0) as usize);
                }
            }
        }
    }
    Err(i.wrong_type_mut("native-comp-unit", &v))
}

/// Whether `s' is one of our native-compiled subrs → trampoline slot.
pub(crate) fn is_native(s: &'static Subr) -> Option<usize> {
    TRAMPS
        .iter()
        .position(|f| *f as usize == s.func as usize)
        .filter(|k| *k < NATIVES.with(|n| n.borrow().len()))
}

pub(crate) fn native_name(subr: &'static Subr) -> Option<Vec<u8>> {
    let index = is_native(subr)?;
    META.with(|metadata| metadata.borrow().get(index)
        .map(|entry| entry.name.to_internal_bytes(true).unwrap()))
}

fn native_function_in_unit(unit: usize) -> Option<Value> {
    META.with(|metadata| metadata.borrow().iter().rev()
        .find(|entry| entry.unit == unit as i64)
        .and_then(|entry| entry.subr).map(Value::Subr))
}

/// Native documentation belongs to the function slot, not its diagnostic name.
pub(crate) fn native_documentation(subr: &'static Subr) -> Option<Value> {
    let index = is_native(subr)?;
    META.with(|metadata| {
        metadata.borrow().get(index).map(|entry| {
            if entry.documentation.is_empty() { Value::Nil }
            else { Value::string(entry.documentation.clone()) }
        })
    })
}

/// Interactive metadata is tied to a native trampoline slot, including nil.
pub(crate) fn native_interactive_form(subr: &'static Subr) -> Option<Value> {
    let index = is_native(subr)?;
    META.with(|metadata| metadata.borrow().get(index).map(|entry| entry.iform.clone()))
}

fn install_native_subr<T: crate::lisp::text::TextSource + ?Sized, N: crate::lisp::text::TextSource + ?Sized>(
    i: &mut Interp,
    unit: i64,
    name: &N,
    min: u32,
    max: u32,
    entry: EmEntry,
    doc: &T,
) -> Result<Value, ()> {
    let name = name.with_text(|text| text.to_owned());
    let documentation = doc.with_text(|text| text.to_owned());
    let func = NATIVES.with(|n| {
        let mut n = n.borrow_mut();
        if n.len() >= TRAMPS.len() {
            return Err(());
        }
        n.push(entry);
        Ok(TRAMPS[n.len() - 1])
    })?;
    META.with(|m| {
        m.borrow_mut().push(NativeMeta {
            name: name.to_string(),
            documentation,
            subr: None,
            unit,
            constants: HashMap::new(),
            arglist: Value::Nil,
            iform: Value::Nil,
        })
    });
    let arity = if max == u32::MAX {
        Arity::Many { min: min as u16 }
    } else {
        Arity::Range {
            min: min as u16,
            max: max as u16,
        }
    };
    let s: &'static Subr = Box::leak(Box::new(Subr {
        name: Box::leak(format!("{name}").into_boxed_str()),
        arity,
        func,
        doc: "",
    }));
    META.with(|metadata| metadata.borrow_mut().last_mut().unwrap().subr = Some(s));
    let id = i.intern(&name);
    let v = Value::Subr(s);
    i.fset(id, v.clone());
    Ok(v)
}

// ---------------------------------------------------------------------------
// C codegen
// ---------------------------------------------------------------------------

const C_PRELUDE: &str = r#"
typedef unsigned long long em_obj;
typedef unsigned long em_env;
typedef em_obj (*em_entry)(const void *, em_env, const em_obj *, long);
struct em_fn_desc {
    const char *name; unsigned long name_len;
    unsigned int min, max; em_entry entry;
    const char *doc; unsigned long doc_len;
    const char *arglist; unsigned long arglist_len;
    const char *iform; unsigned long iform_len;
    const char *sexp; unsigned long sexp_len;
};
struct em_api {
    em_obj (*nil)(em_env);
    em_obj (*t)(em_env);
    em_obj (*mk_int)(em_env, long long);
    em_obj (*mk_float)(em_env, double);
    em_obj (*mk_str)(em_env, const char *, unsigned long);
    em_obj (*intern)(em_env, const char *, unsigned long);
    em_obj (*read)(em_env, const char *, unsigned long);
    long (*truthy)(em_env, em_obj);
    em_obj (*call)(em_env, em_obj, const em_obj *, long);
    em_obj (*eval)(em_env, em_obj);
    em_obj (*call_lambda)(em_env, em_obj, const em_obj *, long);
    em_obj (*sym_set)(em_env, em_obj, em_obj);
    em_obj (*signal)(em_env, em_obj, em_obj);
    em_obj (*car)(em_env, em_obj);
    em_obj (*cdr)(em_env, em_obj);
    em_obj (*cons)(em_env, em_obj, em_obj);
    em_obj (*listn)(em_env, const em_obj *, long);
    long (*defsubr)(const void *, em_env, long, const void *);
    em_obj (*read_constant)(em_env, const char *, unsigned long);
    em_obj (*string_constant)(em_env, const char *, unsigned long);
    em_obj (*float_constant)(em_env, double);
    em_obj (*string_constant_kind)(em_env, const char *, unsigned long, long);
    unsigned long (*binding_mark)(em_env);
    em_obj (*bind)(em_env, em_obj, em_obj);
    em_obj (*unbind)(em_env, unsigned long);
    em_obj (*sym_value)(em_env, em_obj);
    em_obj (*intern_kind)(em_env, const char *, unsigned long, long);
};
typedef const struct em_api *A_t;
#define NIL (A->nil)(E)
#define TT (A->t)(E)
#define INT(n) (A->mk_int)(E,(long long)(n))
#define FLT(x) (A->float_constant)(E,(x))
#define TRUTH(h) (A->truthy)(E,(h))
/* Exit the native entry before any later Lisp form or enclosing operation.
   The pending signal stays in Rust; never unwind Rust frames through C. */
#define CHECK(expr) ({em_obj checked=(expr);if(!checked)return 0;checked;})
#define CALL(f,av,n) CHECK((A->call)(E,(f),(av),(n)))
#define EVAL(form) CHECK((A->eval)(E,(form)))
#define SYMSET(s,v) CHECK((A->sym_set)(E,(s),(v)))
#define LISTN(av,n) (A->listn)(E,(av),(n))
#define RDC(lit) CHECK((A->read)(E,(lit),sizeof(lit)-1))
#define KDC(lit) CHECK((A->read_constant)(E,(lit),sizeof(lit)-1))
#define SDC(lit) (A->intern)(E,(lit),sizeof(lit)-1)
#define SYMVAL(s) CHECK((A->sym_value)(E,(s)))
#define BIND(s,v) CHECK((A->bind)(E,(s),(v)))
#define UNBIND(mark) CHECK((A->unbind)(E,(mark)))
"#;

/// Sanitize a Lisp symbol name to a C identifier.
fn c_name<N: crate::lisp::text::TextSource + ?Sized>(name: &N, k: usize) -> String {
    name.with_text(|name| {
    let mut s = String::new();
    for c in name.chars() {
        s.push(match c.unicode().unwrap_or('u') {
            '-' => '_',
            '*' => 'X',
            '/' => 'F',
            '+' => 'P',
            '<' => 'L',
            '>' => 'G',
            '=' => 'E',
            '?' => 'Q',
            '!' => 'B',
            c if c.is_ascii_alphanumeric() || c == '_' => c,
            _ => 'u',
        });
    }
    if s.is_empty() || s.as_bytes()[0].is_ascii_digit() {
        s.insert(0, 'F');
    }
    format!("F{k}_{s}")
    })
}

/// Escape a string into a C string literal (with quotes).
fn c_str(s: &str) -> String {
    c_bytes(s.as_bytes())
}

/// GNU escapes the UTF-8 octets of surrogate names as internal byte8 text.
fn native_name_bytes(name: &crate::lisp::text::EmacsText) -> Vec<u8> {
    let mut bytes = Vec::new();
    for character in name.chars() {
        if (0xd800..=0xdfff).contains(&character.code()) {
            let mut encoded = Vec::new();
            character.encode_internal(&mut encoded);
            for byte in encoded {
                crate::lisp::text::EmacsChar::new(0x3fff00 + byte as u32).unwrap()
                    .encode_internal(&mut bytes);
            }
        } else {
            character.encode_internal(&mut bytes);
        }
    }
    bytes
}

fn c_text(text: &crate::lisp::text::EmacsText) -> String {
    c_bytes(&text.to_internal_bytes(true).unwrap())
}

fn c_bytes(bytes: &[u8]) -> String {
    let mut o = String::from("\"");
    for &b in bytes {
        match b {
            b'"' => o.push_str("\\\""),
            b'\\' => o.push_str("\\\\"),
            b'\n' => o.push_str("\\n"),
            b'\t' => o.push_str("\\t"),
            0x20..=0x7e => o.push(b as char),
            _ => o.push_str(&format!("\\{:03o}", b)),
        }
    }
    o.push('"');
    o
}

struct Emitter<'a> {
    i: &'a mut Interp,
    /// Lexical variable → C name mapping stack.
    scopes: Vec<Vec<(SymId, Option<String>)>>,
    lexical: bool,
    serial: usize,
    /// Set when a form cannot be emitted directly.
    unsupported: bool,
    error: Option<Flow>,
}

impl<'a> Emitter<'a> {
    fn symbol(&self, symbol: SymId) -> String {
        let original = self.i.symbol_name_text(symbol);
        let (text, kind) = if original.chars().any(|character| (0xd800..=0xdfff).contains(&character.code())) {
            // GNU's native symbol relocation exposes the same C spelling as subr names.
            let (text, multibyte) = crate::lisp::value::auto_string_bytes(&native_name_bytes(&original));
            (text, if multibyte { 2 } else { 1 })
        } else {
            let kind = self.i.obarray.symbol(symbol).name_string.as_ref().map_or(0, |name| {
                i64::from(self.i.is_unibyte_str(name)) | (i64::from(self.i.is_multibyte_str(name)) << 1)
            });
            (original, kind)
        };
        let bytes = text.to_internal_bytes(true).unwrap();
        format!("CHECK((A->intern_kind)(E,{},{},{kind}))", c_bytes(&bytes), bytes.len())
    }

    fn constant(&self, value: &Value) -> String {
        format!("KDC({})", c_text(&self.i.prin1_to_string(value)))
    }

    fn lookup(&self, id: SymId) -> Option<String> {
        self.scopes
            .iter()
            .rev()
            .flat_map(|s| s.iter().rev())
            .find(|(k, _)| *k == id)
            .and_then(|(_, n)| n.clone())
    }

    fn dynamic(&self, id: SymId) -> bool {
        !self.lexical || self.i.obarray.symbol(self.i.var_alias_target(id)).special
    }

    fn parameter_binding(&mut self, id: SymId, value: &str) -> String {
        // Lambda arguments are lexical in a lexical function, even when
        // their names also designate globally special variables.
        if !self.lexical {
            self.scopes.last_mut().unwrap().push((id, None));
            format!("BIND({},{value});", self.symbol(id))
        } else {
            self.scopes.last_mut().unwrap().push((id, Some(value.to_string())));
            String::new()
        }
    }

    /// Emit an expression evaluating to `em_obj'.
    fn form(&mut self, f: &Value) -> String {
        if self.error.is_some() { return "NIL".into(); }
        match f {
            Value::Nil => "NIL".into(),
            Value::Int(n) => {
                if let Ok(x) = i64::try_from(*n) {
                    format!("INT({x})")
                } else {
                    self.constant(f)
                }
            }
            Value::Float(x) => format!("FLT({:?})", **x),
            Value::Str(s) => {
                // The same raw characters can denote unibyte bytes or
                // multibyte eight-bit characters. Carry the metadata too;
                // printing/reading instead would lose surrogate characters.
                let kind = i64::from(self.i.is_unibyte_str(s))
                    | (i64::from(self.i.is_multibyte_str(s)) << 1);
                let s = s.borrow();
                let bytes = s.to_internal_bytes(true).unwrap();
                format!(
                    "(A->string_constant_kind)(E,{},{},{kind})",
                    c_bytes(&bytes),
                    bytes.len()
                )
            }
            Value::Sym(id) => {
                if *id == sym::T {
                    return "TT".into();
                }
                if let Some(n) = self.lookup(*id) {
                    return n;
                }
                let name = self.i.symbol_name_text(*id);
                if name.starts_with(':') {
                    return self.symbol(*id);
                }
                // A free variable reads its current dynamic/buffer-local value,
                // not the symbol itself or the caller's lexical environment.
                // Like GNU native code, honor symbol-value's function cell.
                format!("SYMVAL({})", self.symbol(*id))
            }
            Value::Cons(c) => {
                let car = c.borrow().car.clone();
                self.cons_form(f, &car)
            }
            // Self-evaluating literals (vectors, records, …):
            // retain one read object across calls, just like quote constants.
            other => self.constant(other),
        }
    }

    fn args_of(&self, f: &Value) -> Vec<Value> {
        match f {
            Value::Cons(c) => c.borrow().cdr.list_to_vec().unwrap_or_default(),
            _ => Vec::new(),
        }
    }

    fn cons_form(&mut self, whole: &Value, car: &Value) -> String {
        let head = match car {
            Value::Sym(id) => *id,
            _ => {
                // ((lambda ...) a ...) or a computed head: emit the
                // head as an expression and call it.
                let fexpr = self.form(car);
                return self.call_expr(&fexpr, whole);
            }
        };
        let name = self.i.symbol_name(head);
        match name.as_str() {
            "quote" => {
                let a = self.args_of(whole);
                self.constant(&a[0])
            }
            "if" => {
                let a = self.args_of(whole);
                let c = self.form(&a[0]);
                let th = a
                    .get(1)
                    .map(|v| self.form(v))
                    .unwrap_or_else(|| "NIL".into());
                let el = if a.len() > 2 {
                    self.seq(&a[2..])
                } else {
                    "NIL".into()
                };
                format!("(TRUTH({c})?{th}:{el})")
            }
            "progn" => self.seq(&self.args_of(whole)),
            "prog1" => {
                let a = self.args_of(whole);
                if a.is_empty() {
                    return "NIL".into();
                }
                let first = self.form(&a[0]);
                let rest = if a.len() > 1 {
                    self.seq(&a[1..])
                } else {
                    "NIL".into()
                };
                format!("({{em_obj r={first};(void)({rest});r;}})")
            }
            "let" | "let*" => self.let_form(whole, name == "let*"),
            "while" => {
                let a = self.args_of(whole);
                let c = self.form(&a[0]);
                let body = self.seq(&a[1..]);
                format!("({{while(TRUTH({c})){{(void)({body});}}NIL;}})")
            }
            "and" => {
                let a = self.args_of(whole);
                // Unique label per `and' — `goto' into a nested
                // statement expression is a hard error, and fixed
                // names collide when `and's nest.
                self.serial += 1;
                let lbl = format!("ae{}", self.serial);
                // The identity of conjunction is t, including zero operands.
                let mut out = String::from("({em_obj r=TT;");
                for x in &a {
                    let v = self.form(x);
                    out.push_str(&format!("r={v};if(!TRUTH(r))goto {lbl};"));
                }
                out.push_str(&format!("{lbl}:r;}})"));
                out
            }
            "or" => {
                let a = self.args_of(whole);
                self.serial += 1;
                let lbl = format!("oe{}", self.serial);
                let mut out = String::from("({em_obj r=NIL;");
                for x in &a {
                    let v = self.form(x);
                    out.push_str(&format!("r={v};if(TRUTH(r))goto {lbl};"));
                }
                out.push_str(&format!("{lbl}:r;}})"));
                out
            }
            "not" | "null" => {
                let a = self.args_of(whole);
                let v = self.form(&a[0]);
                format!("(TRUTH({v})?NIL:TT)")
            }
            "setq" => {
                let a = self.args_of(whole);
                let mut out = String::from("({em_obj r=NIL;");
                for pair in a.chunks(2) {
                    if pair.len() < 2 {
                        self.unsupported = true;
                        continue;
                    }
                    let v = self.form(&pair[1]);
                    match &pair[0] {
                        Value::Sym(id) => match self.lookup(*id) {
                            Some(cn) => out.push_str(&format!("r={cn}={v};")),
                            None => {
                                out.push_str(&format!("r=SYMSET({},{v});", self.symbol(*id)));
                            }
                        },
                        _ => self.unsupported = true,
                    }
                }
                out.push_str("r;})");
                out
            }
            "function" => {
                let a = self.args_of(whole);
                match a.first() {
                    Some(Value::Sym(id)) => {
                        self.symbol(*id)
                    }
                    _ => {
                        // C locals are not visible to interpreter eval.  Run
                        // the whole function through the closure fallback so
                        // nested lambdas capture its actual argument/local cells.
                        self.unsupported = true;
                        "NIL".into()
                    }
                }
            }
            // Non-local exits and excursion forms need real unwind
            // semantics — take the eval fallback for the whole fn.
            "catch"
            | "throw"
            | "condition-case"
            | "unwind-protect"
            | "save-excursion"
            | "save-restriction"
            | "save-current-buffer"
            | "save-match-data"
            | "save-window-excursion"
            | "track-mouse"
            | "save-selected-window"
            | "with-local-quit"
            | "defvar"
            | "defconst"
            | "interactive"
            | "declare"
            | "eval-when-compile"
            | "eval-and-compile" => {
                self.unsupported = true;
                "NIL".into()
            }
            _ => {
                // Macros and inline'd special forms: expand, then
                // re-emit the expansion.
                if self.is_macro(head) {
                    match self.expand_once(whole) {
                        Some(x) => return self.form(&x),
                        None => {
                            self.unsupported = true;
                            return "NIL".into();
                        }
                    }
                }
                let fexpr = self.symbol(head);
                self.call_expr(&fexpr, whole)
            }
        }
    }

    fn is_macro(&mut self, head: SymId) -> bool {
        match self.i.symbol_function(head) {
            Value::Lambda(l) => l.is_macro,
            Value::Cons(c) => {
                let b = c.borrow();
                matches!(&b.car, Value::Sym(m) if *m == sym::MACRO)
            }
            _ => false,
        }
    }

    fn expand_once(&mut self, whole: &Value) -> Option<Value> {
        let mid = self.i.q("macroexpand");
        let call = Value::list(vec![
            Value::Sym(mid),
            Value::list(vec![Value::Sym(sym::QUOTE), whole.clone()]),
        ]);
        self.i.eval(&call).ok()
    }

    fn call_expr(&mut self, fexpr: &str, whole: &Value) -> String {
        let argv = self.args_of(whole);
        if argv.is_empty() {
            return format!("CALL({fexpr},0,0)");
        }
        // C initializer elements need not be evaluated left-to-right. Keep
        // Lisp argument order explicit so a failure skips all later args.
        let mut parts = format!("({{em_obj fn={fexpr};em_obj av[{}];", argv.len());
        for (k, x) in argv.iter().enumerate() {
            let v = self.form(x);
            parts.push_str(&format!("av[{k}]={v};"));
        }
        parts.push_str(&format!("CALL(fn,av,{});}})", argv.len()));
        parts
    }

    /// `(e1 e2 …)` as one expression evaluating to the last value.
    fn seq(&mut self, forms: &[Value]) -> String {
        match forms.len() {
            0 => "NIL".into(),
            1 => self.form(&forms[0]),
            _ => {
                let mut s = String::from("({em_obj r;");
                for f in forms {
                    let v = self.form(f);
                    s.push_str(&format!("r={v};"));
                }
                s.push_str("r;})");
                s
            }
        }
    }

    fn let_form(&mut self, whole: &Value, star: bool) -> String {
        let a = self.args_of(whole);
        if a.is_empty() {
            return "NIL".into();
        }
        let Ok(binders) = a[0].list_to_vec() else {
            self.unsupported = true;
            return "NIL".into();
        };
        let optimize = self.i.q("byte-optimize");
        if self.i.symbol_value(optimize).truthy() {
            // cconv turns bare variable binders into one-element lists before
            // byte-optimize-letX sees them.  Keep the caller's form unchanged.
            let input = if binders.iter().any(|b| matches!(b, Value::Sym(_))) {
                let normalized = Value::list(binders.iter().map(|b| match b {
                    Value::Sym(_) => Value::list(vec![b.clone()]),
                    _ => b.clone(),
                }).collect());
                let head = Value::Sym(self.i.intern(if star { "let*" } else { "let" }));
                Value::cons(head, Value::cons(normalized, Value::list(a[1..].to_vec())))
            } else {
                whole.clone()
            };
            let optimizer = Value::Sym(self.i.q("byte-optimize-letX"));
            match self.i.apply(&optimizer, vec![input.clone()]) {
                Ok(form) if !same_form(&form, &input) => return self.form(&form),
                Ok(_) => {}
                Err(flow) => {
                    self.error = Some(flow);
                    return "NIL".into();
                }
            }
        }
        let mut frame = Vec::new();
        let mut decls = String::new();
        let mut bindings = String::new();
        let block = self.serial;
        self.serial += 1;
        let mut has_dynamic = false;
        if star {
            self.scopes.push(Vec::new());
        }
        for b in &binders {
            let (id, init) = match b {
                Value::Sym(id) => (*id, Value::Nil),
                Value::Cons(_) => {
                    let v = b.list_to_vec().unwrap_or_default();
                    match v.first() {
                        Some(Value::Sym(id)) if v.len() <= 2 =>
                            (*id, v.get(1).cloned().unwrap_or(Value::Nil)),
                        _ => {
                            self.unsupported = true;
                            continue;
                        }
                    }
                }
                _ => {
                    self.unsupported = true;
                    continue;
                }
            };
            if self.i.obarray.symbol(self.i.var_alias_target(id)).constant {
                self.unsupported = true;
                continue;
            }
            let dynamic = self.dynamic(id);
            has_dynamic |= dynamic;
            let cn = format!("v{}", self.serial);
            self.serial += 1;
            let iv = self.form(&init);
            decls.push_str(&format!("em_obj {cn}={iv};"));
            let binding = if dynamic {
                format!("BIND({},{cn});", self.symbol(id))
            } else {
                String::new()
            };
            // Compiled dynamic lets shadow a lexical argument of the same
            // name, unlike the interpreted evaluator's environment lookup.
            let entry = (id, (!dynamic).then_some(cn));
            if star {
                // The new binding is visible to later initializers and the
                // body, but its own initializer still sees the outer binding.
                decls.push_str(&binding);
                self.scopes.last_mut().unwrap().push(entry);
            } else {
                // `let': every init form sees the outer scope.
                bindings.push_str(&binding);
                frame.push(entry);
            }
        }
        if !star {
            self.scopes.push(frame);
        }
        let body = self.seq(&a[1..]);
        self.scopes.pop();
        if has_dynamic {
            format!("({{unsigned long scope{block}=A->binding_mark(E);{decls}{bindings}\
                      em_obj result{block}={body};UNBIND(scope{block});result{block};}})")
        } else {
            format!("({{{decls}{body};}})")
        }
    }
}

// ---------------------------------------------------------------------------
// Function emission
// ---------------------------------------------------------------------------

fn arglist_syms(i: &mut Interp, l: &crate::lisp::value::Lambda) -> Vec<Value> {
    let mut v: Vec<Value> = l.required.iter().map(|s| Value::Sym(*s)).collect();
    if !l.optional.is_empty() {
        v.push(Value::Sym(i.q("&optional")));
        for o in &l.optional {
            v.push(Value::Sym(o.sym));
        }
    }
    if let Some(r) = &l.rest {
        v.push(Value::Sym(i.q("&rest")));
        v.push(Value::Sym(*r));
    }
    v
}

/// Emit one function's entry + descriptor; Err when the function
/// cannot be represented at all (its caller decides the fallback).
fn emit_fn(
    i: &mut Interp,
    name: &crate::lisp::text::EmacsText,
    l: &Rc<crate::lisp::value::Lambda>,
    k: usize,
    force_fallback: bool,
) -> Result<(String, String), Flow> {
    let cn = c_name(name, k);
    let min = l.required.len() as u32;
    let max = if l.rest.is_some() {
        u32::MAX
    } else {
        (l.required.len() + l.optional.len()) as u32
    };

    let direct = if force_fallback {
        None
    } else {
        try_emit_body(i, l, &cn)?
    };
    let entry = direct.unwrap_or_else(|| emit_fallback(i, name, l, &cn));
    let arglist = l
        .arglist
        .clone()
        .unwrap_or_else(|| Value::list(arglist_syms(i, l)));
    let args_s = i.prin1_to_string(&arglist);
    let iform_s = l
        .interactive
        .as_ref()
        .map(|v| i.prin1_to_string(v))
        .unwrap_or_default();
    let doc_s = l.doc.clone().unwrap_or_default();
    // `defmacro': keep the whole definition as a `(defmacro …)' form
    // for the loader to evaluate; entry stays a fallback trampoline.
    let sexp_s = if l.is_macro {
        let mut head = vec![
            Value::Sym(i.q("defmacro")),
            Value::Sym(i.intern(name)),
            arglist.clone(),
        ];
        if let Some(d) = &l.doc {
            head.push(Value::string(d.clone()));
        }
        if let Some(f) = &l.interactive {
            head.push(f.clone());
        }
        head.extend(l.body.iter().cloned());
        i.prin1_to_string(&Value::list(head))
    } else {
        (String::new()).into()
    };
    let name_bytes = native_name_bytes(name);
    let desc = format!(
        "static const struct em_fn_desc D{cn}={{{name},{nl},{min},{mx},{cn},{dstr},{dl},{astr},{al},{istr},{il},{sstr},{sl}}};",
        name = c_bytes(&name_bytes),
        nl = name_bytes.len(),
        mx = if max == u32::MAX {
            "0xffffffffu".to_string()
        } else {
            max.to_string()
        },
        dstr = c_text(&doc_s),
        dl = doc_s.to_internal_bytes(true).unwrap().len(),
        astr = c_text(&args_s),
        al = args_s.to_internal_bytes(true).unwrap().len(),
        istr = c_text(&iform_s),
        il = iform_s.to_internal_bytes(true).unwrap().len(),
        sstr = c_text(&sexp_s),
        sl = sexp_s.to_internal_bytes(true).unwrap().len(),
    );
    Ok((entry, desc))
}

/// Direct codegen; None when any form needs eval fallback.
fn try_emit_body(i: &mut Interp, l: &Rc<crate::lisp::value::Lambda>, cn: &str) -> Result<Option<String>, Flow> {
    let require = Value::Sym(i.q("require"));
    let feature = Value::Sym(i.q("byte-opt"));
    i.apply(&require, vec![feature])?;
    let mut e = Emitter {
        i,
        scopes: Vec::new(),
        lexical: l.env.is_some(),
        serial: 0,
        unsupported: false,
        error: None,
    };
    e.scopes.push(Vec::new());

    // Parameter unpacking.
    let mut params = String::new();
    for n in 0..l.required.len() {
        params.push_str(&format!("em_obj p{n}=args[{n}];"));
        params.push_str(&e.parameter_binding(l.required[n], &format!("p{n}")));
    }
    for (k, p) in l.optional.iter().enumerate() {
        let idx = l.required.len() + k;
        let d = p
            .default
            .as_ref()
            .map(|dv| e.form(dv))
            .unwrap_or_else(|| "NIL".into());
        params.push_str(&format!("em_obj o{k}=(n>{idx})?args[{idx}]:{d};"));
        params.push_str(&e.parameter_binding(p.sym, &format!("o{k}")));
        if let Some(s) = p.supplied {
            // supplied-p param: expose as an em_obj too.
            let c = format!("v{}", e.serial);
            e.serial += 1;
            params.push_str(&format!("em_obj {c}=(n>{idx})?TT:NIL;"));
            params.push_str(&e.parameter_binding(s, &c));
        }
    }
    if l.rest.is_some() {
        let fixed = l.required.len() + l.optional.len();
        params.push_str(&format!(
            "em_obj rest=(n>{fixed})?LISTN(args+{fixed},n-{fixed}):NIL;"
        ));
        params.push_str(&e.parameter_binding(l.rest.unwrap(), "rest"));
    }

    // Fully expand each body form so `cond'/`when'/`dolist' & co.
    // reach core forms before emission.
    let mut expanded = Vec::new();
    for f in &l.body {
        match expand_all(&mut e, f) {
            Some(x) => expanded.push(x),
            None => {
                e.unsupported = true;
                break;
            }
        }
    }
    if e.unsupported {
        if let Some(flow) = e.error { return Err(flow); }
        return Ok(None);
    }
    let code = e.seq(&expanded);
    if let Some(flow) = e.error { return Err(flow); }
    if e.unsupported { return Ok(None); }
    Ok(Some(format!(
        "em_obj {cn}(const void *A_, em_env E, const em_obj *args, long n){{A_t A=(A_t)A_;\
         unsigned long parameter_depth=A->binding_mark(E);{params}\
         em_obj native_result={code};UNBIND(parameter_depth);return native_result;}}"
    )))
}

/// Macroexpand each top-level form; None when expansion signals.
fn expand_all(e: &mut Emitter, f: &Value) -> Option<Value> {
    let mut cur = f.clone();
    // Chase macroexpansions to a fixed point (macroexpand is
    // idempotent on non-macro forms).
    for _ in 0..32 {
        if !matches!(cur, Value::Cons(_)) {
            return Some(cur);
        }
        let mid = e.i.q("macroexpand");
        let call = Value::list(vec![
            Value::Sym(mid),
            Value::list(vec![Value::Sym(sym::QUOTE), cur.clone()]),
        ]);
        match e.i.eval(&call) {
            Ok(v) => {
                if same_form(&v, &cur) {
                    return Some(cur);
                }
                cur = v;
            }
            Err(flow) => { e.error = Some(flow); return None; }
        }
    }
    Some(cur)
}

fn same_form(a: &Value, b: &Value) -> bool {
    // `eq'-level check is enough: macroexpand returns the same cons
    // for non-macro forms.
    match (a, b) {
        (Value::Cons(x), Value::Cons(y)) => Rc::ptr_eq(x, y),
        _ => false,
    }
}

/// Eval trampoline: the native subr rebuilds the original closure
/// via `make-interpreted-closure' and calls it.
fn emit_fallback(
    i: &mut Interp,
    _name: &crate::lisp::text::EmacsText,
    l: &Rc<crate::lisp::value::Lambda>,
    cn: &str,
) -> String {
    // A nil environment means dynamic scope; lexenv_as_value's empty
    // display convention is (t), which would turn it into lexical scope.
    let env_v = if l.env.is_some() {
        crate::lisp::eval::lexenv_as_value(&l.env)
    } else {
        Value::Nil
    };
    let env_s = i.prin1_to_string(&env_v);
    let arglist = l
        .arglist
        .clone()
        .unwrap_or_else(|| Value::list(arglist_syms(i, l)));
    let args_s = i.prin1_to_string(&arglist);
    let body_s = i.prin1_to_string(&Value::list(l.body.clone()));
    let mut form = crate::lisp::text::EmacsText::from("(make-interpreted-closure (quote ");
    form.push_str(&args_s);
    form.push_str(") (quote ");
    form.push_str(&body_s);
    form.push_str(") (quote ");
    form.push_str(&env_s);
    form.push_str("))");
    format!(
        "em_obj {cn}(const void *A_, em_env E, const em_obj *args, long n){{\
           A_t A=(A_t)A_;\
           em_obj cl=EVAL(KDC({form}));\
           return (A->call_lambda)(E,cl,args,n);}}",
        form = c_text(&form)
    )
}

/// Emit a whole unit (several functions) to C source text.
fn emit_unit(i: &mut Interp, fns: &[(crate::lisp::text::EmacsText, Rc<crate::lisp::value::Lambda>)]) -> Result<String, Flow> {
    let mut out = String::from(C_PRELUDE);
    let mut inits = String::from(
        "\nint remacs_eln_init_v2(const void *api_, em_env E, long U){A_t A=(A_t)api_;(void)A;int rc=0;",
    );
    for (k, (name, l)) in fns.iter().enumerate() {
        let (entry, desc) = emit_fn(i, name, l, k, false)?;
        out.push_str(&entry);
        out.push_str(&desc);
        let cn = c_name(name, k);
        inits.push_str(&format!("rc|=A->defsubr(A,E,U,&D{cn});"));
    }
    inits.push_str("return rc;}\n");
    out.push_str(&inits);
    Ok(out)
}

// ---------------------------------------------------------------------------
// C compiler driver + .eln naming
// ---------------------------------------------------------------------------

fn cc() -> Option<String> {
    if let Ok(p) = std::env::var("REMACS_CC") {
        if !p.is_empty() {
            return Some(p);
        }
    }
    for c in ["cc", "clang", "gcc"] {
        if std::process::Command::new(c)
            .arg("--version")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
        {
            return Some(c.into());
        }
    }
    None
}

/// Whether the native compiler is usable at all (one-shot probe).
fn compiler_available() -> bool {
    static OK: Mutex<Option<bool>> = Mutex::new(None);
    *OK.lock().unwrap().get_or_insert_with(|| cc().is_some())
}

fn eln_cache_dir() -> String {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
    let d = format!("{home}/.emacs.d/eln-cache/31.1-remacs-v2");
    let _ = std::fs::create_dir_all(&d);
    d
}

/// `<base>-<md5(truename)>-<md5(contents)>.eln' — GNU's naming.
fn eln_rel_filename<P: crate::lisp::text::TextSource + ?Sized>(i: &mut Interp, path: &P) -> Result<crate::lisp::text::EmacsText, Flow> {
    let path = path.with_text(|text| crate::editor::expand_file_name_str(i, text));
    let native = crate::editor::native_path(i, &path)?;
    if !native.exists() {
        return Err(i.signal_data(sym::FILE_MISSING, vec![Value::string(path)]));
    }
    let true_name = std::fs::canonicalize(&native).map_err(|error| crate::editor::report_file_errno(
        i, "Resolving input file", vec![Value::string(path.clone())], &error))?;
    let data = std::fs::read(&native).map_err(|error| crate::editor::report_file_errno(
        i, "Opening input file", vec![Value::string(path.clone())], &error))?;
    let base = crate::lisp::text::EmacsText::from_os_str(true_name.file_stem().unwrap_or_default());
    // A compressed .el.gz name has a second suffix in its stem.
    let mut result = base.strip_suffix(".el").unwrap_or(&base).to_owned();
    let physical = crate::lisp::text::EmacsText::from_os_str(true_name.as_os_str()).to_external_bytes();
    result.push_str(format!("-{}-{}.eln", md5_hex(&physical), md5_hex(&data)));
    Ok(result)
}

fn cached_eln_filename(relative: &crate::lisp::text::EmacsText) -> crate::lisp::text::EmacsText {
    let mut path = crate::lisp::text::EmacsText::from(eln_cache_dir());
    path.push('/');
    path.push_str(relative);
    path
}

fn md5_hex(data: &[u8]) -> String {
    md5(data)[..4].iter().map(|b| format!("{b:02x}")).collect()
}

fn md5(msg: &[u8]) -> [u8; 16] {
    const S: [u32; 64] = [
        7, 12, 17, 22, 7, 12, 17, 22, 7, 12, 17, 22, 7, 12, 17, 22, 5, 9, 14, 20, 5, 9, 14, 20, 5,
        9, 14, 20, 5, 9, 14, 20, 4, 11, 16, 23, 4, 11, 16, 23, 4, 11, 16, 23, 4, 11, 16, 23, 6, 10,
        15, 21, 6, 10, 15, 21, 6, 10, 15, 21, 6, 10, 15, 21,
    ];
    const K: [u32; 64] = [
        0xd76aa478, 0xe8c7b756, 0x242070db, 0xc1bdceee, 0xf57c0faf, 0x4787c62a, 0xa8304613,
        0xfd469501, 0x698098d8, 0x8b44f7af, 0xffff5bb1, 0x895cd7be, 0x6b901122, 0xfd987193,
        0xa679438e, 0x49b40821, 0xf61e2562, 0xc040b340, 0x265e5a51, 0xe9b6c7aa, 0xd62f105d,
        0x02441453, 0xd8a1e681, 0xe7d3fbc8, 0x21e1cde6, 0xc33707d6, 0xf4d50d87, 0x455a14ed,
        0xa9e3e905, 0xfcefa3f8, 0x676f02d9, 0x8d2a4c8a, 0xfffa3942, 0x8771f681, 0x6d9d6122,
        0xfde5380c, 0xa4beea44, 0x4bdecfa9, 0xf6bb4b60, 0xbebfbc70, 0x289b7ec6, 0xeaa127fa,
        0xd4ef3085, 0x04881d05, 0xd9d4d039, 0xe6db99e5, 0x1fa27cf8, 0xc4ac5665, 0xf4292244,
        0x432aff97, 0xab9423a7, 0xfc93a039, 0x655b59c3, 0x8f0ccc92, 0xffeff47d, 0x85845dd1,
        0x6fa87e4f, 0xfe2ce6e0, 0xa3014314, 0x4e0811a1, 0xf7537e82, 0xbd3af235, 0x2ad7d2bb,
        0xeb86d391,
    ];
    let mut m = msg.to_vec();
    let bitlen = (m.len() as u64) * 8;
    m.push(0x80);
    while m.len() % 64 != 56 {
        m.push(0);
    }
    m.extend_from_slice(&bitlen.to_le_bytes());
    let (mut a0, mut b0, mut c0, mut d0) =
        (0x67452301u32, 0xefcdab89u32, 0x98badcfeu32, 0x10325476u32);
    for chunk in m.chunks_exact(64) {
        let mut w = [0u32; 16];
        for (j, w4) in w.iter_mut().enumerate() {
            *w4 = u32::from_le_bytes([
                chunk[j * 4],
                chunk[j * 4 + 1],
                chunk[j * 4 + 2],
                chunk[j * 4 + 3],
            ]);
        }
        let (mut a, mut b, mut c, mut d) = (a0, b0, c0, d0);
        for i in 0..64 {
            let (mut f, g) = match i {
                0..=15 => ((b & c) | (!b & d), i),
                16..=31 => ((d & b) | (!d & c), (5 * i + 1) % 16),
                32..=47 => (b ^ c ^ d, (3 * i + 5) % 16),
                _ => (c ^ (b | !d), (7 * i) % 16),
            };
            f = f.wrapping_add(a).wrapping_add(K[i]).wrapping_add(w[g]);
            let new = b.wrapping_add(f.rotate_left(S[i]));
            a = d;
            d = c;
            c = b;
            b = new;
        }
        a0 = a0.wrapping_add(a);
        b0 = b0.wrapping_add(b);
        c0 = c0.wrapping_add(c);
        d0 = d0.wrapping_add(d);
    }
    let mut out = [0u8; 16];
    out[..4].copy_from_slice(&a0.to_le_bytes());
    out[4..8].copy_from_slice(&b0.to_le_bytes());
    out[8..12].copy_from_slice(&c0.to_le_bytes());
    out[12..].copy_from_slice(&d0.to_le_bytes());
    out
}

/// Compile C source text to an .eln shared object.
enum NativeBuildError {
    Compiler(String),
    File { operation: &'static str, path: std::path::PathBuf, error: std::io::Error },
}
impl From<String> for NativeBuildError {
    fn from(message: String) -> Self { Self::Compiler(message) }
}

struct NativeSourceFile(std::path::PathBuf);
impl Drop for NativeSourceFile {
    fn drop(&mut self) { let _ = std::fs::remove_file(&self.0); }
}

fn run_cc(c_src: &str, out_path: &std::path::Path) -> Result<(), NativeBuildError> {
    let cc = cc().ok_or_else(|| NativeBuildError::Compiler("No C compiler found (set REMACS_CC)".into()))?;
    // GNU's intermediate source names include the output basename. Preserve its
    // native spelling so an unsupported name fails at the real filesystem path.
    static SERIAL: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let (source, mut file) = loop {
        let mut name = std::ffi::OsString::from("emacs-int-comp-");
        name.push(out_path.file_stem().unwrap_or(std::ffi::OsStr::new("remacs")));
        name.push(format!("-{}-{}.c", std::process::id(), SERIAL.fetch_add(1, std::sync::atomic::Ordering::Relaxed)));
        let path = std::env::temp_dir().join(name);
        let mut options = std::fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)] {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        match options.open(&path) {
            Ok(file) => break (NativeSourceFile(path), file),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(NativeBuildError::File { operation: "Creating file with prefix", path, error }),
        }
    };
    file.write_all(c_src.as_bytes()).map_err(|error| NativeBuildError::File {
        operation: "Writing native compilation source", path: source.0.clone(), error,
    })?;
    drop(file);
    let child = std::process::Command::new(cc)
        .args(["-O1", "-fPIC", "-fexceptions", "-funwind-tables", "-shared", "-Wno-unused", "-x", "c"])
        .arg(&source.0).arg("-o").arg(out_path)
        .spawn().map_err(|error| format!("cc failed to start: {error}"))?;
    let status = child.wait_with_output().map_err(|error| format!("cc wait: {error}"))?;
    if status.status.success() { Ok(()) }
    else { Err(NativeBuildError::Compiler(format!("cc exited {}: {}", status.status, String::from_utf8_lossy(&status.stderr)))) }
}

// ---------------------------------------------------------------------------
// High-level compile entry
// ---------------------------------------------------------------------------

/// A file is an ordered program, not just a collection of definitions.
/// Ordinary forms are serialized into the unit, without running them during
/// compilation. Macro expansion is the only compile-time evaluation here.
struct FileEmitter {
    code: String,
    init: String,
    serial: usize,
    macro_env: Value,
    saved_macros: Vec<(SymId, Value)>,
    has_declarations: bool,
}

impl FileEmitter {
    fn form(&mut self, i: &mut Interp, form: &Value) -> Result<(), Flow> {
        let parts = form.list_to_vec().unwrap_or_default();
        let head = parts.first().and_then(|v| match v {
            Value::Sym(id) => Some(i.symbol_name(*id)),
            _ => None,
        });
        match head.as_deref() {
            Some("progn") => {
                for form in &parts[1..] {
                    self.form(i, form)?;
                }
                return Ok(());
            }
            Some("defun" | "defmacro") if parts.len() >= 3 => {
                if let Value::Sym(id) = parts[1] {
                    let is_macro = head.as_deref() == Some("defmacro");
                    let mut body = Vec::new();
                    for form in &parts[3..] {
                        body.push(self.expand(i, form)?);
                    }
                    let mut lambda = form_to_lambda(i, &parts[2], &body, is_macro);
                    lambda.env = i.lexenv.clone();
                    let lambda = Rc::new(lambda);
                    crate::lisp::memory::allocated(&Value::Lambda(lambda.clone()));
                    let name = i.symbol_name_text(id);
                    let (entry, desc) =
                        emit_fn(i, &name, &lambda, self.serial, self.has_declarations)?;
                    self.code.push_str(&entry);
                    self.code.push_str(&desc);
                    let cn = c_name(&name, self.serial);
                    self.init
                        .push_str(&format!("if(A->defsubr(A,E,U,&D{cn}))return 1;"));
                    self.serial += 1;
                    if is_macro {
                        // Compilation sees local macro definitions, but must
                        // leave the caller's function cells as it found them.
                        self.saved_macros.push((id, i.symbol_function(id)));
                        i.fset(id, Value::Lambda(lambda));
                    }
                    return Ok(());
                }
            }
            Some("defvar" | "defconst") => {
                // These declarations affect native binding decisions even
                // in dynamic files, without mutating the compiler's globals.
                self.has_declarations = true;
                if let (Some(Value::Sym(id)), Some(env)) = (parts.get(1), &i.lexenv) {
                    // A compile-time declaration does not initialize the
                    // variable or mark it globally special.
                    crate::lisp::eval::lexenv_declare(&Some(env.clone()), *id);
                }
            }
            _ => {}
        }
        // Expand only the head first, so a macro-generated top-level progn
        // or defun is handled just like its literal counterpart.
        let expand = i.q("macroexpand");
        let expanded = i.apply(
            &Value::Sym(expand),
            vec![form.clone(), self.macro_env.clone()],
        )?;
        if !same_form(&expanded, form) && matches!(form, Value::Cons(_)) {
            return self.form(i, &expanded);
        }
        let expanded = self.expand(i, &expanded)?;
        let literal = c_text(&i.prin1_to_string(&expanded));
        // CHECK returns zero for native *functions*. Unit init uses zero for
        // success, so check each callback explicitly and return one on error.
        self.init.push_str(&format!(
            "{{em_obj f=A->read(E,{literal},sizeof({literal})-1);if(!f)return 1;if(!A->eval(E,f))return 1;}}"
        ));
        Ok(())
    }

    fn expand(&self, i: &mut Interp, form: &Value) -> EvalResult {
        let expand = i.q("macroexpand-all");
        i.apply(
            &Value::Sym(expand),
            vec![form.clone(), self.macro_env.clone()],
        )
    }
}

fn compile_file_to<P: crate::lisp::text::TextSource + ?Sized, O: crate::lisp::text::TextSource + ?Sized>(i: &mut Interp, path: &P, out: &O) -> Result<bool, Flow> {
    let path = path.with_text(|text| crate::editor::expand_file_name_str(i, text));
    // Read through the shared file/coding path before applying native
    // compilation's normal-mode/safe-local-variable lexical protocol.
    let (form, _) = i.read_from_string(
        r#"(lambda (file)
             (with-temp-buffer
               (insert-file-contents file)
               (let ((buffer-file-name file)
                     (dmm (default-value 'major-mode))
                     (enable-local-variables :safe)
                     (enable-local-eval nil))
                 (unwind-protect
                     (progn
                       (setq-default major-mode 'emacs-lisp-mode)
                       (delay-mode-hooks (normal-mode t)))
                   (setq-default major-mode dmm)))
               (cons (buffer-string)
                     (and (local-variable-p 'lexical-binding) lexical-binding))))"#,
        0,
    )?;
    let function = i.eval(&form)?;
    let loaded = i.apply(&function, vec![Value::string(path)])?;
    let (data, lexical) = match &loaded {
        Value::Cons(cell) => {
            let cell = cell.borrow();
            let Value::Str(data) = &cell.car else { return Err(i.wrong_type_mut("stringp", &cell.car)); };
            (data.clone(), cell.cdr.truthy())
        }
        other => return Err(i.wrong_type_mut("consp", other)),
    };
    let source_multibyte = !i.is_unibyte_str(&data);
    let mark = i.specbind_depth();
    let lb = i.q("lexical-binding");
    i.specbind(lb, Value::from_bool(lexical))?;
    let saved_lexenv = std::mem::replace(
        &mut i.lexenv,
        if lexical {
            crate::lisp::eval::lexenv_root()
        } else {
            None
        },
    );
    // Unlike its interpreted expander, eval-and-compile retains the expanded
    // body for load time after evaluating it once at compile time.
    let result = (|| {
        let (form, _) = i.read_from_string(
            "(lambda (&rest body) (let ((expanded (macroexpand-all (cons 'progn body) macroexpand-all-environment))) (eval expanded lexical-binding) expanded))", 0)?;
        let expander = i.eval(&form)?;
        Ok(Value::list(vec![Value::cons(
            Value::Sym(i.q("eval-and-compile")),
            expander,
        )]))
    })();
    let mut emitter = FileEmitter {
        code: String::from(C_PRELUDE),
        init: format!(
            "\nconst int remacs_eln_lexical={};\nint remacs_eln_init_v2(const void *api_, em_env E, long U){{A_t A=(A_t)api_;",
            lexical as u8
        ),
        serial: 0,
        macro_env: Value::Nil,
        saved_macros: Vec::new(),
        has_declarations: false,
    };
    let result = result.and_then(|env| {
        emitter.macro_env = env;
        // Reader::read distinguishes clean EOF (including trailing comments)
        // from an incomplete form. read_from_string turns both into an
        // end-of-file signal, so swallowing its errors loses malformed tails.
        // Share the characters and keep offsets in characters, not UTF-8 bytes.
        let chars: Rc<Vec<crate::lisp::text::EmacsChar>> = Rc::new(data.borrow().chars().collect());
        let mut off = 0;
        let mut any_form = false;
        loop {
            let mut reader = crate::lisp::reader::Reader::with_chars(i, chars.clone());
            reader.set_position(off);
            reader.source_multibyte = source_multibyte;
            let Some(form) = reader.read()? else { break };
            off = reader.position();
            any_form = true;
            emitter.form(i, &form)?;
        }
        if !any_form {
            return Ok(false);
        }
        emitter.init.push_str("return 0;}\n");
        emitter.code.push_str(&emitter.init);
        compile_source_to(i, &emitter.code, out)?;
        Ok(true)
    });
    for (id, definition) in emitter.saved_macros.into_iter().rev() {
        i.fset(id, definition);
    }
    i.lexenv = saved_lexenv;
    i.unbind_to(mark)?;
    result
}

/// Build a Lambda from a `(defun name ARGLIST BODY…)' tail.
fn form_to_lambda(
    i: &mut Interp,
    arglist: &Value,
    body: &[Value],
    is_macro: bool,
) -> crate::lisp::value::Lambda {
    let mut required = Vec::new();
    let mut optional = Vec::new();
    let mut rest = None;
    let mut mode = 0u8;
    let opt_id = i.q("&optional");
    let rest_id = i.q("&rest");
    for a in arglist.list_to_vec().unwrap_or_default() {
        if let Value::Sym(id) = a {
            if id == opt_id {
                mode = 1;
                continue;
            }
            if id == rest_id {
                mode = 2;
                continue;
            }
            match mode {
                0 => required.push(id),
                1 => optional.push(OptParam {
                    sym: id,
                    default: None,
                    supplied: None,
                }),
                _ => {
                    rest = Some(id);
                    mode = 3;
                }
            }
        } else if mode == 1 {
            // `(var default [supplied-p])' optional entry.
            let parts = a.list_to_vec().unwrap_or_default();
            if let Some(Value::Sym(id)) = parts.first() {
                optional.push(OptParam {
                    sym: *id,
                    default: parts.get(1).cloned(),
                    supplied: parts.get(2).and_then(|v| {
                        if let Value::Sym(s) = v {
                            Some(*s)
                        } else {
                            None
                        }
                    }),
                });
            }
        }
    }
    let mut doc = None;
    let mut interactive = None;
    let mut start = 0;
    if let Some(Value::Str(s)) = body.first() {
        if body.len() > 1 {
            doc = Some(s.borrow().clone());
            start = 1;
        }
    }
    for f in body.iter().skip(start) {
        if let Value::Cons(c) = f {
            let b = c.borrow();
            if let Value::Sym(h) = &b.car {
                let n = i.symbol_name(*h);
                if n == "interactive" {
                    interactive = Some(f.clone());
                    start += 1;
                    continue;
                }
                if n == "declare" {
                    start += 1;
                    continue;
                }
            }
        }
        break;
    }
    crate::lisp::value::Lambda {
        is_macro,
        required,
        optional,
        rest,
        body: body[start..].to_vec(),
        env: None,
        doc,
        interactive,
        name: None,
        bad_arglist: false,
        arglist: Some(arglist.clone()),
        plain: false,
        dumped_doc: false,
        compiled: false,
        advice_link: None,
        bc_items: None,
        bc_closure: false,
        bc_prog: std::cell::RefCell::new(None),
        doc_value: None,
        env_value: None,
    }
}

fn compile_fns_to<O: crate::lisp::text::TextSource + ?Sized>(
    i: &mut Interp,
    fns: &[(crate::lisp::text::EmacsText, Rc<crate::lisp::value::Lambda>)],
    out: &O,
) -> Result<(), Flow> {
    let src = emit_unit(i, fns)?;
    compile_source_to(i, &src, out)
}

fn compile_source_to<O: crate::lisp::text::TextSource + ?Sized>(i: &mut Interp, src: &str, out: &O) -> Result<(), Flow> {
    if let Some(path) = std::env::var_os("REMACS_COMP_DUMP_C") {
        let mut path = path; path.push(".c");
        let _ = std::fs::write(path, src);
    }
    let expanded = out.with_text(|text| crate::editor::expand_file_name_str(i, text));
    let native = crate::editor::native_path(i, &expanded)?;
    run_cc(src, &native).map_err(|error| match error {
        NativeBuildError::Compiler(message) => i.error(message),
        NativeBuildError::File { operation, path, error } => crate::editor::report_file_errno(
            i, operation, vec![Value::string(crate::lisp::text::EmacsText::from_os_str(path.as_os_str()))], &error),
    })
}

// ---------------------------------------------------------------------------
// DEFUNs
// ---------------------------------------------------------------------------

fn f_native_comp_available_p(_i: &mut Interp, _a: crate::lisp::Argv) -> EvalResult {
    Ok(Value::from_bool(compiler_available()))
}

/// `(load "x.eln")' entry: dlopen a native unit file.
pub(crate) fn native_load_file<P: crate::lisp::text::TextSource + ?Sized>(i: &mut Interp, path: &P) -> EvalResult {
    let path = path.with_text(|p| p.to_string());
    match load_eln(i, &path) {
        Ok(idx) => Ok(unit_value(i, idx)),
        Err(LoadError::Open(e)) => Err(i.error(format!("Cannot open {path}: {e}"))),
        Err(LoadError::Lisp(flow)) => Err(flow),
    }
}

fn f_native_elisp_load(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let path = match &a[0] {
        Value::Str(s) => s.borrow().clone(),
        other => return Err(i.wrong_type_mut("stringp", &other)),
    };
    // GNU comp.el `native-elisp-load' checks the file exists before
    // dlopen: a missing file signals `native-lisp-load-failed' with
    // data ("file does not exists" FILE) — sic, GNU's typo — while a
    // dlopen failure carries (FILE dlerror-string).
    if !crate::editor::native_path(i, &path)?.exists() {
        let s = i.q("native-lisp-load-failed");
        return Err(i.signal_data(
            s,
            vec![Value::string("file does not exists"), Value::string(path)],
        ));
    }
    match load_eln(i, &path) {
        Ok(idx) => Ok(unit_value(i, idx)),
        Err(LoadError::Lisp(flow)) => Err(flow),
        Err(LoadError::Open(e)) => {
            let s = i.q("native-lisp-load-failed");
            Err(i.signal_data(s, vec![Value::string(path), Value::string(e)]))
        }
    }
}

fn f_comp_el_to_eln_rel_filename(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let path = match &a[0] {
        Value::Str(s) => s.borrow().clone(),
        other => return Err(i.wrong_type_mut("stringp", &other)),
    };
    match eln_rel_filename(i, &path) {
        Ok(p) => Ok(Value::string(p)),
        Err(e) => Err(e),
    }
}

fn f_comp_el_to_eln_filename(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let path = match &a[0] {
        Value::Str(s) => s.borrow().clone(),
        other => return Err(i.wrong_type_mut("stringp", &other)),
    };
    let rel = match eln_rel_filename(i, &path) {
        Ok(p) => p,
        Err(e) => return Err(e),
    };
    let base = match a.get(1) {
        Some(Value::Str(s)) => s.borrow().clone(),
        _ => format!("{}/", eln_cache_dir()).into(),
    };
    let mut result = base;
    if !result.ends_with('/') { result.push('/'); }
    result.push_str(&rel);
    Ok(Value::string(result))
}

fn f_native_compile(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    if !compiler_available() {
        return Err(i.error("Cannot native-compile: no C compiler available"));
    }
    // GNU `comp--native-compile' rejects anything but a function or a
    // file name before any pass runs.
    if !matches!(&a[0], Value::Str(_)) && !crate::lisp::builtins::data::functionp_val(i, &a[0]) {
        let nce = i.q("native-compiler-error");
        return Err(i.signal_data(
            nce,
            vec![Value::string("Not a function symbol or file"), a[0].clone()],
        ));
    }
    match &a[0] {
        Value::Str(s) => {
            let path = s.borrow().clone();
            // GNU: the first pass's `insert-file-contents' error,
            // re-signaled by `comp--error-add-context' with the input
            // appended.
            let ef = i.q("expand-file-name");
            let abs = i.apply(&Value::Sym(ef), vec![a[0].clone()])?;
            let abs_path = match &abs {
                Value::Str(s) => s.borrow().clone(),
                _ => path.clone(),
            };
            let read_err = match std::fs::metadata(crate::editor::native_path(i, &abs_path)?) {
                Err(_) => Some((
                    sym::FILE_MISSING,
                    "Opening input file",
                    "No such file or directory",
                )),
                Ok(m) if m.is_dir() => {
                    Some((i.q("file-error"), "Read error", "Is a directory"))
                }
                Ok(_) => None,
            };
            if let Some((err, what, why)) = read_err {
                let data = vec![Value::string(what), Value::string(why), abs, a[0].clone()];
                return Err(i.signal_data(err, data));
            }
            let out = match a.get(1) {
                Some(Value::Str(o)) => o.borrow().clone(),
                _ => {
                    let rel = match eln_rel_filename(i, &path) {
                        Ok(r) => r,
                        Err(e) => return Err(e),
                    };
                    cached_eln_filename(&rel)
                }
            };
            if !compile_file_to(i, &path, &out)? {
                return Err(i.error(format!("No defuns to compile in {path}")));
            }
            Ok(Value::string(out))
        }
        Value::Sym(id) => {
            let f = i.symbol_function(*id);
            let name = i.symbol_name_text(*id);
            compile_one(i, &name, &f, a.get(1))
        }
        Value::Lambda(l) => {
            let l = l.clone();
            let name = l.name.clone().unwrap_or_else(|| "anonymous".into());
            compile_one_lambda(i, &name, &l, a.get(1))
        }
        // A `(lambda ...)' FORM — evaluate to a closure and compile.
        Value::Cons(_) => match i.eval(&a[0]) {
            Ok(v) => compile_one(i, "anonymous", &v, a.get(1)),
            Err(f) => Err(f),
        },
        other => Err(i.wrong_type_mut("functionp", &other)),
    }
}

fn compile_one<N: crate::lisp::text::TextSource + ?Sized>(i: &mut Interp, name: &N, f: &Value, out: Option<&Value>) -> EvalResult {
    let name = name.with_text(|text| text.to_owned());
    match f {
        Value::Lambda(l) => {
            let l = l.clone();
            compile_one_lambda(i, &name, &l, out)
        }
        // Already compiled — idempotent, like GNU re-using the unit.
        Value::Subr(s) if is_native(s).is_some() => Ok(f.clone()),
        other => Err(i.wrong_type_mut("functionp", &other)),
    }
}

fn compile_one_lambda(
    i: &mut Interp,
    name: &crate::lisp::text::EmacsText,
    l: &Rc<crate::lisp::value::Lambda>,
    out: Option<&Value>,
) -> EvalResult {
    let out_path = match out {
        Some(Value::Str(s)) => s.borrow().clone(),
        _ => format!(
            "{}/{}-{}.eln",
            eln_cache_dir(),
            sanitize(name),
            unit_serial()
        ).into(),
    };
    let fns = vec![(name.to_string(), l.clone())];
    match compile_fns_to(i, &fns, &out_path) {
        Ok(()) => match load_eln(i, &out_path) {
            Ok(unit) => Ok(native_function_in_unit(unit)
                .unwrap_or_else(|| Value::string(out_path))),
            Err(LoadError::Open(e)) => Err(i.error(e)),
            Err(LoadError::Lisp(flow)) => Err(flow),
        },
        Err(flow) => Err(flow),
    }
}

fn sanitize<N: crate::lisp::text::TextSource + ?Sized>(n: &N) -> String {
    c_name(n, 0)
}

static SERIAL: Mutex<u64> = Mutex::new(0);
fn unit_serial() -> u64 {
    let mut s = SERIAL.lock().unwrap();
    *s += 1;
    *s
}

fn f_native_compile_async(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let files: Vec<String> = match &a[0] {
        Value::Str(s) => vec![(s.borrow().clone()).to_unicode_lossy()],
        Value::Nil => Vec::new(),
        other => other
            .list_to_vec()
            .unwrap_or_default()
            .iter()
            .filter_map(|v| {
                if let Value::Str(s) = v {
                    Some(s.borrow().to_unicode_lossy())
                } else {
                    None
                }
            })
            .collect(),
    };
    let load = a.get(2).map(|v| !v.is_nil()).unwrap_or(false);
    let mut outs = Vec::new();
    for f in files {
        let mut paths = Vec::new();
        if std::path::Path::new(&f).is_dir() {
            let rec = a.get(1).map(|v| !v.is_nil()).unwrap_or(false);
            collect_el_files(&f, rec, &mut paths);
        } else {
            paths.push(f);
        }
        for p in paths {
            let rel = match eln_rel_filename(i, &p) {
                Ok(r) => r,
                Err(e) => return Err(e),
            };
            let out = cached_eln_filename(&rel);
            if !compile_file_to(i, &p, &out)? {
                continue;
            }
            if load {
                load_eln(i, &out).map_err(|e| match e {
                    LoadError::Open(message) => i.error(message),
                    LoadError::Lisp(flow) => flow,
                })?;
            }
            outs.push(Value::string(out));
        }
    }
    Ok(Value::list(outs))
}

fn collect_el_files(dir: &str, rec: bool, out: &mut Vec<String>) {
    if let Ok(rd) = std::fs::read_dir(dir) {
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() && rec {
                collect_el_files(&p.to_string_lossy(), rec, out);
            } else if p.extension().map(|x| x == "el").unwrap_or(false) {
                out.push(p.to_string_lossy().into_owned());
            }
        }
    }
}

fn f_native_comp_function_p(_i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    Ok(Value::from_bool(match &a[0] {
        Value::Subr(s) => is_native(s).is_some(),
        // `compiled' Lambdas stand in for GNU's native-compiled defuns
        // (see `subrp') — macros and user `#[...]' objects stay nil.
        Value::Lambda(l) => l.compiled && !l.is_macro,
        _ => false,
    }))
}

fn f_subr_native_lambda_list(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    match &a[0] {
        Value::Subr(s) => {
            if let Some(k) = is_native(s) {
                Ok(META.with(|m| m.borrow()[k].arglist.clone()))
            } else {
                Ok(Value::t())
            }
        }
        // `compiled' Lambdas stand in for GNU's native-compiled defuns
        // (`subrp' t), but carry no stored native arglist — answer t
        // (opaque) like a non-native subr.  User `#[...]' objects are
        // not `subrp' in GNU and fall through to the type error.
        Value::Lambda(l) if l.compiled && !l.is_macro => Ok(Value::t()),
        other => Err(i.wrong_type_mut("subrp", &other)),
    }
}

fn f_subr_native_comp_unit(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    match &a[0] {
        Value::Subr(s) => {
            if let Some(k) = is_native(s) {
                let u = META.with(|m| m.borrow()[k].unit);
                if u >= 0 {
                    Ok(unit_value(i, u as usize))
                } else {
                    Ok(Value::Nil)
                }
            } else {
                Ok(Value::Nil)
            }
        }
        // `compiled' Lambdas are `subrp' t (native-compiled in GNU) but
        // have no comp-unit of their own here — answer nil.
        Value::Lambda(l) if l.compiled && !l.is_macro => Ok(Value::Nil),
        other => Err(i.wrong_type_mut("subrp", &other)),
    }
}

fn f_native_comp_unit_file(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let idx = unit_index(i, &a[0])?;
    UNITS.with(|u| match u.borrow().get(idx) {
        Some(u) => Ok(Value::string(u.path.clone())),
        None => Ok(Value::Nil),
    })
}

fn f_native_comp_unit_set_file(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let idx = unit_index(i, &a[0])?;
    let p = match &a[1] {
        Value::Str(s) => s.borrow().clone(),
        other => return Err(i.wrong_type_mut("stringp", &other)),
    };
    UNITS.with(|u| {
        if let Some(u) = u.borrow_mut().get_mut(idx) {
            u.path = p;
        }
    });
    Ok(Value::Nil)
}

fn f_comp_libgccjit_version(_i: &mut Interp, _a: crate::lisp::Argv) -> EvalResult {
    // remacs emits C and drives the system compiler — no libgccjit.
    Ok(Value::Nil)
}

/// `comp--subr-signature' (SUBR): GNU takes the subr object itself
/// (`CHECK_SUBR' — a symbol naming a subr is NOT resolved) and answers
/// the "NAME(MIN . MAX)" string, where MAX is `many' or `unevalled'
/// for MANY/UNEVALLED arities.
fn f_comp_subr_signature(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let (name, arity) = match &a[0] {
        Value::Subr(s) => (s.name.to_string(), s.arity),
        // Native-compiled Lisp functions report `subrp' t in GNU;
        // remacs marks dumped definitions `compiled' (see `subrp'), so
        // they get a signature here too.  Macros and `#[...]' objects
        // are not subrs.
        Value::Lambda(l) if l.compiled && !l.is_macro => (
            i.lambda_name(l).map(|s| s.to_unicode_lossy()).unwrap_or_else(|| "anonymous".to_string()),
            signature_lambda_arity(i, l),
        ),
        other => return Err(i.wrong_type_mut("subrp", other)),
    };
    let (min, max) = match arity {
        Arity::Range { min, max } => (min.to_string(), max.to_string()),
        Arity::Many { min } => (min.to_string(), "many".to_string()),
        Arity::Unevalled => (
            crate::lisp::special::special_form_min_args(i.intern(&name)).to_string(),
            "unevalled".to_string(),
        ),
    };
    Ok(Value::string(format!("{name}({min} . {max})")))
}

/// Same argdesc walk as `subr-arity': prefer the packed/list argdesc in
/// `bc_items[0]' for `#[...]' objects, else the Lambda's own arity.
fn signature_lambda_arity(i: &mut Interp, l: &Rc<crate::lisp::value::Lambda>) -> Arity {
    if let Some(items) = &l.bc_items {
        match items.borrow().first() {
            Some(Value::Int(n)) => {
                let req = (*n & 0x7f) as u16;
                let rest = (*n >> 7) & 1 != 0;
                let max = ((*n >> 8) & 0x7f) as u16;
                if rest {
                    Arity::Many { min: req }
                } else {
                    Arity::Range { min: req, max }
                }
            }
            Some(list @ (Value::Cons(_) | Value::Nil)) => {
                let opt = i.q("&optional");
                let rst = i.q("&rest");
                let (mut min, mut max, mut rest, mut mode) = (0u16, 0u16, false, 0);
                let mut cur = list.clone();
                while let Value::Cons(c) = cur {
                    let (hd, tl) = {
                        let b = c.borrow();
                        (b.car.clone(), b.cdr.clone())
                    };
                    match i.sym_id(&hd) {
                        Some(s) if s == opt => mode = 1,
                        Some(s) if s == rst => {
                            rest = true;
                            mode = 2;
                        }
                        Some(_) => {
                            min += (mode == 0) as u16;
                            max += (mode <= 1) as u16;
                        }
                        None => {}
                    }
                    cur = tl;
                }
                if rest {
                    Arity::Many { min }
                } else {
                    Arity::Range { min, max }
                }
            }
            _ => l.arity(),
        }
    } else {
        l.arity()
    }
}

/// Pending ctxt source for `comp--compile-ctxt-to-file0' — our own
/// pipeline doesn't need the ctxt builder calls (comp.el's LIMPLE
/// pass targets GNU's ABI), but the trio stays honest.
fn f_comp_init_ctxt(_i: &mut Interp, _a: crate::lisp::Argv) -> EvalResult {
    CTXT.with(|c| *c.borrow_mut() = Some(String::new()));
    Ok(Value::t())
}

fn f_comp_release_ctxt(_i: &mut Interp, _a: crate::lisp::Argv) -> EvalResult {
    CTXT.with(|c| *c.borrow_mut() = None);
    Ok(Value::Nil)
}

fn f_comp_compile_ctxt_to_file0(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let path = match &a[0] {
        Value::Str(s) => s.borrow().clone(),
        other => return Err(i.wrong_type_mut("stringp", &other)),
    };
    let src = CTXT.with(|c| c.borrow().clone()).unwrap_or_default();
    if src.is_empty() {
        return Err(i.error("Nothing in compilation context to emit"));
    }
    match compile_source_to(i, &src, &path) {
        Ok(()) => Ok(Value::string(path)),
        Err(flow) => Err(flow),
    }
}

/// `comp--install-trampoline' (SUBR-NAME TRAMPOLINE): TRAMPOLINE is an
/// .eln file exporting `comp_trampoline'; install it as SUBR-NAME's
/// definition (advice/late-trampoline machinery).
fn f_comp_install_trampoline(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let name = match &a[0] {
        Value::Sym(id) => i.symbol_name(*id),
        Value::Str(s) => (s.borrow().clone()).to_unicode_lossy(),
        other => return Err(i.wrong_type_mut("symbolp", &other)),
    };
    let file = match &a[1] {
        Value::Str(s) => s.borrow().clone(),
        other => return Err(i.wrong_type_mut("stringp", &other)),
    };
    let lib = match unsafe { libloading::Library::new(crate::editor::native_path(i, &file)?) } {
        Ok(l) => l,
        Err(e) => return Err(i.error(format!("Cannot load trampoline: {e}"))),
    };
    let entry: Option<EmEntry> = unsafe { crate::lisp::dynlib::sym(&lib, b"comp_trampoline\0") };
    let Some(entry) = entry else {
        return Err(i.error("Trampoline lacks comp_trampoline entry"));
    };
    core::mem::forget(lib);
    match install_native_subr(i, -1, &name, 0, u32::MAX, entry, "") {
        Ok(v) => Ok(v),
        Err(()) => Err(i.error("Native subr table full")),
    }
}

/// `comp--register-subr'/`-late-register-subr' (NAME C-NAME MINARG
/// MAXARG TYPE REST COMP-U): resolve C_NAME inside COMP-U's handle
/// and install it as the native subr NAME.
fn f_comp_register_subr(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    register_subr_common(i, &a)
}

fn register_subr_common(i: &mut Interp, a: &[Value]) -> EvalResult {
    let name = match &a[0] {
        Value::Sym(id) => i.symbol_name(*id),
        Value::Str(s) => (s.borrow().clone()).to_unicode_lossy(),
        other => return Err(i.wrong_type_mut("symbolp", &other)),
    };
    let cname = match &a[1] {
        Value::Str(s) => s.borrow().clone(),
        other => return Err(i.wrong_type_mut("stringp", &other)),
    };
    let min = match &a[2] {
        Value::Int(n) => (*n).max(0) as u32,
        other => return Err(i.wrong_type_mut("fixnump", &other)),
    };
    let mut max = match &a[3] {
        Value::Int(n) => (*n).max(0) as u32,
        _ => u32::MAX,
    };
    if let Some(v) = a.get(5) {
        if !v.is_nil() {
            max = u32::MAX;
        }
    }
    let (unit_idx, entry) = match a.get(6) {
        Some(v) => {
            let idx = unit_index(i, v)?;
            let e = UNITS.with(|us| {
                us.borrow().get(idx).and_then(|u| unsafe {
                    u._lib
                        .get::<EmEntry>(format!("{cname}\0").as_bytes())
                        .ok()
                        .map(|s| *s)
                })
            });
            (idx as i64, e)
        }
        None => (-1, None),
    };
    let Some(entry) = entry else {
        return Err(i.error(format!("Cannot resolve {cname} in comp unit")));
    };
    match install_native_subr(i, unit_idx, &name, min, max, entry, "") {
        Ok(v) => Ok(v),
        Err(()) => Err(i.error("Native subr table full")),
    }
}

/// `comp--register-lambda' (RELOC-IDX C-NAME MINARG MAXARG TYPE REST
/// COMP-U): produce the native function object (no fset).
fn f_comp_register_lambda(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let cname = match &a[1] {
        Value::Str(s) => s.borrow().clone(),
        other => return Err(i.wrong_type_mut("stringp", &other)),
    };
    let min = match &a[2] {
        Value::Int(n) => (*n).max(0) as u32,
        other => return Err(i.wrong_type_mut("fixnump", &other)),
    };
    let mut max = match &a[3] {
        Value::Int(n) => (*n).max(0) as u32,
        _ => u32::MAX,
    };
    if let Some(v) = a.get(5) {
        if !v.is_nil() {
            max = u32::MAX;
        }
    }
    let unit_idx = match a.get(6) {
        Some(v) => unit_index(i, v)?,
        None => return Err(i.error("comp--register-lambda needs COMP-U")),
    };
    let entry = UNITS.with(|us| {
        us.borrow().get(unit_idx).and_then(|u| unsafe {
            u._lib
                .get::<EmEntry>(format!("{cname}\0").as_bytes())
                .ok()
                .map(|s| *s)
        })
    });
    let Some(entry) = entry else {
        return Err(i.error(format!("Cannot resolve {cname}")));
    };
    let func = NATIVES.with(|n| {
        let mut n = n.borrow_mut();
        if n.len() >= TRAMPS.len() {
            return Err(i.error("Native subr table full"));
        }
        n.push(entry);
        Ok(TRAMPS[n.len() - 1])
    })?;
    META.with(|m| {
        m.borrow_mut().push(NativeMeta {
            constants: HashMap::new(),
            name: cname.clone(),
            documentation: crate::lisp::text::EmacsText::default(),
            subr: None,
            unit: unit_idx as i64,
            arglist: Value::Nil,
            iform: Value::Nil,
        })
    });
    let s: &'static Subr = Box::leak(Box::new(Subr {
        name: Box::leak(cname.to_unicode_lossy().into_boxed_str()),
        arity: if max == u32::MAX {
            Arity::Many { min: min as u16 }
        } else {
            Arity::Range {
                min: min as u16,
                max: max as u16,
            }
        },
        func,
        doc: "",
    }));
    META.with(|metadata| metadata.borrow_mut().last_mut().unwrap().subr = Some(s));
    Ok(Value::Subr(s))
}

fn f_comp_native_compiler_options_effective_p(
    _i: &mut Interp,
    _a: crate::lisp::Argv,
) -> EvalResult {
    Ok(Value::from_bool(compiler_available()))
}
fn f_comp_native_driver_options_effective_p(_i: &mut Interp, _a: crate::lisp::Argv) -> EvalResult {
    Ok(Value::from_bool(compiler_available()))
}

// ---------------------------------------------------------------------------
// SUBRS table + install
// ---------------------------------------------------------------------------

pub(crate) const SUBRS: &[Subr] = &[
    S!(
        "native-comp-available-p",
        0,
        0,
        f_native_comp_available_p,
        "Return non-nil if native compilation support is built-in."
    ),
    S!(
        "native-elisp-load",
        1,
        2,
        f_native_elisp_load,
        "Load native elisp code FILENAME."
    ),
    S!(
        "comp-el-to-eln-filename",
        1,
        2,
        f_comp_el_to_eln_filename,
        "Return the absolute .eln file name for source FILENAME."
    ),
    S!(
        "comp-el-to-eln-rel-filename",
        1,
        1,
        f_comp_el_to_eln_rel_filename,
        "Return the relative name of the .eln file for FILENAME."
    ),
    // The public `native-compile'/`native-compile-async' cells are
    // rebound to Lisp wrappers after startup loads (see eval.rs) —
    // comp.el's LIMPLE pipeline can't run in remacs, so the loaddefs
    // autoloads must not win.
    S!(
        "comp--remacs-native-compile",
        1,
        2,
        f_native_compile,
        "Compile FUNCTION-OR-FILE into native code."
    ),
    S!(
        "comp--remacs-native-compile-async",
        1,
        4,
        f_native_compile_async,
        "Compile FILES asynchronously (synchronous in remacs)."
    ),
    S!(
        "native-comp-function-p",
        1,
        1,
        f_native_comp_function_p,
        "Return t if OBJECT is a native-compiled Lisp function."
    ),
    S!(
        "module-function-p",
        1,
        1,
        f_module_function_p,
        "Return t if OBJECT is a function provided via the module API.\nremacs has no module support, so this is always nil."
    ),
    S!(
        "subr-native-lambda-list",
        1,
        1,
        f_subr_native_lambda_list,
        "Return the lambda list for a native-compiled function."
    ),
    S!(
        "subr-native-comp-unit",
        1,
        1,
        f_subr_native_comp_unit,
        "Return the native compilation unit."
    ),
    S!(
        "native-comp-unit-file",
        1,
        1,
        f_native_comp_unit_file,
        "Return the file of the native compilation unit."
    ),
    S!(
        "native-comp-unit-set-file",
        2,
        2,
        f_native_comp_unit_set_file,
        "Set the file of the native compilation unit."
    ),
    S!(
        "comp-libgccjit-version",
        0,
        0,
        f_comp_libgccjit_version,
        "Return libgccjit version (nil — remacs uses cc)."
    ),
    S!(
        "comp--subr-signature",
        1,
        1,
        f_comp_subr_signature,
        "Support function for internal ABI hashing."
    ),
    S!(
        "comp--init-ctxt",
        0,
        0,
        f_comp_init_ctxt,
        "Initialize the native compiler context."
    ),
    S!(
        "comp--release-ctxt",
        0,
        0,
        f_comp_release_ctxt,
        "Release the native compiler context."
    ),
    S!(
        "comp--compile-ctxt-to-file0",
        1,
        1,
        f_comp_compile_ctxt_to_file0,
        "Compile the current context as native code to file FILENAME."
    ),
    S!(
        "comp--install-trampoline",
        2,
        2,
        f_comp_install_trampoline,
        "Install a TRAMPOLINE for primitive SUBR-NAME."
    ),
    S!(
        "comp--register-subr",
        7,
        7,
        f_comp_register_subr,
        "Register exported subr (called by .eln load code)."
    ),
    S!(
        "comp--late-register-subr",
        7,
        7,
        f_comp_register_subr,
        "Register exported subr at late load phase."
    ),
    S!(
        "comp--register-lambda",
        7,
        7,
        f_comp_register_lambda,
        "Register anonymous lambda from a comp unit."
    ),
    S!(
        "comp-native-compiler-options-effective-p",
        0,
        0,
        f_comp_native_compiler_options_effective_p,
        "Return t if `comp-native-compiler-options' is effective."
    ),
    S!(
        "comp-native-driver-options-effective-p",
        0,
        0,
        f_comp_native_driver_options_effective_p,
        "Return t if `comp-native-driver-options' is effective."
    ),
];

pub(crate) fn install(i: &mut Interp) {
    // GNU advertises the compiled-in backend as `native-compile'.
    // Runtime compiler usability is a separate native-comp-available-p
    // probe; there is no additional `native-comp' feature.
    let f = i.q("native-compile");
    if !i.features.contains(&f) {
        i.features.push(f);
    }
}

/// `module-function-p' — remacs has no module support; always nil.
fn f_module_function_p(_i: &mut Interp, _a: crate::lisp::Argv) -> EvalResult {
    Ok(Value::Nil)
}

// ---------------------------------------------------------------------------
// Trampoline pool — generated 0..255
// ---------------------------------------------------------------------------

tramps!(t0 => 0, t1 => 1, t2 => 2, t3 => 3, t4 => 4, t5 => 5, t6 => 6, t7 => 7, t8 => 8, t9 => 9, t10 => 10, t11 => 11, t12 => 12, t13 => 13, t14 => 14, t15 => 15, t16 => 16, t17 => 17, t18 => 18, t19 => 19, t20 => 20, t21 => 21, t22 => 22, t23 => 23, t24 => 24, t25 => 25, t26 => 26, t27 => 27, t28 => 28, t29 => 29, t30 => 30, t31 => 31, t32 => 32, t33 => 33, t34 => 34, t35 => 35, t36 => 36, t37 => 37, t38 => 38, t39 => 39, t40 => 40, t41 => 41, t42 => 42, t43 => 43, t44 => 44, t45 => 45, t46 => 46, t47 => 47, t48 => 48, t49 => 49, t50 => 50, t51 => 51, t52 => 52, t53 => 53, t54 => 54, t55 => 55, t56 => 56, t57 => 57, t58 => 58, t59 => 59, t60 => 60, t61 => 61, t62 => 62, t63 => 63, t64 => 64, t65 => 65, t66 => 66, t67 => 67, t68 => 68, t69 => 69, t70 => 70, t71 => 71, t72 => 72, t73 => 73, t74 => 74, t75 => 75, t76 => 76, t77 => 77, t78 => 78, t79 => 79, t80 => 80, t81 => 81, t82 => 82, t83 => 83, t84 => 84, t85 => 85, t86 => 86, t87 => 87, t88 => 88, t89 => 89, t90 => 90, t91 => 91, t92 => 92, t93 => 93, t94 => 94, t95 => 95, t96 => 96, t97 => 97, t98 => 98, t99 => 99, t100 => 100, t101 => 101, t102 => 102, t103 => 103, t104 => 104, t105 => 105, t106 => 106, t107 => 107, t108 => 108, t109 => 109, t110 => 110, t111 => 111, t112 => 112, t113 => 113, t114 => 114, t115 => 115, t116 => 116, t117 => 117, t118 => 118, t119 => 119, t120 => 120, t121 => 121, t122 => 122, t123 => 123, t124 => 124, t125 => 125, t126 => 126, t127 => 127, t128 => 128, t129 => 129, t130 => 130, t131 => 131, t132 => 132, t133 => 133, t134 => 134, t135 => 135, t136 => 136, t137 => 137, t138 => 138, t139 => 139, t140 => 140, t141 => 141, t142 => 142, t143 => 143, t144 => 144, t145 => 145, t146 => 146, t147 => 147, t148 => 148, t149 => 149, t150 => 150, t151 => 151, t152 => 152, t153 => 153, t154 => 154, t155 => 155, t156 => 156, t157 => 157, t158 => 158, t159 => 159, t160 => 160, t161 => 161, t162 => 162, t163 => 163, t164 => 164, t165 => 165, t166 => 166, t167 => 167, t168 => 168, t169 => 169, t170 => 170, t171 => 171, t172 => 172, t173 => 173, t174 => 174, t175 => 175, t176 => 176, t177 => 177, t178 => 178, t179 => 179, t180 => 180, t181 => 181, t182 => 182, t183 => 183, t184 => 184, t185 => 185, t186 => 186, t187 => 187, t188 => 188, t189 => 189, t190 => 190, t191 => 191, t192 => 192, t193 => 193, t194 => 194, t195 => 195, t196 => 196, t197 => 197, t198 => 198, t199 => 199, t200 => 200, t201 => 201, t202 => 202, t203 => 203, t204 => 204, t205 => 205, t206 => 206, t207 => 207, t208 => 208, t209 => 209, t210 => 210, t211 => 211, t212 => 212, t213 => 213, t214 => 214, t215 => 215, t216 => 216, t217 => 217, t218 => 218, t219 => 219, t220 => 220, t221 => 221, t222 => 222, t223 => 223, t224 => 224, t225 => 225, t226 => 226, t227 => 227, t228 => 228, t229 => 229, t230 => 230, t231 => 231, t232 => 232, t233 => 233, t234 => 234, t235 => 235, t236 => 236, t237 => 237, t238 => 238, t239 => 239, t240 => 240, t241 => 241, t242 => 242, t243 => 243, t244 => 244, t245 => 245, t246 => 246, t247 => 247, t248 => 248, t249 => 249, t250 => 250, t251 => 251, t252 => 252, t253 => 253, t254 => 254, t255 => 255, t256 => 256, t257 => 257, t258 => 258, t259 => 259, t260 => 260, t261 => 261, t262 => 262, t263 => 263, t264 => 264, t265 => 265, t266 => 266, t267 => 267, t268 => 268, t269 => 269, t270 => 270, t271 => 271, t272 => 272, t273 => 273, t274 => 274, t275 => 275, t276 => 276, t277 => 277, t278 => 278, t279 => 279, t280 => 280, t281 => 281, t282 => 282, t283 => 283, t284 => 284, t285 => 285, t286 => 286, t287 => 287, t288 => 288, t289 => 289, t290 => 290, t291 => 291, t292 => 292, t293 => 293, t294 => 294, t295 => 295, t296 => 296, t297 => 297, t298 => 298, t299 => 299, t300 => 300, t301 => 301, t302 => 302, t303 => 303, t304 => 304, t305 => 305, t306 => 306, t307 => 307, t308 => 308, t309 => 309, t310 => 310, t311 => 311, t312 => 312, t313 => 313, t314 => 314, t315 => 315, t316 => 316, t317 => 317, t318 => 318, t319 => 319, t320 => 320, t321 => 321, t322 => 322, t323 => 323, t324 => 324, t325 => 325, t326 => 326, t327 => 327, t328 => 328, t329 => 329, t330 => 330, t331 => 331, t332 => 332, t333 => 333, t334 => 334, t335 => 335, t336 => 336, t337 => 337, t338 => 338, t339 => 339, t340 => 340, t341 => 341, t342 => 342, t343 => 343, t344 => 344, t345 => 345, t346 => 346, t347 => 347, t348 => 348, t349 => 349, t350 => 350, t351 => 351, t352 => 352, t353 => 353, t354 => 354, t355 => 355, t356 => 356, t357 => 357, t358 => 358, t359 => 359, t360 => 360, t361 => 361, t362 => 362, t363 => 363, t364 => 364, t365 => 365, t366 => 366, t367 => 367, t368 => 368, t369 => 369, t370 => 370, t371 => 371, t372 => 372, t373 => 373, t374 => 374, t375 => 375, t376 => 376, t377 => 377, t378 => 378, t379 => 379, t380 => 380, t381 => 381, t382 => 382, t383 => 383, t384 => 384, t385 => 385, t386 => 386, t387 => 387, t388 => 388, t389 => 389, t390 => 390, t391 => 391, t392 => 392, t393 => 393, t394 => 394, t395 => 395, t396 => 396, t397 => 397, t398 => 398, t399 => 399, t400 => 400, t401 => 401, t402 => 402, t403 => 403, t404 => 404, t405 => 405, t406 => 406, t407 => 407, t408 => 408, t409 => 409, t410 => 410, t411 => 411, t412 => 412, t413 => 413, t414 => 414, t415 => 415, t416 => 416, t417 => 417, t418 => 418, t419 => 419, t420 => 420, t421 => 421, t422 => 422, t423 => 423, t424 => 424, t425 => 425, t426 => 426, t427 => 427, t428 => 428, t429 => 429, t430 => 430, t431 => 431, t432 => 432, t433 => 433, t434 => 434, t435 => 435, t436 => 436, t437 => 437, t438 => 438, t439 => 439, t440 => 440, t441 => 441, t442 => 442, t443 => 443, t444 => 444, t445 => 445, t446 => 446, t447 => 447, t448 => 448, t449 => 449, t450 => 450, t451 => 451, t452 => 452, t453 => 453, t454 => 454, t455 => 455, t456 => 456, t457 => 457, t458 => 458, t459 => 459, t460 => 460, t461 => 461, t462 => 462, t463 => 463, t464 => 464, t465 => 465, t466 => 466, t467 => 467, t468 => 468, t469 => 469, t470 => 470, t471 => 471, t472 => 472, t473 => 473, t474 => 474, t475 => 475, t476 => 476, t477 => 477, t478 => 478, t479 => 479, t480 => 480, t481 => 481, t482 => 482, t483 => 483, t484 => 484, t485 => 485, t486 => 486, t487 => 487, t488 => 488, t489 => 489, t490 => 490, t491 => 491, t492 => 492, t493 => 493, t494 => 494, t495 => 495, t496 => 496, t497 => 497, t498 => 498, t499 => 499, t500 => 500, t501 => 501, t502 => 502, t503 => 503, t504 => 504, t505 => 505, t506 => 506, t507 => 507, t508 => 508, t509 => 509, t510 => 510, t511 => 511, t512 => 512, t513 => 513, t514 => 514, t515 => 515, t516 => 516, t517 => 517, t518 => 518, t519 => 519, t520 => 520, t521 => 521, t522 => 522, t523 => 523, t524 => 524, t525 => 525, t526 => 526, t527 => 527, t528 => 528, t529 => 529, t530 => 530, t531 => 531, t532 => 532, t533 => 533, t534 => 534, t535 => 535, t536 => 536, t537 => 537, t538 => 538, t539 => 539, t540 => 540, t541 => 541, t542 => 542, t543 => 543, t544 => 544, t545 => 545, t546 => 546, t547 => 547, t548 => 548, t549 => 549, t550 => 550, t551 => 551, t552 => 552, t553 => 553, t554 => 554, t555 => 555, t556 => 556, t557 => 557, t558 => 558, t559 => 559, t560 => 560, t561 => 561, t562 => 562, t563 => 563, t564 => 564, t565 => 565, t566 => 566, t567 => 567, t568 => 568, t569 => 569, t570 => 570, t571 => 571, t572 => 572, t573 => 573, t574 => 574, t575 => 575, t576 => 576, t577 => 577, t578 => 578, t579 => 579, t580 => 580, t581 => 581, t582 => 582, t583 => 583, t584 => 584, t585 => 585, t586 => 586, t587 => 587, t588 => 588, t589 => 589, t590 => 590, t591 => 591, t592 => 592, t593 => 593, t594 => 594, t595 => 595, t596 => 596, t597 => 597, t598 => 598, t599 => 599, t600 => 600, t601 => 601, t602 => 602, t603 => 603, t604 => 604, t605 => 605, t606 => 606, t607 => 607, t608 => 608, t609 => 609, t610 => 610, t611 => 611, t612 => 612, t613 => 613, t614 => 614, t615 => 615, t616 => 616, t617 => 617, t618 => 618, t619 => 619, t620 => 620, t621 => 621, t622 => 622, t623 => 623, t624 => 624, t625 => 625, t626 => 626, t627 => 627, t628 => 628, t629 => 629, t630 => 630, t631 => 631, t632 => 632, t633 => 633, t634 => 634, t635 => 635, t636 => 636, t637 => 637, t638 => 638, t639 => 639, t640 => 640, t641 => 641, t642 => 642, t643 => 643, t644 => 644, t645 => 645, t646 => 646, t647 => 647, t648 => 648, t649 => 649, t650 => 650, t651 => 651, t652 => 652, t653 => 653, t654 => 654, t655 => 655, t656 => 656, t657 => 657, t658 => 658, t659 => 659, t660 => 660, t661 => 661, t662 => 662, t663 => 663, t664 => 664, t665 => 665, t666 => 666, t667 => 667, t668 => 668, t669 => 669, t670 => 670, t671 => 671, t672 => 672, t673 => 673, t674 => 674, t675 => 675, t676 => 676, t677 => 677, t678 => 678, t679 => 679, t680 => 680, t681 => 681, t682 => 682, t683 => 683, t684 => 684, t685 => 685, t686 => 686, t687 => 687, t688 => 688, t689 => 689, t690 => 690, t691 => 691, t692 => 692, t693 => 693, t694 => 694, t695 => 695, t696 => 696, t697 => 697, t698 => 698, t699 => 699, t700 => 700, t701 => 701, t702 => 702, t703 => 703, t704 => 704, t705 => 705, t706 => 706, t707 => 707, t708 => 708, t709 => 709, t710 => 710, t711 => 711, t712 => 712, t713 => 713, t714 => 714, t715 => 715, t716 => 716, t717 => 717, t718 => 718, t719 => 719, t720 => 720, t721 => 721, t722 => 722, t723 => 723, t724 => 724, t725 => 725, t726 => 726, t727 => 727, t728 => 728, t729 => 729, t730 => 730, t731 => 731, t732 => 732, t733 => 733, t734 => 734, t735 => 735, t736 => 736, t737 => 737, t738 => 738, t739 => 739, t740 => 740, t741 => 741, t742 => 742, t743 => 743, t744 => 744, t745 => 745, t746 => 746, t747 => 747, t748 => 748, t749 => 749, t750 => 750, t751 => 751, t752 => 752, t753 => 753, t754 => 754, t755 => 755, t756 => 756, t757 => 757, t758 => 758, t759 => 759, t760 => 760, t761 => 761, t762 => 762, t763 => 763, t764 => 764, t765 => 765, t766 => 766, t767 => 767, t768 => 768, t769 => 769, t770 => 770, t771 => 771, t772 => 772, t773 => 773, t774 => 774, t775 => 775, t776 => 776, t777 => 777, t778 => 778, t779 => 779, t780 => 780, t781 => 781, t782 => 782, t783 => 783, t784 => 784, t785 => 785, t786 => 786, t787 => 787, t788 => 788, t789 => 789, t790 => 790, t791 => 791, t792 => 792, t793 => 793, t794 => 794, t795 => 795, t796 => 796, t797 => 797, t798 => 798, t799 => 799, t800 => 800, t801 => 801, t802 => 802, t803 => 803, t804 => 804, t805 => 805, t806 => 806, t807 => 807, t808 => 808, t809 => 809, t810 => 810, t811 => 811, t812 => 812, t813 => 813, t814 => 814, t815 => 815, t816 => 816, t817 => 817, t818 => 818, t819 => 819, t820 => 820, t821 => 821, t822 => 822, t823 => 823, t824 => 824, t825 => 825, t826 => 826, t827 => 827, t828 => 828, t829 => 829, t830 => 830, t831 => 831, t832 => 832, t833 => 833, t834 => 834, t835 => 835, t836 => 836, t837 => 837, t838 => 838, t839 => 839, t840 => 840, t841 => 841, t842 => 842, t843 => 843, t844 => 844, t845 => 845, t846 => 846, t847 => 847, t848 => 848, t849 => 849, t850 => 850, t851 => 851, t852 => 852, t853 => 853, t854 => 854, t855 => 855, t856 => 856, t857 => 857, t858 => 858, t859 => 859, t860 => 860, t861 => 861, t862 => 862, t863 => 863, t864 => 864, t865 => 865, t866 => 866, t867 => 867, t868 => 868, t869 => 869, t870 => 870, t871 => 871, t872 => 872, t873 => 873, t874 => 874, t875 => 875, t876 => 876, t877 => 877, t878 => 878, t879 => 879, t880 => 880, t881 => 881, t882 => 882, t883 => 883, t884 => 884, t885 => 885, t886 => 886, t887 => 887, t888 => 888, t889 => 889, t890 => 890, t891 => 891, t892 => 892, t893 => 893, t894 => 894, t895 => 895, t896 => 896, t897 => 897, t898 => 898, t899 => 899, t900 => 900, t901 => 901, t902 => 902, t903 => 903, t904 => 904, t905 => 905, t906 => 906, t907 => 907, t908 => 908, t909 => 909, t910 => 910, t911 => 911, t912 => 912, t913 => 913, t914 => 914, t915 => 915, t916 => 916, t917 => 917, t918 => 918, t919 => 919, t920 => 920, t921 => 921, t922 => 922, t923 => 923, t924 => 924, t925 => 925, t926 => 926, t927 => 927, t928 => 928, t929 => 929, t930 => 930, t931 => 931, t932 => 932, t933 => 933, t934 => 934, t935 => 935, t936 => 936, t937 => 937, t938 => 938, t939 => 939, t940 => 940, t941 => 941, t942 => 942, t943 => 943, t944 => 944, t945 => 945, t946 => 946, t947 => 947, t948 => 948, t949 => 949, t950 => 950, t951 => 951, t952 => 952, t953 => 953, t954 => 954, t955 => 955, t956 => 956, t957 => 957, t958 => 958, t959 => 959, t960 => 960, t961 => 961, t962 => 962, t963 => 963, t964 => 964, t965 => 965, t966 => 966, t967 => 967, t968 => 968, t969 => 969, t970 => 970, t971 => 971, t972 => 972, t973 => 973, t974 => 974, t975 => 975, t976 => 976, t977 => 977, t978 => 978, t979 => 979, t980 => 980, t981 => 981, t982 => 982, t983 => 983, t984 => 984, t985 => 985, t986 => 986, t987 => 987, t988 => 988, t989 => 989, t990 => 990, t991 => 991, t992 => 992, t993 => 993, t994 => 994, t995 => 995, t996 => 996, t997 => 997, t998 => 998, t999 => 999, t1000 => 1000, t1001 => 1001, t1002 => 1002, t1003 => 1003, t1004 => 1004, t1005 => 1005, t1006 => 1006, t1007 => 1007, t1008 => 1008, t1009 => 1009, t1010 => 1010, t1011 => 1011, t1012 => 1012, t1013 => 1013, t1014 => 1014, t1015 => 1015, t1016 => 1016, t1017 => 1017, t1018 => 1018, t1019 => 1019, t1020 => 1020, t1021 => 1021, t1022 => 1022, t1023 => 1023, t1024 => 1024, t1025 => 1025, t1026 => 1026, t1027 => 1027, t1028 => 1028, t1029 => 1029, t1030 => 1030, t1031 => 1031, t1032 => 1032, t1033 => 1033, t1034 => 1034, t1035 => 1035, t1036 => 1036, t1037 => 1037, t1038 => 1038, t1039 => 1039, t1040 => 1040, t1041 => 1041, t1042 => 1042, t1043 => 1043, t1044 => 1044, t1045 => 1045, t1046 => 1046, t1047 => 1047, t1048 => 1048, t1049 => 1049, t1050 => 1050, t1051 => 1051, t1052 => 1052, t1053 => 1053, t1054 => 1054, t1055 => 1055, t1056 => 1056, t1057 => 1057, t1058 => 1058, t1059 => 1059, t1060 => 1060, t1061 => 1061, t1062 => 1062, t1063 => 1063, t1064 => 1064, t1065 => 1065, t1066 => 1066, t1067 => 1067, t1068 => 1068, t1069 => 1069, t1070 => 1070, t1071 => 1071, t1072 => 1072, t1073 => 1073, t1074 => 1074, t1075 => 1075, t1076 => 1076, t1077 => 1077, t1078 => 1078, t1079 => 1079, t1080 => 1080, t1081 => 1081, t1082 => 1082, t1083 => 1083, t1084 => 1084, t1085 => 1085, t1086 => 1086, t1087 => 1087, t1088 => 1088, t1089 => 1089, t1090 => 1090, t1091 => 1091, t1092 => 1092, t1093 => 1093, t1094 => 1094, t1095 => 1095, t1096 => 1096, t1097 => 1097, t1098 => 1098, t1099 => 1099, t1100 => 1100, t1101 => 1101, t1102 => 1102, t1103 => 1103, t1104 => 1104, t1105 => 1105, t1106 => 1106, t1107 => 1107, t1108 => 1108, t1109 => 1109, t1110 => 1110, t1111 => 1111, t1112 => 1112, t1113 => 1113, t1114 => 1114, t1115 => 1115, t1116 => 1116, t1117 => 1117, t1118 => 1118, t1119 => 1119, t1120 => 1120, t1121 => 1121, t1122 => 1122, t1123 => 1123, t1124 => 1124, t1125 => 1125, t1126 => 1126, t1127 => 1127, t1128 => 1128, t1129 => 1129, t1130 => 1130, t1131 => 1131, t1132 => 1132, t1133 => 1133, t1134 => 1134, t1135 => 1135, t1136 => 1136, t1137 => 1137, t1138 => 1138, t1139 => 1139, t1140 => 1140, t1141 => 1141, t1142 => 1142, t1143 => 1143, t1144 => 1144, t1145 => 1145, t1146 => 1146, t1147 => 1147, t1148 => 1148, t1149 => 1149, t1150 => 1150, t1151 => 1151, t1152 => 1152, t1153 => 1153, t1154 => 1154, t1155 => 1155, t1156 => 1156, t1157 => 1157, t1158 => 1158, t1159 => 1159, t1160 => 1160, t1161 => 1161, t1162 => 1162, t1163 => 1163, t1164 => 1164, t1165 => 1165, t1166 => 1166, t1167 => 1167, t1168 => 1168, t1169 => 1169, t1170 => 1170, t1171 => 1171, t1172 => 1172, t1173 => 1173, t1174 => 1174, t1175 => 1175, t1176 => 1176, t1177 => 1177, t1178 => 1178, t1179 => 1179, t1180 => 1180, t1181 => 1181, t1182 => 1182, t1183 => 1183, t1184 => 1184, t1185 => 1185, t1186 => 1186, t1187 => 1187, t1188 => 1188, t1189 => 1189, t1190 => 1190, t1191 => 1191, t1192 => 1192, t1193 => 1193, t1194 => 1194, t1195 => 1195, t1196 => 1196, t1197 => 1197, t1198 => 1198, t1199 => 1199, t1200 => 1200, t1201 => 1201, t1202 => 1202, t1203 => 1203, t1204 => 1204, t1205 => 1205, t1206 => 1206, t1207 => 1207, t1208 => 1208, t1209 => 1209, t1210 => 1210, t1211 => 1211, t1212 => 1212, t1213 => 1213, t1214 => 1214, t1215 => 1215, t1216 => 1216, t1217 => 1217, t1218 => 1218, t1219 => 1219, t1220 => 1220, t1221 => 1221, t1222 => 1222, t1223 => 1223, t1224 => 1224, t1225 => 1225, t1226 => 1226, t1227 => 1227, t1228 => 1228, t1229 => 1229, t1230 => 1230, t1231 => 1231, t1232 => 1232, t1233 => 1233, t1234 => 1234, t1235 => 1235, t1236 => 1236, t1237 => 1237, t1238 => 1238, t1239 => 1239, t1240 => 1240, t1241 => 1241, t1242 => 1242, t1243 => 1243, t1244 => 1244, t1245 => 1245, t1246 => 1246, t1247 => 1247, t1248 => 1248, t1249 => 1249, t1250 => 1250, t1251 => 1251, t1252 => 1252, t1253 => 1253, t1254 => 1254, t1255 => 1255, t1256 => 1256, t1257 => 1257, t1258 => 1258, t1259 => 1259, t1260 => 1260, t1261 => 1261, t1262 => 1262, t1263 => 1263, t1264 => 1264, t1265 => 1265, t1266 => 1266, t1267 => 1267, t1268 => 1268, t1269 => 1269, t1270 => 1270, t1271 => 1271, t1272 => 1272, t1273 => 1273, t1274 => 1274, t1275 => 1275, t1276 => 1276, t1277 => 1277, t1278 => 1278, t1279 => 1279, t1280 => 1280, t1281 => 1281, t1282 => 1282, t1283 => 1283, t1284 => 1284, t1285 => 1285, t1286 => 1286, t1287 => 1287, t1288 => 1288, t1289 => 1289, t1290 => 1290, t1291 => 1291, t1292 => 1292, t1293 => 1293, t1294 => 1294, t1295 => 1295, t1296 => 1296, t1297 => 1297, t1298 => 1298, t1299 => 1299, t1300 => 1300, t1301 => 1301, t1302 => 1302, t1303 => 1303, t1304 => 1304, t1305 => 1305, t1306 => 1306, t1307 => 1307, t1308 => 1308, t1309 => 1309, t1310 => 1310, t1311 => 1311, t1312 => 1312, t1313 => 1313, t1314 => 1314, t1315 => 1315, t1316 => 1316, t1317 => 1317, t1318 => 1318, t1319 => 1319, t1320 => 1320, t1321 => 1321, t1322 => 1322, t1323 => 1323, t1324 => 1324, t1325 => 1325, t1326 => 1326, t1327 => 1327, t1328 => 1328, t1329 => 1329, t1330 => 1330, t1331 => 1331, t1332 => 1332, t1333 => 1333, t1334 => 1334, t1335 => 1335, t1336 => 1336, t1337 => 1337, t1338 => 1338, t1339 => 1339, t1340 => 1340, t1341 => 1341, t1342 => 1342, t1343 => 1343, t1344 => 1344, t1345 => 1345, t1346 => 1346, t1347 => 1347, t1348 => 1348, t1349 => 1349, t1350 => 1350, t1351 => 1351, t1352 => 1352, t1353 => 1353, t1354 => 1354, t1355 => 1355, t1356 => 1356, t1357 => 1357, t1358 => 1358, t1359 => 1359, t1360 => 1360, t1361 => 1361, t1362 => 1362, t1363 => 1363, t1364 => 1364, t1365 => 1365, t1366 => 1366, t1367 => 1367, t1368 => 1368, t1369 => 1369, t1370 => 1370, t1371 => 1371, t1372 => 1372, t1373 => 1373, t1374 => 1374, t1375 => 1375, t1376 => 1376, t1377 => 1377, t1378 => 1378, t1379 => 1379, t1380 => 1380, t1381 => 1381, t1382 => 1382, t1383 => 1383, t1384 => 1384, t1385 => 1385, t1386 => 1386, t1387 => 1387, t1388 => 1388, t1389 => 1389, t1390 => 1390, t1391 => 1391, t1392 => 1392, t1393 => 1393, t1394 => 1394, t1395 => 1395, t1396 => 1396, t1397 => 1397, t1398 => 1398, t1399 => 1399, t1400 => 1400, t1401 => 1401, t1402 => 1402, t1403 => 1403, t1404 => 1404, t1405 => 1405, t1406 => 1406, t1407 => 1407, t1408 => 1408, t1409 => 1409, t1410 => 1410, t1411 => 1411, t1412 => 1412, t1413 => 1413, t1414 => 1414, t1415 => 1415, t1416 => 1416, t1417 => 1417, t1418 => 1418, t1419 => 1419, t1420 => 1420, t1421 => 1421, t1422 => 1422, t1423 => 1423, t1424 => 1424, t1425 => 1425, t1426 => 1426, t1427 => 1427, t1428 => 1428, t1429 => 1429, t1430 => 1430, t1431 => 1431, t1432 => 1432, t1433 => 1433, t1434 => 1434, t1435 => 1435, t1436 => 1436, t1437 => 1437, t1438 => 1438, t1439 => 1439, t1440 => 1440, t1441 => 1441, t1442 => 1442, t1443 => 1443, t1444 => 1444, t1445 => 1445, t1446 => 1446, t1447 => 1447, t1448 => 1448, t1449 => 1449, t1450 => 1450, t1451 => 1451, t1452 => 1452, t1453 => 1453, t1454 => 1454, t1455 => 1455, t1456 => 1456, t1457 => 1457, t1458 => 1458, t1459 => 1459, t1460 => 1460, t1461 => 1461, t1462 => 1462, t1463 => 1463, t1464 => 1464, t1465 => 1465, t1466 => 1466, t1467 => 1467, t1468 => 1468, t1469 => 1469, t1470 => 1470, t1471 => 1471, t1472 => 1472, t1473 => 1473, t1474 => 1474, t1475 => 1475, t1476 => 1476, t1477 => 1477, t1478 => 1478, t1479 => 1479, t1480 => 1480, t1481 => 1481, t1482 => 1482, t1483 => 1483, t1484 => 1484, t1485 => 1485, t1486 => 1486, t1487 => 1487, t1488 => 1488, t1489 => 1489, t1490 => 1490, t1491 => 1491, t1492 => 1492, t1493 => 1493, t1494 => 1494, t1495 => 1495, t1496 => 1496, t1497 => 1497, t1498 => 1498, t1499 => 1499, t1500 => 1500, t1501 => 1501, t1502 => 1502, t1503 => 1503, t1504 => 1504, t1505 => 1505, t1506 => 1506, t1507 => 1507, t1508 => 1508, t1509 => 1509, t1510 => 1510, t1511 => 1511, t1512 => 1512, t1513 => 1513, t1514 => 1514, t1515 => 1515, t1516 => 1516, t1517 => 1517, t1518 => 1518, t1519 => 1519, t1520 => 1520, t1521 => 1521, t1522 => 1522, t1523 => 1523, t1524 => 1524, t1525 => 1525, t1526 => 1526, t1527 => 1527, t1528 => 1528, t1529 => 1529, t1530 => 1530, t1531 => 1531, t1532 => 1532, t1533 => 1533, t1534 => 1534, t1535 => 1535, t1536 => 1536, t1537 => 1537, t1538 => 1538, t1539 => 1539, t1540 => 1540, t1541 => 1541, t1542 => 1542, t1543 => 1543, t1544 => 1544, t1545 => 1545, t1546 => 1546, t1547 => 1547, t1548 => 1548, t1549 => 1549, t1550 => 1550, t1551 => 1551, t1552 => 1552, t1553 => 1553, t1554 => 1554, t1555 => 1555, t1556 => 1556, t1557 => 1557, t1558 => 1558, t1559 => 1559, t1560 => 1560, t1561 => 1561, t1562 => 1562, t1563 => 1563, t1564 => 1564, t1565 => 1565, t1566 => 1566, t1567 => 1567, t1568 => 1568, t1569 => 1569, t1570 => 1570, t1571 => 1571, t1572 => 1572, t1573 => 1573, t1574 => 1574, t1575 => 1575, t1576 => 1576, t1577 => 1577, t1578 => 1578, t1579 => 1579, t1580 => 1580, t1581 => 1581, t1582 => 1582, t1583 => 1583, t1584 => 1584, t1585 => 1585, t1586 => 1586, t1587 => 1587, t1588 => 1588, t1589 => 1589, t1590 => 1590, t1591 => 1591, t1592 => 1592, t1593 => 1593, t1594 => 1594, t1595 => 1595, t1596 => 1596, t1597 => 1597, t1598 => 1598, t1599 => 1599, t1600 => 1600, t1601 => 1601, t1602 => 1602, t1603 => 1603, t1604 => 1604, t1605 => 1605, t1606 => 1606, t1607 => 1607, t1608 => 1608, t1609 => 1609, t1610 => 1610, t1611 => 1611, t1612 => 1612, t1613 => 1613, t1614 => 1614, t1615 => 1615, t1616 => 1616, t1617 => 1617, t1618 => 1618, t1619 => 1619, t1620 => 1620, t1621 => 1621, t1622 => 1622, t1623 => 1623, t1624 => 1624, t1625 => 1625, t1626 => 1626, t1627 => 1627, t1628 => 1628, t1629 => 1629, t1630 => 1630, t1631 => 1631, t1632 => 1632, t1633 => 1633, t1634 => 1634, t1635 => 1635, t1636 => 1636, t1637 => 1637, t1638 => 1638, t1639 => 1639, t1640 => 1640, t1641 => 1641, t1642 => 1642, t1643 => 1643, t1644 => 1644, t1645 => 1645, t1646 => 1646, t1647 => 1647, t1648 => 1648, t1649 => 1649, t1650 => 1650, t1651 => 1651, t1652 => 1652, t1653 => 1653, t1654 => 1654, t1655 => 1655, t1656 => 1656, t1657 => 1657, t1658 => 1658, t1659 => 1659, t1660 => 1660, t1661 => 1661, t1662 => 1662, t1663 => 1663, t1664 => 1664, t1665 => 1665, t1666 => 1666, t1667 => 1667, t1668 => 1668, t1669 => 1669, t1670 => 1670, t1671 => 1671, t1672 => 1672, t1673 => 1673, t1674 => 1674, t1675 => 1675, t1676 => 1676, t1677 => 1677, t1678 => 1678, t1679 => 1679, t1680 => 1680, t1681 => 1681, t1682 => 1682, t1683 => 1683, t1684 => 1684, t1685 => 1685, t1686 => 1686, t1687 => 1687, t1688 => 1688, t1689 => 1689, t1690 => 1690, t1691 => 1691, t1692 => 1692, t1693 => 1693, t1694 => 1694, t1695 => 1695, t1696 => 1696, t1697 => 1697, t1698 => 1698, t1699 => 1699, t1700 => 1700, t1701 => 1701, t1702 => 1702, t1703 => 1703, t1704 => 1704, t1705 => 1705, t1706 => 1706, t1707 => 1707, t1708 => 1708, t1709 => 1709, t1710 => 1710, t1711 => 1711, t1712 => 1712, t1713 => 1713, t1714 => 1714, t1715 => 1715, t1716 => 1716, t1717 => 1717, t1718 => 1718, t1719 => 1719, t1720 => 1720, t1721 => 1721, t1722 => 1722, t1723 => 1723, t1724 => 1724, t1725 => 1725, t1726 => 1726, t1727 => 1727, t1728 => 1728, t1729 => 1729, t1730 => 1730, t1731 => 1731, t1732 => 1732, t1733 => 1733, t1734 => 1734, t1735 => 1735, t1736 => 1736, t1737 => 1737, t1738 => 1738, t1739 => 1739, t1740 => 1740, t1741 => 1741, t1742 => 1742, t1743 => 1743, t1744 => 1744, t1745 => 1745, t1746 => 1746, t1747 => 1747, t1748 => 1748, t1749 => 1749, t1750 => 1750, t1751 => 1751, t1752 => 1752, t1753 => 1753, t1754 => 1754, t1755 => 1755, t1756 => 1756, t1757 => 1757, t1758 => 1758, t1759 => 1759, t1760 => 1760, t1761 => 1761, t1762 => 1762, t1763 => 1763, t1764 => 1764, t1765 => 1765, t1766 => 1766, t1767 => 1767, t1768 => 1768, t1769 => 1769, t1770 => 1770, t1771 => 1771, t1772 => 1772, t1773 => 1773, t1774 => 1774, t1775 => 1775, t1776 => 1776, t1777 => 1777, t1778 => 1778, t1779 => 1779, t1780 => 1780, t1781 => 1781, t1782 => 1782, t1783 => 1783, t1784 => 1784, t1785 => 1785, t1786 => 1786, t1787 => 1787, t1788 => 1788, t1789 => 1789, t1790 => 1790, t1791 => 1791, t1792 => 1792, t1793 => 1793, t1794 => 1794, t1795 => 1795, t1796 => 1796, t1797 => 1797, t1798 => 1798, t1799 => 1799, t1800 => 1800, t1801 => 1801, t1802 => 1802, t1803 => 1803, t1804 => 1804, t1805 => 1805, t1806 => 1806, t1807 => 1807, t1808 => 1808, t1809 => 1809, t1810 => 1810, t1811 => 1811, t1812 => 1812, t1813 => 1813, t1814 => 1814, t1815 => 1815, t1816 => 1816, t1817 => 1817, t1818 => 1818, t1819 => 1819, t1820 => 1820, t1821 => 1821, t1822 => 1822, t1823 => 1823, t1824 => 1824, t1825 => 1825, t1826 => 1826, t1827 => 1827, t1828 => 1828, t1829 => 1829, t1830 => 1830, t1831 => 1831, t1832 => 1832, t1833 => 1833, t1834 => 1834, t1835 => 1835, t1836 => 1836, t1837 => 1837, t1838 => 1838, t1839 => 1839, t1840 => 1840, t1841 => 1841, t1842 => 1842, t1843 => 1843, t1844 => 1844, t1845 => 1845, t1846 => 1846, t1847 => 1847, t1848 => 1848, t1849 => 1849, t1850 => 1850, t1851 => 1851, t1852 => 1852, t1853 => 1853, t1854 => 1854, t1855 => 1855, t1856 => 1856, t1857 => 1857, t1858 => 1858, t1859 => 1859, t1860 => 1860, t1861 => 1861, t1862 => 1862, t1863 => 1863, t1864 => 1864, t1865 => 1865, t1866 => 1866, t1867 => 1867, t1868 => 1868, t1869 => 1869, t1870 => 1870, t1871 => 1871, t1872 => 1872, t1873 => 1873, t1874 => 1874, t1875 => 1875, t1876 => 1876, t1877 => 1877, t1878 => 1878, t1879 => 1879, t1880 => 1880, t1881 => 1881, t1882 => 1882, t1883 => 1883, t1884 => 1884, t1885 => 1885, t1886 => 1886, t1887 => 1887, t1888 => 1888, t1889 => 1889, t1890 => 1890, t1891 => 1891, t1892 => 1892, t1893 => 1893, t1894 => 1894, t1895 => 1895, t1896 => 1896, t1897 => 1897, t1898 => 1898, t1899 => 1899, t1900 => 1900, t1901 => 1901, t1902 => 1902, t1903 => 1903, t1904 => 1904, t1905 => 1905, t1906 => 1906, t1907 => 1907, t1908 => 1908, t1909 => 1909, t1910 => 1910, t1911 => 1911, t1912 => 1912, t1913 => 1913, t1914 => 1914, t1915 => 1915, t1916 => 1916, t1917 => 1917, t1918 => 1918, t1919 => 1919, t1920 => 1920, t1921 => 1921, t1922 => 1922, t1923 => 1923, t1924 => 1924, t1925 => 1925, t1926 => 1926, t1927 => 1927, t1928 => 1928, t1929 => 1929, t1930 => 1930, t1931 => 1931, t1932 => 1932, t1933 => 1933, t1934 => 1934, t1935 => 1935, t1936 => 1936, t1937 => 1937, t1938 => 1938, t1939 => 1939, t1940 => 1940, t1941 => 1941, t1942 => 1942, t1943 => 1943, t1944 => 1944, t1945 => 1945, t1946 => 1946, t1947 => 1947, t1948 => 1948, t1949 => 1949, t1950 => 1950, t1951 => 1951, t1952 => 1952, t1953 => 1953, t1954 => 1954, t1955 => 1955, t1956 => 1956, t1957 => 1957, t1958 => 1958, t1959 => 1959, t1960 => 1960, t1961 => 1961, t1962 => 1962, t1963 => 1963, t1964 => 1964, t1965 => 1965, t1966 => 1966, t1967 => 1967, t1968 => 1968, t1969 => 1969, t1970 => 1970, t1971 => 1971, t1972 => 1972, t1973 => 1973, t1974 => 1974, t1975 => 1975, t1976 => 1976, t1977 => 1977, t1978 => 1978, t1979 => 1979, t1980 => 1980, t1981 => 1981, t1982 => 1982, t1983 => 1983, t1984 => 1984, t1985 => 1985, t1986 => 1986, t1987 => 1987, t1988 => 1988, t1989 => 1989, t1990 => 1990, t1991 => 1991, t1992 => 1992, t1993 => 1993, t1994 => 1994, t1995 => 1995, t1996 => 1996, t1997 => 1997, t1998 => 1998, t1999 => 1999, t2000 => 2000, t2001 => 2001, t2002 => 2002, t2003 => 2003, t2004 => 2004, t2005 => 2005, t2006 => 2006, t2007 => 2007, t2008 => 2008, t2009 => 2009, t2010 => 2010, t2011 => 2011, t2012 => 2012, t2013 => 2013, t2014 => 2014, t2015 => 2015, t2016 => 2016, t2017 => 2017, t2018 => 2018, t2019 => 2019, t2020 => 2020, t2021 => 2021, t2022 => 2022, t2023 => 2023, t2024 => 2024, t2025 => 2025, t2026 => 2026, t2027 => 2027, t2028 => 2028, t2029 => 2029, t2030 => 2030, t2031 => 2031, t2032 => 2032, t2033 => 2033, t2034 => 2034, t2035 => 2035, t2036 => 2036, t2037 => 2037, t2038 => 2038, t2039 => 2039, t2040 => 2040, t2041 => 2041, t2042 => 2042, t2043 => 2043, t2044 => 2044, t2045 => 2045, t2046 => 2046, t2047 => 2047, t2048 => 2048, t2049 => 2049, t2050 => 2050, t2051 => 2051, t2052 => 2052, t2053 => 2053, t2054 => 2054, t2055 => 2055, t2056 => 2056, t2057 => 2057, t2058 => 2058, t2059 => 2059, t2060 => 2060, t2061 => 2061, t2062 => 2062, t2063 => 2063, t2064 => 2064, t2065 => 2065, t2066 => 2066, t2067 => 2067, t2068 => 2068, t2069 => 2069, t2070 => 2070, t2071 => 2071, t2072 => 2072, t2073 => 2073, t2074 => 2074, t2075 => 2075, t2076 => 2076, t2077 => 2077, t2078 => 2078, t2079 => 2079, t2080 => 2080, t2081 => 2081, t2082 => 2082, t2083 => 2083, t2084 => 2084, t2085 => 2085, t2086 => 2086, t2087 => 2087, t2088 => 2088, t2089 => 2089, t2090 => 2090, t2091 => 2091, t2092 => 2092, t2093 => 2093, t2094 => 2094, t2095 => 2095, t2096 => 2096, t2097 => 2097, t2098 => 2098, t2099 => 2099, t2100 => 2100, t2101 => 2101, t2102 => 2102, t2103 => 2103, t2104 => 2104, t2105 => 2105, t2106 => 2106, t2107 => 2107, t2108 => 2108, t2109 => 2109, t2110 => 2110, t2111 => 2111, t2112 => 2112, t2113 => 2113, t2114 => 2114, t2115 => 2115, t2116 => 2116, t2117 => 2117, t2118 => 2118, t2119 => 2119, t2120 => 2120, t2121 => 2121, t2122 => 2122, t2123 => 2123, t2124 => 2124, t2125 => 2125, t2126 => 2126, t2127 => 2127, t2128 => 2128, t2129 => 2129, t2130 => 2130, t2131 => 2131, t2132 => 2132, t2133 => 2133, t2134 => 2134, t2135 => 2135, t2136 => 2136, t2137 => 2137, t2138 => 2138, t2139 => 2139, t2140 => 2140, t2141 => 2141, t2142 => 2142, t2143 => 2143, t2144 => 2144, t2145 => 2145, t2146 => 2146, t2147 => 2147, t2148 => 2148, t2149 => 2149, t2150 => 2150, t2151 => 2151, t2152 => 2152, t2153 => 2153, t2154 => 2154, t2155 => 2155, t2156 => 2156, t2157 => 2157, t2158 => 2158, t2159 => 2159, t2160 => 2160, t2161 => 2161, t2162 => 2162, t2163 => 2163, t2164 => 2164, t2165 => 2165, t2166 => 2166, t2167 => 2167, t2168 => 2168, t2169 => 2169, t2170 => 2170, t2171 => 2171, t2172 => 2172, t2173 => 2173, t2174 => 2174, t2175 => 2175, t2176 => 2176, t2177 => 2177, t2178 => 2178, t2179 => 2179, t2180 => 2180, t2181 => 2181, t2182 => 2182, t2183 => 2183, t2184 => 2184, t2185 => 2185, t2186 => 2186, t2187 => 2187, t2188 => 2188, t2189 => 2189, t2190 => 2190, t2191 => 2191, t2192 => 2192, t2193 => 2193, t2194 => 2194, t2195 => 2195, t2196 => 2196, t2197 => 2197, t2198 => 2198, t2199 => 2199, t2200 => 2200, t2201 => 2201, t2202 => 2202, t2203 => 2203, t2204 => 2204, t2205 => 2205, t2206 => 2206, t2207 => 2207, t2208 => 2208, t2209 => 2209, t2210 => 2210, t2211 => 2211, t2212 => 2212, t2213 => 2213, t2214 => 2214, t2215 => 2215, t2216 => 2216, t2217 => 2217, t2218 => 2218, t2219 => 2219, t2220 => 2220, t2221 => 2221, t2222 => 2222, t2223 => 2223, t2224 => 2224, t2225 => 2225, t2226 => 2226, t2227 => 2227, t2228 => 2228, t2229 => 2229, t2230 => 2230, t2231 => 2231, t2232 => 2232, t2233 => 2233, t2234 => 2234, t2235 => 2235, t2236 => 2236, t2237 => 2237, t2238 => 2238, t2239 => 2239, t2240 => 2240, t2241 => 2241, t2242 => 2242, t2243 => 2243, t2244 => 2244, t2245 => 2245, t2246 => 2246, t2247 => 2247, t2248 => 2248, t2249 => 2249, t2250 => 2250, t2251 => 2251, t2252 => 2252, t2253 => 2253, t2254 => 2254, t2255 => 2255, t2256 => 2256, t2257 => 2257, t2258 => 2258, t2259 => 2259, t2260 => 2260, t2261 => 2261, t2262 => 2262, t2263 => 2263, t2264 => 2264, t2265 => 2265, t2266 => 2266, t2267 => 2267, t2268 => 2268, t2269 => 2269, t2270 => 2270, t2271 => 2271, t2272 => 2272, t2273 => 2273, t2274 => 2274, t2275 => 2275, t2276 => 2276, t2277 => 2277, t2278 => 2278, t2279 => 2279, t2280 => 2280, t2281 => 2281, t2282 => 2282, t2283 => 2283, t2284 => 2284, t2285 => 2285, t2286 => 2286, t2287 => 2287, t2288 => 2288, t2289 => 2289, t2290 => 2290, t2291 => 2291, t2292 => 2292, t2293 => 2293, t2294 => 2294, t2295 => 2295, t2296 => 2296, t2297 => 2297, t2298 => 2298, t2299 => 2299, t2300 => 2300, t2301 => 2301, t2302 => 2302, t2303 => 2303, t2304 => 2304, t2305 => 2305, t2306 => 2306, t2307 => 2307, t2308 => 2308, t2309 => 2309, t2310 => 2310, t2311 => 2311, t2312 => 2312, t2313 => 2313, t2314 => 2314, t2315 => 2315, t2316 => 2316, t2317 => 2317, t2318 => 2318, t2319 => 2319, t2320 => 2320, t2321 => 2321, t2322 => 2322, t2323 => 2323, t2324 => 2324, t2325 => 2325, t2326 => 2326, t2327 => 2327, t2328 => 2328, t2329 => 2329, t2330 => 2330, t2331 => 2331, t2332 => 2332, t2333 => 2333, t2334 => 2334, t2335 => 2335, t2336 => 2336, t2337 => 2337, t2338 => 2338, t2339 => 2339, t2340 => 2340, t2341 => 2341, t2342 => 2342, t2343 => 2343, t2344 => 2344, t2345 => 2345, t2346 => 2346, t2347 => 2347, t2348 => 2348, t2349 => 2349, t2350 => 2350, t2351 => 2351, t2352 => 2352, t2353 => 2353, t2354 => 2354, t2355 => 2355, t2356 => 2356, t2357 => 2357, t2358 => 2358, t2359 => 2359, t2360 => 2360, t2361 => 2361, t2362 => 2362, t2363 => 2363, t2364 => 2364, t2365 => 2365, t2366 => 2366, t2367 => 2367, t2368 => 2368, t2369 => 2369, t2370 => 2370, t2371 => 2371, t2372 => 2372, t2373 => 2373, t2374 => 2374, t2375 => 2375, t2376 => 2376, t2377 => 2377, t2378 => 2378, t2379 => 2379, t2380 => 2380, t2381 => 2381, t2382 => 2382, t2383 => 2383, t2384 => 2384, t2385 => 2385, t2386 => 2386, t2387 => 2387, t2388 => 2388, t2389 => 2389, t2390 => 2390, t2391 => 2391, t2392 => 2392, t2393 => 2393, t2394 => 2394, t2395 => 2395, t2396 => 2396, t2397 => 2397, t2398 => 2398, t2399 => 2399, t2400 => 2400, t2401 => 2401, t2402 => 2402, t2403 => 2403, t2404 => 2404, t2405 => 2405, t2406 => 2406, t2407 => 2407, t2408 => 2408, t2409 => 2409, t2410 => 2410, t2411 => 2411, t2412 => 2412, t2413 => 2413, t2414 => 2414, t2415 => 2415, t2416 => 2416, t2417 => 2417, t2418 => 2418, t2419 => 2419, t2420 => 2420, t2421 => 2421, t2422 => 2422, t2423 => 2423, t2424 => 2424, t2425 => 2425, t2426 => 2426, t2427 => 2427, t2428 => 2428, t2429 => 2429, t2430 => 2430, t2431 => 2431, t2432 => 2432, t2433 => 2433, t2434 => 2434, t2435 => 2435, t2436 => 2436, t2437 => 2437, t2438 => 2438, t2439 => 2439, t2440 => 2440, t2441 => 2441, t2442 => 2442, t2443 => 2443, t2444 => 2444, t2445 => 2445, t2446 => 2446, t2447 => 2447, t2448 => 2448, t2449 => 2449, t2450 => 2450, t2451 => 2451, t2452 => 2452, t2453 => 2453, t2454 => 2454, t2455 => 2455, t2456 => 2456, t2457 => 2457, t2458 => 2458, t2459 => 2459, t2460 => 2460, t2461 => 2461, t2462 => 2462, t2463 => 2463, t2464 => 2464, t2465 => 2465, t2466 => 2466, t2467 => 2467, t2468 => 2468, t2469 => 2469, t2470 => 2470, t2471 => 2471, t2472 => 2472, t2473 => 2473, t2474 => 2474, t2475 => 2475, t2476 => 2476, t2477 => 2477, t2478 => 2478, t2479 => 2479, t2480 => 2480, t2481 => 2481, t2482 => 2482, t2483 => 2483, t2484 => 2484, t2485 => 2485, t2486 => 2486, t2487 => 2487, t2488 => 2488, t2489 => 2489, t2490 => 2490, t2491 => 2491, t2492 => 2492, t2493 => 2493, t2494 => 2494, t2495 => 2495, t2496 => 2496, t2497 => 2497, t2498 => 2498, t2499 => 2499, t2500 => 2500, t2501 => 2501, t2502 => 2502, t2503 => 2503, t2504 => 2504, t2505 => 2505, t2506 => 2506, t2507 => 2507, t2508 => 2508, t2509 => 2509, t2510 => 2510, t2511 => 2511, t2512 => 2512, t2513 => 2513, t2514 => 2514, t2515 => 2515, t2516 => 2516, t2517 => 2517, t2518 => 2518, t2519 => 2519, t2520 => 2520, t2521 => 2521, t2522 => 2522, t2523 => 2523, t2524 => 2524, t2525 => 2525, t2526 => 2526, t2527 => 2527, t2528 => 2528, t2529 => 2529, t2530 => 2530, t2531 => 2531, t2532 => 2532, t2533 => 2533, t2534 => 2534, t2535 => 2535, t2536 => 2536, t2537 => 2537, t2538 => 2538, t2539 => 2539, t2540 => 2540, t2541 => 2541, t2542 => 2542, t2543 => 2543, t2544 => 2544, t2545 => 2545, t2546 => 2546, t2547 => 2547, t2548 => 2548, t2549 => 2549, t2550 => 2550, t2551 => 2551, t2552 => 2552, t2553 => 2553, t2554 => 2554, t2555 => 2555, t2556 => 2556, t2557 => 2557, t2558 => 2558, t2559 => 2559, t2560 => 2560, t2561 => 2561, t2562 => 2562, t2563 => 2563, t2564 => 2564, t2565 => 2565, t2566 => 2566, t2567 => 2567, t2568 => 2568, t2569 => 2569, t2570 => 2570, t2571 => 2571, t2572 => 2572, t2573 => 2573, t2574 => 2574, t2575 => 2575, t2576 => 2576, t2577 => 2577, t2578 => 2578, t2579 => 2579, t2580 => 2580, t2581 => 2581, t2582 => 2582, t2583 => 2583, t2584 => 2584, t2585 => 2585, t2586 => 2586, t2587 => 2587, t2588 => 2588, t2589 => 2589, t2590 => 2590, t2591 => 2591, t2592 => 2592, t2593 => 2593, t2594 => 2594, t2595 => 2595, t2596 => 2596, t2597 => 2597, t2598 => 2598, t2599 => 2599, t2600 => 2600, t2601 => 2601, t2602 => 2602, t2603 => 2603, t2604 => 2604, t2605 => 2605, t2606 => 2606, t2607 => 2607, t2608 => 2608, t2609 => 2609, t2610 => 2610, t2611 => 2611, t2612 => 2612, t2613 => 2613, t2614 => 2614, t2615 => 2615, t2616 => 2616, t2617 => 2617, t2618 => 2618, t2619 => 2619, t2620 => 2620, t2621 => 2621, t2622 => 2622, t2623 => 2623, t2624 => 2624, t2625 => 2625, t2626 => 2626, t2627 => 2627, t2628 => 2628, t2629 => 2629, t2630 => 2630, t2631 => 2631, t2632 => 2632, t2633 => 2633, t2634 => 2634, t2635 => 2635, t2636 => 2636, t2637 => 2637, t2638 => 2638, t2639 => 2639, t2640 => 2640, t2641 => 2641, t2642 => 2642, t2643 => 2643, t2644 => 2644, t2645 => 2645, t2646 => 2646, t2647 => 2647, t2648 => 2648, t2649 => 2649, t2650 => 2650, t2651 => 2651, t2652 => 2652, t2653 => 2653, t2654 => 2654, t2655 => 2655, t2656 => 2656, t2657 => 2657, t2658 => 2658, t2659 => 2659, t2660 => 2660, t2661 => 2661, t2662 => 2662, t2663 => 2663, t2664 => 2664, t2665 => 2665, t2666 => 2666, t2667 => 2667, t2668 => 2668, t2669 => 2669, t2670 => 2670, t2671 => 2671, t2672 => 2672, t2673 => 2673, t2674 => 2674, t2675 => 2675, t2676 => 2676, t2677 => 2677, t2678 => 2678, t2679 => 2679, t2680 => 2680, t2681 => 2681, t2682 => 2682, t2683 => 2683, t2684 => 2684, t2685 => 2685, t2686 => 2686, t2687 => 2687, t2688 => 2688, t2689 => 2689, t2690 => 2690, t2691 => 2691, t2692 => 2692, t2693 => 2693, t2694 => 2694, t2695 => 2695, t2696 => 2696, t2697 => 2697, t2698 => 2698, t2699 => 2699, t2700 => 2700, t2701 => 2701, t2702 => 2702, t2703 => 2703, t2704 => 2704, t2705 => 2705, t2706 => 2706, t2707 => 2707, t2708 => 2708, t2709 => 2709, t2710 => 2710, t2711 => 2711, t2712 => 2712, t2713 => 2713, t2714 => 2714, t2715 => 2715, t2716 => 2716, t2717 => 2717, t2718 => 2718, t2719 => 2719, t2720 => 2720, t2721 => 2721, t2722 => 2722, t2723 => 2723, t2724 => 2724, t2725 => 2725, t2726 => 2726, t2727 => 2727, t2728 => 2728, t2729 => 2729, t2730 => 2730, t2731 => 2731, t2732 => 2732, t2733 => 2733, t2734 => 2734, t2735 => 2735, t2736 => 2736, t2737 => 2737, t2738 => 2738, t2739 => 2739, t2740 => 2740, t2741 => 2741, t2742 => 2742, t2743 => 2743, t2744 => 2744, t2745 => 2745, t2746 => 2746, t2747 => 2747, t2748 => 2748, t2749 => 2749, t2750 => 2750, t2751 => 2751, t2752 => 2752, t2753 => 2753, t2754 => 2754, t2755 => 2755, t2756 => 2756, t2757 => 2757, t2758 => 2758, t2759 => 2759, t2760 => 2760, t2761 => 2761, t2762 => 2762, t2763 => 2763, t2764 => 2764, t2765 => 2765, t2766 => 2766, t2767 => 2767, t2768 => 2768, t2769 => 2769, t2770 => 2770, t2771 => 2771, t2772 => 2772, t2773 => 2773, t2774 => 2774, t2775 => 2775, t2776 => 2776, t2777 => 2777, t2778 => 2778, t2779 => 2779, t2780 => 2780, t2781 => 2781, t2782 => 2782, t2783 => 2783, t2784 => 2784, t2785 => 2785, t2786 => 2786, t2787 => 2787, t2788 => 2788, t2789 => 2789, t2790 => 2790, t2791 => 2791, t2792 => 2792, t2793 => 2793, t2794 => 2794, t2795 => 2795, t2796 => 2796, t2797 => 2797, t2798 => 2798, t2799 => 2799, t2800 => 2800, t2801 => 2801, t2802 => 2802, t2803 => 2803, t2804 => 2804, t2805 => 2805, t2806 => 2806, t2807 => 2807, t2808 => 2808, t2809 => 2809, t2810 => 2810, t2811 => 2811, t2812 => 2812, t2813 => 2813, t2814 => 2814, t2815 => 2815, t2816 => 2816, t2817 => 2817, t2818 => 2818, t2819 => 2819, t2820 => 2820, t2821 => 2821, t2822 => 2822, t2823 => 2823, t2824 => 2824, t2825 => 2825, t2826 => 2826, t2827 => 2827, t2828 => 2828, t2829 => 2829, t2830 => 2830, t2831 => 2831, t2832 => 2832, t2833 => 2833, t2834 => 2834, t2835 => 2835, t2836 => 2836, t2837 => 2837, t2838 => 2838, t2839 => 2839, t2840 => 2840, t2841 => 2841, t2842 => 2842, t2843 => 2843, t2844 => 2844, t2845 => 2845, t2846 => 2846, t2847 => 2847, t2848 => 2848, t2849 => 2849, t2850 => 2850, t2851 => 2851, t2852 => 2852, t2853 => 2853, t2854 => 2854, t2855 => 2855, t2856 => 2856, t2857 => 2857, t2858 => 2858, t2859 => 2859, t2860 => 2860, t2861 => 2861, t2862 => 2862, t2863 => 2863, t2864 => 2864, t2865 => 2865, t2866 => 2866, t2867 => 2867, t2868 => 2868, t2869 => 2869, t2870 => 2870, t2871 => 2871, t2872 => 2872, t2873 => 2873, t2874 => 2874, t2875 => 2875, t2876 => 2876, t2877 => 2877, t2878 => 2878, t2879 => 2879, t2880 => 2880, t2881 => 2881, t2882 => 2882, t2883 => 2883, t2884 => 2884, t2885 => 2885, t2886 => 2886, t2887 => 2887, t2888 => 2888, t2889 => 2889, t2890 => 2890, t2891 => 2891, t2892 => 2892, t2893 => 2893, t2894 => 2894, t2895 => 2895, t2896 => 2896, t2897 => 2897, t2898 => 2898, t2899 => 2899, t2900 => 2900, t2901 => 2901, t2902 => 2902, t2903 => 2903, t2904 => 2904, t2905 => 2905, t2906 => 2906, t2907 => 2907, t2908 => 2908, t2909 => 2909, t2910 => 2910, t2911 => 2911, t2912 => 2912, t2913 => 2913, t2914 => 2914, t2915 => 2915, t2916 => 2916, t2917 => 2917, t2918 => 2918, t2919 => 2919, t2920 => 2920, t2921 => 2921, t2922 => 2922, t2923 => 2923, t2924 => 2924, t2925 => 2925, t2926 => 2926, t2927 => 2927, t2928 => 2928, t2929 => 2929, t2930 => 2930, t2931 => 2931, t2932 => 2932, t2933 => 2933, t2934 => 2934, t2935 => 2935, t2936 => 2936, t2937 => 2937, t2938 => 2938, t2939 => 2939, t2940 => 2940, t2941 => 2941, t2942 => 2942, t2943 => 2943, t2944 => 2944, t2945 => 2945, t2946 => 2946, t2947 => 2947, t2948 => 2948, t2949 => 2949, t2950 => 2950, t2951 => 2951, t2952 => 2952, t2953 => 2953, t2954 => 2954, t2955 => 2955, t2956 => 2956, t2957 => 2957, t2958 => 2958, t2959 => 2959, t2960 => 2960, t2961 => 2961, t2962 => 2962, t2963 => 2963, t2964 => 2964, t2965 => 2965, t2966 => 2966, t2967 => 2967, t2968 => 2968, t2969 => 2969, t2970 => 2970, t2971 => 2971, t2972 => 2972, t2973 => 2973, t2974 => 2974, t2975 => 2975, t2976 => 2976, t2977 => 2977, t2978 => 2978, t2979 => 2979, t2980 => 2980, t2981 => 2981, t2982 => 2982, t2983 => 2983, t2984 => 2984, t2985 => 2985, t2986 => 2986, t2987 => 2987, t2988 => 2988, t2989 => 2989, t2990 => 2990, t2991 => 2991, t2992 => 2992, t2993 => 2993, t2994 => 2994, t2995 => 2995, t2996 => 2996, t2997 => 2997, t2998 => 2998, t2999 => 2999, t3000 => 3000, t3001 => 3001, t3002 => 3002, t3003 => 3003, t3004 => 3004, t3005 => 3005, t3006 => 3006, t3007 => 3007, t3008 => 3008, t3009 => 3009, t3010 => 3010, t3011 => 3011, t3012 => 3012, t3013 => 3013, t3014 => 3014, t3015 => 3015, t3016 => 3016, t3017 => 3017, t3018 => 3018, t3019 => 3019, t3020 => 3020, t3021 => 3021, t3022 => 3022, t3023 => 3023, t3024 => 3024, t3025 => 3025, t3026 => 3026, t3027 => 3027, t3028 => 3028, t3029 => 3029, t3030 => 3030, t3031 => 3031, t3032 => 3032, t3033 => 3033, t3034 => 3034, t3035 => 3035, t3036 => 3036, t3037 => 3037, t3038 => 3038, t3039 => 3039, t3040 => 3040, t3041 => 3041, t3042 => 3042, t3043 => 3043, t3044 => 3044, t3045 => 3045, t3046 => 3046, t3047 => 3047, t3048 => 3048, t3049 => 3049, t3050 => 3050, t3051 => 3051, t3052 => 3052, t3053 => 3053, t3054 => 3054, t3055 => 3055, t3056 => 3056, t3057 => 3057, t3058 => 3058, t3059 => 3059, t3060 => 3060, t3061 => 3061, t3062 => 3062, t3063 => 3063, t3064 => 3064, t3065 => 3065, t3066 => 3066, t3067 => 3067, t3068 => 3068, t3069 => 3069, t3070 => 3070, t3071 => 3071, t3072 => 3072, t3073 => 3073, t3074 => 3074, t3075 => 3075, t3076 => 3076, t3077 => 3077, t3078 => 3078, t3079 => 3079, t3080 => 3080, t3081 => 3081, t3082 => 3082, t3083 => 3083, t3084 => 3084, t3085 => 3085, t3086 => 3086, t3087 => 3087, t3088 => 3088, t3089 => 3089, t3090 => 3090, t3091 => 3091, t3092 => 3092, t3093 => 3093, t3094 => 3094, t3095 => 3095, t3096 => 3096, t3097 => 3097, t3098 => 3098, t3099 => 3099, t3100 => 3100, t3101 => 3101, t3102 => 3102, t3103 => 3103, t3104 => 3104, t3105 => 3105, t3106 => 3106, t3107 => 3107, t3108 => 3108, t3109 => 3109, t3110 => 3110, t3111 => 3111, t3112 => 3112, t3113 => 3113, t3114 => 3114, t3115 => 3115, t3116 => 3116, t3117 => 3117, t3118 => 3118, t3119 => 3119, t3120 => 3120, t3121 => 3121, t3122 => 3122, t3123 => 3123, t3124 => 3124, t3125 => 3125, t3126 => 3126, t3127 => 3127, t3128 => 3128, t3129 => 3129, t3130 => 3130, t3131 => 3131, t3132 => 3132, t3133 => 3133, t3134 => 3134, t3135 => 3135, t3136 => 3136, t3137 => 3137, t3138 => 3138, t3139 => 3139, t3140 => 3140, t3141 => 3141, t3142 => 3142, t3143 => 3143, t3144 => 3144, t3145 => 3145, t3146 => 3146, t3147 => 3147, t3148 => 3148, t3149 => 3149, t3150 => 3150, t3151 => 3151, t3152 => 3152, t3153 => 3153, t3154 => 3154, t3155 => 3155, t3156 => 3156, t3157 => 3157, t3158 => 3158, t3159 => 3159, t3160 => 3160, t3161 => 3161, t3162 => 3162, t3163 => 3163, t3164 => 3164, t3165 => 3165, t3166 => 3166, t3167 => 3167, t3168 => 3168, t3169 => 3169, t3170 => 3170, t3171 => 3171, t3172 => 3172, t3173 => 3173, t3174 => 3174, t3175 => 3175, t3176 => 3176, t3177 => 3177, t3178 => 3178, t3179 => 3179, t3180 => 3180, t3181 => 3181, t3182 => 3182, t3183 => 3183, t3184 => 3184, t3185 => 3185, t3186 => 3186, t3187 => 3187, t3188 => 3188, t3189 => 3189, t3190 => 3190, t3191 => 3191, t3192 => 3192, t3193 => 3193, t3194 => 3194, t3195 => 3195, t3196 => 3196, t3197 => 3197, t3198 => 3198, t3199 => 3199, t3200 => 3200, t3201 => 3201, t3202 => 3202, t3203 => 3203, t3204 => 3204, t3205 => 3205, t3206 => 3206, t3207 => 3207, t3208 => 3208, t3209 => 3209, t3210 => 3210, t3211 => 3211, t3212 => 3212, t3213 => 3213, t3214 => 3214, t3215 => 3215, t3216 => 3216, t3217 => 3217, t3218 => 3218, t3219 => 3219, t3220 => 3220, t3221 => 3221, t3222 => 3222, t3223 => 3223, t3224 => 3224, t3225 => 3225, t3226 => 3226, t3227 => 3227, t3228 => 3228, t3229 => 3229, t3230 => 3230, t3231 => 3231, t3232 => 3232, t3233 => 3233, t3234 => 3234, t3235 => 3235, t3236 => 3236, t3237 => 3237, t3238 => 3238, t3239 => 3239, t3240 => 3240, t3241 => 3241, t3242 => 3242, t3243 => 3243, t3244 => 3244, t3245 => 3245, t3246 => 3246, t3247 => 3247, t3248 => 3248, t3249 => 3249, t3250 => 3250, t3251 => 3251, t3252 => 3252, t3253 => 3253, t3254 => 3254, t3255 => 3255, t3256 => 3256, t3257 => 3257, t3258 => 3258, t3259 => 3259, t3260 => 3260, t3261 => 3261, t3262 => 3262, t3263 => 3263, t3264 => 3264, t3265 => 3265, t3266 => 3266, t3267 => 3267, t3268 => 3268, t3269 => 3269, t3270 => 3270, t3271 => 3271, t3272 => 3272, t3273 => 3273, t3274 => 3274, t3275 => 3275, t3276 => 3276, t3277 => 3277, t3278 => 3278, t3279 => 3279, t3280 => 3280, t3281 => 3281, t3282 => 3282, t3283 => 3283, t3284 => 3284, t3285 => 3285, t3286 => 3286, t3287 => 3287, t3288 => 3288, t3289 => 3289, t3290 => 3290, t3291 => 3291, t3292 => 3292, t3293 => 3293, t3294 => 3294, t3295 => 3295, t3296 => 3296, t3297 => 3297, t3298 => 3298, t3299 => 3299, t3300 => 3300, t3301 => 3301, t3302 => 3302, t3303 => 3303, t3304 => 3304, t3305 => 3305, t3306 => 3306, t3307 => 3307, t3308 => 3308, t3309 => 3309, t3310 => 3310, t3311 => 3311, t3312 => 3312, t3313 => 3313, t3314 => 3314, t3315 => 3315, t3316 => 3316, t3317 => 3317, t3318 => 3318, t3319 => 3319, t3320 => 3320, t3321 => 3321, t3322 => 3322, t3323 => 3323, t3324 => 3324, t3325 => 3325, t3326 => 3326, t3327 => 3327, t3328 => 3328, t3329 => 3329, t3330 => 3330, t3331 => 3331, t3332 => 3332, t3333 => 3333, t3334 => 3334, t3335 => 3335, t3336 => 3336, t3337 => 3337, t3338 => 3338, t3339 => 3339, t3340 => 3340, t3341 => 3341, t3342 => 3342, t3343 => 3343, t3344 => 3344, t3345 => 3345, t3346 => 3346, t3347 => 3347, t3348 => 3348, t3349 => 3349, t3350 => 3350, t3351 => 3351, t3352 => 3352, t3353 => 3353, t3354 => 3354, t3355 => 3355, t3356 => 3356, t3357 => 3357, t3358 => 3358, t3359 => 3359, t3360 => 3360, t3361 => 3361, t3362 => 3362, t3363 => 3363, t3364 => 3364, t3365 => 3365, t3366 => 3366, t3367 => 3367, t3368 => 3368, t3369 => 3369, t3370 => 3370, t3371 => 3371, t3372 => 3372, t3373 => 3373, t3374 => 3374, t3375 => 3375, t3376 => 3376, t3377 => 3377, t3378 => 3378, t3379 => 3379, t3380 => 3380, t3381 => 3381, t3382 => 3382, t3383 => 3383, t3384 => 3384, t3385 => 3385, t3386 => 3386, t3387 => 3387, t3388 => 3388, t3389 => 3389, t3390 => 3390, t3391 => 3391, t3392 => 3392, t3393 => 3393, t3394 => 3394, t3395 => 3395, t3396 => 3396, t3397 => 3397, t3398 => 3398, t3399 => 3399, t3400 => 3400, t3401 => 3401, t3402 => 3402, t3403 => 3403, t3404 => 3404, t3405 => 3405, t3406 => 3406, t3407 => 3407, t3408 => 3408, t3409 => 3409, t3410 => 3410, t3411 => 3411, t3412 => 3412, t3413 => 3413, t3414 => 3414, t3415 => 3415, t3416 => 3416, t3417 => 3417, t3418 => 3418, t3419 => 3419, t3420 => 3420, t3421 => 3421, t3422 => 3422, t3423 => 3423, t3424 => 3424, t3425 => 3425, t3426 => 3426, t3427 => 3427, t3428 => 3428, t3429 => 3429, t3430 => 3430, t3431 => 3431, t3432 => 3432, t3433 => 3433, t3434 => 3434, t3435 => 3435, t3436 => 3436, t3437 => 3437, t3438 => 3438, t3439 => 3439, t3440 => 3440, t3441 => 3441, t3442 => 3442, t3443 => 3443, t3444 => 3444, t3445 => 3445, t3446 => 3446, t3447 => 3447, t3448 => 3448, t3449 => 3449, t3450 => 3450, t3451 => 3451, t3452 => 3452, t3453 => 3453, t3454 => 3454, t3455 => 3455, t3456 => 3456, t3457 => 3457, t3458 => 3458, t3459 => 3459, t3460 => 3460, t3461 => 3461, t3462 => 3462, t3463 => 3463, t3464 => 3464, t3465 => 3465, t3466 => 3466, t3467 => 3467, t3468 => 3468, t3469 => 3469, t3470 => 3470, t3471 => 3471, t3472 => 3472, t3473 => 3473, t3474 => 3474, t3475 => 3475, t3476 => 3476, t3477 => 3477, t3478 => 3478, t3479 => 3479, t3480 => 3480, t3481 => 3481, t3482 => 3482, t3483 => 3483, t3484 => 3484, t3485 => 3485, t3486 => 3486, t3487 => 3487, t3488 => 3488, t3489 => 3489, t3490 => 3490, t3491 => 3491, t3492 => 3492, t3493 => 3493, t3494 => 3494, t3495 => 3495, t3496 => 3496, t3497 => 3497, t3498 => 3498, t3499 => 3499, t3500 => 3500, t3501 => 3501, t3502 => 3502, t3503 => 3503, t3504 => 3504, t3505 => 3505, t3506 => 3506, t3507 => 3507, t3508 => 3508, t3509 => 3509, t3510 => 3510, t3511 => 3511, t3512 => 3512, t3513 => 3513, t3514 => 3514, t3515 => 3515, t3516 => 3516, t3517 => 3517, t3518 => 3518, t3519 => 3519, t3520 => 3520, t3521 => 3521, t3522 => 3522, t3523 => 3523, t3524 => 3524, t3525 => 3525, t3526 => 3526, t3527 => 3527, t3528 => 3528, t3529 => 3529, t3530 => 3530, t3531 => 3531, t3532 => 3532, t3533 => 3533, t3534 => 3534, t3535 => 3535, t3536 => 3536, t3537 => 3537, t3538 => 3538, t3539 => 3539, t3540 => 3540, t3541 => 3541, t3542 => 3542, t3543 => 3543, t3544 => 3544, t3545 => 3545, t3546 => 3546, t3547 => 3547, t3548 => 3548, t3549 => 3549, t3550 => 3550, t3551 => 3551, t3552 => 3552, t3553 => 3553, t3554 => 3554, t3555 => 3555, t3556 => 3556, t3557 => 3557, t3558 => 3558, t3559 => 3559, t3560 => 3560, t3561 => 3561, t3562 => 3562, t3563 => 3563, t3564 => 3564, t3565 => 3565, t3566 => 3566, t3567 => 3567, t3568 => 3568, t3569 => 3569, t3570 => 3570, t3571 => 3571, t3572 => 3572, t3573 => 3573, t3574 => 3574, t3575 => 3575, t3576 => 3576, t3577 => 3577, t3578 => 3578, t3579 => 3579, t3580 => 3580, t3581 => 3581, t3582 => 3582, t3583 => 3583, t3584 => 3584, t3585 => 3585, t3586 => 3586, t3587 => 3587, t3588 => 3588, t3589 => 3589, t3590 => 3590, t3591 => 3591, t3592 => 3592, t3593 => 3593, t3594 => 3594, t3595 => 3595, t3596 => 3596, t3597 => 3597, t3598 => 3598, t3599 => 3599, t3600 => 3600, t3601 => 3601, t3602 => 3602, t3603 => 3603, t3604 => 3604, t3605 => 3605, t3606 => 3606, t3607 => 3607, t3608 => 3608, t3609 => 3609, t3610 => 3610, t3611 => 3611, t3612 => 3612, t3613 => 3613, t3614 => 3614, t3615 => 3615, t3616 => 3616, t3617 => 3617, t3618 => 3618, t3619 => 3619, t3620 => 3620, t3621 => 3621, t3622 => 3622, t3623 => 3623, t3624 => 3624, t3625 => 3625, t3626 => 3626, t3627 => 3627, t3628 => 3628, t3629 => 3629, t3630 => 3630, t3631 => 3631, t3632 => 3632, t3633 => 3633, t3634 => 3634, t3635 => 3635, t3636 => 3636, t3637 => 3637, t3638 => 3638, t3639 => 3639, t3640 => 3640, t3641 => 3641, t3642 => 3642, t3643 => 3643, t3644 => 3644, t3645 => 3645, t3646 => 3646, t3647 => 3647, t3648 => 3648, t3649 => 3649, t3650 => 3650, t3651 => 3651, t3652 => 3652, t3653 => 3653, t3654 => 3654, t3655 => 3655, t3656 => 3656, t3657 => 3657, t3658 => 3658, t3659 => 3659, t3660 => 3660, t3661 => 3661, t3662 => 3662, t3663 => 3663, t3664 => 3664, t3665 => 3665, t3666 => 3666, t3667 => 3667, t3668 => 3668, t3669 => 3669, t3670 => 3670, t3671 => 3671, t3672 => 3672, t3673 => 3673, t3674 => 3674, t3675 => 3675, t3676 => 3676, t3677 => 3677, t3678 => 3678, t3679 => 3679, t3680 => 3680, t3681 => 3681, t3682 => 3682, t3683 => 3683, t3684 => 3684, t3685 => 3685, t3686 => 3686, t3687 => 3687, t3688 => 3688, t3689 => 3689, t3690 => 3690, t3691 => 3691, t3692 => 3692, t3693 => 3693, t3694 => 3694, t3695 => 3695, t3696 => 3696, t3697 => 3697, t3698 => 3698, t3699 => 3699, t3700 => 3700, t3701 => 3701, t3702 => 3702, t3703 => 3703, t3704 => 3704, t3705 => 3705, t3706 => 3706, t3707 => 3707, t3708 => 3708, t3709 => 3709, t3710 => 3710, t3711 => 3711, t3712 => 3712, t3713 => 3713, t3714 => 3714, t3715 => 3715, t3716 => 3716, t3717 => 3717, t3718 => 3718, t3719 => 3719, t3720 => 3720, t3721 => 3721, t3722 => 3722, t3723 => 3723, t3724 => 3724, t3725 => 3725, t3726 => 3726, t3727 => 3727, t3728 => 3728, t3729 => 3729, t3730 => 3730, t3731 => 3731, t3732 => 3732, t3733 => 3733, t3734 => 3734, t3735 => 3735, t3736 => 3736, t3737 => 3737, t3738 => 3738, t3739 => 3739, t3740 => 3740, t3741 => 3741, t3742 => 3742, t3743 => 3743, t3744 => 3744, t3745 => 3745, t3746 => 3746, t3747 => 3747, t3748 => 3748, t3749 => 3749, t3750 => 3750, t3751 => 3751, t3752 => 3752, t3753 => 3753, t3754 => 3754, t3755 => 3755, t3756 => 3756, t3757 => 3757, t3758 => 3758, t3759 => 3759, t3760 => 3760, t3761 => 3761, t3762 => 3762, t3763 => 3763, t3764 => 3764, t3765 => 3765, t3766 => 3766, t3767 => 3767, t3768 => 3768, t3769 => 3769, t3770 => 3770, t3771 => 3771, t3772 => 3772, t3773 => 3773, t3774 => 3774, t3775 => 3775, t3776 => 3776, t3777 => 3777, t3778 => 3778, t3779 => 3779, t3780 => 3780, t3781 => 3781, t3782 => 3782, t3783 => 3783, t3784 => 3784, t3785 => 3785, t3786 => 3786, t3787 => 3787, t3788 => 3788, t3789 => 3789, t3790 => 3790, t3791 => 3791, t3792 => 3792, t3793 => 3793, t3794 => 3794, t3795 => 3795, t3796 => 3796, t3797 => 3797, t3798 => 3798, t3799 => 3799, t3800 => 3800, t3801 => 3801, t3802 => 3802, t3803 => 3803, t3804 => 3804, t3805 => 3805, t3806 => 3806, t3807 => 3807, t3808 => 3808, t3809 => 3809, t3810 => 3810, t3811 => 3811, t3812 => 3812, t3813 => 3813, t3814 => 3814, t3815 => 3815, t3816 => 3816, t3817 => 3817, t3818 => 3818, t3819 => 3819, t3820 => 3820, t3821 => 3821, t3822 => 3822, t3823 => 3823, t3824 => 3824, t3825 => 3825, t3826 => 3826, t3827 => 3827, t3828 => 3828, t3829 => 3829, t3830 => 3830, t3831 => 3831, t3832 => 3832, t3833 => 3833, t3834 => 3834, t3835 => 3835, t3836 => 3836, t3837 => 3837, t3838 => 3838, t3839 => 3839, t3840 => 3840, t3841 => 3841, t3842 => 3842, t3843 => 3843, t3844 => 3844, t3845 => 3845, t3846 => 3846, t3847 => 3847, t3848 => 3848, t3849 => 3849, t3850 => 3850, t3851 => 3851, t3852 => 3852, t3853 => 3853, t3854 => 3854, t3855 => 3855, t3856 => 3856, t3857 => 3857, t3858 => 3858, t3859 => 3859, t3860 => 3860, t3861 => 3861, t3862 => 3862, t3863 => 3863, t3864 => 3864, t3865 => 3865, t3866 => 3866, t3867 => 3867, t3868 => 3868, t3869 => 3869, t3870 => 3870, t3871 => 3871, t3872 => 3872, t3873 => 3873, t3874 => 3874, t3875 => 3875, t3876 => 3876, t3877 => 3877, t3878 => 3878, t3879 => 3879, t3880 => 3880, t3881 => 3881, t3882 => 3882, t3883 => 3883, t3884 => 3884, t3885 => 3885, t3886 => 3886, t3887 => 3887, t3888 => 3888, t3889 => 3889, t3890 => 3890, t3891 => 3891, t3892 => 3892, t3893 => 3893, t3894 => 3894, t3895 => 3895, t3896 => 3896, t3897 => 3897, t3898 => 3898, t3899 => 3899, t3900 => 3900, t3901 => 3901, t3902 => 3902, t3903 => 3903, t3904 => 3904, t3905 => 3905, t3906 => 3906, t3907 => 3907, t3908 => 3908, t3909 => 3909, t3910 => 3910, t3911 => 3911, t3912 => 3912, t3913 => 3913, t3914 => 3914, t3915 => 3915, t3916 => 3916, t3917 => 3917, t3918 => 3918, t3919 => 3919, t3920 => 3920, t3921 => 3921, t3922 => 3922, t3923 => 3923, t3924 => 3924, t3925 => 3925, t3926 => 3926, t3927 => 3927, t3928 => 3928, t3929 => 3929, t3930 => 3930, t3931 => 3931, t3932 => 3932, t3933 => 3933, t3934 => 3934, t3935 => 3935, t3936 => 3936, t3937 => 3937, t3938 => 3938, t3939 => 3939, t3940 => 3940, t3941 => 3941, t3942 => 3942, t3943 => 3943, t3944 => 3944, t3945 => 3945, t3946 => 3946, t3947 => 3947, t3948 => 3948, t3949 => 3949, t3950 => 3950, t3951 => 3951, t3952 => 3952, t3953 => 3953, t3954 => 3954, t3955 => 3955, t3956 => 3956, t3957 => 3957, t3958 => 3958, t3959 => 3959, t3960 => 3960, t3961 => 3961, t3962 => 3962, t3963 => 3963, t3964 => 3964, t3965 => 3965, t3966 => 3966, t3967 => 3967, t3968 => 3968, t3969 => 3969, t3970 => 3970, t3971 => 3971, t3972 => 3972, t3973 => 3973, t3974 => 3974, t3975 => 3975, t3976 => 3976, t3977 => 3977, t3978 => 3978, t3979 => 3979, t3980 => 3980, t3981 => 3981, t3982 => 3982, t3983 => 3983, t3984 => 3984, t3985 => 3985, t3986 => 3986, t3987 => 3987, t3988 => 3988, t3989 => 3989, t3990 => 3990, t3991 => 3991, t3992 => 3992, t3993 => 3993, t3994 => 3994, t3995 => 3995, t3996 => 3996, t3997 => 3997, t3998 => 3998, t3999 => 3999, t4000 => 4000, t4001 => 4001, t4002 => 4002, t4003 => 4003, t4004 => 4004, t4005 => 4005, t4006 => 4006, t4007 => 4007, t4008 => 4008, t4009 => 4009, t4010 => 4010, t4011 => 4011, t4012 => 4012, t4013 => 4013, t4014 => 4014, t4015 => 4015, t4016 => 4016, t4017 => 4017, t4018 => 4018, t4019 => 4019, t4020 => 4020, t4021 => 4021, t4022 => 4022, t4023 => 4023, t4024 => 4024, t4025 => 4025, t4026 => 4026, t4027 => 4027, t4028 => 4028, t4029 => 4029, t4030 => 4030, t4031 => 4031, t4032 => 4032, t4033 => 4033, t4034 => 4034, t4035 => 4035, t4036 => 4036, t4037 => 4037, t4038 => 4038, t4039 => 4039, t4040 => 4040, t4041 => 4041, t4042 => 4042, t4043 => 4043, t4044 => 4044, t4045 => 4045, t4046 => 4046, t4047 => 4047, t4048 => 4048, t4049 => 4049, t4050 => 4050, t4051 => 4051, t4052 => 4052, t4053 => 4053, t4054 => 4054, t4055 => 4055, t4056 => 4056, t4057 => 4057, t4058 => 4058, t4059 => 4059, t4060 => 4060, t4061 => 4061, t4062 => 4062, t4063 => 4063, t4064 => 4064, t4065 => 4065, t4066 => 4066, t4067 => 4067, t4068 => 4068, t4069 => 4069, t4070 => 4070, t4071 => 4071, t4072 => 4072, t4073 => 4073, t4074 => 4074, t4075 => 4075, t4076 => 4076, t4077 => 4077, t4078 => 4078, t4079 => 4079, t4080 => 4080, t4081 => 4081, t4082 => 4082, t4083 => 4083, t4084 => 4084, t4085 => 4085, t4086 => 4086, t4087 => 4087, t4088 => 4088, t4089 => 4089, t4090 => 4090, t4091 => 4091, t4092 => 4092, t4093 => 4093, t4094 => 4094, t4095 => 4095, t4096 => 4096, t4097 => 4097, t4098 => 4098, t4099 => 4099, t4100 => 4100, t4101 => 4101, t4102 => 4102, t4103 => 4103, t4104 => 4104, t4105 => 4105, t4106 => 4106, t4107 => 4107, t4108 => 4108, t4109 => 4109, t4110 => 4110, t4111 => 4111, t4112 => 4112, t4113 => 4113, t4114 => 4114, t4115 => 4115, t4116 => 4116, t4117 => 4117, t4118 => 4118, t4119 => 4119, t4120 => 4120, t4121 => 4121, t4122 => 4122, t4123 => 4123, t4124 => 4124, t4125 => 4125, t4126 => 4126, t4127 => 4127, t4128 => 4128, t4129 => 4129, t4130 => 4130, t4131 => 4131, t4132 => 4132, t4133 => 4133, t4134 => 4134, t4135 => 4135, t4136 => 4136, t4137 => 4137, t4138 => 4138, t4139 => 4139, t4140 => 4140, t4141 => 4141, t4142 => 4142, t4143 => 4143, t4144 => 4144, t4145 => 4145, t4146 => 4146, t4147 => 4147, t4148 => 4148, t4149 => 4149, t4150 => 4150, t4151 => 4151, t4152 => 4152, t4153 => 4153, t4154 => 4154, t4155 => 4155, t4156 => 4156, t4157 => 4157, t4158 => 4158, t4159 => 4159, t4160 => 4160, t4161 => 4161, t4162 => 4162, t4163 => 4163, t4164 => 4164, t4165 => 4165, t4166 => 4166, t4167 => 4167, t4168 => 4168, t4169 => 4169, t4170 => 4170, t4171 => 4171, t4172 => 4172, t4173 => 4173, t4174 => 4174, t4175 => 4175, t4176 => 4176, t4177 => 4177, t4178 => 4178, t4179 => 4179, t4180 => 4180, t4181 => 4181, t4182 => 4182, t4183 => 4183, t4184 => 4184, t4185 => 4185, t4186 => 4186, t4187 => 4187, t4188 => 4188, t4189 => 4189, t4190 => 4190, t4191 => 4191, t4192 => 4192, t4193 => 4193, t4194 => 4194, t4195 => 4195, t4196 => 4196, t4197 => 4197, t4198 => 4198, t4199 => 4199, t4200 => 4200, t4201 => 4201, t4202 => 4202, t4203 => 4203, t4204 => 4204, t4205 => 4205, t4206 => 4206, t4207 => 4207, t4208 => 4208, t4209 => 4209, t4210 => 4210, t4211 => 4211, t4212 => 4212, t4213 => 4213, t4214 => 4214, t4215 => 4215, t4216 => 4216, t4217 => 4217, t4218 => 4218, t4219 => 4219, t4220 => 4220, t4221 => 4221, t4222 => 4222, t4223 => 4223, t4224 => 4224, t4225 => 4225, t4226 => 4226, t4227 => 4227, t4228 => 4228, t4229 => 4229, t4230 => 4230, t4231 => 4231, t4232 => 4232, t4233 => 4233, t4234 => 4234, t4235 => 4235, t4236 => 4236, t4237 => 4237, t4238 => 4238, t4239 => 4239, t4240 => 4240, t4241 => 4241, t4242 => 4242, t4243 => 4243, t4244 => 4244, t4245 => 4245, t4246 => 4246, t4247 => 4247, t4248 => 4248, t4249 => 4249, t4250 => 4250, t4251 => 4251, t4252 => 4252, t4253 => 4253, t4254 => 4254, t4255 => 4255, t4256 => 4256, t4257 => 4257, t4258 => 4258, t4259 => 4259, t4260 => 4260, t4261 => 4261, t4262 => 4262, t4263 => 4263, t4264 => 4264, t4265 => 4265, t4266 => 4266, t4267 => 4267, t4268 => 4268, t4269 => 4269, t4270 => 4270, t4271 => 4271, t4272 => 4272, t4273 => 4273, t4274 => 4274, t4275 => 4275, t4276 => 4276, t4277 => 4277, t4278 => 4278, t4279 => 4279, t4280 => 4280, t4281 => 4281, t4282 => 4282, t4283 => 4283, t4284 => 4284, t4285 => 4285, t4286 => 4286, t4287 => 4287, t4288 => 4288, t4289 => 4289, t4290 => 4290, t4291 => 4291, t4292 => 4292, t4293 => 4293, t4294 => 4294, t4295 => 4295, t4296 => 4296, t4297 => 4297, t4298 => 4298, t4299 => 4299, t4300 => 4300, t4301 => 4301, t4302 => 4302, t4303 => 4303, t4304 => 4304, t4305 => 4305, t4306 => 4306, t4307 => 4307, t4308 => 4308, t4309 => 4309, t4310 => 4310, t4311 => 4311, t4312 => 4312, t4313 => 4313, t4314 => 4314, t4315 => 4315, t4316 => 4316, t4317 => 4317, t4318 => 4318, t4319 => 4319, t4320 => 4320, t4321 => 4321, t4322 => 4322, t4323 => 4323, t4324 => 4324, t4325 => 4325, t4326 => 4326, t4327 => 4327, t4328 => 4328, t4329 => 4329, t4330 => 4330, t4331 => 4331, t4332 => 4332, t4333 => 4333, t4334 => 4334, t4335 => 4335, t4336 => 4336, t4337 => 4337, t4338 => 4338, t4339 => 4339, t4340 => 4340, t4341 => 4341, t4342 => 4342, t4343 => 4343, t4344 => 4344, t4345 => 4345, t4346 => 4346, t4347 => 4347, t4348 => 4348, t4349 => 4349, t4350 => 4350, t4351 => 4351, t4352 => 4352, t4353 => 4353, t4354 => 4354, t4355 => 4355, t4356 => 4356, t4357 => 4357, t4358 => 4358, t4359 => 4359, t4360 => 4360, t4361 => 4361, t4362 => 4362, t4363 => 4363, t4364 => 4364, t4365 => 4365, t4366 => 4366, t4367 => 4367, t4368 => 4368, t4369 => 4369, t4370 => 4370, t4371 => 4371, t4372 => 4372, t4373 => 4373, t4374 => 4374, t4375 => 4375, t4376 => 4376, t4377 => 4377, t4378 => 4378, t4379 => 4379, t4380 => 4380, t4381 => 4381, t4382 => 4382, t4383 => 4383, t4384 => 4384, t4385 => 4385, t4386 => 4386, t4387 => 4387, t4388 => 4388, t4389 => 4389, t4390 => 4390, t4391 => 4391, t4392 => 4392, t4393 => 4393, t4394 => 4394, t4395 => 4395, t4396 => 4396, t4397 => 4397, t4398 => 4398, t4399 => 4399, t4400 => 4400, t4401 => 4401, t4402 => 4402, t4403 => 4403, t4404 => 4404, t4405 => 4405, t4406 => 4406, t4407 => 4407, t4408 => 4408, t4409 => 4409, t4410 => 4410, t4411 => 4411, t4412 => 4412, t4413 => 4413, t4414 => 4414, t4415 => 4415, t4416 => 4416, t4417 => 4417, t4418 => 4418, t4419 => 4419, t4420 => 4420, t4421 => 4421, t4422 => 4422, t4423 => 4423, t4424 => 4424, t4425 => 4425, t4426 => 4426, t4427 => 4427, t4428 => 4428, t4429 => 4429, t4430 => 4430, t4431 => 4431, t4432 => 4432, t4433 => 4433, t4434 => 4434, t4435 => 4435, t4436 => 4436, t4437 => 4437, t4438 => 4438, t4439 => 4439, t4440 => 4440, t4441 => 4441, t4442 => 4442, t4443 => 4443, t4444 => 4444, t4445 => 4445, t4446 => 4446, t4447 => 4447, t4448 => 4448, t4449 => 4449, t4450 => 4450, t4451 => 4451, t4452 => 4452, t4453 => 4453, t4454 => 4454, t4455 => 4455, t4456 => 4456, t4457 => 4457, t4458 => 4458, t4459 => 4459, t4460 => 4460, t4461 => 4461, t4462 => 4462, t4463 => 4463, t4464 => 4464, t4465 => 4465, t4466 => 4466, t4467 => 4467, t4468 => 4468, t4469 => 4469, t4470 => 4470, t4471 => 4471, t4472 => 4472, t4473 => 4473, t4474 => 4474, t4475 => 4475, t4476 => 4476, t4477 => 4477, t4478 => 4478, t4479 => 4479, t4480 => 4480, t4481 => 4481, t4482 => 4482, t4483 => 4483, t4484 => 4484, t4485 => 4485, t4486 => 4486, t4487 => 4487, t4488 => 4488, t4489 => 4489, t4490 => 4490, t4491 => 4491, t4492 => 4492, t4493 => 4493, t4494 => 4494, t4495 => 4495, t4496 => 4496, t4497 => 4497, t4498 => 4498, t4499 => 4499, t4500 => 4500, t4501 => 4501, t4502 => 4502, t4503 => 4503, t4504 => 4504, t4505 => 4505, t4506 => 4506, t4507 => 4507, t4508 => 4508, t4509 => 4509, t4510 => 4510, t4511 => 4511, t4512 => 4512, t4513 => 4513, t4514 => 4514, t4515 => 4515, t4516 => 4516, t4517 => 4517, t4518 => 4518, t4519 => 4519, t4520 => 4520, t4521 => 4521, t4522 => 4522, t4523 => 4523, t4524 => 4524, t4525 => 4525, t4526 => 4526, t4527 => 4527, t4528 => 4528, t4529 => 4529, t4530 => 4530, t4531 => 4531, t4532 => 4532, t4533 => 4533, t4534 => 4534, t4535 => 4535, t4536 => 4536, t4537 => 4537, t4538 => 4538, t4539 => 4539, t4540 => 4540, t4541 => 4541, t4542 => 4542, t4543 => 4543, t4544 => 4544, t4545 => 4545, t4546 => 4546, t4547 => 4547, t4548 => 4548, t4549 => 4549, t4550 => 4550, t4551 => 4551, t4552 => 4552, t4553 => 4553, t4554 => 4554, t4555 => 4555, t4556 => 4556, t4557 => 4557, t4558 => 4558, t4559 => 4559, t4560 => 4560, t4561 => 4561, t4562 => 4562, t4563 => 4563, t4564 => 4564, t4565 => 4565, t4566 => 4566, t4567 => 4567, t4568 => 4568, t4569 => 4569, t4570 => 4570, t4571 => 4571, t4572 => 4572, t4573 => 4573, t4574 => 4574, t4575 => 4575, t4576 => 4576, t4577 => 4577, t4578 => 4578, t4579 => 4579, t4580 => 4580, t4581 => 4581, t4582 => 4582, t4583 => 4583, t4584 => 4584, t4585 => 4585, t4586 => 4586, t4587 => 4587, t4588 => 4588, t4589 => 4589, t4590 => 4590, t4591 => 4591, t4592 => 4592, t4593 => 4593, t4594 => 4594, t4595 => 4595, t4596 => 4596, t4597 => 4597, t4598 => 4598, t4599 => 4599, t4600 => 4600, t4601 => 4601, t4602 => 4602, t4603 => 4603, t4604 => 4604, t4605 => 4605, t4606 => 4606, t4607 => 4607, t4608 => 4608, t4609 => 4609, t4610 => 4610, t4611 => 4611, t4612 => 4612, t4613 => 4613, t4614 => 4614, t4615 => 4615, t4616 => 4616, t4617 => 4617, t4618 => 4618, t4619 => 4619, t4620 => 4620, t4621 => 4621, t4622 => 4622, t4623 => 4623, t4624 => 4624, t4625 => 4625, t4626 => 4626, t4627 => 4627, t4628 => 4628, t4629 => 4629, t4630 => 4630, t4631 => 4631, t4632 => 4632, t4633 => 4633, t4634 => 4634, t4635 => 4635, t4636 => 4636, t4637 => 4637, t4638 => 4638, t4639 => 4639, t4640 => 4640, t4641 => 4641, t4642 => 4642, t4643 => 4643, t4644 => 4644, t4645 => 4645, t4646 => 4646, t4647 => 4647, t4648 => 4648, t4649 => 4649, t4650 => 4650, t4651 => 4651, t4652 => 4652, t4653 => 4653, t4654 => 4654, t4655 => 4655, t4656 => 4656, t4657 => 4657, t4658 => 4658, t4659 => 4659, t4660 => 4660, t4661 => 4661, t4662 => 4662, t4663 => 4663, t4664 => 4664, t4665 => 4665, t4666 => 4666, t4667 => 4667, t4668 => 4668, t4669 => 4669, t4670 => 4670, t4671 => 4671, t4672 => 4672, t4673 => 4673, t4674 => 4674, t4675 => 4675, t4676 => 4676, t4677 => 4677, t4678 => 4678, t4679 => 4679, t4680 => 4680, t4681 => 4681, t4682 => 4682, t4683 => 4683, t4684 => 4684, t4685 => 4685, t4686 => 4686, t4687 => 4687, t4688 => 4688, t4689 => 4689, t4690 => 4690, t4691 => 4691, t4692 => 4692, t4693 => 4693, t4694 => 4694, t4695 => 4695, t4696 => 4696, t4697 => 4697, t4698 => 4698, t4699 => 4699, t4700 => 4700, t4701 => 4701, t4702 => 4702, t4703 => 4703, t4704 => 4704, t4705 => 4705, t4706 => 4706, t4707 => 4707, t4708 => 4708, t4709 => 4709, t4710 => 4710, t4711 => 4711, t4712 => 4712, t4713 => 4713, t4714 => 4714, t4715 => 4715, t4716 => 4716, t4717 => 4717, t4718 => 4718, t4719 => 4719, t4720 => 4720, t4721 => 4721, t4722 => 4722, t4723 => 4723, t4724 => 4724, t4725 => 4725, t4726 => 4726, t4727 => 4727, t4728 => 4728, t4729 => 4729, t4730 => 4730, t4731 => 4731, t4732 => 4732, t4733 => 4733, t4734 => 4734, t4735 => 4735, t4736 => 4736, t4737 => 4737, t4738 => 4738, t4739 => 4739, t4740 => 4740, t4741 => 4741, t4742 => 4742, t4743 => 4743, t4744 => 4744, t4745 => 4745, t4746 => 4746, t4747 => 4747, t4748 => 4748, t4749 => 4749, t4750 => 4750, t4751 => 4751, t4752 => 4752, t4753 => 4753, t4754 => 4754, t4755 => 4755, t4756 => 4756, t4757 => 4757, t4758 => 4758, t4759 => 4759, t4760 => 4760, t4761 => 4761, t4762 => 4762, t4763 => 4763, t4764 => 4764, t4765 => 4765, t4766 => 4766, t4767 => 4767, t4768 => 4768, t4769 => 4769, t4770 => 4770, t4771 => 4771, t4772 => 4772, t4773 => 4773, t4774 => 4774, t4775 => 4775, t4776 => 4776, t4777 => 4777, t4778 => 4778, t4779 => 4779, t4780 => 4780, t4781 => 4781, t4782 => 4782, t4783 => 4783, t4784 => 4784, t4785 => 4785, t4786 => 4786, t4787 => 4787, t4788 => 4788, t4789 => 4789, t4790 => 4790, t4791 => 4791, t4792 => 4792, t4793 => 4793, t4794 => 4794, t4795 => 4795, t4796 => 4796, t4797 => 4797, t4798 => 4798, t4799 => 4799, t4800 => 4800, t4801 => 4801, t4802 => 4802, t4803 => 4803, t4804 => 4804, t4805 => 4805, t4806 => 4806, t4807 => 4807, t4808 => 4808, t4809 => 4809, t4810 => 4810, t4811 => 4811, t4812 => 4812, t4813 => 4813, t4814 => 4814, t4815 => 4815, t4816 => 4816, t4817 => 4817, t4818 => 4818, t4819 => 4819, t4820 => 4820, t4821 => 4821, t4822 => 4822, t4823 => 4823, t4824 => 4824, t4825 => 4825, t4826 => 4826, t4827 => 4827, t4828 => 4828, t4829 => 4829, t4830 => 4830, t4831 => 4831, t4832 => 4832, t4833 => 4833, t4834 => 4834, t4835 => 4835, t4836 => 4836, t4837 => 4837, t4838 => 4838, t4839 => 4839, t4840 => 4840, t4841 => 4841, t4842 => 4842, t4843 => 4843, t4844 => 4844, t4845 => 4845, t4846 => 4846, t4847 => 4847, t4848 => 4848, t4849 => 4849, t4850 => 4850, t4851 => 4851, t4852 => 4852, t4853 => 4853, t4854 => 4854, t4855 => 4855, t4856 => 4856, t4857 => 4857, t4858 => 4858, t4859 => 4859, t4860 => 4860, t4861 => 4861, t4862 => 4862, t4863 => 4863, t4864 => 4864, t4865 => 4865, t4866 => 4866, t4867 => 4867, t4868 => 4868, t4869 => 4869, t4870 => 4870, t4871 => 4871, t4872 => 4872, t4873 => 4873, t4874 => 4874, t4875 => 4875, t4876 => 4876, t4877 => 4877, t4878 => 4878, t4879 => 4879, t4880 => 4880, t4881 => 4881, t4882 => 4882, t4883 => 4883, t4884 => 4884, t4885 => 4885, t4886 => 4886, t4887 => 4887, t4888 => 4888, t4889 => 4889, t4890 => 4890, t4891 => 4891, t4892 => 4892, t4893 => 4893, t4894 => 4894, t4895 => 4895, t4896 => 4896, t4897 => 4897, t4898 => 4898, t4899 => 4899, t4900 => 4900, t4901 => 4901, t4902 => 4902, t4903 => 4903, t4904 => 4904, t4905 => 4905, t4906 => 4906, t4907 => 4907, t4908 => 4908, t4909 => 4909, t4910 => 4910, t4911 => 4911, t4912 => 4912, t4913 => 4913, t4914 => 4914, t4915 => 4915, t4916 => 4916, t4917 => 4917, t4918 => 4918, t4919 => 4919, t4920 => 4920, t4921 => 4921, t4922 => 4922, t4923 => 4923, t4924 => 4924, t4925 => 4925, t4926 => 4926, t4927 => 4927, t4928 => 4928, t4929 => 4929, t4930 => 4930, t4931 => 4931, t4932 => 4932, t4933 => 4933, t4934 => 4934, t4935 => 4935, t4936 => 4936, t4937 => 4937, t4938 => 4938, t4939 => 4939, t4940 => 4940, t4941 => 4941, t4942 => 4942, t4943 => 4943, t4944 => 4944, t4945 => 4945, t4946 => 4946, t4947 => 4947, t4948 => 4948, t4949 => 4949, t4950 => 4950, t4951 => 4951, t4952 => 4952, t4953 => 4953, t4954 => 4954, t4955 => 4955, t4956 => 4956, t4957 => 4957, t4958 => 4958, t4959 => 4959, t4960 => 4960, t4961 => 4961, t4962 => 4962, t4963 => 4963, t4964 => 4964, t4965 => 4965, t4966 => 4966, t4967 => 4967, t4968 => 4968, t4969 => 4969, t4970 => 4970, t4971 => 4971, t4972 => 4972, t4973 => 4973, t4974 => 4974, t4975 => 4975, t4976 => 4976, t4977 => 4977, t4978 => 4978, t4979 => 4979, t4980 => 4980, t4981 => 4981, t4982 => 4982, t4983 => 4983, t4984 => 4984, t4985 => 4985, t4986 => 4986, t4987 => 4987, t4988 => 4988, t4989 => 4989, t4990 => 4990, t4991 => 4991, t4992 => 4992, t4993 => 4993, t4994 => 4994, t4995 => 4995, t4996 => 4996, t4997 => 4997, t4998 => 4998, t4999 => 4999, t5000 => 5000, t5001 => 5001, t5002 => 5002, t5003 => 5003, t5004 => 5004, t5005 => 5005, t5006 => 5006, t5007 => 5007, t5008 => 5008, t5009 => 5009, t5010 => 5010, t5011 => 5011, t5012 => 5012, t5013 => 5013, t5014 => 5014, t5015 => 5015, t5016 => 5016, t5017 => 5017, t5018 => 5018, t5019 => 5019, t5020 => 5020, t5021 => 5021, t5022 => 5022, t5023 => 5023, t5024 => 5024, t5025 => 5025, t5026 => 5026, t5027 => 5027, t5028 => 5028, t5029 => 5029, t5030 => 5030, t5031 => 5031, t5032 => 5032, t5033 => 5033, t5034 => 5034, t5035 => 5035, t5036 => 5036, t5037 => 5037, t5038 => 5038, t5039 => 5039, t5040 => 5040, t5041 => 5041, t5042 => 5042, t5043 => 5043, t5044 => 5044, t5045 => 5045, t5046 => 5046, t5047 => 5047, t5048 => 5048, t5049 => 5049, t5050 => 5050, t5051 => 5051, t5052 => 5052, t5053 => 5053, t5054 => 5054, t5055 => 5055, t5056 => 5056, t5057 => 5057, t5058 => 5058, t5059 => 5059, t5060 => 5060, t5061 => 5061, t5062 => 5062, t5063 => 5063, t5064 => 5064, t5065 => 5065, t5066 => 5066, t5067 => 5067, t5068 => 5068, t5069 => 5069, t5070 => 5070, t5071 => 5071, t5072 => 5072, t5073 => 5073, t5074 => 5074, t5075 => 5075, t5076 => 5076, t5077 => 5077, t5078 => 5078, t5079 => 5079, t5080 => 5080, t5081 => 5081, t5082 => 5082, t5083 => 5083, t5084 => 5084, t5085 => 5085, t5086 => 5086, t5087 => 5087, t5088 => 5088, t5089 => 5089, t5090 => 5090, t5091 => 5091, t5092 => 5092, t5093 => 5093, t5094 => 5094, t5095 => 5095, t5096 => 5096, t5097 => 5097, t5098 => 5098, t5099 => 5099, t5100 => 5100, t5101 => 5101, t5102 => 5102, t5103 => 5103, t5104 => 5104, t5105 => 5105, t5106 => 5106, t5107 => 5107, t5108 => 5108, t5109 => 5109, t5110 => 5110, t5111 => 5111, t5112 => 5112, t5113 => 5113, t5114 => 5114, t5115 => 5115, t5116 => 5116, t5117 => 5117, t5118 => 5118, t5119 => 5119, t5120 => 5120, t5121 => 5121, t5122 => 5122, t5123 => 5123, t5124 => 5124, t5125 => 5125, t5126 => 5126, t5127 => 5127, t5128 => 5128, t5129 => 5129, t5130 => 5130, t5131 => 5131, t5132 => 5132, t5133 => 5133, t5134 => 5134, t5135 => 5135, t5136 => 5136, t5137 => 5137, t5138 => 5138, t5139 => 5139, t5140 => 5140, t5141 => 5141, t5142 => 5142, t5143 => 5143, t5144 => 5144, t5145 => 5145, t5146 => 5146, t5147 => 5147, t5148 => 5148, t5149 => 5149, t5150 => 5150, t5151 => 5151, t5152 => 5152, t5153 => 5153, t5154 => 5154, t5155 => 5155, t5156 => 5156, t5157 => 5157, t5158 => 5158, t5159 => 5159, t5160 => 5160, t5161 => 5161, t5162 => 5162, t5163 => 5163, t5164 => 5164, t5165 => 5165, t5166 => 5166, t5167 => 5167, t5168 => 5168, t5169 => 5169, t5170 => 5170, t5171 => 5171, t5172 => 5172, t5173 => 5173, t5174 => 5174, t5175 => 5175, t5176 => 5176, t5177 => 5177, t5178 => 5178, t5179 => 5179, t5180 => 5180, t5181 => 5181, t5182 => 5182, t5183 => 5183, t5184 => 5184, t5185 => 5185, t5186 => 5186, t5187 => 5187, t5188 => 5188, t5189 => 5189, t5190 => 5190, t5191 => 5191, t5192 => 5192, t5193 => 5193, t5194 => 5194, t5195 => 5195, t5196 => 5196, t5197 => 5197, t5198 => 5198, t5199 => 5199, t5200 => 5200, t5201 => 5201, t5202 => 5202, t5203 => 5203, t5204 => 5204, t5205 => 5205, t5206 => 5206, t5207 => 5207, t5208 => 5208, t5209 => 5209, t5210 => 5210, t5211 => 5211, t5212 => 5212, t5213 => 5213, t5214 => 5214, t5215 => 5215, t5216 => 5216, t5217 => 5217, t5218 => 5218, t5219 => 5219, t5220 => 5220, t5221 => 5221, t5222 => 5222, t5223 => 5223, t5224 => 5224, t5225 => 5225, t5226 => 5226, t5227 => 5227, t5228 => 5228, t5229 => 5229, t5230 => 5230, t5231 => 5231, t5232 => 5232, t5233 => 5233, t5234 => 5234, t5235 => 5235, t5236 => 5236, t5237 => 5237, t5238 => 5238, t5239 => 5239, t5240 => 5240, t5241 => 5241, t5242 => 5242, t5243 => 5243, t5244 => 5244, t5245 => 5245, t5246 => 5246, t5247 => 5247, t5248 => 5248, t5249 => 5249, t5250 => 5250, t5251 => 5251, t5252 => 5252, t5253 => 5253, t5254 => 5254, t5255 => 5255, t5256 => 5256, t5257 => 5257, t5258 => 5258, t5259 => 5259, t5260 => 5260, t5261 => 5261, t5262 => 5262, t5263 => 5263, t5264 => 5264, t5265 => 5265, t5266 => 5266, t5267 => 5267, t5268 => 5268, t5269 => 5269, t5270 => 5270, t5271 => 5271, t5272 => 5272, t5273 => 5273, t5274 => 5274, t5275 => 5275, t5276 => 5276, t5277 => 5277, t5278 => 5278, t5279 => 5279, t5280 => 5280, t5281 => 5281, t5282 => 5282, t5283 => 5283, t5284 => 5284, t5285 => 5285, t5286 => 5286, t5287 => 5287, t5288 => 5288, t5289 => 5289, t5290 => 5290, t5291 => 5291, t5292 => 5292, t5293 => 5293, t5294 => 5294, t5295 => 5295, t5296 => 5296, t5297 => 5297, t5298 => 5298, t5299 => 5299, t5300 => 5300, t5301 => 5301, t5302 => 5302, t5303 => 5303, t5304 => 5304, t5305 => 5305, t5306 => 5306, t5307 => 5307, t5308 => 5308, t5309 => 5309, t5310 => 5310, t5311 => 5311, t5312 => 5312, t5313 => 5313, t5314 => 5314, t5315 => 5315, t5316 => 5316, t5317 => 5317, t5318 => 5318, t5319 => 5319, t5320 => 5320, t5321 => 5321, t5322 => 5322, t5323 => 5323, t5324 => 5324, t5325 => 5325, t5326 => 5326, t5327 => 5327, t5328 => 5328, t5329 => 5329, t5330 => 5330, t5331 => 5331, t5332 => 5332, t5333 => 5333, t5334 => 5334, t5335 => 5335, t5336 => 5336, t5337 => 5337, t5338 => 5338, t5339 => 5339, t5340 => 5340, t5341 => 5341, t5342 => 5342, t5343 => 5343, t5344 => 5344, t5345 => 5345, t5346 => 5346, t5347 => 5347, t5348 => 5348, t5349 => 5349, t5350 => 5350, t5351 => 5351, t5352 => 5352, t5353 => 5353, t5354 => 5354, t5355 => 5355, t5356 => 5356, t5357 => 5357, t5358 => 5358, t5359 => 5359, t5360 => 5360, t5361 => 5361, t5362 => 5362, t5363 => 5363, t5364 => 5364, t5365 => 5365, t5366 => 5366, t5367 => 5367, t5368 => 5368, t5369 => 5369, t5370 => 5370, t5371 => 5371, t5372 => 5372, t5373 => 5373, t5374 => 5374, t5375 => 5375, t5376 => 5376, t5377 => 5377, t5378 => 5378, t5379 => 5379, t5380 => 5380, t5381 => 5381, t5382 => 5382, t5383 => 5383, t5384 => 5384, t5385 => 5385, t5386 => 5386, t5387 => 5387, t5388 => 5388, t5389 => 5389, t5390 => 5390, t5391 => 5391, t5392 => 5392, t5393 => 5393, t5394 => 5394, t5395 => 5395, t5396 => 5396, t5397 => 5397, t5398 => 5398, t5399 => 5399, t5400 => 5400, t5401 => 5401, t5402 => 5402, t5403 => 5403, t5404 => 5404, t5405 => 5405, t5406 => 5406, t5407 => 5407, t5408 => 5408, t5409 => 5409, t5410 => 5410, t5411 => 5411, t5412 => 5412, t5413 => 5413, t5414 => 5414, t5415 => 5415, t5416 => 5416, t5417 => 5417, t5418 => 5418, t5419 => 5419, t5420 => 5420, t5421 => 5421, t5422 => 5422, t5423 => 5423, t5424 => 5424, t5425 => 5425, t5426 => 5426, t5427 => 5427, t5428 => 5428, t5429 => 5429, t5430 => 5430, t5431 => 5431, t5432 => 5432, t5433 => 5433, t5434 => 5434, t5435 => 5435, t5436 => 5436, t5437 => 5437, t5438 => 5438, t5439 => 5439, t5440 => 5440, t5441 => 5441, t5442 => 5442, t5443 => 5443, t5444 => 5444, t5445 => 5445, t5446 => 5446, t5447 => 5447, t5448 => 5448, t5449 => 5449, t5450 => 5450, t5451 => 5451, t5452 => 5452, t5453 => 5453, t5454 => 5454, t5455 => 5455, t5456 => 5456, t5457 => 5457, t5458 => 5458, t5459 => 5459, t5460 => 5460, t5461 => 5461, t5462 => 5462, t5463 => 5463, t5464 => 5464, t5465 => 5465, t5466 => 5466, t5467 => 5467, t5468 => 5468, t5469 => 5469, t5470 => 5470, t5471 => 5471, t5472 => 5472, t5473 => 5473, t5474 => 5474, t5475 => 5475, t5476 => 5476, t5477 => 5477, t5478 => 5478, t5479 => 5479, t5480 => 5480, t5481 => 5481, t5482 => 5482, t5483 => 5483, t5484 => 5484, t5485 => 5485, t5486 => 5486, t5487 => 5487, t5488 => 5488, t5489 => 5489, t5490 => 5490, t5491 => 5491, t5492 => 5492, t5493 => 5493, t5494 => 5494, t5495 => 5495, t5496 => 5496, t5497 => 5497, t5498 => 5498, t5499 => 5499, t5500 => 5500, t5501 => 5501, t5502 => 5502, t5503 => 5503, t5504 => 5504, t5505 => 5505, t5506 => 5506, t5507 => 5507, t5508 => 5508, t5509 => 5509, t5510 => 5510, t5511 => 5511, t5512 => 5512, t5513 => 5513, t5514 => 5514, t5515 => 5515, t5516 => 5516, t5517 => 5517, t5518 => 5518, t5519 => 5519, t5520 => 5520, t5521 => 5521, t5522 => 5522, t5523 => 5523, t5524 => 5524, t5525 => 5525, t5526 => 5526, t5527 => 5527, t5528 => 5528, t5529 => 5529, t5530 => 5530, t5531 => 5531, t5532 => 5532, t5533 => 5533, t5534 => 5534, t5535 => 5535, t5536 => 5536, t5537 => 5537, t5538 => 5538, t5539 => 5539, t5540 => 5540, t5541 => 5541, t5542 => 5542, t5543 => 5543, t5544 => 5544, t5545 => 5545, t5546 => 5546, t5547 => 5547, t5548 => 5548, t5549 => 5549, t5550 => 5550, t5551 => 5551, t5552 => 5552, t5553 => 5553, t5554 => 5554, t5555 => 5555, t5556 => 5556, t5557 => 5557, t5558 => 5558, t5559 => 5559, t5560 => 5560, t5561 => 5561, t5562 => 5562, t5563 => 5563, t5564 => 5564, t5565 => 5565, t5566 => 5566, t5567 => 5567, t5568 => 5568, t5569 => 5569, t5570 => 5570, t5571 => 5571, t5572 => 5572, t5573 => 5573, t5574 => 5574, t5575 => 5575, t5576 => 5576, t5577 => 5577, t5578 => 5578, t5579 => 5579, t5580 => 5580, t5581 => 5581, t5582 => 5582, t5583 => 5583, t5584 => 5584, t5585 => 5585, t5586 => 5586, t5587 => 5587, t5588 => 5588, t5589 => 5589, t5590 => 5590, t5591 => 5591, t5592 => 5592, t5593 => 5593, t5594 => 5594, t5595 => 5595, t5596 => 5596, t5597 => 5597, t5598 => 5598, t5599 => 5599, t5600 => 5600, t5601 => 5601, t5602 => 5602, t5603 => 5603, t5604 => 5604, t5605 => 5605, t5606 => 5606, t5607 => 5607, t5608 => 5608, t5609 => 5609, t5610 => 5610, t5611 => 5611, t5612 => 5612, t5613 => 5613, t5614 => 5614, t5615 => 5615, t5616 => 5616, t5617 => 5617, t5618 => 5618, t5619 => 5619, t5620 => 5620, t5621 => 5621, t5622 => 5622, t5623 => 5623, t5624 => 5624, t5625 => 5625, t5626 => 5626, t5627 => 5627, t5628 => 5628, t5629 => 5629, t5630 => 5630, t5631 => 5631, t5632 => 5632, t5633 => 5633, t5634 => 5634, t5635 => 5635, t5636 => 5636, t5637 => 5637, t5638 => 5638, t5639 => 5639, t5640 => 5640, t5641 => 5641, t5642 => 5642, t5643 => 5643, t5644 => 5644, t5645 => 5645, t5646 => 5646, t5647 => 5647, t5648 => 5648, t5649 => 5649, t5650 => 5650, t5651 => 5651, t5652 => 5652, t5653 => 5653, t5654 => 5654, t5655 => 5655, t5656 => 5656, t5657 => 5657, t5658 => 5658, t5659 => 5659, t5660 => 5660, t5661 => 5661, t5662 => 5662, t5663 => 5663, t5664 => 5664, t5665 => 5665, t5666 => 5666, t5667 => 5667, t5668 => 5668, t5669 => 5669, t5670 => 5670, t5671 => 5671, t5672 => 5672, t5673 => 5673, t5674 => 5674, t5675 => 5675, t5676 => 5676, t5677 => 5677, t5678 => 5678, t5679 => 5679, t5680 => 5680, t5681 => 5681, t5682 => 5682, t5683 => 5683, t5684 => 5684, t5685 => 5685, t5686 => 5686, t5687 => 5687, t5688 => 5688, t5689 => 5689, t5690 => 5690, t5691 => 5691, t5692 => 5692, t5693 => 5693, t5694 => 5694, t5695 => 5695, t5696 => 5696, t5697 => 5697, t5698 => 5698, t5699 => 5699, t5700 => 5700, t5701 => 5701, t5702 => 5702, t5703 => 5703, t5704 => 5704, t5705 => 5705, t5706 => 5706, t5707 => 5707, t5708 => 5708, t5709 => 5709, t5710 => 5710, t5711 => 5711, t5712 => 5712, t5713 => 5713, t5714 => 5714, t5715 => 5715, t5716 => 5716, t5717 => 5717, t5718 => 5718, t5719 => 5719, t5720 => 5720, t5721 => 5721, t5722 => 5722, t5723 => 5723, t5724 => 5724, t5725 => 5725, t5726 => 5726, t5727 => 5727, t5728 => 5728, t5729 => 5729, t5730 => 5730, t5731 => 5731, t5732 => 5732, t5733 => 5733, t5734 => 5734, t5735 => 5735, t5736 => 5736, t5737 => 5737, t5738 => 5738, t5739 => 5739, t5740 => 5740, t5741 => 5741, t5742 => 5742, t5743 => 5743, t5744 => 5744, t5745 => 5745, t5746 => 5746, t5747 => 5747, t5748 => 5748, t5749 => 5749, t5750 => 5750, t5751 => 5751, t5752 => 5752, t5753 => 5753, t5754 => 5754, t5755 => 5755, t5756 => 5756, t5757 => 5757, t5758 => 5758, t5759 => 5759, t5760 => 5760, t5761 => 5761, t5762 => 5762, t5763 => 5763, t5764 => 5764, t5765 => 5765, t5766 => 5766, t5767 => 5767, t5768 => 5768, t5769 => 5769, t5770 => 5770, t5771 => 5771, t5772 => 5772, t5773 => 5773, t5774 => 5774, t5775 => 5775, t5776 => 5776, t5777 => 5777, t5778 => 5778, t5779 => 5779, t5780 => 5780, t5781 => 5781, t5782 => 5782, t5783 => 5783, t5784 => 5784, t5785 => 5785, t5786 => 5786, t5787 => 5787, t5788 => 5788, t5789 => 5789, t5790 => 5790, t5791 => 5791, t5792 => 5792, t5793 => 5793, t5794 => 5794, t5795 => 5795, t5796 => 5796, t5797 => 5797, t5798 => 5798, t5799 => 5799, t5800 => 5800, t5801 => 5801, t5802 => 5802, t5803 => 5803, t5804 => 5804, t5805 => 5805, t5806 => 5806, t5807 => 5807, t5808 => 5808, t5809 => 5809, t5810 => 5810, t5811 => 5811, t5812 => 5812, t5813 => 5813, t5814 => 5814, t5815 => 5815, t5816 => 5816, t5817 => 5817, t5818 => 5818, t5819 => 5819, t5820 => 5820, t5821 => 5821, t5822 => 5822, t5823 => 5823, t5824 => 5824, t5825 => 5825, t5826 => 5826, t5827 => 5827, t5828 => 5828, t5829 => 5829, t5830 => 5830, t5831 => 5831, t5832 => 5832, t5833 => 5833, t5834 => 5834, t5835 => 5835, t5836 => 5836, t5837 => 5837, t5838 => 5838, t5839 => 5839, t5840 => 5840, t5841 => 5841, t5842 => 5842, t5843 => 5843, t5844 => 5844, t5845 => 5845, t5846 => 5846, t5847 => 5847, t5848 => 5848, t5849 => 5849, t5850 => 5850, t5851 => 5851, t5852 => 5852, t5853 => 5853, t5854 => 5854, t5855 => 5855, t5856 => 5856, t5857 => 5857, t5858 => 5858, t5859 => 5859, t5860 => 5860, t5861 => 5861, t5862 => 5862, t5863 => 5863, t5864 => 5864, t5865 => 5865, t5866 => 5866, t5867 => 5867, t5868 => 5868, t5869 => 5869, t5870 => 5870, t5871 => 5871, t5872 => 5872, t5873 => 5873, t5874 => 5874, t5875 => 5875, t5876 => 5876, t5877 => 5877, t5878 => 5878, t5879 => 5879, t5880 => 5880, t5881 => 5881, t5882 => 5882, t5883 => 5883, t5884 => 5884, t5885 => 5885, t5886 => 5886, t5887 => 5887, t5888 => 5888, t5889 => 5889, t5890 => 5890, t5891 => 5891, t5892 => 5892, t5893 => 5893, t5894 => 5894, t5895 => 5895, t5896 => 5896, t5897 => 5897, t5898 => 5898, t5899 => 5899, t5900 => 5900, t5901 => 5901, t5902 => 5902, t5903 => 5903, t5904 => 5904, t5905 => 5905, t5906 => 5906, t5907 => 5907, t5908 => 5908, t5909 => 5909, t5910 => 5910, t5911 => 5911, t5912 => 5912, t5913 => 5913, t5914 => 5914, t5915 => 5915, t5916 => 5916, t5917 => 5917, t5918 => 5918, t5919 => 5919, t5920 => 5920, t5921 => 5921, t5922 => 5922, t5923 => 5923, t5924 => 5924, t5925 => 5925, t5926 => 5926, t5927 => 5927, t5928 => 5928, t5929 => 5929, t5930 => 5930, t5931 => 5931, t5932 => 5932, t5933 => 5933, t5934 => 5934, t5935 => 5935, t5936 => 5936, t5937 => 5937, t5938 => 5938, t5939 => 5939, t5940 => 5940, t5941 => 5941, t5942 => 5942, t5943 => 5943, t5944 => 5944, t5945 => 5945, t5946 => 5946, t5947 => 5947, t5948 => 5948, t5949 => 5949, t5950 => 5950, t5951 => 5951, t5952 => 5952, t5953 => 5953, t5954 => 5954, t5955 => 5955, t5956 => 5956, t5957 => 5957, t5958 => 5958, t5959 => 5959, t5960 => 5960, t5961 => 5961, t5962 => 5962, t5963 => 5963, t5964 => 5964, t5965 => 5965, t5966 => 5966, t5967 => 5967, t5968 => 5968, t5969 => 5969, t5970 => 5970, t5971 => 5971, t5972 => 5972, t5973 => 5973, t5974 => 5974, t5975 => 5975, t5976 => 5976, t5977 => 5977, t5978 => 5978, t5979 => 5979, t5980 => 5980, t5981 => 5981, t5982 => 5982, t5983 => 5983, t5984 => 5984, t5985 => 5985, t5986 => 5986, t5987 => 5987, t5988 => 5988, t5989 => 5989, t5990 => 5990, t5991 => 5991, t5992 => 5992, t5993 => 5993, t5994 => 5994, t5995 => 5995, t5996 => 5996, t5997 => 5997, t5998 => 5998, t5999 => 5999, t6000 => 6000, t6001 => 6001, t6002 => 6002, t6003 => 6003, t6004 => 6004, t6005 => 6005, t6006 => 6006, t6007 => 6007, t6008 => 6008, t6009 => 6009, t6010 => 6010, t6011 => 6011, t6012 => 6012, t6013 => 6013, t6014 => 6014, t6015 => 6015, t6016 => 6016, t6017 => 6017, t6018 => 6018, t6019 => 6019, t6020 => 6020, t6021 => 6021, t6022 => 6022, t6023 => 6023, t6024 => 6024, t6025 => 6025, t6026 => 6026, t6027 => 6027, t6028 => 6028, t6029 => 6029, t6030 => 6030, t6031 => 6031, t6032 => 6032, t6033 => 6033, t6034 => 6034, t6035 => 6035, t6036 => 6036, t6037 => 6037, t6038 => 6038, t6039 => 6039, t6040 => 6040, t6041 => 6041, t6042 => 6042, t6043 => 6043, t6044 => 6044, t6045 => 6045, t6046 => 6046, t6047 => 6047, t6048 => 6048, t6049 => 6049, t6050 => 6050, t6051 => 6051, t6052 => 6052, t6053 => 6053, t6054 => 6054, t6055 => 6055, t6056 => 6056, t6057 => 6057, t6058 => 6058, t6059 => 6059, t6060 => 6060, t6061 => 6061, t6062 => 6062, t6063 => 6063, t6064 => 6064, t6065 => 6065, t6066 => 6066, t6067 => 6067, t6068 => 6068, t6069 => 6069, t6070 => 6070, t6071 => 6071, t6072 => 6072, t6073 => 6073, t6074 => 6074, t6075 => 6075, t6076 => 6076, t6077 => 6077, t6078 => 6078, t6079 => 6079, t6080 => 6080, t6081 => 6081, t6082 => 6082, t6083 => 6083, t6084 => 6084, t6085 => 6085, t6086 => 6086, t6087 => 6087, t6088 => 6088, t6089 => 6089, t6090 => 6090, t6091 => 6091, t6092 => 6092, t6093 => 6093, t6094 => 6094, t6095 => 6095, t6096 => 6096, t6097 => 6097, t6098 => 6098, t6099 => 6099, t6100 => 6100, t6101 => 6101, t6102 => 6102, t6103 => 6103, t6104 => 6104, t6105 => 6105, t6106 => 6106, t6107 => 6107, t6108 => 6108, t6109 => 6109, t6110 => 6110, t6111 => 6111, t6112 => 6112, t6113 => 6113, t6114 => 6114, t6115 => 6115, t6116 => 6116, t6117 => 6117, t6118 => 6118, t6119 => 6119, t6120 => 6120, t6121 => 6121, t6122 => 6122, t6123 => 6123, t6124 => 6124, t6125 => 6125, t6126 => 6126, t6127 => 6127, t6128 => 6128, t6129 => 6129, t6130 => 6130, t6131 => 6131, t6132 => 6132, t6133 => 6133, t6134 => 6134, t6135 => 6135, t6136 => 6136, t6137 => 6137, t6138 => 6138, t6139 => 6139, t6140 => 6140, t6141 => 6141, t6142 => 6142, t6143 => 6143, t6144 => 6144, t6145 => 6145, t6146 => 6146, t6147 => 6147, t6148 => 6148, t6149 => 6149, t6150 => 6150, t6151 => 6151, t6152 => 6152, t6153 => 6153, t6154 => 6154, t6155 => 6155, t6156 => 6156, t6157 => 6157, t6158 => 6158, t6159 => 6159, t6160 => 6160, t6161 => 6161, t6162 => 6162, t6163 => 6163, t6164 => 6164, t6165 => 6165, t6166 => 6166, t6167 => 6167, t6168 => 6168, t6169 => 6169, t6170 => 6170, t6171 => 6171, t6172 => 6172, t6173 => 6173, t6174 => 6174, t6175 => 6175, t6176 => 6176, t6177 => 6177, t6178 => 6178, t6179 => 6179, t6180 => 6180, t6181 => 6181, t6182 => 6182, t6183 => 6183, t6184 => 6184, t6185 => 6185, t6186 => 6186, t6187 => 6187, t6188 => 6188, t6189 => 6189, t6190 => 6190, t6191 => 6191, t6192 => 6192, t6193 => 6193, t6194 => 6194, t6195 => 6195, t6196 => 6196, t6197 => 6197, t6198 => 6198, t6199 => 6199, t6200 => 6200, t6201 => 6201, t6202 => 6202, t6203 => 6203, t6204 => 6204, t6205 => 6205, t6206 => 6206, t6207 => 6207, t6208 => 6208, t6209 => 6209, t6210 => 6210, t6211 => 6211, t6212 => 6212, t6213 => 6213, t6214 => 6214, t6215 => 6215, t6216 => 6216, t6217 => 6217, t6218 => 6218, t6219 => 6219, t6220 => 6220, t6221 => 6221, t6222 => 6222, t6223 => 6223, t6224 => 6224, t6225 => 6225, t6226 => 6226, t6227 => 6227, t6228 => 6228, t6229 => 6229, t6230 => 6230, t6231 => 6231, t6232 => 6232, t6233 => 6233, t6234 => 6234, t6235 => 6235, t6236 => 6236, t6237 => 6237, t6238 => 6238, t6239 => 6239, t6240 => 6240, t6241 => 6241, t6242 => 6242, t6243 => 6243, t6244 => 6244, t6245 => 6245, t6246 => 6246, t6247 => 6247, t6248 => 6248, t6249 => 6249, t6250 => 6250, t6251 => 6251, t6252 => 6252, t6253 => 6253, t6254 => 6254, t6255 => 6255, t6256 => 6256, t6257 => 6257, t6258 => 6258, t6259 => 6259, t6260 => 6260, t6261 => 6261, t6262 => 6262, t6263 => 6263, t6264 => 6264, t6265 => 6265, t6266 => 6266, t6267 => 6267, t6268 => 6268, t6269 => 6269, t6270 => 6270, t6271 => 6271, t6272 => 6272, t6273 => 6273, t6274 => 6274, t6275 => 6275, t6276 => 6276, t6277 => 6277, t6278 => 6278, t6279 => 6279, t6280 => 6280, t6281 => 6281, t6282 => 6282, t6283 => 6283, t6284 => 6284, t6285 => 6285, t6286 => 6286, t6287 => 6287, t6288 => 6288, t6289 => 6289, t6290 => 6290, t6291 => 6291, t6292 => 6292, t6293 => 6293, t6294 => 6294, t6295 => 6295, t6296 => 6296, t6297 => 6297, t6298 => 6298, t6299 => 6299, t6300 => 6300, t6301 => 6301, t6302 => 6302, t6303 => 6303, t6304 => 6304, t6305 => 6305, t6306 => 6306, t6307 => 6307, t6308 => 6308, t6309 => 6309, t6310 => 6310, t6311 => 6311, t6312 => 6312, t6313 => 6313, t6314 => 6314, t6315 => 6315, t6316 => 6316, t6317 => 6317, t6318 => 6318, t6319 => 6319, t6320 => 6320, t6321 => 6321, t6322 => 6322, t6323 => 6323, t6324 => 6324, t6325 => 6325, t6326 => 6326, t6327 => 6327, t6328 => 6328, t6329 => 6329, t6330 => 6330, t6331 => 6331, t6332 => 6332, t6333 => 6333, t6334 => 6334, t6335 => 6335, t6336 => 6336, t6337 => 6337, t6338 => 6338, t6339 => 6339, t6340 => 6340, t6341 => 6341, t6342 => 6342, t6343 => 6343, t6344 => 6344, t6345 => 6345, t6346 => 6346, t6347 => 6347, t6348 => 6348, t6349 => 6349, t6350 => 6350, t6351 => 6351, t6352 => 6352, t6353 => 6353, t6354 => 6354, t6355 => 6355, t6356 => 6356, t6357 => 6357, t6358 => 6358, t6359 => 6359, t6360 => 6360, t6361 => 6361, t6362 => 6362, t6363 => 6363, t6364 => 6364, t6365 => 6365, t6366 => 6366, t6367 => 6367, t6368 => 6368, t6369 => 6369, t6370 => 6370, t6371 => 6371, t6372 => 6372, t6373 => 6373, t6374 => 6374, t6375 => 6375, t6376 => 6376, t6377 => 6377, t6378 => 6378, t6379 => 6379, t6380 => 6380, t6381 => 6381, t6382 => 6382, t6383 => 6383, t6384 => 6384, t6385 => 6385, t6386 => 6386, t6387 => 6387, t6388 => 6388, t6389 => 6389, t6390 => 6390, t6391 => 6391, t6392 => 6392, t6393 => 6393, t6394 => 6394, t6395 => 6395, t6396 => 6396, t6397 => 6397, t6398 => 6398, t6399 => 6399, t6400 => 6400, t6401 => 6401, t6402 => 6402, t6403 => 6403, t6404 => 6404, t6405 => 6405, t6406 => 6406, t6407 => 6407, t6408 => 6408, t6409 => 6409, t6410 => 6410, t6411 => 6411, t6412 => 6412, t6413 => 6413, t6414 => 6414, t6415 => 6415, t6416 => 6416, t6417 => 6417, t6418 => 6418, t6419 => 6419, t6420 => 6420, t6421 => 6421, t6422 => 6422, t6423 => 6423, t6424 => 6424, t6425 => 6425, t6426 => 6426, t6427 => 6427, t6428 => 6428, t6429 => 6429, t6430 => 6430, t6431 => 6431, t6432 => 6432, t6433 => 6433, t6434 => 6434, t6435 => 6435, t6436 => 6436, t6437 => 6437, t6438 => 6438, t6439 => 6439, t6440 => 6440, t6441 => 6441, t6442 => 6442, t6443 => 6443, t6444 => 6444, t6445 => 6445, t6446 => 6446, t6447 => 6447, t6448 => 6448, t6449 => 6449, t6450 => 6450, t6451 => 6451, t6452 => 6452, t6453 => 6453, t6454 => 6454, t6455 => 6455, t6456 => 6456, t6457 => 6457, t6458 => 6458, t6459 => 6459, t6460 => 6460, t6461 => 6461, t6462 => 6462, t6463 => 6463, t6464 => 6464, t6465 => 6465, t6466 => 6466, t6467 => 6467, t6468 => 6468, t6469 => 6469, t6470 => 6470, t6471 => 6471, t6472 => 6472, t6473 => 6473, t6474 => 6474, t6475 => 6475, t6476 => 6476, t6477 => 6477, t6478 => 6478, t6479 => 6479, t6480 => 6480, t6481 => 6481, t6482 => 6482, t6483 => 6483, t6484 => 6484, t6485 => 6485, t6486 => 6486, t6487 => 6487, t6488 => 6488, t6489 => 6489, t6490 => 6490, t6491 => 6491, t6492 => 6492, t6493 => 6493, t6494 => 6494, t6495 => 6495, t6496 => 6496, t6497 => 6497, t6498 => 6498, t6499 => 6499, t6500 => 6500, t6501 => 6501, t6502 => 6502, t6503 => 6503, t6504 => 6504, t6505 => 6505, t6506 => 6506, t6507 => 6507, t6508 => 6508, t6509 => 6509, t6510 => 6510, t6511 => 6511, t6512 => 6512, t6513 => 6513, t6514 => 6514, t6515 => 6515, t6516 => 6516, t6517 => 6517, t6518 => 6518, t6519 => 6519, t6520 => 6520, t6521 => 6521, t6522 => 6522, t6523 => 6523, t6524 => 6524, t6525 => 6525, t6526 => 6526, t6527 => 6527, t6528 => 6528, t6529 => 6529, t6530 => 6530, t6531 => 6531, t6532 => 6532, t6533 => 6533, t6534 => 6534, t6535 => 6535, t6536 => 6536, t6537 => 6537, t6538 => 6538, t6539 => 6539, t6540 => 6540, t6541 => 6541, t6542 => 6542, t6543 => 6543, t6544 => 6544, t6545 => 6545, t6546 => 6546, t6547 => 6547, t6548 => 6548, t6549 => 6549, t6550 => 6550, t6551 => 6551, t6552 => 6552, t6553 => 6553, t6554 => 6554, t6555 => 6555, t6556 => 6556, t6557 => 6557, t6558 => 6558, t6559 => 6559, t6560 => 6560, t6561 => 6561, t6562 => 6562, t6563 => 6563, t6564 => 6564, t6565 => 6565, t6566 => 6566, t6567 => 6567, t6568 => 6568, t6569 => 6569, t6570 => 6570, t6571 => 6571, t6572 => 6572, t6573 => 6573, t6574 => 6574, t6575 => 6575, t6576 => 6576, t6577 => 6577, t6578 => 6578, t6579 => 6579, t6580 => 6580, t6581 => 6581, t6582 => 6582, t6583 => 6583, t6584 => 6584, t6585 => 6585, t6586 => 6586, t6587 => 6587, t6588 => 6588, t6589 => 6589, t6590 => 6590, t6591 => 6591, t6592 => 6592, t6593 => 6593, t6594 => 6594, t6595 => 6595, t6596 => 6596, t6597 => 6597, t6598 => 6598, t6599 => 6599, t6600 => 6600, t6601 => 6601, t6602 => 6602, t6603 => 6603, t6604 => 6604, t6605 => 6605, t6606 => 6606, t6607 => 6607, t6608 => 6608, t6609 => 6609, t6610 => 6610, t6611 => 6611, t6612 => 6612, t6613 => 6613, t6614 => 6614, t6615 => 6615, t6616 => 6616, t6617 => 6617, t6618 => 6618, t6619 => 6619, t6620 => 6620, t6621 => 6621, t6622 => 6622, t6623 => 6623, t6624 => 6624, t6625 => 6625, t6626 => 6626, t6627 => 6627, t6628 => 6628, t6629 => 6629, t6630 => 6630, t6631 => 6631, t6632 => 6632, t6633 => 6633, t6634 => 6634, t6635 => 6635, t6636 => 6636, t6637 => 6637, t6638 => 6638, t6639 => 6639, t6640 => 6640, t6641 => 6641, t6642 => 6642, t6643 => 6643, t6644 => 6644, t6645 => 6645, t6646 => 6646, t6647 => 6647, t6648 => 6648, t6649 => 6649, t6650 => 6650, t6651 => 6651, t6652 => 6652, t6653 => 6653, t6654 => 6654, t6655 => 6655, t6656 => 6656, t6657 => 6657, t6658 => 6658, t6659 => 6659, t6660 => 6660, t6661 => 6661, t6662 => 6662, t6663 => 6663, t6664 => 6664, t6665 => 6665, t6666 => 6666, t6667 => 6667, t6668 => 6668, t6669 => 6669, t6670 => 6670, t6671 => 6671, t6672 => 6672, t6673 => 6673, t6674 => 6674, t6675 => 6675, t6676 => 6676, t6677 => 6677, t6678 => 6678, t6679 => 6679, t6680 => 6680, t6681 => 6681, t6682 => 6682, t6683 => 6683, t6684 => 6684, t6685 => 6685, t6686 => 6686, t6687 => 6687, t6688 => 6688, t6689 => 6689, t6690 => 6690, t6691 => 6691, t6692 => 6692, t6693 => 6693, t6694 => 6694, t6695 => 6695, t6696 => 6696, t6697 => 6697, t6698 => 6698, t6699 => 6699, t6700 => 6700, t6701 => 6701, t6702 => 6702, t6703 => 6703, t6704 => 6704, t6705 => 6705, t6706 => 6706, t6707 => 6707, t6708 => 6708, t6709 => 6709, t6710 => 6710, t6711 => 6711, t6712 => 6712, t6713 => 6713, t6714 => 6714, t6715 => 6715, t6716 => 6716, t6717 => 6717, t6718 => 6718, t6719 => 6719, t6720 => 6720, t6721 => 6721, t6722 => 6722, t6723 => 6723, t6724 => 6724, t6725 => 6725, t6726 => 6726, t6727 => 6727, t6728 => 6728, t6729 => 6729, t6730 => 6730, t6731 => 6731, t6732 => 6732, t6733 => 6733, t6734 => 6734, t6735 => 6735, t6736 => 6736, t6737 => 6737, t6738 => 6738, t6739 => 6739, t6740 => 6740, t6741 => 6741, t6742 => 6742, t6743 => 6743, t6744 => 6744, t6745 => 6745, t6746 => 6746, t6747 => 6747, t6748 => 6748, t6749 => 6749, t6750 => 6750, t6751 => 6751, t6752 => 6752, t6753 => 6753, t6754 => 6754, t6755 => 6755, t6756 => 6756, t6757 => 6757, t6758 => 6758, t6759 => 6759, t6760 => 6760, t6761 => 6761, t6762 => 6762, t6763 => 6763, t6764 => 6764, t6765 => 6765, t6766 => 6766, t6767 => 6767, t6768 => 6768, t6769 => 6769, t6770 => 6770, t6771 => 6771, t6772 => 6772, t6773 => 6773, t6774 => 6774, t6775 => 6775, t6776 => 6776, t6777 => 6777, t6778 => 6778, t6779 => 6779, t6780 => 6780, t6781 => 6781, t6782 => 6782, t6783 => 6783, t6784 => 6784, t6785 => 6785, t6786 => 6786, t6787 => 6787, t6788 => 6788, t6789 => 6789, t6790 => 6790, t6791 => 6791, t6792 => 6792, t6793 => 6793, t6794 => 6794, t6795 => 6795, t6796 => 6796, t6797 => 6797, t6798 => 6798, t6799 => 6799, t6800 => 6800, t6801 => 6801, t6802 => 6802, t6803 => 6803, t6804 => 6804, t6805 => 6805, t6806 => 6806, t6807 => 6807, t6808 => 6808, t6809 => 6809, t6810 => 6810, t6811 => 6811, t6812 => 6812, t6813 => 6813, t6814 => 6814, t6815 => 6815, t6816 => 6816, t6817 => 6817, t6818 => 6818, t6819 => 6819, t6820 => 6820, t6821 => 6821, t6822 => 6822, t6823 => 6823, t6824 => 6824, t6825 => 6825, t6826 => 6826, t6827 => 6827, t6828 => 6828, t6829 => 6829, t6830 => 6830, t6831 => 6831, t6832 => 6832, t6833 => 6833, t6834 => 6834, t6835 => 6835, t6836 => 6836, t6837 => 6837, t6838 => 6838, t6839 => 6839, t6840 => 6840, t6841 => 6841, t6842 => 6842, t6843 => 6843, t6844 => 6844, t6845 => 6845, t6846 => 6846, t6847 => 6847, t6848 => 6848, t6849 => 6849, t6850 => 6850, t6851 => 6851, t6852 => 6852, t6853 => 6853, t6854 => 6854, t6855 => 6855, t6856 => 6856, t6857 => 6857, t6858 => 6858, t6859 => 6859, t6860 => 6860, t6861 => 6861, t6862 => 6862, t6863 => 6863, t6864 => 6864, t6865 => 6865, t6866 => 6866, t6867 => 6867, t6868 => 6868, t6869 => 6869, t6870 => 6870, t6871 => 6871, t6872 => 6872, t6873 => 6873, t6874 => 6874, t6875 => 6875, t6876 => 6876, t6877 => 6877, t6878 => 6878, t6879 => 6879, t6880 => 6880, t6881 => 6881, t6882 => 6882, t6883 => 6883, t6884 => 6884, t6885 => 6885, t6886 => 6886, t6887 => 6887, t6888 => 6888, t6889 => 6889, t6890 => 6890, t6891 => 6891, t6892 => 6892, t6893 => 6893, t6894 => 6894, t6895 => 6895, t6896 => 6896, t6897 => 6897, t6898 => 6898, t6899 => 6899, t6900 => 6900, t6901 => 6901, t6902 => 6902, t6903 => 6903, t6904 => 6904, t6905 => 6905, t6906 => 6906, t6907 => 6907, t6908 => 6908, t6909 => 6909, t6910 => 6910, t6911 => 6911, t6912 => 6912, t6913 => 6913, t6914 => 6914, t6915 => 6915, t6916 => 6916, t6917 => 6917, t6918 => 6918, t6919 => 6919, t6920 => 6920, t6921 => 6921, t6922 => 6922, t6923 => 6923, t6924 => 6924, t6925 => 6925, t6926 => 6926, t6927 => 6927, t6928 => 6928, t6929 => 6929, t6930 => 6930, t6931 => 6931, t6932 => 6932, t6933 => 6933, t6934 => 6934, t6935 => 6935, t6936 => 6936, t6937 => 6937, t6938 => 6938, t6939 => 6939, t6940 => 6940, t6941 => 6941, t6942 => 6942, t6943 => 6943, t6944 => 6944, t6945 => 6945, t6946 => 6946, t6947 => 6947, t6948 => 6948, t6949 => 6949, t6950 => 6950, t6951 => 6951, t6952 => 6952, t6953 => 6953, t6954 => 6954, t6955 => 6955, t6956 => 6956, t6957 => 6957, t6958 => 6958, t6959 => 6959, t6960 => 6960, t6961 => 6961, t6962 => 6962, t6963 => 6963, t6964 => 6964, t6965 => 6965, t6966 => 6966, t6967 => 6967, t6968 => 6968, t6969 => 6969, t6970 => 6970, t6971 => 6971, t6972 => 6972, t6973 => 6973, t6974 => 6974, t6975 => 6975, t6976 => 6976, t6977 => 6977, t6978 => 6978, t6979 => 6979, t6980 => 6980, t6981 => 6981, t6982 => 6982, t6983 => 6983, t6984 => 6984, t6985 => 6985, t6986 => 6986, t6987 => 6987, t6988 => 6988, t6989 => 6989, t6990 => 6990, t6991 => 6991, t6992 => 6992, t6993 => 6993, t6994 => 6994, t6995 => 6995, t6996 => 6996, t6997 => 6997, t6998 => 6998, t6999 => 6999, t7000 => 7000, t7001 => 7001, t7002 => 7002, t7003 => 7003, t7004 => 7004, t7005 => 7005, t7006 => 7006, t7007 => 7007, t7008 => 7008, t7009 => 7009, t7010 => 7010, t7011 => 7011, t7012 => 7012, t7013 => 7013, t7014 => 7014, t7015 => 7015, t7016 => 7016, t7017 => 7017, t7018 => 7018, t7019 => 7019, t7020 => 7020, t7021 => 7021, t7022 => 7022, t7023 => 7023, t7024 => 7024, t7025 => 7025, t7026 => 7026, t7027 => 7027, t7028 => 7028, t7029 => 7029, t7030 => 7030, t7031 => 7031, t7032 => 7032, t7033 => 7033, t7034 => 7034, t7035 => 7035, t7036 => 7036, t7037 => 7037, t7038 => 7038, t7039 => 7039, t7040 => 7040, t7041 => 7041, t7042 => 7042, t7043 => 7043, t7044 => 7044, t7045 => 7045, t7046 => 7046, t7047 => 7047, t7048 => 7048, t7049 => 7049, t7050 => 7050, t7051 => 7051, t7052 => 7052, t7053 => 7053, t7054 => 7054, t7055 => 7055, t7056 => 7056, t7057 => 7057, t7058 => 7058, t7059 => 7059, t7060 => 7060, t7061 => 7061, t7062 => 7062, t7063 => 7063, t7064 => 7064, t7065 => 7065, t7066 => 7066, t7067 => 7067, t7068 => 7068, t7069 => 7069, t7070 => 7070, t7071 => 7071, t7072 => 7072, t7073 => 7073, t7074 => 7074, t7075 => 7075, t7076 => 7076, t7077 => 7077, t7078 => 7078, t7079 => 7079, t7080 => 7080, t7081 => 7081, t7082 => 7082, t7083 => 7083, t7084 => 7084, t7085 => 7085, t7086 => 7086, t7087 => 7087, t7088 => 7088, t7089 => 7089, t7090 => 7090, t7091 => 7091, t7092 => 7092, t7093 => 7093, t7094 => 7094, t7095 => 7095, t7096 => 7096, t7097 => 7097, t7098 => 7098, t7099 => 7099, t7100 => 7100, t7101 => 7101, t7102 => 7102, t7103 => 7103, t7104 => 7104, t7105 => 7105, t7106 => 7106, t7107 => 7107, t7108 => 7108, t7109 => 7109, t7110 => 7110, t7111 => 7111, t7112 => 7112, t7113 => 7113, t7114 => 7114, t7115 => 7115, t7116 => 7116, t7117 => 7117, t7118 => 7118, t7119 => 7119, t7120 => 7120, t7121 => 7121, t7122 => 7122, t7123 => 7123, t7124 => 7124, t7125 => 7125, t7126 => 7126, t7127 => 7127, t7128 => 7128, t7129 => 7129, t7130 => 7130, t7131 => 7131, t7132 => 7132, t7133 => 7133, t7134 => 7134, t7135 => 7135, t7136 => 7136, t7137 => 7137, t7138 => 7138, t7139 => 7139, t7140 => 7140, t7141 => 7141, t7142 => 7142, t7143 => 7143, t7144 => 7144, t7145 => 7145, t7146 => 7146, t7147 => 7147, t7148 => 7148, t7149 => 7149, t7150 => 7150, t7151 => 7151, t7152 => 7152, t7153 => 7153, t7154 => 7154, t7155 => 7155, t7156 => 7156, t7157 => 7157, t7158 => 7158, t7159 => 7159, t7160 => 7160, t7161 => 7161, t7162 => 7162, t7163 => 7163, t7164 => 7164, t7165 => 7165, t7166 => 7166, t7167 => 7167, t7168 => 7168, t7169 => 7169, t7170 => 7170, t7171 => 7171, t7172 => 7172, t7173 => 7173, t7174 => 7174, t7175 => 7175, t7176 => 7176, t7177 => 7177, t7178 => 7178, t7179 => 7179, t7180 => 7180, t7181 => 7181, t7182 => 7182, t7183 => 7183, t7184 => 7184, t7185 => 7185, t7186 => 7186, t7187 => 7187, t7188 => 7188, t7189 => 7189, t7190 => 7190, t7191 => 7191, t7192 => 7192, t7193 => 7193, t7194 => 7194, t7195 => 7195, t7196 => 7196, t7197 => 7197, t7198 => 7198, t7199 => 7199, t7200 => 7200, t7201 => 7201, t7202 => 7202, t7203 => 7203, t7204 => 7204, t7205 => 7205, t7206 => 7206, t7207 => 7207, t7208 => 7208, t7209 => 7209, t7210 => 7210, t7211 => 7211, t7212 => 7212, t7213 => 7213, t7214 => 7214, t7215 => 7215, t7216 => 7216, t7217 => 7217, t7218 => 7218, t7219 => 7219, t7220 => 7220, t7221 => 7221, t7222 => 7222, t7223 => 7223, t7224 => 7224, t7225 => 7225, t7226 => 7226, t7227 => 7227, t7228 => 7228, t7229 => 7229, t7230 => 7230, t7231 => 7231, t7232 => 7232, t7233 => 7233, t7234 => 7234, t7235 => 7235, t7236 => 7236, t7237 => 7237, t7238 => 7238, t7239 => 7239, t7240 => 7240, t7241 => 7241, t7242 => 7242, t7243 => 7243, t7244 => 7244, t7245 => 7245, t7246 => 7246, t7247 => 7247, t7248 => 7248, t7249 => 7249, t7250 => 7250, t7251 => 7251, t7252 => 7252, t7253 => 7253, t7254 => 7254, t7255 => 7255, t7256 => 7256, t7257 => 7257, t7258 => 7258, t7259 => 7259, t7260 => 7260, t7261 => 7261, t7262 => 7262, t7263 => 7263, t7264 => 7264, t7265 => 7265, t7266 => 7266, t7267 => 7267, t7268 => 7268, t7269 => 7269, t7270 => 7270, t7271 => 7271, t7272 => 7272, t7273 => 7273, t7274 => 7274, t7275 => 7275, t7276 => 7276, t7277 => 7277, t7278 => 7278, t7279 => 7279, t7280 => 7280, t7281 => 7281, t7282 => 7282, t7283 => 7283, t7284 => 7284, t7285 => 7285, t7286 => 7286, t7287 => 7287, t7288 => 7288, t7289 => 7289, t7290 => 7290, t7291 => 7291, t7292 => 7292, t7293 => 7293, t7294 => 7294, t7295 => 7295, t7296 => 7296, t7297 => 7297, t7298 => 7298, t7299 => 7299, t7300 => 7300, t7301 => 7301, t7302 => 7302, t7303 => 7303, t7304 => 7304, t7305 => 7305, t7306 => 7306, t7307 => 7307, t7308 => 7308, t7309 => 7309, t7310 => 7310, t7311 => 7311, t7312 => 7312, t7313 => 7313, t7314 => 7314, t7315 => 7315, t7316 => 7316, t7317 => 7317, t7318 => 7318, t7319 => 7319, t7320 => 7320, t7321 => 7321, t7322 => 7322, t7323 => 7323, t7324 => 7324, t7325 => 7325, t7326 => 7326, t7327 => 7327, t7328 => 7328, t7329 => 7329, t7330 => 7330, t7331 => 7331, t7332 => 7332, t7333 => 7333, t7334 => 7334, t7335 => 7335, t7336 => 7336, t7337 => 7337, t7338 => 7338, t7339 => 7339, t7340 => 7340, t7341 => 7341, t7342 => 7342, t7343 => 7343, t7344 => 7344, t7345 => 7345, t7346 => 7346, t7347 => 7347, t7348 => 7348, t7349 => 7349, t7350 => 7350, t7351 => 7351, t7352 => 7352, t7353 => 7353, t7354 => 7354, t7355 => 7355, t7356 => 7356, t7357 => 7357, t7358 => 7358, t7359 => 7359, t7360 => 7360, t7361 => 7361, t7362 => 7362, t7363 => 7363, t7364 => 7364, t7365 => 7365, t7366 => 7366, t7367 => 7367, t7368 => 7368, t7369 => 7369, t7370 => 7370, t7371 => 7371, t7372 => 7372, t7373 => 7373, t7374 => 7374, t7375 => 7375, t7376 => 7376, t7377 => 7377, t7378 => 7378, t7379 => 7379, t7380 => 7380, t7381 => 7381, t7382 => 7382, t7383 => 7383, t7384 => 7384, t7385 => 7385, t7386 => 7386, t7387 => 7387, t7388 => 7388, t7389 => 7389, t7390 => 7390, t7391 => 7391, t7392 => 7392, t7393 => 7393, t7394 => 7394, t7395 => 7395, t7396 => 7396, t7397 => 7397, t7398 => 7398, t7399 => 7399, t7400 => 7400, t7401 => 7401, t7402 => 7402, t7403 => 7403, t7404 => 7404, t7405 => 7405, t7406 => 7406, t7407 => 7407, t7408 => 7408, t7409 => 7409, t7410 => 7410, t7411 => 7411, t7412 => 7412, t7413 => 7413, t7414 => 7414, t7415 => 7415, t7416 => 7416, t7417 => 7417, t7418 => 7418, t7419 => 7419, t7420 => 7420, t7421 => 7421, t7422 => 7422, t7423 => 7423, t7424 => 7424, t7425 => 7425, t7426 => 7426, t7427 => 7427, t7428 => 7428, t7429 => 7429, t7430 => 7430, t7431 => 7431, t7432 => 7432, t7433 => 7433, t7434 => 7434, t7435 => 7435, t7436 => 7436, t7437 => 7437, t7438 => 7438, t7439 => 7439, t7440 => 7440, t7441 => 7441, t7442 => 7442, t7443 => 7443, t7444 => 7444, t7445 => 7445, t7446 => 7446, t7447 => 7447, t7448 => 7448, t7449 => 7449, t7450 => 7450, t7451 => 7451, t7452 => 7452, t7453 => 7453, t7454 => 7454, t7455 => 7455, t7456 => 7456, t7457 => 7457, t7458 => 7458, t7459 => 7459, t7460 => 7460, t7461 => 7461, t7462 => 7462, t7463 => 7463, t7464 => 7464, t7465 => 7465, t7466 => 7466, t7467 => 7467, t7468 => 7468, t7469 => 7469, t7470 => 7470, t7471 => 7471, t7472 => 7472, t7473 => 7473, t7474 => 7474, t7475 => 7475, t7476 => 7476, t7477 => 7477, t7478 => 7478, t7479 => 7479, t7480 => 7480, t7481 => 7481, t7482 => 7482, t7483 => 7483, t7484 => 7484, t7485 => 7485, t7486 => 7486, t7487 => 7487, t7488 => 7488, t7489 => 7489, t7490 => 7490, t7491 => 7491, t7492 => 7492, t7493 => 7493, t7494 => 7494, t7495 => 7495, t7496 => 7496, t7497 => 7497, t7498 => 7498, t7499 => 7499, t7500 => 7500, t7501 => 7501, t7502 => 7502, t7503 => 7503, t7504 => 7504, t7505 => 7505, t7506 => 7506, t7507 => 7507, t7508 => 7508, t7509 => 7509, t7510 => 7510, t7511 => 7511, t7512 => 7512, t7513 => 7513, t7514 => 7514, t7515 => 7515, t7516 => 7516, t7517 => 7517, t7518 => 7518, t7519 => 7519, t7520 => 7520, t7521 => 7521, t7522 => 7522, t7523 => 7523, t7524 => 7524, t7525 => 7525, t7526 => 7526, t7527 => 7527, t7528 => 7528, t7529 => 7529, t7530 => 7530, t7531 => 7531, t7532 => 7532, t7533 => 7533, t7534 => 7534, t7535 => 7535, t7536 => 7536, t7537 => 7537, t7538 => 7538, t7539 => 7539, t7540 => 7540, t7541 => 7541, t7542 => 7542, t7543 => 7543, t7544 => 7544, t7545 => 7545, t7546 => 7546, t7547 => 7547, t7548 => 7548, t7549 => 7549, t7550 => 7550, t7551 => 7551, t7552 => 7552, t7553 => 7553, t7554 => 7554, t7555 => 7555, t7556 => 7556, t7557 => 7557, t7558 => 7558, t7559 => 7559, t7560 => 7560, t7561 => 7561, t7562 => 7562, t7563 => 7563, t7564 => 7564, t7565 => 7565, t7566 => 7566, t7567 => 7567, t7568 => 7568, t7569 => 7569, t7570 => 7570, t7571 => 7571, t7572 => 7572, t7573 => 7573, t7574 => 7574, t7575 => 7575, t7576 => 7576, t7577 => 7577, t7578 => 7578, t7579 => 7579, t7580 => 7580, t7581 => 7581, t7582 => 7582, t7583 => 7583, t7584 => 7584, t7585 => 7585, t7586 => 7586, t7587 => 7587, t7588 => 7588, t7589 => 7589, t7590 => 7590, t7591 => 7591, t7592 => 7592, t7593 => 7593, t7594 => 7594, t7595 => 7595, t7596 => 7596, t7597 => 7597, t7598 => 7598, t7599 => 7599, t7600 => 7600, t7601 => 7601, t7602 => 7602, t7603 => 7603, t7604 => 7604, t7605 => 7605, t7606 => 7606, t7607 => 7607, t7608 => 7608, t7609 => 7609, t7610 => 7610, t7611 => 7611, t7612 => 7612, t7613 => 7613, t7614 => 7614, t7615 => 7615, t7616 => 7616, t7617 => 7617, t7618 => 7618, t7619 => 7619, t7620 => 7620, t7621 => 7621, t7622 => 7622, t7623 => 7623, t7624 => 7624, t7625 => 7625, t7626 => 7626, t7627 => 7627, t7628 => 7628, t7629 => 7629, t7630 => 7630, t7631 => 7631, t7632 => 7632, t7633 => 7633, t7634 => 7634, t7635 => 7635, t7636 => 7636, t7637 => 7637, t7638 => 7638, t7639 => 7639, t7640 => 7640, t7641 => 7641, t7642 => 7642, t7643 => 7643, t7644 => 7644, t7645 => 7645, t7646 => 7646, t7647 => 7647, t7648 => 7648, t7649 => 7649, t7650 => 7650, t7651 => 7651, t7652 => 7652, t7653 => 7653, t7654 => 7654, t7655 => 7655, t7656 => 7656, t7657 => 7657, t7658 => 7658, t7659 => 7659, t7660 => 7660, t7661 => 7661, t7662 => 7662, t7663 => 7663, t7664 => 7664, t7665 => 7665, t7666 => 7666, t7667 => 7667, t7668 => 7668, t7669 => 7669, t7670 => 7670, t7671 => 7671, t7672 => 7672, t7673 => 7673, t7674 => 7674, t7675 => 7675, t7676 => 7676, t7677 => 7677, t7678 => 7678, t7679 => 7679, t7680 => 7680, t7681 => 7681, t7682 => 7682, t7683 => 7683, t7684 => 7684, t7685 => 7685, t7686 => 7686, t7687 => 7687, t7688 => 7688, t7689 => 7689, t7690 => 7690, t7691 => 7691, t7692 => 7692, t7693 => 7693, t7694 => 7694, t7695 => 7695, t7696 => 7696, t7697 => 7697, t7698 => 7698, t7699 => 7699, t7700 => 7700, t7701 => 7701, t7702 => 7702, t7703 => 7703, t7704 => 7704, t7705 => 7705, t7706 => 7706, t7707 => 7707, t7708 => 7708, t7709 => 7709, t7710 => 7710, t7711 => 7711, t7712 => 7712, t7713 => 7713, t7714 => 7714, t7715 => 7715, t7716 => 7716, t7717 => 7717, t7718 => 7718, t7719 => 7719, t7720 => 7720, t7721 => 7721, t7722 => 7722, t7723 => 7723, t7724 => 7724, t7725 => 7725, t7726 => 7726, t7727 => 7727, t7728 => 7728, t7729 => 7729, t7730 => 7730, t7731 => 7731, t7732 => 7732, t7733 => 7733, t7734 => 7734, t7735 => 7735, t7736 => 7736, t7737 => 7737, t7738 => 7738, t7739 => 7739, t7740 => 7740, t7741 => 7741, t7742 => 7742, t7743 => 7743, t7744 => 7744, t7745 => 7745, t7746 => 7746, t7747 => 7747, t7748 => 7748, t7749 => 7749, t7750 => 7750, t7751 => 7751, t7752 => 7752, t7753 => 7753, t7754 => 7754, t7755 => 7755, t7756 => 7756, t7757 => 7757, t7758 => 7758, t7759 => 7759, t7760 => 7760, t7761 => 7761, t7762 => 7762, t7763 => 7763, t7764 => 7764, t7765 => 7765, t7766 => 7766, t7767 => 7767, t7768 => 7768, t7769 => 7769, t7770 => 7770, t7771 => 7771, t7772 => 7772, t7773 => 7773, t7774 => 7774, t7775 => 7775, t7776 => 7776, t7777 => 7777, t7778 => 7778, t7779 => 7779, t7780 => 7780, t7781 => 7781, t7782 => 7782, t7783 => 7783, t7784 => 7784, t7785 => 7785, t7786 => 7786, t7787 => 7787, t7788 => 7788, t7789 => 7789, t7790 => 7790, t7791 => 7791, t7792 => 7792, t7793 => 7793, t7794 => 7794, t7795 => 7795, t7796 => 7796, t7797 => 7797, t7798 => 7798, t7799 => 7799, t7800 => 7800, t7801 => 7801, t7802 => 7802, t7803 => 7803, t7804 => 7804, t7805 => 7805, t7806 => 7806, t7807 => 7807, t7808 => 7808, t7809 => 7809, t7810 => 7810, t7811 => 7811, t7812 => 7812, t7813 => 7813, t7814 => 7814, t7815 => 7815, t7816 => 7816, t7817 => 7817, t7818 => 7818, t7819 => 7819, t7820 => 7820, t7821 => 7821, t7822 => 7822, t7823 => 7823, t7824 => 7824, t7825 => 7825, t7826 => 7826, t7827 => 7827, t7828 => 7828, t7829 => 7829, t7830 => 7830, t7831 => 7831, t7832 => 7832, t7833 => 7833, t7834 => 7834, t7835 => 7835, t7836 => 7836, t7837 => 7837, t7838 => 7838, t7839 => 7839, t7840 => 7840, t7841 => 7841, t7842 => 7842, t7843 => 7843, t7844 => 7844, t7845 => 7845, t7846 => 7846, t7847 => 7847, t7848 => 7848, t7849 => 7849, t7850 => 7850, t7851 => 7851, t7852 => 7852, t7853 => 7853, t7854 => 7854, t7855 => 7855, t7856 => 7856, t7857 => 7857, t7858 => 7858, t7859 => 7859, t7860 => 7860, t7861 => 7861, t7862 => 7862, t7863 => 7863, t7864 => 7864, t7865 => 7865, t7866 => 7866, t7867 => 7867, t7868 => 7868, t7869 => 7869, t7870 => 7870, t7871 => 7871, t7872 => 7872, t7873 => 7873, t7874 => 7874, t7875 => 7875, t7876 => 7876, t7877 => 7877, t7878 => 7878, t7879 => 7879, t7880 => 7880, t7881 => 7881, t7882 => 7882, t7883 => 7883, t7884 => 7884, t7885 => 7885, t7886 => 7886, t7887 => 7887, t7888 => 7888, t7889 => 7889, t7890 => 7890, t7891 => 7891, t7892 => 7892, t7893 => 7893, t7894 => 7894, t7895 => 7895, t7896 => 7896, t7897 => 7897, t7898 => 7898, t7899 => 7899, t7900 => 7900, t7901 => 7901, t7902 => 7902, t7903 => 7903, t7904 => 7904, t7905 => 7905, t7906 => 7906, t7907 => 7907, t7908 => 7908, t7909 => 7909, t7910 => 7910, t7911 => 7911, t7912 => 7912, t7913 => 7913, t7914 => 7914, t7915 => 7915, t7916 => 7916, t7917 => 7917, t7918 => 7918, t7919 => 7919, t7920 => 7920, t7921 => 7921, t7922 => 7922, t7923 => 7923, t7924 => 7924, t7925 => 7925, t7926 => 7926, t7927 => 7927, t7928 => 7928, t7929 => 7929, t7930 => 7930, t7931 => 7931, t7932 => 7932, t7933 => 7933, t7934 => 7934, t7935 => 7935, t7936 => 7936, t7937 => 7937, t7938 => 7938, t7939 => 7939, t7940 => 7940, t7941 => 7941, t7942 => 7942, t7943 => 7943, t7944 => 7944, t7945 => 7945, t7946 => 7946, t7947 => 7947, t7948 => 7948, t7949 => 7949, t7950 => 7950, t7951 => 7951, t7952 => 7952, t7953 => 7953, t7954 => 7954, t7955 => 7955, t7956 => 7956, t7957 => 7957, t7958 => 7958, t7959 => 7959, t7960 => 7960, t7961 => 7961, t7962 => 7962, t7963 => 7963, t7964 => 7964, t7965 => 7965, t7966 => 7966, t7967 => 7967, t7968 => 7968, t7969 => 7969, t7970 => 7970, t7971 => 7971, t7972 => 7972, t7973 => 7973, t7974 => 7974, t7975 => 7975, t7976 => 7976, t7977 => 7977, t7978 => 7978, t7979 => 7979, t7980 => 7980, t7981 => 7981, t7982 => 7982, t7983 => 7983, t7984 => 7984, t7985 => 7985, t7986 => 7986, t7987 => 7987, t7988 => 7988, t7989 => 7989, t7990 => 7990, t7991 => 7991, t7992 => 7992, t7993 => 7993, t7994 => 7994, t7995 => 7995, t7996 => 7996, t7997 => 7997, t7998 => 7998, t7999 => 7999, t8000 => 8000, t8001 => 8001, t8002 => 8002, t8003 => 8003, t8004 => 8004, t8005 => 8005, t8006 => 8006, t8007 => 8007, t8008 => 8008, t8009 => 8009, t8010 => 8010, t8011 => 8011, t8012 => 8012, t8013 => 8013, t8014 => 8014, t8015 => 8015, t8016 => 8016, t8017 => 8017, t8018 => 8018, t8019 => 8019, t8020 => 8020, t8021 => 8021, t8022 => 8022, t8023 => 8023, t8024 => 8024, t8025 => 8025, t8026 => 8026, t8027 => 8027, t8028 => 8028, t8029 => 8029, t8030 => 8030, t8031 => 8031, t8032 => 8032, t8033 => 8033, t8034 => 8034, t8035 => 8035, t8036 => 8036, t8037 => 8037, t8038 => 8038, t8039 => 8039, t8040 => 8040, t8041 => 8041, t8042 => 8042, t8043 => 8043, t8044 => 8044, t8045 => 8045, t8046 => 8046, t8047 => 8047, t8048 => 8048, t8049 => 8049, t8050 => 8050, t8051 => 8051, t8052 => 8052, t8053 => 8053, t8054 => 8054, t8055 => 8055, t8056 => 8056, t8057 => 8057, t8058 => 8058, t8059 => 8059, t8060 => 8060, t8061 => 8061, t8062 => 8062, t8063 => 8063, t8064 => 8064, t8065 => 8065, t8066 => 8066, t8067 => 8067, t8068 => 8068, t8069 => 8069, t8070 => 8070, t8071 => 8071, t8072 => 8072, t8073 => 8073, t8074 => 8074, t8075 => 8075, t8076 => 8076, t8077 => 8077, t8078 => 8078, t8079 => 8079, t8080 => 8080, t8081 => 8081, t8082 => 8082, t8083 => 8083, t8084 => 8084, t8085 => 8085, t8086 => 8086, t8087 => 8087, t8088 => 8088, t8089 => 8089, t8090 => 8090, t8091 => 8091, t8092 => 8092, t8093 => 8093, t8094 => 8094, t8095 => 8095, t8096 => 8096, t8097 => 8097, t8098 => 8098, t8099 => 8099, t8100 => 8100, t8101 => 8101, t8102 => 8102, t8103 => 8103, t8104 => 8104, t8105 => 8105, t8106 => 8106, t8107 => 8107, t8108 => 8108, t8109 => 8109, t8110 => 8110, t8111 => 8111, t8112 => 8112, t8113 => 8113, t8114 => 8114, t8115 => 8115, t8116 => 8116, t8117 => 8117, t8118 => 8118, t8119 => 8119, t8120 => 8120, t8121 => 8121, t8122 => 8122, t8123 => 8123, t8124 => 8124, t8125 => 8125, t8126 => 8126, t8127 => 8127, t8128 => 8128, t8129 => 8129, t8130 => 8130, t8131 => 8131, t8132 => 8132, t8133 => 8133, t8134 => 8134, t8135 => 8135, t8136 => 8136, t8137 => 8137, t8138 => 8138, t8139 => 8139, t8140 => 8140, t8141 => 8141, t8142 => 8142, t8143 => 8143, t8144 => 8144, t8145 => 8145, t8146 => 8146, t8147 => 8147, t8148 => 8148, t8149 => 8149, t8150 => 8150, t8151 => 8151, t8152 => 8152, t8153 => 8153, t8154 => 8154, t8155 => 8155, t8156 => 8156, t8157 => 8157, t8158 => 8158, t8159 => 8159, t8160 => 8160, t8161 => 8161, t8162 => 8162, t8163 => 8163, t8164 => 8164, t8165 => 8165, t8166 => 8166, t8167 => 8167, t8168 => 8168, t8169 => 8169, t8170 => 8170, t8171 => 8171, t8172 => 8172, t8173 => 8173, t8174 => 8174, t8175 => 8175, t8176 => 8176, t8177 => 8177, t8178 => 8178, t8179 => 8179, t8180 => 8180, t8181 => 8181, t8182 => 8182, t8183 => 8183, t8184 => 8184, t8185 => 8185, t8186 => 8186, t8187 => 8187, t8188 => 8188, t8189 => 8189, t8190 => 8190, t8191 => 8191);

const TRAMPS: [SubrFn; 8192] = [
    t0, t1, t2, t3, t4, t5, t6, t7, t8, t9, t10, t11, t12, t13, t14, t15, t16, t17, t18, t19, t20,
    t21, t22, t23, t24, t25, t26, t27, t28, t29, t30, t31, t32, t33, t34, t35, t36, t37, t38, t39,
    t40, t41, t42, t43, t44, t45, t46, t47, t48, t49, t50, t51, t52, t53, t54, t55, t56, t57, t58,
    t59, t60, t61, t62, t63, t64, t65, t66, t67, t68, t69, t70, t71, t72, t73, t74, t75, t76, t77,
    t78, t79, t80, t81, t82, t83, t84, t85, t86, t87, t88, t89, t90, t91, t92, t93, t94, t95, t96,
    t97, t98, t99, t100, t101, t102, t103, t104, t105, t106, t107, t108, t109, t110, t111, t112,
    t113, t114, t115, t116, t117, t118, t119, t120, t121, t122, t123, t124, t125, t126, t127, t128,
    t129, t130, t131, t132, t133, t134, t135, t136, t137, t138, t139, t140, t141, t142, t143, t144,
    t145, t146, t147, t148, t149, t150, t151, t152, t153, t154, t155, t156, t157, t158, t159, t160,
    t161, t162, t163, t164, t165, t166, t167, t168, t169, t170, t171, t172, t173, t174, t175, t176,
    t177, t178, t179, t180, t181, t182, t183, t184, t185, t186, t187, t188, t189, t190, t191, t192,
    t193, t194, t195, t196, t197, t198, t199, t200, t201, t202, t203, t204, t205, t206, t207, t208,
    t209, t210, t211, t212, t213, t214, t215, t216, t217, t218, t219, t220, t221, t222, t223, t224,
    t225, t226, t227, t228, t229, t230, t231, t232, t233, t234, t235, t236, t237, t238, t239, t240,
    t241, t242, t243, t244, t245, t246, t247, t248, t249, t250, t251, t252, t253, t254, t255, t256,
    t257, t258, t259, t260, t261, t262, t263, t264, t265, t266, t267, t268, t269, t270, t271, t272,
    t273, t274, t275, t276, t277, t278, t279, t280, t281, t282, t283, t284, t285, t286, t287, t288,
    t289, t290, t291, t292, t293, t294, t295, t296, t297, t298, t299, t300, t301, t302, t303, t304,
    t305, t306, t307, t308, t309, t310, t311, t312, t313, t314, t315, t316, t317, t318, t319, t320,
    t321, t322, t323, t324, t325, t326, t327, t328, t329, t330, t331, t332, t333, t334, t335, t336,
    t337, t338, t339, t340, t341, t342, t343, t344, t345, t346, t347, t348, t349, t350, t351, t352,
    t353, t354, t355, t356, t357, t358, t359, t360, t361, t362, t363, t364, t365, t366, t367, t368,
    t369, t370, t371, t372, t373, t374, t375, t376, t377, t378, t379, t380, t381, t382, t383, t384,
    t385, t386, t387, t388, t389, t390, t391, t392, t393, t394, t395, t396, t397, t398, t399, t400,
    t401, t402, t403, t404, t405, t406, t407, t408, t409, t410, t411, t412, t413, t414, t415, t416,
    t417, t418, t419, t420, t421, t422, t423, t424, t425, t426, t427, t428, t429, t430, t431, t432,
    t433, t434, t435, t436, t437, t438, t439, t440, t441, t442, t443, t444, t445, t446, t447, t448,
    t449, t450, t451, t452, t453, t454, t455, t456, t457, t458, t459, t460, t461, t462, t463, t464,
    t465, t466, t467, t468, t469, t470, t471, t472, t473, t474, t475, t476, t477, t478, t479, t480,
    t481, t482, t483, t484, t485, t486, t487, t488, t489, t490, t491, t492, t493, t494, t495, t496,
    t497, t498, t499, t500, t501, t502, t503, t504, t505, t506, t507, t508, t509, t510, t511, t512,
    t513, t514, t515, t516, t517, t518, t519, t520, t521, t522, t523, t524, t525, t526, t527, t528,
    t529, t530, t531, t532, t533, t534, t535, t536, t537, t538, t539, t540, t541, t542, t543, t544,
    t545, t546, t547, t548, t549, t550, t551, t552, t553, t554, t555, t556, t557, t558, t559, t560,
    t561, t562, t563, t564, t565, t566, t567, t568, t569, t570, t571, t572, t573, t574, t575, t576,
    t577, t578, t579, t580, t581, t582, t583, t584, t585, t586, t587, t588, t589, t590, t591, t592,
    t593, t594, t595, t596, t597, t598, t599, t600, t601, t602, t603, t604, t605, t606, t607, t608,
    t609, t610, t611, t612, t613, t614, t615, t616, t617, t618, t619, t620, t621, t622, t623, t624,
    t625, t626, t627, t628, t629, t630, t631, t632, t633, t634, t635, t636, t637, t638, t639, t640,
    t641, t642, t643, t644, t645, t646, t647, t648, t649, t650, t651, t652, t653, t654, t655, t656,
    t657, t658, t659, t660, t661, t662, t663, t664, t665, t666, t667, t668, t669, t670, t671, t672,
    t673, t674, t675, t676, t677, t678, t679, t680, t681, t682, t683, t684, t685, t686, t687, t688,
    t689, t690, t691, t692, t693, t694, t695, t696, t697, t698, t699, t700, t701, t702, t703, t704,
    t705, t706, t707, t708, t709, t710, t711, t712, t713, t714, t715, t716, t717, t718, t719, t720,
    t721, t722, t723, t724, t725, t726, t727, t728, t729, t730, t731, t732, t733, t734, t735, t736,
    t737, t738, t739, t740, t741, t742, t743, t744, t745, t746, t747, t748, t749, t750, t751, t752,
    t753, t754, t755, t756, t757, t758, t759, t760, t761, t762, t763, t764, t765, t766, t767, t768,
    t769, t770, t771, t772, t773, t774, t775, t776, t777, t778, t779, t780, t781, t782, t783, t784,
    t785, t786, t787, t788, t789, t790, t791, t792, t793, t794, t795, t796, t797, t798, t799, t800,
    t801, t802, t803, t804, t805, t806, t807, t808, t809, t810, t811, t812, t813, t814, t815, t816,
    t817, t818, t819, t820, t821, t822, t823, t824, t825, t826, t827, t828, t829, t830, t831, t832,
    t833, t834, t835, t836, t837, t838, t839, t840, t841, t842, t843, t844, t845, t846, t847, t848,
    t849, t850, t851, t852, t853, t854, t855, t856, t857, t858, t859, t860, t861, t862, t863, t864,
    t865, t866, t867, t868, t869, t870, t871, t872, t873, t874, t875, t876, t877, t878, t879, t880,
    t881, t882, t883, t884, t885, t886, t887, t888, t889, t890, t891, t892, t893, t894, t895, t896,
    t897, t898, t899, t900, t901, t902, t903, t904, t905, t906, t907, t908, t909, t910, t911, t912,
    t913, t914, t915, t916, t917, t918, t919, t920, t921, t922, t923, t924, t925, t926, t927, t928,
    t929, t930, t931, t932, t933, t934, t935, t936, t937, t938, t939, t940, t941, t942, t943, t944,
    t945, t946, t947, t948, t949, t950, t951, t952, t953, t954, t955, t956, t957, t958, t959, t960,
    t961, t962, t963, t964, t965, t966, t967, t968, t969, t970, t971, t972, t973, t974, t975, t976,
    t977, t978, t979, t980, t981, t982, t983, t984, t985, t986, t987, t988, t989, t990, t991, t992,
    t993, t994, t995, t996, t997, t998, t999, t1000, t1001, t1002, t1003, t1004, t1005, t1006,
    t1007, t1008, t1009, t1010, t1011, t1012, t1013, t1014, t1015, t1016, t1017, t1018, t1019,
    t1020, t1021, t1022, t1023, t1024, t1025, t1026, t1027, t1028, t1029, t1030, t1031, t1032,
    t1033, t1034, t1035, t1036, t1037, t1038, t1039, t1040, t1041, t1042, t1043, t1044, t1045,
    t1046, t1047, t1048, t1049, t1050, t1051, t1052, t1053, t1054, t1055, t1056, t1057, t1058,
    t1059, t1060, t1061, t1062, t1063, t1064, t1065, t1066, t1067, t1068, t1069, t1070, t1071,
    t1072, t1073, t1074, t1075, t1076, t1077, t1078, t1079, t1080, t1081, t1082, t1083, t1084,
    t1085, t1086, t1087, t1088, t1089, t1090, t1091, t1092, t1093, t1094, t1095, t1096, t1097,
    t1098, t1099, t1100, t1101, t1102, t1103, t1104, t1105, t1106, t1107, t1108, t1109, t1110,
    t1111, t1112, t1113, t1114, t1115, t1116, t1117, t1118, t1119, t1120, t1121, t1122, t1123,
    t1124, t1125, t1126, t1127, t1128, t1129, t1130, t1131, t1132, t1133, t1134, t1135, t1136,
    t1137, t1138, t1139, t1140, t1141, t1142, t1143, t1144, t1145, t1146, t1147, t1148, t1149,
    t1150, t1151, t1152, t1153, t1154, t1155, t1156, t1157, t1158, t1159, t1160, t1161, t1162,
    t1163, t1164, t1165, t1166, t1167, t1168, t1169, t1170, t1171, t1172, t1173, t1174, t1175,
    t1176, t1177, t1178, t1179, t1180, t1181, t1182, t1183, t1184, t1185, t1186, t1187, t1188,
    t1189, t1190, t1191, t1192, t1193, t1194, t1195, t1196, t1197, t1198, t1199, t1200, t1201,
    t1202, t1203, t1204, t1205, t1206, t1207, t1208, t1209, t1210, t1211, t1212, t1213, t1214,
    t1215, t1216, t1217, t1218, t1219, t1220, t1221, t1222, t1223, t1224, t1225, t1226, t1227,
    t1228, t1229, t1230, t1231, t1232, t1233, t1234, t1235, t1236, t1237, t1238, t1239, t1240,
    t1241, t1242, t1243, t1244, t1245, t1246, t1247, t1248, t1249, t1250, t1251, t1252, t1253,
    t1254, t1255, t1256, t1257, t1258, t1259, t1260, t1261, t1262, t1263, t1264, t1265, t1266,
    t1267, t1268, t1269, t1270, t1271, t1272, t1273, t1274, t1275, t1276, t1277, t1278, t1279,
    t1280, t1281, t1282, t1283, t1284, t1285, t1286, t1287, t1288, t1289, t1290, t1291, t1292,
    t1293, t1294, t1295, t1296, t1297, t1298, t1299, t1300, t1301, t1302, t1303, t1304, t1305,
    t1306, t1307, t1308, t1309, t1310, t1311, t1312, t1313, t1314, t1315, t1316, t1317, t1318,
    t1319, t1320, t1321, t1322, t1323, t1324, t1325, t1326, t1327, t1328, t1329, t1330, t1331,
    t1332, t1333, t1334, t1335, t1336, t1337, t1338, t1339, t1340, t1341, t1342, t1343, t1344,
    t1345, t1346, t1347, t1348, t1349, t1350, t1351, t1352, t1353, t1354, t1355, t1356, t1357,
    t1358, t1359, t1360, t1361, t1362, t1363, t1364, t1365, t1366, t1367, t1368, t1369, t1370,
    t1371, t1372, t1373, t1374, t1375, t1376, t1377, t1378, t1379, t1380, t1381, t1382, t1383,
    t1384, t1385, t1386, t1387, t1388, t1389, t1390, t1391, t1392, t1393, t1394, t1395, t1396,
    t1397, t1398, t1399, t1400, t1401, t1402, t1403, t1404, t1405, t1406, t1407, t1408, t1409,
    t1410, t1411, t1412, t1413, t1414, t1415, t1416, t1417, t1418, t1419, t1420, t1421, t1422,
    t1423, t1424, t1425, t1426, t1427, t1428, t1429, t1430, t1431, t1432, t1433, t1434, t1435,
    t1436, t1437, t1438, t1439, t1440, t1441, t1442, t1443, t1444, t1445, t1446, t1447, t1448,
    t1449, t1450, t1451, t1452, t1453, t1454, t1455, t1456, t1457, t1458, t1459, t1460, t1461,
    t1462, t1463, t1464, t1465, t1466, t1467, t1468, t1469, t1470, t1471, t1472, t1473, t1474,
    t1475, t1476, t1477, t1478, t1479, t1480, t1481, t1482, t1483, t1484, t1485, t1486, t1487,
    t1488, t1489, t1490, t1491, t1492, t1493, t1494, t1495, t1496, t1497, t1498, t1499, t1500,
    t1501, t1502, t1503, t1504, t1505, t1506, t1507, t1508, t1509, t1510, t1511, t1512, t1513,
    t1514, t1515, t1516, t1517, t1518, t1519, t1520, t1521, t1522, t1523, t1524, t1525, t1526,
    t1527, t1528, t1529, t1530, t1531, t1532, t1533, t1534, t1535, t1536, t1537, t1538, t1539,
    t1540, t1541, t1542, t1543, t1544, t1545, t1546, t1547, t1548, t1549, t1550, t1551, t1552,
    t1553, t1554, t1555, t1556, t1557, t1558, t1559, t1560, t1561, t1562, t1563, t1564, t1565,
    t1566, t1567, t1568, t1569, t1570, t1571, t1572, t1573, t1574, t1575, t1576, t1577, t1578,
    t1579, t1580, t1581, t1582, t1583, t1584, t1585, t1586, t1587, t1588, t1589, t1590, t1591,
    t1592, t1593, t1594, t1595, t1596, t1597, t1598, t1599, t1600, t1601, t1602, t1603, t1604,
    t1605, t1606, t1607, t1608, t1609, t1610, t1611, t1612, t1613, t1614, t1615, t1616, t1617,
    t1618, t1619, t1620, t1621, t1622, t1623, t1624, t1625, t1626, t1627, t1628, t1629, t1630,
    t1631, t1632, t1633, t1634, t1635, t1636, t1637, t1638, t1639, t1640, t1641, t1642, t1643,
    t1644, t1645, t1646, t1647, t1648, t1649, t1650, t1651, t1652, t1653, t1654, t1655, t1656,
    t1657, t1658, t1659, t1660, t1661, t1662, t1663, t1664, t1665, t1666, t1667, t1668, t1669,
    t1670, t1671, t1672, t1673, t1674, t1675, t1676, t1677, t1678, t1679, t1680, t1681, t1682,
    t1683, t1684, t1685, t1686, t1687, t1688, t1689, t1690, t1691, t1692, t1693, t1694, t1695,
    t1696, t1697, t1698, t1699, t1700, t1701, t1702, t1703, t1704, t1705, t1706, t1707, t1708,
    t1709, t1710, t1711, t1712, t1713, t1714, t1715, t1716, t1717, t1718, t1719, t1720, t1721,
    t1722, t1723, t1724, t1725, t1726, t1727, t1728, t1729, t1730, t1731, t1732, t1733, t1734,
    t1735, t1736, t1737, t1738, t1739, t1740, t1741, t1742, t1743, t1744, t1745, t1746, t1747,
    t1748, t1749, t1750, t1751, t1752, t1753, t1754, t1755, t1756, t1757, t1758, t1759, t1760,
    t1761, t1762, t1763, t1764, t1765, t1766, t1767, t1768, t1769, t1770, t1771, t1772, t1773,
    t1774, t1775, t1776, t1777, t1778, t1779, t1780, t1781, t1782, t1783, t1784, t1785, t1786,
    t1787, t1788, t1789, t1790, t1791, t1792, t1793, t1794, t1795, t1796, t1797, t1798, t1799,
    t1800, t1801, t1802, t1803, t1804, t1805, t1806, t1807, t1808, t1809, t1810, t1811, t1812,
    t1813, t1814, t1815, t1816, t1817, t1818, t1819, t1820, t1821, t1822, t1823, t1824, t1825,
    t1826, t1827, t1828, t1829, t1830, t1831, t1832, t1833, t1834, t1835, t1836, t1837, t1838,
    t1839, t1840, t1841, t1842, t1843, t1844, t1845, t1846, t1847, t1848, t1849, t1850, t1851,
    t1852, t1853, t1854, t1855, t1856, t1857, t1858, t1859, t1860, t1861, t1862, t1863, t1864,
    t1865, t1866, t1867, t1868, t1869, t1870, t1871, t1872, t1873, t1874, t1875, t1876, t1877,
    t1878, t1879, t1880, t1881, t1882, t1883, t1884, t1885, t1886, t1887, t1888, t1889, t1890,
    t1891, t1892, t1893, t1894, t1895, t1896, t1897, t1898, t1899, t1900, t1901, t1902, t1903,
    t1904, t1905, t1906, t1907, t1908, t1909, t1910, t1911, t1912, t1913, t1914, t1915, t1916,
    t1917, t1918, t1919, t1920, t1921, t1922, t1923, t1924, t1925, t1926, t1927, t1928, t1929,
    t1930, t1931, t1932, t1933, t1934, t1935, t1936, t1937, t1938, t1939, t1940, t1941, t1942,
    t1943, t1944, t1945, t1946, t1947, t1948, t1949, t1950, t1951, t1952, t1953, t1954, t1955,
    t1956, t1957, t1958, t1959, t1960, t1961, t1962, t1963, t1964, t1965, t1966, t1967, t1968,
    t1969, t1970, t1971, t1972, t1973, t1974, t1975, t1976, t1977, t1978, t1979, t1980, t1981,
    t1982, t1983, t1984, t1985, t1986, t1987, t1988, t1989, t1990, t1991, t1992, t1993, t1994,
    t1995, t1996, t1997, t1998, t1999, t2000, t2001, t2002, t2003, t2004, t2005, t2006, t2007,
    t2008, t2009, t2010, t2011, t2012, t2013, t2014, t2015, t2016, t2017, t2018, t2019, t2020,
    t2021, t2022, t2023, t2024, t2025, t2026, t2027, t2028, t2029, t2030, t2031, t2032, t2033,
    t2034, t2035, t2036, t2037, t2038, t2039, t2040, t2041, t2042, t2043, t2044, t2045, t2046,
    t2047, t2048, t2049, t2050, t2051, t2052, t2053, t2054, t2055, t2056, t2057, t2058, t2059,
    t2060, t2061, t2062, t2063, t2064, t2065, t2066, t2067, t2068, t2069, t2070, t2071, t2072,
    t2073, t2074, t2075, t2076, t2077, t2078, t2079, t2080, t2081, t2082, t2083, t2084, t2085,
    t2086, t2087, t2088, t2089, t2090, t2091, t2092, t2093, t2094, t2095, t2096, t2097, t2098,
    t2099, t2100, t2101, t2102, t2103, t2104, t2105, t2106, t2107, t2108, t2109, t2110, t2111,
    t2112, t2113, t2114, t2115, t2116, t2117, t2118, t2119, t2120, t2121, t2122, t2123, t2124,
    t2125, t2126, t2127, t2128, t2129, t2130, t2131, t2132, t2133, t2134, t2135, t2136, t2137,
    t2138, t2139, t2140, t2141, t2142, t2143, t2144, t2145, t2146, t2147, t2148, t2149, t2150,
    t2151, t2152, t2153, t2154, t2155, t2156, t2157, t2158, t2159, t2160, t2161, t2162, t2163,
    t2164, t2165, t2166, t2167, t2168, t2169, t2170, t2171, t2172, t2173, t2174, t2175, t2176,
    t2177, t2178, t2179, t2180, t2181, t2182, t2183, t2184, t2185, t2186, t2187, t2188, t2189,
    t2190, t2191, t2192, t2193, t2194, t2195, t2196, t2197, t2198, t2199, t2200, t2201, t2202,
    t2203, t2204, t2205, t2206, t2207, t2208, t2209, t2210, t2211, t2212, t2213, t2214, t2215,
    t2216, t2217, t2218, t2219, t2220, t2221, t2222, t2223, t2224, t2225, t2226, t2227, t2228,
    t2229, t2230, t2231, t2232, t2233, t2234, t2235, t2236, t2237, t2238, t2239, t2240, t2241,
    t2242, t2243, t2244, t2245, t2246, t2247, t2248, t2249, t2250, t2251, t2252, t2253, t2254,
    t2255, t2256, t2257, t2258, t2259, t2260, t2261, t2262, t2263, t2264, t2265, t2266, t2267,
    t2268, t2269, t2270, t2271, t2272, t2273, t2274, t2275, t2276, t2277, t2278, t2279, t2280,
    t2281, t2282, t2283, t2284, t2285, t2286, t2287, t2288, t2289, t2290, t2291, t2292, t2293,
    t2294, t2295, t2296, t2297, t2298, t2299, t2300, t2301, t2302, t2303, t2304, t2305, t2306,
    t2307, t2308, t2309, t2310, t2311, t2312, t2313, t2314, t2315, t2316, t2317, t2318, t2319,
    t2320, t2321, t2322, t2323, t2324, t2325, t2326, t2327, t2328, t2329, t2330, t2331, t2332,
    t2333, t2334, t2335, t2336, t2337, t2338, t2339, t2340, t2341, t2342, t2343, t2344, t2345,
    t2346, t2347, t2348, t2349, t2350, t2351, t2352, t2353, t2354, t2355, t2356, t2357, t2358,
    t2359, t2360, t2361, t2362, t2363, t2364, t2365, t2366, t2367, t2368, t2369, t2370, t2371,
    t2372, t2373, t2374, t2375, t2376, t2377, t2378, t2379, t2380, t2381, t2382, t2383, t2384,
    t2385, t2386, t2387, t2388, t2389, t2390, t2391, t2392, t2393, t2394, t2395, t2396, t2397,
    t2398, t2399, t2400, t2401, t2402, t2403, t2404, t2405, t2406, t2407, t2408, t2409, t2410,
    t2411, t2412, t2413, t2414, t2415, t2416, t2417, t2418, t2419, t2420, t2421, t2422, t2423,
    t2424, t2425, t2426, t2427, t2428, t2429, t2430, t2431, t2432, t2433, t2434, t2435, t2436,
    t2437, t2438, t2439, t2440, t2441, t2442, t2443, t2444, t2445, t2446, t2447, t2448, t2449,
    t2450, t2451, t2452, t2453, t2454, t2455, t2456, t2457, t2458, t2459, t2460, t2461, t2462,
    t2463, t2464, t2465, t2466, t2467, t2468, t2469, t2470, t2471, t2472, t2473, t2474, t2475,
    t2476, t2477, t2478, t2479, t2480, t2481, t2482, t2483, t2484, t2485, t2486, t2487, t2488,
    t2489, t2490, t2491, t2492, t2493, t2494, t2495, t2496, t2497, t2498, t2499, t2500, t2501,
    t2502, t2503, t2504, t2505, t2506, t2507, t2508, t2509, t2510, t2511, t2512, t2513, t2514,
    t2515, t2516, t2517, t2518, t2519, t2520, t2521, t2522, t2523, t2524, t2525, t2526, t2527,
    t2528, t2529, t2530, t2531, t2532, t2533, t2534, t2535, t2536, t2537, t2538, t2539, t2540,
    t2541, t2542, t2543, t2544, t2545, t2546, t2547, t2548, t2549, t2550, t2551, t2552, t2553,
    t2554, t2555, t2556, t2557, t2558, t2559, t2560, t2561, t2562, t2563, t2564, t2565, t2566,
    t2567, t2568, t2569, t2570, t2571, t2572, t2573, t2574, t2575, t2576, t2577, t2578, t2579,
    t2580, t2581, t2582, t2583, t2584, t2585, t2586, t2587, t2588, t2589, t2590, t2591, t2592,
    t2593, t2594, t2595, t2596, t2597, t2598, t2599, t2600, t2601, t2602, t2603, t2604, t2605,
    t2606, t2607, t2608, t2609, t2610, t2611, t2612, t2613, t2614, t2615, t2616, t2617, t2618,
    t2619, t2620, t2621, t2622, t2623, t2624, t2625, t2626, t2627, t2628, t2629, t2630, t2631,
    t2632, t2633, t2634, t2635, t2636, t2637, t2638, t2639, t2640, t2641, t2642, t2643, t2644,
    t2645, t2646, t2647, t2648, t2649, t2650, t2651, t2652, t2653, t2654, t2655, t2656, t2657,
    t2658, t2659, t2660, t2661, t2662, t2663, t2664, t2665, t2666, t2667, t2668, t2669, t2670,
    t2671, t2672, t2673, t2674, t2675, t2676, t2677, t2678, t2679, t2680, t2681, t2682, t2683,
    t2684, t2685, t2686, t2687, t2688, t2689, t2690, t2691, t2692, t2693, t2694, t2695, t2696,
    t2697, t2698, t2699, t2700, t2701, t2702, t2703, t2704, t2705, t2706, t2707, t2708, t2709,
    t2710, t2711, t2712, t2713, t2714, t2715, t2716, t2717, t2718, t2719, t2720, t2721, t2722,
    t2723, t2724, t2725, t2726, t2727, t2728, t2729, t2730, t2731, t2732, t2733, t2734, t2735,
    t2736, t2737, t2738, t2739, t2740, t2741, t2742, t2743, t2744, t2745, t2746, t2747, t2748,
    t2749, t2750, t2751, t2752, t2753, t2754, t2755, t2756, t2757, t2758, t2759, t2760, t2761,
    t2762, t2763, t2764, t2765, t2766, t2767, t2768, t2769, t2770, t2771, t2772, t2773, t2774,
    t2775, t2776, t2777, t2778, t2779, t2780, t2781, t2782, t2783, t2784, t2785, t2786, t2787,
    t2788, t2789, t2790, t2791, t2792, t2793, t2794, t2795, t2796, t2797, t2798, t2799, t2800,
    t2801, t2802, t2803, t2804, t2805, t2806, t2807, t2808, t2809, t2810, t2811, t2812, t2813,
    t2814, t2815, t2816, t2817, t2818, t2819, t2820, t2821, t2822, t2823, t2824, t2825, t2826,
    t2827, t2828, t2829, t2830, t2831, t2832, t2833, t2834, t2835, t2836, t2837, t2838, t2839,
    t2840, t2841, t2842, t2843, t2844, t2845, t2846, t2847, t2848, t2849, t2850, t2851, t2852,
    t2853, t2854, t2855, t2856, t2857, t2858, t2859, t2860, t2861, t2862, t2863, t2864, t2865,
    t2866, t2867, t2868, t2869, t2870, t2871, t2872, t2873, t2874, t2875, t2876, t2877, t2878,
    t2879, t2880, t2881, t2882, t2883, t2884, t2885, t2886, t2887, t2888, t2889, t2890, t2891,
    t2892, t2893, t2894, t2895, t2896, t2897, t2898, t2899, t2900, t2901, t2902, t2903, t2904,
    t2905, t2906, t2907, t2908, t2909, t2910, t2911, t2912, t2913, t2914, t2915, t2916, t2917,
    t2918, t2919, t2920, t2921, t2922, t2923, t2924, t2925, t2926, t2927, t2928, t2929, t2930,
    t2931, t2932, t2933, t2934, t2935, t2936, t2937, t2938, t2939, t2940, t2941, t2942, t2943,
    t2944, t2945, t2946, t2947, t2948, t2949, t2950, t2951, t2952, t2953, t2954, t2955, t2956,
    t2957, t2958, t2959, t2960, t2961, t2962, t2963, t2964, t2965, t2966, t2967, t2968, t2969,
    t2970, t2971, t2972, t2973, t2974, t2975, t2976, t2977, t2978, t2979, t2980, t2981, t2982,
    t2983, t2984, t2985, t2986, t2987, t2988, t2989, t2990, t2991, t2992, t2993, t2994, t2995,
    t2996, t2997, t2998, t2999, t3000, t3001, t3002, t3003, t3004, t3005, t3006, t3007, t3008,
    t3009, t3010, t3011, t3012, t3013, t3014, t3015, t3016, t3017, t3018, t3019, t3020, t3021,
    t3022, t3023, t3024, t3025, t3026, t3027, t3028, t3029, t3030, t3031, t3032, t3033, t3034,
    t3035, t3036, t3037, t3038, t3039, t3040, t3041, t3042, t3043, t3044, t3045, t3046, t3047,
    t3048, t3049, t3050, t3051, t3052, t3053, t3054, t3055, t3056, t3057, t3058, t3059, t3060,
    t3061, t3062, t3063, t3064, t3065, t3066, t3067, t3068, t3069, t3070, t3071, t3072, t3073,
    t3074, t3075, t3076, t3077, t3078, t3079, t3080, t3081, t3082, t3083, t3084, t3085, t3086,
    t3087, t3088, t3089, t3090, t3091, t3092, t3093, t3094, t3095, t3096, t3097, t3098, t3099,
    t3100, t3101, t3102, t3103, t3104, t3105, t3106, t3107, t3108, t3109, t3110, t3111, t3112,
    t3113, t3114, t3115, t3116, t3117, t3118, t3119, t3120, t3121, t3122, t3123, t3124, t3125,
    t3126, t3127, t3128, t3129, t3130, t3131, t3132, t3133, t3134, t3135, t3136, t3137, t3138,
    t3139, t3140, t3141, t3142, t3143, t3144, t3145, t3146, t3147, t3148, t3149, t3150, t3151,
    t3152, t3153, t3154, t3155, t3156, t3157, t3158, t3159, t3160, t3161, t3162, t3163, t3164,
    t3165, t3166, t3167, t3168, t3169, t3170, t3171, t3172, t3173, t3174, t3175, t3176, t3177,
    t3178, t3179, t3180, t3181, t3182, t3183, t3184, t3185, t3186, t3187, t3188, t3189, t3190,
    t3191, t3192, t3193, t3194, t3195, t3196, t3197, t3198, t3199, t3200, t3201, t3202, t3203,
    t3204, t3205, t3206, t3207, t3208, t3209, t3210, t3211, t3212, t3213, t3214, t3215, t3216,
    t3217, t3218, t3219, t3220, t3221, t3222, t3223, t3224, t3225, t3226, t3227, t3228, t3229,
    t3230, t3231, t3232, t3233, t3234, t3235, t3236, t3237, t3238, t3239, t3240, t3241, t3242,
    t3243, t3244, t3245, t3246, t3247, t3248, t3249, t3250, t3251, t3252, t3253, t3254, t3255,
    t3256, t3257, t3258, t3259, t3260, t3261, t3262, t3263, t3264, t3265, t3266, t3267, t3268,
    t3269, t3270, t3271, t3272, t3273, t3274, t3275, t3276, t3277, t3278, t3279, t3280, t3281,
    t3282, t3283, t3284, t3285, t3286, t3287, t3288, t3289, t3290, t3291, t3292, t3293, t3294,
    t3295, t3296, t3297, t3298, t3299, t3300, t3301, t3302, t3303, t3304, t3305, t3306, t3307,
    t3308, t3309, t3310, t3311, t3312, t3313, t3314, t3315, t3316, t3317, t3318, t3319, t3320,
    t3321, t3322, t3323, t3324, t3325, t3326, t3327, t3328, t3329, t3330, t3331, t3332, t3333,
    t3334, t3335, t3336, t3337, t3338, t3339, t3340, t3341, t3342, t3343, t3344, t3345, t3346,
    t3347, t3348, t3349, t3350, t3351, t3352, t3353, t3354, t3355, t3356, t3357, t3358, t3359,
    t3360, t3361, t3362, t3363, t3364, t3365, t3366, t3367, t3368, t3369, t3370, t3371, t3372,
    t3373, t3374, t3375, t3376, t3377, t3378, t3379, t3380, t3381, t3382, t3383, t3384, t3385,
    t3386, t3387, t3388, t3389, t3390, t3391, t3392, t3393, t3394, t3395, t3396, t3397, t3398,
    t3399, t3400, t3401, t3402, t3403, t3404, t3405, t3406, t3407, t3408, t3409, t3410, t3411,
    t3412, t3413, t3414, t3415, t3416, t3417, t3418, t3419, t3420, t3421, t3422, t3423, t3424,
    t3425, t3426, t3427, t3428, t3429, t3430, t3431, t3432, t3433, t3434, t3435, t3436, t3437,
    t3438, t3439, t3440, t3441, t3442, t3443, t3444, t3445, t3446, t3447, t3448, t3449, t3450,
    t3451, t3452, t3453, t3454, t3455, t3456, t3457, t3458, t3459, t3460, t3461, t3462, t3463,
    t3464, t3465, t3466, t3467, t3468, t3469, t3470, t3471, t3472, t3473, t3474, t3475, t3476,
    t3477, t3478, t3479, t3480, t3481, t3482, t3483, t3484, t3485, t3486, t3487, t3488, t3489,
    t3490, t3491, t3492, t3493, t3494, t3495, t3496, t3497, t3498, t3499, t3500, t3501, t3502,
    t3503, t3504, t3505, t3506, t3507, t3508, t3509, t3510, t3511, t3512, t3513, t3514, t3515,
    t3516, t3517, t3518, t3519, t3520, t3521, t3522, t3523, t3524, t3525, t3526, t3527, t3528,
    t3529, t3530, t3531, t3532, t3533, t3534, t3535, t3536, t3537, t3538, t3539, t3540, t3541,
    t3542, t3543, t3544, t3545, t3546, t3547, t3548, t3549, t3550, t3551, t3552, t3553, t3554,
    t3555, t3556, t3557, t3558, t3559, t3560, t3561, t3562, t3563, t3564, t3565, t3566, t3567,
    t3568, t3569, t3570, t3571, t3572, t3573, t3574, t3575, t3576, t3577, t3578, t3579, t3580,
    t3581, t3582, t3583, t3584, t3585, t3586, t3587, t3588, t3589, t3590, t3591, t3592, t3593,
    t3594, t3595, t3596, t3597, t3598, t3599, t3600, t3601, t3602, t3603, t3604, t3605, t3606,
    t3607, t3608, t3609, t3610, t3611, t3612, t3613, t3614, t3615, t3616, t3617, t3618, t3619,
    t3620, t3621, t3622, t3623, t3624, t3625, t3626, t3627, t3628, t3629, t3630, t3631, t3632,
    t3633, t3634, t3635, t3636, t3637, t3638, t3639, t3640, t3641, t3642, t3643, t3644, t3645,
    t3646, t3647, t3648, t3649, t3650, t3651, t3652, t3653, t3654, t3655, t3656, t3657, t3658,
    t3659, t3660, t3661, t3662, t3663, t3664, t3665, t3666, t3667, t3668, t3669, t3670, t3671,
    t3672, t3673, t3674, t3675, t3676, t3677, t3678, t3679, t3680, t3681, t3682, t3683, t3684,
    t3685, t3686, t3687, t3688, t3689, t3690, t3691, t3692, t3693, t3694, t3695, t3696, t3697,
    t3698, t3699, t3700, t3701, t3702, t3703, t3704, t3705, t3706, t3707, t3708, t3709, t3710,
    t3711, t3712, t3713, t3714, t3715, t3716, t3717, t3718, t3719, t3720, t3721, t3722, t3723,
    t3724, t3725, t3726, t3727, t3728, t3729, t3730, t3731, t3732, t3733, t3734, t3735, t3736,
    t3737, t3738, t3739, t3740, t3741, t3742, t3743, t3744, t3745, t3746, t3747, t3748, t3749,
    t3750, t3751, t3752, t3753, t3754, t3755, t3756, t3757, t3758, t3759, t3760, t3761, t3762,
    t3763, t3764, t3765, t3766, t3767, t3768, t3769, t3770, t3771, t3772, t3773, t3774, t3775,
    t3776, t3777, t3778, t3779, t3780, t3781, t3782, t3783, t3784, t3785, t3786, t3787, t3788,
    t3789, t3790, t3791, t3792, t3793, t3794, t3795, t3796, t3797, t3798, t3799, t3800, t3801,
    t3802, t3803, t3804, t3805, t3806, t3807, t3808, t3809, t3810, t3811, t3812, t3813, t3814,
    t3815, t3816, t3817, t3818, t3819, t3820, t3821, t3822, t3823, t3824, t3825, t3826, t3827,
    t3828, t3829, t3830, t3831, t3832, t3833, t3834, t3835, t3836, t3837, t3838, t3839, t3840,
    t3841, t3842, t3843, t3844, t3845, t3846, t3847, t3848, t3849, t3850, t3851, t3852, t3853,
    t3854, t3855, t3856, t3857, t3858, t3859, t3860, t3861, t3862, t3863, t3864, t3865, t3866,
    t3867, t3868, t3869, t3870, t3871, t3872, t3873, t3874, t3875, t3876, t3877, t3878, t3879,
    t3880, t3881, t3882, t3883, t3884, t3885, t3886, t3887, t3888, t3889, t3890, t3891, t3892,
    t3893, t3894, t3895, t3896, t3897, t3898, t3899, t3900, t3901, t3902, t3903, t3904, t3905,
    t3906, t3907, t3908, t3909, t3910, t3911, t3912, t3913, t3914, t3915, t3916, t3917, t3918,
    t3919, t3920, t3921, t3922, t3923, t3924, t3925, t3926, t3927, t3928, t3929, t3930, t3931,
    t3932, t3933, t3934, t3935, t3936, t3937, t3938, t3939, t3940, t3941, t3942, t3943, t3944,
    t3945, t3946, t3947, t3948, t3949, t3950, t3951, t3952, t3953, t3954, t3955, t3956, t3957,
    t3958, t3959, t3960, t3961, t3962, t3963, t3964, t3965, t3966, t3967, t3968, t3969, t3970,
    t3971, t3972, t3973, t3974, t3975, t3976, t3977, t3978, t3979, t3980, t3981, t3982, t3983,
    t3984, t3985, t3986, t3987, t3988, t3989, t3990, t3991, t3992, t3993, t3994, t3995, t3996,
    t3997, t3998, t3999, t4000, t4001, t4002, t4003, t4004, t4005, t4006, t4007, t4008, t4009,
    t4010, t4011, t4012, t4013, t4014, t4015, t4016, t4017, t4018, t4019, t4020, t4021, t4022,
    t4023, t4024, t4025, t4026, t4027, t4028, t4029, t4030, t4031, t4032, t4033, t4034, t4035,
    t4036, t4037, t4038, t4039, t4040, t4041, t4042, t4043, t4044, t4045, t4046, t4047, t4048,
    t4049, t4050, t4051, t4052, t4053, t4054, t4055, t4056, t4057, t4058, t4059, t4060, t4061,
    t4062, t4063, t4064, t4065, t4066, t4067, t4068, t4069, t4070, t4071, t4072, t4073, t4074,
    t4075, t4076, t4077, t4078, t4079, t4080, t4081, t4082, t4083, t4084, t4085, t4086, t4087,
    t4088, t4089, t4090, t4091, t4092, t4093, t4094, t4095, t4096, t4097, t4098, t4099, t4100,
    t4101, t4102, t4103, t4104, t4105, t4106, t4107, t4108, t4109, t4110, t4111, t4112, t4113,
    t4114, t4115, t4116, t4117, t4118, t4119, t4120, t4121, t4122, t4123, t4124, t4125, t4126,
    t4127, t4128, t4129, t4130, t4131, t4132, t4133, t4134, t4135, t4136, t4137, t4138, t4139,
    t4140, t4141, t4142, t4143, t4144, t4145, t4146, t4147, t4148, t4149, t4150, t4151, t4152,
    t4153, t4154, t4155, t4156, t4157, t4158, t4159, t4160, t4161, t4162, t4163, t4164, t4165,
    t4166, t4167, t4168, t4169, t4170, t4171, t4172, t4173, t4174, t4175, t4176, t4177, t4178,
    t4179, t4180, t4181, t4182, t4183, t4184, t4185, t4186, t4187, t4188, t4189, t4190, t4191,
    t4192, t4193, t4194, t4195, t4196, t4197, t4198, t4199, t4200, t4201, t4202, t4203, t4204,
    t4205, t4206, t4207, t4208, t4209, t4210, t4211, t4212, t4213, t4214, t4215, t4216, t4217,
    t4218, t4219, t4220, t4221, t4222, t4223, t4224, t4225, t4226, t4227, t4228, t4229, t4230,
    t4231, t4232, t4233, t4234, t4235, t4236, t4237, t4238, t4239, t4240, t4241, t4242, t4243,
    t4244, t4245, t4246, t4247, t4248, t4249, t4250, t4251, t4252, t4253, t4254, t4255, t4256,
    t4257, t4258, t4259, t4260, t4261, t4262, t4263, t4264, t4265, t4266, t4267, t4268, t4269,
    t4270, t4271, t4272, t4273, t4274, t4275, t4276, t4277, t4278, t4279, t4280, t4281, t4282,
    t4283, t4284, t4285, t4286, t4287, t4288, t4289, t4290, t4291, t4292, t4293, t4294, t4295,
    t4296, t4297, t4298, t4299, t4300, t4301, t4302, t4303, t4304, t4305, t4306, t4307, t4308,
    t4309, t4310, t4311, t4312, t4313, t4314, t4315, t4316, t4317, t4318, t4319, t4320, t4321,
    t4322, t4323, t4324, t4325, t4326, t4327, t4328, t4329, t4330, t4331, t4332, t4333, t4334,
    t4335, t4336, t4337, t4338, t4339, t4340, t4341, t4342, t4343, t4344, t4345, t4346, t4347,
    t4348, t4349, t4350, t4351, t4352, t4353, t4354, t4355, t4356, t4357, t4358, t4359, t4360,
    t4361, t4362, t4363, t4364, t4365, t4366, t4367, t4368, t4369, t4370, t4371, t4372, t4373,
    t4374, t4375, t4376, t4377, t4378, t4379, t4380, t4381, t4382, t4383, t4384, t4385, t4386,
    t4387, t4388, t4389, t4390, t4391, t4392, t4393, t4394, t4395, t4396, t4397, t4398, t4399,
    t4400, t4401, t4402, t4403, t4404, t4405, t4406, t4407, t4408, t4409, t4410, t4411, t4412,
    t4413, t4414, t4415, t4416, t4417, t4418, t4419, t4420, t4421, t4422, t4423, t4424, t4425,
    t4426, t4427, t4428, t4429, t4430, t4431, t4432, t4433, t4434, t4435, t4436, t4437, t4438,
    t4439, t4440, t4441, t4442, t4443, t4444, t4445, t4446, t4447, t4448, t4449, t4450, t4451,
    t4452, t4453, t4454, t4455, t4456, t4457, t4458, t4459, t4460, t4461, t4462, t4463, t4464,
    t4465, t4466, t4467, t4468, t4469, t4470, t4471, t4472, t4473, t4474, t4475, t4476, t4477,
    t4478, t4479, t4480, t4481, t4482, t4483, t4484, t4485, t4486, t4487, t4488, t4489, t4490,
    t4491, t4492, t4493, t4494, t4495, t4496, t4497, t4498, t4499, t4500, t4501, t4502, t4503,
    t4504, t4505, t4506, t4507, t4508, t4509, t4510, t4511, t4512, t4513, t4514, t4515, t4516,
    t4517, t4518, t4519, t4520, t4521, t4522, t4523, t4524, t4525, t4526, t4527, t4528, t4529,
    t4530, t4531, t4532, t4533, t4534, t4535, t4536, t4537, t4538, t4539, t4540, t4541, t4542,
    t4543, t4544, t4545, t4546, t4547, t4548, t4549, t4550, t4551, t4552, t4553, t4554, t4555,
    t4556, t4557, t4558, t4559, t4560, t4561, t4562, t4563, t4564, t4565, t4566, t4567, t4568,
    t4569, t4570, t4571, t4572, t4573, t4574, t4575, t4576, t4577, t4578, t4579, t4580, t4581,
    t4582, t4583, t4584, t4585, t4586, t4587, t4588, t4589, t4590, t4591, t4592, t4593, t4594,
    t4595, t4596, t4597, t4598, t4599, t4600, t4601, t4602, t4603, t4604, t4605, t4606, t4607,
    t4608, t4609, t4610, t4611, t4612, t4613, t4614, t4615, t4616, t4617, t4618, t4619, t4620,
    t4621, t4622, t4623, t4624, t4625, t4626, t4627, t4628, t4629, t4630, t4631, t4632, t4633,
    t4634, t4635, t4636, t4637, t4638, t4639, t4640, t4641, t4642, t4643, t4644, t4645, t4646,
    t4647, t4648, t4649, t4650, t4651, t4652, t4653, t4654, t4655, t4656, t4657, t4658, t4659,
    t4660, t4661, t4662, t4663, t4664, t4665, t4666, t4667, t4668, t4669, t4670, t4671, t4672,
    t4673, t4674, t4675, t4676, t4677, t4678, t4679, t4680, t4681, t4682, t4683, t4684, t4685,
    t4686, t4687, t4688, t4689, t4690, t4691, t4692, t4693, t4694, t4695, t4696, t4697, t4698,
    t4699, t4700, t4701, t4702, t4703, t4704, t4705, t4706, t4707, t4708, t4709, t4710, t4711,
    t4712, t4713, t4714, t4715, t4716, t4717, t4718, t4719, t4720, t4721, t4722, t4723, t4724,
    t4725, t4726, t4727, t4728, t4729, t4730, t4731, t4732, t4733, t4734, t4735, t4736, t4737,
    t4738, t4739, t4740, t4741, t4742, t4743, t4744, t4745, t4746, t4747, t4748, t4749, t4750,
    t4751, t4752, t4753, t4754, t4755, t4756, t4757, t4758, t4759, t4760, t4761, t4762, t4763,
    t4764, t4765, t4766, t4767, t4768, t4769, t4770, t4771, t4772, t4773, t4774, t4775, t4776,
    t4777, t4778, t4779, t4780, t4781, t4782, t4783, t4784, t4785, t4786, t4787, t4788, t4789,
    t4790, t4791, t4792, t4793, t4794, t4795, t4796, t4797, t4798, t4799, t4800, t4801, t4802,
    t4803, t4804, t4805, t4806, t4807, t4808, t4809, t4810, t4811, t4812, t4813, t4814, t4815,
    t4816, t4817, t4818, t4819, t4820, t4821, t4822, t4823, t4824, t4825, t4826, t4827, t4828,
    t4829, t4830, t4831, t4832, t4833, t4834, t4835, t4836, t4837, t4838, t4839, t4840, t4841,
    t4842, t4843, t4844, t4845, t4846, t4847, t4848, t4849, t4850, t4851, t4852, t4853, t4854,
    t4855, t4856, t4857, t4858, t4859, t4860, t4861, t4862, t4863, t4864, t4865, t4866, t4867,
    t4868, t4869, t4870, t4871, t4872, t4873, t4874, t4875, t4876, t4877, t4878, t4879, t4880,
    t4881, t4882, t4883, t4884, t4885, t4886, t4887, t4888, t4889, t4890, t4891, t4892, t4893,
    t4894, t4895, t4896, t4897, t4898, t4899, t4900, t4901, t4902, t4903, t4904, t4905, t4906,
    t4907, t4908, t4909, t4910, t4911, t4912, t4913, t4914, t4915, t4916, t4917, t4918, t4919,
    t4920, t4921, t4922, t4923, t4924, t4925, t4926, t4927, t4928, t4929, t4930, t4931, t4932,
    t4933, t4934, t4935, t4936, t4937, t4938, t4939, t4940, t4941, t4942, t4943, t4944, t4945,
    t4946, t4947, t4948, t4949, t4950, t4951, t4952, t4953, t4954, t4955, t4956, t4957, t4958,
    t4959, t4960, t4961, t4962, t4963, t4964, t4965, t4966, t4967, t4968, t4969, t4970, t4971,
    t4972, t4973, t4974, t4975, t4976, t4977, t4978, t4979, t4980, t4981, t4982, t4983, t4984,
    t4985, t4986, t4987, t4988, t4989, t4990, t4991, t4992, t4993, t4994, t4995, t4996, t4997,
    t4998, t4999, t5000, t5001, t5002, t5003, t5004, t5005, t5006, t5007, t5008, t5009, t5010,
    t5011, t5012, t5013, t5014, t5015, t5016, t5017, t5018, t5019, t5020, t5021, t5022, t5023,
    t5024, t5025, t5026, t5027, t5028, t5029, t5030, t5031, t5032, t5033, t5034, t5035, t5036,
    t5037, t5038, t5039, t5040, t5041, t5042, t5043, t5044, t5045, t5046, t5047, t5048, t5049,
    t5050, t5051, t5052, t5053, t5054, t5055, t5056, t5057, t5058, t5059, t5060, t5061, t5062,
    t5063, t5064, t5065, t5066, t5067, t5068, t5069, t5070, t5071, t5072, t5073, t5074, t5075,
    t5076, t5077, t5078, t5079, t5080, t5081, t5082, t5083, t5084, t5085, t5086, t5087, t5088,
    t5089, t5090, t5091, t5092, t5093, t5094, t5095, t5096, t5097, t5098, t5099, t5100, t5101,
    t5102, t5103, t5104, t5105, t5106, t5107, t5108, t5109, t5110, t5111, t5112, t5113, t5114,
    t5115, t5116, t5117, t5118, t5119, t5120, t5121, t5122, t5123, t5124, t5125, t5126, t5127,
    t5128, t5129, t5130, t5131, t5132, t5133, t5134, t5135, t5136, t5137, t5138, t5139, t5140,
    t5141, t5142, t5143, t5144, t5145, t5146, t5147, t5148, t5149, t5150, t5151, t5152, t5153,
    t5154, t5155, t5156, t5157, t5158, t5159, t5160, t5161, t5162, t5163, t5164, t5165, t5166,
    t5167, t5168, t5169, t5170, t5171, t5172, t5173, t5174, t5175, t5176, t5177, t5178, t5179,
    t5180, t5181, t5182, t5183, t5184, t5185, t5186, t5187, t5188, t5189, t5190, t5191, t5192,
    t5193, t5194, t5195, t5196, t5197, t5198, t5199, t5200, t5201, t5202, t5203, t5204, t5205,
    t5206, t5207, t5208, t5209, t5210, t5211, t5212, t5213, t5214, t5215, t5216, t5217, t5218,
    t5219, t5220, t5221, t5222, t5223, t5224, t5225, t5226, t5227, t5228, t5229, t5230, t5231,
    t5232, t5233, t5234, t5235, t5236, t5237, t5238, t5239, t5240, t5241, t5242, t5243, t5244,
    t5245, t5246, t5247, t5248, t5249, t5250, t5251, t5252, t5253, t5254, t5255, t5256, t5257,
    t5258, t5259, t5260, t5261, t5262, t5263, t5264, t5265, t5266, t5267, t5268, t5269, t5270,
    t5271, t5272, t5273, t5274, t5275, t5276, t5277, t5278, t5279, t5280, t5281, t5282, t5283,
    t5284, t5285, t5286, t5287, t5288, t5289, t5290, t5291, t5292, t5293, t5294, t5295, t5296,
    t5297, t5298, t5299, t5300, t5301, t5302, t5303, t5304, t5305, t5306, t5307, t5308, t5309,
    t5310, t5311, t5312, t5313, t5314, t5315, t5316, t5317, t5318, t5319, t5320, t5321, t5322,
    t5323, t5324, t5325, t5326, t5327, t5328, t5329, t5330, t5331, t5332, t5333, t5334, t5335,
    t5336, t5337, t5338, t5339, t5340, t5341, t5342, t5343, t5344, t5345, t5346, t5347, t5348,
    t5349, t5350, t5351, t5352, t5353, t5354, t5355, t5356, t5357, t5358, t5359, t5360, t5361,
    t5362, t5363, t5364, t5365, t5366, t5367, t5368, t5369, t5370, t5371, t5372, t5373, t5374,
    t5375, t5376, t5377, t5378, t5379, t5380, t5381, t5382, t5383, t5384, t5385, t5386, t5387,
    t5388, t5389, t5390, t5391, t5392, t5393, t5394, t5395, t5396, t5397, t5398, t5399, t5400,
    t5401, t5402, t5403, t5404, t5405, t5406, t5407, t5408, t5409, t5410, t5411, t5412, t5413,
    t5414, t5415, t5416, t5417, t5418, t5419, t5420, t5421, t5422, t5423, t5424, t5425, t5426,
    t5427, t5428, t5429, t5430, t5431, t5432, t5433, t5434, t5435, t5436, t5437, t5438, t5439,
    t5440, t5441, t5442, t5443, t5444, t5445, t5446, t5447, t5448, t5449, t5450, t5451, t5452,
    t5453, t5454, t5455, t5456, t5457, t5458, t5459, t5460, t5461, t5462, t5463, t5464, t5465,
    t5466, t5467, t5468, t5469, t5470, t5471, t5472, t5473, t5474, t5475, t5476, t5477, t5478,
    t5479, t5480, t5481, t5482, t5483, t5484, t5485, t5486, t5487, t5488, t5489, t5490, t5491,
    t5492, t5493, t5494, t5495, t5496, t5497, t5498, t5499, t5500, t5501, t5502, t5503, t5504,
    t5505, t5506, t5507, t5508, t5509, t5510, t5511, t5512, t5513, t5514, t5515, t5516, t5517,
    t5518, t5519, t5520, t5521, t5522, t5523, t5524, t5525, t5526, t5527, t5528, t5529, t5530,
    t5531, t5532, t5533, t5534, t5535, t5536, t5537, t5538, t5539, t5540, t5541, t5542, t5543,
    t5544, t5545, t5546, t5547, t5548, t5549, t5550, t5551, t5552, t5553, t5554, t5555, t5556,
    t5557, t5558, t5559, t5560, t5561, t5562, t5563, t5564, t5565, t5566, t5567, t5568, t5569,
    t5570, t5571, t5572, t5573, t5574, t5575, t5576, t5577, t5578, t5579, t5580, t5581, t5582,
    t5583, t5584, t5585, t5586, t5587, t5588, t5589, t5590, t5591, t5592, t5593, t5594, t5595,
    t5596, t5597, t5598, t5599, t5600, t5601, t5602, t5603, t5604, t5605, t5606, t5607, t5608,
    t5609, t5610, t5611, t5612, t5613, t5614, t5615, t5616, t5617, t5618, t5619, t5620, t5621,
    t5622, t5623, t5624, t5625, t5626, t5627, t5628, t5629, t5630, t5631, t5632, t5633, t5634,
    t5635, t5636, t5637, t5638, t5639, t5640, t5641, t5642, t5643, t5644, t5645, t5646, t5647,
    t5648, t5649, t5650, t5651, t5652, t5653, t5654, t5655, t5656, t5657, t5658, t5659, t5660,
    t5661, t5662, t5663, t5664, t5665, t5666, t5667, t5668, t5669, t5670, t5671, t5672, t5673,
    t5674, t5675, t5676, t5677, t5678, t5679, t5680, t5681, t5682, t5683, t5684, t5685, t5686,
    t5687, t5688, t5689, t5690, t5691, t5692, t5693, t5694, t5695, t5696, t5697, t5698, t5699,
    t5700, t5701, t5702, t5703, t5704, t5705, t5706, t5707, t5708, t5709, t5710, t5711, t5712,
    t5713, t5714, t5715, t5716, t5717, t5718, t5719, t5720, t5721, t5722, t5723, t5724, t5725,
    t5726, t5727, t5728, t5729, t5730, t5731, t5732, t5733, t5734, t5735, t5736, t5737, t5738,
    t5739, t5740, t5741, t5742, t5743, t5744, t5745, t5746, t5747, t5748, t5749, t5750, t5751,
    t5752, t5753, t5754, t5755, t5756, t5757, t5758, t5759, t5760, t5761, t5762, t5763, t5764,
    t5765, t5766, t5767, t5768, t5769, t5770, t5771, t5772, t5773, t5774, t5775, t5776, t5777,
    t5778, t5779, t5780, t5781, t5782, t5783, t5784, t5785, t5786, t5787, t5788, t5789, t5790,
    t5791, t5792, t5793, t5794, t5795, t5796, t5797, t5798, t5799, t5800, t5801, t5802, t5803,
    t5804, t5805, t5806, t5807, t5808, t5809, t5810, t5811, t5812, t5813, t5814, t5815, t5816,
    t5817, t5818, t5819, t5820, t5821, t5822, t5823, t5824, t5825, t5826, t5827, t5828, t5829,
    t5830, t5831, t5832, t5833, t5834, t5835, t5836, t5837, t5838, t5839, t5840, t5841, t5842,
    t5843, t5844, t5845, t5846, t5847, t5848, t5849, t5850, t5851, t5852, t5853, t5854, t5855,
    t5856, t5857, t5858, t5859, t5860, t5861, t5862, t5863, t5864, t5865, t5866, t5867, t5868,
    t5869, t5870, t5871, t5872, t5873, t5874, t5875, t5876, t5877, t5878, t5879, t5880, t5881,
    t5882, t5883, t5884, t5885, t5886, t5887, t5888, t5889, t5890, t5891, t5892, t5893, t5894,
    t5895, t5896, t5897, t5898, t5899, t5900, t5901, t5902, t5903, t5904, t5905, t5906, t5907,
    t5908, t5909, t5910, t5911, t5912, t5913, t5914, t5915, t5916, t5917, t5918, t5919, t5920,
    t5921, t5922, t5923, t5924, t5925, t5926, t5927, t5928, t5929, t5930, t5931, t5932, t5933,
    t5934, t5935, t5936, t5937, t5938, t5939, t5940, t5941, t5942, t5943, t5944, t5945, t5946,
    t5947, t5948, t5949, t5950, t5951, t5952, t5953, t5954, t5955, t5956, t5957, t5958, t5959,
    t5960, t5961, t5962, t5963, t5964, t5965, t5966, t5967, t5968, t5969, t5970, t5971, t5972,
    t5973, t5974, t5975, t5976, t5977, t5978, t5979, t5980, t5981, t5982, t5983, t5984, t5985,
    t5986, t5987, t5988, t5989, t5990, t5991, t5992, t5993, t5994, t5995, t5996, t5997, t5998,
    t5999, t6000, t6001, t6002, t6003, t6004, t6005, t6006, t6007, t6008, t6009, t6010, t6011,
    t6012, t6013, t6014, t6015, t6016, t6017, t6018, t6019, t6020, t6021, t6022, t6023, t6024,
    t6025, t6026, t6027, t6028, t6029, t6030, t6031, t6032, t6033, t6034, t6035, t6036, t6037,
    t6038, t6039, t6040, t6041, t6042, t6043, t6044, t6045, t6046, t6047, t6048, t6049, t6050,
    t6051, t6052, t6053, t6054, t6055, t6056, t6057, t6058, t6059, t6060, t6061, t6062, t6063,
    t6064, t6065, t6066, t6067, t6068, t6069, t6070, t6071, t6072, t6073, t6074, t6075, t6076,
    t6077, t6078, t6079, t6080, t6081, t6082, t6083, t6084, t6085, t6086, t6087, t6088, t6089,
    t6090, t6091, t6092, t6093, t6094, t6095, t6096, t6097, t6098, t6099, t6100, t6101, t6102,
    t6103, t6104, t6105, t6106, t6107, t6108, t6109, t6110, t6111, t6112, t6113, t6114, t6115,
    t6116, t6117, t6118, t6119, t6120, t6121, t6122, t6123, t6124, t6125, t6126, t6127, t6128,
    t6129, t6130, t6131, t6132, t6133, t6134, t6135, t6136, t6137, t6138, t6139, t6140, t6141,
    t6142, t6143, t6144, t6145, t6146, t6147, t6148, t6149, t6150, t6151, t6152, t6153, t6154,
    t6155, t6156, t6157, t6158, t6159, t6160, t6161, t6162, t6163, t6164, t6165, t6166, t6167,
    t6168, t6169, t6170, t6171, t6172, t6173, t6174, t6175, t6176, t6177, t6178, t6179, t6180,
    t6181, t6182, t6183, t6184, t6185, t6186, t6187, t6188, t6189, t6190, t6191, t6192, t6193,
    t6194, t6195, t6196, t6197, t6198, t6199, t6200, t6201, t6202, t6203, t6204, t6205, t6206,
    t6207, t6208, t6209, t6210, t6211, t6212, t6213, t6214, t6215, t6216, t6217, t6218, t6219,
    t6220, t6221, t6222, t6223, t6224, t6225, t6226, t6227, t6228, t6229, t6230, t6231, t6232,
    t6233, t6234, t6235, t6236, t6237, t6238, t6239, t6240, t6241, t6242, t6243, t6244, t6245,
    t6246, t6247, t6248, t6249, t6250, t6251, t6252, t6253, t6254, t6255, t6256, t6257, t6258,
    t6259, t6260, t6261, t6262, t6263, t6264, t6265, t6266, t6267, t6268, t6269, t6270, t6271,
    t6272, t6273, t6274, t6275, t6276, t6277, t6278, t6279, t6280, t6281, t6282, t6283, t6284,
    t6285, t6286, t6287, t6288, t6289, t6290, t6291, t6292, t6293, t6294, t6295, t6296, t6297,
    t6298, t6299, t6300, t6301, t6302, t6303, t6304, t6305, t6306, t6307, t6308, t6309, t6310,
    t6311, t6312, t6313, t6314, t6315, t6316, t6317, t6318, t6319, t6320, t6321, t6322, t6323,
    t6324, t6325, t6326, t6327, t6328, t6329, t6330, t6331, t6332, t6333, t6334, t6335, t6336,
    t6337, t6338, t6339, t6340, t6341, t6342, t6343, t6344, t6345, t6346, t6347, t6348, t6349,
    t6350, t6351, t6352, t6353, t6354, t6355, t6356, t6357, t6358, t6359, t6360, t6361, t6362,
    t6363, t6364, t6365, t6366, t6367, t6368, t6369, t6370, t6371, t6372, t6373, t6374, t6375,
    t6376, t6377, t6378, t6379, t6380, t6381, t6382, t6383, t6384, t6385, t6386, t6387, t6388,
    t6389, t6390, t6391, t6392, t6393, t6394, t6395, t6396, t6397, t6398, t6399, t6400, t6401,
    t6402, t6403, t6404, t6405, t6406, t6407, t6408, t6409, t6410, t6411, t6412, t6413, t6414,
    t6415, t6416, t6417, t6418, t6419, t6420, t6421, t6422, t6423, t6424, t6425, t6426, t6427,
    t6428, t6429, t6430, t6431, t6432, t6433, t6434, t6435, t6436, t6437, t6438, t6439, t6440,
    t6441, t6442, t6443, t6444, t6445, t6446, t6447, t6448, t6449, t6450, t6451, t6452, t6453,
    t6454, t6455, t6456, t6457, t6458, t6459, t6460, t6461, t6462, t6463, t6464, t6465, t6466,
    t6467, t6468, t6469, t6470, t6471, t6472, t6473, t6474, t6475, t6476, t6477, t6478, t6479,
    t6480, t6481, t6482, t6483, t6484, t6485, t6486, t6487, t6488, t6489, t6490, t6491, t6492,
    t6493, t6494, t6495, t6496, t6497, t6498, t6499, t6500, t6501, t6502, t6503, t6504, t6505,
    t6506, t6507, t6508, t6509, t6510, t6511, t6512, t6513, t6514, t6515, t6516, t6517, t6518,
    t6519, t6520, t6521, t6522, t6523, t6524, t6525, t6526, t6527, t6528, t6529, t6530, t6531,
    t6532, t6533, t6534, t6535, t6536, t6537, t6538, t6539, t6540, t6541, t6542, t6543, t6544,
    t6545, t6546, t6547, t6548, t6549, t6550, t6551, t6552, t6553, t6554, t6555, t6556, t6557,
    t6558, t6559, t6560, t6561, t6562, t6563, t6564, t6565, t6566, t6567, t6568, t6569, t6570,
    t6571, t6572, t6573, t6574, t6575, t6576, t6577, t6578, t6579, t6580, t6581, t6582, t6583,
    t6584, t6585, t6586, t6587, t6588, t6589, t6590, t6591, t6592, t6593, t6594, t6595, t6596,
    t6597, t6598, t6599, t6600, t6601, t6602, t6603, t6604, t6605, t6606, t6607, t6608, t6609,
    t6610, t6611, t6612, t6613, t6614, t6615, t6616, t6617, t6618, t6619, t6620, t6621, t6622,
    t6623, t6624, t6625, t6626, t6627, t6628, t6629, t6630, t6631, t6632, t6633, t6634, t6635,
    t6636, t6637, t6638, t6639, t6640, t6641, t6642, t6643, t6644, t6645, t6646, t6647, t6648,
    t6649, t6650, t6651, t6652, t6653, t6654, t6655, t6656, t6657, t6658, t6659, t6660, t6661,
    t6662, t6663, t6664, t6665, t6666, t6667, t6668, t6669, t6670, t6671, t6672, t6673, t6674,
    t6675, t6676, t6677, t6678, t6679, t6680, t6681, t6682, t6683, t6684, t6685, t6686, t6687,
    t6688, t6689, t6690, t6691, t6692, t6693, t6694, t6695, t6696, t6697, t6698, t6699, t6700,
    t6701, t6702, t6703, t6704, t6705, t6706, t6707, t6708, t6709, t6710, t6711, t6712, t6713,
    t6714, t6715, t6716, t6717, t6718, t6719, t6720, t6721, t6722, t6723, t6724, t6725, t6726,
    t6727, t6728, t6729, t6730, t6731, t6732, t6733, t6734, t6735, t6736, t6737, t6738, t6739,
    t6740, t6741, t6742, t6743, t6744, t6745, t6746, t6747, t6748, t6749, t6750, t6751, t6752,
    t6753, t6754, t6755, t6756, t6757, t6758, t6759, t6760, t6761, t6762, t6763, t6764, t6765,
    t6766, t6767, t6768, t6769, t6770, t6771, t6772, t6773, t6774, t6775, t6776, t6777, t6778,
    t6779, t6780, t6781, t6782, t6783, t6784, t6785, t6786, t6787, t6788, t6789, t6790, t6791,
    t6792, t6793, t6794, t6795, t6796, t6797, t6798, t6799, t6800, t6801, t6802, t6803, t6804,
    t6805, t6806, t6807, t6808, t6809, t6810, t6811, t6812, t6813, t6814, t6815, t6816, t6817,
    t6818, t6819, t6820, t6821, t6822, t6823, t6824, t6825, t6826, t6827, t6828, t6829, t6830,
    t6831, t6832, t6833, t6834, t6835, t6836, t6837, t6838, t6839, t6840, t6841, t6842, t6843,
    t6844, t6845, t6846, t6847, t6848, t6849, t6850, t6851, t6852, t6853, t6854, t6855, t6856,
    t6857, t6858, t6859, t6860, t6861, t6862, t6863, t6864, t6865, t6866, t6867, t6868, t6869,
    t6870, t6871, t6872, t6873, t6874, t6875, t6876, t6877, t6878, t6879, t6880, t6881, t6882,
    t6883, t6884, t6885, t6886, t6887, t6888, t6889, t6890, t6891, t6892, t6893, t6894, t6895,
    t6896, t6897, t6898, t6899, t6900, t6901, t6902, t6903, t6904, t6905, t6906, t6907, t6908,
    t6909, t6910, t6911, t6912, t6913, t6914, t6915, t6916, t6917, t6918, t6919, t6920, t6921,
    t6922, t6923, t6924, t6925, t6926, t6927, t6928, t6929, t6930, t6931, t6932, t6933, t6934,
    t6935, t6936, t6937, t6938, t6939, t6940, t6941, t6942, t6943, t6944, t6945, t6946, t6947,
    t6948, t6949, t6950, t6951, t6952, t6953, t6954, t6955, t6956, t6957, t6958, t6959, t6960,
    t6961, t6962, t6963, t6964, t6965, t6966, t6967, t6968, t6969, t6970, t6971, t6972, t6973,
    t6974, t6975, t6976, t6977, t6978, t6979, t6980, t6981, t6982, t6983, t6984, t6985, t6986,
    t6987, t6988, t6989, t6990, t6991, t6992, t6993, t6994, t6995, t6996, t6997, t6998, t6999,
    t7000, t7001, t7002, t7003, t7004, t7005, t7006, t7007, t7008, t7009, t7010, t7011, t7012,
    t7013, t7014, t7015, t7016, t7017, t7018, t7019, t7020, t7021, t7022, t7023, t7024, t7025,
    t7026, t7027, t7028, t7029, t7030, t7031, t7032, t7033, t7034, t7035, t7036, t7037, t7038,
    t7039, t7040, t7041, t7042, t7043, t7044, t7045, t7046, t7047, t7048, t7049, t7050, t7051,
    t7052, t7053, t7054, t7055, t7056, t7057, t7058, t7059, t7060, t7061, t7062, t7063, t7064,
    t7065, t7066, t7067, t7068, t7069, t7070, t7071, t7072, t7073, t7074, t7075, t7076, t7077,
    t7078, t7079, t7080, t7081, t7082, t7083, t7084, t7085, t7086, t7087, t7088, t7089, t7090,
    t7091, t7092, t7093, t7094, t7095, t7096, t7097, t7098, t7099, t7100, t7101, t7102, t7103,
    t7104, t7105, t7106, t7107, t7108, t7109, t7110, t7111, t7112, t7113, t7114, t7115, t7116,
    t7117, t7118, t7119, t7120, t7121, t7122, t7123, t7124, t7125, t7126, t7127, t7128, t7129,
    t7130, t7131, t7132, t7133, t7134, t7135, t7136, t7137, t7138, t7139, t7140, t7141, t7142,
    t7143, t7144, t7145, t7146, t7147, t7148, t7149, t7150, t7151, t7152, t7153, t7154, t7155,
    t7156, t7157, t7158, t7159, t7160, t7161, t7162, t7163, t7164, t7165, t7166, t7167, t7168,
    t7169, t7170, t7171, t7172, t7173, t7174, t7175, t7176, t7177, t7178, t7179, t7180, t7181,
    t7182, t7183, t7184, t7185, t7186, t7187, t7188, t7189, t7190, t7191, t7192, t7193, t7194,
    t7195, t7196, t7197, t7198, t7199, t7200, t7201, t7202, t7203, t7204, t7205, t7206, t7207,
    t7208, t7209, t7210, t7211, t7212, t7213, t7214, t7215, t7216, t7217, t7218, t7219, t7220,
    t7221, t7222, t7223, t7224, t7225, t7226, t7227, t7228, t7229, t7230, t7231, t7232, t7233,
    t7234, t7235, t7236, t7237, t7238, t7239, t7240, t7241, t7242, t7243, t7244, t7245, t7246,
    t7247, t7248, t7249, t7250, t7251, t7252, t7253, t7254, t7255, t7256, t7257, t7258, t7259,
    t7260, t7261, t7262, t7263, t7264, t7265, t7266, t7267, t7268, t7269, t7270, t7271, t7272,
    t7273, t7274, t7275, t7276, t7277, t7278, t7279, t7280, t7281, t7282, t7283, t7284, t7285,
    t7286, t7287, t7288, t7289, t7290, t7291, t7292, t7293, t7294, t7295, t7296, t7297, t7298,
    t7299, t7300, t7301, t7302, t7303, t7304, t7305, t7306, t7307, t7308, t7309, t7310, t7311,
    t7312, t7313, t7314, t7315, t7316, t7317, t7318, t7319, t7320, t7321, t7322, t7323, t7324,
    t7325, t7326, t7327, t7328, t7329, t7330, t7331, t7332, t7333, t7334, t7335, t7336, t7337,
    t7338, t7339, t7340, t7341, t7342, t7343, t7344, t7345, t7346, t7347, t7348, t7349, t7350,
    t7351, t7352, t7353, t7354, t7355, t7356, t7357, t7358, t7359, t7360, t7361, t7362, t7363,
    t7364, t7365, t7366, t7367, t7368, t7369, t7370, t7371, t7372, t7373, t7374, t7375, t7376,
    t7377, t7378, t7379, t7380, t7381, t7382, t7383, t7384, t7385, t7386, t7387, t7388, t7389,
    t7390, t7391, t7392, t7393, t7394, t7395, t7396, t7397, t7398, t7399, t7400, t7401, t7402,
    t7403, t7404, t7405, t7406, t7407, t7408, t7409, t7410, t7411, t7412, t7413, t7414, t7415,
    t7416, t7417, t7418, t7419, t7420, t7421, t7422, t7423, t7424, t7425, t7426, t7427, t7428,
    t7429, t7430, t7431, t7432, t7433, t7434, t7435, t7436, t7437, t7438, t7439, t7440, t7441,
    t7442, t7443, t7444, t7445, t7446, t7447, t7448, t7449, t7450, t7451, t7452, t7453, t7454,
    t7455, t7456, t7457, t7458, t7459, t7460, t7461, t7462, t7463, t7464, t7465, t7466, t7467,
    t7468, t7469, t7470, t7471, t7472, t7473, t7474, t7475, t7476, t7477, t7478, t7479, t7480,
    t7481, t7482, t7483, t7484, t7485, t7486, t7487, t7488, t7489, t7490, t7491, t7492, t7493,
    t7494, t7495, t7496, t7497, t7498, t7499, t7500, t7501, t7502, t7503, t7504, t7505, t7506,
    t7507, t7508, t7509, t7510, t7511, t7512, t7513, t7514, t7515, t7516, t7517, t7518, t7519,
    t7520, t7521, t7522, t7523, t7524, t7525, t7526, t7527, t7528, t7529, t7530, t7531, t7532,
    t7533, t7534, t7535, t7536, t7537, t7538, t7539, t7540, t7541, t7542, t7543, t7544, t7545,
    t7546, t7547, t7548, t7549, t7550, t7551, t7552, t7553, t7554, t7555, t7556, t7557, t7558,
    t7559, t7560, t7561, t7562, t7563, t7564, t7565, t7566, t7567, t7568, t7569, t7570, t7571,
    t7572, t7573, t7574, t7575, t7576, t7577, t7578, t7579, t7580, t7581, t7582, t7583, t7584,
    t7585, t7586, t7587, t7588, t7589, t7590, t7591, t7592, t7593, t7594, t7595, t7596, t7597,
    t7598, t7599, t7600, t7601, t7602, t7603, t7604, t7605, t7606, t7607, t7608, t7609, t7610,
    t7611, t7612, t7613, t7614, t7615, t7616, t7617, t7618, t7619, t7620, t7621, t7622, t7623,
    t7624, t7625, t7626, t7627, t7628, t7629, t7630, t7631, t7632, t7633, t7634, t7635, t7636,
    t7637, t7638, t7639, t7640, t7641, t7642, t7643, t7644, t7645, t7646, t7647, t7648, t7649,
    t7650, t7651, t7652, t7653, t7654, t7655, t7656, t7657, t7658, t7659, t7660, t7661, t7662,
    t7663, t7664, t7665, t7666, t7667, t7668, t7669, t7670, t7671, t7672, t7673, t7674, t7675,
    t7676, t7677, t7678, t7679, t7680, t7681, t7682, t7683, t7684, t7685, t7686, t7687, t7688,
    t7689, t7690, t7691, t7692, t7693, t7694, t7695, t7696, t7697, t7698, t7699, t7700, t7701,
    t7702, t7703, t7704, t7705, t7706, t7707, t7708, t7709, t7710, t7711, t7712, t7713, t7714,
    t7715, t7716, t7717, t7718, t7719, t7720, t7721, t7722, t7723, t7724, t7725, t7726, t7727,
    t7728, t7729, t7730, t7731, t7732, t7733, t7734, t7735, t7736, t7737, t7738, t7739, t7740,
    t7741, t7742, t7743, t7744, t7745, t7746, t7747, t7748, t7749, t7750, t7751, t7752, t7753,
    t7754, t7755, t7756, t7757, t7758, t7759, t7760, t7761, t7762, t7763, t7764, t7765, t7766,
    t7767, t7768, t7769, t7770, t7771, t7772, t7773, t7774, t7775, t7776, t7777, t7778, t7779,
    t7780, t7781, t7782, t7783, t7784, t7785, t7786, t7787, t7788, t7789, t7790, t7791, t7792,
    t7793, t7794, t7795, t7796, t7797, t7798, t7799, t7800, t7801, t7802, t7803, t7804, t7805,
    t7806, t7807, t7808, t7809, t7810, t7811, t7812, t7813, t7814, t7815, t7816, t7817, t7818,
    t7819, t7820, t7821, t7822, t7823, t7824, t7825, t7826, t7827, t7828, t7829, t7830, t7831,
    t7832, t7833, t7834, t7835, t7836, t7837, t7838, t7839, t7840, t7841, t7842, t7843, t7844,
    t7845, t7846, t7847, t7848, t7849, t7850, t7851, t7852, t7853, t7854, t7855, t7856, t7857,
    t7858, t7859, t7860, t7861, t7862, t7863, t7864, t7865, t7866, t7867, t7868, t7869, t7870,
    t7871, t7872, t7873, t7874, t7875, t7876, t7877, t7878, t7879, t7880, t7881, t7882, t7883,
    t7884, t7885, t7886, t7887, t7888, t7889, t7890, t7891, t7892, t7893, t7894, t7895, t7896,
    t7897, t7898, t7899, t7900, t7901, t7902, t7903, t7904, t7905, t7906, t7907, t7908, t7909,
    t7910, t7911, t7912, t7913, t7914, t7915, t7916, t7917, t7918, t7919, t7920, t7921, t7922,
    t7923, t7924, t7925, t7926, t7927, t7928, t7929, t7930, t7931, t7932, t7933, t7934, t7935,
    t7936, t7937, t7938, t7939, t7940, t7941, t7942, t7943, t7944, t7945, t7946, t7947, t7948,
    t7949, t7950, t7951, t7952, t7953, t7954, t7955, t7956, t7957, t7958, t7959, t7960, t7961,
    t7962, t7963, t7964, t7965, t7966, t7967, t7968, t7969, t7970, t7971, t7972, t7973, t7974,
    t7975, t7976, t7977, t7978, t7979, t7980, t7981, t7982, t7983, t7984, t7985, t7986, t7987,
    t7988, t7989, t7990, t7991, t7992, t7993, t7994, t7995, t7996, t7997, t7998, t7999, t8000,
    t8001, t8002, t8003, t8004, t8005, t8006, t8007, t8008, t8009, t8010, t8011, t8012, t8013,
    t8014, t8015, t8016, t8017, t8018, t8019, t8020, t8021, t8022, t8023, t8024, t8025, t8026,
    t8027, t8028, t8029, t8030, t8031, t8032, t8033, t8034, t8035, t8036, t8037, t8038, t8039,
    t8040, t8041, t8042, t8043, t8044, t8045, t8046, t8047, t8048, t8049, t8050, t8051, t8052,
    t8053, t8054, t8055, t8056, t8057, t8058, t8059, t8060, t8061, t8062, t8063, t8064, t8065,
    t8066, t8067, t8068, t8069, t8070, t8071, t8072, t8073, t8074, t8075, t8076, t8077, t8078,
    t8079, t8080, t8081, t8082, t8083, t8084, t8085, t8086, t8087, t8088, t8089, t8090, t8091,
    t8092, t8093, t8094, t8095, t8096, t8097, t8098, t8099, t8100, t8101, t8102, t8103, t8104,
    t8105, t8106, t8107, t8108, t8109, t8110, t8111, t8112, t8113, t8114, t8115, t8116, t8117,
    t8118, t8119, t8120, t8121, t8122, t8123, t8124, t8125, t8126, t8127, t8128, t8129, t8130,
    t8131, t8132, t8133, t8134, t8135, t8136, t8137, t8138, t8139, t8140, t8141, t8142, t8143,
    t8144, t8145, t8146, t8147, t8148, t8149, t8150, t8151, t8152, t8153, t8154, t8155, t8156,
    t8157, t8158, t8159, t8160, t8161, t8162, t8163, t8164, t8165, t8166, t8167, t8168, t8169,
    t8170, t8171, t8172, t8173, t8174, t8175, t8176, t8177, t8178, t8179, t8180, t8181, t8182,
    t8183, t8184, t8185, t8186, t8187, t8188, t8189, t8190, t8191,
];
