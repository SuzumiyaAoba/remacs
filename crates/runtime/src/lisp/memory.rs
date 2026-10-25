//! Allocation counters and liveness of managed Lisp objects.
//!
//! The allocation registry uses Weak entries without keeping values alive. Dead
//! entries are periodically removed, bounding accounting overhead between
//! explicit collections. Rc releases objects immediately; there is no cons,
//! float, or string-object free list to report as reusable storage.

use std::cell::{Cell, RefCell};
use std::mem::size_of;
use std::ops::{Deref, DerefMut};
use std::rc::{Rc, Weak};

use rustc_hash::FxHashMap;

use super::value::*;

mod weak;

pub const CONS: usize = 0;
pub const FLOAT: usize = 1;
pub const VECTOR: usize = 2;
pub const SYMBOL: usize = 3;
pub const CHARS: usize = 4;
pub const INTERVAL: usize = 5;
pub const STRING: usize = 6;
pub type Interval = (usize, usize, Vec<Value>);

#[derive(Clone, Copy, Default, Debug)]
struct Counters {
    allocated: [u64; 7],
    symbols: usize,
    symbol_capacity: usize,
    intervals: usize,
    interval_capacity: usize,
    bytes: u64,
}

thread_local! {
    static COUNTERS: Cell<Counters> = Cell::new(Counters::default());
    static OBJECTS: RefCell<Objects> = RefCell::new(Objects::default());
    static LAST_GC_BYTES: Cell<u64> = const { Cell::new(0) };
    static COLLECTING: Cell<bool> = const { Cell::new(false) };
}

fn change(f: impl FnOnce(&mut Counters)) {
    // Other thread-local values can release Lisp objects during teardown.
    let _ = COUNTERS.try_with(|cell| {
        let mut counters = cell.get();
        f(&mut counters);
        cell.set(counters);
    });
}

fn add(counters: &mut Counters, index: usize, count: usize) {
    counters.allocated[index] =
        counters.allocated[index].wrapping_add(count as u64) & FIXNUM_MAX as u64;
}

pub fn counts() -> [u64; 7] {
    COUNTERS.with(|cell| cell.get().allocated)
}

pub fn allocated_bytes() -> u64 {
    COUNTERS.with(|cell| cell.get().bytes)
}

pub fn symbol_allocated() {
    change(|c| {
        add(c, SYMBOL, 1);
        c.symbols += 1;
        c.bytes = c.bytes.saturating_add(symbol_size() as u64);
    });
}

pub fn symbol_freed() {
    change(|c| c.symbols = c.symbols.saturating_sub(1));
}

pub fn symbol_capacity(old: usize, new: usize) {
    change(|c| c.symbol_capacity = c.symbol_capacity.saturating_sub(old) + new);
}

fn heap_size<T>() -> usize {
    // Rc's strong/weak counts precede the payload. All managed payloads
    // have alignment no greater than the two-word header.
    size_of::<T>() + 2 * size_of::<usize>()
}

#[derive(Clone)]
enum Object {
    Cons(Weak<RefCell<Cons>>),
    Float(Weak<f64>),
    BigInt(Weak<num_bigint::BigInt>),
    String(Weak<RefCell<String>>),
    Vector(Weak<RefCell<Vec<Value>>>),
    Hash(Weak<RefCell<LispHash>>),
    Lambda(Weak<Lambda>),
    Buffer(Weak<RefCell<crate::buffer::Buffer>>),
    Marker(Weak<RefCell<Marker>>),
    Window(Weak<RefCell<crate::editor::Window>>),
    Frame(Weak<RefCell<crate::editor::Frame>>),
    Process(Weak<RefCell<super::process::Proc>>),
    Thread(Weak<RefCell<Thread>>),
    Mutex(Weak<RefCell<Mutex>>),
    CondVar(Weak<RefCell<CondVar>>),
    Finalizer(Weak<RefCell<Value>>),
}

impl Object {
    fn alive(&self) -> bool {
        macro_rules! alive { ($($variant:ident),*) => {
            match self { $(Self::$variant(value) => value.strong_count() != 0,)* }
        }; }
        alive!(
            Cons, Float, BigInt, String, Vector, Hash, Lambda, Buffer, Marker, Window, Frame,
            Process, Thread, Mutex, CondVar, Finalizer
        )
    }

