//! Primitive functions (subrs) implemented in Rust.
//!
//! Like Emacs's `DEFUN`s in C. Each subr is a static `Subr` with a name,
//! arity, and a `fn(&mut Interp, Vec<Value>) -> EvalResult`.

pub(crate) mod arith;
pub(crate) mod backquote;
pub(crate) mod bidi_table;
pub(crate) mod charset;
pub(crate) mod coding_detect;
pub(crate) mod coding_detection;
pub(crate) mod coding_iso2022;
pub(crate) mod coding_legacy;
pub(crate) mod coding_stream;
pub(crate) mod coding_unicode;
pub(crate) mod comp;
pub(crate) mod csl_table;
pub(crate) mod data;
pub(crate) mod dbus;
pub(crate) mod enc_tables;
pub(crate) mod evalfn;
pub(crate) mod filenotify;
pub(crate) mod gnutls;
pub(crate) mod hashfn;
pub(crate) mod json;
pub(crate) mod lcms;
pub(crate) mod listfn;
pub mod misc;
pub(crate) mod nsgui;
pub(crate) mod printfn;
pub(crate) mod readfn;
pub(crate) mod seq;
pub(crate) mod sort;
pub(crate) mod sqlite;
pub(crate) mod strfn;
pub(crate) mod timefmt;
pub(crate) mod treesit;
pub(crate) mod xml;

use super::Interp;
use super::error::Flow;
use super::obarray::sym;
use super::value::{Arity, FIXNUM_MAX, FIXNUM_MIN, Subr, SymId, Value};
use num_traits::ToPrimitive;

/// Declare a subr entry.
macro_rules! S {
    // fixed arity
    ($name:literal, $min:expr, $max:expr, $f:expr, $doc:literal) => {
        $crate::lisp::value::Subr {
            name: $name,
            arity: $crate::lisp::value::Arity::Range {
                min: $min,
                max: $max,
            },
            func: $f,
            doc: $doc,
        }
    };
    // MANY (at least $min args)
    ($name:literal, many $min:expr, $f:expr, $doc:literal) => {
        $crate::lisp::value::Subr {
            name: $name,
            arity: $crate::lisp::value::Arity::Many { min: $min },
            func: $f,
            doc: $doc,
        }
    };
    // UNEVALLED (special-form-style raw args)
    ($name:literal, raw, $f:expr, $doc:literal) => {
        $crate::lisp::value::Subr {
            name: $name,
            arity: $crate::lisp::value::Arity::Unevalled,
            func: $f,
            doc: $doc,
        }
    };
}

pub(crate) use S;
pub use misc::error_message;

/// All subrs, aggregated from the category modules.
pub(crate) fn collect() -> Vec<&'static Subr> {
    let mut v: Vec<&'static Subr> = Vec::new();
    v.extend(backquote::SUBRS);
    v.extend(arith::SUBRS);
    v.extend(data::SUBRS);
    v.extend(evalfn::SUBRS);
    v.extend(hashfn::SUBRS);
    v.extend(json::SUBRS);
    v.extend(listfn::SUBRS);
    v.extend(misc::SUBRS);
    v.extend(printfn::SUBRS);
    v.extend(readfn::SUBRS);
    v.extend(seq::SUBRS);
    v.extend(strfn::SUBRS);
    v.extend(charset::SUBRS);
    v.extend(sqlite::SUBRS);
    v.extend(xml::SUBRS);
    v.extend(treesit::SUBRS);
    v.extend(lcms::SUBRS);
    v.extend(filenotify::SUBRS);
    v.extend(gnutls::SUBRS);
    v.extend(dbus::SUBRS);
    v.extend(nsgui::SUBRS);
    v.extend(comp::SUBRS);
    v.extend(crate::lisp::process::SUBRS);
    v
}

