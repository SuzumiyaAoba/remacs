//! Reachability for weak hash entries over the reference-counted heap.
//!
//! Count every owning edge, including weak table slots, to distinguish
//! external Rc owners from heap-internal references. Only ordinary edges
//! participate in the initial mark. Conditional weak-table edges are then
//! followed to a fixed point, so an entry cannot keep its own key alive.
//! Scalar symbol IDs retain their existing interpreter-long obarray lifetime;
//! their value/function/property cells remain external owners of Rc objects.

use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;

use rustc_hash::FxHashMap;

use super::{OBJECTS, Object};
use crate::buffer::textprops::TextProperties;
use crate::lisp::value::*;
use crate::lisp::{Interp, LexFrame};

const MAX_RETAINED_GRAPH_BYTES: usize = 256 * 1024 * 1024;

#[derive(Clone, Copy, Hash, PartialEq, Eq)]
enum Identity {
    Heap(usize),
    Environment(usize),
    Properties(usize),
}

#[derive(Clone)]
enum Held {
    Value(Value),
    Environment(Rc<LexFrame>),
    Properties(Rc<RefCell<TextProperties>>),
}

struct Node {
    held: Option<Held>,
    owners: usize,
    incoming: usize,
    strong_start: usize,
    strong_len: usize,
    root: bool,
    marked: bool,
}

fn survives(weakness: HashWeakness, key: bool, value: bool) -> bool {
    match weakness {
        HashWeakness::Key => key,
        HashWeakness::Value => value,
        HashWeakness::KeyAndValue => key && value,
        HashWeakness::KeyOrValue => key || value,
    }
}

struct Entry {
    table: usize,
    slot: usize,
    key: Option<usize>,
    value: Option<usize>,
    weakness: HashWeakness,
}

struct Graph {
    ids: FxHashMap<Identity, usize>,
    nodes: Vec<Node>,
    edges: Vec<usize>,
    entries: Vec<Entry>,
    next_node: usize,
}

impl Graph {
    fn new() -> Self {
        Self {
            ids: FxHashMap::default(),
            nodes: Vec::new(),
            edges: Vec::new(),
            entries: Vec::new(),
            next_node: 0,
        }
    }

    fn reset(&mut self) {
        self.ids.clear();
        self.edges.clear();
        self.entries.clear();
        for node in self.nodes.iter_mut().take(self.next_node) {
            node.held = None;
            node.owners = 0;
            node.incoming = 0;
            node.strong_start = 0;
            node.strong_len = 0;
            node.root = false;
            node.marked = false;
        }
        self.next_node = 0;
        // Reuse ordinary collection scratch, but do not pin an unusually
        // large one-time graph in a long-lived interpreter thread.
        if self.retained_bytes() > MAX_RETAINED_GRAPH_BYTES {
            *self = Self::new();
        }
    }

    fn retained_bytes(&self) -> usize {
        self.nodes
            .capacity()
            .saturating_mul(std::mem::size_of::<Node>())
            .saturating_add(
                self.edges
                    .capacity()
                    .saturating_mul(std::mem::size_of::<usize>()),
            )
            .saturating_add(
                self.entries
                    .capacity()
                    .saturating_mul(std::mem::size_of::<Entry>()),
            )
            .saturating_add(
                self.ids
                    .capacity()
                    .saturating_mul(std::mem::size_of::<(Identity, usize)>() + 1),
            )
    }

    fn insert(&mut self, identity: Identity, held: Held, owners: usize) -> usize {
        if let Some(&id) = self.ids.get(&identity) {
            return id;
        }
        let id = self.next_node;
        self.next_node += 1;
        let node = Node {
            held: Some(held),
            owners,
            incoming: 0,
            strong_start: 0,
            strong_len: 0,
            root: false,
            marked: false,
        };
        if id == self.nodes.len() {
            self.nodes.push(node);
        } else {
            self.nodes[id] = node;
        }
        self.ids.insert(identity, id);
        id
    }

