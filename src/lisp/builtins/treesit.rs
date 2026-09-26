//! Tree-sitter primitives (`treesit-*`), a port of GNU `src/treesit.c`.
//!
//! Language grammars are loaded dynamically (`libtree-sitter-LANG.so'/
//! `.dylib') exactly like GNU: `treesit-extra-load-path', then
//! `tree-sitter/' under `user-emacs-directory', then bare `dlopen'.
//!
//! Lisp objects are represented as records: `[treesit-parser ID]',
//! `[treesit-node PARSER-ID NODE-ID]', `[treesit-compiled-query ID]'.
//! That gives `type-of' the right symbol, `eq' object identity, and
//! `equal' node identity (same parser + same tree-sitter node id)
//! without new `Value' variants.  Rust objects live in `TsState'
//! (an `Interp' field), addressed by the integer ids in the records.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use tree_sitter::{ffi, Language, Node, Parser, Point, Range, Tree, TreeCursor};
use tree_sitter_language::LanguageFn;

use super::{arg, want_int, want_string, want_sym, S};
use crate::lisp::error::{EvalResult, Flow};
use crate::lisp::obarray::sym;
use crate::lisp::value::{BufferRef, SymId, Value};

/// GNU `TREESIT_RECURSION_LIMIT'.
const RECURSION_LIMIT: usize = 1000;

// ---------------------------------------------------------------------------
// State
// ---------------------------------------------------------------------------

/// A loaded grammar shared library (kept alive so its `Language' stays
/// valid) plus the constructed `tree_sitter::Language`.
struct LoadedLang {
    _lib: libloading::Library,
    language: Language,
    raw: *const ffi::TSLanguage,
    path: String,
}

pub struct TsParser {
    parser: Parser,
    tree: Option<Tree>,
    /// The buffer this parser parses (base buffer for indirect buffers).
    buffer: BufferRef,
    /// The `Value::Buffer' originally handed to Lisp (identity matters
    /// for `treesit-parser-list' filtering).
    buffer_val: Value,
    /// Value handed back to Lisp (one record per parser).
    self_val: Value,
    language: SymId,
    tag: Value,
    embed_level: Value,
    /// Ranges as last set by `treesit-parser-set-included-ranges'
    /// (charpos `(BEG . END)' list); nil = parse whole visible region.
    ranges: Value,
    notifiers: Vec<Value>,
    deleted: bool,
    need_reparse: bool,
    /// Incremented on each reparse; nodes carry the count they were
    /// created under — a mismatch makes them `outdated'.
    parse_count: u64,
    buf_tick: u64,
    begv: usize,
    zv: usize,
    /// Line/column tracking enabled for this parser's language.
    tracking_linecol: bool,
}

struct TsNodeObj {
    node: Node<'static>,
    parser_id: u64,
    parse_count: u64,
}

struct TsQueryObj {
    source: Value,
    language: SymId,
    /// Raw compiled query.  Freed on drop like GNU's GC finalizer.
    query: Option<*mut ffi::TSQuery>,
}

impl Drop for TsQueryObj {
    fn drop(&mut self) {
        if let Some(q) = self.query.take() {
            unsafe { ffi::ts_query_delete(q) }
        }
    }
}

pub struct TsState {
    next_id: u64,
    parsers: HashMap<u64, TsParser>,
    /// Parser ids in creation order (`treesit-parser-list' order).
    parser_order: Vec<u64>,
    /// Node objects keyed by (parser id, `ts_node.id()`).
    nodes: HashMap<(u64, usize), TsNodeObj>,
    queries: HashMap<u64, TsQueryObj>,
    languages: HashMap<SymId, LoadedLang>,
    /// Global line-column tracking flag (`treesit-tracking-line-column-p').
    pub tracking_linecol: bool,
    /// Per-buffer linecol cache: buffer id -> (line, col, bytepos).
    linecol_caches: HashMap<usize, (i64, i64, i64)>,
}

impl TsState {
    pub fn new() -> Self {
        TsState {
            next_id: 1,
            parsers: HashMap::new(),
            parser_order: Vec::new(),
            nodes: HashMap::new(),
            queries: HashMap::new(),
            languages: HashMap::new(),
            tracking_linecol: false,
            linecol_caches: HashMap::new(),
        }
    }
}

// ---------------------------------------------------------------------------
// Symbol / object plumbing
// ---------------------------------------------------------------------------

fn sname(i: &mut Interp, name: &str) -> SymId {
    i.intern(name)
}

fn sig(i: &mut Interp, name: &str, data: Vec<Value>) -> Flow {
    let s = sname(i, name);
    i.signal_data(s, data)
}

/// Record `[TAG . REST]' constructor.
fn tag_record(tag: SymId, rest: Vec<Value>) -> Value {
    let mut v = Vec::with_capacity(rest.len() + 1);
    v.push(Value::Sym(tag));
    v.extend(rest);
    Value::Record(Rc::new(RefCell::new(v)))
}

/// If `v' is a tagged record `[TAG ...]', return the rest of the slots.
fn tagged(i: &mut Interp, v: &Value, tag_name: &str) -> Option<Vec<Value>> {
    let tag = i.intern_soft(tag_name)?;
    if let Value::Record(r) = v {
        let rr = r.borrow();
        if let Some(Value::Sym(s)) = rr.first() {
            if *s == tag {
                return Some(rr[1..].to_vec());
            }
        }
    }
    None
}

fn want_parser(i: &mut Interp, v: &Value) -> Result<u64, Flow> {
    if let Some(rest) = tagged(i, v, "treesit-parser") {
        if let Some(Value::Int(id)) = rest.first() {
            let id = *id as u64;
            if let Some(p) = i.treesit.parsers.get(&id) {
                if p.deleted {
                    return Err(sig(i, "treesit-parser-deleted", vec![v.clone()]));
                }
                return Ok(id);
            }
        }
    }
    Err(i.wrong_type_mut("treesit-parser-p", v))
}

fn node_entry(i: &mut Interp, v: &Value) -> Result<(u64, usize), Flow> {
    if let Some(rest) = tagged(i, v, "treesit-node") {
        if rest.len() == 2 {
            if let (Value::Int(p), Value::Int(n)) = (rest[0].clone(), rest[1].clone()) {
                let key = (p as u64, n as usize);
                if i.treesit.nodes.contains_key(&key) {
                    return Ok(key);
                }
            }
        }
    }
    Err(i.wrong_type_mut("treesit-node-p", v))
}

fn node_entry_soft(i: &Interp, v: &Value) -> Option<(u64, usize)> {
    let tag = i.intern_soft("treesit-node")?;
    if let Value::Record(r) = v {
        let rr = r.borrow();
        if let (Some(Value::Sym(s)), Some(Value::Int(p)), Some(Value::Int(n))) =
            (rr.first(), rr.get(1), rr.get(2))
        {
            if *s == tag {
                let key = (*p as u64, *n as usize);
                if i.treesit.nodes.contains_key(&key) {
                    return Some(key);
                }
            }
        }
    }
    None
}

/// `treesit_check_node': object exists, is up-to-date, buffer alive.
fn check_node(i: &mut Interp, v: &Value) -> Result<(u64, usize), Flow> {
    let key = node_entry(i, v)?;
    let entry = &i.treesit.nodes[&key];
    let p = &i.treesit.parsers[&entry.parser_id];
    if entry.parse_count != p.parse_count {
        return Err(sig(i, "treesit-node-outdated", vec![v.clone()]));
    }
    if !p.buffer.borrow().live {
        return Err(sig(i, "treesit-node-buffer-killed", vec![v.clone()]));
    }
    Ok(key)
}

fn is_node_obj(i: &mut Interp, v: &Value) -> bool {
    node_entry(i, v).is_ok()
}

fn want_query(i: &mut Interp, v: &Value) -> Result<u64, Flow> {
    if let Some(rest) = tagged(i, v, "treesit-compiled-query") {
        if let Some(Value::Int(id)) = rest.first() {
            let id = *id as u64;
            if i.treesit.queries.contains_key(&id) {
                return Ok(id);
            }
        }
    }
    Err(i.wrong_type_mut("treesit-compiled-query-p", v))
}

fn is_query_obj(i: &mut Interp, v: &Value) -> bool {
    want_query(i, v).is_ok()
}

fn parser_live(i: &Interp, id: u64) -> bool {
    i.treesit
        .parsers
        .get(&id)
        .map(|p| !p.deleted && p.buffer.borrow().live)
        .unwrap_or(false)
}

/// Wrap `node` in a fresh `[treesit-node ...]' record, registering the
/// node under (parser_id, node.id()).
fn make_node(i: &mut Interp, parser_id: u64, node: Node) -> Value {
    let nid = node.id();
    let pc = i.treesit.parsers[&parser_id].parse_count;
    let node_static: Node<'static> = unsafe { std::mem::transmute(node) };
    i.treesit.nodes.insert(
        (parser_id, nid),
        TsNodeObj {
            node: node_static,
            parser_id,
            parse_count: pc,
        },
    );
    let tag = sname(i, "treesit-node");
    tag_record(
        tag,
        vec![Value::Int(parser_id as i128), Value::Int(nid as i128)],
    )
}

fn ts_node_of<'a>(i: &'a Interp, key: (u64, usize)) -> &'a Node<'static> {
    &i.treesit.nodes[&key].node
}

/// Root node of the parser's *stored* tree, detached to `Node<'static>`.
///
/// Never derive nodes from `Tree::clone()`: it is `ts_tree_copy`, which
/// builds a *new* `TSTree` (the root subtree is stored inline so even its
/// id differs, and `tree` points at the copy), so nodes taken from a clone
/// dangle once the clone drops.  `into_raw` detaches the node from the
/// `&Tree` borrow; the parser's `tree` slot keeps the memory alive and
/// `check_node` enforces freshness via `parse_count`.
fn stored_root(i: &Interp, pid: u64) -> Node<'static> {
    let raw = i.treesit.parsers[&pid]
        .tree
        .as_ref()
        .expect("parsed")
        .root_node()
        .into_raw();
    unsafe { Node::from_raw(raw) }
}

// ---------------------------------------------------------------------------
// Buffer / position helpers
// ---------------------------------------------------------------------------

/// `fix_position': accept integer or marker, return 1-based charpos.
fn fix_position(i: &mut Interp, v: &Value) -> Result<i128, Flow> {
    match v {
        Value::Int(n) => Ok(*n),
        Value::Marker(m) => Ok(m.borrow().position as i128 + 1),
        _ => Err(i.wrong_type_mut("number-or-marker-p", v)),
    }
}

fn check_position(i: &mut Interp, v: &Value, buf: &crate::buffer::Buffer) -> Result<i128, Flow> {
    let pos = fix_position(i, v)?;
    let lo = buf.begv as i128 + 1;
    let hi = buf.zv as i128 + 1;
    if pos < lo || pos > hi {
        return Err(sig(
            i,
            "args-out-of-range",
            vec![Value::Int(pos), Value::Int(lo), Value::Int(hi)],
        ));
    }
    Ok(pos)
}

/// charpos (1-based, buffer-wide) -> byte offset inside the visible
/// region.  Caller guarantees `lo <= pos <= hi`.
fn charpos_to_byte(buf: &crate::buffer::Buffer, pos: i128) -> usize {
    let visible_start = buf.begv;
    let target = (pos as usize).saturating_sub(1); // 0-based char index
    if target <= visible_start {
        return 0;
    }
    buf.text.substring(visible_start, target.min(buf.zv)).len()
}

/// byte offset inside the visible region -> 1-based charpos.
fn byte_to_charpos(buf: &crate::buffer::Buffer, byte: usize) -> i128 {
    let mut acc = 0usize;
    let mut idx = buf.begv;
    let end = buf.zv;
    while acc < byte && idx < end {
        let c = buf.text.char_at(idx);
        let w = c.len_utf8();
        if acc + w > byte {
            break;
        }
        acc += w;
        idx += 1;
    }
    (idx as i128) + 1
}

// ---------------------------------------------------------------------------
// Language loading
// ---------------------------------------------------------------------------

/// `resolve_language_symbol': follow `treesit-language-remap-alist'.
fn resolve_language(i: &mut Interp, lang: SymId) -> SymId {
    let var = sname(i, "treesit-language-remap-alist");
    let alist = i.symbol_value(var);
    if let Ok(items) = alist.list_to_vec() {
        for item in items {
            if let Value::Cons(c) = &item {
                let (k, v) = {
                    let cc = c.borrow();
                    (cc.car.clone(), cc.cdr.clone())
                };
                if let (Value::Sym(k), Value::Sym(v)) = (k, v) {
                    if k == lang {
                        return v;
                    }
                }
            }
        }
    }
    lang
}

