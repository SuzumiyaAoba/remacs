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

use std::cell::RefCell;
use std::ffi::{c_char, c_void};
use std::io::Write;
use std::rc::Rc;
use std::sync::Mutex;

use super::S;
use crate::lisp::value::{Arity, OptParam, Subr, SubrFn, Value};
use crate::lisp::{sym, EvalResult, Flow, Interp, SymId};

// ---------------------------------------------------------------------------
// em_obj handle space shared with generated code
// ---------------------------------------------------------------------------

/// Opaque object reference across the FFI boundary: index+1 into
/// `OBJTAB` (0 is the error sentinel `EM_INVALID`).
type EmObj = u64;
type EmEnv = usize;
type EmEntry = unsafe extern "C" fn(*const EmApi, EmEnv, *const EmObj, i64) -> EmObj;

const EM_INVALID: EmObj = 0;

thread_local! {
    static OBJTAB: RefCell<Vec<Value>> = const { RefCell::new(Vec::new()) };
    static PENDING: RefCell<Option<(Value, Value)>> = const { RefCell::new(None) };
    static NATIVES: RefCell<Vec<EmEntry>> = const { RefCell::new(Vec::new()) };
    static META: RefCell<Vec<NativeMeta>> = RefCell::new(Vec::new());
    static UNITS: RefCell<Vec<Unit>> = RefCell::new(Vec::new());
    static CTXT: RefCell<Option<String>> = RefCell::new(None);
}

fn reg(v: Value) -> EmObj {
    OBJTAB.with(|t| {
        let mut t = t.borrow_mut();
        t.push(v);
        t.len() as EmObj
    })
}

fn obj(h: EmObj) -> Value {
    OBJTAB.with(|t| t.borrow()[(h - 1) as usize].clone())
}

// ---------------------------------------------------------------------------
// Host API table — every callback runs inside a live Interp
// ---------------------------------------------------------------------------

unsafe fn host(e: EmEnv) -> &'static mut Interp {
    &mut *(e as *mut Interp)
}

unsafe fn outcome(e: EmEnv, r: Result<Value, Flow>) -> EmObj {
    match r {
        Ok(v) => reg(v),
        Err(Flow::Signal(s, d, _)) => {
            PENDING.with(|p| *p.borrow_mut() = Some((s, d)));
            EM_INVALID
        }
        Err(other) => {
            let i = host(e);
            let f = i.error(format!("Abnormal exit from native code: {other:?}"));
            if let Flow::Signal(s, d, _) = f {
                PENDING.with(|p| *p.borrow_mut() = Some((s, d)));
            }
            EM_INVALID
        }
    }
}

#[repr(C)]
struct EmApi {
    mk_nil: unsafe extern "C" fn(EmEnv) -> EmObj,
    mk_t: unsafe extern "C" fn(EmEnv) -> EmObj,
    mk_int: unsafe extern "C" fn(EmEnv, i64) -> EmObj,
    mk_float: unsafe extern "C" fn(EmEnv, f64) -> EmObj,
    mk_str: unsafe extern "C" fn(EmEnv, *const c_char, usize) -> EmObj,
    intern: unsafe extern "C" fn(EmEnv, *const c_char, usize) -> EmObj,
    /// Parse a printed literal (`read-from-string').
    read: unsafe extern "C" fn(EmEnv, *const c_char, usize) -> EmObj,
    /// Truth test (non-nil → 1).
    truthy: unsafe extern "C" fn(EmEnv, EmObj) -> i64,
    /// `funcall' a function object or symbol.
    call: unsafe extern "C" fn(EmEnv, EmObj, *const EmObj, i64) -> EmObj,
    /// `eval' a form.
    eval: unsafe extern "C" fn(EmEnv, EmObj) -> EmObj,
    /// Call a lambda/subr object with argv (fallback path).
    call_lambda: unsafe extern "C" fn(EmEnv, EmObj, *const EmObj, i64) -> EmObj,
    /// `set' a (dynamic/global) variable symbol.
    sym_set: unsafe extern "C" fn(EmEnv, EmObj, EmObj) -> EmObj,
    /// Record a non-local exit; always returns EM_INVALID.
    signal: unsafe extern "C" fn(EmEnv, EmObj, EmObj) -> EmObj,
    car: unsafe extern "C" fn(EmEnv, EmObj) -> EmObj,
    cdr: unsafe extern "C" fn(EmEnv, EmObj) -> EmObj,
    cons: unsafe extern "C" fn(EmEnv, EmObj, EmObj) -> EmObj,
    listn: unsafe extern "C" fn(EmEnv, *const EmObj, i64) -> EmObj,
    /// Registration entry used by generated init code.
    defsubr: unsafe extern "C" fn(*const EmApi, EmEnv, i64, *const c_void) -> i64,
}