/// Stable within one executable fingerprint, including private macro subrs.
pub(crate) fn dump_subrs() -> Vec<&'static Subr> {
    let mut subrs = collect();
    subrs.extend(SPECIAL_FORM_SUBRS);
    subrs.extend(backquote::dump_macro_subrs());
    subrs.extend(crate::editor::SUBRS);
    subrs.extend(crate::editor::winxtra::SUBRS);
    subrs.extend(crate::buffer::primitives::SUBRS);
    subrs.extend(crate::buffer::extra::SUBRS);
    subrs
}

/// Calls made by native-compiled dumped Lisp can use a C subr directly until
/// fset installs its trampoline. Dedicated inlined byte-code operations use
/// the canonical entry even when the symbol's function cell has changed.
pub(crate) fn call_native_builtin(interp: &mut Interp, name: &str, args: Vec<Value>) -> Result<Value, Flow> {
    if interp.native_trampolines.contains(name) {
        let function = Value::Sym(interp.q(name));
        return interp.apply(&function, args);
    }
    call_canonical_builtin(interp, name, args)
}

pub(crate) fn call_canonical_builtin(interp: &mut Interp, name: &str, args: Vec<Value>) -> Result<Value, Flow> {
    static SUBRS: std::sync::OnceLock<rustc_hash::FxHashMap<&'static str, &'static Subr>> = std::sync::OnceLock::new();
    let subrs = SUBRS.get_or_init(|| dump_subrs().into_iter().map(|subr| (subr.name, subr)).collect());
    let Some(subr) = subrs.get(name) else { return Err(interp.error(format!("Unknown native primitive: {name}"))); };
    let result = (subr.func)(interp, args.into());
    interp.offer_flow(result)
}

/// Intern every subr name and bind its function cell.
pub fn install(interp: &mut Interp) {
    let _t = std::env::var_os("REMACS_BOOT_PROF")
        .is_some()
        .then(std::time::Instant::now);
    for s in collect() {
        let id = interp.intern(s.name);
        interp.fset(id, Value::Subr(s));
    }
    if let Some(t) = _t {
        eprintln!("[install] collect+fset: {}ms", t.elapsed().as_millis());
    }
    let _t = std::env::var_os("REMACS_BOOT_PROF")
        .is_some()
        .then(std::time::Instant::now);
    // Special forms are subrs in Emacs: `symbol-function'/'subrp'/'fboundp'
    // must see them. Calling one through funcall signals invalid-function.
    for s in SPECIAL_FORM_SUBRS {
        let id = interp.intern(s.name);
        interp.fset(id, Value::Subr(s));
    }
    if let Some(t) = _t {
        eprintln!("[install] special: {}ms", t.elapsed().as_millis());
    }
    macro_rules! timed {
        ($f:expr, $n:literal) => {{
            let _t2 = std::env::var_os("REMACS_BOOT_PROF")
                .is_some()
                .then(std::time::Instant::now);
            $f;
            if let Some(t2) = _t2 {
                eprintln!("[install] {}: {}ms", $n, t2.elapsed().as_millis());
            }
        }};
    }
    timed!(sqlite::install(interp), "sqlite");
    timed!(lcms::install(interp), "lcms");
    timed!(filenotify::install(interp), "filenotify");
    timed!(gnutls::install(interp), "gnutls");
    timed!(dbus::install(interp), "dbus");
    timed!(comp::install(interp), "comp");
    timed!(crate::lisp::process::install(interp), "process");
    timed!(install_aliases(interp), "aliases");
}

/// Re-register runtime (dlopen-gated) features after the post-boot
/// `features' list is restored from the dump snapshot.
pub fn register_extlib_features(interp: &mut Interp) {
    lcms::install(interp);
    filenotify::install(interp);
    gnutls::install(interp);
    dbus::install(interp);
    comp::install(interp);
}

fn sf_cannot_call(i: &mut Interp, a: crate::lisp::Argv) -> super::error::EvalResult {
    let _ = a;
    Err(i.signal_data(sym::INVALID_FUNCTION, vec![Value::Nil]))
}