fn string_list(_i: &mut Interp, v: &Value) -> Vec<String> {
    v.list_to_vec()
        .map(|xs| {
            xs.iter()
                .filter_map(|x| {
                    if let Value::Str(s) = x {
                        Some(s.borrow().clone())
                    } else {
                        None
                    }
                })
                .collect()
        })
        .unwrap_or_default()
}

fn dynamic_library_suffixes(i: &mut Interp) -> Vec<String> {
    let var = sname(i, "dynamic-library-suffixes");
    let v = i.symbol_value(var);
    let mut xs = string_list(i, &v);
    if xs.is_empty() {
        xs = vec![".so".to_string(), ".dylib".to_string()];
    }
    xs
}

/// Build the candidate name list like GNU's
/// `treesit_load_language_push_for_each_suffix'.
fn push_suffix_candidates(base: &str, suffixes: &[String], out: &mut Vec<String>) {
    for sfx in suffixes {
        let c1 = format!("{base}{sfx}");
        out.push(format!("{c1}.0.0"));
        out.push(format!("{c1}.0"));
        for v in
            tree_sitter::MIN_COMPATIBLE_LANGUAGE_VERSION..=tree_sitter::LANGUAGE_VERSION
        {
            out.push(format!("{c1}.{v}.0"));
        }
        out.push(c1);
    }
}

/// Find `(LANG LIB-NAME C-NAME)' in `treesit-load-name-override-list'.
fn find_override(i: &mut Interp, lang: SymId) -> Option<(String, String)> {
    let var = sname(i, "treesit-load-name-override-list");
    let v = i.symbol_value(var);
    let items = v.list_to_vec().ok()?;
    for item in items {
        let xs = item.list_to_vec().ok()?;
        if xs.len() >= 3 {
            if let Value::Sym(s) = &xs[0] {
                if *s == lang {
                    let lib = want_string(i, &xs[1]).ok()?;
                    let cname = want_string(i, &xs[2]).ok()?;
                    return Some((lib, cname));
                }
            }
        }
    }
    None
}

/// Load (or reuse) the grammar for LANGUAGE symbol.
/// On error returns (error-symbol, data) suitable for a signal.
fn load_language(
    i: &mut Interp,
    lang: SymId,
) -> Result<&'static LoadedLang, (SymId, Vec<Value>)> {
    let mapped = resolve_language(i, lang);
    if i.treesit.languages.contains_key(&mapped) {
        let r: &'static LoadedLang =
            unsafe { std::mem::transmute(i.treesit.languages.get(&mapped).unwrap()) };
        return Ok(r);
    }

    let name = i.symbol_name(mapped);
    let mut lib_base = format!("libtree-sitter-{name}");
    let mut c_name = format!("tree_sitter_{name}");
    if let Some((lib, cname)) = find_override(i, mapped) {
        lib_base = lib;
        c_name = cname;
    }
    let c_name = c_name.replace('-', "_");

    // `expand-file-name' ("~" etc.) like GNU's treesit_load_language.
    let expand = |i: &mut Interp, path: &str| -> String {
        let esym = sname(i, "expand-file-name");
        i.call_function(
            &Value::Sym(esym),
            &Value::list(vec![Value::string(path), Value::Nil]),
            Some(esym),
        )
        .ok()
        .and_then(|v| {
            if let Value::Str(s) = v {
                Some(s.borrow().clone())
            } else {
                None
            }
        })
        .unwrap_or_else(|| path.to_string())
    };

    let suffixes = dynamic_library_suffixes(i);
    let mut candidates: Vec<String> = Vec::new();
    // GNU search order: treesit-extra-load-path, then
    // user-emacs-directory/tree-sitter/, then system (bare names).
    let extra = {
        let var = sname(i, "treesit-extra-load-path");
        let v = i.symbol_value(var);
        string_list(i, &v)
    };
    for dir in extra.iter().rev() {
        let full = expand(i, &format!("{}/{lib_base}", dir.trim_end_matches('/')));
        push_suffix_candidates(&full, &suffixes, &mut candidates);
    }
    let ued = {
        let var = sname(i, "user-emacs-directory");
        let v = i.symbol_value(var);
        if let Value::Str(s) = v {
            s.borrow().clone()
        } else {
            "~/.emacs.d/".to_string()
        }
    };
    let home_dir = expand(i, &format!("{ued}tree-sitter/{lib_base}"));
    push_suffix_candidates(&home_dir, &suffixes, &mut candidates);
    push_suffix_candidates(&lib_base, &suffixes, &mut candidates);

    let mut errors: Vec<String> = Vec::new();
    for cand in &candidates {
        unsafe {
            let lib = match libloading::Library::new(cand) {
                Ok(l) => l,
                Err(e) => {
                    errors.push(format!("{cand}: {e}"));
                    continue;
                }
            };
            let symres: Result<
                libloading::Symbol<unsafe extern "C" fn() -> *const ()>,
                libloading::Error,
            > = lib.get(c_name.as_bytes());
            let f = match symres {
                Ok(f) => *f,
                Err(e) => {
                    errors.push(format!("{cand}: {e}"));
                    continue;
                }
            };
            let lfn = LanguageFn::from_raw(f);
            let language = Language::new(lfn);
            let abi = language.abi_version();
            if abi < tree_sitter::MIN_COMPATIBLE_LANGUAGE_VERSION
                || abi > tree_sitter::LANGUAGE_VERSION
            {
                errors.push(format!(
                    "{cand}: ABI version {abi} incompatible (supported {}..={})",
                    tree_sitter::MIN_COMPATIBLE_LANGUAGE_VERSION,
                    tree_sitter::LANGUAGE_VERSION
                ));
                continue;
            }
            let raw = lfn.into_raw()() as *const ffi::TSLanguage;
            let loaded = LoadedLang {
                _lib: lib,
                language,
                raw,
                path: cand.clone(),
            };
            i.treesit.languages.insert(mapped, loaded);
            // The map owns it; hand out a 'static reference for the
            // interpreter's lifetime.
            let r: &'static LoadedLang =
                std::mem::transmute(i.treesit.languages.get(&mapped).unwrap());
            return Ok(r);
        }
    }

    Err((
        sname(i, "treesit-load-language-error"),
        vec![
            Value::string("Cannot load language definition"),
            Value::Sym(lang),
            Value::string(errors.join("\n")),
        ],
    ))
}

fn load_language_signal(i: &mut Interp, lang: SymId) -> Result<&'static LoadedLang, Flow> {
    match load_language(i, lang) {
        Ok(l) => Ok(l),
        Err((s, data)) => Err(i.signal_data(s, data)),
    }
}

// ---------------------------------------------------------------------------
// Parse management
// ---------------------------------------------------------------------------

/// Reparse if the buffer changed since the last parse.
/// Returns changed regions as (beg . end) charpos conses, like
/// `treesit_ensure_parsed'.
fn ensure_parsed(i: &mut Interp, pid: u64) -> Result<Value, Flow> {
    let (need, buf) = {
        let p = &i.treesit.parsers[&pid];
        let b = p.buffer.borrow();
        (
            p.tree.is_none()
                || p.need_reparse
                || p.buf_tick != b.chars_mod_tick
                || p.begv != b.begv
                || p.zv != b.zv,
            p.buffer.clone(),
        )
    };
    if !need {
        return Ok(Value::Nil);
    }
    let changed = reparse(i, pid, &buf)?;
    // Notify after-change functions (GNU `safe_calln', errors ignored).
    let ranges_val = ranges_value(&changed);
    let pval = i.treesit.parsers[&pid].self_val.clone();
    let fns = i.treesit.parsers[&pid].notifiers.clone();
    for f in fns {
        let args = Value::list(vec![ranges_val.clone(), pval.clone()]);
        let _ = i.call_function(&f, &args, None);
    }
    Ok(ranges_val)
}

fn ranges_value(changed: &[(usize, usize)]) -> Value {
    let mut out = Value::Nil;
    for (b, e) in changed.iter().rev() {
        out = Value::cons(
            Value::cons(Value::Int(*b as i128), Value::Int(*e as i128)),
            out,
        );
    }
    out
}

fn reparse(i: &mut Interp, pid: u64, buf: &BufferRef) -> Result<Vec<(usize, usize)>, Flow> {
    let (text, begv, zv, tick) = {
        let b = buf.borrow();
        let chars: Vec<char> = b.text.text().chars().collect();
        let begv = b.begv.min(chars.len());
        let zv = b.zv.min(chars.len()).max(begv);
        let text: String = chars[begv..zv].iter().collect();
        (text, b.begv, b.zv, b.chars_mod_tick)
    };
    if text.len() > u32::MAX as usize {
        return Err(sig(i, "treesit-buffer-too-large", vec![]));
    }
    let old_tree = i
        .treesit
        .parsers
        .get_mut(&pid)
        .and_then(|p| p.tree.take());
    let p = i.treesit.parsers.get_mut(&pid).unwrap();
    let new_tree = p.parser.parse(&text, old_tree.as_ref());
    let new_tree = match new_tree {
        Some(t) => t,
        None => {
            p.tree = old_tree;
            return Err(sig(i, "treesit-parse-error", vec![]));
        }
    };
    // Changed byte regions between the old and new tree, mapped to
    // charpos ranges.
    let mut changed: Vec<(usize, usize)> = Vec::new();
    if let Some(old) = &old_tree {
        let b = buf.borrow();
        for r in old.changed_ranges(&new_tree) {
            let sb = byte_to_charpos(&b, r.start_byte as usize) as usize;
            let eb = byte_to_charpos(&b, r.end_byte as usize) as usize;
            if sb != eb {
                changed.push((sb, eb));
            }
        }
    }
    p.tree = Some(new_tree);
    p.buf_tick = tick;
    p.begv = begv;
    p.zv = zv;
    p.need_reparse = false;
    p.parse_count += 1;
    Ok(changed)
}

// ---------------------------------------------------------------------------
// Language / library predicates
// ---------------------------------------------------------------------------

fn f_treesit_available_p(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let _ = (i, a);
    Ok(Value::t())
}

fn f_language_available_p(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let lang = want_sym(i, &a[0])?;
    let detail = arg(&a, 1).truthy();
    match load_language(i, lang) {
        Ok(_) => {
            if detail {
                Ok(Value::cons(Value::t(), Value::Nil))
            } else {
                Ok(Value::t())
            }
        }
        Err((_, data)) => {
            if detail {
                // (nil . SIGNAL-DATA) — the data list of
                // `treesit-load-language-error', like GNU.
                Ok(Value::cons(Value::Nil, Value::list(data)))
            } else {
                Ok(Value::Nil)
            }
        }
    }
}

fn f_library_abi_version(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let min = arg(&a, 0).truthy();
    let _ = i;
    Ok(Value::Int(if min {
        tree_sitter::MIN_COMPATIBLE_LANGUAGE_VERSION as i128
    } else {
        tree_sitter::LANGUAGE_VERSION as i128
    }))
}

fn f_language_abi_version(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let lang = want_sym(i, &a[0])?;
    match load_language(i, lang) {
        Ok(l) => {
            let detail = arg(&a, 1).truthy();
            let abi = l.language.abi_version() as i128;
            if detail {
                let name = l.language.name().unwrap_or("");
                Ok(Value::cons(
                    Value::Int(abi),
                    Value::string(name.to_string()),
                ))
            } else {
                Ok(Value::Int(abi))
            }
        }
        Err(_) => Ok(Value::Nil),
    }
}

fn f_grammar_location(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let lang = want_sym(i, &a[0])?;
    match load_language(i, lang) {
        Ok(l) => Ok(Value::string(l.path.clone())),
        Err(_) => Ok(Value::Nil),
    }
}

// ---------------------------------------------------------------------------
// Line-column tracking
// ---------------------------------------------------------------------------

fn f_tracking_line_column_p(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let _ = a;
    Ok(Value::from_bool(i.treesit.tracking_linecol))
}

fn f_parser_tracking_line_column_p(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let pid = want_parser(i, &a[0])?;
    Ok(Value::from_bool(i.treesit.parsers[&pid].tracking_linecol))
}

// ---------------------------------------------------------------------------
// Object predicates
// ---------------------------------------------------------------------------

fn f_parser_p(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    Ok(Value::from_bool(
        tagged(i, &a[0], "treesit-parser")
            .and_then(|r| r.first().cloned())
            .and_then(|v| v.int().map(|n| n as u64))
            .map(|id| i.treesit.parsers.contains_key(&id))
            .unwrap_or(false),
    ))
}

fn f_node_p(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    Ok(Value::from_bool(is_node_obj(i, &a[0])))
}

fn f_compiled_query_p(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    Ok(Value::from_bool(is_query_obj(i, &a[0])))
}