unsafe extern "C" fn api_nil(_e: EmEnv) -> EmObj {
    reg(Value::Nil)
}
unsafe extern "C" fn api_t(_e: EmEnv) -> EmObj {
    reg(Value::t())
}
unsafe extern "C" fn api_int(_e: EmEnv, n: i64) -> EmObj {
    reg(Value::Int(n as i128))
}
unsafe extern "C" fn api_float(_e: EmEnv, f: f64) -> EmObj {
    reg(Value::Float(Rc::new(f)))
}
unsafe extern "C" fn api_str(_e: EmEnv, p: *const c_char, n: usize) -> EmObj {
    let b = unsafe { std::slice::from_raw_parts(p as *const u8, n) };
    reg(Value::string(String::from_utf8_lossy(b)))
}
unsafe extern "C" fn api_intern(e: EmEnv, p: *const c_char, n: usize) -> EmObj {
    let b = unsafe { std::slice::from_raw_parts(p as *const u8, n) };
    let i = host(e);
    let s = String::from_utf8_lossy(b);
    reg(Value::Sym(i.intern(&s)))
}
unsafe extern "C" fn api_read(e: EmEnv, p: *const c_char, n: usize) -> EmObj {
    let b = unsafe { std::slice::from_raw_parts(p as *const u8, n) };
    let i = host(e);
    let s = String::from_utf8_lossy(b).into_owned();
    outcome(e, i.read_from_string(&s, 0).map(|(v, _)| v))
}
unsafe extern "C" fn api_truthy(_e: EmEnv, h: EmObj) -> i64 {
    (!obj(h).is_nil()) as i64
}
unsafe extern "C" fn api_call(e: EmEnv, f: EmObj, argv: *const EmObj, argc: i64) -> EmObj {
    let i = host(e);
    // `call' takes already-evaluated values → `apply', not
    // `call_function' (which evals each arg as a form).
    let args: Vec<Value> = (0..argc as usize)
        .map(|k| obj(unsafe { *argv.add(k) }))
        .collect();
    let f = obj(f);
    outcome(e, i.apply(&f, args))
}
unsafe extern "C" fn api_eval(e: EmEnv, form: EmObj) -> EmObj {
    let i = host(e);
    let form = obj(form);
    outcome(e, i.eval(&form))
}
unsafe extern "C" fn api_sym_set(e: EmEnv, s: EmObj, v: EmObj) -> EmObj {
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
unsafe extern "C" fn api_signal(_e: EmEnv, s: EmObj, d: EmObj) -> EmObj {
    PENDING.with(|p| *p.borrow_mut() = Some((obj(s), obj(d))));
    EM_INVALID
}
unsafe extern "C" fn api_car(e: EmEnv, h: EmObj) -> EmObj {
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
unsafe extern "C" fn api_cdr(e: EmEnv, h: EmObj) -> EmObj {
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
unsafe extern "C" fn api_cons(_e: EmEnv, a: EmObj, d: EmObj) -> EmObj {
    reg(Value::cons(obj(a), obj(d)))
}
unsafe extern "C" fn api_listn(_e: EmEnv, argv: *const EmObj, n: i64) -> EmObj {
    let items: Vec<Value> = (0..n.max(0) as usize)
        .map(|k| obj(unsafe { *argv.add(k) }))
        .collect();
    reg(Value::list(items))
}
unsafe extern "C" fn api_defsubr(
    _api: *const EmApi,
    e: EmEnv,
    unit: i64,
    desc: *const c_void,
) -> i64 {
    let d = unsafe { &*(desc as *const EmFnDesc) };
    let i = host(e);
    let name = unsafe { cstr(d.name, d.name_len) };
    let doc = if d.doc.is_null() {
        String::new()
    } else {
        unsafe { cstr(d.doc, d.doc_len) }
    };
    let mut read = |p: *const c_char, n: usize| -> Value {
        if p.is_null() || n == 0 {
            return Value::Nil;
        }
        let s = unsafe { cstr(p, n) };
        i.read_from_string(&s, 0)
            .map(|(v, _)| v)
            .unwrap_or(Value::Nil)
    };
    // `defmacro': evaluate the recorded `(defmacro …)' form — a
    // macro's meaning is its expander, not a callable subr.
    if !d.sexp.is_null() && d.sexp_len > 0 {
        let s = unsafe { cstr(d.sexp, d.sexp_len) };
        return match i.read_from_string(&s, 0) {
            Ok((form, _)) => match i.eval(&form) {
                Ok(_) => 0,
                Err(Flow::Signal(sg, dd, _)) => {
                    PENDING.with(|p| *p.borrow_mut() = Some((sg, dd)));
                    1
                }
                Err(_) => 1,
            },
            Err(_) => 1,
        };
    }
    let arglist = read(d.arglist, d.arglist_len);
    let iform = read(d.iform, d.iform_len);
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
    name: String,
    unit: i64,
    arglist: Value,
    iform: Value,
}

fn tramp<const K: usize>(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let entry = NATIVES.with(|n| n.borrow()[K]);
    let env = i as *mut Interp as EmEnv;
    let mark = OBJTAB.with(|t| t.borrow().len());
    let args: Vec<EmObj> = a.iter().map(|v| reg(v.clone())).collect();
    let r = unsafe { entry(&API, env, args.as_ptr(), args.len() as i64) };
    let out = if r == EM_INVALID {
        match PENDING.with(|p| p.borrow_mut().take()) {
            Some((s, d)) => Err(Flow::Signal(s, d, false)),
            None => Err(i.error("native function failed")),
        }
    } else {
        Ok(obj(r))
    };
    // Handles registered during this call are disposable — the result
    // was cloned out and generated code never caches handles.
    OBJTAB.with(|t| t.borrow_mut().truncate(mark));
    out
}

macro_rules! tramps {
    ($($k:ident => $n:literal),+) => {
        $(fn $k(i: &mut Interp, a: Vec<Value>) -> EvalResult {
            tramp::<$n>(i, a)
        })*
    };
}

// ---------------------------------------------------------------------------
// Compilation units
// ---------------------------------------------------------------------------

struct Unit {
    path: String,
    /// Kept loaded for the life of the process (like GNU's units).
    _lib: libloading::Library,
}



type UnitInit = unsafe extern "C" fn(*const EmApi, EmEnv, i64) -> i32;

/// dlopen an .eln and run its registration.  Returns unit index.
fn load_eln(i: &mut Interp, path: &str) -> Result<usize, String> {
    let lib =
        unsafe { libloading::Library::new(path) }.map_err(|e| format!("Cannot open {path}: {e}"))?;
    let init: UnitInit = unsafe {
        *lib.get(b"remacs_eln_init\0")
            .map_err(|_| "Not a remacs .eln (missing remacs_eln_init)")?
    };
    // Run init first (it calls `defsubr' with the unit index); push
    // the unit only after a successful registration.
    let idx = UNITS.with(|u| u.borrow().len());
    let env = i as *mut Interp as EmEnv;
    let rc = unsafe { init(&API, env, idx as i64) };
    if rc != 0 {
        return Err("remacs_eln_init failed".into());
    }
    UNITS.with(|u| {
        u.borrow_mut().push(Unit {
            path: path.to_string(),
            _lib: lib,
        })
    });
    Ok(idx)
}

fn unit_tag(i: &mut Interp) -> SymId {
    i.intern("native-comp-unit")
}

fn unit_value(i: &mut Interp, idx: usize) -> Value {
    let tag = unit_tag(i);
    Value::Vec(Rc::new(std::cell::RefCell::new(vec![
        Value::Sym(tag),
        Value::Int(idx as i128),
    ])))
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
    Err(i.wrong_type_mut("comp-unit-p", &v))
}

/// Whether `s' is one of our native-compiled subrs → trampoline slot.
pub(crate) fn is_native(s: &'static Subr) -> Option<usize> {
    TRAMPS
        .iter()
        .position(|f| *f as usize == s.func as usize)
        .filter(|k| *k < NATIVES.with(|n| n.borrow().len()))
}

/// `subr_interactive_form' hook: stored `(interactive ...)' for a
/// native subr by function name.
pub(crate) fn native_iform_by_name(name: &str) -> Option<Value> {
    META.with(|m| {
        m.borrow()
            .iter()
            .find(|m| m.name == name)
            .map(|m| m.iform.clone())
            .filter(|v| !v.is_nil())
    })
}

fn install_native_subr(
    i: &mut Interp,
    unit: i64,
    name: &str,
    min: u32,
    max: u32,
    entry: EmEntry,
    doc: &str,
) -> Result<Value, ()> {
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
            unit,
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
        name: Box::leak(name.to_string().into_boxed_str()),
        arity,
        func,
        doc: Box::leak(doc.to_string().into_boxed_str()),
    }));
    let id = i.intern(name);
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
};
typedef const struct em_api *A_t;
#define NIL (A->nil)(E)
#define TT (A->t)(E)
#define INT(n) (A->mk_int)(E,(long long)(n))
#define FLT(x) (A->mk_float)(E,(x))
#define TRUTH(h) (A->truthy)(E,(h))
#define CALL(f,av,n) (A->call)(E,(f),(av),(n))
#define SYMSET(s,v) (A->sym_set)(E,(s),(v))
#define LISTN(av,n) (A->listn)(E,(av),(n))
#define RDC(lit) (A->read)(E,(lit),sizeof(lit)-1)
#define SDC(lit) (A->intern)(E,(lit),sizeof(lit)-1)
"#;