    fn heap<T>(&mut self, value: &Rc<T>, wrap: fn(Rc<T>) -> Value) -> usize {
        let identity = Identity::Heap(Rc::as_ptr(value) as usize);
        if let Some(&id) = self.ids.get(&identity) {
            return id;
        }
        // Read before cloning: our analysis owner is not a Lisp root.
        let owners = Rc::strong_count(value);
        self.insert(identity, Held::Value(wrap(value.clone())), owners)
    }

    fn value(&mut self, value: &Value) -> Option<usize> {
        macro_rules! managed { ($($kind:ident),*) => { match value {
            $(Value::$kind(value) => Some(self.heap(value, Value::$kind)),)*
            _ => None,
        } }; }
        managed!(
            Cons, Float, BigInt, Str, Vec, Record, Hash, Lambda, Buffer, Marker, Window, Frame,
            Process, Thread, Mutex, CondVar, Finalizer
        )
    }

    fn environment(&mut self, env: &Rc<LexFrame>) -> usize {
        let identity = Identity::Environment(Rc::as_ptr(env) as usize);
        if let Some(&id) = self.ids.get(&identity) {
            return id;
        }
        let owners = Rc::strong_count(env);
        self.insert(identity, Held::Environment(env.clone()), owners)
    }

    fn properties(&mut self, props: &Rc<RefCell<TextProperties>>) -> usize {
        let identity = Identity::Properties(Rc::as_ptr(props) as usize);
        if let Some(&id) = self.ids.get(&identity) {
            return id;
        }
        let owners = Rc::strong_count(props);
        self.insert(identity, Held::Properties(props.clone()), owners)
    }

    fn link(&mut self, owner: usize, child: usize, strong: bool) {
        self.nodes[child].incoming += 1;
        if strong {
            self.edges.push(child);
            self.nodes[owner].strong_len += 1;
        }
    }

    fn edge(&mut self, owner: usize, value: &Value, strong: bool) -> Option<usize> {
        let child = self.value(value);
        if let Some(child) = child {
            self.link(owner, child, strong);
        }
        child
    }

    fn rc_edge<T>(&mut self, owner: usize, value: &Rc<T>, wrap: fn(Rc<T>) -> Value) {
        let child = self.heap(value, wrap);
        self.link(owner, child, true);
    }

    fn root(&mut self, value: &Value) {
        if let Some(id) = self.value(value) {
            self.nodes[id].root = true;
        }
    }

    fn seed(&mut self) {
        OBJECTS.with(|objects| {
            let objects = objects.borrow();
            for object in objects.entries.values() {
                macro_rules! seed {
                    ($($kind:ident($weak:ident $(, $extra:tt)*) => $value:ident),* $(,)?) => {
                        match object {
                            $(
                                Object::$kind($weak $(, $extra)*) => {
                                    let owners = $weak.strong_count();
                                    if let Some(value) = $weak.upgrade() {
                                        let id = Rc::as_ptr(&value) as usize;
                                        self.insert(
                                            Identity::Heap(id),
                                            Held::Value(Value::$value(value)),
                                            owners,
                                        );
                                    }
                                },
                            )*
                        }
                    };
                }
                seed!(
                    Cons(weak) => Cons,
                    Float(weak) => Float,
                    BigInt(weak) => BigInt,
                    String(weak, _) => Str,
                    Vector(weak) => Vec,
                    Hash(weak) => Hash,
                    Lambda(weak) => Lambda,
                    Buffer(weak) => Buffer,
                    Marker(weak) => Marker,
                    Window(weak) => Window,
                    Frame(weak) => Frame,
                    Process(weak) => Process,
                    Thread(weak) => Thread,
                    Mutex(weak) => Mutex,
                    CondVar(weak) => CondVar,
                    Finalizer(weak) => Finalizer,
                );
            }
        });
    }