fn query_p_val(i: &mut Interp, v: &Value) -> bool {
    match v {
        Value::Str(_) | Value::Cons(_) => true,
        _ => is_query_obj(i, v),
    }
}

fn f_query_p(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    Ok(Value::from_bool(query_p_val(i, &a[0])))
}

fn f_query_eagerly_compiled_p(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    match want_query(i, &a[0]) {
        Ok(id) => Ok(Value::from_bool(
            i.treesit.queries[&id].query.is_some(),
        )),
        Err(_) => Ok(Value::Nil),
    }
}

fn f_query_language(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let id = want_query(i, &a[0])?;
    Ok(Value::Sym(i.treesit.queries[&id].language))
}

fn f_query_source(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let id = want_query(i, &a[0])?;
    Ok(i.treesit.queries[&id].source.clone())
}

fn f_node_parser(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let key = check_node(i, &a[0])?;
    let pid = i.treesit.nodes[&key].parser_id;
    Ok(i.treesit.parsers[&pid].self_val.clone())
}

// ---------------------------------------------------------------------------
// Parser API
// ---------------------------------------------------------------------------

fn language_needs_linecol(i: &mut Interp, lang: SymId) -> bool {
    let var = sname(i, "treesit-languages-require-line-column-tracking");
    let v = i.symbol_value(var);
    v.list_to_vec()
        .map(|xs| xs.iter().any(|x| matches!(x, Value::Sym(s) if *s == lang)))
        .unwrap_or(false)
}

/// `(BUFFER)' arg → (base BufferRef, base buffer Value).  nil = current.
fn want_buffer_or_current(
    i: &mut Interp,
    v: &Value,
) -> Result<(BufferRef, Value), Flow> {
    let (buf, buf_val) = match v {
        Value::Nil => {
            let b = i
                .current_buffer_ref()
                .ok_or_else(|| i.error("Selecting deleted buffer"))?;
            let val = i.buffer_value(i.current_buffer).unwrap_or(Value::Nil);
            (b, val)
        }
        Value::Buffer(b) => (b.clone(), v.clone()),
        other => return Err(i.wrong_type_mut("bufferp", other)),
    };
    // Indirect buffers use their base buffer's parsers.
    if let Some(base) = buf.borrow().base_buffer {
        if let Some(bb) = i.buffers.get(base) {
            let val = i.buffer_value(base).unwrap_or(buf_val);
            return Ok((bb, val));
        }
    }
    Ok((buf, buf_val))
}

fn f_parser_create(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let lang = want_sym(i, &a[0])?;
    let buffer = arg(&a, 1);
    let no_reuse = arg(&a, 2).truthy();
    let tag = arg(&a, 3);
    if !matches!(tag, Value::Sym(_) | Value::Nil) {
        return Err(i.wrong_type_mut("symbolp", &tag));
    }
    if matches!(tag, Value::Sym(s) if s == sym::T) {
        let not_s = sname(i, "not");
        return Err(sig(
            i,
            "wrong-type-argument",
            vec![Value::list(vec![Value::Sym(not_s), Value::t()]), tag],
        ));
    }

    let (buf, buf_val) = want_buffer_or_current(i, &buffer)?;

    // Reuse an existing parser for (language, tag, buffer).
    if !no_reuse {
        let bid = buf.borrow().id;
        for pid in &i.treesit.parser_order {
            let p = &i.treesit.parsers[pid];
            if !p.deleted
                && p.buffer.borrow().live
                && p.language == lang
                && super::eq_values(&p.tag, &tag)
                && p.buffer.borrow().id == bid
            {
                return Ok(p.self_val.clone());
            }
        }
    }

    let lang_ref = load_language_signal(i, lang)?;
    let mut parser = Parser::new();
    if parser.set_language(&lang_ref.language).is_err() {
        return Err(sig(
            i,
            "treesit-load-language-error",
            vec![Value::Sym(lang)],
        ));
    }

    let tracking = language_needs_linecol(i, lang);
    if tracking {
        i.treesit.tracking_linecol = true;
    }

    let id = i.treesit.next_id;
    i.treesit.next_id += 1;
    let tag_id = sname(i, "treesit-parser");
    let self_val = tag_record(tag_id, vec![Value::Int(id as i128)]);
    i.treesit.parsers.insert(
        id,
        TsParser {
            parser,
            tree: None,
            buffer: buf,
            buffer_val: buf_val,
            self_val: self_val.clone(),
            language: lang,
            tag,
            embed_level: Value::Nil,
            ranges: Value::Nil,
            notifiers: Vec::new(),
            deleted: false,
            need_reparse: true,
            parse_count: 0,
            buf_tick: 0,
            begv: usize::MAX,
            zv: usize::MAX,
            tracking_linecol: tracking,
        },
    );
    i.treesit.parser_order.push(id);
    Ok(self_val)
}

fn f_parser_delete(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let pid = want_parser(i, &a[0])?;
    i.treesit.parsers.get_mut(&pid).unwrap().deleted = true;
    Ok(Value::Nil)
}

fn f_parser_list(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let buffer = arg(&a, 0);
    let language = arg(&a, 1);
    let tag = arg(&a, 2);
    let lang_sym = if matches!(language, Value::Nil) {
        None
    } else {
        Some(want_sym(i, &language)?)
    };
    let (buf, buf_val) = want_buffer_or_current(i, &buffer)?;
    let bid = buf.borrow().id;
    let tag_is_t = matches!(tag, Value::Sym(s) if s == sym::T);
    let mut items = Vec::new();
    for pid in &i.treesit.parser_order {
        let p = &i.treesit.parsers[pid];
        if p.deleted || !p.buffer.borrow().live {
            continue;
        }
        if p.buffer.borrow().id != bid {
            continue;
        }
        if let Some(l) = lang_sym {
            if p.language != l {
                continue;
            }
        }
        if !tag_is_t && !super::eq_values(&p.tag, &tag) {
            continue;
        }
        if !super::eq_values(&p.buffer_val, &buf_val) {
            continue;
        }
        items.push(p.self_val.clone());
    }
    Ok(Value::list(items))
}

fn f_parser_buffer(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let pid = want_parser(i, &a[0])?;
    Ok(i.treesit.parsers[&pid].buffer_val.clone())
}

fn f_parser_language(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let pid = want_parser(i, &a[0])?;
    Ok(Value::Sym(i.treesit.parsers[&pid].language))
}

fn f_parser_tag(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let pid = want_parser(i, &a[0])?;
    Ok(i.treesit.parsers[&pid].tag.clone())
}

fn f_parser_embed_level(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let pid = want_parser(i, &a[0])?;
    Ok(i.treesit.parsers[&pid].embed_level.clone())
}

fn f_parser_set_embed_level(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let pid = want_parser(i, &a[0])?;
    let level = a[1].clone();
    if !matches!(level, Value::Nil) {
        let n = want_int(i, &level)?;
        if n < 0 {
            return Err(sig(i, "args-out-of-range", vec![level]));
        }
    }
    i.treesit.parsers.get_mut(&pid).unwrap().embed_level = level.clone();
    Ok(level)
}

fn f_parser_root_node(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let pid = want_parser(i, &a[0])?;
    ensure_parsed(i, pid)?;
    let root = stored_root(i, pid);
    Ok(make_node(i, pid, root))
}

/// `treesit-buffer-root-node': first parser (or first for LANGUAGE)
/// in the current buffer.
fn f_buffer_root_node(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let language = arg(&a, 0);
    let lang_sym = if matches!(language, Value::Nil) {
        None
    } else {
        Some(want_sym(i, &language)?)
    };
    let buf = i
        .current_buffer_ref()
        .ok_or_else(|| i.error("Selecting deleted buffer"))?;
    let bid = buf.borrow().id;
    for pid in &i.treesit.parser_order {
        let p = &i.treesit.parsers[pid];
        if p.deleted || !p.buffer.borrow().live || p.buffer.borrow().id != bid {
            continue;
        }
        if let Some(l) = lang_sym {
            if p.language != l {
                continue;
            }
        }
        let pid = *pid;
        ensure_parsed(i, pid)?;
        let root = stored_root(i, pid);
        return Ok(make_node(i, pid, root));
    }
    Ok(Value::Nil)
}

/// Convert `(BEG . END)' charpos ranges into `TSRange's in visible-text
/// byte offsets, like `treesit_make_ts_ranges'.
fn make_ts_ranges(
    i: &mut Interp,
    ranges: &Value,
    buf: &BufferRef,
) -> Result<Vec<Range>, Flow> {
    let items = ranges.list_to_vec().map_err(|e| match e {
        crate::lisp::value::ListError::Circular => {
            i.signal_data(sym::CIRCULAR_LIST, vec![ranges.clone()])
        }
        crate::lisp::value::ListError::Dotted(t) => i.wrong_type_mut("listp", &t),
    })?;
    let mut out = Vec::with_capacity(items.len());
    let mut prev_end: i128 = i128::MIN;
    {
        let b = buf.borrow();
        for item in &items {
            let (beg, end) = match item {
                Value::Cons(c) => {
                    let cc = c.borrow();
                    (fix_position(i, &cc.car)?, fix_position(i, &cc.cdr)?)
                }
                other => return Err(i.wrong_type_mut("consp", other)),
            };
            let lo = b.begv as i128 + 1;
            let hi = b.zv as i128 + 1;
            if beg < lo || beg > hi || end < lo || end > hi || beg > end {
                return Err(sig(i, "treesit-range-invalid", vec![item.clone()]));
            }
            if beg < prev_end {
                // GNU requires ordered non-overlapping ranges.
                return Err(sig(i, "treesit-range-invalid", vec![item.clone()]));
            }
            prev_end = end;
            out.push(Range {
                start_byte: charpos_to_byte(&b, beg),
                end_byte: charpos_to_byte(&b, end),
                start_point: Point { row: 0, column: 0 },
                end_point: Point { row: 0, column: 0 },
            });
        }
    }
    Ok(out)
}

fn f_parser_set_included_ranges(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let pid = want_parser(i, &a[0])?;
    let ranges = a[1].clone();
    if !matches!(ranges, Value::Nil) && !matches!(ranges, Value::Cons(_)) {
        return Err(i.wrong_type_mut("consp", &ranges));
    }
    if super::equal_values(i, &i.treesit.parsers[&pid].ranges, &ranges) {
        return Ok(Value::Nil);
    }
    let buf = i.treesit.parsers[&pid].buffer.clone();
    let ts_ranges = make_ts_ranges(i, &ranges, &buf)?;
    let ok = i
        .treesit
        .parsers
        .get_mut(&pid)
        .unwrap()
        .parser
        .set_included_ranges(&ts_ranges)
        .is_ok();
    if !ok {
        return Err(sig(
            i,
            "treesit-range-invalid",
            vec![
                Value::string("Something went wrong when setting ranges"),
                ranges,
            ],
        ));
    }
    let p = i.treesit.parsers.get_mut(&pid).unwrap();
    p.ranges = ranges;
    p.need_reparse = true;
    Ok(Value::Nil)
}

fn f_parser_included_ranges(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let pid = want_parser(i, &a[0])?;
    Ok(i.treesit.parsers[&pid].ranges.clone())
}

fn f_parser_notifiers(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let pid = want_parser(i, &a[0])?;
    Ok(Value::list(i.treesit.parsers[&pid].notifiers.clone()))
}

fn f_parser_add_notifier(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let pid = want_parser(i, &a[0])?;
    let f = a[1].clone();
    i.treesit
        .parsers
        .get_mut(&pid)
        .unwrap()
        .notifiers
        .push(f);
    Ok(Value::Nil)
}

fn f_parser_remove_notifier(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let pid = want_parser(i, &a[0])?;
    let f = a[1].clone();
    let p = i.treesit.parsers.get_mut(&pid).unwrap();
    p.notifiers.retain(|x| !super::eq_values(x, &f));
    Ok(Value::Nil)
}

fn f_parse_string(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let text = want_string(i, &a[0])?;
    let lang = want_sym(i, &a[1])?;
    // Create a scratch buffer like GNU's " *treesit-parse-string*".
    let gen_sym = sname(i, "generate-new-buffer-name");
    let name = i.call_function(
        &Value::Sym(gen_sym),
        &Value::list(vec![
            Value::string(" *treesit-parse-string*"),
            Value::Nil,
        ]),
        Some(gen_sym),
    )?;
    let gc = sname(i, "get-buffer-create");
    let bufv = i.call_function(
        &Value::Sym(gc),
        &Value::list(vec![name, Value::Nil]),
        Some(gc),
    )?;
    let buf = match &bufv {
        Value::Buffer(b) => b.clone(),
        _ => return Err(i.error("get-buffer-create did not return a buffer")),
    };
    {
        let mut b = buf.borrow_mut();
        b.text.set_text(&text);
        b.begv = 0;
        b.zv = b.text.len();
        b.chars_mod_tick += 1;
        b.mod_tick += 1;
    }
    let parser = f_parser_create(
        i,
        vec![Value::Sym(lang), bufv.clone(), Value::t(), Value::Nil],
    )?;
    let pid = want_parser(i, &parser)?;
    ensure_parsed(i, pid)?;
    let root = stored_root(i, pid);
    Ok(make_node(i, pid, root))
}

