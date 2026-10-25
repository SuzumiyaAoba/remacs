//! Core Lisp object representation.
//!
//! Unlike Emacs (tagged pointers), we use a plain enum of reference-counted
//! objects. Mutability goes through `RefCell`; sharing through `Rc`. Cycles
//! can leak — acceptable for now, and the architecture note in AGENTS.md
//! documents this as a deliberate simplification over Emacs's mark-and-sweep.

use num_bigint::BigInt;
use num_traits::ToPrimitive;
use rustc_hash::FxHashMap as HashMap;
use std::cell::RefCell;
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

/// A Lisp thread object. Its coroutine runs on the interpreter's OS thread
/// and releases execution at Lisp yield and blocking operations.
#[derive(Debug)]
pub struct Thread {
    pub name: Option<String>,
    /// True only while the thread function is running.
    pub alive: bool,
    /// Result of the thread's function, once finished.
    pub result: Option<Value>,
    /// Error the thread function died with, if any.
    pub last_error: Option<Value>,
    /// A later signal to a completed thread, observed by `thread-join'.
    pub pending_signal: Option<(Value, Value)>,
    /// Lisp object this thread is waiting for, if any.
    pub blocker: Option<Value>,
    /// True once the function has run to completion.
    pub finished: bool,
}

/// A recursive Lisp mutex (`make-mutex').
#[derive(Debug)]
pub struct Mutex {
    pub name: Option<String>,
    /// `Some(thread)` while locked; cooperative model keeps this simple.
    pub owner: Option<Value>,
    pub count: usize,
}

/// A condition variable (`make-condition-variable').
#[derive(Debug)]
pub struct CondVar {
    pub name: Option<String>,
    pub mutex: Value,
    pub waiters: Vec<std::rc::Weak<std::cell::Cell<bool>>>,
}

/// Emacs fixnum range on 64-bit builds: 62 bits (2 tag bits in C).
/// Integers outside this range are immutable heap-allocated bignums.
pub const FIXNUM_MAX: i128 = (1i128 << 61) - 1;
pub const FIXNUM_MIN: i128 = -(1i128 << 61);
/// GNU's PSEUDOVECTOR_SIZE_MASK: record size limit, including the type slot.
pub const MAX_RECORD_SLOTS: usize = 4095;

#[derive(Clone)]
pub enum Value {
    Nil,
    Int(i128),
    BigInt(Rc<BigInt>),
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
        let cell = Rc::new(RefCell::new(Cons { car, cdr }));
        super::memory::allocated(&Value::Cons(cell.clone()));
        cell
    }
}

/// Hash table test function (make-hash-table :test ...).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum HashTest {
    Eq,
    Eql,
    Equal,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum HashWeakness {
    Key,
    Value,
    KeyAndValue,
    KeyOrValue,
}

impl HashWeakness {
    pub(crate) fn from_value(i: &crate::lisp::Interp, value: &Option<Value>) -> Option<Self> {
        let name = value
            .as_ref()
            .and_then(|value| i.sym_id(value))
            .map(|id| i.symbol_name(id));
        match name.as_deref() {
            Some("key") => Some(Self::Key),
            Some("value") => Some(Self::Value),
            Some("key-and-value" | "t") => Some(Self::KeyAndValue),
            Some("key-or-value") => Some(Self::KeyOrValue),
            _ => None,
        }
    }
}

#[derive(Clone)]
pub struct LispHash {
    pub test: HashTest,
    /// We key on a normalized form so `equal` keys hash correctly.
    pub map: HashMap<HashKey, Value>,
    /// Original keys for `maphash`/`hash-table-keys`, kept in GNU's
    /// slot order: entries append at the end, `remhash' leaves a
    /// tombstone and the next `puthash' reuses the most recently freed slot.
    pub keys: KeySlots,
    /// Lisp-visible weakness, also retained in printed/dumped tables.
    pub weakness: Option<Value>,
    /// Cache the policy independently of an interpreter's symbol IDs:
    /// the allocation registry can contain objects from several interpreters.
    pub(crate) gc_weakness: Option<HashWeakness>,
    /// Current GNU entry capacity (the initial :size hint, then grown).
    pub size: i128,
    /// :rehash-size / :rehash-threshold values (GNU stores them and
    /// returns them verbatim from the accessors).
    pub rehash_size: Value,
    pub rehash_threshold: Value,
    /// Name given to :test when it was a `define-hash-table-test'
    /// symbol — `hash-table-test' reports it verbatim.
    pub test_name: Value,
    /// Comparison and hash functions captured when a custom table is made.
    pub user_test: Option<(Value, Value)>,
    /// User callbacks may read this table, but must not mutate it.
    pub mutable: bool,
    /// Collision-chain insertion order, rebuilt from slots when resized.
    pub collision_order: Vec<usize>,
    /// GNU reuses removed collision-checked table slots in last-removed-first order.
    pub free_slots: Vec<usize>,
}

