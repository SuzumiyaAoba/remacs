//! Core Lisp object representation.
//!
//! Unlike Emacs (tagged pointers), we use a plain enum of reference-counted
//! objects. Mutability goes through `RefCell`; sharing through `Rc`. Cycles
//! can leak — acceptable for now, and the architecture note in AGENTS.md
//! documents this as a deliberate simplification over Emacs's mark-and-sweep.

use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt;
use std::rc::Rc;

use crate::buffer::Buffer;
use crate::editor::{Frame, Window};

/// Symbol identifier: index into the obarray's symbol vector.
pub type SymId = u32;

pub type ConsRef = Rc<RefCell<Cons>>;
pub type StrRef = Rc<RefCell<String>>;
pub type VecRef = Rc<RefCell<Vec<Value>>>;
pub type HashRef = Rc<RefCell<LispHash>>;
pub type LambdaRef = Rc<Lambda>;
pub type BufferRef = Rc<RefCell<Buffer>>;
pub type MarkerRef = Rc<RefCell<Marker>>;
pub type WindowRef = Rc<RefCell<Window>>;
pub type FrameRef = Rc<RefCell<Frame>>;
pub type ProcessRef = Rc<RefCell<crate::lisp::process::Proc>>;
pub type ThreadRef = Rc<RefCell<Thread>>;
pub type MutexRef = Rc<RefCell<Mutex>>;
pub type CondVarRef = Rc<RefCell<CondVar>>;
pub type FinalizerRef = Rc<RefCell<Value>>;

/// A Lisp thread object. Threads run synchronously at `make-thread'
/// time (cooperative model: no preemption), so a created thread is
/// dead but joinable by the time `make-thread' returns.
#[derive(Debug)]
pub struct Thread {
    pub name: Option<String>,
    /// GNU semantics: a finished thread stays "live" until reaped by
    /// `thread-join' (it is a zombie holding its result).
    pub alive: bool,
    /// Result of the thread's function, once finished.
    pub result: Option<Value>,
    /// Error the thread function died with, if any.
    pub last_error: Option<Value>,
    /// True once the function has run to completion.
    pub finished: bool,
}

/// A mutex object (`make-mutex'). With the cooperative thread model a
/// mutex can only ever be held by the running thread.
#[derive(Debug)]
pub struct Mutex {
    pub name: Option<String>,
    /// `Some(thread)` while locked; cooperative model keeps this simple.
    pub owner: Option<Value>,
}

/// A condition variable (`make-condition-variable').
#[derive(Debug)]
pub struct CondVar {
    pub name: Option<String>,
    pub mutex: Value,
}

/// Emacs fixnum range on 64-bit builds: 62 bits (2 tag bits in C).
/// Integers outside this range are bignums — we represent all integers
/// as i128 so promotion is exact for any value a real program produces.
pub const FIXNUM_MAX: i128 = (1i128 << 61) - 1;
pub const FIXNUM_MIN: i128 = -(1i128 << 61);

#[derive(Clone)]
pub enum Value {
    Nil,
    Int(i128),
    /// Emacs floats are heap objects: two reads of `1.0` are distinct
    /// objects, so `eq` compares identity (Rc pointer), not value.
    Float(Rc<f64>),
    Sym(SymId),
    Cons(ConsRef),
    Str(StrRef),
    Vec(VecRef),
    /// Emacs record object (read syntax `#s(tag fields...)`).
    Record(VecRef),
    Hash(HashRef),
    Subr(&'static Subr),
    Lambda(LambdaRef),
    Buffer(BufferRef),
    Marker(MarkerRef),
    Window(WindowRef),
    Frame(FrameRef),
    Process(ProcessRef),
    Thread(ThreadRef),
    Mutex(MutexRef),
    CondVar(CondVarRef),
    /// `make-finalizer' object; holds the finalizer function.
    Finalizer(FinalizerRef),
}

/// A cons cell. `cdr` may be any value (dotted pair).
pub struct Cons {
    pub car: Value,
    pub cdr: Value,
}

impl Cons {
    pub fn new(car: Value, cdr: Value) -> ConsRef {
        Rc::new(RefCell::new(Cons { car, cdr }))
    }
}

/// Hash table test function (make-hash-table :test ...).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum HashTest {
    Eq,
    Eql,
    Equal,
}