fn f_parser_changed_regions(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let pid = want_parser(i, &a[0])?;
    ensure_parsed(i, pid)
}

// ---------------------------------------------------------------------------
// Node API
// ---------------------------------------------------------------------------

fn node_value_or_nil(i: &mut Interp, node: Option<Node>, pid: u64) -> Value {
    match node {
        Some(n) => make_node(i, pid, n),
        _ => Value::Nil,
    }
}

fn node_charpos(i: &Interp, pid: u64, byte: usize) -> i128 {
    let p = &i.treesit.parsers[&pid];
    let b = p.buffer.borrow();
    byte_to_charpos(&b, byte)
}

fn f_node_type(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    if matches!(a[0], Value::Nil) {
        return Ok(Value::Nil);
    }
    let key = check_node(i, &a[0])?;
    let n = *ts_node_of(i, key);
    Ok(Value::string(n.kind()))
}

fn f_node_start(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    if matches!(a[0], Value::Nil) {
        return Ok(Value::Nil);
    }
    let key = check_node(i, &a[0])?;
    let pid = i.treesit.nodes[&key].parser_id;
    let byte = ts_node_of(i, key).start_byte();
    Ok(Value::Int(node_charpos(i, pid, byte)))
}

fn f_node_end(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    if matches!(a[0], Value::Nil) {
        return Ok(Value::Nil);
    }
    let key = check_node(i, &a[0])?;
    let pid = i.treesit.nodes[&key].parser_id;
    let byte = ts_node_of(i, key).end_byte();
    Ok(Value::Int(node_charpos(i, pid, byte)))
}

fn f_node_string(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    if matches!(a[0], Value::Nil) {
        return Ok(Value::Nil);
    }
    let key = check_node(i, &a[0])?;
    let n = *ts_node_of(i, key);
    Ok(Value::string(n.to_sexp()))
}

fn f_node_parent(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    if matches!(a[0], Value::Nil) {
        return Ok(Value::Nil);
    }
    let key = check_node(i, &a[0])?;
    let pid = i.treesit.nodes[&key].parser_id;
    let n = *ts_node_of(i, key);
    let p = n.parent();
    Ok(node_value_or_nil(i, p, pid))
}

fn f_node_child(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    if matches!(a[0], Value::Nil) {
        return Ok(Value::Nil);
    }
    let key = check_node(i, &a[0])?;
    let pid = i.treesit.nodes[&key].parser_id;
    let n = *ts_node_of(i, key);
    let named = arg(&a, 2).truthy();
    let mut idx = want_int(i, &a[1])?;
    if idx < 0 {
        idx += if named {
            n.named_child_count() as i128
        } else {
            n.child_count() as i128
        };
    }
    if idx < 0 {
        return Ok(Value::Nil);
    }
    if idx > u32::MAX as i128 {
        return Err(sig(i, "args-out-of-range", vec![a[1].clone()]));
    }
    let child = if named {
        n.named_child(idx as usize)
    } else {
        n.child(idx as usize)
    };
    Ok(node_value_or_nil(i, child, pid))
}

fn f_node_check(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    if matches!(a[0], Value::Nil) {
        return Ok(Value::Nil);
    }
    let prop = want_sym(i, &a[1])?;
    // `outdated' answers without the live check, like GNU.
    let key = node_entry(i, &a[0])?;
    let entry = &i.treesit.nodes[&key];
    let pname = i.symbol_name(prop);
    if pname == "outdated" {
        let p = &i.treesit.parsers[&entry.parser_id];
        return Ok(Value::from_bool(entry.parse_count != p.parse_count));
    }
    let key = check_node(i, &a[0])?;
    let n = *ts_node_of(i, key);
    let result = match pname.as_str() {
        "named" => n.is_named(),
        "missing" => n.is_missing(),
        "extra" => n.is_extra(),
        "has-error" => n.has_error(),
        "live" => parser_live(i, i.treesit.nodes[&key].parser_id),
        _ => {
            return Err(sig(
                i,
                "error",
                vec![Value::string(format!(
                    "Expecting `named', `missing', `extra', `outdated', `has-error', or `live', but got {}",
                    pname
                ))],
            ))
        }
    };
    Ok(Value::from_bool(result))
}

fn f_node_field_name_for_child(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    if matches!(a[0], Value::Nil) {
        return Ok(Value::Nil);
    }
    let key = check_node(i, &a[0])?;
    let n = *ts_node_of(i, key);
    let mut idx = want_int(i, &a[1])?;
    if idx < 0 {
        idx += n.child_count() as i128;
    }
    if idx < 0 {
        return Ok(Value::Nil);
    }
    if idx > u32::MAX as i128 {
        return Err(sig(i, "args-out-of-range", vec![a[1].clone()]));
    }
    match n.field_name_for_child(idx as u32) {
        Some(f) => Ok(Value::string(f)),
        None => Ok(Value::Nil),
    }
}

fn f_node_child_count(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    if matches!(a[0], Value::Nil) {
        return Ok(Value::Nil);
    }
    let key = check_node(i, &a[0])?;
    let n = *ts_node_of(i, key);
    let named = arg(&a, 1).truthy();
    Ok(Value::Int(
        (if named {
            n.named_child_count()
        } else {
            n.child_count()
        }) as i128,
    ))
}

fn f_node_child_by_field_name(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    if matches!(a[0], Value::Nil) {
        return Ok(Value::Nil);
    }
    let key = check_node(i, &a[0])?;
    let pid = i.treesit.nodes[&key].parser_id;
    let n = *ts_node_of(i, key);
    let name = want_string(i, &a[1])?;
    let c = n.child_by_field_name(&name);
    Ok(node_value_or_nil(i, c, pid))
}

fn f_node_next_sibling(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    if matches!(a[0], Value::Nil) {
        return Ok(Value::Nil);
    }
    let key = check_node(i, &a[0])?;
    let pid = i.treesit.nodes[&key].parser_id;
    let n = *ts_node_of(i, key);
    let named = arg(&a, 1).truthy();
    let sib = if named {
        n.next_named_sibling()
    } else {
        n.next_sibling()
    };
    Ok(node_value_or_nil(i, sib, pid))
}

fn f_node_prev_sibling(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    if matches!(a[0], Value::Nil) {
        return Ok(Value::Nil);
    }
    let key = check_node(i, &a[0])?;
    let pid = i.treesit.nodes[&key].parser_id;
    let n = *ts_node_of(i, key);
    let named = arg(&a, 1).truthy();
    let sib = if named {
        n.prev_named_sibling()
    } else {
        n.prev_sibling()
    };
    Ok(node_value_or_nil(i, sib, pid))
}

fn f_node_first_child_for_pos(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    if matches!(a[0], Value::Nil) {
        return Ok(Value::Nil);
    }
    let key = check_node(i, &a[0])?;
    let pid = i.treesit.nodes[&key].parser_id;
    let n = *ts_node_of(i, key);
    let (pos, byte) = {
        let bufr = i.treesit.parsers[&pid].buffer.clone();
        let bb = bufr.borrow();
        let pos = check_position(i, &a[1], &bb)?;
        (pos, charpos_to_byte(&bb, pos))
    };
    let _ = pos;
    let named = arg(&a, 2).truthy();
    // GNU `treesit_cursor_first_child_for_byte': first child extending
    // beyond POS (end_byte > pos), honoring NAMED.
    let mut cursor = n.walk();
    if !cursor.goto_first_child() {
        return Ok(Value::Nil);
    }
    loop {
        let c = cursor.node();
        if (!named || c.is_named()) && c.end_byte() > byte {
            return Ok(make_node(i, pid, c));
        }
        if !cursor.goto_next_sibling() {
            return Ok(Value::Nil);
        }
    }
}

fn f_node_descendant_for_range(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    if matches!(a[0], Value::Nil) {
        return Ok(Value::Nil);
    }
    let key = check_node(i, &a[0])?;
    let pid = i.treesit.nodes[&key].parser_id;
    let n = *ts_node_of(i, key);
    let (beg, end) = {
        let bufr = i.treesit.parsers[&pid].buffer.clone();
        let bb = bufr.borrow();
        let beg = check_position(i, &a[1], &bb)?;
        let end = check_position(i, &a[2], &bb)?;
        (charpos_to_byte(&bb, beg), charpos_to_byte(&bb, end))
    };
    let named = arg(&a, 3).truthy();
    let d = if named {
        n.named_descendant_for_byte_range(beg, end)
    } else {
        n.descendant_for_byte_range(beg, end)
    };
    Ok(node_value_or_nil(i, d, pid))
}

fn f_node_eq(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    if matches!(a[0], Value::Nil) || matches!(a[1], Value::Nil) {
        return Ok(Value::Nil);
    }
    let k1 = node_entry(i, &a[0])?;
    let k2 = node_entry(i, &a[1])?;
    let (e1, e2) = (&i.treesit.nodes[&k1], &i.treesit.nodes[&k2]);
    let same = e1.parser_id == e2.parser_id
        && e1.parse_count == e2.parse_count
        && e1.node.id() == e2.node.id();
    Ok(Value::from_bool(same))
}

// ---------------------------------------------------------------------------
// Query expansion
// ---------------------------------------------------------------------------

fn query_string_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '\0' => out.push_str("\\0"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '"' | '\\' => {
                out.push('\\');
                out.push(c);
            }
            _ => out.push(c),
        }
    }
    out.push('"');
    out
}

fn prin1_string(i: &mut Interp, v: &Value) -> String {
    i.prin1_to_string(v)
}

fn pattern_expand(i: &mut Interp, pat: &Value) -> Value {
    match pat {
        Value::Sym(s) => {
            let name = i.symbol_name(*s);
            match name.as_str() {
                ":anchor" => return Value::string("."),
                ":?" => return Value::string("?"),
                ":*" => return Value::string("*"),
                ":+" => return Value::string("+"),
                ":equal" | ":eq?" => return Value::string("#eq?"),
                ":match" | ":match?" => return Value::string("#match?"),
                ":pred" | ":pred?" => return Value::string("#pred?"),
                _ => {}
            }
            Value::string(prin1_string(i, pat))
        }
        Value::Vec(v) => {
            let items = v.borrow();
            let parts: Vec<String> = items
                .iter()
                .map(|x| {
                    let e = pattern_expand(i, x);
                    match e {
                        Value::Str(s) => s.borrow().clone(),
                        _ => String::new(),
                    }
                })
                .collect();
            Value::string(format!("[{}]", parts.join(" ")))
        }
        Value::Cons(_) => {
            let items = pat.list_to_vec().unwrap_or_default();
            let parts: Vec<String> = items
                .iter()
                .map(|x| {
                    let e = pattern_expand(i, x);
                    match e {
                        Value::Str(s) => s.borrow().clone(),
                        _ => String::new(),
                    }
                })
                .collect();
            Value::string(format!("({})", parts.join(" ")))
        }
        Value::Str(s) => Value::string(query_string_escape(&s.borrow())),
        _ => Value::string(prin1_string(i, pat)),
    }
}

fn f_pattern_expand(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    Ok(pattern_expand(i, &a[0]))
}

fn f_query_expand(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let items = a[0]
        .list_to_vec()
        .map_err(|_| i.wrong_type_mut("listp", &a[0]))?;
    let parts: Vec<String> = items
        .iter()
        .map(|x| {
            let e = pattern_expand(i, x);
            match e {
                Value::Str(s) => s.borrow().clone(),
                _ => String::new(),
            }
        })
        .collect();
    Ok(Value::string(parts.join(" ")))
}

// ---------------------------------------------------------------------------
// Query compile/capture (raw FFI, like GNU)
// ---------------------------------------------------------------------------

/// Compile `src' for `lang' — returns raw TSQuery or signals
/// `treesit-query-error'.
fn compile_query_raw(
    i: &mut Interp,
    lang: &LoadedLang,
    src: &str,
) -> Result<*mut ffi::TSQuery, Flow> {
    let mut err_off: u32 = 0;
    let mut err_type: ffi::TSQueryError = 0;
    let q = unsafe {
        ffi::ts_query_new(
            lang.raw,
            src.as_ptr() as *const core::ffi::c_char,
            src.len() as u32,
            &mut err_off,
            &mut err_type,
        )
    };
    if q.is_null() {
        let data = query_error_data(src, err_off as usize, err_type);
        return Err(sig(i, "treesit-query-error", data));
    }
    Ok(q)
}