    fn roots(&mut self, i: &Interp) {
        if let Some(env) = &i.lexenv {
            let node = self.environment(env);
            self.nodes[node].root = true;
        }
        for (function, args) in &i.lisp_stack {
            self.root(function);
            for value in args.iter() {
                self.root(value);
            }
        }
        self.root(&i.cur_head_form);
        self.root(&i.read_expression_history);
        self.root(&i.thread_last_error);
        self.root(&i.unread_switch_frame);
        for value in i.native_lisp_literals.values() {
            self.root(value);
        }
        for &symbol in i.fixed_symbols.values() {
            self.root(&Value::Sym(symbol));
        }
        for value in i
            .catch_tags
            .iter()
            .chain(i.command_args.iter())
            .chain(i.pending_insert_behind.iter())
            .chain(i.pending_insert_front.iter())
            .chain(i.kbd_macro_events.iter())
            .chain(i.command_key_events.iter())
            .chain(i.command_raw_key_events.iter())
            .chain(i.recent_key_events.iter())
        {
            self.root(value);
        }
        for handler in &i.case_handlers {
            for conditions in &handler.clauses {
                self.root(conditions);
            }
        }
        if let Some((a, b)) = &i.last_error {
            self.root(a);
            self.root(b);
        }
        for (a, b) in i
            .pending_overlay_hooks
            .iter()
            .chain(i.terminal_params.iter())
        {
            self.root(a);
            self.root(b);
        }
        for (conditions, handler, _) in &i.handler_bindings {
            self.root(conditions);
            self.root(handler);
        }
        for value in [
            &i.minibuf_exit_fn,
            &i.standard_case_table,
            &i.standard_category_table,
            &i.terminal,
            &i.unwind_quit,
        ] {
            if let Some(value) = value {
                self.root(value);
            }
        }
        for (face, value) in i.face_table.iter().chain(i.face_t_table.iter()) {
            self.root(&Value::Sym(*face));
            self.root(value);
        }
        for (_, value) in i.charsets.iter().chain(i.char_code_prop_tables.iter()) {
            self.root(value);
        }
        for (_, values) in &i.char_code_props {
            for (_, value) in values {
                self.root(value);
            }
        }
        for (symbol, value) in &i.var_watchers {
            self.root(&Value::Sym(*symbol));
            self.root(value);
        }
        for (symbol, value) in &i.advice_bases {
            self.root(&Value::Sym(*symbol));
            self.root(value);
        }
        for (symbol, values) in &i.advices {
            self.root(&Value::Sym(*symbol));
            for (name, function, props) in values {
                self.root(&Value::Sym(*name));
                self.root(function);
                self.root(props);
            }
        }
        for (a, b, c, d) in &i.advice_links {
            self.root(a);
            self.root(b);
            self.root(c);
            self.root(d);
        }
        self.root(&i.terminal_coding);
        self.root(&i.keyboard_coding);
    }