impl LispHash {
    pub fn new(test: HashTest) -> Self {
        LispHash {
            test,
            map: HashMap::default(),
            keys: KeySlots(Vec::new()),
            weakness: None,
            gc_weakness: None,
            size: 0,
            rehash_size: Value::float(1.5),
            rehash_threshold: Value::float(0.8125),
            test_name: Value::Nil,
            user_test: None,
            mutable: true,
            collision_order: Vec::new(),
            free_slots: Vec::new(),
        }
    }

    /// Record an original key: an already-present normalized key
    /// keeps its original object (as GNU's hash_put); otherwise the
    /// key takes the first tombstone slot, or appends.
    pub fn put_key(&mut self, hk: HashKey, k: Value) {
        if self.keys.0.iter().flatten().any(|(e, _)| *e == hk) {
            return;
        }
        let collision_checked = self.user_test.is_some() || self.test == HashTest::Equal;
        if self.keys.0.len() as i128 == self.size && self.keys.0.iter().all(Option::is_some) {
            self.size = if self.size == 0 {
                6
            } else {
                self.size.max(6) * if self.size <= 64 { 4 } else { 2 }
            };
            if collision_checked {
                // Resizing re-links collision chains in slot order,
                // without invoking the user comparison/hash functions.
                self.collision_order = (0..self.keys.0.len()).collect();
            }
        }
        if collision_checked {
            let slot = self.free_slots.pop().unwrap_or(self.keys.0.len());
            self.collision_order.push(slot);
            if slot == self.keys.0.len() {
                self.keys.0.push(Some((hk, k)));
            } else {
                self.keys.0[slot] = Some((hk, k));
            }
            return;
        }
        let slot = self.free_slots.pop().unwrap_or(self.keys.0.len());
        if slot == self.keys.0.len() {
            self.keys.0.push(Some((hk, k)));
        } else {
            self.keys.0[slot] = Some((hk, k));
        }
    }