fn query_error_data(src: &str, offset: usize, etype: ffi::TSQueryError) -> Vec<Value> {
    let (mut line_start, mut row, mut found) = (0usize, 0usize, None);
    for line in src.lines() {
        let end = line_start + line.len() + 1;
        if end > offset {
            found = Some(line);
            break;
        }
        line_start = end;
        row += 1;
    }
    let col = offset.saturating_sub(line_start);
    vec![
        Value::string(format!(
            "Query error at row {row} col {col}: {}",
            found.unwrap_or("EOF")
        )),
        Value::Int(etype as i128),
        Value::Int(offset as i128),
    ]
}

/// Source text of a query value (string stays, sexp expands).
fn query_source_string(i: &mut Interp, q: &Value) -> Result<String, Flow> {
    match q {
        Value::Str(s) => Ok(s.borrow().clone()),
        Value::Cons(_) => {
            let expanded = f_query_expand(i, vec![q.clone()])?;
            if let Value::Str(s) = expanded {
                Ok(s.borrow().clone())
            } else {
                Err(i.error("query expansion failed"))
            }
        }
        _ => Err(i.wrong_type_mut("treesit-query-p", q)),
    }
}

/// Ensure a compiled-query object has its raw query.
fn ensure_query_compiled(i: &mut Interp, qid: u64) -> Result<*mut ffi::TSQuery, Flow> {
    if let Some(q) = i.treesit.queries[&qid].query {
        return Ok(q);
    }
    let (src_v, lang) = {
        let q = &i.treesit.queries[&qid];
        (q.source.clone(), q.language)
    };
    let lang_ref = load_language_signal(i, lang)?;
    let src = query_source_string(i, &src_v)?;
    let q = compile_query_raw(i, lang_ref, &src)?;
    i.treesit.queries.get_mut(&qid).unwrap().query = Some(q);
    Ok(q)
}

fn f_query_compile(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let lang = want_sym(i, &a[0])?;
    let query = a[1].clone();
    let eager = arg(&a, 2).truthy();
    if !query_p_val(i, &query) {
        return Err(i.wrong_type_mut("treesit-query-p", &query));
    }
    if let Ok(id) = want_query(i, &query) {
        if eager {
            ensure_query_compiled(i, id)?;
        }
        return Ok(query);
    }
    let id = i.treesit.next_id;
    i.treesit.next_id += 1;
    i.treesit.queries.insert(
        id,
        TsQueryObj {
            source: query,
            language: lang,
            query: None,
        },
    );
    if eager {
        ensure_query_compiled(i, id)?;
    }
    let tag = sname(i, "treesit-compiled-query");
    Ok(tag_record(tag, vec![Value::Int(id as i128)]))
}

/// Resolve NODE to a node key (accepts node, parser, or language
/// symbol), like `treesit_resolve_node'.
fn resolve_node(i: &mut Interp, v: &Value) -> Result<(u64, usize), Flow> {
    if is_node_obj(i, v) {
        return check_node(i, v);
    }
    let pid = if tagged(i, v, "treesit-parser").is_some() {
        want_parser(i, v)?
    } else if i.sym_id(v).is_some() {
        // Language symbol: create (or reuse) a parser in the current
        // buffer like GNU.
        let parser = f_parser_create(
            i,
            vec![v.clone(), Value::Nil, Value::Nil, Value::Nil],
        )?;
        want_parser(i, &parser)?
    } else {
        return Err(i.wrong_type_mut(
            "(or treesit-node-p treesit-parser-p symbolp)",
            v,
        ));
    };
    ensure_parsed(i, pid)?;
    let root = stored_root(i, pid);
    let val = make_node(i, pid, root);
    check_node(i, &val)
}

/// Predicates for one pattern index as a Lisp list of clauses; each
/// clause is a list of symbols (capture names) and strings, the head
/// being the operator name — `treesit_predicates_for_pattern'.
unsafe fn predicates_for_pattern(
    i: &mut Interp,
    q: *const ffi::TSQuery,
    pattern_index: u32,
) -> Value {
    let mut count: u32 = 0;
    let steps = unsafe { ffi::ts_query_predicates_for_pattern(q, pattern_index, &mut count) };
    let mut clauses: Vec<Value> = Vec::new();
    let mut cur: Vec<Value> = Vec::new();
    for idx in 0..count as isize {
        let step = unsafe { &*steps.offset(idx) };
        match step.type_ {
            ffi::TSQueryPredicateStepTypeCapture => {
                let mut len: u32 = 0;
                let s = unsafe {
                    ffi::ts_query_capture_name_for_id(q, step.value_id, &mut len)
                };
                let name = unsafe {
                    std::str::from_utf8_unchecked(std::slice::from_raw_parts(
                        s as *const u8,
                        len as usize,
                    ))
                };
                let sname_v = Value::Sym(sname(i, name));
                cur.push(sname_v);
            }
            ffi::TSQueryPredicateStepTypeString => {
                let mut len: u32 = 0;
                let s = unsafe {
                    ffi::ts_query_string_value_for_id(q, step.value_id, &mut len)
                };
                let text = unsafe {
                    std::str::from_utf8_unchecked(std::slice::from_raw_parts(
                        s as *const u8,
                        len as usize,
                    ))
                };
                cur.push(Value::string(text));
            }
            _ => {
                clauses.push(Value::list(std::mem::take(&mut cur)));
            }
        }
    }
    Value::list(clauses)
}

/// Text of a captured node (buffer substring).
fn captured_node_text(i: &Interp, node_val: &Value) -> Result<String, Flow> {
    let key = node_entry_soft(i, node_val).ok_or_else(|| i.error("invalid node"))?;
    let entry = &i.treesit.nodes[&key];
    let p = &i.treesit.parsers[&entry.parser_id];
    let b = p.buffer.borrow();
    let s = byte_to_charpos(&b, entry.node.start_byte()) as usize;
    let e = byte_to_charpos(&b, entry.node.end_byte()) as usize;
    Ok(b.text.substring(s - 1, e - 1))
}

/// `treesit_pred_capture_name_to_node'.
fn capture_name_to_node(
    name: &Value,
    captures: &[Value],
    _i: &Interp,
) -> Result<Value, Vec<Value>> {
    if let Value::Sym(target) = name {
        for cap in captures {
            if let Value::Cons(c) = cap {
                let (k, v) = {
                    let cc = c.borrow();
                    (cc.car.clone(), cc.cdr.clone())
                };
                if let Value::Sym(s) = &k {
                    if s == target {
                        return Ok(v);
                    }
                }
            }
        }
    }
    Err(vec![
        Value::string("Cannot find captured node"),
        name.clone(),
        Value::string(
            "A predicate can only refer to captured nodes in the same pattern",
        ),
    ])
}

fn capture_name_to_text(
    name: &Value,
    captures: &[Value],
    i: &Interp,
) -> Result<Value, Vec<Value>> {
    let node = capture_name_to_node(name, captures, i)?;
    captured_node_text(i, &node)
        .map(Value::string)
        .map_err(|_| vec![Value::string("Cannot get node text")])
}

/// `#eq?' predicate — GNU `treesit_predicate_equal'.
fn predicate_equal(
    args: &[Value],
    captures: &[Value],
    i: &Interp,
) -> Result<bool, Vec<Value>> {
    if args.len() != 2 {
        return Err(vec![
            Value::string("Predicate `equal' requires two arguments but got"),
            Value::Int(args.len() as i128),
        ]);
    }
    let mut texts: [Value; 2] = [args[0].clone(), args[1].clone()];
    for (k, a) in args.iter().enumerate() {
        if matches!(a, Value::Sym(_)) {
            texts[k] = capture_name_to_text(a, captures, i)?;
        }
    }
    let eq = match (&texts[0], &texts[1]) {
        (Value::Str(x), Value::Str(y)) => *x.borrow() == *y.borrow(),
        (Value::Int(x), Value::Int(y)) => x == y,
        _ => false,
    };
    Ok(eq)
}

/// `#match?' predicate — GNU `treesit_predicate_match'.  Uses the
/// Emacs regexp engine on the node text.
fn predicate_match(
    i: &mut Interp,
    args: &[Value],
    captures: &[Value],
) -> Result<bool, Vec<Value>> {
    if args.len() != 2 {
        return Err(vec![
            Value::string("Predicate `match?' requires two arguments but got"),
            Value::Int(args.len() as i128),
        ]);
    }
    let (regexp, capname) = if matches!(args[1], Value::Sym(_)) {
        (args[0].clone(), args[1].clone())
    } else {
        (args[1].clone(), args[0].clone())
    };
    let pat = match &regexp {
        Value::Str(s) => s.borrow().clone(),
        _ => {
            return Err(vec![
                Value::string(
                    "Predicate `match?' takes a regexp and a node capture (order doesn't matter), but got",
                ),
                Value::Int(args.len() as i128),
            ])
        }
    };
    let node = capture_name_to_node(&capname, captures, i)?;
    let text = captured_node_text(i, &node)
        .map_err(|_| vec![Value::string("Cannot get node text")])?;
    let case_fold = {
        let cf = i.intern_soft("case-fold-search");
        cf.map(|s| i.symbol_value(s).truthy()).unwrap_or(false)
    };
    let re = crate::lisp::regexp::compile_case(&pat, case_fold)
        .map_err(|e| vec![Value::string(format!("Invalid regexp: {}", e.0))])?;
    let syn = crate::editor::re_syntax(i);
    let chars: Vec<char> = text.chars().collect();
    Ok(crate::lisp::regexp::search(&re, &chars, 0, &syn).is_some())
}

/// `#pred?' predicate — apply the interned function name to the
/// captured nodes.
fn predicate_pred(
    i: &mut Interp,
    args: &[Value],
    captures: &[Value],
    pid: u64,
) -> Result<bool, Vec<Value>> {
    if args.len() < 2 {
        return Err(vec![
            Value::string(
                "Predicate `pred' requires at least two arguments, but only got",
            ),
            Value::Int(args.len() as i128),
        ]);
    }
    let fname = match &args[0] {
        Value::Str(s) => s.borrow().clone(),
        Value::Sym(s) => i.symbol_name(*s),
        other => {
            return Err(vec![
                Value::string("Invalid `pred' function name"),
                other.clone(),
            ])
        }
    };
    let fsym = i.intern(&fname);
    let mut nodes = Vec::new();
    for a in &args[1..] {
        nodes.push(capture_name_to_node(a, captures, i)?);
    }
    let ts = i.treesit.parsers[&pid].parse_count;
    let fun = Value::Sym(fsym);
    let val = i
        .call_function(&fun, &Value::list(nodes), Some(fsym))
        .map_err(|_| vec![Value::string("Predicate function signaled an error")])?;
    if i.treesit.parsers[&pid].parse_count != ts {
        return Err(vec![Value::Sym(fsym)]);
    }
    Ok(val.truthy())
}

/// Evaluate predicate clauses like `treesit_eval_predicates'.
fn eval_predicates(
    i: &mut Interp,
    predicates: &Value,
    captures: &[Value],
    pid: u64,
) -> Result<bool, Vec<Value>> {
    let clauses = predicates.list_to_vec().unwrap_or_default();
    for clause in clauses {
        let parts = clause.list_to_vec().unwrap_or_default();
        if parts.is_empty() {
            continue;
        }
        let fname = match &parts[0] {
            Value::Str(s) => s.borrow().clone(),
            Value::Sym(s) => i.symbol_name(*s),
            _ => String::new(),
        };
        let args = &parts[1..];
        match fname.as_str() {
            "eq?" => {
                if !predicate_equal(args, captures, i)? {
                    return Ok(false);
                }
            }
            "match?" => {
                if !predicate_match(i, args, captures)? {
                    return Ok(false);
                }
            }
            "pred?" => {
                if !predicate_pred(i, args, captures, pid)? {
                    return Ok(false);
                }
            }
            _ => {
                return Err(vec![
                    Value::string("Invalid predicate"),
                    parts[0].clone(),
                    Value::string(
                        "Currently Emacs only supports `equal', `match', and `pred' predicates",
                    ),
                ]);
            }
        }
    }
    Ok(true)
}