    fn metrics(&self) -> Metrics {
        let vector = |bytes: usize| Metrics {
            vectors: 1,
            vector_words: bytes.div_ceil(size_of::<usize>()),
            cells: bytes.div_ceil(size_of::<Value>()),
            bytes,
            ..Metrics::default()
        };
        match self {
            Self::Cons(_) => Metrics {
                conses: 1,
                bytes: cons_size(),
                ..Metrics::default()
            },
            Self::Float(_) => Metrics {
                floats: 1,
                bytes: float_size(),
                ..Metrics::default()
            },
            Self::BigInt(weak) => {
                let bytes = weak
                    .upgrade()
                    .map_or(0, |n| n.bits().div_ceil(64) as usize * 8);
                vector(heap_size::<num_bigint::BigInt>() + bytes)
            }
            Self::String(weak) => {
                let string = weak.upgrade();
                let string = string.as_ref().and_then(|s| s.try_borrow().ok());
                Metrics {
                    strings: 1,
                    // GNU counts allocated string bytes, including the
                    // bytes of multibyte characters (alloc.c).
                    chars: string.as_ref().map_or(0, |s| s.len()),
                    string_bytes: string.as_ref().map_or(0, |s| s.capacity()),
                    bytes: string_size() + string.as_ref().map_or(0, |s| s.capacity()),
                    ..Metrics::default()
                }
            }
            Self::Vector(weak) => {
                let values = weak.upgrade();
                let values = values.as_ref().and_then(|v| v.try_borrow().ok());
                let len = values.as_ref().map_or(0, |v| v.len());
                let capacity = values.as_ref().map_or(0, |v| v.capacity());
                let mut metrics =
                    vector(heap_size::<RefCell<Vec<Value>>>() + len * size_of::<Value>());
                metrics.cells = len;
                metrics.free_vector_words =
                    capacity.saturating_sub(len) * size_of::<Value>() / size_of::<usize>();
                metrics.bytes += capacity.saturating_sub(len) * size_of::<Value>();
                metrics
            }
            Self::Hash(_) => vector(heap_size::<RefCell<LispHash>>()),
            Self::Lambda(_) => vector(heap_size::<Lambda>()),
            Self::Buffer(_) => {
                let mut metrics = vector(buffer_size());
                metrics.buffers = 1;
                metrics
            }
            Self::Marker(_) => vector(heap_size::<RefCell<Marker>>()),
            Self::Window(_) => vector(heap_size::<RefCell<crate::editor::Window>>()),
            Self::Frame(_) => vector(heap_size::<RefCell<crate::editor::Frame>>()),
            Self::Process(_) => vector(heap_size::<RefCell<super::process::Proc>>()),
            Self::Thread(_) => vector(heap_size::<RefCell<Thread>>()),
            Self::Mutex(_) => vector(heap_size::<RefCell<Mutex>>()),
            Self::CondVar(_) => vector(heap_size::<RefCell<CondVar>>()),
            Self::Finalizer(_) => vector(heap_size::<RefCell<Value>>()),
        }
    }
}

#[derive(Default)]
struct Objects {
    entries: FxHashMap<usize, Object>,
    next_reap: usize,
}

impl Objects {
    fn reap(&mut self) {
        self.entries.retain(|_, object| object.alive());
        self.next_reap = self.entries.len() + (self.entries.len() / 2).max(8192);
    }
}

#[derive(Default)]
struct Metrics {
    conses: usize,
    floats: usize,
    strings: usize,
    chars: usize,
    string_bytes: usize,
    vectors: usize,
    vector_words: usize,
    free_vector_words: usize,
    cells: usize,
    buffers: usize,
    bytes: usize,
}