/// Sanitize a Lisp symbol name to a C identifier.
fn c_name(name: &str, k: usize) -> String {
    let mut s = String::new();
    for c in name.chars() {
        s.push(match c {
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
}

/// Escape a string into a C string literal (with quotes).
fn c_str(s: &str) -> String {
    let mut o = String::from("\"");
    for b in s.bytes() {
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
    scopes: Vec<Vec<(SymId, String)>>,
    serial: usize,
    /// Set when a form cannot be emitted directly.
    unsupported: bool,
}

impl<'a> Emitter<'a> {
    fn lookup(&self, id: SymId) -> Option<String> {
        self.scopes
            .iter()
            .rev()
            .flat_map(|s| s.iter().rev())
            .find(|(k, _)| *k == id)
            .map(|(_, n)| n.clone())
    }

    /// Emit an expression evaluating to `em_obj'.
    fn form(&mut self, f: &Value) -> String {
        match f {
            Value::Nil => "NIL".into(),
            Value::Int(n) => {
                if let Ok(x) = i64::try_from(*n) {
                    format!("INT({x})")
                } else {
                    let s = n.to_string();
                    format!("RDC({})", c_str(&s))
                }
            }
            Value::Float(x) => format!("FLT({:?})", **x),
            Value::Str(s) => {
                let s = s.borrow();
                format!("(A->mk_str)(E,{},{})", c_str(&s), s.len())
            }
            Value::Sym(id) => {
                if *id == sym::T {
                    return "TT".into();
                }
                if let Some(n) = self.lookup(*id) {
                    return n;
                }
                let name = self.i.symbol_name(*id);
                format!("SDC({})", c_str(&name))
            }
            Value::Cons(c) => {
                let car = c.borrow().car.clone();
                self.cons_form(f, &car)
            }
            // Self-evaluating literals (vectors, records, …):
            // reconstruct from the printed form.
            other => {
                let s = self.i.prin1_to_string(other);
                format!("RDC({})", c_str(&s))
            }
        }
    }

    fn args_of(&self, f: &Value) -> Vec<Value> {
        match f {
            Value::Cons(c) => c
                .borrow()
                .cdr
                .list_to_vec()
                .unwrap_or_default(),
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
                let s = self.i.prin1_to_string(&a[0]);
                format!("RDC({})", c_str(&s))
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
                let mut out = String::from("({em_obj r=NIL;");
                for x in &a {
                    let v = self.form(x);
                    out.push_str(&format!("r={v};if(!TRUTH(r))goto ae;"));
                }
                out.push_str("ae:r;})");
                out
            }
            "or" => {
                let a = self.args_of(whole);
                let mut out = String::from("({em_obj r=NIL;");
                for x in &a {
                    let v = self.form(x);
                    out.push_str(&format!("r={v};if(TRUTH(r))goto oe;"));
                }
                out.push_str("oe:r;})");
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
                                let sn = self.i.symbol_name(*id);
                                out.push_str(&format!(
                                    "r=SYMSET(SDC({}),{v});",
                                    c_str(&sn)
                                ));
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
                        let n = self.i.symbol_name(*id);
                        format!("SDC({})", c_str(&n))
                    }
                    _ => {
                        // #'(lambda …) needs a real closure — eval it
                        // rather than guessing.
                        let s = self.i.prin1_to_string(whole);
                        format!("(A->eval)(E,RDC({}))", c_str(&s))
                    }
                }
            }
            // Non-local exits and excursion forms need real unwind
            // semantics — take the eval fallback for the whole fn.
            "catch" | "throw" | "condition-case" | "unwind-protect"
            | "save-excursion" | "save-restriction" | "save-current-buffer"
            | "save-match-data" | "save-window-excursion" | "track-mouse"
            | "save-selected-window" | "with-local-quit" | "defvar"
            | "defconst" | "interactive" | "declare" | "eval-when-compile"
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
                let fexpr = format!("SDC({})", c_str(&name));
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
        let mid = self.i.intern("macroexpand");
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
        let mut parts = String::from("({em_obj av[]={");
        for x in &argv {
            parts.push_str(&self.form(x));
            parts.push(',');
        }
        parts.push_str(&format!("}};CALL({fexpr},av,{});}})", argv.len()));
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
        let binders = a[0].list_to_vec().unwrap_or_default();
        let mut frame = Vec::new();
        let mut decls = String::new();
        if star {
            self.scopes.push(Vec::new());
        }
        for b in &binders {
            let (id, init) = match b {
                Value::Sym(id) => (*id, Value::Nil),
                Value::Cons(_) => {
                    let v = b.list_to_vec().unwrap_or_default();
                    match v.first() {
                        Some(Value::Sym(id)) => {
                            (*id, v.get(1).cloned().unwrap_or(Value::Nil))
                        }
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
            // Dynamic (special) variables need specbind + unwind
            // protection — the whole function falls back to eval.
            if self.i.obarray.symbol(id).special {
                self.unsupported = true;
                continue;
            }
            let cn = format!("v{}", self.serial);
            self.serial += 1;
            if star {
                self.scopes.last_mut().unwrap().push((id, cn.clone()));
                let iv = self.form(&init);
                decls.push_str(&format!("em_obj {cn}={iv};"));
            } else {
                // `let': every init form sees the outer scope.
                let iv = self.form(&init);
                decls.push_str(&format!("em_obj {cn}={iv};"));
                frame.push((id, cn));
            }
        }
        if !star {
            self.scopes.push(frame);
        }
        let body = self.seq(&a[1..]);
        self.scopes.pop();
        format!("({{{decls}{body};}})")
    }
}

// ---------------------------------------------------------------------------
// Function emission
// ---------------------------------------------------------------------------

fn arglist_syms(i: &mut Interp, l: &crate::lisp::value::Lambda) -> Vec<Value> {
    let mut v: Vec<Value> = l.required.iter().map(|s| Value::Sym(*s)).collect();
    if !l.optional.is_empty() {
        v.push(Value::Sym(i.intern("&optional")));
        for o in &l.optional {
            v.push(Value::Sym(o.sym));
        }
    }
    if let Some(r) = &l.rest {
        v.push(Value::Sym(i.intern("&rest")));
        v.push(Value::Sym(*r));
    }
    v
}

/// Emit one function's entry + descriptor; Err when the function
/// cannot be represented at all (its caller decides the fallback).
fn emit_fn(
    i: &mut Interp,
    name: &str,
    l: &Rc<crate::lisp::value::Lambda>,
    k: usize,
) -> (String, String) {
    let cn = c_name(name, k);
    let min = l.required.len() as u32;
    let max = if l.rest.is_some() {
        u32::MAX
    } else {
        (l.required.len() + l.optional.len()) as u32
    };

    let entry = try_emit_body(i, l, &cn).unwrap_or_else(|| emit_fallback(i, name, l, &cn));
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
            Value::Sym(i.intern("defmacro")),
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
        String::new()
    };
    let desc = format!(
        "static const struct em_fn_desc D{cn}={{\"{name}\",{nl},{min},{mx},{cn},{dstr},{dl},{astr},{al},{istr},{il},{sstr},{sl}}};",
        name = name,
        nl = name.len(),
        mx = if max == u32::MAX {
            "0xffffffffu".to_string()
        } else {
            max.to_string()
        },
        dstr = c_str(&doc_s),
        dl = doc_s.len(),
        astr = c_str(&args_s),
        al = args_s.len(),
        istr = c_str(&iform_s),
        il = iform_s.len(),
        sstr = c_str(&sexp_s),
        sl = sexp_s.len(),
    );
    (entry, desc)
}

/// Direct codegen; None when any form needs eval fallback.
fn try_emit_body(i: &mut Interp, l: &Rc<crate::lisp::value::Lambda>, cn: &str) -> Option<String> {
    let mut e = Emitter {
        i,
        scopes: Vec::new(),
        serial: 0,
        unsupported: false,
    };
    let mut sc = Vec::new();
    for (n, p) in l.required.iter().enumerate() {
        sc.push((*p, format!("p{n}")));
    }
    for (k, p) in l.optional.iter().enumerate() {
        sc.push((p.sym, format!("o{k}")));
    }
    if let Some(r) = &l.rest {
        sc.push((*r, "rest".into()));
    }
    e.scopes.push(sc);

    // Parameter unpacking.
    let mut params = String::new();
    for n in 0..l.required.len() {
        params.push_str(&format!("em_obj p{n}=args[{n}];"));
    }
    for (k, p) in l.optional.iter().enumerate() {
        let idx = l.required.len() + k;
        let d = p
            .default
            .as_ref()
            .map(|dv| e.form(dv))
            .unwrap_or_else(|| "NIL".into());
        params.push_str(&format!("em_obj o{k}=(n>{idx})?args[{idx}]:{d};"));
        if let Some(s) = p.supplied {
            // supplied-p param: expose as an em_obj too.
            let c = format!("v{}", e.serial);
            e.serial += 1;
            e.scopes.last_mut().unwrap().push((s, c.clone()));
            params.push_str(&format!("em_obj {c}=(n>{idx})?TT:NIL;"));
        }
    }
    if l.rest.is_some() {
        let fixed = l.required.len() + l.optional.len();
        params.push_str(&format!(
            "em_obj rest=(n>{fixed})?LISTN(args+{fixed},n-{fixed}):NIL;"
        ));
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
        return None;
    }
    let code = e.seq(&expanded);
    if e.unsupported {
        return None;
    }
    Some(format!(
        "em_obj {cn}(const void *A_, em_env E, const em_obj *args, long n){{A_t A=(A_t)A_;(void)A;{params}return {code};}}"
    ))
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
        let mid = e.i.intern("macroexpand");
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
            Err(_) => return None,
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
    _name: &str,
    l: &Rc<crate::lisp::value::Lambda>,
    cn: &str,
) -> String {
    let env_v = crate::lisp::eval::lexenv_as_value(&l.env);
    let env_s = i.prin1_to_string(&env_v);
    let arglist = l
        .arglist
        .clone()
        .unwrap_or_else(|| Value::list(arglist_syms(i, l)));
    let args_s = i.prin1_to_string(&arglist);
    let body_s = i.prin1_to_string(&Value::list(l.body.clone()));
    let form = format!(
        "(make-interpreted-closure (quote {}) (quote {}) (quote {}))",
        args_s, body_s, env_s
    );
    format!(
        "em_obj {cn}(const void *A_, em_env E, const em_obj *args, long n){{\
           A_t A=(A_t)A_;\
           em_obj cl=(A->eval)(E,RDC({form}));\
           return (A->call_lambda)(E,cl,args,n);}}",
        form = c_str(&form)
    )
}

/// Emit a whole unit (several functions) to C source text.
fn emit_unit(i: &mut Interp, fns: &[(String, Rc<crate::lisp::value::Lambda>)]) -> String {
    let mut out = String::from(C_PRELUDE);
    let mut inits = String::from(
        "\nint remacs_eln_init(const void *api_, em_env E, long U){A_t A=(A_t)api_;(void)A;int rc=0;",
    );
    for (k, (name, l)) in fns.iter().enumerate() {
        let (entry, desc) = emit_fn(i, name, l, k);
        out.push_str(&entry);
        out.push_str(&desc);
        let cn = c_name(name, k);
        inits.push_str(&format!("rc|=A->defsubr(A,E,U,&D{cn});"));
    }
    inits.push_str("return rc;}\n");
    out.push_str(&inits);
    out
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
    let d = format!("{home}/.emacs.d/eln-cache/31.1-remacs");
    let _ = std::fs::create_dir_all(&d);
    d
}

/// `<base>-<md5(truename)>-<md5(contents)>.eln' — GNU's naming.
fn eln_rel_filename(path: &str) -> Result<String, String> {
    let true_name =
        std::fs::canonicalize(path).map_err(|e| format!("{path}: {e}"))?;
    let data =
        std::fs::read(path).map_err(|e| format!("Cannot read {path}: {e}"))?;
    let base = std::path::Path::new(path)
        .file_stem()
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned();
    // Strip a possible second extension like GNU (.el.gz → base).
    let base = base
        .strip_suffix(".el")
        .map(|b| b.to_string())
        .unwrap_or(base);
    Ok(format!(
        "{base}-{}-{}.eln",
        md5_hex(true_name.to_string_lossy().as_bytes()),
        md5_hex(&data)
    ))
}

fn md5_hex(data: &[u8]) -> String {
    md5(data)[..4].iter().map(|b| format!("{b:02x}")).collect()
}

fn md5(msg: &[u8]) -> [u8; 16] {
    const S: [u32; 64] = [
        7, 12, 17, 22, 7, 12, 17, 22, 7, 12, 17, 22, 7, 12, 17, 22, 5, 9, 14, 20, 5, 9,
        14, 20, 5, 9, 14, 20, 5, 9, 14, 20, 4, 11, 16, 23, 4, 11, 16, 23, 4, 11, 16,
        23, 4, 11, 16, 23, 6, 10, 15, 21, 6, 10, 15, 21, 6, 10, 15, 21, 6, 10, 15, 21,
    ];
    const K: [u32; 64] = [
        0xd76aa478, 0xe8c7b756, 0x242070db, 0xc1bdceee, 0xf57c0faf, 0x4787c62a,
        0xa8304613, 0xfd469501, 0x698098d8, 0x8b44f7af, 0xffff5bb1, 0x895cd7be,
        0x6b901122, 0xfd987193, 0xa679438e, 0x49b40821, 0xf61e2562, 0xc040b340,
        0x265e5a51, 0xe9b6c7aa, 0xd62f105d, 0x02441453, 0xd8a1e681, 0xe7d3fbc8,
        0x21e1cde6, 0xc33707d6, 0xf4d50d87, 0x455a14ed, 0xa9e3e905, 0xfcefa3f8,
        0x676f02d9, 0x8d2a4c8a, 0xfffa3942, 0x8771f681, 0x6d9d6122, 0xfde5380c,
        0xa4beea44, 0x4bdecfa9, 0xf6bb4b60, 0xbebfbc70, 0x289b7ec6, 0xeaa127fa,
        0xd4ef3085, 0x04881d05, 0xd9d4d039, 0xe6db99e5, 0x1fa27cf8, 0xc4ac5665,
        0xf4292244, 0x432aff97, 0xab9423a7, 0xfc93a039, 0x655b59c3, 0x8f0ccc92,
        0xffeff47d, 0x85845dd1, 0x6fa87e4f, 0xfe2ce6e0, 0xa3014314, 0x4e0811a1,
        0xf7537e82, 0xbd3af235, 0x2ad7d2bb, 0xeb86d391,
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
fn run_cc(c_src: &str, out_path: &str) -> Result<(), String> {
    let cc = cc().ok_or("No C compiler found (set REMACS_CC)")?;
    let mut child = std::process::Command::new(cc)
        .args([
            "-O1",
            "-fPIC",
            "-shared",
            "-Wno-unused",
            "-x",
            "c",
            "-",
            "-o",
            out_path,
        ])
        .stdin(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| format!("cc failed to start: {e}"))?;
    child
        .stdin
        .as_mut()
        .unwrap()
        .write_all(c_src.as_bytes())
        .map_err(|e| format!("cc stdin: {e}"))?;
    let st = child
        .wait_with_output()
        .map_err(|e| format!("cc wait: {e}"))?;
    if st.status.success() {
        Ok(())
    } else {
        Err(format!(
            "cc exited {}: {}",
            st.status,
            String::from_utf8_lossy(&st.stderr)
        ))
    }
}

// ---------------------------------------------------------------------------
// High-level compile entry
// ---------------------------------------------------------------------------

/// Collect `(defun …)'/`(defmacro …)' top-level forms of a file.
fn file_defuns(
    i: &mut Interp,
    path: &str,
) -> Result<Vec<(String, Rc<crate::lisp::value::Lambda>)>, String> {
    let data =
        std::fs::read_to_string(path).map_err(|e| format!("Cannot read {path}: {e}"))?;
    let mut out = Vec::new();
    let mut off = 0usize;
    while off < data.len() {
        match i.read_from_string(&data, off) {
            Ok((v, end)) => {
                if end <= off {
                    break;
                }
                off = end;
                if let Value::Cons(c) = &v {
                    let car = c.borrow().car.clone();
                    if let Value::Sym(id) = car {
                        let hname = i.symbol_name(id);
                        if hname == "defun" || hname == "defmacro" {
                            let parts = v.list_to_vec().unwrap_or_default();
                            if parts.len() >= 3 {
                                if let Value::Sym(n) = parts[1] {
                                    let lam = form_to_lambda(
                                        i,
                                        &parts[2],
                                        &parts[3..],
                                        hname == "defmacro",
                                    );
                                    let nm = i.symbol_name(n);
                                    out.push((nm, Rc::new(lam)));
                                }
                            }
                            // Like the byte-compiler, a `defmacro' is
                            // live during compilation so later forms
                            // expand through it.
                            if hname == "defmacro" {
                                let _ = i.eval(&v);
                            }
                        }
                    }
                }
            }
            Err(_) => break,
        }
    }
    Ok(out)
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
    let opt_id = i.intern("&optional");
    let rest_id = i.intern("&rest");
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
        advice_link: None,
        bc_items: None,
        doc_value: None,
        env_value: None,
    }
}

fn compile_fns_to(
    i: &mut Interp,
    fns: &[(String, Rc<crate::lisp::value::Lambda>)],
    out: &str,
) -> Result<(), String> {
    let src = emit_unit(i, fns);
    if let Ok(p) = std::env::var("REMACS_COMP_DUMP_C") {
        let _ = std::fs::write(format!("{p}.c"), &src);
    }
    run_cc(&src, out)
}

// ---------------------------------------------------------------------------
// DEFUNs
// ---------------------------------------------------------------------------

fn f_native_comp_available_p(_i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    Ok(Value::from_bool(compiler_available()))
}

/// `(load "x.eln")' entry: dlopen a native unit file.
pub(crate) fn native_load_file(i: &mut Interp, path: &str) -> EvalResult {
    match load_eln(i, path) {
        Ok(idx) => Ok(unit_value(i, idx)),
        Err(e) => Err(i.error(e)),
    }
}

fn f_native_elisp_load(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let path = match &a[0] {
        Value::Str(s) => s.borrow().clone(),
        other => return Err(i.wrong_type_mut("stringp", &other)),
    };
    match load_eln(i, &path) {
        Ok(idx) => Ok(unit_value(i, idx)),
        Err(e) => Err(i.error(e)),
    }
}

fn f_comp_el_to_eln_rel_filename(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let path = match &a[0] {
        Value::Str(s) => s.borrow().clone(),
        other => return Err(i.wrong_type_mut("stringp", &other)),
    };
    match eln_rel_filename(&path) {
        Ok(p) => Ok(Value::string(p)),
        Err(e) => Err(i.error(e)),
    }
}

fn f_comp_el_to_eln_filename(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let path = match &a[0] {
        Value::Str(s) => s.borrow().clone(),
        other => return Err(i.wrong_type_mut("stringp", &other)),
    };
    let rel = match eln_rel_filename(&path) {
        Ok(p) => p,
        Err(e) => return Err(i.error(e)),
    };
    let base = match a.get(1) {
        Some(Value::Str(s)) => s.borrow().clone(),
        _ => format!("{}/", eln_cache_dir()),
    };
    let base = if base.ends_with('/') {
        base
    } else {
        format!("{base}/")
    };
    Ok(Value::string(format!("{base}{rel}")))
}

fn f_native_compile(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    if !compiler_available() {
        return Err(i.error("Cannot native-compile: no C compiler available"));
    }
    match &a[0] {
        Value::Str(s) => {
            let path = s.borrow().clone();
            let out = match a.get(1) {
                Some(Value::Str(o)) => o.borrow().clone(),
                _ => {
                    let rel = match eln_rel_filename(&path) {
                        Ok(r) => r,
                        Err(e) => return Err(i.error(e)),
                    };
                    format!("{}/{rel}", eln_cache_dir())
                }
            };
            let fns = match file_defuns(i, &path) {
                Ok(v) if !v.is_empty() => v,
                Ok(_) => {
                    return Err(i.error(format!("No defuns to compile in {path}")));
                }
                Err(e) => return Err(i.error(e)),
            };
            match compile_fns_to(i, &fns, &out) {
                Ok(()) => Ok(Value::string(out)),
                Err(e) => Err(i.error(e)),
            }
        }
        Value::Sym(id) => {
            let f = i.symbol_function(*id);
            let name = i.symbol_name(*id);
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

fn compile_one(i: &mut Interp, name: &str, f: &Value, out: Option<&Value>) -> EvalResult {
    match f {
        Value::Lambda(l) => {
            let l = l.clone();
            compile_one_lambda(i, name, &l, out)
        }
        // Already compiled — idempotent, like GNU re-using the unit.
        Value::Subr(s) if is_native(s).is_some() => Ok(f.clone()),
        other => Err(i.wrong_type_mut("functionp", &other)),
    }
}

fn compile_one_lambda(
    i: &mut Interp,
    name: &str,
    l: &Rc<crate::lisp::value::Lambda>,
    out: Option<&Value>,
) -> EvalResult {
    let out_path = match out {
        Some(Value::Str(s)) => s.borrow().clone(),
        _ => format!("{}/{}-{}.eln", eln_cache_dir(), sanitize(name), unit_serial()),
    };
    let fns = vec![(name.to_string(), l.clone())];
    match compile_fns_to(i, &fns, &out_path) {
        Ok(()) => match load_eln(i, &out_path) {
            Ok(_) => {
                let id = i.intern(name);
                let v = i.symbol_function(id);
                match v {
                    Value::Subr(_) => Ok(v),
                    _ => Ok(Value::string(out_path)),
                }
            }
            Err(e) => Err(i.error(e)),
        },
        Err(e) => Err(i.error(e)),
    }
}

fn sanitize(n: &str) -> String {
    c_name(n, 0)
}

static SERIAL: Mutex<u64> = Mutex::new(0);
fn unit_serial() -> u64 {
    let mut s = SERIAL.lock().unwrap();
    *s += 1;
    *s
}

fn f_native_compile_async(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let files: Vec<String> = match &a[0] {
        Value::Str(s) => vec![s.borrow().clone()],
        Value::Nil => Vec::new(),
        other => other
            .list_to_vec()
            .unwrap_or_default()
            .iter()
            .filter_map(|v| {
                if let Value::Str(s) = v {
                    Some(s.borrow().clone())
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
            let rel = match eln_rel_filename(&p) {
                Ok(r) => r,
                Err(e) => return Err(i.error(e)),
            };
            let out = format!("{}/{rel}", eln_cache_dir());
            let fns = file_defuns(i, &p).unwrap_or_default();
            if fns.is_empty() {
                continue;
            }
            compile_fns_to(i, &fns, &out).map_err(|e| i.error(e))?;
            if load {
                load_eln(i, &out).map_err(|e| i.error(e))?;
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

fn f_native_comp_function_p(_i: &mut Interp, a: Vec<Value>) -> EvalResult {
    Ok(Value::from_bool(match &a[0] {
        Value::Subr(s) => is_native(s).is_some(),
        _ => false,
    }))
}

fn f_subr_native_lambda_list(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    match &a[0] {
        Value::Subr(s) => {
            if let Some(k) = is_native(s) {
                Ok(META.with(|m| m.borrow()[k].arglist.clone()))
            } else {
                Ok(Value::t())
            }
        }
        other => Err(i.wrong_type_mut("subrp", &other)),
    }
}

fn f_subr_native_comp_unit(i: &mut Interp, a: Vec<Value>) -> EvalResult {
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
        other => Err(i.wrong_type_mut("subrp", &other)),
    }
}

fn f_native_comp_unit_file(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let idx = unit_index(i, &a[0])?;
    UNITS.with(|u| match u.borrow().get(idx) {
        Some(u) => Ok(Value::string(u.path.clone())),
        None => Ok(Value::Nil),
    })
}

fn f_native_comp_unit_set_file(i: &mut Interp, a: Vec<Value>) -> EvalResult {
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

fn f_comp_libgccjit_version(_i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    // remacs emits C and drives the system compiler — no libgccjit.
    Ok(Value::Nil)
}

fn f_comp_subr_signature(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    // GNU resolves a symbol to its function cell here.
    let v = match &a[0] {
        Value::Sym(id) => i.symbol_function(*id),
        other => other.clone(),
    };
    match &v {
        Value::Subr(s) => {
            let (min, max) = match s.arity {
                Arity::Range { min, max } => {
                    (Value::Int(min as i128), Value::Int(max as i128))
                }
                Arity::Many { min } => (
                    Value::Int(min as i128),
                    Value::Sym(i.intern("many")),
                ),
                Arity::Unevalled => (Value::Int(0), Value::Sym(i.intern("many"))),
            };
            Ok(Value::cons(
                Value::Sym(i.intern(s.name)),
                Value::cons(min, max),
            ))
        }
        other => Err(i.wrong_type_mut("subrp", &other)),
    }
}

/// Pending ctxt source for `comp--compile-ctxt-to-file0' — our own
/// pipeline doesn't need the ctxt builder calls (comp.el's LIMPLE
/// pass targets GNU's ABI), but the trio stays honest.
fn f_comp_init_ctxt(_i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    CTXT.with(|c| *c.borrow_mut() = Some(String::new()));
    Ok(Value::t())
}

fn f_comp_release_ctxt(_i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    CTXT.with(|c| *c.borrow_mut() = None);
    Ok(Value::Nil)
}

fn f_comp_compile_ctxt_to_file0(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let path = match &a[0] {
        Value::Str(s) => s.borrow().clone(),
        other => return Err(i.wrong_type_mut("stringp", &other)),
    };
    let src = CTXT.with(|c| c.borrow().clone()).unwrap_or_default();
    if src.is_empty() {
        return Err(i.error("Nothing in compilation context to emit"));
    }
    match run_cc(&src, &path) {
        Ok(()) => Ok(Value::string(path)),
        Err(e) => Err(i.error(e)),
    }
}

/// `comp--install-trampoline' (SUBR-NAME TRAMPOLINE): TRAMPOLINE is an
/// .eln file exporting `comp_trampoline'; install it as SUBR-NAME's
/// definition (advice/late-trampoline machinery).
fn f_comp_install_trampoline(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let name = match &a[0] {
        Value::Sym(id) => i.symbol_name(*id),
        Value::Str(s) => s.borrow().clone(),
        other => return Err(i.wrong_type_mut("symbolp", &other)),
    };
    let file = match &a[1] {
        Value::Str(s) => s.borrow().clone(),
        other => return Err(i.wrong_type_mut("stringp", &other)),
    };
    let lib = match unsafe { libloading::Library::new(&file) } {
        Ok(l) => l,
        Err(e) => return Err(i.error(format!("Cannot load trampoline: {e}"))),
    };
    let entry: Option<EmEntry> =
        unsafe { crate::lisp::dynlib::sym(&lib, b"comp_trampoline\0") };
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
fn f_comp_register_subr(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    register_subr_common(i, &a)
}

fn register_subr_common(i: &mut Interp, a: &[Value]) -> EvalResult {
    let name = match &a[0] {
        Value::Sym(id) => i.symbol_name(*id),
        Value::Str(s) => s.borrow().clone(),
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
fn f_comp_register_lambda(i: &mut Interp, a: Vec<Value>) -> EvalResult {
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
            name: cname.clone(),
            unit: unit_idx as i64,
            arglist: Value::Nil,
            iform: Value::Nil,
        })
    });
    let s: &'static Subr = Box::leak(Box::new(Subr {
        name: Box::leak(cname.into_boxed_str()),
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
    Ok(Value::Subr(s))
}

fn f_comp_native_compiler_options_effective_p(_i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    Ok(Value::from_bool(compiler_available()))
}
fn f_comp_native_driver_options_effective_p(_i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    Ok(Value::from_bool(compiler_available()))
}

// ---------------------------------------------------------------------------
// SUBRS table + install
// ---------------------------------------------------------------------------

pub(crate) const SUBRS: &[Subr] = &[
    S!("native-comp-available-p", 0, 0, f_native_comp_available_p,
       "Return non-nil if native compilation support is built-in."),
    S!("native-elisp-load", 1, 2, f_native_elisp_load,
       "Load native elisp code FILENAME."),
    S!("comp-el-to-eln-filename", 1, 2, f_comp_el_to_eln_filename,
       "Return the absolute .eln file name for source FILENAME."),
    S!("comp-el-to-eln-rel-filename", 1, 1, f_comp_el_to_eln_rel_filename,
       "Return the relative name of the .eln file for FILENAME."),
    // The public `native-compile'/`native-compile-async' cells are
    // rebound to Lisp wrappers after startup loads (see eval.rs) —
    // comp.el's LIMPLE pipeline can't run in remacs, so the loaddefs
    // autoloads must not win.
    S!("comp--remacs-native-compile", 1, 2, f_native_compile,
       "Compile FUNCTION-OR-FILE into native code."),
    S!("comp--remacs-native-compile-async", 1, 4, f_native_compile_async,
       "Compile FILES asynchronously (synchronous in remacs)."),
    S!("native-comp-function-p", 1, 1, f_native_comp_function_p,
       "Return t if OBJECT is a native-compiled Lisp function."),
    S!("subr-native-lambda-list", 1, 1, f_subr_native_lambda_list,
       "Return the lambda list for a native-compiled function."),
    S!("subr-native-comp-unit", 1, 1, f_subr_native_comp_unit,
       "Return the native compilation unit."),
    S!("native-comp-unit-file", 1, 1, f_native_comp_unit_file,
       "Return the file of the native compilation unit."),
    S!("native-comp-unit-set-file", 2, 2, f_native_comp_unit_set_file,
       "Set the file of the native compilation unit."),
    S!("comp-libgccjit-version", 0, 0, f_comp_libgccjit_version,
       "Return libgccjit version (nil — remacs uses cc)."),
    S!("comp--subr-signature", 1, 1, f_comp_subr_signature,
       "Support function for internal ABI hashing."),
    S!("comp--init-ctxt", 0, 0, f_comp_init_ctxt,
       "Initialize the native compiler context."),
    S!("comp--release-ctxt", 0, 0, f_comp_release_ctxt,
       "Release the native compiler context."),
    S!("comp--compile-ctxt-to-file0", 1, 1, f_comp_compile_ctxt_to_file0,
       "Compile the current context as native code to file FILENAME."),
    S!("comp--install-trampoline", 2, 2, f_comp_install_trampoline,
       "Install a TRAMPOLINE for primitive SUBR-NAME."),
    S!("comp--register-subr", 7, 7, f_comp_register_subr,
       "Register exported subr (called by .eln load code)."),
    S!("comp--late-register-subr", 7, 7, f_comp_register_subr,
       "Register exported subr at late load phase."),
    S!("comp--register-lambda", 7, 7, f_comp_register_lambda,
       "Register anonymous lambda from a comp unit."),
    S!("comp-native-compiler-options-effective-p", 0, 0,
       f_comp_native_compiler_options_effective_p,
       "Return t if `comp-native-compiler-options' is effective."),
    S!("comp-native-driver-options-effective-p", 0, 0,
       f_comp_native_driver_options_effective_p,
       "Return t if `comp-native-driver-options' is effective."),
];

pub(crate) fn install(i: &mut Interp) {
    // The `native-comp' feature is registered only when a working C
    // compiler exists — same contract as GNU's deferred probe.
    if compiler_available() {
        let f = i.intern("native-comp");
        if !i.features.contains(&f) {
            i.features.push(f);
        }
    }
}

// ---------------------------------------------------------------------------
// Trampoline pool — generated 0..255
// ---------------------------------------------------------------------------

tramps!(t0 => 0, t1 => 1, t2 => 2, t3 => 3, t4 => 4, t5 => 5, t6 => 6, t7 => 7, t8 => 8, t9 => 9, t10 => 10, t11 => 11, t12 => 12, t13 => 13, t14 => 14, t15 => 15, t16 => 16, t17 => 17, t18 => 18, t19 => 19, t20 => 20, t21 => 21, t22 => 22, t23 => 23, t24 => 24, t25 => 25, t26 => 26, t27 => 27, t28 => 28, t29 => 29, t30 => 30, t31 => 31, t32 => 32, t33 => 33, t34 => 34, t35 => 35, t36 => 36, t37 => 37, t38 => 38, t39 => 39, t40 => 40, t41 => 41, t42 => 42, t43 => 43, t44 => 44, t45 => 45, t46 => 46, t47 => 47, t48 => 48, t49 => 49, t50 => 50, t51 => 51, t52 => 52, t53 => 53, t54 => 54, t55 => 55, t56 => 56, t57 => 57, t58 => 58, t59 => 59, t60 => 60, t61 => 61, t62 => 62, t63 => 63, t64 => 64, t65 => 65, t66 => 66, t67 => 67, t68 => 68, t69 => 69, t70 => 70, t71 => 71, t72 => 72, t73 => 73, t74 => 74, t75 => 75, t76 => 76, t77 => 77, t78 => 78, t79 => 79, t80 => 80, t81 => 81, t82 => 82, t83 => 83, t84 => 84, t85 => 85, t86 => 86, t87 => 87, t88 => 88, t89 => 89, t90 => 90, t91 => 91, t92 => 92, t93 => 93, t94 => 94, t95 => 95, t96 => 96, t97 => 97, t98 => 98, t99 => 99, t100 => 100, t101 => 101, t102 => 102, t103 => 103, t104 => 104, t105 => 105, t106 => 106, t107 => 107, t108 => 108, t109 => 109, t110 => 110, t111 => 111, t112 => 112, t113 => 113, t114 => 114, t115 => 115, t116 => 116, t117 => 117, t118 => 118, t119 => 119, t120 => 120, t121 => 121, t122 => 122, t123 => 123, t124 => 124, t125 => 125, t126 => 126, t127 => 127, t128 => 128, t129 => 129, t130 => 130, t131 => 131, t132 => 132, t133 => 133, t134 => 134, t135 => 135, t136 => 136, t137 => 137, t138 => 138, t139 => 139, t140 => 140, t141 => 141, t142 => 142, t143 => 143, t144 => 144, t145 => 145, t146 => 146, t147 => 147, t148 => 148, t149 => 149, t150 => 150, t151 => 151, t152 => 152, t153 => 153, t154 => 154, t155 => 155, t156 => 156, t157 => 157, t158 => 158, t159 => 159, t160 => 160, t161 => 161, t162 => 162, t163 => 163, t164 => 164, t165 => 165, t166 => 166, t167 => 167, t168 => 168, t169 => 169, t170 => 170, t171 => 171, t172 => 172, t173 => 173, t174 => 174, t175 => 175, t176 => 176, t177 => 177, t178 => 178, t179 => 179, t180 => 180, t181 => 181, t182 => 182, t183 => 183, t184 => 184, t185 => 185, t186 => 186, t187 => 187, t188 => 188, t189 => 189, t190 => 190, t191 => 191, t192 => 192, t193 => 193, t194 => 194, t195 => 195, t196 => 196, t197 => 197, t198 => 198, t199 => 199, t200 => 200, t201 => 201, t202 => 202, t203 => 203, t204 => 204, t205 => 205, t206 => 206, t207 => 207, t208 => 208, t209 => 209, t210 => 210, t211 => 211, t212 => 212, t213 => 213, t214 => 214, t215 => 215, t216 => 216, t217 => 217, t218 => 218, t219 => 219, t220 => 220, t221 => 221, t222 => 222, t223 => 223, t224 => 224, t225 => 225, t226 => 226, t227 => 227, t228 => 228, t229 => 229, t230 => 230, t231 => 231, t232 => 232, t233 => 233, t234 => 234, t235 => 235, t236 => 236, t237 => 237, t238 => 238, t239 => 239, t240 => 240, t241 => 241, t242 => 242, t243 => 243, t244 => 244, t245 => 245, t246 => 246, t247 => 247, t248 => 248, t249 => 249, t250 => 250, t251 => 251, t252 => 252, t253 => 253, t254 => 254, t255 => 255);

const TRAMPS: [SubrFn; 256] = [
    t0, t1, t2, t3, t4, t5, t6, t7, t8, t9, t10, t11, t12, t13, t14, t15,
    t16, t17, t18, t19, t20, t21, t22, t23, t24, t25, t26, t27, t28, t29,
    t30, t31, t32, t33, t34, t35, t36, t37, t38, t39, t40, t41, t42, t43,
    t44, t45, t46, t47, t48, t49, t50, t51, t52, t53, t54, t55, t56, t57,
    t58, t59, t60, t61, t62, t63, t64, t65, t66, t67, t68, t69, t70, t71,
    t72, t73, t74, t75, t76, t77, t78, t79, t80, t81, t82, t83, t84, t85,
    t86, t87, t88, t89, t90, t91, t92, t93, t94, t95, t96, t97, t98, t99,
    t100, t101, t102, t103, t104, t105, t106, t107, t108, t109, t110, t111,
    t112, t113, t114, t115, t116, t117, t118, t119, t120, t121, t122, t123,
    t124, t125, t126, t127, t128, t129, t130, t131, t132, t133, t134, t135,
    t136, t137, t138, t139, t140, t141, t142, t143, t144, t145, t146, t147,
    t148, t149, t150, t151, t152, t153, t154, t155, t156, t157, t158, t159,
    t160, t161, t162, t163, t164, t165, t166, t167, t168, t169, t170, t171,
    t172, t173, t174, t175, t176, t177, t178, t179, t180, t181, t182, t183,
    t184, t185, t186, t187, t188, t189, t190, t191, t192, t193, t194, t195,
    t196, t197, t198, t199, t200, t201, t202, t203, t204, t205, t206, t207,
    t208, t209, t210, t211, t212, t213, t214, t215, t216, t217, t218, t219,
    t220, t221, t222, t223, t224, t225, t226, t227, t228, t229, t230, t231,
    t232, t233, t234, t235, t236, t237, t238, t239, t240, t241, t242, t243,
    t244, t245, t246, t247, t248, t249, t250, t251, t252, t253, t254, t255,
];
