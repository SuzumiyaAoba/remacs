use std::cell::RefCell;
use std::rc::Rc;

use rustc_hash::FxHashMap;
use serde::{Deserialize, Serialize};

use crate::lisp::eval::LexFrame;
use crate::lisp::obarray::sym;
use crate::lisp::value::*;
use crate::lisp::{Interp, memory};

/// Encode Emacs characters rather than process-local Unicode proxy indices.
pub(super) fn encode_text(text: &str) -> Vec<u32> {
    text.chars()
        .map(|ch| mule_seq_code(ch).unwrap_or(ch as u32))
        .collect()
}
pub(super) fn decode_text(text: &[u32]) -> String {
    text.iter()
        .map(|code| char::from_u32(*code).unwrap_or_else(|| mule_seq_char(*code)))
        .collect()
}

pub(super) type Result<T> = std::result::Result<T, String>;

/// References, not nested copies, are the wire representation of heap
/// objects. In particular records/vectors may share the same backing Rc.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub(super) enum Ref {
    Nil,
    Int(String),
    Symbol(u32),
    Object(u32),
    Record(u32),
    Subr(u32, String),
}

#[derive(Serialize, Deserialize)]
pub(super) enum Node {
    Symbol(Box<SymbolData>),
    String {
        text: Vec<u32>,
        representation: u8,
        properties: Option<Vec<(usize, usize, Vec<Ref>)>>,
    },
    Float(u64),
    Cons(Ref, Ref),
    Vector(Vec<Ref>),
    OverlayVector(Vec<Ref>),
    Hash(Box<HashData>),
    Lambda(Box<LambdaData>),
    Environment {
        raw: Ref,
        declared: Vec<Ref>,
    },
    Marker {
        buffer: Option<usize>,
        position: usize,
        insertion_type: bool,
    },
    Mutex {
        name: Option<Vec<u32>>,
        owner: Option<Ref>,
    },
    CondVar {
        name: Option<Vec<u32>>,
        mutex: Ref,
    },
    Finalizer(Ref),
    External(String),
}

impl Node {
    pub fn references(&self) -> Vec<u32> {
        fn add(out: &mut Vec<u32>, reference: &Ref) {
            if let Ref::Symbol(id) | Ref::Object(id) | Ref::Record(id) = reference {
                out.push(*id);
            }
        }
        let mut out = Vec::new();
        match self {
            Self::Symbol(s) => {
                for r in [&s.value, &s.function, &s.plist]
                    .into_iter()
                    .chain(s.alias.iter())
                {
                    add(&mut out, r);
                }
            }
            Self::String { properties, .. } => {
                if let Some(properties) = properties {
                    for (_, _, p) in properties {
                        for r in p {
                            add(&mut out, r);
                        }
                    }
                }
            }
            Self::Cons(a, b) => {
                add(&mut out, a);
                add(&mut out, b);
            }
            Self::Vector(values) | Self::OverlayVector(values) => {
                for r in values {
                    add(&mut out, r);
                }
            }
            Self::Hash(h) => {
                for (a, b) in &h.contents {
                    add(&mut out, a);
                    add(&mut out, b);
                }
                for r in [&h.rehash_size, &h.rehash_threshold, &h.test_name]
                    .into_iter()
                    .chain(h.weakness.iter())
                {
                    add(&mut out, r);
                }
            }
            Self::Lambda(l) => {
                for r in l
                    .required
                    .iter()
                    .chain(l.rest.iter())
                    .chain(l.body.iter())
                    .chain(l.interactive.iter())
                    .chain(l.arglist.iter())
                    .chain(l.bc_items.iter())
                    .chain(l.doc_value.iter())
                    .chain(l.env_value.iter())
                {
                    add(&mut out, r);
                }
                for (name, value, supplied) in &l.optional {
                    add(&mut out, name);
                    for r in value.iter().chain(supplied.iter()) {
                        add(&mut out, r);
                    }
                }
                if let Some(id) = l.environment {
                    out.push(id);
                }
            }
            Self::Environment { raw, declared } => {
                add(&mut out, raw);
                for r in declared {
                    add(&mut out, r);
                }
            }
            Self::Mutex { owner, .. } => {
                if let Some(r) = owner {
                    add(&mut out, r);
                }
            }
            Self::CondVar { mutex, .. } => add(&mut out, mutex),
            Self::Finalizer(r) => add(&mut out, r),
            _ => {}
        }
        out.sort_unstable();
        out.dedup();
        out
    }
}

#[derive(Serialize, Deserialize)]
pub(super) struct SymbolData {
    pub name: Vec<u32>,
    pub uninterned: bool,
    pub initial_obarray: bool,
    pub void_marker: bool,
    pub core_id: Option<SymId>,
    pub value: Ref,
    pub function: Ref,
    pub plist: Ref,
    pub flags: [bool; 7],
    pub documentation: Option<Vec<u32>>,
    pub alias: Option<Ref>,
}