pub struct LispHash {
    pub test: HashTest,
    /// We key on a normalized form so `equal` keys hash correctly.
    pub map: HashMap<HashKey, Value>,
    /// Original keys for `maphash`/`hash-table-keys`, kept in GNU's
    /// slot order: entries append at the end, `remhash' leaves a
    /// tombstone and the next `puthash' reuses the first free slot.
    pub keys: KeySlots,
    /// :weakness argument recorded for `hash-table-weakness` (#<..>
    /// printed representation). Weak references aren't implemented;
    /// the tag is metadata only.
    pub weakness: Option<Value>,
    /// The :size argument as given (GNU's `hash-table-size').
    pub size: i128,
    /// :rehash-size / :rehash-threshold values (GNU stores them and
    /// returns them verbatim from the accessors).
    pub rehash_size: Value,
    pub rehash_threshold: Value,
}

impl LispHash {
    pub fn new(test: HashTest) -> Self {
        LispHash {
            test,
            map: HashMap::new(),
            keys: KeySlots(Vec::new()),
            weakness: None,
            size: 0,
            rehash_size: Value::float(1.5),
            rehash_threshold: Value::float(0.8125),
        }
    }

    /// Record an original key: an already-present normalized key
    /// keeps its original object (as GNU's hash_put); otherwise the
    /// key takes the first tombstone slot, or appends.
    pub fn put_key(&mut self, hk: HashKey, k: Value) {
        if self.keys.0.iter().flatten().any(|(e, _)| *e == hk) {
            return;
        }
        match self.keys.0.iter_mut().find(|e| e.is_none()) {
            Some(slot) => *slot = Some((hk, k)),
            None => self.keys.0.push(Some((hk, k))),
        }
    }

    pub fn remove_key(&mut self, hk: &HashKey) {
        if let Some(e) = self
            .keys
            .0
            .iter_mut()
            .find(|e| matches!(e, Some((h, _)) if h == hk))
        {
            *e = None;
        }
    }
}

/// Insertion-ordered hash keys with tombstone slots, mirroring GNU's
/// hash-table slot reuse. `iter' yields only live entries.
#[derive(Clone)]
pub struct KeySlots(pub Vec<Option<(HashKey, Value)>>);

impl KeySlots {
    pub fn iter(&self) -> impl Iterator<Item = &(HashKey, Value)> {
        self.0.iter().filter_map(|e| e.as_ref())
    }
    pub fn is_empty(&self) -> bool {
        self.0.iter().all(|e| e.is_none())
    }
    pub fn len(&self) -> usize {
        self.0.iter().flatten().count()
    }
    pub fn clear(&mut self) {
        self.0.clear();
    }
}

/// Normalized hash key. Deep-normalizes conses/strings so that `equal`
/// comparisons work; `eq`/`eql` use identity-ish keys.
#[derive(PartialEq, Eq, Hash, Clone, Debug)]
pub enum HashKey {
    Nil,
    True,
    Sym(SymId),
    Int(i128),
    /// Bit pattern so -0.0/NaN behave deterministically.
    Float(u64),
    Str(String),
    /// Identity key for objects where `eq` compares by pointer.
    Ptr(usize),
    Cons(Box<HashKey>, Box<HashKey>),
    Vec(Vec<HashKey>),
}

/// Max argument count for a subr: fixed or unlimited.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Arity {
    /// Exactly `min..=max` evaluated args.
    Range { min: u16, max: u16 },
    /// At least `min` evaluated args (MANY).
    Many { min: u16 },
    /// Args passed raw (unevaluated) as a list — a special form.
    Unevalled,
}

/// A primitive function implemented in Rust.
pub struct Subr {
    pub name: &'static str,
    pub arity: Arity,
    pub func: SubrFn,
    pub doc: &'static str,
}

pub type SubrFn = fn(&mut crate::lisp::Interp, Vec<Value>) -> crate::lisp::EvalResult;

/// An interpreted function (or macro) defined in Lisp.
pub struct Lambda {
    /// `Some` for macros: body returns a form to re-evaluate.
    pub is_macro: bool,
    pub required: Vec<SymId>,
    pub optional: Vec<OptParam>,
    pub rest: Option<SymId>,
    pub body: Vec<Value>,
    /// Captured lexical environment (used when `lexical-binding` is t).
    pub env: crate::lisp::LexEnv,
    pub doc: Option<String>,
    /// Raw `interactive` spec form, if this is a command.
    pub interactive: Option<Value>,
    /// Name for display purposes (from defun or set-name).
    pub name: Option<String>,
    /// Arglist contains extended `(var init)' params — calling it in a
    /// dynamic (non-macro) context signals `invalid-function' like Emacs.
    pub bad_arglist: bool,
    /// The raw lambda-list form, kept so the `#[args body env]' printer
    /// shows exactly what the user wrote (like Emacs).
    pub arglist: Option<Value>,
    /// True when built by wrapping a raw `(lambda ...)' list for
    /// `funcall'/direct call rather than evaluating a lambda form — the
    /// printer shows `nil' as the environment for these.
    pub plain: bool,
    /// True when defined while a dumped/embedded library was loading
    /// (prelude or a `builtin:' load).  GNU keeps such docstrings
    /// externally (DOC file/.elc), and `documentation' appends the
    /// `(fn ARGLIST)' usage trailer for them only.
    pub dumped_doc: bool,
    /// `Some(idx)' when this lambda is an advice trampoline: applying it
    /// runs `Interp::advice_links[idx]'s (WHERE FUN . NEXT) layer, like
    /// GNU's `advice' oclosure layers.
    pub advice_link: Option<usize>,
}