    fn trace(&mut self, owner: usize, held: Held, i: &Interp) {
        self.nodes[owner].strong_start = self.edges.len();
        self.nodes[owner].strong_len = 0;
        macro_rules! borrow {
            ($value:expr) => {{
                match $value.try_borrow_mut() {
                    Ok(guard) => drop(guard),
                    Err(_) => {
                        self.nodes[owner].root = true;
                        return;
                    }
                }
                match $value.try_borrow() {
                    Ok(value) => value,
                    Err(_) => {
                        self.nodes[owner].root = true;
                        return;
                    }
                }
            }};
        }
        match held {
            Held::Environment(env) => {
                let raw = borrow!(env.raw);
                self.edge(owner, &*raw, true);
            }
            Held::Properties(props) => {
                let props = borrow!(props);
                // Both the canonical intervals and the flat projection own
                // Values; count each actual Rc, including duplicate copies.
                for prop in props.iter() {
                    self.edge(owner, &prop.value, true);
                    self.edge(owner, &Value::Sym(prop.prop), true);
                }
                for (_, _, plist) in props.intervals() {
                    for value in plist {
                        self.edge(owner, value, true);
                    }
                }
            }
            Held::Value(value) => match value {
                Value::Cons(cell) => {
                    let cell = borrow!(cell);
                    self.edge(owner, &cell.car, true);
                    self.edge(owner, &cell.cdr, true);
                }
                Value::Vec(values) | Value::Record(values) => {
                    for value in borrow!(values).iter() {
                        self.edge(owner, value, true);
                    }
                    let pointer = Rc::as_ptr(&values) as usize;
                    for (id, _, value) in i
                        .char_table_parents
                        .iter()
                        .chain(i.char_table_defalts.iter())
                    {
                        if *id == pointer {
                            self.edge(owner, value, true);
                        }
                    }
                }
                Value::Str(string) => {
                    if let Some((_, props)) = i.string_props.get(&(Rc::as_ptr(&string) as usize)) {
                        for (_, _, plist) in props.iter() {
                            for value in plist {
                                self.edge(owner, value, true);
                            }
                        }
                    }
                }
                Value::Hash(table) => {
                    let table = borrow!(table);
                    for value in [
                        &table.rehash_size,
                        &table.rehash_threshold,
                        &table.test_name,
                    ] {
                        self.edge(owner, value, true);
                    }
                    if let Some(value) = &table.weakness {
                        self.edge(owner, value, true);
                    }
                    if let Some((test, hash)) = &table.user_test {
                        self.edge(owner, test, true);
                        self.edge(owner, hash, true);
                    }
                    let weakness = table.gc_weakness;
                    for (slot, pair) in table.keys.0.iter().enumerate() {
                        if let Some((normalized, key)) = pair {
                            let key = self.edge(owner, key, weakness.is_none());
                            if let Some(value) = table.map.get(normalized) {
                                let value = self.edge(owner, value, weakness.is_none());
                                if let Some(weakness) = weakness {
                                    self.entries.push(Entry {
                                        table: owner,
                                        slot,
                                        key,
                                        value,
                                        weakness,
                                    });
                                }
                            }
                        }
                    }
                }
                Value::Lambda(function) => {
                    for value in &function.body {
                        self.edge(owner, value, true);
                    }
                    for optional in &function.optional {
                        if let Some(value) = &optional.default {
                            self.edge(owner, value, true);
                        }
                    }
                    for value in [
                        &function.body_value,
                        &function.interactive,
                        &function.arglist,
                        &function.doc_value,
                        &function.env_value,
                    ] {
                        if let Some(value) = value {
                            self.edge(owner, value, true);
                        }
                    }
                    if let Some(items) = &function.bc_items {
                        self.rc_edge(owner, items, Value::Vec);
                    }
                    if let Some(env) = &function.env {
                        let child = self.environment(env);
                        self.link(owner, child, true);
                    }
                }
                Value::Buffer(buffer) => {
                    let buffer = borrow!(buffer);
                    for (&symbol, value) in &buffer.locals {
                        self.edge(owner, &Value::Sym(symbol), true);
                        self.edge(owner, value, true);
                    }
                    for overlay in &buffer.overlays {
                        self.edge(owner, &overlay.plist, true);
                        self.edge(owner, &overlay.handle, true);
                    }
                    for value in [
                        &buffer.syntax_table,
                        &buffer.case_table,
                        &buffer.category_table,
                        &buffer.suspended_mode,
                    ] {
                        if let Some(value) = value {
                            self.edge(owner, value, true);
                        }
                    }
                    for (_, _, value) in &buffer.narrow_labels {
                        self.edge(owner, value, true);
                    }
                    if let Some(base) = &buffer.base_buffer {
                        self.rc_edge(owner, base, Value::Buffer);
                    }
                    if let Some(mark) = &buffer.mark {
                        self.rc_edge(owner, mark, Value::Marker);
                    }
                    let props = self.properties(&buffer.text.props);
                    self.link(owner, props, true);
                }
                Value::Window(window) => {
                    let window = borrow!(window);
                    for value in [
                        &window.dedicated,
                        &window.params,
                        &window.prev_buffers,
                        &window.next_buffers,
                        &window.new_normal,
                        &window.old_buffer,
                        &window.old_point_marker,
                        &window.cursor_type,
                    ] {
                        self.edge(owner, value, true);
                    }
                    for child in &window.children {
                        self.rc_edge(owner, child, Value::Window);
                    }
                }
                Value::Frame(frame) => {
                    let frame = borrow!(frame);
                    self.rc_edge(owner, &frame.name, Value::Str);
                    self.edge(owner, &frame.params, true);
                    for window in &frame.windows {
                        self.rc_edge(owner, window, Value::Window);
                    }
                    self.rc_edge(owner, &frame.selected, Value::Window);
                    for window in [&frame.minibuffer, &frame.root, &frame.old_selected] {
                        if let Some(window) = window {
                            self.rc_edge(owner, window, Value::Window);
                        }
                    }
                }
                Value::Process(process) => {
                    let process = borrow!(process);
                    for value in process.gc_values() {
                        self.edge(owner, value, true);
                    }
                    if let Some(mark) = &process.mark {
                        self.rc_edge(owner, mark, Value::Marker);
                    }
                }
                Value::Thread(thread) => {
                    let thread = borrow!(thread);
                    for value in [&thread.result, &thread.last_error, &thread.blocker] {
                        if let Some(value) = value {
                            self.edge(owner, value, true);
                        }
                    }
                    if let Some((a, b)) = &thread.pending_signal {
                        self.edge(owner, a, true);
                        self.edge(owner, b, true);
                    }
                }
                Value::Mutex(mutex) => {
                    if let Some(value) = &borrow!(mutex).owner {
                        self.edge(owner, value, true);
                    }
                }
                Value::CondVar(variable) => {
                    self.edge(owner, &borrow!(variable).mutex, true);
                }
                Value::Finalizer(function) => {
                    let function = borrow!(function);
                    self.edge(owner, &*function, true);
                }
                _ => {}
            },
        }
    }