#[derive(Serialize, Deserialize)]
pub(super) struct HashData {
    test: u8,
    contents: Vec<(Ref, Ref)>,
    weakness: Option<Ref>,
    size: String,
    rehash_size: Ref,
    rehash_threshold: Ref,
    test_name: Ref,
    mutable: bool,
}

#[derive(Serialize, Deserialize)]
pub(super) struct LambdaData {
    is_macro: bool,
    required: Vec<Ref>,
    optional: Vec<(Ref, Option<Ref>, Option<Ref>)>,
    rest: Option<Ref>,
    body: Vec<Ref>,
    environment: Option<u32>,
    doc: Option<Vec<u32>>,
    interactive: Option<Ref>,
    name: Option<Vec<u32>>,
    bad_arglist: bool,
    arglist: Option<Ref>,
    plain: bool,
    dumped_doc: bool,
    compiled: bool,
    advice_link: Option<usize>,
    bc_items: Option<Ref>,
    bc_closure: bool,
    doc_value: Option<Ref>,
    env_value: Option<Ref>,
}

enum Work {
    Value(Value),
    Symbol(SymId),
    Environment(Rc<LexFrame>),
}

pub(super) struct Encoder<'a> {
    interp: &'a Interp,
    ids: FxHashMap<(u8, usize), u32>,
    work: Vec<Work>,
    nodes: Vec<Option<Node>>,
    externals: FxHashMap<(u8, usize), String>,
    external_values: FxHashMap<String, Value>,
    subrs: Vec<&'static Subr>,
}

fn identity(value: &Value) -> Option<(u8, usize)> {
    macro_rules! rc {
        ($v:expr, $tag:expr) => {
            Some(($tag, Rc::as_ptr($v) as usize))
        };
    }
    match value {
        Value::Cons(v) => rc!(v, 1),
        Value::Str(v) => rc!(v, 2),
        Value::Float(v) => rc!(v, 3),
        Value::Vec(v) | Value::Record(v) => rc!(v, 4),
        Value::Hash(v) => rc!(v, 5),
        Value::Lambda(v) => rc!(v, 6),
        Value::Marker(v) => rc!(v, 7),
        Value::Mutex(v) => rc!(v, 8),
        Value::CondVar(v) => rc!(v, 9),
        Value::Finalizer(v) => rc!(v, 10),
        Value::Buffer(v) => rc!(v, 16),
        Value::Window(v) => rc!(v, 17),
        Value::Frame(v) => rc!(v, 18),
        Value::Process(v) => rc!(v, 19),
        Value::Thread(v) => rc!(v, 20),
        _ => None,
    }
}

