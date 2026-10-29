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
    held: Held,
    owners: usize,
    incoming: usize,
    strong: Vec<usize>,
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
    entries: Vec<Entry>,
}

impl Graph {
    fn new() -> Self {
        Self {
            ids: FxHashMap::default(),
            nodes: Vec::new(),
            entries: Vec::new(),
        }
    }

    fn insert(&mut self, identity: Identity, held: Held, owners: usize) -> usize {
        if let Some(&id) = self.ids.get(&identity) {
            return id;
        }
        let id = self.nodes.len();
        self.ids.insert(identity, id);
        self.nodes.push(Node {
            held,
            owners,
            incoming: 0,
            strong: Vec::new(),
            root: false,
            marked: false,
        });
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
            self.nodes[owner].strong.push(child);
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
        let objects: Vec<Object> =
            OBJECTS.with(|objects| objects.borrow().entries.values().cloned().collect());
        for object in objects {
            macro_rules! seed { ($($kind:ident => $value:ident),*) => { match object {
                $(Object::$kind(weak) => {
                    let owners = weak.strong_count();
                    if let Some(value) = weak.upgrade() {
                        let id = Rc::as_ptr(&value) as usize;
                        self.insert(Identity::Heap(id), Held::Value(Value::$value(value)), owners);
                    }
                },)*
            } }; }
            seed!(Cons=>Cons, Float=>Float, BigInt=>BigInt, String=>Str, Vector=>Vec,
                  Hash=>Hash, Lambda=>Lambda, Buffer=>Buffer, Marker=>Marker,
                  Window=>Window, Frame=>Frame, Process=>Process, Thread=>Thread,
                  Mutex=>Mutex, CondVar=>CondVar, Finalizer=>Finalizer);
        }
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
        for value in i.native_lisp_literals.values() { self.root(value); }
        for &symbol in i.fixed_symbols.values() { self.root(&Value::Sym(symbol)); }
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
        for (a, b) in i.pending_overlay_hooks.iter().chain(i.terminal_params.iter()) {
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
            queue.extend(self.nodes[id].strong.iter().copied());
        }
    }

    fn alive(&self, node: Option<usize>) -> bool {
        node.is_none_or(|id| self.nodes[id].marked)
    }

    fn finish(&mut self) {
        let roots: Vec<_> = self
            .nodes
            .iter()
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
            let Held::Value(Value::Hash(table)) = &self.nodes[entry.table].held else {
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

pub(super) fn sweep(i: &mut Interp) {
    let mut graph = Graph::new();
    graph.seed();
    graph.roots(i);
    let mut cursor = 0;
    while cursor < graph.nodes.len() {
        let held = graph.nodes[cursor].held.clone();
        graph.trace(cursor, held, i);
        cursor += 1;
    }
    graph.finish();
    graph.release_dead_side_tables(i);
}

#[cfg(test)]
mod tests {
    use super::*;

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