    fn mark(&mut self, initial: impl IntoIterator<Item = usize>) {
        let mut queue: VecDeque<_> = initial.into_iter().collect();
        while let Some(id) = queue.pop_front() {
            if self.nodes[id].marked {
                continue;
            }
            self.nodes[id].marked = true;
            let start = self.nodes[id].strong_start;
            let end = start + self.nodes[id].strong_len;
            queue.extend(self.edges[start..end].iter().copied());
        }
    }

    fn alive(&self, node: Option<usize>) -> bool {
        node.is_none_or(|id| self.nodes[id].marked)
    }

    fn finish(&mut self) {
        let roots: Vec<_> = self
            .nodes
            .iter()
            .take(self.next_node)
            .enumerate()
            .filter_map(|(id, node)| (node.root || node.owners > node.incoming).then_some(id))
            .collect();
        self.mark(roots);
        loop {
            let mut pending = Vec::new();
            for entry in &self.entries {
                if self.nodes[entry.table].marked
                    && survives(
                        entry.weakness,
                        self.alive(entry.key),
                        self.alive(entry.value),
                    )
                {
                    for node in [entry.key, entry.value].into_iter().flatten() {
                        if !self.nodes[node].marked {
                            pending.push(node);
                        }
                    }
                }
            }
            if pending.is_empty() {
                break;
            }
            self.mark(pending);
        }
        for entry in &self.entries {
            if self.nodes[entry.table].marked
                && survives(
                    entry.weakness,
                    self.alive(entry.key),
                    self.alive(entry.value),
                )
            {
                continue;
            }
            let Some(Held::Value(Value::Hash(table))) = &self.nodes[entry.table].held else {
                continue;
            };
            if let Ok(mut table) = table.try_borrow_mut() {
                if let Some(Some((key, _))) = table.keys.0.get(entry.slot) {
                    let key = key.clone();
                    table.map.remove(&key);
                    table.remove_key(&key);
                }
            }
        }
    }

    fn release_dead_side_tables(&self, i: &mut Interp) {
        let live = |pointer: usize| {
            self.ids
                .get(&Identity::Heap(pointer))
                .is_none_or(|id| self.nodes[*id].marked)
        };
        i.string_props.retain(|pointer, _| live(*pointer));
        i.char_table_parents
            .retain(|(pointer, _, _)| live(*pointer));
        i.char_table_defalts
            .retain(|(pointer, _, _)| live(*pointer));
    }
}

thread_local! {
    /// Weak-table collection visits the same interpreter object graph many
    /// times during source reads. Reuse scratch capacities, while dropping
    /// all temporary Lisp roots after each pass.
    static SCRATCH_GRAPH: RefCell<Graph> = RefCell::new(Graph::new());
}

struct ResetGraphOnDrop<'a>(&'a mut Graph);

impl Drop for ResetGraphOnDrop<'_> {
    fn drop(&mut self) {
        self.0.reset();
    }
}