impl<'a> Encoder<'a> {
    pub fn new(interp: &'a Interp) -> Self {
        Self {
            interp,
            ids: FxHashMap::default(),
            work: Vec::new(),
            nodes: Vec::new(),
            externals: FxHashMap::default(),
            external_values: FxHashMap::default(),
            subrs: crate::lisp::builtins::dump_subrs(),
        }
    }
    pub fn external(&mut self, value: &Value, name: String) -> Result<()> {
        let key = identity(value).ok_or("Non-object external reference")?;
        if let Some(old) = self.external_values.get(&name) {
            if identity(old) != Some(key) {
                return Err("Conflicting session object ids in dump".into());
            }
        }
        self.external_values.insert(name.clone(), value.clone());
        self.externals.insert(key, name);
        Ok(())
    }
    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    pub fn knows(&self, value: &Value) -> bool {
        identity(value).is_some_and(|key| self.ids.contains_key(&key))
    }
    pub fn external_objects(&self) -> Vec<(String, Value)> {
        self.external_values
            .iter()
            .map(|(name, value)| (name.clone(), value.clone()))
            .collect()
    }
    fn reserve(&mut self, key: (u8, usize), work: Work) -> Result<u32> {
        if let Some(id) = self.ids.get(&key) {
            return Ok(*id);
        }
        let id = u32::try_from(self.nodes.len()).map_err(|_| "Too many dump objects")?;
        self.ids.insert(key, id);
        self.nodes.push(None);
        self.work.push(work);
        Ok(id)
    }
    pub fn symbol(&mut self, id: SymId) -> Result<Ref> {
        Ok(Ref::Symbol(
            self.reserve((255, id as usize), Work::Symbol(id))?,
        ))
    }
    pub fn value(&mut self, value: &Value) -> Result<Ref> {
        match value {
            Value::Nil => Ok(Ref::Nil),
            Value::Int(n) => Ok(Ref::Int(n.to_string())),
            Value::Sym(id) => self.symbol(*id),
            Value::Subr(subr) => {
                if crate::lisp::builtins::comp::is_native(subr).is_some() {
                    return Err("Dumping native compilation units is not yet implemented".into());
                }
                let index = self
                    .subrs
                    .iter()
                    .position(|entry| std::ptr::eq(*entry, *subr))
                    .ok_or_else(|| {
                        format!("Cannot dump dynamically loaded primitive {}", subr.name)
                    })?;
                Ok(Ref::Subr(index as u32, subr.name.to_owned()))
            }
            _ => {
                let key = identity(value).ok_or("Unknown dump object")?;
                if !self.externals.contains_key(&key) {
                    let name = match value {
                        Value::Buffer(v) => Some(format!("buffer:{}", v.borrow().id)),
                        Value::Frame(v) => Some(format!("frame:{}", v.borrow().id)),
                        Value::Window(v) => Some(format!("window:{}", v.borrow().id)),
                        _ => None,
                    };
                    if let Some(name) = name {
                        self.external(value, name)?;
                    }
                }
                let id = self.reserve(key, Work::Value(value.clone()))?;
                Ok(if matches!(value, Value::Record(_)) {
                    Ref::Record(id)
                } else {
                    Ref::Object(id)
                })
            }
        }
    }
    pub fn values(&mut self, values: &[Value]) -> Result<Vec<Ref>> {
        values.iter().map(|value| self.value(value)).collect()
    }
    fn optional(&mut self, value: &Option<Value>) -> Result<Option<Ref>> {
        value.as_ref().map(|value| self.value(value)).transpose()
    }
    pub fn drain(&mut self) -> Result<()> {
        // A work list also handles very long cons spines without Rust recursion.
        while let Some(work) = self.work.pop() {
            let (key, node) = match work {
                Work::Symbol(id) => {
                    let symbol = self.interp.obarray.symbol(id);
                    let data = SymbolData {
                        name: encode_text(&symbol.name),
                        uninterned: symbol.uninterned,
                        initial_obarray: self.interp.obarray.is_initial_symbol(id),
                        void_marker: id == sym::UNBOUND,
                        core_id: self.interp.obarray.is_core_symbol(id).then_some(id),
                        value: self.value(&symbol.value)?,
                        function: self.value(&symbol.function)?,
                        plist: self.value(&symbol.plist)?,
                        flags: [
                            symbol.special,
                            symbol.constant,
                            symbol.make_local_if_set,
                            symbol.always_local,
                            symbol.builtin_variable,
                            symbol.forwarded_variable,
                            symbol.uninterned,
                        ],
                        documentation: symbol.variable_documentation.as_deref().map(encode_text),
                        alias: symbol.var_alias.map(|id| self.symbol(id)).transpose()?,
                    };
                    ((255, id as usize), Node::Symbol(Box::new(data)))
                }
                Work::Environment(env) => {
                    let raw = self.value(&env.raw.borrow())?;
                    let declared = env
                        .declared
                        .borrow()
                        .iter()
                        .map(|id| self.symbol(*id))
                        .collect::<Result<_>>()?;
                    (
                        (254, Rc::as_ptr(&env) as usize),
                        Node::Environment { raw, declared },
                    )
                }
                Work::Value(value) => {
                    let key = identity(&value).ok_or("Invalid graph work item")?;
                    let node = if let Some(name) = self.externals.get(&key) {
                        Node::External(name.clone())
                    } else {
                        self.object(&value)?
                    };
                    (key, node)
                }
            };
            self.nodes[self.ids[&key] as usize] = Some(node);
        }
        Ok(())
    }
    pub fn finish(mut self) -> Result<Vec<Node>> {
        self.drain()?;
        self.nodes
            .into_iter()
            .map(|node| node.ok_or_else(|| "Incomplete dump graph".into()))
            .collect()
    }
    fn object(&mut self, value: &Value) -> Result<Node> {
        Ok(match value {
            Value::Cons(cell) => {
                let cell = cell.borrow();
                Node::Cons(self.value(&cell.car)?, self.value(&cell.cdr)?)
            }
            Value::Float(value) => Node::Float(value.to_bits()),
            Value::Vec(values) | Value::Record(values) => {
                let overlay = self.interp.is_overlay(&Value::Vec(values.clone()));
                let values = values.borrow();
                if let Some(Value::Sym(tag)) = values.first() {
                    if matches!(
                        self.interp.symbol_name(*tag).as_str(),
                        "treesit-parser"
                            | "treesit-node"
                            | "treesit-compiled-query"
                            | "sqlite"
                            | "native-comp-unit"
                    ) {
                        return Err("Cannot dump external runtime handle objects".into());
                    }
                }
                let items = self.values(&values)?;
                if overlay {
                    Node::OverlayVector(items)
                } else {
                    Node::Vector(items)
                }
            }
            Value::Str(string) => {
                let representation = if self.interp.is_unibyte_str(string) {
                    1
                } else if self.interp.is_multibyte_str(string) {
                    2
                } else {
                    0
                };
                let properties = if self.interp.has_str_props(string) {
                    Some(
                        self.interp
                            .str_props(string)
                            .iter()
                            .map(|(a, b, plist)| Ok((*a, *b, self.values(plist)?)))
                            .collect::<Result<_>>()?,
                    )
                } else {
                    None
                };
                Node::String {
                    text: encode_text(&string.borrow()),
                    representation,
                    properties,
                }
            }
            Value::Hash(table) => {
                let table = table.borrow();
                if table.user_test.is_some() {
                    return Err("cannot dump hash tables with user-defined tests".into());
                }
                let mut contents = Vec::with_capacity(table.map.len());
                for (key, original) in table.keys.iter() {
                    let value = table
                        .map
                        .get(key)
                        .ok_or("Inconsistent hash-table slots in dump")?;
                    contents.push((self.value(original)?, self.value(value)?));
                }
                Node::Hash(Box::new(HashData {
                    test: match table.test {
                        HashTest::Eq => 0,
                        HashTest::Eql => 1,
                        HashTest::Equal => 2,
                    },
                    contents,
                    weakness: self.optional(&table.weakness)?,
                    size: table.size.to_string(),
                    rehash_size: self.value(&table.rehash_size)?,
                    rehash_threshold: self.value(&table.rehash_threshold)?,
                    test_name: self.value(&table.test_name)?,
                    mutable: table.mutable,
                }))
            }
            Value::Lambda(lambda) => Node::Lambda(Box::new(LambdaData {
                is_macro: lambda.is_macro,
                required: lambda
                    .required
                    .iter()
                    .map(|id| self.symbol(*id))
                    .collect::<Result<_>>()?,
                optional: lambda
                    .optional
                    .iter()
                    .map(|p| {
                        Ok((
                            self.symbol(p.sym)?,
                            self.optional(&p.default)?,
                            p.supplied.map(|id| self.symbol(id)).transpose()?,
                        ))
                    })
                    .collect::<Result<_>>()?,
                rest: lambda.rest.map(|id| self.symbol(id)).transpose()?,
                body: self.values(&lambda.body)?,
                environment: lambda
                    .env
                    .as_ref()
                    .map(|env| {
                        self.reserve(
                            (254, Rc::as_ptr(env) as usize),
                            Work::Environment(env.clone()),
                        )
                    })
                    .transpose()?,
                doc: lambda.doc.as_deref().map(encode_text),
                interactive: self.optional(&lambda.interactive)?,
                name: lambda.name.as_deref().map(encode_text),
                bad_arglist: lambda.bad_arglist,
                arglist: self.optional(&lambda.arglist)?,
                plain: lambda.plain,
                dumped_doc: lambda.dumped_doc,
                compiled: lambda.compiled,
                advice_link: lambda.advice_link,
                bc_items: lambda
                    .bc_items
                    .as_ref()
                    .map(|v| self.value(&Value::Vec(v.clone())))
                    .transpose()?,
                bc_closure: lambda.bc_closure,
                doc_value: self.optional(&lambda.doc_value)?,
                env_value: self.optional(&lambda.env_value)?,
            })),
            Value::Marker(marker) => {
                let m = marker.borrow();
                Node::Marker {
                    buffer: m.buffer,
                    position: m.position,
                    insertion_type: m.insertion_type,
                }
            }
            Value::Mutex(mutex) => {
                let m = mutex.borrow();
                Node::Mutex {
                    name: m.name.as_deref().map(encode_text),
                    owner: self.optional(&m.owner)?,
                }
            }
            Value::CondVar(condvar) => {
                let c = condvar.borrow();
                Node::CondVar {
                    name: c.name.as_deref().map(encode_text),
                    mutex: self.value(&c.mutex)?,
                }
            }
            Value::Finalizer(function) => Node::Finalizer(self.value(&function.borrow())?),
            Value::Buffer(_)
            | Value::Frame(_)
            | Value::Window(_)
            | Value::Thread(_)
            | Value::Process(_) => return Err("Unregistered session object in dump graph".into()),
            _ => return Err("Invalid heap object in dump graph".into()),
        })
    }
}