/// Special forms (dispatch lives in `lisp::special`).
static SPECIAL_FORM_SUBRS: &[Subr] = &[
    Subr {
        name: "quote",
        arity: Arity::Unevalled,
        func: sf_cannot_call,
        doc: "",
    },
    Subr {
        name: "function",
        arity: Arity::Unevalled,
        func: sf_cannot_call,
        doc: "",
    },
    Subr {
        name: "if",
        arity: Arity::Unevalled,
        func: sf_cannot_call,
        doc: "",
    },
    Subr {
        name: "cond",
        arity: Arity::Unevalled,
        func: sf_cannot_call,
        doc: "",
    },
    Subr {
        name: "progn",
        arity: Arity::Unevalled,
        func: sf_cannot_call,
        doc: "",
    },
    Subr {
        name: "prog1",
        arity: Arity::Unevalled,
        func: sf_cannot_call,
        doc: "",
    },
    Subr {
        name: "and",
        arity: Arity::Unevalled,
        func: sf_cannot_call,
        doc: "",
    },
    Subr {
        name: "or",
        arity: Arity::Unevalled,
        func: sf_cannot_call,
        doc: "",
    },
    Subr {
        name: "let",
        arity: Arity::Unevalled,
        func: sf_cannot_call,
        doc: "",
    },
    Subr {
        name: "let*",
        arity: Arity::Unevalled,
        func: sf_cannot_call,
        doc: "",
    },
    Subr {
        name: "setq",
        arity: Arity::Unevalled,
        func: sf_cannot_call,
        doc: "",
    },
    Subr {
        name: "setq-default",
        arity: Arity::Unevalled,
        func: sf_cannot_call,
        doc: "",
    },
    Subr {
        name: "defvar",
        arity: Arity::Unevalled,
        func: sf_cannot_call,
        doc: "",
    },
    Subr {
        name: "defconst",
        arity: Arity::Unevalled,
        func: sf_cannot_call,
        doc: "",
    },
    // GNU 31.1: `defun', `defmacro', `prog2', `lambda',
    // `with-current-buffer', `save-mark-and-excursion', `backquote'
    // are Lisp macros — no Unevalled subr registration, or
    // `special-form-p'/indirect-function would see a special.
    Subr {
        name: "while",
        arity: Arity::Unevalled,
        func: sf_cannot_call,
        doc: "",
    },
    Subr {
        name: "catch",
        arity: Arity::Unevalled,
        func: sf_cannot_call,
        doc: "",
    },
    Subr {
        name: "unwind-protect",
        arity: Arity::Unevalled,
        func: sf_cannot_call,
        doc: "",
    },
    Subr {
        name: "condition-case",
        arity: Arity::Unevalled,
        func: sf_cannot_call,
        doc: "",
    },
    Subr {
        name: "interactive",
        arity: Arity::Unevalled,
        func: sf_cannot_call,
        doc: "",
    },
    Subr {
        name: "save-excursion",
        arity: Arity::Unevalled,
        func: sf_cannot_call,
        doc: "",
    },
    Subr {
        name: "save-current-buffer",
        arity: Arity::Unevalled,
        func: sf_cannot_call,
        doc: "",
    },
    Subr {
        name: "save-restriction",
        arity: Arity::Unevalled,
        func: sf_cannot_call,
        doc: "",
    },
];
// `track-mouse' is deliberately absent: it is a Lisp macro in GNU 31.1
// (`special-form-p' must return nil), not a special form.

/// `defalias`-style alternate names.
fn install_aliases(interp: &mut Interp) {
    let aliases: &[(&str, &str)] = &[
        // GNU direction: the verbose name is the primitive; the short name
        // is a Lisp alias (installed by the prelude).
        ("same-names-p", "string="),
        ("buffer-name-as-string", "buffer-name"),
    ];
    for (alias, target) in aliases {
        let aid = interp.intern(alias);
        let tid = interp.intern(target);
        interp.fset(aid, Value::Sym(tid));
    }
}

