//! Primitive functions (subrs) implemented in Rust.
//!
//! Like Emacs's `DEFUN`s in C. Each subr is a static `Subr` with a name,
//! arity, and a `fn(&mut Interp, Vec<Value>) -> EvalResult`.

pub(crate) mod arith;
pub(crate) mod charset;
pub(crate) mod data;
pub(crate) mod evalfn;
pub(crate) mod hashfn;
pub(crate) mod json;
pub(crate) mod listfn;
pub mod misc;
pub(crate) mod printfn;
pub(crate) mod readfn;
pub(crate) mod seq;
pub(crate) mod strfn;

use super::Interp;
use super::error::Flow;
use super::obarray::sym;
use super::value::{Arity, Subr, SymId, Value};

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
fn collect() -> Vec<&'static Subr> {
    let mut v: Vec<&'static Subr> = Vec::new();
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
    v.extend(crate::lisp::process::SUBRS);
    v
}

/// Intern every subr name and bind its function cell.
pub fn install(interp: &mut Interp) {
    for s in collect() {
        let id = interp.intern(s.name);
        interp.fset(id, Value::Subr(s));
    }
    // Special forms are subrs in Emacs: `symbol-function'/'subrp'/'fboundp'
    // must see them. Calling one through funcall signals invalid-function.
    for s in SPECIAL_FORM_SUBRS {
        let id = interp.intern(s.name);
        interp.fset(id, Value::Subr(s));
    }
    install_aliases(interp);
}

fn sf_cannot_call(i: &mut Interp, a: Vec<Value>) -> super::error::EvalResult {
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
        name: "prog2",
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
    Subr {
        name: "defun",
        arity: Arity::Unevalled,
        func: sf_cannot_call,
        doc: "",
    },
    Subr {
        name: "defmacro",
        arity: Arity::Unevalled,
        func: sf_cannot_call,
        doc: "",
    },
    Subr {
        name: "lambda",
        arity: Arity::Unevalled,
        func: sf_cannot_call,
        doc: "",
    },
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
        name: "with-current-buffer",
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
    Subr {
        name: "track-mouse",
        arity: Arity::Unevalled,
        func: sf_cannot_call,
        doc: "",
    },
];

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
        _ => Err(i.wrong_type_mut("integerp", v)),
    }
}

pub(crate) fn want_num(i: &mut Interp, v: &Value) -> Result<f64, Flow> {
    match v {
        Value::Int(n) => Ok(*n as f64),
        Value::Float(f) => Ok(*f),
        Value::Marker(m) => Ok(m.borrow().position as f64 + 1.0),
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
        Err(super::value::ListError::Circular) => {
            Err(i.signal_data(sym::CIRCULAR_LIST, vec![v.clone()]))
        }
        Err(super::value::ListError::Dotted(tail)) => Err(i.wrong_type_mut("listp", &tail)),
    }
}

/// `eq` — identity.
pub fn eq_values(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Nil, Value::Nil) => true,
        (Value::Nil, Value::Sym(0)) | (Value::Sym(0), Value::Nil) => true,
        (Value::Int(x), Value::Int(y)) => x == y,
        (Value::Float(_), Value::Float(_)) => {
            // eq on floats is object identity; two distinct Float values are
            // never the same object (eql compares by value).
            false
        }
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
pub fn eql_values(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Float(x), Value::Float(y)) => x == y,
        _ => eq_values(a, b),
    }
}

/// `equal` — structural equality.
pub fn equal_values(interp: &Interp, a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Str(x), Value::Str(y)) => *x.borrow() == *y.borrow(),
        (Value::Cons(_x), Value::Cons(_y)) => {
            // Structural walk with cycle guard.
            let mut ax = a.clone();
            let mut bx = b.clone();
            let mut guard = 0usize;
            loop {
                guard += 1;
                if guard > 100_000 {
                    return false;
                }
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
                        if !equal_values(interp, &acar, &bcar) {
                            return false;
                        }
                        ax = acdr;
                        bx = bcdr;
                    }
                    _ => {
                        return equal_values(interp, &ax, &bx);
                    }
                }
            }
        }
        (Value::Vec(x), Value::Vec(y)) => {
            let xv = x.borrow();
            let yv = y.borrow();
            if xv.len() != yv.len() {
                return false;
            }
            xv.iter()
                .zip(yv.iter())
                .all(|(a, b)| equal_values(interp, a, b))
        }
        (Value::Record(x), Value::Record(y)) => {
            let xv = x.borrow();
            let yv = y.borrow();
            if xv.len() != yv.len() {
                return false;
            }
            xv.iter()
                .zip(yv.iter())
                .all(|(a, b)| equal_values(interp, a, b))
        }
        _ => eql_values(a, b),
    }
}