#[derive(Clone)]
pub struct OptParam {
    pub sym: SymId,
    pub default: Option<Value>,
    /// `(var init supplied-p)' triple — bound to t when the arg was given.
    pub supplied: Option<SymId>,
}

impl Lambda {
    pub fn arity(&self) -> Arity {
        let min = self.required.len() as u16;
        if self.rest.is_some() {
            Arity::Many { min }
        } else {
            Arity::Range {
                min,
                max: min + self.optional.len() as u16,
            }
        }
    }
}

/// A marker: a position in a buffer that tracks edits.
pub struct Marker {
    /// Buffer name or identity of the owner; `None` if marker points nowhere.
    pub buffer: Option<usize>,
    /// Character position (0-based byte-in-chars offset).
    pub position: usize,
    /// Insertion type: if true, text inserted at the marker goes after it.
    pub insertion_type: bool,
}

/// GNU's eight-bit chars (0x3FFF80..=0x3FFFFF) exceed Rust's char
/// range, so inside strings they are proxied into plane-15 PUA:
/// byte b (0x80..=0xFF) maps to U+F0000+b.  As `Value::Int` they keep
/// their real GNU codes.
pub const EIGHT_BIT_BASE: u32 = 0xF_0000;

/// Map a GNU char code to a storable Rust char; eight-bit codes
/// become PUA proxies.
pub fn lisp_char(code: u32) -> Option<char> {
    match code {
        0x3FFF80..=0x3FFFFF => char::from_u32(EIGHT_BIT_BASE + (code & 0xFF)),
        _ => char::from_u32(code),
    }
}

/// If `c` is an eight-bit proxy char, return its byte value.
pub fn eight_bit_byte(c: char) -> Option<u8> {
    let u = c as u32;
    (0xF0080..=0xF00FF)
        .contains(&u)
        .then(|| (u - EIGHT_BIT_BASE) as u8)
}

/// The GNU char code a string char stands for (proxies return their
/// 0x3FFFxx code).
pub fn lisp_char_code(c: char) -> i128 {
    match eight_bit_byte(c) {
        Some(b) => 0x3FFF00 + b as i128,
        None => c as i128,
    }
}

impl Value {
    pub fn is_nil(&self) -> bool {
        matches!(self, Value::Nil)
    }

    pub fn truthy(&self) -> bool {
        !self.is_nil()
    }

    pub fn t() -> Value {
        Value::Sym(crate::lisp::sym::T)
    }

    /// Canonical `t`/`nil` from a boolean.
    pub fn from_bool(b: bool) -> Value {
        if b { Value::t() } else { Value::Nil }
    }

    /// Integer value if this is an Int.
    pub fn int(&self) -> Option<i128> {
        match self {
            Value::Int(n) => Some(*n),
            _ => None,
        }
    }

    /// `fixnump`: integer in Emacs's fixnum range.
    pub fn fixnump(&self) -> bool {
        matches!(self, Value::Int(n) if (FIXNUM_MIN..=FIXNUM_MAX).contains(n))
    }

    pub fn cons(car: Value, cdr: Value) -> Value {
        Value::Cons(Cons::new(car, cdr))
    }

    pub fn string(s: impl Into<String>) -> Value {
        Value::Str(Rc::new(RefCell::new(s.into())))
    }

    /// A fresh float object (each call yields a distinct `eq' identity).
    pub fn float(x: f64) -> Value {
        Value::Float(Rc::new(x))
    }

    pub fn list(items: Vec<Value>) -> Value {
        let mut tail = Value::Nil;
        for item in items.into_iter().rev() {
            tail = Value::cons(item, tail);
        }
        tail
    }

    /// Iterate over a proper or dotted list's cars. Returns the tail after
    /// the last cons (nil for proper lists, the dotted value otherwise).
    /// The callback gets a cloned car so it may mutate conses freely.
    /// NOTE: no cycle detection — callers that need it use `list_to_vec`.
    pub fn each_car(&self, mut f: impl FnMut(&Value)) -> Value {
        let mut cur = self.clone();
        loop {
            match cur {
                Value::Cons(c) => {
                    let (car, next) = {
                        let b = c.borrow();
                        (b.car.clone(), b.cdr.clone())
                    };
                    f(&car);
                    cur = next;
                }
                other => return other,
            }
        }
    }