pub(super) fn sweep(i: &mut Interp) {
    SCRATCH_GRAPH.with(|scratch| {
        let mut scratch = scratch.borrow_mut();
        scratch.reset();
        let mut graph = ResetGraphOnDrop(&mut scratch);
        let graph = &mut graph.0;
        graph.seed();
        graph.roots(i);
        let mut cursor = 0;
        while cursor < graph.next_node {
            let held = graph.nodes[cursor]
                .held
                .as_ref()
                .expect("active GC graph node has a held value")
                .clone();
            graph.trace(cursor, held, i);
            cursor += 1;
        }
        graph.finish();
        graph.release_dead_side_tables(i);
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn graph_reset_releases_held_values_and_reuses_storage() {
        let mut graph = Graph::new();
        let first = Value::cons(Value::Int(1), Value::Nil);
        let Value::Cons(cell) = &first else {
            unreachable!();
        };
        let weak = Rc::downgrade(cell);
        let id = graph.insert(
            Identity::Heap(Rc::as_ptr(cell) as usize),
            Held::Value(first.clone()),
            Rc::strong_count(cell),
        );
        assert_eq!(id, 0);
        graph.edges.extend([id, id]);
        graph.nodes[id].strong_len = 2;
        let node_capacity = graph.nodes.capacity();
        let edge_capacity = graph.edges.capacity();
        let id_capacity = graph.ids.capacity();

        graph.reset();
        assert_eq!(graph.nodes.len(), 1);
        assert!(graph.nodes[id].held.is_none());
        assert_eq!(weak.strong_count(), 1);
        assert_eq!(graph.edges.len(), 0);
        assert_eq!(graph.nodes.capacity(), node_capacity);
        assert_eq!(graph.edges.capacity(), edge_capacity);
        assert_eq!(graph.ids.capacity(), id_capacity);
        drop(first);
        assert_eq!(weak.strong_count(), 0);

        let second = Value::cons(Value::Int(2), Value::Nil);
        let Value::Cons(cell) = &second else {
            unreachable!();
        };
        assert_eq!(
            graph.insert(
                Identity::Heap(Rc::as_ptr(cell) as usize),
                Held::Value(second.clone()),
                Rc::strong_count(cell),
            ),
            0
        );
        assert_eq!(graph.nodes.len(), 1);
        graph.reset();
    }

    #[test]
    fn sweep_reuses_scratch_storage_between_collections() {
        std::thread::Builder::new()
            .stack_size(128 << 20)
            .spawn(|| {
                let mut interp = Interp::new();
                sweep(&mut interp);
                let first = SCRATCH_GRAPH.with(|scratch| {
                    let graph = scratch.borrow();
                    (
                        graph.nodes.len(),
                        graph.nodes.capacity(),
                        graph.edges.capacity(),
                        graph.ids.capacity(),
                        graph.entries.capacity(),
                    )
                });
                assert!(first.0 > 0);

                sweep(&mut interp);
                let second = SCRATCH_GRAPH.with(|scratch| {
                    let graph = scratch.borrow();
                    (
                        graph.nodes.len(),
                        graph.nodes.capacity(),
                        graph.edges.capacity(),
                        graph.ids.capacity(),
                        graph.entries.capacity(),
                    )
                });
                assert_eq!(second, first);
            })
            .unwrap()
            .join()
            .unwrap();
    }

    #[test]
    fn collection_preserves_a_borrowed_table_until_its_borrow_ends() {
        std::thread::Builder::new()
            .stack_size(128 << 20)
            .spawn(|| {
                let mut i = Interp::new();
                let value = i
                    .eval_str(
                        "(progn \
                (setq weak194-unit (make-hash-table :test 'eq :weakness 'key)) \
                (dotimes (n 32) (puthash (vector n) n weak194-unit)) weak194-unit)",
                    )
                    .unwrap();
                let Value::Hash(table) = value else {
                    panic!("expected hash table");
                };
                let guard = table.borrow();
                i.eval_str("(garbage-collect)").unwrap();
                assert_eq!(guard.map.len(), 32);
                drop(guard);
                i.eval_str("(garbage-collect)").unwrap();
                assert_eq!(table.borrow().map.len(), 0);
            })
            .unwrap()
            .join()
            .unwrap();
    }
}