/// Record a freshly allocated managed object. Aliases can safely pass
/// through here too: pointer identity and a weak entry prevent recounting.
pub fn allocated(value: &Value) {
    macro_rules! object {
        ($value:expr, $variant:ident) => {
            (
                Rc::as_ptr($value) as usize,
                Object::$variant(Rc::downgrade($value)),
            )
        };
    }
    let (id, object) = match value {
        Value::Cons(v) => object!(v, Cons),
        Value::Float(v) => object!(v, Float),
        Value::BigInt(v) => object!(v, BigInt),
        Value::Str(v) => object!(v, String),
        Value::Vec(v) | Value::Record(v) => object!(v, Vector),
        Value::Hash(v) => object!(v, Hash),
        Value::Lambda(v) => object!(v, Lambda),
        Value::Buffer(v) => object!(v, Buffer),
        Value::Marker(v) => object!(v, Marker),
        Value::Window(v) => object!(v, Window),
        Value::Frame(v) => object!(v, Frame),
        Value::Process(v) => object!(v, Process),
        Value::Thread(v) => object!(v, Thread),
        Value::Mutex(v) => object!(v, Mutex),
        Value::CondVar(v) => object!(v, CondVar),
        Value::Finalizer(v) => object!(v, Finalizer),
        _ => return,
    };
    OBJECTS.with(|objects| {
        let mut objects = objects.borrow_mut();
        if objects.entries.contains_key(&id) {
            return;
        }
        let metrics = object.metrics();
        change(|c| {
            add(c, CONS, metrics.conses);
            add(c, FLOAT, metrics.floats);
            add(c, VECTOR, metrics.cells);
            add(c, CHARS, metrics.chars);
            add(c, STRING, metrics.strings);
            c.bytes = c.bytes.saturating_add(metrics.bytes as u64);
        });
        objects.entries.insert(id, object);
        if objects.entries.len() >= objects.next_reap {
            objects.reap();
        }
    });
}

pub fn allocate<T>(value: T, variant: fn(Rc<RefCell<T>>) -> Value) -> Rc<RefCell<T>> {
    let object = Rc::new(RefCell::new(value));
    allocated(&variant(object.clone()));
    object
}

/// Accounting weak references must not change Rc::get_mut's behavior at
/// the closure metadata update site. Temporarily release only our weak
/// observer; shared closures and any other weak observers still prevent
/// mutation, just as before allocation tracking was installed.
pub fn update_unique_lambda(lambda: &mut LambdaRef, update: impl FnOnce(&mut Lambda)) {
    if Rc::strong_count(lambda) != 1 {
        return;
    }
    let id = Rc::as_ptr(lambda) as usize;
    let tracked = OBJECTS.with(|objects| objects.borrow_mut().entries.remove(&id).is_some());
    if let Some(value) = Rc::get_mut(lambda) {
        update(value);
    }
    if tracked {
        OBJECTS.with(|objects| {
            objects
                .borrow_mut()
                .entries
                .insert(id, Object::Lambda(Rc::downgrade(lambda)));
        });
    }
}

/// Rc immediately frees object payloads. This releases dead accounting
/// weak references and their control blocks; it does not invent a free pool.
pub fn collect() {
    OBJECTS.with(|objects| objects.borrow_mut().reap());
}

fn prune_side_tables(i: &mut super::Interp) {
    loop {
        let before = i.string_props.len();
        i.string_props
            .retain(|_, (owner, _)| owner.strong_count() != 0);
        if i.string_props.len() == before {
            break;
        }
    }
    i.unibyte_strings
        .retain(|_, owner| owner.strong_count() != 0);
    i.multibyte_strings
        .retain(|_, owner| owner.strong_count() != 0);
    i.char_table_parents
        .retain(|(_, owner, _)| owner.strong_count() != 0);
    i.char_table_defalts
        .retain(|(_, owner, _)| owner.strong_count() != 0);
}