// ---------- shared helpers for subr bodies ----------

/// Fetch arg or signal.
pub(crate) fn arg(args: &[Value], i: usize) -> Value {
    args.get(i).cloned().unwrap_or(Value::Nil)
}

pub(crate) fn want_int(i: &mut Interp, v: &Value) -> Result<i128, Flow> {
    match v {
        Value::Int(n) => Ok(*n),
        Value::BigInt(n) => n.to_i128().ok_or_else(|| {
            let condition = i.q("args-out-of-range");
            i.signal_data(condition, vec![v.clone()])
        }),
        _ => Err(i.wrong_type_mut("integerp", v)),
    }
}

/// GNU `CHECK_FIXNUM' rejects bignums rather than projecting them into
/// a bounded Rust integer. `aref', `aset' and related array operations
/// use the `fixnump' error predicate.
pub(crate) fn want_fixnum(i: &mut Interp, v: &Value) -> Result<i128, Flow> {
    // GNU CHECK_FIXNUM: fixnums are signed 62-bit (MOST_POSITIVE_FIXNUM
    // = 2^61-1); larger integers (bignum literals) signal too.
    match v {
        Value::Int(n) if (-(1i128 << 61)..=((1i128 << 61) - 1)).contains(n) => Ok(*n),
        _ => Err(i.wrong_type_mut("fixnump", v)),
    }
}

/// GNU `integer_or_marker' / `decode_buffer_position': ints pass
/// through, markers yield their position (+1 bias), other values
/// signal `wrong-type-argument integer-or-marker-p'.
/// GNU CHECK_FIXNUM_COERCE_MARKER: a marker with no buffer signals
/// "Marker does not point anywhere"; a live marker yields its 1-based
/// position; anything else signals `wrong-type-argument
/// integer-or-marker-p'.
pub(crate) fn want_int_or_mark(i: &mut Interp, v: &Value) -> Result<i128, Flow> {
    match v {
        Value::Int(n) => Ok(*n),
        Value::BigInt(_) => want_int(i, v),
        Value::Marker(m) if m.borrow().buffer.is_some() => Ok(m.borrow().position as i128 + 1),
        Value::Marker(_) => Err(i.error("Marker does not point anywhere")),
        other => Err(i.wrong_type_mut("integer-or-marker-p", other)),
    }
}

/// GNU `number_or_marker' path (count-lines & friends): ints and
/// floats pass, live markers yield their position (detached markers
/// signal "Marker does not point anywhere"), others signal
/// `wrong-type-argument number-or-marker-p'.
pub(crate) fn want_num_or_mark(i: &mut Interp, v: &Value) -> Result<i128, Flow> {
    match v {
        Value::Int(n) => Ok(*n),
        Value::BigInt(_) => want_int(i, v),
        Value::Marker(m) if m.borrow().buffer.is_some() => Ok(m.borrow().position as i128 + 1),
        Value::Marker(_) => Err(i.error("Marker does not point anywhere")),
        Value::Float(f) => Ok(**f as i128),
        other => Err(i.wrong_type_mut("number-or-marker-p", other)),
    }
}

pub(crate) fn want_num(i: &mut Interp, v: &Value) -> Result<f64, Flow> {
    match v {
        Value::Int(n) => Ok(*n as f64),
        Value::BigInt(n) => Ok(n
            .to_f64()
            .unwrap_or(if n.sign() == num_bigint::Sign::Minus {
                f64::NEG_INFINITY
            } else {
                f64::INFINITY
            })),
        Value::Float(f) => Ok(**f),
        // GNU CHECK_NUMBER_OR_MARKER_COERCE: dead markers error.
        Value::Marker(m) if m.borrow().buffer.is_some() => Ok(m.borrow().position as f64 + 1.0),
        Value::Marker(_) => Err(i.error("Marker does not point anywhere")),
        _ => Err(i.wrong_type_mut("number-or-marker-p", v)),
    }
}