fn f_query_capture(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let node = a[0].clone();
    let query = a[1].clone();
    let beg = arg(&a, 2);
    let end = arg(&a, 3);
    let node_only = arg(&a, 4).truthy();
    let grouped = arg(&a, 5).truthy();

    if !query_p_val(i, &query) {
        return Err(i.wrong_type_mut("treesit-query-p", &query));
    }

    let (pid, node_key) = resolve_node(i, &node)?;
    let ts_node = *ts_node_of(i, (pid, node_key));
    let parser_lang = i.treesit.parsers[&pid].language;

    // A compiled query must match the node's parser language (GNU
    // signals "language of query and node differ").
    if let Ok(qid) = want_query(i, &query) {
        if i.treesit.queries[&qid].language != parser_lang {
            return Err(sig(
                i,
                "treesit-error",
                vec![Value::string(
                    "Query's language is different from the node's parser's language",
                )],
            ));
        }
    }

    // Check BEG/END.
    let (beg_byte, end_byte) = {
        let bufr = i.treesit.parsers[&pid].buffer.clone();
        let bb = bufr.borrow();
        let mut bb_ = (0usize, u32::MAX as usize);
        if !matches!(beg, Value::Nil) {
            let p1 = check_position(i, &beg, &bb)?;
            bb_.0 = charpos_to_byte(&bb, p1);
        }
        if !matches!(end, Value::Nil) {
            let p2 = check_position(i, &end, &bb)?;
            bb_.1 = charpos_to_byte(&bb, p2);
        }
        if bb_.0 > bb_.1 {
            return Err(sig(
                i,
                "args-out-of-range",
                vec![beg.clone(), end.clone()],
            ));
        }
        bb_
    };

    // Initialize query + cursor (raw FFI).
    let (qptr, need_free) = match &query {
        v if is_query_obj(i, v) => {
            let qid = want_query(i, v)?;
            (ensure_query_compiled(i, qid)?, false)
        }
        _ => {
            let lang_ref = load_language_signal(i, parser_lang)?;
            let src = query_source_string(i, &query)?;
            (compile_query_raw(i, lang_ref, &src)?, true)
        }
    };

    let mut result: Vec<Value> = Vec::new();
    let mut pred_cache: HashMap<u16, Value> = HashMap::new();
    let mut pred_error: Option<Vec<Value>> = None;

    unsafe {
        let cursor = ffi::ts_query_cursor_new();
        ffi::ts_query_cursor_set_byte_range(
            cursor,
            beg_byte.min(u32::MAX as usize) as u32,
            end_byte.min(u32::MAX as usize) as u32,
        );
        let raw_node = ts_node.into_raw();
        ffi::ts_query_cursor_exec(cursor, qptr, raw_node);
        let mut m: ffi::TSQueryMatch = std::mem::zeroed();
        while ffi::ts_query_cursor_next_match(cursor, &mut m) {
            let mut group: Vec<Value> = Vec::new();
            // A match can carry zero captures (e.g. a pattern that is only
            // a predicate); `captures` is NULL then.
            let caps: &[ffi::TSQueryCapture] = if m.capture_count == 0 || m.captures.is_null()
            {
                &[]
            } else {
                std::slice::from_raw_parts(m.captures, m.capture_count as usize)
            };
            for cap in caps {
                let mut len: u32 = 0;
                let cname =
                    ffi::ts_query_capture_name_for_id(qptr, cap.index, &mut len);
                let name = std::str::from_utf8_unchecked(std::slice::from_raw_parts(
                    cname as *const u8,
                    len as usize,
                ));
                let node_val = make_node(i, pid, Node::from_raw(cap.node));
                let csym = sname(i, name);
                group.push(Value::cons(Value::Sym(csym), node_val));
            }
            let predicates = match pred_cache.get(&m.pattern_index) {
                Some(v) => v.clone(),
                None => {
                    let v = predicates_for_pattern(i, qptr, m.pattern_index as u32);
                    pred_cache.insert(m.pattern_index, v.clone());
                    v
                }
            };
            let pass = match eval_predicates(i, &predicates, &group, pid) {
                Ok(p) => p,
                Err(e) => {
                    pred_error = Some(e);
                    break;
                }
            };
            if !pass {
                continue;
            }
            if node_only {
                let nodes: Vec<Value> = group
                    .into_iter()
                    .map(|c| {
                        if let Value::Cons(cc) = &c {
                            cc.borrow().cdr.clone()
                        } else {
                            c
                        }
                    })
                    .collect();
                if grouped {
                    result.push(Value::list(nodes));
                } else {
                    result.extend(nodes);
                }
            } else if grouped {
                result.push(Value::list(group));
            } else {
                result.extend(group);
            }
        }
        ffi::ts_query_cursor_delete(cursor);
        if need_free {
            ffi::ts_query_delete(qptr);
        }
    }

    if let Some(e) = pred_error {
        return Err(sig(i, "treesit-query-error", e));
    }
    Ok(Value::list(result))
}

// ---------------------------------------------------------------------------
// Traversal helpers (ports of GNU's cursor utilities)
// ---------------------------------------------------------------------------

/// `treesit_traverse_sibling_helper'.
fn traverse_sibling(cursor: &mut TreeCursor, forward: bool, named: bool) -> bool {
    if forward {
        if !named {
            return cursor.goto_next_sibling();
        }
        while cursor.goto_next_sibling() {
            if cursor.node().is_named() {
                return true;
            }
        }
        false
    } else {
        if !named {
            return cursor.goto_previous_sibling();
        }
        while cursor.goto_previous_sibling() {
            if cursor.node().is_named() {
                return true;
            }
        }
        false
    }
}

/// `treesit_traverse_child_helper'.
fn traverse_child(cursor: &mut TreeCursor, forward: bool, named: bool) -> bool {
    if forward {
        if !named {
            return cursor.goto_first_child();
        }
        if !cursor.goto_first_child() {
            return false;
        }
        if cursor.node().is_named() {
            return true;
        }
        if traverse_sibling(cursor, true, true) {
            return true;
        }
        cursor.goto_parent();
        false
    } else {
        if !cursor.goto_first_child() {
            return false;
        }
        while cursor.goto_next_sibling() {}
        if !named || cursor.node().is_named() {
            return true;
        }
        if traverse_sibling(cursor, false, true) {
            return true;
        }
        cursor.goto_parent();
        false
    }
}

/// `treesit_traverse_get_predicate': look up THING in
/// `treesit-thing-settings' for LANGUAGE.
fn traverse_get_predicate(i: &mut Interp, thing: SymId, language: SymId) -> Value {
    let var = sname(i, "treesit-thing-settings");
    let settings = i.symbol_value(var);
    let lang_entry = match settings.list_to_vec() {
        Ok(items) => items.iter().find_map(|it| {
            if let Value::Cons(c) = it {
                let (k, v) = {
                    let cc = c.borrow();
                    (cc.car.clone(), cc.cdr.clone())
                };
                if let Value::Sym(s) = &k {
                    if *s == language {
                        return Some(v);
                    }
                }
            }
            None
        }),
        Err(_) => None,
    };
    let lang_entry = match lang_entry {
        Some(v) => v,
        None => return Value::Nil,
    };
    if let Ok(defs) = lang_entry.list_to_vec() {
        for d in defs {
            if let Value::Cons(c) = &d {
                let (k, v) = {
                    let cc = c.borrow();
                    (cc.car.clone(), cc.cdr.clone())
                };
                if let Value::Sym(s) = &k {
                    if *s == thing {
                        if let Value::Cons(vv) = &v {
                            return vv.borrow().car.clone();
                        }
                        return Value::Nil;
                    }
                }
            }
        }
    }
    Value::Nil
}

/// FUNCTIONP minus thing symbols (`treesit-thing-symbol' property).
fn pred_function_p(i: &mut Interp, v: &Value) -> bool {
    match v {
        Value::Sym(s) => {
            let thing_sym = sname(i, "treesit-thing-symbol");
            if !matches!(i.get_prop(*s, thing_sym), Value::Nil) {
                return false;
            }
            i.fbound_p(*s)
        }
        Value::Lambda(_) | Value::Subr(_) => true,
        _ => false,
    }
}

/// `treesit_traverse_validate_predicate' → Ok(()) or
/// Err((err-symbol-name, data-values)).
fn validate_predicate(
    i: &mut Interp,
    pred: &Value,
    language: SymId,
    depth: usize,
) -> Result<(), (String, Vec<Value>)> {
    if depth > 99 {
        return Err((
            "treesit-invalid-predicate".into(),
            vec![Value::string(
                "Predicate recursion level exceeded: it must not exceed 100 levels",
            )],
        ));
    }
    match pred {
        Value::Str(_) => Ok(()),
        Value::Sym(s) => {
            let name = i.symbol_name(*s);
            if name == "named" || name == "anonymous" {
                return Ok(());
            }
            if pred_function_p(i, pred) {
                return Ok(());
            }
            let def = traverse_get_predicate(i, *s, language);
            if matches!(def, Value::Nil) {
                return Err((
                    "treesit-predicate-not-found".into(),
                    vec![
                        Value::string(
                            "Cannot find the definition of the predicate in `treesit-thing-settings'",
                        ),
                        pred.clone(),
                    ],
                ));
            }
            validate_predicate(i, &def, language, depth + 1)
        }
        Value::Cons(c) => {
            let (car, cdr) = {
                let cc = c.borrow();
                (cc.car.clone(), cc.cdr.clone())
            };
            if let Value::Sym(s) = &car {
                let op = i.symbol_name(*s);
                if op == "not" {
                    let xs = cdr.list_to_vec().map_err(|_| {
                        (
                            "treesit-invalid-predicate".to_string(),
                            vec![
                                Value::string("Invalid `not' predicate"),
                                pred.clone(),
                            ],
                        )
                    })?;
                    if xs.len() != 1 {
                        return Err((
                            "treesit-invalid-predicate".into(),
                            vec![
                                Value::string("`not' can only have one argument"),
                                pred.clone(),
                            ],
                        ));
                    }
                    return validate_predicate(i, &xs[0], language, depth + 1);
                }
                if op == "or" || op == "and" {
                    let xs = cdr.list_to_vec().map_err(|_| {
                        (
                            "treesit-invalid-predicate".to_string(),
                            vec![
                                Value::string(
                                    "`or' or `and' must have a list of patterns as arguments",
                                ),
                                pred.clone(),
                            ],
                        )
                    })?;
                    if xs.is_empty() {
                        return Err((
                            "treesit-invalid-predicate".into(),
                            vec![
                                Value::string(
                                    "`or' or `and' must have a list of patterns as arguments",
                                ),
                                pred.clone(),
                            ],
                        ));
                    }
                    for x in xs {
                        validate_predicate(i, &x, language, depth + 1)?;
                    }
                    return Ok(());
                }
            }
            if matches!(&car, Value::Str(_)) && pred_function_p(i, &cdr) {
                return Ok(());
            }
            Err((
                "treesit-invalid-predicate".into(),
                vec![
                    Value::string(
                        "Invalid predicate, see `treesit-thing-settings' for valid forms of predicate",
                    ),
                    pred.clone(),
                ],
            ))
        }
        _ if pred_function_p(i, pred) => Ok(()),
        _ => Err((
            "treesit-invalid-predicate".into(),
            vec![
                Value::string(
                    "Invalid predicate, see `treesit-thing-settings' for valid forms of predicate",
                ),
                pred.clone(),
            ],
        )),
    }
}

/// Call a predicate function with a node, checking the buffer was not
/// re-parsed during the call (`treesit_pred_with_guard').
fn pred_call_guard(
    i: &mut Interp,
    f: &Value,
    node_val: Value,
    pid: u64,
) -> Result<Value, Flow> {
    let ts = i.treesit.parsers[&pid].parse_count;
    let args = Value::list(vec![node_val]);
    let v = i.call_function(f, &args, None)?;
    if i.treesit.parsers[&pid].parse_count != ts {
        return Err(sig(i, "treesit-buffer_changed", vec![]));
    }
    Ok(v)
}