/// Release stale side-table roots before observing Rc liveness. Ordinary
/// object payloads are reclaimed as soon as their last strong Rc is dropped.
pub fn collect_interp(i: &mut super::Interp) -> bool {
    if COLLECTING.with(|flag| flag.replace(true)) {
        return false;
    }
    struct Guard;
    impl Drop for Guard {
        fn drop(&mut self) {
            COLLECTING.with(|flag| flag.set(false));
        }
    }
    let _guard = Guard;
    let start = std::time::Instant::now();
    // Memoized expansions are implementation caches, not Lisp roots.
    i.macro_cache.clear();
    i.macroexpand_all_cache.borrow_mut().1.clear();
    prune_side_tables(i);
    weak::sweep(i);
    // A dead string's property values can own another string with its
    // own property tree. Prune to a fixed point, regardless of map order.
    prune_side_tables(i);
    collect();
    LAST_GC_BYTES.with(|bytes| bytes.set(allocated_bytes()));
    let done = i.q("gcs-done");
    let count = match i.symbol_value(done) {
        Value::Int(n) => n,
        _ => 0,
    };
    i.obarray.symbol_mut(done).value = Value::Int(count + 1);
    let elapsed = i.q("gc-elapsed");
    let seconds = match i.symbol_value(elapsed) {
        Value::Float(n) => *n,
        _ => 0.0,
    };
    i.obarray.symbol_mut(elapsed).value = Value::float(seconds + start.elapsed().as_secs_f64());
    // GNU runs post-gc-hook safely; a failing hook must not abort collection.
    let _ = super::builtins::evalfn::call_hook(i, "post-gc-hook");
    true
}

pub fn allocated_since_gc() -> u64 {
    allocated_bytes().saturating_sub(LAST_GC_BYTES.with(Cell::get))
}

#[derive(Default, Debug)]
pub struct HeapStats {
    pub bytes: usize,
    pub conses: usize,
    pub symbols: usize,
    pub free_symbols: usize,
    pub strings: usize,
    pub string_bytes: usize,
    pub vectors: usize,
    pub vector_words: usize,
    pub free_vector_words: usize,
    pub floats: usize,
    pub intervals: usize,
    pub free_intervals: usize,
    pub buffers: usize,
}

pub fn heap_stats() -> HeapStats {
    let c = COUNTERS.with(Cell::get);
    let mut stats = HeapStats {
        bytes: c.symbols * symbol_size() + c.intervals * interval_size(),
        symbols: c.symbols,
        free_symbols: c.symbol_capacity.saturating_sub(c.symbols),
        intervals: c.intervals,
        free_intervals: c.interval_capacity.saturating_sub(c.intervals),
        ..HeapStats::default()
    };
    OBJECTS.with(|objects| {
        for object in objects
            .borrow()
            .entries
            .values()
            .filter(|object| object.alive())
        {
            let metrics = object.metrics();
            stats.bytes += metrics.bytes;
            stats.conses += metrics.conses;
            stats.strings += metrics.strings;
            stats.string_bytes += metrics.string_bytes;
            stats.floats += metrics.floats;
            stats.vectors += metrics.vectors;
            stats.vector_words += metrics.vector_words;
            stats.free_vector_words += metrics.free_vector_words;
            stats.buffers += metrics.buffers;
        }
    });
    stats
}

pub fn cons_size() -> usize {
    heap_size::<RefCell<Cons>>()
}
pub fn string_size() -> usize {
    heap_size::<RefCell<String>>()
}
pub fn float_size() -> usize {
    heap_size::<f64>()
}
pub fn buffer_size() -> usize {
    heap_size::<RefCell<crate::buffer::Buffer>>()
}
pub fn vector_size() -> usize {
    heap_size::<RefCell<Vec<Value>>>() + size_of::<Value>()
}
pub fn symbol_size() -> usize {
    size_of::<super::obarray::Symbol>()
}
pub fn interval_size() -> usize {
    size_of::<Interval>()
}

/// Snapshot before allocating the report itself, preserving GNU's row
/// shapes. Sizes describe this Rust representation, not GNU's C structs.
pub fn report(i: &mut super::Interp) -> Value {
    let s = heap_stats();
    let mut row = |name: &str, size: usize, used: usize, free: Option<usize>| {
        let mut values = vec![
            Value::Sym(i.intern(name)),
            Value::Int(size as i128),
            Value::Int(used as i128),
        ];
        if let Some(free) = free {
            values.push(Value::Int(free as i128));
        }
        Value::list(values)
    };
    Value::list(vec![
        row("conses", cons_size(), s.conses, Some(0)),
        row("symbols", symbol_size(), s.symbols, Some(s.free_symbols)),
        row("strings", string_size(), s.strings, Some(0)),
        row("string-bytes", 1, s.string_bytes, None),
        row("vectors", vector_size(), s.vectors, None),
        row(
            "vector-slots",
            size_of::<usize>(),
            s.vector_words,
            Some(s.free_vector_words),
        ),
        row("floats", float_size(), s.floats, Some(0)),
        row(
            "intervals",
            interval_size(),
            s.intervals,
            Some(s.free_intervals),
        ),
        row("buffers", buffer_size(), s.buffers, None),
    ])
}