pub(crate) fn want_string(i: &mut Interp, v: &Value) -> Result<String, Flow> {
    match v {
        Value::Str(s) => Ok(s.borrow().clone()),
        _ => Err(i.wrong_type_mut("stringp", v)),
    }
}

pub(crate) fn want_sym(i: &mut Interp, v: &Value) -> Result<SymId, Flow> {
    match i.sym_id(v) {
        Some(s) => Ok(s),
        None => Err(i.wrong_type_mut("symbolp", v)),
    }
}

pub(crate) fn want_cons(i: &mut Interp, v: &Value) -> Result<super::value::ConsRef, Flow> {
    match v {
        Value::Cons(c) => Ok(c.clone()),
        _ => Err(i.wrong_type_mut("consp", v)),
    }
}

pub(crate) fn want_list(i: &mut Interp, v: &Value) -> Result<Vec<Value>, Flow> {
    match v.list_to_vec() {
        Ok(items) => Ok(items),
        Err(super::value::ListError::Circular(tail)) => {
            Err(i.signal_data(sym::CIRCULAR_LIST, vec![tail]))
        }
        Err(super::value::ListError::Dotted(tail)) => Err(i.wrong_type_mut("listp", &tail)),
    }
}

/// `eq` — identity.  GNU's `symbols-with-pos-enabled' objects are
/// `eq' to their bare symbol (and to each other across positions):
/// unwrap them here so list/search predicates agree.
pub fn eq_values(a: &Value, b: &Value) -> bool {
    if misc::as_sympos(a).is_some() || misc::as_sympos(b).is_some() {
        return match (misc::as_sympos(a), misc::as_sympos(b)) {
            (Some((x, _)), Some((y, _))) => x == y,
            (Some((x, _)), None) => matches!(b, Value::Sym(y) if *y == x),
            (None, Some((y, _))) => matches!(a, Value::Sym(x) if *x == y),
            (None, None) => unreachable!(),
        };
    }
    match (a, b) {
        (Value::Nil, Value::Nil) => true,
        (Value::Nil, Value::Sym(0)) | (Value::Sym(0), Value::Nil) => true,
        // Fixnums are immediate values. Canonical bignums use the Rc
        // identity arm below; wider legacy/native Int projections have
        // no heap identity and must not compare as immediate fixnums.
        (Value::Int(x), Value::Int(y)) => x == y && (FIXNUM_MIN..=FIXNUM_MAX).contains(x),
        // Emacs floats are heap objects: `eq' compares object identity.
        (Value::Float(x), Value::Float(y)) => std::rc::Rc::ptr_eq(x, y),
        (Value::BigInt(x), Value::BigInt(y)) => std::rc::Rc::ptr_eq(x, y),
        (Value::Sym(x), Value::Sym(y)) => x == y,
        (Value::Cons(x), Value::Cons(y)) => std::rc::Rc::ptr_eq(x, y),
        (Value::Str(x), Value::Str(y)) => std::rc::Rc::ptr_eq(x, y),
        (Value::Vec(x), Value::Vec(y)) => std::rc::Rc::ptr_eq(x, y),
        (Value::Record(x), Value::Record(y)) => std::rc::Rc::ptr_eq(x, y),
        (Value::Hash(x), Value::Hash(y)) => std::rc::Rc::ptr_eq(x, y),
        (Value::Subr(x), Value::Subr(y)) => std::ptr::eq(*x, *y),
        (Value::Lambda(x), Value::Lambda(y)) => std::rc::Rc::ptr_eq(x, y),
        (Value::Buffer(x), Value::Buffer(y)) => std::rc::Rc::ptr_eq(x, y),
        (Value::Marker(x), Value::Marker(y)) => std::rc::Rc::ptr_eq(x, y),
        (Value::Window(x), Value::Window(y)) => std::rc::Rc::ptr_eq(x, y),
        (Value::Frame(x), Value::Frame(y)) => std::rc::Rc::ptr_eq(x, y),
        (Value::Process(x), Value::Process(y)) => std::rc::Rc::ptr_eq(x, y),
        (Value::Thread(x), Value::Thread(y)) => std::rc::Rc::ptr_eq(x, y),
        (Value::Mutex(x), Value::Mutex(y)) => std::rc::Rc::ptr_eq(x, y),
        (Value::CondVar(x), Value::CondVar(y)) => std::rc::Rc::ptr_eq(x, y),
        (Value::Finalizer(x), Value::Finalizer(y)) => std::rc::Rc::ptr_eq(x, y),
        _ => false,
    }
}