pub(super) struct Decoder<'a> {
    nodes: &'a [Node],
    objects: Vec<Option<Value>>,
    symbols: Vec<Option<SymId>>,
    environments: FxHashMap<u32, Rc<LexFrame>>,
    building: Vec<bool>,
    subrs: Vec<&'static Subr>,
}

impl<'a> Decoder<'a> {
    pub fn new(
        i: &mut Interp,
        nodes: &'a [Node],
        externals: &FxHashMap<String, Value>,
    ) -> Result<Self> {
        let subrs = crate::lisp::builtins::dump_subrs();
        let mut decoder = Self {
            nodes,
            objects: vec![None; nodes.len()],
            symbols: vec![None; nodes.len()],
            environments: FxHashMap::default(),
            building: vec![false; nodes.len()],
            subrs,
        };
        // Detach an uninterned built-in before allocating a different
        // interned symbol with its old name. Core reader references retain
        // their fixed ids while ordinary symbol references are rebased.
        for node in nodes {
            if let Node::Symbol(data) = node {
                if let Some(id) = data.core_id {
                    i.obarray.prepare_dump_core(id, !data.initial_obarray)?;
                }
            }
        }
        for (index, node) in nodes.iter().enumerate() {
            decoder.objects[index] = match node {
                Node::Symbol(data) => {
                    let name = decode_text(&data.name);
                    let id = if let Some(id) = data.core_id {
                        if !i.obarray.is_core_symbol(id) || i.symbol_name(id) != name {
                            return Err("Invalid core symbol in portable dump".into());
                        }
                        id
                    } else if data.void_marker {
                        sym::UNBOUND
                    } else if !data.initial_obarray {
                        i.make_symbol(&name)
                    } else {
                        i.intern(&name)
                    };
                    decoder.symbols[index] = Some(id);
                    Some(i.sym(id))
                }
                Node::String { text, .. } => Some(Value::string(decode_text(text))),
                Node::Float(bits) => Some(Value::float(f64::from_bits(*bits))),
                Node::Cons(_, _) => Some(Value::cons(Value::Nil, Value::Nil)),
                Node::Vector(items) | Node::OverlayVector(items) => {
                    Some(Value::vector(vec![Value::Nil; items.len()]))
                }
                Node::Hash(data) => {
                    let test = match data.test {
                        0 => HashTest::Eq,
                        1 => HashTest::Eql,
                        2 => HashTest::Equal,
                        _ => return Err("Invalid dumped hash test".into()),
                    };
                    Some(Value::Hash(memory::allocate(
                        LispHash::new(test),
                        Value::Hash,
                    )))
                }
                Node::Environment { .. } => {
                    decoder.environments.insert(
                        index as u32,
                        Rc::new(LexFrame {
                            raw: RefCell::new(Value::Nil),
                            declared: RefCell::new(Vec::new()),
                        }),
                    );
                    None
                }
                Node::Lambda(_) => None,
                Node::Marker {
                    buffer,
                    position,
                    insertion_type,
                } => Some(Value::Marker(memory::allocate(
                    Marker {
                        buffer: *buffer,
                        position: *position,
                        insertion_type: *insertion_type,
                    },
                    Value::Marker,
                ))),
                Node::Mutex { name, .. } => Some(Value::Mutex(memory::allocate(
                    Mutex {
                        name: name.as_deref().map(decode_text),
                        owner: None,
                    },
                    Value::Mutex,
                ))),
                Node::CondVar { name, .. } => Some(Value::CondVar(memory::allocate(
                    CondVar {
                        name: name.as_deref().map(decode_text),
                        mutex: Value::Nil,
                    },
                    Value::CondVar,
                ))),
                Node::Finalizer(_) => Some(Value::Finalizer(memory::allocate(
                    Value::Nil,
                    Value::Finalizer,
                ))),
                Node::External(name) => Some(
                    externals
                        .get(name)
                        .ok_or_else(|| format!("Missing dump external {name}"))?
                        .clone(),
                ),
            };
        }
        // Immutable closures can refer to mutable placeholders, but must
        // themselves be fully built before a mutable slot points at them.
        let mut dependencies = vec![0usize; nodes.len()];
        let mut users = vec![Vec::new(); nodes.len()];
        let mut ready = std::collections::VecDeque::new();
        let mut closures = 0usize;
        for (index, node) in nodes.iter().enumerate() {
            if let Node::Lambda(data) = node {
                closures += 1;
                let references = data
                    .body
                    .iter()
                    .chain(
                        data.optional
                            .iter()
                            .filter_map(|(_, value, _)| value.as_ref()),
                    )
                    .chain(
                        [
                            &data.interactive,
                            &data.arglist,
                            &data.bc_items,
                            &data.doc_value,
                            &data.env_value,
                        ]
                        .into_iter()
                        .filter_map(Option::as_ref),
                    );
                let mut seen = rustc_hash::FxHashSet::default();
                for reference in references {
                    if let Ref::Object(id) | Ref::Record(id) = reference {
                        let target = *id as usize;
                        if matches!(nodes.get(target), Some(Node::Lambda(_))) && seen.insert(target)
                        {
                            dependencies[index] += 1;
                            users[target].push(index);
                        }
                    }
                }
                if dependencies[index] == 0 {
                    ready.push_back(index);
                }
            }
        }
        while let Some(index) = ready.pop_front() {
            decoder.object(i, index as u32)?;
            closures -= 1;
            for user in &users[index] {
                dependencies[*user] -= 1;
                if dependencies[*user] == 0 {
                    ready.push_back(*user);
                }
            }
        }
        if closures != 0 {
            return Err("Invalid direct closure cycle in portable dump".into());
        }
        decoder.fill_mutable(i)?;
        Ok(decoder)
    }