    /// Convert a proper list to a Vec. Errors on improper lists or circles.
    pub fn list_to_vec(&self) -> Result<Vec<Value>, ListError> {
        let mut out = Vec::new();
        let mut cur = self.clone();
        let mut slow = self.clone();
        let mut steps = 0usize;
        loop {
            match cur {
                Value::Nil => return Ok(out),
                Value::Cons(c) => {
                    let b = c.borrow();
                    let next = b.cdr.clone();
                    out.push(b.car.clone());
                    drop(b);
                    cur = next;
                }
                other => return Err(ListError::Dotted(other)),
            }
            steps += 1;
            if steps % 2 == 0 {
                if let Value::Cons(c) = slow {
                    let next = c.borrow().cdr.clone();
                    slow = next;
                }
                if let (Value::Cons(a), Value::Cons(b)) = (&slow, &cur) {
                    if Rc::ptr_eq(a, b) {
                        return Err(ListError::Circular);
                    }
                }
            }
        }
    }
}

/// Errors traversing a list.
#[derive(Debug)]
pub enum ListError {
    /// List ended in a non-nil non-cons (dotted pair); carries the tail.
    Dotted(Value),
    /// Circular list detected.
    Circular,
}

impl fmt::Debug for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Nil => write!(f, "nil"),
            Value::Int(i) => write!(f, "{i}"),
            Value::Float(x) => write!(f, "{}", **x),
            Value::Sym(id) => write!(f, "Sym({id})"),
            Value::Cons(_) => write!(f, "Cons(..)"),
            Value::Str(s) => write!(f, "{:?}", s.borrow()),
            Value::Vec(_) => write!(f, "Vec(..)"),
            Value::Record(_) => write!(f, "Record(..)"),
            Value::Hash(_) => write!(f, "Hash(..)"),
            Value::Subr(s) => write!(f, "#<subr {}>", s.name),
            Value::Lambda(_) => write!(f, "Lambda(..)"),
            Value::Buffer(_) => write!(f, "Buffer(..)"),
            Value::Marker(_) => write!(f, "Marker(..)"),
            Value::Window(_) => write!(f, "Window(..)"),
            Value::Frame(_) => write!(f, "Frame(..)"),
            Value::Process(_) => write!(f, "Process(..)"),
            Value::Thread(_) => write!(f, "Thread(..)"),
            Value::Mutex(_) => write!(f, "Mutex(..)"),
            Value::CondVar(_) => write!(f, "CondVar(..)"),
            Value::Finalizer(_) => write!(f, "Finalizer(..)"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::rc::Rc;

    #[test]
    fn debug_fmt_covers_every_variant() {
        static SUBR: Subr = Subr {
            name: "x",
            arity: Arity::Many { min: 0 },
            func: |_, _| Ok(Value::Nil),
            doc: "",
        };
        let cases: Vec<(Value, &str)> = vec![
            (Value::Nil, "nil"),
            (Value::Int(3), "3"),
            (Value::float(1.5), "1.5"),
            (Value::Sym(7), "Sym(7)"),
            (Value::cons(Value::Nil, Value::Nil), "Cons(..)"),
            (Value::string("hi"), "\"hi\""),
            (Value::Vec(Rc::new(RefCell::new(vec![]))), "Vec(..)"),
            (Value::Record(Rc::new(RefCell::new(vec![]))), "Record(..)"),
            (
                Value::Hash(Rc::new(RefCell::new(LispHash::new(HashTest::Eq)))),
                "Hash(..)",
            ),
            (Value::Subr(&SUBR), "#<subr x>"),
        ];
        for (v, want) in cases {
            assert_eq!(format!("{v:?}"), want);
        }
        // Rc-backed variants: build via an Interp where needed.
        let mut i = crate::lisp::Interp::new();
        let lam = i.eval_str("(lambda (x) x)").unwrap();
        assert_eq!(format!("{lam:?}"), "Lambda(..)");
        let buf = i.eval_str("(current-buffer)").unwrap();
        assert_eq!(format!("{buf:?}"), "Buffer(..)");
        let m = i.eval_str("(point-marker)").unwrap();
        assert_eq!(format!("{m:?}"), "Marker(..)");
        let w = i.eval_str("(selected-window)").unwrap();
        assert_eq!(format!("{w:?}"), "Window(..)");
        let f = i.eval_str("(selected-frame)").unwrap();
        assert_eq!(format!("{f:?}"), "Frame(..)");
        let p = i.eval_str("(make-process :name \"pdbg\")").unwrap();
        assert_eq!(format!("{p:?}"), "Process(..)");
    }
}