/// `eql` — eq, or equal numbers of the same type (int vs float differ).
/// GNU compares floats bitwise: 0.0 and -0.0 differ, while NaN eqls
/// NaN when the payloads match.
pub fn eql_values(a: &Value, b: &Value) -> bool {
    match (a, b) {
        // eql compares numbers by value — including bignums, which are
        // not `eq' unless literally the same object.
        (Value::Int(x), Value::Int(y)) => x == y,
        (Value::BigInt(x), Value::BigInt(y)) => **x == **y,
        (Value::BigInt(x), Value::Int(y)) | (Value::Int(y), Value::BigInt(x)) => {
            x.to_i128() == Some(*y)
        }
        (Value::Float(x), Value::Float(y)) => (**x).to_bits() == (**y).to_bits(),
        _ => eq_values(a, b),
    }
}

/// `equal` — structural equality. List tails use constant-space cycle
/// detection, like GNU `internal_equal'. Recursive object pairs are
/// memoized after ten levels, and recursion past 200 levels signals an
/// error. Callers must propagate that error rather than treating it as
/// a failed comparison.
pub fn equal_values(interp: &Interp, a: &Value, b: &Value) -> Result<bool, Flow> {
    let mut seen = None;
    equal_values_1(interp, a, b, 0, &mut seen, false)
}

/// The same recursive comparison with string text properties enabled.
/// Property values themselves still use ordinary equal, as in GNU's
/// compare_string_intervals -> intervals_equal_1.
pub(crate) fn equal_values_including_properties(
    interp: &Interp,
    a: &Value,
    b: &Value,
) -> Result<bool, Flow> {
    equal_values_1(interp, a, b, 0, &mut None, true)
}

fn equal_string_properties(
    interp: &Interp,
    a: &crate::lisp::value::StrRef,
    b: &crate::lisp::value::StrRef,
) -> Result<bool, Flow> {
    let (ia, ib) = (interp.str_props(a).to_vec(), interp.str_props(b).to_vec());
    for position in 0..a.borrow().chars().count() {
        let pa = crate::buffer::primitives::str_plist_at(&ia, position);
        let pb = crate::buffer::primitives::str_plist_at(&ib, position);
        for (a, b_at_pos) in pa.chunks(2).zip(pb.chunks(2)) {
            if a.len() != 2 {
                return Ok(false);
            }
            let Some(b) = pb.chunks(2).find(|b| eq_values(&b[0], &a[0])) else {
                return Ok(false);
            };
            if b.len() != 2 || !equal_values(interp, &b[1], &a[1])? {
                return Ok(false);
            }
            if b_at_pos.len() != 2 {
                return Ok(false);
            }
        }
        if pa.len() != pb.len() {
            return Ok(false);
        }
    }
    Ok(true)
}

type EqualSeen = std::collections::HashSet<(usize, usize)>;