/// Owned interval records. Counting the collection (rather than its
/// legacy flat projection) avoids double-counting shared buffer text.
#[derive(Default)]
pub struct Intervals(Vec<Interval>);

impl Intervals {
    pub fn new(values: Vec<Interval>) -> Self {
        change(|c| {
            add(c, INTERVAL, values.len());
            c.intervals += values.len();
            c.interval_capacity += values.capacity();
            c.bytes = c
                .bytes
                .saturating_add((values.capacity() * interval_size()) as u64);
        });
        Self(values)
    }
    pub fn edit(&mut self) -> IntervalMut<'_> {
        let len = self.0.len();
        let capacity = self.0.capacity();
        IntervalMut {
            values: &mut self.0,
            len,
            capacity,
        }
    }
    pub fn into_vec(mut self) -> Vec<Interval> {
        let values = std::mem::take(&mut self.0);
        change(|c| {
            c.intervals = c.intervals.saturating_sub(values.len());
            c.interval_capacity = c.interval_capacity.saturating_sub(values.capacity());
        });
        values
    }
}
impl From<Vec<Interval>> for Intervals {
    fn from(value: Vec<Interval>) -> Self {
        Self::new(value)
    }
}
impl Clone for Intervals {
    fn clone(&self) -> Self {
        Self::new(self.0.clone())
    }
}
impl Deref for Intervals {
    type Target = Vec<Interval>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl Drop for Intervals {
    fn drop(&mut self) {
        change(|c| {
            c.intervals = c.intervals.saturating_sub(self.0.len());
            c.interval_capacity = c.interval_capacity.saturating_sub(self.0.capacity());
        });
    }
}
pub struct IntervalMut<'a> {
    values: &'a mut Vec<Interval>,
    len: usize,
    capacity: usize,
}
impl Deref for IntervalMut<'_> {
    type Target = Vec<Interval>;
    fn deref(&self) -> &Self::Target {
        self.values
    }
}
impl DerefMut for IntervalMut<'_> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.values
    }
}
impl Drop for IntervalMut<'_> {
    fn drop(&mut self) {
        change(|c| {
            add(c, INTERVAL, self.values.len().saturating_sub(self.len));
            c.intervals = c.intervals.saturating_sub(self.len) + self.values.len();
            c.interval_capacity =
                c.interval_capacity.saturating_sub(self.capacity) + self.values.capacity();
            c.bytes = c.bytes.saturating_add(
                (self.values.capacity().saturating_sub(self.capacity) * interval_size()) as u64,
            );
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accounting_observer_preserves_unique_closure_updates() {
        std::thread::Builder::new()
            .stack_size(64 << 20)
            .spawn(|| {
                let mut interp = super::super::Interp::new();
                let Value::Lambda(mut lambda) =
                    interp.eval_str("(function (lambda () nil))").unwrap()
                else {
                    panic!("expected a closure");
                };
                assert_eq!(Rc::strong_count(&lambda), 1);
                let allocated = counts();
                let old = lambda.compiled;
                update_unique_lambda(&mut lambda, |value| value.compiled = !old);
                assert_eq!(lambda.compiled, !old);
                let alias = lambda.clone();
                update_unique_lambda(&mut lambda, |value| value.compiled = old);
                assert_eq!(lambda.compiled, !old);
                drop(alias);
                let weak = Rc::downgrade(&lambda);
                update_unique_lambda(&mut lambda, |value| value.compiled = old);
                assert_eq!(lambda.compiled, !old);
                drop(weak);
                update_unique_lambda(&mut lambda, |value| value.compiled = old);
                assert_eq!(lambda.compiled, old);
                assert_eq!(counts(), allocated);
            })
            .unwrap()
            .join()
            .unwrap();
    }

    #[test]
    fn allocation_counts_distinguish_types_and_aliases() {
        let before = counts();
        let live = heap_stats();
        let cons = Value::cons(Value::Nil, Value::Nil);
        let float = Value::float(2.5);
        let string = Value::string("日本語");
        let vector = Value::vector(vec![Value::Nil; 37]);
        let alias = cons.clone().tracked();
        let after = counts();
        assert_eq!(after[CONS] - before[CONS], 1);
        assert_eq!(after[FLOAT] - before[FLOAT], 1);
        assert_eq!(after[STRING] - before[STRING], 1);
        assert_eq!(after[CHARS] - before[CHARS], "日本語".len() as u64);
        assert_eq!(after[VECTOR] - before[VECTOR], 37);
        let used = heap_stats();
        assert_eq!(used.conses, live.conses + 1);
        assert_eq!(used.floats, live.floats + 1);
        assert_eq!(used.strings, live.strings + 1);
        assert_eq!(used.vectors, live.vectors + 1);
        drop((cons, float, string, vector));
        assert_eq!(heap_stats().conses, live.conses + 1);
        drop(alias);
        collect();
        let released = heap_stats();
        assert_eq!(released.conses, live.conses);
        assert_eq!(released.floats, live.floats);
        assert_eq!(released.strings, live.strings);
        assert_eq!(released.vectors, live.vectors);
        assert_eq!(counts(), after);
    }

    #[test]
    fn reusable_capacity_is_not_cumulative_deallocation() {
        let before = heap_stats();
        let mut values = Vec::with_capacity(64);
        values.resize(7, Value::Nil);
        let vector = Value::vector_ref(values);
        let word_ratio = size_of::<Value>() / size_of::<usize>();
        assert_eq!(
            heap_stats().free_vector_words,
            before.free_vector_words + 57 * word_ratio
        );
        vector.borrow_mut().truncate(2);
        assert_eq!(
            heap_stats().free_vector_words,
            before.free_vector_words + 62 * word_ratio
        );
        drop(vector);
        collect();
        assert_eq!(heap_stats().free_vector_words, before.free_vector_words);
        assert_eq!(heap_stats().conses, before.conses);
    }

    #[test]
    fn interval_ownership_and_edits_track_storage() {
        let before = heap_stats();
        let allocated = counts()[INTERVAL];
        let mut intervals = Intervals::new(vec![(0, 3, vec![])]);
        let copy = intervals.clone();
        intervals.edit().push((3, 6, vec![]));
        assert_eq!(heap_stats().intervals, before.intervals + 3);
        assert_eq!(counts()[INTERVAL] - allocated, 3);
        intervals.edit().clear();
        assert_eq!(heap_stats().intervals, before.intervals + 1);
        assert!(heap_stats().free_intervals > before.free_intervals);
        drop((intervals, copy));
        assert_eq!(heap_stats().intervals, before.intervals);
        assert_eq!(heap_stats().free_intervals, before.free_intervals);
        assert_eq!(counts()[INTERVAL] - allocated, 3);
    }

    #[test]
    fn symbols_and_editor_objects_are_accounted() {
        let before = heap_stats();
        let counts_before = counts();
        let mut symbols = super::super::obarray::Obarray::new();
        let count = symbols.len();
        let id = symbols.intern("memory-counter-test");
        assert_eq!(symbols.intern("memory-counter-test"), id);
        symbols.make_symbol("memory-counter-test");
        assert_eq!(counts()[SYMBOL] - counts_before[SYMBOL], count as u64 + 2);
        assert_eq!(heap_stats().symbols, before.symbols + count + 2);
        let frame = crate::editor::Frame::new_tty(0, 1, 80, 24);
        assert_eq!(heap_stats().vectors, before.vectors + 3); // frame + two windows
        assert!(counts()[VECTOR] > counts_before[VECTOR]);
        drop((frame, symbols));
        collect();
        assert_eq!(heap_stats().symbols, before.symbols);
        assert_eq!(heap_stats().free_symbols, before.free_symbols);
        assert_eq!(heap_stats().vectors, before.vectors);
    }
}