/// `treesit_traverse_match_predicate' — does the node match PRED?
fn match_predicate(
    i: &mut Interp,
    cursor_node: Node,
    pred: &Value,
    pid: u64,
    named: bool,
) -> Result<bool, Flow> {
    if named && !cursor_node.is_named() {
        return Ok(false);
    }
    match pred {
        Value::Str(s) => {
            let pat = s.borrow().clone();
            let typ = cursor_node.kind();
            let case_fold = {
                let cf = i.intern_soft("case-fold-search");
                cf.map(|x| i.symbol_value(x).truthy()).unwrap_or(false)
            };
            match crate::lisp::regexp::compile_case(&pat, case_fold) {
                Ok(re) => {
                    let syn = crate::editor::re_syntax(i);
                    let chars: Vec<char> = typ.chars().collect();
                    Ok(crate::lisp::regexp::search(&re, &chars, 0, &syn).is_some())
                }
                Err(_) => Ok(false),
            }
        }
        Value::Sym(s) => {
            let name = i.symbol_name(*s);
            if name == "named" {
                return Ok(cursor_node.is_named());
            }
            if name == "anonymous" {
                return Ok(!cursor_node.is_named());
            }
            if pred_function_p(i, pred) {
                let nv = make_node(i, pid, cursor_node);
                let r = pred_call_guard(i, pred, nv, pid)?;
                return Ok(r.truthy());
            }
            let lang = i.treesit.parsers[&pid].language;
            let def = traverse_get_predicate(i, *s, lang);
            match_predicate(i, cursor_node, &def, pid, named)
        }
        Value::Cons(c) => {
            let (car, cdr) = {
                let cc = c.borrow();
                (cc.car.clone(), cc.cdr.clone())
            };
            if let Value::Sym(s) = &car {
                let op = i.symbol_name(*s);
                if op == "not" {
                    let xs = cdr.list_to_vec().unwrap_or_default();
                    let inner = match_predicate(i, cursor_node, &xs[0], pid, named)?;
                    return Ok(!inner);
                }
                if op == "or" {
                    for x in cdr.list_to_vec().unwrap_or_default() {
                        if match_predicate(i, cursor_node, &x, pid, named)? {
                            return Ok(true);
                        }
                    }
                    return Ok(false);
                }
                if op == "and" {
                    for x in cdr.list_to_vec().unwrap_or_default() {
                        if !match_predicate(i, cursor_node, &x, pid, named)? {
                            return Ok(false);
                        }
                    }
                    return Ok(true);
                }
            }
            if matches!(&car, Value::Str(_)) && pred_function_p(i, &cdr) {
                if let Value::Str(s) = &car {
                    let pat = s.borrow().clone();
                    let typ = cursor_node.kind();
                    let case_fold = {
                        let cf = i.intern_soft("case-fold-search");
                        cf.map(|x| i.symbol_value(x).truthy()).unwrap_or(false)
                    };
                    let re = crate::lisp::regexp::compile_case(&pat, case_fold)
                        .map_err(|e| i.error(e.0.clone()))?;
                    let syn = crate::editor::re_syntax(i);
                    let chars: Vec<char> = typ.chars().collect();
                    if crate::lisp::regexp::search(&re, &chars, 0, &syn).is_none() {
                        return Ok(false);
                    }
                }
                let nv = make_node(i, pid, cursor_node);
                let r = pred_call_guard(i, &cdr, nv, pid)?;
                return Ok(r.truthy());
            }
            Ok(false)
        }
        _ if pred_function_p(i, pred) => {
            let nv = make_node(i, pid, cursor_node);
            let r = pred_call_guard(i, pred, nv, pid)?;
            Ok(r.truthy())
        }
        _ => Ok(false),
    }
}

/// `treesit_search_dfs'.
fn search_dfs(
    i: &mut Interp,
    cursor: &mut TreeCursor,
    pred: &Value,
    pid: u64,
    forward: bool,
    named: bool,
    limit: usize,
    skip_root: bool,
) -> Result<bool, Flow> {
    if !skip_root && match_predicate(i, cursor.node(), pred, pid, named)? {
        return Ok(true);
    }
    if limit == 0 {
        return Ok(false);
    }
    if !traverse_child(cursor, forward, named) {
        return Ok(false);
    }
    loop {
        if search_dfs(i, cursor, pred, pid, forward, named, limit - 1, false)? {
            return Ok(true);
        }
        if !traverse_sibling(cursor, forward, false) {
            break;
        }
    }
    cursor.goto_parent();
    Ok(false)
}

/// `treesit_search_forward' — linear leaf-first traversal.
fn search_forward_impl(
    i: &mut Interp,
    cursor: &mut TreeCursor,
    pred: &Value,
    pid: u64,
    forward: bool,
    named: bool,
) -> Result<bool, Flow> {
    let mut initial = true;
    loop {
        if !initial && match_predicate(i, cursor.node(), pred, pid, named)? {
            return Ok(true);
        }
        initial = false;
        while !traverse_sibling(cursor, forward, named) {
            if !cursor.goto_parent() {
                return Ok(false);
            }
            if match_predicate(i, cursor.node(), pred, pid, named)? {
                return Ok(true);
            }
        }
        while traverse_child(cursor, forward, false) {}
    }
}

/// `treesit_cursor_helper_1' — descend CURSOR (on root) down to TARGET.
fn cursor_helper_1(
    cursor: &mut TreeCursor,
    target: Node,
    start_pos: usize,
    end_pos: usize,
    limit: usize,
) -> bool {
    if limit == 0 {
        return false;
    }
    let mut cursor_node = cursor.node();
    if cursor_node == target {
        return true;
    }
    let descended = if start_pos != end_pos {
        cursor.goto_first_child_for_byte(start_pos).is_some()
    } else {
        false
    } || cursor.goto_first_child();
    if !descended {
        return false;
    }
    cursor_node = cursor.node();
    while cursor_node.start_byte() <= end_pos {
        if cursor_node.end_byte() >= end_pos
            && cursor_helper_1(cursor, target, start_pos, end_pos, limit - 1)
        {
            return true;
        }
        if !cursor.goto_next_sibling() {
            break;
        }
        cursor_node = cursor.node();
    }
    cursor.goto_parent();
    false
}

/// Position CURSOR at NODE with a proper parent stack.
fn cursor_helper<'t>(cursor: &mut TreeCursor<'t>, node: Node, root: Node<'t>) -> bool {
    let start = node.start_byte();
    let end = node.end_byte();
    *cursor = root.walk();
    cursor_helper_1(cursor, node, start, end, RECURSION_LIMIT)
}

fn signal_pred(i: &mut Interp, e: (String, Vec<Value>)) -> Flow {
    let s = sname(i, &e.0);
    i.signal_data(s, e.1)
}

fn check_sym_arg(i: &mut Interp, v: &Value) -> Result<(), Flow> {
    if matches!(v, Value::Sym(_) | Value::Nil) {
        Ok(())
    } else {
        Err(i.wrong_type_mut("symbolp", v))
    }
}

fn f_search_subtree(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let node_v = a[0].clone();
    let predicate = a[1].clone();
    let backward = arg(&a, 2);
    let all = arg(&a, 3);
    let depth = arg(&a, 4);
    check_sym_arg(i, &backward)?;
    check_sym_arg(i, &all)?;
    let backward = backward.truthy();
    let named = !all.truthy();
    let limit = if matches!(depth, Value::Nil) {
        RECURSION_LIMIT
    } else {
        want_int(i, &depth)? as usize
    };
    let key = check_node(i, &node_v)?;
    let pid = i.treesit.nodes[&key].parser_id;
    let language = i.treesit.parsers[&pid].language;
    validate_predicate(i, &predicate, language, 0).map_err(|e| signal_pred(i, e))?;

    ensure_parsed(i, pid)?;
    let node = *ts_node_of(i, key);
    let root = stored_root(i, pid);
    let mut cursor = root.walk();
    if !cursor_helper(&mut cursor, node, root) {
        return Ok(Value::Nil);
    }
    let found = search_dfs(
        i,
        &mut cursor,
        &predicate,
        pid,
        !backward,
        named,
        limit,
        true,
    )?;
    if found {
        Ok(make_node(i, pid, cursor.node()))
    } else {
        Ok(Value::Nil)
    }
}

fn f_search_forward(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let node_v = a[0].clone();
    let predicate = a[1].clone();
    let backward = arg(&a, 2);
    let all = arg(&a, 3);
    check_sym_arg(i, &backward)?;
    check_sym_arg(i, &all)?;
    let backward = backward.truthy();
    let named = !all.truthy();
    let key = check_node(i, &node_v)?;
    let pid = i.treesit.nodes[&key].parser_id;
    let language = i.treesit.parsers[&pid].language;
    validate_predicate(i, &predicate, language, 0).map_err(|e| signal_pred(i, e))?;

    ensure_parsed(i, pid)?;
    let node = *ts_node_of(i, key);
    let root = stored_root(i, pid);
    let mut cursor = root.walk();
    if !cursor_helper(&mut cursor, node, root) {
        return Ok(Value::Nil);
    }
    let found = search_forward_impl(i, &mut cursor, &predicate, pid, !backward, named)?;
    if found {
        Ok(make_node(i, pid, cursor.node()))
    } else {
        Ok(Value::Nil)
    }
}

/// `treesit_build_sparse_tree' — matched nodes become
/// `(NODE . children)'; unmatched levels are flattened out.
fn build_flat(
    i: &mut Interp,
    node: Node,
    pred: &Value,
    pfn: &Value,
    limit: usize,
    pid: u64,
    out: &mut Vec<Value>,
) -> Result<(), Flow> {
    let m = match_predicate(i, node, pred, pid, false)?;
    if m {
        let mut nv = make_node(i, pid, node);
        if !matches!(pfn, Value::Nil) {
            nv = pred_call_guard(i, pfn, nv, pid)?;
        }
        let mut kids: Vec<Value> = Vec::new();
        if limit > 0 {
            let mut cursor = node.walk();
            if cursor.goto_first_child() {
                loop {
                    let child = cursor.node();
                    build_flat(i, child, pred, pfn, limit - 1, pid, &mut kids)?;
                    if !cursor.goto_next_sibling() {
                        break;
                    }
                }
            }
        }
        let mut l = vec![nv];
        l.extend(kids);
        out.push(Value::list(l));
        Ok(())
    } else {
        if limit > 0 {
            let mut cursor = node.walk();
            if cursor.goto_first_child() {
                loop {
                    let child = cursor.node();
                    build_flat(i, child, pred, pfn, limit - 1, pid, out)?;
                    if !cursor.goto_next_sibling() {
                        break;
                    }
                }
            }
        }
        Ok(())
    }
}

fn f_induce_sparse_tree(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let node_v = a[0].clone();
    let predicate = a[1].clone();
    let process_fn = arg(&a, 2);
    let depth = arg(&a, 3);
    let limit = if matches!(depth, Value::Nil) {
        RECURSION_LIMIT
    } else {
        want_int(i, &depth)? as usize
    };
    let key = check_node(i, &node_v)?;
    let pid = i.treesit.nodes[&key].parser_id;
    let language = i.treesit.parsers[&pid].language;
    match validate_predicate(i, &predicate, language, 0) {
        Ok(()) => {}
        Err((name, data)) => {
            if name == "treesit-predicate-not-found" {
                return Ok(Value::Nil);
            }
            return Err(signal_pred(i, (name, data)));
        }
    }

    ensure_parsed(i, pid)?;
    let node = *ts_node_of(i, key);

    let mut out: Vec<Value> = Vec::new();
    if match_predicate(i, node, &predicate, pid, false)? {
        let mut nv = make_node(i, pid, node);
        if !matches!(process_fn, Value::Nil) {
            nv = pred_call_guard(i, &process_fn, nv, pid)?;
        }
        let mut kids: Vec<Value> = Vec::new();
        if limit > 0 {
            let mut cursor = node.walk();
            if cursor.goto_first_child() {
                loop {
                    let child = cursor.node();
                    build_flat(i, child, &predicate, &process_fn, limit - 1, pid, &mut kids)?;
                    if !cursor.goto_next_sibling() {
                        break;
                    }
                }
            }
        }
        let mut l = vec![nv];
        l.extend(kids);
        out.push(Value::list(l));
    } else if limit > 0 {
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                build_flat(i, child, &predicate, &process_fn, limit - 1, pid, &mut out)?;
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
    }
    if out.is_empty() {
        Ok(Value::Nil)
    } else {
        let mut l = vec![Value::Nil];
        l.extend(out);
        Ok(Value::list(l))
    }
}

fn f_node_match_p(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    if matches!(a[0], Value::Nil) {
        return Ok(Value::Nil);
    }
    let key = check_node(i, &a[0])?;
    let predicate = a[1].clone();
    let ignore_missing = arg(&a, 2).truthy();
    let pid = i.treesit.nodes[&key].parser_id;
    let language = i.treesit.parsers[&pid].language;
    match validate_predicate(i, &predicate, language, 0) {
        Ok(()) => {}
        Err((name, data)) => {
            if ignore_missing && name == "treesit-predicate-not-found" {
                return Ok(Value::Nil);
            }
            return Err(signal_pred(i, (name, data)));
        }
    }
    let node = *ts_node_of(i, key);
    let r = match_predicate(i, node, &predicate, pid, false)?;
    Ok(Value::from_bool(r))
}

fn f_subtree_stat(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let key = check_node(i, &a[0])?;
    let n = *ts_node_of(i, key);
    fn stat(n: Node, depth: usize, acc: &mut (usize, usize, usize)) {
        acc.0 = acc.0.max(depth + 1);
        acc.1 = acc.1.max(n.child_count());
        acc.2 += 1;
        for c in 0..n.child_count() {
            if let Some(ch) = n.child(c) {
                stat(ch, depth + 1, acc);
            }
        }
    }
    let mut acc = (0usize, 0usize, 0usize);
    stat(n, 0, &mut acc);
    Ok(Value::list(vec![
        Value::Int(acc.0 as i128),
        Value::Int(acc.1 as i128),
        Value::Int(acc.2 as i128),
    ]))
}