    pub fn remove_key(&mut self, hk: &HashKey) {
        if let Some(slot) = self
            .keys
            .0
            .iter()
            .position(|e| matches!(e, Some((h, _)) if h == hk))
        {
            self.keys.0[slot] = None;
            self.free_slots.push(slot);
            if self.user_test.is_some() || self.test == HashTest::Equal {
                self.collision_order.retain(|index| *index != slot);
            }
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
    BigInt(BigInt),
    /// Bit pattern so -0.0/NaN behave deterministically.
    Float(u64),
    /// GNU string equality/hash use the stored byte representation,
    /// distinguishing unibyte bytes from multibyte Mule encodings.
    Str(Vec<u8>),
    /// Identity key for objects where `eq` compares by pointer.
    Ptr(usize),
    /// `equal' markers compare buffer and position, not object identity.
    /// Detached markers use position zero, regardless of their stale offset.
    Marker(Option<usize>, usize),
    Cons(Box<HashKey>, Box<HashKey>),
    Vec(Vec<HashKey>),
    /// Content key for records / lambdas (`equal' compares them
    /// elementwise but they are never `equal' to a plain vector, so
    /// the tag keeps the namespaces apart).  Tag: 1 = record,
    /// 2 = `#[...]' byte-code object, 3 = interpreted lambda.
    Tagged(u64, Vec<HashKey>),
    /// Bounded structural hash and root identity for cyclic/large equal
    /// keys. The hash only selects candidates; compare original keys.
    EqualFallback(u32, usize),
    /// User-provided hash code and an entry slot. Equal hash
    /// codes select candidates; only the user's comparator merges keys.
    Custom(u32, usize),
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
#[derive(Clone, Copy)]
pub struct Subr {
    pub name: &'static str,
    pub arity: Arity,
    pub func: SubrFn,
    pub doc: &'static str,
}

pub type SubrFn = fn(&mut crate::lisp::Interp, Argv) -> crate::lisp::EvalResult;

/// A subr argument vector, shared with the call frame on `lisp_stack'.
/// `Rc<Vec<Value>>' lets `apply_resolved' push the live argv onto the
/// backtrace stack with a refcount bump instead of cloning every
/// argument `Value' per call (a top allocator driver at startup).
/// `Deref' to the inner `Vec' keeps `args[i]'/`args.len()'/`args.iter()'
/// working; `&*args' coerces to `&[Value]' through two derefs.
#[derive(Clone)]
pub struct Argv(pub Rc<Vec<Value>>);

impl std::ops::Deref for Argv {
    type Target = [Value];
    fn deref(&self) -> &[Value] {
        &self.0
    }
}

impl Argv {
    /// Wrap a freshly-built argument vector.
    pub fn new(v: Vec<Value>) -> Argv {
        Argv(Rc::new(v))
    }
    /// Borrow the arguments as a plain slice.
    pub fn as_slice(&self) -> &[Value] {
        &self.0
    }
    /// Number of arguments.
    pub fn len(&self) -> usize {
        self.0.len()
    }
    /// Whether the vector is empty.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl From<Vec<Value>> for Argv {
    fn from(v: Vec<Value>) -> Argv {
        Argv(Rc::new(v))
    }
}

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
    /// True for functions whose source would be byte-compiled in GNU
    /// (dumped/shipped-library functions, plus closures created while
    /// one is running — e.g. `trace-make-advice''s wrapper).  Such a
    /// function's symbol-head calls to byte-op primitives resolve to
    /// the canonical subr directly, bypassing `symbol-function' like
    /// GNU's Bcar/Bcdr/... opcodes, so `advice' on a primitive can't
    /// recurse into the advice machinery's own calls.
    pub compiled: bool,
    /// `Some(items)' for objects read via `#[...]' — GNU treats those
    /// as byte-code objects, which are `arrayp' and `aref'-able.  We
    /// keep the original element list so `aref'/`length' see GNU's
    /// [args body env] slots; `nil' for ordinary lambdas.
    pub bc_items: Option<Rc<std::cell::RefCell<Vec<Value>>>>,
    /// True when this byte-code object was produced by `make-closure'
    /// (env slots spliced into the constants vector), as opposed to a
    /// bare `#[...]' literal read from data — e.g. the `#[...]' byte-code
    /// in uni-*.el char-table extra slots, which `unidata_bc_dispatch'
    /// routes natively and must not confuse with closures whose captured
    /// variables happen to sit in consts[0].
    pub bc_closure: bool,
    /// Cached instruction bytes and argument metadata. Rebuilt when the
    /// argument/code slots change or the instruction string is edited;
    /// constants are read live from their vector rather than snapshotted.
    pub(crate) bc_prog: RefCell<Option<Rc<crate::lisp::bytecode::BcProg>>>,
    /// GNU closure slot 4: the docstring *value* — a string for
    /// documented functions, but `(:documentation FORM)' may produce
    /// any value (oclosure.el stores its type symbol there).
    /// `doc' only holds the string case for `documentation'.
    pub doc_value: Option<Value>,
    /// GNU closure slot 2 verbatim: the env alist passed to
    /// `make-interpreted-closure'.  `None' → reconstructed from `env'
    /// (oldest-to-newest frame order reversed, like GNU's specbind
    /// prepends).  `oclosure--get' indexes this positionally.
    pub env_value: Option<Value>,
}

impl Lambda {
    /// GNU's PVEC_COMPILED check: a `#[...]' object is byte-code only
    /// when its code slot is a byte string (`#[ARGS "BYTESTR" CONSTS
    /// DEPTH ...]'); a cons code slot reads as an interpreted closure
    /// (lread.c read_bytecode).  Ordinary lambdas never are.
    pub fn is_bytecode(&self) -> bool {
        match &self.bc_items {
            Some(items) => matches!(items.borrow().get(1), Some(Value::Str(_))),
            None => false,
        }
    }
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

/// Lone surrogates (0xD800..=0xDFFF) aren't Rust chars but ARE valid
/// GNU char codes (`characterp' → t, `concat' inserts them, printing
/// yields U+FFFD).  Proxy them into plane-15 PUA above the eight-bit
/// block: codepoint c maps to U+F0100+(c-0xD800).
pub const SURROGATE_BASE: u32 = 0xF_0100;

/// A *raw output byte* marker: codepoints that can't live in a Rust
/// `char` (GNU mule codes 0x110000..=0x3FFFFF) are emitted into
/// strings as their mule byte sequence, one marker char per byte —
/// U+F0A00+b holds literal byte b.  The stdout/file encoders expand
/// the markers back to raw bytes; nothing else should see them.
pub const RAW_BYTE_BASE: u32 = 0xF_0A00;

/// Map a GNU char code to a storable Rust char; eight-bit codes and
/// surrogates become PUA proxies.
pub fn lisp_char(code: u32) -> Option<char> {
    match code {
        0x3FFF80..=0x3FFFFF => char::from_u32(EIGHT_BIT_BASE + (code & 0xFF)),
        0xD800..=0xDFFF => char::from_u32(SURROGATE_BASE + (code - 0xD800)),
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

/// If `c` is a surrogate proxy char, return its surrogate code.
pub fn surrogate_code(c: char) -> Option<u32> {
    let u = c as u32;
    (SURROGATE_BASE..SURROGATE_BASE + 0x800)
        .contains(&u)
        .then(|| 0xD800 + (u - SURROGATE_BASE))
}

/// alloc.c's make_string: use multibyte storage only when every byte belongs
/// to a valid canonical Emacs UTF-8 sequence, including surrogates and Mule.
/// A single invalid byte makes the entire input unibyte.
pub(crate) fn auto_string_bytes(bytes: &[u8]) -> (String, bool) {
    let unibyte = || (bytes.iter().map(|&byte| char::from(byte)).collect(), false);
    let mut text = String::new();
    let mut at = 0;
    let mut multibyte = false;
    while at < bytes.len() {
        let first = bytes[at];
        let length = match first {
            0..=127 => 1,
            0xc2..=0xdf => 2,
            0xe0..=0xef => 3,
            0xf0..=0xf7 => 4,
            0xf8 => 5,
            _ => return unibyte(),
        };
        if at + length > bytes.len() || bytes[at + 1..at + length].iter().any(|b| b & 0xc0 != 0x80) {
            return unibyte();
        }
        let mut code = u32::from(first & [0, 127, 31, 15, 7, 3][length]);
        for &byte in &bytes[at + 1..at + length] {
            code = (code << 6) | u32::from(byte & 63);
        }
        if code < [0, 0, 0x80, 0x800, 0x10000, 0x200000][length] || code >= 0x3fff80 {
            return unibyte();
        }
        text.push(lisp_char(code).unwrap_or_else(|| mule_seq_char(code)));
        multibyte |= length > 1;
        at += length;
    }
    (text, multibyte)
}

/// Decode stored Emacs multibyte characters, including byte8, surrogates
/// and five-byte Mule characters. Invalid octets retain byte8 identity.
pub(crate) fn emacs_string_bytes(bytes: &[u8]) -> String {
    let mut result = String::new();
    let mut at = 0;
    while at < bytes.len() {
        let first = bytes[at];
        let length = match first {
            0..=127 => 1,
            0xc0..=0xdf => 2,
            0xe0..=0xef => 3,
            0xf0..=0xf7 => 4,
            0xf8 => 5,
            _ => 0,
        };
        if length == 0 || at + length > bytes.len()
            || bytes[at + 1..at + length].iter().any(|b| b & 0xc0 != 0x80)
        {
            result.push(lisp_char(0x3fff00 + first as u32).unwrap());
            at += 1;
            continue;
        }
        let mut code = u32::from(first & [0, 127, 31, 15, 7, 3][length]);
        for byte in &bytes[at + 1..at + length] { code = (code << 6) | u32::from(byte & 63); }
        if first < 0xc2 && length == 2 { code += 0x3fff80; }
        if code > 0x3fffff {
            result.push(lisp_char(0x3fff00 + first as u32).unwrap());
            at += 1;
            continue;
        }
        result.push(lisp_char(code).unwrap_or_else(|| mule_seq_char(code)));
        at += length;
    }
    result
}

/// The GNU char code a string char stands for (proxies return their
/// real code: 0x3FFFxx for eight-bit, 0xD8xx..0xDFxx for surrogates,
/// 0x110000..=0x3FFF7F for mule-seq proxies).
pub fn lisp_char_code(c: char) -> i128 {
    if let Some(b) = eight_bit_byte(c) {
        return 0x3FFF00 + b as i128;
    }
    if let Some(s) = surrogate_code(c) {
        return s as i128;
    }
    if let Some(m) = mule_seq_code(c) {
        return m as i128;
    }
    c as i128
}

/// If `c` is a raw-byte marker, return the literal byte it carries.
pub fn raw_byte_marker(c: char) -> Option<u8> {
    let u = c as u32;
    (RAW_BYTE_BASE..RAW_BYTE_BASE + 0x100)
        .contains(&u)
        .then(|| (u - RAW_BYTE_BASE) as u8)
}

/// Base for printer-sentinel chars: `print`/`prin1` emit these for
/// unibyte byte-chars ≥0x80 so the *sink* can choose GNU's encoding —
/// stdout writes the overlong pair, a string/buffer gets `\NNN' text,
/// a function stream gets the eight-bit Lisp char code (0x3FFF80+b).
/// Distinct from `eight_bit_byte' proxies, which `princ' sinks treat
/// as the raw byte value.
pub const PRINT_EIGHT_BIT_BASE: u32 = 0xF_0C00;

/// Sentinel char for a printed eight-bit byte.
pub fn print_eight_bit_char(b: u8) -> char {
    char::from_u32(PRINT_EIGHT_BIT_BASE + b as u32).unwrap_or(' ')
}

/// If `c` is a printer-sentinel eight-bit char, return its byte.
pub fn print_eight_bit_byte(c: char) -> Option<u8> {
    let u = c as u32;
    (PRINT_EIGHT_BIT_BASE..PRINT_EIGHT_BIT_BASE + 0x100)
        .contains(&u)
        .then(|| (u - PRINT_EIGHT_BIT_BASE) as u8)
}

/// Base for `write-char' sentinels: GNU's printchar(0x3FFF80+code)
/// emits the eight-bit *character* — function streams get the code,
/// text sinks store the char itself (not `\NNN' text), stdout writes
/// the overlong pair.
pub const WRITE_CHAR_BASE: u32 = 0xF_0D00;

/// Sentinel standing for the eight-bit char of byte `b`.
pub fn write_char_eight_bit(b: u8) -> char {
    char::from_u32(WRITE_CHAR_BASE + b as u32).unwrap_or(' ')
}

/// If `c` is a `write-char' sentinel, return its byte.
pub fn write_char_eight_bit_byte(c: char) -> Option<u8> {
    let u = c as u32;
    (WRITE_CHAR_BASE..WRITE_CHAR_BASE + 0x100)
        .contains(&u)
        .then(|| (u - WRITE_CHAR_BASE) as u8)
}

/// A high-mule sequence proxy: GNU codes 0x110000..=0x3FFF7F can't
/// sit in a Rust `char`, so each distinct code is registered once and
/// surfaces in strings as a single U+F0B00+idx char — keeping the
/// description's column layout (GNU stores the code as one char).
/// The output encoder expands the proxy back to GNU's 5-byte mule
/// form 0xF8|(c>>24) + four 0x80|6-bit trails.
pub const MULE_SEQ_BASE: u32 = 0xF_0B00;

thread_local! {
    static MULE_SEQS: std::cell::RefCell<std::collections::HashMap<u32, u32>> =
        std::cell::RefCell::new(std::collections::HashMap::new());
    static MULE_SEQ_CODES: std::cell::RefCell<Vec<u32>> =
        std::cell::RefCell::new(Vec::new());
}

/// The proxy char for a mule code 0x110000..=0x3FFF7F.
pub fn mule_seq_char(code: u32) -> char {
    let idx = MULE_SEQS.with(|m| {
        let mut m = m.borrow_mut();
        if let Some(&i) = m.get(&code) {
            i
        } else {
            let i = m.len() as u32;
            m.insert(code, i);
            MULE_SEQ_CODES.with(|c| c.borrow_mut().push(code));
            i
        }
    });
    char::from_u32(MULE_SEQ_BASE + idx).unwrap_or('?')
}

/// If `c` is a mule-seq proxy, return its real GNU code.
pub fn mule_seq_code(c: char) -> Option<u32> {
    let u = c as u32;
    if (MULE_SEQ_BASE..MULE_SEQ_BASE + 0x10000).contains(&u) {
        MULE_SEQ_CODES.with(|codes| codes.borrow().get((u - MULE_SEQ_BASE) as usize).copied())
    } else {
        None
    }
}

/// The bytes GNU writes on stdout for a codepoint a Rust `char`
/// can't hold (0x110000..=0x3FFF7F): old-style extended UTF-8 —
/// four bytes 0xF0|(c>>18) + three 0x80|6-bit trails below 0x200000,
/// five bytes 0xF8|(c>>24) + four trails at/above it.
pub fn mule_high_bytes(code: u32) -> Vec<u8> {
    if code < 0x200000 {
        vec![
            0xF0 + ((code >> 18) & 7) as u8,
            0x80 + ((code >> 12) & 0x3F) as u8,
            0x80 + ((code >> 6) & 0x3F) as u8,
            0x80 + (code & 0x3F) as u8,
        ]
    } else {
        vec![
            0xF8 + ((code >> 24) & 3) as u8,
            0x80 + ((code >> 18) & 0x3F) as u8,
            0x80 + ((code >> 12) & 0x3F) as u8,
            0x80 + ((code >> 6) & 0x3F) as u8,
            0x80 + (code & 0x3F) as u8,
        ]
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
            Value::BigInt(n) => n.to_i128(),
            _ => None,
        }
    }

    /// Canonical integer construction: immediate fixnum or a fresh bignum.
    pub fn integer(n: i128) -> Value {
        if (FIXNUM_MIN..=FIXNUM_MAX).contains(&n) {
            Value::Int(n)
        } else {
            Value::BigInt(Rc::new(BigInt::from(n))).tracked()
        }
    }

    pub fn from_bigint(n: BigInt) -> Value {
        if let Some(n) = n
            .to_i128()
            .filter(|n| (FIXNUM_MIN..=FIXNUM_MAX).contains(n))
        {
            Value::Int(n)
        } else {
            Value::BigInt(Rc::new(n)).tracked()
        }
    }

    pub fn integerp(&self) -> bool {
        matches!(self, Value::Int(_) | Value::BigInt(_))
    }

    pub fn as_bigint(&self) -> Option<BigInt> {
        match self {
            Value::Int(n) => Some(BigInt::from(*n)),
            Value::BigInt(n) => Some((**n).clone()),
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
        Value::Str(Self::string_ref(s))
    }

    pub(crate) fn is_empty_string(&self) -> bool {
        matches!(self, Value::Str(text) if text.borrow().is_empty())
    }

    pub fn string_ref(s: impl Into<String>) -> StrRef {
        let string = Rc::new(RefCell::new(s.into()));
        super::memory::allocated(&Value::Str(string.clone()));
        string
    }

    pub fn vector_ref(values: Vec<Value>) -> VecRef {
        let vector = Rc::new(RefCell::new(values));
        super::memory::allocated(&Value::Vec(vector.clone()));
        vector
    }

    pub fn vector(values: Vec<Value>) -> Value {
        Value::Vec(Self::vector_ref(values))
    }

    pub fn record(values: Vec<Value>) -> Value {
        Value::Record(Self::vector_ref(values))
    }

    pub fn tracked(self) -> Value {
        super::memory::allocated(&self);
        self
    }

    /// A fresh float object (each call yields a distinct `eq' identity).
    pub fn float(x: f64) -> Value {
        Value::Float(Rc::new(x)).tracked()
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
        // Match GNU FOR_EACH_TAIL's Brent detector, including the tail
        // carried in a circular-list condition (not necessarily SELF).
        let mut tortoise = self.clone();
        let mut max = 2usize;
        let mut n = 0isize;
        let mut q = 2u16;
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
            q = q.wrapping_sub(1);
            if q == 0 {
                n -= 1;
                if n <= 0 {
                    max = max.saturating_mul(2);
                    q = max as u16;
                    n = (max >> 16) as isize;
                    tortoise = cur.clone();
                    continue;
                }
            }
            if let (Value::Cons(a), Value::Cons(b)) = (&tortoise, &cur) {
                if Rc::ptr_eq(a, b) {
                    return Err(ListError::Circular(cur));
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
    /// Circular list detected; carries GNU's meeting tail.
    Circular(Value),
}

impl fmt::Debug for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Nil => write!(f, "nil"),
            Value::Int(i) => write!(f, "{i}"),
            Value::BigInt(i) => write!(f, "{i}"),
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
            (Value::vector(vec![]), "Vec(..)"),
            (Value::record(vec![]), "Record(..)"),
            (
                Value::Hash(crate::lisp::memory::allocate(
                    LispHash::new(HashTest::Eq),
                    crate::lisp::Value::Hash,
                ))
                .tracked(),
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