    pub fn value(&mut self, i: &mut Interp, reference: &Ref) -> Result<Value> {
        match reference {
            Ref::Nil => Ok(Value::Nil),
            Ref::Int(n) => Ok(Value::Int(n.parse().map_err(|_| "Invalid dumped integer")?)),
            Ref::Symbol(index) => {
                let id = self
                    .symbols
                    .get(*index as usize)
                    .and_then(|id| *id)
                    .ok_or("Invalid dumped symbol reference")?;
                Ok(i.sym(id))
            }
            Ref::Subr(index, name) => {
                let subr = self
                    .subrs
                    .get(*index as usize)
                    .filter(|subr| subr.name == name)
                    .ok_or_else(|| format!("Unknown dumped primitive {name}"))?;
                Ok(Value::Subr(*subr))
            }
            Ref::Object(index) => self.object(i, *index),
            Ref::Record(index) => match self.object(i, *index)? {
                Value::Vec(vector) => Ok(Value::Record(vector)),
                _ => Err("Invalid dumped record backing store".into()),
            },
        }
    }
    pub fn values(&mut self, i: &mut Interp, references: &[Ref]) -> Result<Vec<Value>> {
        references.iter().map(|r| self.value(i, r)).collect()
    }
    pub fn symbol(&self, reference: &Ref) -> Result<SymId> {
        match reference {
            Ref::Nil => Ok(sym::NIL),
            Ref::Symbol(index) => self
                .symbols
                .get(*index as usize)
                .and_then(|id| *id)
                .ok_or_else(|| "Invalid dumped symbol id".into()),
            _ => Err("Expected a dumped symbol".into()),
        }
    }
    pub fn symbol_ids(&self) -> Vec<SymId> {
        self.symbols.iter().flatten().copied().collect()
    }
    fn optional(&mut self, i: &mut Interp, reference: &Option<Ref>) -> Result<Option<Value>> {
        reference.as_ref().map(|r| self.value(i, r)).transpose()
    }
    fn object(&mut self, i: &mut Interp, index: u32) -> Result<Value> {
        let index = index as usize;
        let slot = self
            .objects
            .get(index)
            .ok_or("Dump object id is out of range")?;
        if let Some(value) = slot {
            return Ok(value.clone());
        }
        let Node::Lambda(data) = self.nodes.get(index).ok_or("Invalid dump node")? else {
            return Err("A lexical environment was used as a Lisp value".into());
        };
        if self.building[index] {
            return Err("Invalid direct closure cycle in portable dump".into());
        }
        self.building[index] = true;
        let lambda = Lambda {
            is_macro: data.is_macro,
            required: data
                .required
                .iter()
                .map(|r| self.symbol(r))
                .collect::<Result<_>>()?,
            optional: data
                .optional
                .iter()
                .map(|(name, default, supplied)| {
                    Ok(OptParam {
                        sym: self.symbol(name)?,
                        default: self.optional(i, default)?,
                        supplied: supplied.as_ref().map(|r| self.symbol(r)).transpose()?,
                    })
                })
                .collect::<Result<_>>()?,
            rest: data.rest.as_ref().map(|r| self.symbol(r)).transpose()?,
            body: self.values(i, &data.body)?,
            env: data
                .environment
                .map(|id| {
                    self.environments
                        .get(&id)
                        .cloned()
                        .ok_or_else(|| "Invalid dumped environment id".to_owned())
                })
                .transpose()?,
            doc: data.doc.as_deref().map(decode_text),
            interactive: self.optional(i, &data.interactive)?,
            name: data.name.as_deref().map(decode_text),
            bad_arglist: data.bad_arglist,
            arglist: self.optional(i, &data.arglist)?,
            plain: data.plain,
            dumped_doc: data.dumped_doc,
            compiled: data.compiled,
            advice_link: data.advice_link,
            bc_items: data
                .bc_items
                .as_ref()
                .map(|r| match self.value(i, r)? {
                    Value::Vec(vector) => Ok(vector),
                    _ => Err("Invalid dumped byte-code slots".to_owned()),
                })
                .transpose()?,
            bc_closure: data.bc_closure,
            bc_prog: RefCell::new(None),
            doc_value: self.optional(i, &data.doc_value)?,
            env_value: self.optional(i, &data.env_value)?,
        };
        let value = Value::Lambda(Rc::new(lambda)).tracked();
        self.building[index] = false;
        self.objects[index] = Some(value.clone());
        Ok(value)
    }
    fn fill_mutable(&mut self, i: &mut Interp) -> Result<()> {
        for (index, node) in self.nodes.iter().enumerate() {
            let object = self.objects[index].clone();
            match (node, object) {
                (Node::Cons(car, cdr), Some(Value::Cons(cell))) => {
                    let car = self.value(i, car)?;
                    let cdr = self.value(i, cdr)?;
                    let mut cell = cell.borrow_mut();
                    cell.car = car;
                    cell.cdr = cdr;
                }
                (Node::Vector(items) | Node::OverlayVector(items), Some(Value::Vec(vector))) => {
                    *vector.borrow_mut() = self.values(i, items)?;
                    if matches!(node, Node::OverlayVector(_)) {
                        i.register_overlay(&Value::Vec(vector));
                    }
                }
                (
                    Node::String {
                        representation,
                        properties,
                        ..
                    },
                    Some(Value::Str(string)),
                ) => {
                    match representation {
                        0 => {}
                        1 => i.mark_unibyte(&string),
                        2 => i.mark_multibyte(&string),
                        _ => return Err("Invalid dumped string representation".into()),
                    }
                    if let Some(properties) = properties {
                        let properties = properties
                            .iter()
                            .map(|(a, b, plist)| Ok((*a, *b, self.values(i, plist)?)))
                            .collect::<Result<_>>()?;
                        i.set_str_props(&string, properties);
                    }
                }
                (Node::Environment { raw, declared }, _) => {
                    let raw = self.value(i, raw)?;
                    let declared = declared
                        .iter()
                        .map(|r| self.symbol(r))
                        .collect::<Result<_>>()?;
                    let env = &self.environments[&(index as u32)];
                    *env.raw.borrow_mut() = raw;
                    *env.declared.borrow_mut() = declared;
                }
                (Node::Mutex { owner, .. }, Some(Value::Mutex(mutex))) => {
                    mutex.borrow_mut().owner = self.optional(i, owner)?;
                }
                (Node::CondVar { mutex, .. }, Some(Value::CondVar(condvar))) => {
                    condvar.borrow_mut().mutex = self.value(i, mutex)?;
                }
                (Node::Finalizer(function), Some(Value::Finalizer(finalizer))) => {
                    *finalizer.borrow_mut() = self.value(i, function)?;
                }
                _ => {}
            }
        }
        Ok(())
    }
    /// Rehash only after all referents and session-side char-table metadata
    /// have been restored. Object addresses deliberately differ after load.
    pub fn finish(&mut self, i: &mut Interp) -> Result<()> {
        for (index, node) in self.nodes.iter().enumerate() {
            if let Node::Hash(data) = node {
                let Some(Value::Hash(table)) = self.objects[index].clone() else {
                    return Err("Invalid dumped hash object".into());
                };
                {
                    let mut table = table.borrow_mut();
                    table.size = data.size.parse().map_err(|_| "Invalid dumped hash size")?;
                    table.weakness = self.optional(i, &data.weakness)?;
                    table.rehash_size = self.value(i, &data.rehash_size)?;
                    table.rehash_threshold = self.value(i, &data.rehash_threshold)?;
                    table.test_name = self.value(i, &data.test_name)?;
                }
                for (key, value) in &data.contents {
                    let key = self.value(i, key)?;
                    let value = self.value(i, value)?;
                    let normalized = crate::lisp::builtins::hashfn::table_key(i, &table, &key)
                        .map_err(|_| "Cannot restore a dumped hash key")?;
                    let mut table = table.borrow_mut();
                    table.map.insert(normalized.clone(), value);
                    table.put_key(normalized, key);
                }
                table.borrow_mut().mutable = data.mutable;
            }
        }
        for (index, node) in self.nodes.iter().enumerate() {
            if let Node::Symbol(data) = node {
                let id = self.symbols[index].ok_or("Missing dumped symbol")?;
                let value = self.value(i, &data.value)?;
                let function = self.value(i, &data.function)?;
                let plist = self.value(i, &data.plist)?;
                let alias = data.alias.as_ref().map(|r| self.symbol(r)).transpose()?;
                let symbol = i.obarray.symbol_mut(id);
                symbol.value = value;
                symbol.function = function;
                symbol.plist = plist;
                [
                    symbol.special,
                    symbol.constant,
                    symbol.make_local_if_set,
                    symbol.always_local,
                    symbol.builtin_variable,
                    symbol.forwarded_variable,
                    symbol.uninterned,
                ] = data.flags;
                symbol.variable_documentation = data.documentation.as_deref().map(decode_text);
                symbol.var_alias = alias;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn on_stack(test: impl FnOnce() + Send + 'static) {
        std::thread::Builder::new()
            .stack_size(64 << 20)
            .spawn(test)
            .unwrap()
            .join()
            .unwrap();
    }

    fn roundtrip(i: &mut Interp, value: &Value) -> Value {
        let mut encoder = Encoder::new(i);
        let root = encoder.value(value).unwrap();
        let nodes = encoder.finish().unwrap();
        let encoded = serde_json::to_vec(&(root, nodes)).unwrap();
        let (root, nodes): (Ref, Vec<Node>) = serde_json::from_slice(&encoded).unwrap();
        let mut decoder = Decoder::new(i, &nodes, &FxHashMap::default()).unwrap();
        decoder.finish(i).unwrap();
        decoder.value(i, &root).unwrap()
    }

    #[test]
    fn graph_roundtrip_keeps_cycles_aliases_float_keys_and_large_integers() {
        on_stack(|| {
            let mut i = Interp::new();
            let cell = Value::cons(Value::Int(7), Value::Nil);
            if let Value::Cons(cons) = &cell {
                cons.borrow_mut().cdr = cell.clone();
            }
            let float = Value::float(f64::from_bits(0x7ff8_0000_0000_0001));
            let hash = memory::allocate(LispHash::new(HashTest::Eq), Value::Hash);
            let key = crate::lisp::builtins::hashfn::hash_key_for(&i, &float, HashTest::Eq);
            hash.borrow_mut().map.insert(key.clone(), cell.clone());
            hash.borrow_mut().put_key(key, float.clone());
            let value = Value::vector(vec![
                cell.clone(),
                cell,
                float.clone(),
                float,
                Value::Hash(hash),
                Value::Int(i128::MAX),
            ]);
            let Value::Vec(vector) = roundtrip(&mut i, &value) else {
                panic!("vector");
            };
            let vector = vector.borrow();
            assert!(crate::lisp::builtins::eq_values(&vector[0], &vector[1]));
            let Value::Cons(cell) = &vector[0] else {
                panic!("cons");
            };
            assert!(crate::lisp::builtins::eq_values(
                &cell.borrow().cdr,
                &vector[0]
            ));
            assert!(crate::lisp::builtins::eq_values(&vector[2], &vector[3]));
            let Value::Float(float) = &vector[2] else {
                panic!("float");
            };
            assert_eq!(float.to_bits(), 0x7ff8_0000_0000_0001);
            let Value::Hash(hash) = &vector[4] else {
                panic!("hash");
            };
            let key = crate::lisp::builtins::hashfn::hash_key_for(&i, &vector[2], HashTest::Eq);
            assert!(crate::lisp::builtins::eq_values(
                &hash.borrow().map[&key],
                &vector[0]
            ));
            assert!(matches!(vector[5], Value::Int(i128::MAX)));
        });
    }

    #[test]
    fn graph_roundtrip_keeps_string_properties_and_record_backing_aliases() {
        on_stack(|| {
            let mut i = Interp::new();
            let string = Value::string_ref("abc");
            i.mark_unibyte(&string);
            let property = i.make_symbol("dump-property");
            i.set_str_props(&string, vec![(0, 3, vec![i.sym(property), Value::Int(42)])]);
            let shared = Value::vector_ref(vec![Value::Str(string)]);
            let value = Value::vector(vec![Value::Vec(shared.clone()), Value::Record(shared)]);
            let Value::Vec(result) = roundtrip(&mut i, &value) else {
                panic!("vector");
            };
            let result = result.borrow();
            let (Value::Vec(vector), Value::Record(record)) = (&result[0], &result[1]) else {
                panic!("backing views");
            };
            assert!(Rc::ptr_eq(vector, record));
            let Value::Str(string) = &vector.borrow()[0] else {
                panic!("string");
            };
            assert!(i.is_unibyte_str(string));
            assert_eq!(i.str_props(string).len(), 1);
            assert!(matches!(i.str_props(string)[0].2[1], Value::Int(42)));
        });
    }

    #[test]
    fn graph_roundtrip_keeps_shared_lexical_binding_cells() {
        on_stack(|| {
            let mut i = Interp::new();
            let name = i.make_symbol("dump-captured");
            let binding = Value::cons(i.sym(name), Value::Int(7));
            let env = Rc::new(LexFrame {
                raw: RefCell::new(Value::list(vec![binding.clone()])),
                declared: RefCell::new(vec![]),
            });
            let mut first = i.parse_lambda(&Value::Nil, &[i.sym(name)], None).unwrap();
            first.env = Some(env.clone());
            let mut second = i.parse_lambda(&Value::Nil, &[i.sym(name)], None).unwrap();
            second.env = Some(env);
            let value = Value::vector(vec![
                Value::Lambda(Rc::new(first)).tracked(),
                Value::Lambda(Rc::new(second)).tracked(),
                binding,
            ]);
            let Value::Vec(result) = roundtrip(&mut i, &value) else {
                panic!("vector");
            };
            let result = result.borrow();
            let (Value::Lambda(first), Value::Lambda(second), Value::Cons(binding)) =
                (&result[0], &result[1], &result[2])
            else {
                panic!("closures");
            };
            assert!(Rc::ptr_eq(
                first.env.as_ref().unwrap(),
                second.env.as_ref().unwrap()
            ));
            binding.borrow_mut().cdr = Value::Int(91);
            assert!(matches!(
                i.apply(&result[0], vec![]).unwrap(),
                Value::Int(91)
            ));
            assert!(matches!(
                i.apply(&result[1], vec![]).unwrap(),
                Value::Int(91)
            ));
        });
    }

    #[test]
    fn invalid_graph_references_are_errors() {
        on_stack(|| {
            let mut i = Interp::new();
            let mut decoder = Decoder::new(&mut i, &[], &FxHashMap::default()).unwrap();
            assert!(decoder.value(&mut i, &Ref::Object(4)).is_err());
            assert!(decoder.value(&mut i, &Ref::Symbol(4)).is_err());
            assert!(
                decoder
                    .value(&mut i, &Ref::Subr(u32::MAX, "not-a-primitive".into()))
                    .is_err()
            );
        });
    }
}