// ---------------------------------------------------------------------------
// linecol internals
// ---------------------------------------------------------------------------

fn f_linecol_at(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let buf = i
        .current_buffer_ref()
        .ok_or_else(|| i.error("Selecting deleted buffer"))?;
    let b = buf.borrow();
    let pos = check_position(i, &a[0], &b)?;
    let byte = b
        .text
        .substring(0, (pos as usize).saturating_sub(1).min(b.text.len()))
        .len();
    let full = b.text.text();
    let mut line = 0i128;
    let mut col = 0i128;
    for c in &full.as_bytes()[..byte.min(full.len())] {
        if *c == b'\n' {
            line += 1;
            col = 0;
        } else {
            col += 1;
        }
    }
    Ok(Value::cons(Value::Int(line), Value::Int(col)))
}

fn f_linecol_cache_set(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let line = want_int(i, &a[0])?;
    let col = want_int(i, &a[1])?;
    let bytepos = want_int(i, &a[2])?;
    let bid = i.current_buffer;
    i.treesit
        .linecol_caches
        .insert(bid, (line as i64, col as i64, bytepos as i64));
    Ok(Value::Nil)
}

fn f_linecol_cache(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let _ = a;
    let bid = i.current_buffer;
    let (line, col, bytepos) = i
        .treesit
        .linecol_caches
        .get(&bid)
        .copied()
        .unwrap_or((0, 0, 0));
    Ok(Value::list(vec![
        Value::Sym(sname(i, ":line")),
        Value::Int(line as i128),
        Value::Sym(sname(i, ":col")),
        Value::Int(col as i128),
        Value::Sym(sname(i, ":bytepos")),
        Value::Int(bytepos as i128),
    ]))
}

// ---------------------------------------------------------------------------
// Printing support (`print.c' equivalent), called from `print.rs'.
// ---------------------------------------------------------------------------

/// If `items' is a treesit tagged record `[treesit-* ...]', render it
/// the way GNU's `print_object' does.
pub(crate) fn treesit_repr(i: &Interp, items: &[Value]) -> Option<String> {
    let tag = match items.first() {
        Some(Value::Sym(s)) => i.symbol_name(*s),
        _ => return None,
    };
    match tag.as_str() {
        "treesit-parser" => {
            let id = match items.get(1) {
                Some(Value::Int(n)) => *n as u64,
                _ => return Some("#<treesit-parser>".into()),
            };
            let p = i.treesit.parsers.get(&id)?;
            let bufname = p.buffer.borrow().name.clone();
            let lang = i.symbol_name(p.language);
            if p.deleted {
                Some(format!("#<deleted treesit-parser for {lang}>"))
            } else {
                Some(format!("#<treesit-parser in {bufname} for {lang}>"))
            }
        }
        "treesit-node" => {
            let (pid, nid) = match (items.get(1), items.get(2)) {
                (Some(Value::Int(p)), Some(Value::Int(n))) => (*p as u64, *n as usize),
                _ => return Some("#<treesit-node>".into()),
            };
            let entry = i.treesit.nodes.get(&(pid, nid))?;
            let p = i.treesit.parsers.get(&entry.parser_id)?;
            let outdated = entry.parse_count != p.parse_count;
            let ty = entry.node.kind();
            if outdated {
                Some(format!("#<treesit-node {ty} (outdated)>"))
            } else {
                let b = p.buffer.borrow();
                let s = byte_to_charpos(&b, entry.node.start_byte());
                let e = byte_to_charpos(&b, entry.node.end_byte());
                Some(format!("#<treesit-node {ty} in {s}-{e}>"))
            }
        }
        "treesit-compiled-query" => Some("#<treesit-compiled-query>".into()),
        _ => None,
    }
}

use super::Interp;
use crate::lisp::value::Subr;

// ---------------------------------------------------------------------------
// SUBRS table
// ---------------------------------------------------------------------------

pub(crate) static SUBRS: &[Subr] = &[
    S!(
        "treesit-available-p",
        0,
        0,
        f_treesit_available_p,
        "Return non-nil if tree-sitter is available in this session."
    ),
    S!(
        "treesit-language-available-p",
        1,
        2,
        f_language_available_p,
        "Return non-nil if LANGUAGE exists and is loadable."
    ),
    S!(
        "treesit-library-abi-version",
        0,
        1,
        f_library_abi_version,
        "ABI version of the tree-sitter library."
    ),
    S!(
        "treesit-language-abi-version",
        1,
        2,
        f_language_abi_version,
        "ABI version of LANGUAGE's grammar."
    ),
    S!(
        "treesit-grammar-location",
        1,
        1,
        f_grammar_location,
        "File name from which LANGUAGE's grammar was loaded."
    ),
    S!(
        "treesit-tracking-line-column-p",
        0,
        1,
        f_tracking_line_column_p,
        "Whether line-column tracking is enabled."
    ),
    S!(
        "treesit-parser-tracking-line-column-p",
        1,
        1,
        f_parser_tracking_line_column_p,
        "Whether PARSER tracks line and column."
    ),
    S!("treesit-parser-p", 1, 1, f_parser_p, "t if OBJECT is a parser."),
    S!("treesit-node-p", 1, 1, f_node_p, "t if OBJECT is a node."),
    S!(
        "treesit-compiled-query-p",
        1,
        1,
        f_compiled_query_p,
        "t if OBJECT is a compiled query."
    ),
    S!(
        "treesit-query-p",
        1,
        1,
        f_query_p,
        "t if OBJECT is a query (string, sexp, or compiled)."
    ),
    S!(
        "treesit-query-eagerly-compiled-p",
        1,
        1,
        f_query_eagerly_compiled_p,
        "t if QUERY is a compiled query that is already compiled."
    ),
    S!(
        "treesit-query-language",
        1,
        1,
        f_query_language,
        "Language of compiled QUERY."
    ),
    S!(
        "treesit-query-source",
        1,
        1,
        f_query_source,
        "Source form of compiled QUERY."
    ),
    S!("treesit-node-parser", 1, 1, f_node_parser, "Parser of NODE."),
    S!(
        "treesit-parser-create",
        1,
        4,
        f_parser_create,
        "Create a parser in BUFFER for LANGUAGE with TAG."
    ),
    S!(
        "treesit-parser-delete",
        1,
        1,
        f_parser_delete,
        "Delete PARSER."
    ),
    S!(
        "treesit-parser-list",
        0,
        3,
        f_parser_list,
        "List BUFFER's parsers, filtered by LANGUAGE and TAG."
    ),
    S!(
        "treesit-parser-buffer",
        1,
        1,
        f_parser_buffer,
        "Buffer of PARSER."
    ),
    S!(
        "treesit-parser-language",
        1,
        1,
        f_parser_language,
        "Language symbol of PARSER."
    ),
    S!("treesit-parser-tag", 1, 1, f_parser_tag, "Tag of PARSER."),
    S!(
        "treesit-parser-embed-level",
        1,
        1,
        f_parser_embed_level,
        "Embed level of PARSER."
    ),
    S!(
        "treesit-parser-set-embed-level",
        2,
        2,
        f_parser_set_embed_level,
        "Set PARSER's embed level."
    ),
    S!(
        "treesit-parser-root-node",
        1,
        1,
        f_parser_root_node,
        "Root node of PARSER's tree."
    ),
    S!(
        "treesit-buffer-root-node",
        1,
        1,
        f_buffer_root_node,
        "Root node of the current buffer's parser (or of LANGUAGE's)."
    ),
    S!(
        "treesit-parser-set-included-ranges",
        2,
        2,
        f_parser_set_included_ranges,
        "Limit PARSER to RANGES."
    ),
    S!(
        "treesit-parser-included-ranges",
        1,
        1,
        f_parser_included_ranges,
        "Ranges set for PARSER."
    ),
    S!(
        "treesit-parser-notifiers",
        1,
        1,
        f_parser_notifiers,
        "After-change notifier functions of PARSER."
    ),
    S!(
        "treesit-parser-add-notifier",
        2,
        2,
        f_parser_add_notifier,
        "Add FUNCTION as PARSER notifier."
    ),
    S!(
        "treesit-parser-remove-notifier",
        2,
        2,
        f_parser_remove_notifier,
        "Remove FUNCTION from PARSER's notifiers."
    ),
    S!(
        "treesit-parse-string",
        2,
        2,
        f_parse_string,
        "Parse STRING with LANGUAGE, return the root node."
    ),
    S!(
        "treesit-parser-changed-regions",
        1,
        1,
        f_parser_changed_regions,
        "Force re-parse and return affected regions."
    ),
    S!("treesit-node-type", 1, 1, f_node_type, "Type of NODE."),
    S!(
        "treesit-node-start",
        1,
        1,
        f_node_start,
        "Start position of NODE."
    ),
    S!("treesit-node-end", 1, 1, f_node_end, "End position of NODE."),
    S!(
        "treesit-node-string",
        1,
        1,
        f_node_string,
        "S-expression describing NODE."
    ),
    S!(
        "treesit-node-parent",
        1,
        1,
        f_node_parent,
        "Parent of NODE."
    ),
    S!(
        "treesit-node-child",
        2,
        3,
        f_node_child,
        "Nth child of NODE."
    ),
    S!(
        "treesit-node-check",
        2,
        2,
        f_node_check,
        "Check a PROPERTY of NODE."
    ),
    S!(
        "treesit-node-field-name-for-child",
        2,
        2,
        f_node_field_name_for_child,
        "Field name of NODE's Nth child."
    ),
    S!(
        "treesit-node-child-count",
        1,
        2,
        f_node_child_count,
        "Child count of NODE."
    ),
    S!(
        "treesit-node-child-by-field-name",
        2,
        2,
        f_node_child_by_field_name,
        "Child of NODE for FIELD-NAME."
    ),
    S!(
        "treesit-node-next-sibling",
        1,
        2,
        f_node_next_sibling,
        "Next sibling of NODE."
    ),
    S!(
        "treesit-node-prev-sibling",
        1,
        2,
        f_node_prev_sibling,
        "Previous sibling of NODE."
    ),
    S!(
        "treesit-node-first-child-for-pos",
        2,
        3,
        f_node_first_child_for_pos,
        "First child of NODE extending beyond POS."
    ),
    S!(
        "treesit-node-descendant-for-range",
        3,
        4,
        f_node_descendant_for_range,
        "Smallest descendant of NODE covering BEG to END."
    ),
    S!(
        "treesit-node-eq",
        2,
        2,
        f_node_eq,
        "t if NODE1 and NODE2 are the same node."
    ),
    S!(
        "treesit-pattern-expand",
        1,
        1,
        f_pattern_expand,
        "Expand PATTERN to its string form."
    ),
    S!(
        "treesit-query-expand",
        1,
        1,
        f_query_expand,
        "Expand sexp QUERY to its string form."
    ),
    S!(
        "treesit-query-compile",
        2,
        3,
        f_query_compile,
        "Compile QUERY for LANGUAGE."
    ),
    S!(
        "treesit-query-capture",
        2,
        6,
        f_query_capture,
        "Query NODE with patterns in QUERY."
    ),
    S!(
        "treesit-search-subtree",
        2,
        5,
        f_search_subtree,
        "Traverse NODE's subtree depth-first with PREDICATE."
    ),
    S!(
        "treesit-search-forward",
        2,
        4,
        f_search_forward,
        "Search NODE's tree for a match of PREDICATE."
    ),
    S!(
        "treesit-induce-sparse-tree",
        2,
        4,
        f_induce_sparse_tree,
        "Create a sparse tree of NODE's subtree."
    ),
    S!(
        "treesit-node-match-p",
        2,
        3,
        f_node_match_p,
        "Whether NODE matches PREDICATE."
    ),
    S!(
        "treesit-subtree-stat",
        1,
        1,
        f_subtree_stat,
        "(MAX-DEPTH MAX-WIDTH COUNT) of NODE's subtree."
    ),
    S!(
        "treesit--linecol-at",
        1,
        1,
        f_linecol_at,
        "(LINE . COL) at POS using the buffer-local cache."
    ),
    S!(
        "treesit--linecol-cache-set",
        3,
        3,
        f_linecol_cache_set,
        "Set the current buffer's linecol cache."
    ),
    S!(
        "treesit--linecol-cache",
        0,
        0,
        f_linecol_cache,
        "Return the current buffer's linecol cache."
    ),
];