/// Heap identity for the pair memo, mirroring GNU's object-pointer
/// keys; atoms return None and take the plain branch.
fn equal_obj_ptr(v: &Value) -> Option<usize> {
    match v {
        Value::Cons(x) => Some(std::rc::Rc::as_ptr(x) as usize),
        Value::Vec(x) => Some(std::rc::Rc::as_ptr(x) as usize),
        Value::Record(x) => Some(std::rc::Rc::as_ptr(x) as usize),
        Value::Lambda(x) => Some(std::rc::Rc::as_ptr(x) as usize),
        _ => None,
    }
}

const EQUAL_MEMO_DEPTH: u32 = 10;
const EQUAL_MAX_DEPTH: u32 = 200;

fn equal_values_1(
    interp: &Interp,
    a: &Value,
    b: &Value,
    depth: u32,
    seen: &mut Option<EqualSeen>,
    with_properties: bool,
) -> Result<bool, Flow> {
    // GNU checks the limit before identity, type, or memo shortcuts.
    // An identical leaf at depth 201 therefore still signals an error.
    if depth > EQUAL_MAX_DEPTH {
        return Err(interp.error("Stack overflow in equal"));
    }
    if depth > EQUAL_MEMO_DEPTH {
        if let (Some(pa), Some(pb)) = (equal_obj_ptr(a), equal_obj_ptr(b)) {
            let set = seen.get_or_insert_with(EqualSeen::new);
            if !set.insert((pa, pb)) {
                // This pair is already being compared — coinduction.
                return Ok(true);
            }
        }
    }
    // Enabled sympos are atoms (symbols): compare by `eq' unwrapping,
    // not record slot contents (positions must not matter).
    if misc::as_sympos(a).is_some() || misc::as_sympos(b).is_some() {
        return Ok(eql_values(a, b));
    }
    if eq_values(a, b) {
        return Ok(true);
    }
    let left_config = crate::editor::window_configuration_object(a);
    let right_config = crate::editor::window_configuration_object(b);
    if left_config || right_config {
        // GNU configurations are opaque for equal. Identity was handled
        // above; layout equivalence is a separate explicit operation.
        return Ok(false);
    }
    if interp.is_overlay(a) || interp.is_overlay(b) {
        return match (interp.overlay_state(a), interp.overlay_state(b)) {
            (Some(a), Some(b))
                if a.buffer == b.buffer
                    && (a.buffer.is_none() || (a.start == b.start && a.end == b.end)) =>
            {
                equal_values_1(interp, &a.plist, &b.plist, depth + 1, seen, with_properties)
            }
            _ => Ok(false),
        };
    }
    Ok(match (a, b) {
        (Value::Str(x), Value::Str(y)) => {
            strfn::gnu_sbytes(interp, x) == strfn::gnu_sbytes(interp, y)
                && (!with_properties || equal_string_properties(interp, x, y)?)
        }
        (Value::Marker(x), Value::Marker(y)) => {
            // GNU `equal' on markers compares buffer + position (they're
            // `misc' objects with value semantics), not identity.
            let (xb, yb) = (x.borrow(), y.borrow());
            xb.buffer == yb.buffer && (xb.buffer.is_none() || xb.position == yb.position)
        }
        (Value::Cons(_x), Value::Cons(_y)) => {
            // Cdrs do not increase recursion depth. Detect their cycles
            // separately with Brent's walk, without a per-cons allocation.
            // After finding one cycle, swap tails and detect the other,
            // comparing cars throughout (GNU `internal_equal_cycle').
            let mut ax = a.clone();
            let mut bx = b.clone();
            let mut tortoise = ax.clone();
            let mut power = 2usize;
            let mut steps = 0usize;
            let mut one_cycle = false;
            loop {
                match (&ax, &bx) {
                    (Value::Cons(ac), Value::Cons(bc)) => {
                        let (acar, acdr) = {
                            let ab = ac.borrow();
                            (ab.car.clone(), ab.cdr.clone())
                        };
                        let (bcar, bcdr) = {
                            let bb = bc.borrow();
                            (bb.car.clone(), bb.cdr.clone())
                        };
                        if !equal_values_1(interp, &acar, &bcar, depth + 1, seen, with_properties)?
                        {
                            return Ok(false);
                        }
                        ax = acdr;
                        bx = bcdr;
                        if eq_values(&ax, &bx) {
                            return Ok(true);
                        }
                        steps += 1;
                        // Match GNU's checkpoint cadence: the checkpoint
                        // step resets the tortoise without testing equality.
                        if steps == power {
                            tortoise = ax.clone();
                            power = power.saturating_mul(2);
                            steps = 0;
                        } else if eq_values(&ax, &tortoise) {
                            if one_cycle {
                                return Ok(true);
                            }
                            if !matches!(bx, Value::Cons(_)) {
                                return Ok(false);
                            }
                            std::mem::swap(&mut ax, &mut bx);
                            tortoise = ax.clone();
                            power = 2;
                            steps = 0;
                            one_cycle = true;
                        }
                    }
                    // A missing right-hand cons is checked in the loop,
                    // without another recursion level. The second cycle
                    // pass likewise rejects a finite left-hand tail here.
                    (Value::Cons(_), _) => return Ok(false),
                    _ if one_cycle => return Ok(false),
                    _ => {
                        return equal_values_1(interp, &ax, &bx, depth + 1, seen, with_properties);
                    }
                }
            }
        }
        (Value::Record(_), Value::Record(_))
            if crate::lisp::builtins::data::is_obarray(interp, a)
                || crate::lisp::builtins::data::is_obarray(interp, b) =>
        {
            // GNU obarrays are opaque pseudovectors: after the identity
            // shortcut above, distinct obarrays are never structurally equal.
            false
        }
        (Value::Vec(x), Value::Vec(y)) | (Value::Record(x), Value::Record(y)) => equal_items(
            interp,
            &x.borrow(),
            &y.borrow(),
            depth,
            seen,
            with_properties,
        )?,
        (Value::Lambda(x), Value::Lambda(y)) => {
            // `#[...]' byte-code objects are pseudovectors in GNU:
            // `equal' compares their elements (argdesc, bytecode
            // string, constants vector, ...) pairwise.
            match (&x.bc_items, &y.bc_items) {
                (Some(xi), Some(yi)) => {
                    return Ok(equal_items(
                        interp,
                        &xi.borrow(),
                        &yi.borrow(),
                        depth,
                        seen,
                        with_properties,
                    )? && x.env.is_none() == y.env.is_none());
                }
                (None, None) => {}
                _ => return Ok(false),
            }
            // GNU compares interpreted lambdas as list structure.
            if x.is_macro != y.is_macro
                || x.required != y.required
                || x.rest != y.rest
                || x.optional.len() != y.optional.len()
            {
                return Ok(false);
            }
            for (a, b) in x.optional.iter().zip(y.optional.iter()) {
                if a.sym != b.sym || a.supplied != b.supplied {
                    return Ok(false);
                }
                match (&a.default, &b.default) {
                    (Some(d), Some(e)) => {
                        if !equal_values_1(interp, d, e, depth + 1, seen, with_properties)? {
                            return Ok(false);
                        }
                    }
                    (None, None) => {}
                    _ => return Ok(false),
                }
            }
            equal_items(interp, &x.body, &y.body, depth, seen, with_properties)?
                && x.env.is_none() == y.env.is_none()
        }
        _ => eql_values(a, b),
    })
}

fn equal_items(
    interp: &Interp,
    a: &[Value],
    b: &[Value],
    depth: u32,
    seen: &mut Option<EqualSeen>,
    with_properties: bool,
) -> Result<bool, Flow> {
    if a.len() != b.len() {
        return Ok(false);
    }
    for (a, b) in a.iter().zip(b.iter()) {
        if !equal_values_1(interp, a, b, depth + 1, seen, with_properties)? {
            return Ok(false);
        }
    }
    Ok(true)
}
