//! Keymap traversal, lookup and mutation primitives.

use super::err_sym;

use super::is_keymap;

use crate::lisp::builtins::arg;
use crate::lisp::{
    Interp,
    error::{EvalResult, Flow},
    value::Value,
};

pub(super) fn f_keymap_canonicalize(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU subr.el's algorithm: walk MAP (inheritance flattened), gather
    // (KEY . DEF) pairs — char-range keys ((LO . HI)) go into a fresh
    // char-table via define-key; everything else lands in the alist
    // (duplicate keys merged by `keymap--merge-bindings').
    if keymap_of(i, &a[0])?.is_none() {
        return Ok(Value::list(vec![Value::Sym(i.q("keymap"))]));
    }
    let src = a[0].clone();
    let prompt = {
        let kp = Value::Sym(i.q("keymap-prompt"));
        i.apply(&kp, vec![src.clone()]).unwrap_or(Value::Nil)
    };
    let mut bindings: Vec<Value> = Vec::new();
    let mut ranges: Vec<(i128, i128, Value)> = Vec::new();
    if let Some(pairs) = keymap_of(i, &src)? {
        for (k, def) in pairs {
            if let Value::Cons(r) = &k {
                let rb = r.borrow();
                if let (Value::Int(lo), Value::Int(hi)) = (&rb.car, &rb.cdr) {
                    let (lo, hi) = (*lo, *hi);
                    drop(rb);
                    if hi == lo + 1 {
                        // Two-char range → two single bindings (GNU).
                        bindings.push(Value::cons(Value::Int(lo), def.clone()));
                        bindings.push(Value::cons(Value::Int(hi), def));
                    } else {
                        ranges.push((lo, hi, def));
                    }
                    continue;
                }
            }
            bindings.push(Value::cons(k, def));
        }
    }
    let mk = Value::Sym(i.intern(if ranges.is_empty() {
        "make-sparse-keymap"
    } else {
        "make-keymap"
    }));
    let map = i.apply(
        &mk,
        if prompt.is_nil() {
            vec![]
        } else {
            vec![prompt]
        },
    )?;
    let dk = Value::Sym(i.q("define-key"));
    for (lo, hi, def) in ranges {
        let rk = Value::cons(Value::Int(lo), Value::Int(hi));
        let kv = Value::vector(vec![rk]);
        i.apply(&dk, vec![map.clone(), kv, def])?;
    }
    // Merge duplicate keys; GNU pushes each binding then nconcs, so the
    // result's alist is reversed (later bindings come first).
    let merge = Value::Sym(i.q("keymap--merge-bindings"));
    let mut out: Vec<Value> = Vec::new();
    for b in bindings {
        let key = match &b {
            Value::Cons(c) => c.borrow().car.clone(),
            _ => continue,
        };
        let dup = out
            .iter()
            .position(|e| matches!(e, Value::Cons(c) if crate::lisp::builtins::eq_values(&c.borrow().car, &key)));
        match dup {
            None => out.push(b),
            Some(pos) => {
                let olddef = match &out[pos] {
                    Value::Cons(c) => c.borrow().cdr.clone(),
                    _ => Value::Nil,
                };
                let newdef = match &b {
                    Value::Cons(c) => c.borrow().cdr.clone(),
                    _ => Value::Nil,
                };
                let merged = i.apply(&merge, vec![newdef, olddef])?;
                out[pos] = Value::cons(key, merged);
            }
        }
    }
    // nconc: splice the alist onto the map tail, reversed like GNU's
    // (push (cons k v) bindings) ordering.
    let mut last = map.clone();
    loop {
        let next = match &last {
            Value::Cons(c) => c.borrow().cdr.clone(),
            _ => break,
        };
        if matches!(next, Value::Cons(_)) {
            last = next;
        } else {
            break;
        }
    }
    if let Value::Cons(c) = &last {
        let tail: Vec<Value> = out.into_iter().rev().collect();
        if !tail.is_empty() {
            c.borrow_mut().cdr = Value::list(tail);
        }
    }
    Ok(map)
}

fn keymap_of(i: &mut Interp, v: &Value) -> Result<Option<Vec<(Value, Value)>>, Flow> {
    // GNU's C `map_keymap' traversal order: char-table contents
    // (compressed ranges) at the table's spine position, then alist
    // pairs; embedded keymaps expand in place and the parent tail's
    // elements follow.  Symbols resolve via the function cell.
    let v = crate::editor::keymap_def(i, v.clone())?;
    if !crate::editor::is_keymap(i, &v) {
        return Ok(None);
    }
    Ok(Some(crate::editor::keymap_all_bindings(i, &v)))
}

pub(super) fn f_accessible_keymaps(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    // GNU Faccessible_keymaps: breadth-first walk of nested keymaps.
    // Entries are (PREFIX-VECTOR . MAP); the map itself is under PREFIX
    // ([] when nil). Maps reached via the meta-prefix (ESC=27) binding
    // are listed under meta-bit keys: [27] + key K => [K | CHAR_META].
    let prefix_elems: Vec<Value> = match arg(&args, 1) {
        Value::Nil => Vec::new(),
        Value::Vec(v) => v.borrow().clone(),
        Value::Str(s) => s
            .borrow()
            .chars()
            .map(|c| Value::Int((c).code() as i128))
            .collect(),
        Value::Cons(_) => match arg(&args, 1).list_to_vec() {
            Ok(v) => v,
            Err(_) => return Err(i.wrong_type_mut("sequencep", &args[1])),
        },
        // GNU Faccessible_keymaps validates PREFIX with CHECK_SEQUENCE:
        // a non-nil non-sequence signals wrong-type-argument.
        _ => return Err(i.wrong_type_mut("sequencep", &args[1])),
    };
    let prefixlen = prefix_elems.len();
    // GNU resolves the map arg through get_keymap (symbols OK).
    let map0 = crate::editor::keymap_def(i, args[0].clone())?;
    if !crate::editor::is_keymap(i, &map0) {
        return Err(i.wrong_type_mut("keymapp", &args[0]));
    }
    // (prefix-elems, map) queue, processed breadth-first. With a
    // PREFIX, GNU starts at the submap bound by that prefix.
    let mut maps: Vec<(Vec<Value>, Value)> = if prefixlen > 0 {
        let mut keys = Vec::new();
        for value in &prefix_elems {
            if let Some(code) = crate::editor::key_seq_elt(i, value)? {
                keys.push(code);
            }
        }
        let mut km = map0.clone();
        let mut ok = true;
        for &k in &keys {
            let raw = crate::editor::lookup_in_keymap(i, &km, k, false)?;
            let d = crate::editor::keymap_def(i, raw)?;
            if crate::editor::is_keymap(i, &d) {
                km = d;
            } else {
                ok = false;
                break;
            }
        }
        if !ok {
            return Ok(Value::Nil);
        }
        vec![(prefix_elems.clone(), km)]
    } else {
        vec![(prefix_elems, map0)]
    };

    /// get_keyelt + get_keymap(cmd, 0, 0): scan the binding's cons
    /// spine for a keymap — the cons itself, a `keymap`-tagged
    /// element, a menu-item's real def, or a symbol tail resolving
    /// via its function cell. Autoload forms are NOT loaded here
    /// (GNU "can't run lisp code" in this traversal).
    fn find_keymap(i: &mut Interp, def: Value) -> Option<Value> {
        let mut d = def;
        loop {
            match d.clone() {
                Value::Cons(c) => {
                    if crate::editor::is_keymap(i, &d) {
                        return Some(d);
                    }
                    let (car, cdr) = {
                        let b = c.borrow();
                        (b.car.clone(), b.cdr.clone())
                    };
                    if crate::editor::is_keymap(i, &car) {
                        return Some(car);
                    }
                    // (menu-item NAME DEF . PROPS): real def is DEF.
                    if i.sym_is(&car, i.q_soft("menu-item").unwrap_or(u32::MAX)) {
                        if let Some(dd) = d.list_to_vec().ok().and_then(|v| v.get(2).cloned()) {
                            d = dd;
                            continue;
                        }
                    }
                    d = cdr;
                }
                Value::Sym(s) => {
                    let f = i.symbol_function(s);
                    if crate::editor::is_keymap(i, &f) {
                        return Some(f);
                    }
                    // Autoload keymap: GNU lists the SYMBOL as the map
                    // (it is never scanned further — not a cons).
                    if crate::editor::is_autoload_keymap(i, &f) {
                        return Some(d);
                    }
                    return None;
                }
                _ => return None,
            }
        }
    }

    /// GNU cycle check: skip CMD if it already appears in MAPS under
    /// a prefix that is a prefix of THISSEQ.
    fn seen_as_prefix(maps: &[(Vec<Value>, Value)], cmd: &Value, thisseq: &[Value]) -> bool {
        for (pfx, m) in maps {
            if !crate::lisp::builtins::eq_values(m, cmd) {
                continue;
            }
            if pfx.len() <= thisseq.len()
                && pfx
                    .iter()
                    .zip(thisseq.iter())
                    .all(|(a, b)| crate::lisp::builtins::eq_values(a, b))
            {
                return true;
            }
        }
        false
    }

    let mut qi = 0usize;
    while qi < maps.len() {
        let (thisseq, thismap) = maps[qi].clone();
        // is_metized: last elem is the meta-prefix char (27) and not
        // part of a user-supplied PREFIX.
        let is_metized = thisseq
            .last()
            .map(|v| matches!(v, Value::Int(27)))
            .unwrap_or(false)
            && thisseq.len() - 1 >= prefixlen;
        // Only cons maps are scanned (autoload-keymap symbols are
        // leaf entries, as in GNU's `CONSP (thismap)` guard).
        if !matches!(thismap, Value::Cons(_)) {
            qi += 1;
            continue;
        }
        if let Some(pairs) = keymap_of(i, &thismap)? {
            for (k, def) in pairs {
                if let Some(m) = find_keymap(i, def) {
                    if seen_as_prefix(&maps, &m, &thisseq) {
                        continue;
                    }
                    let pfx = if is_metized {
                        // Replace trailing 27 with K | CHAR_META.
                        if let Value::Int(kc) = k {
                            let mut s = thisseq.clone();
                            let n = s.len() - 1;
                            s[n] = Value::Int(kc | crate::editor::META_BIT);
                            s
                        } else {
                            let mut s = thisseq.clone();
                            s.push(k.clone());
                            s
                        }
                    } else {
                        let mut s = thisseq.clone();
                        s.push(k.clone());
                        s
                    };
                    // Metized entries go right after the current node;
                    // plain entries append at the queue tail (GNU).
                    let pos = if is_metized { qi + 1 } else { maps.len() };
                    maps.insert(pos, (pfx, m));
                }
            }
        }
        qi += 1;
    }
    let out: Vec<Value> = maps
        .into_iter()
        .map(|(pfx, m)| Value::cons(Value::vector(pfx), m))
        .collect();
    Ok(Value::list(out))
}

fn map_keymap_impl(i: &mut Interp, args: &[Value], own_only: bool) -> EvalResult {
    let v = crate::editor::keymap_def(i, args[1].clone())?;
    if !crate::editor::is_keymap(i, &v) {
        return Err(i.wrong_type_mut("keymapp", &args[1]));
    }
    // GNU `map-keymap-internal' stops at the parent tail and at
    // embedded keymaps; `map-keymap' traverses them.
    let pairs = if own_only {
        crate::editor::keymap_own_bindings(i, &v)
    } else {
        crate::editor::keymap_all_bindings(i, &v)
    };
    for (k, def) in pairs {
        if matches!(&k, Value::Sym(s) if i.symbol_name(*s) == "keymap") {
            continue;
        }
        let fnv = args[0].clone();
        i.apply(&fnv, vec![k, def])?;
    }
    Ok(Value::Nil)
}

pub(super) fn f_map_keymap(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    map_keymap_impl(i, &args, false)
}

pub(super) fn f_map_keymap_internal(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    map_keymap_impl(i, &args, true)
}

pub(super) fn f_keymap_get_keyelt(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    // GNU Fkeymap__get_keyelt = get_keyelt(object, autoload): unwrap
    // `menu-item' forms and (STRING . DEFN) menu-name prefixes.
    Ok(crate::editor::keyelt_value(i, &args[0]))
}

pub(super) fn f_describe_bindings(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU Fdescribe_buffer_bindings
    // (keymap.c): drive `help--describe-map-tree' over translation maps,
    // overriding/minor/major/local/global maps, then the input-decode maps.
    // Rendering lives in the Lisp helper; the subr only walks the sources.
    let buffer = crate::buffer::primitives::buf_of(i, &a[0])?;
    let prefix = a.get(1).cloned().unwrap_or(Value::Nil);
    let nomenu = if a.get(2).cloned().unwrap_or(Value::Nil).is_nil() {
        Value::t()
    } else {
        Value::Nil
    };
    let qt = Value::t();
    let qnil = Value::Nil;
    let bufv = Value::Buffer(buffer.clone());
    macro_rules! call_tree {
        ($map:expr, $partial:expr, $shadow:expr, $title:expr, $transl:expr, $always:expr, $mention:expr) => {{
            let fun = Value::Sym(i.q("help--describe-map-tree"));
            i.apply(
                &fun,
                vec![
                    $map,
                    $partial,
                    $shadow,
                    prefix.clone(),
                    Value::string($title),
                    nomenu.clone(),
                    $transl,
                    $always,
                    $mention,
                    bufv.clone(),
                ],
            )?;
        }};
    }
    // Key translations map.
    let ktm = i.symbol_value(i.q_soft("key-translation-map").unwrap_or(u32::MAX));
    if crate::editor::is_keymap(i, &ktm) {
        call_tree!(
            ktm,
            qnil.clone(),
            qnil.clone(),
            "Key translations",
            qt.clone(),
            qt.clone(),
            qnil.clone()
        );
    }
    // Overriding maps.
    let mut shadow = Value::Nil;
    let otlm = i.symbol_value(
        i.q_soft("overriding-terminal-local-map")
            .unwrap_or(u32::MAX),
    );
    let olm = i.symbol_value(i.q_soft("overriding-local-map").unwrap_or(u32::MAX));
    for start1 in [otlm, olm] {
        if crate::editor::is_keymap(i, &start1) {
            call_tree!(
                start1.clone(),
                qt.clone(),
                shadow.clone(),
                "\x0c\nOverriding Bindings",
                qnil.clone(),
                qnil.clone(),
                qnil.clone()
            );
            shadow = Value::cons(start1, shadow);
        }
    }
    if shadow.is_nil() {
        // Minor-mode maps (evaluated with BUFFER current so minor-mode
        // predicates resolve there), then the buffer's local map.
        let bid = buffer.borrow().id;
        let save = i.current_buffer;
        i.set_current_buffer(bid);
        let minors = crate::editor::current_minor_maps(i);
        i.set_current_buffer(save);
        if let Ok(minors) = minors {
            for (mode, map) in minors {
                if !crate::editor::is_keymap(i, &map) {
                    continue;
                }
                let name = match &mode {
                    Value::Sym(s) => i.obarray.name(*s).to_string(),
                    _ => continue,
                };
                call_tree!(
                    map.clone(),
                    qt.clone(),
                    shadow.clone(),
                    format!("\x0c\n‘{}’ Minor Mode Bindings", name),
                    qnil.clone(),
                    qnil.clone(),
                    qnil.clone()
                );
                shadow = Value::cons(map, shadow);
            }
        }
        let lk = {
            let bb = buffer.borrow();
            bb.locals
                .get(&i.q_soft("local-keymap").unwrap_or(u32::MAX))
                .cloned()
                .unwrap_or(Value::Nil)
        };
        if crate::editor::is_keymap(i, &lk) {
            let major = {
                let bb = buffer.borrow();
                bb.locals
                    .get(&i.q_soft("major-mode").unwrap_or(u32::MAX))
                    .cloned()
                    .unwrap_or(Value::Nil)
            };
            let name = match &major {
                Value::Sym(s) => i.obarray.name(*s).to_string(),
                _ => "fundamental-mode".to_string(),
            };
            call_tree!(
                lk.clone(),
                qt.clone(),
                shadow.clone(),
                format!("\x0c\n‘{}’ Major Mode Bindings", name),
                qnil.clone(),
                qnil.clone(),
                qnil.clone()
            );
            shadow = Value::cons(lk, shadow);
        }
    }
    // Global map.
    let gm = i.symbol_value(i.q_soft("global-map").unwrap_or(u32::MAX));
    if crate::editor::is_keymap(i, &gm) {
        call_tree!(
            gm,
            qt.clone(),
            shadow.clone(),
            "\x0c\nGlobal Bindings",
            qnil.clone(),
            qt.clone(),
            qnil.clone()
        );
    }
    // Function key map + input decode map translations.
    for (var, title) in [
        (
            "local-function-key-map",
            "\x0c\nFunction key map translations",
        ),
        ("input-decode-map", "\x0c\nInput decoding map translations"),
    ] {
        let m = i.symbol_value(i.intern_soft(var).unwrap_or(u32::MAX));
        if crate::editor::is_keymap(i, &m) {
            call_tree!(
                m,
                qnil.clone(),
                qnil.clone(),
                title,
                qt.clone(),
                qnil.clone(),
                qnil.clone()
            );
        }
    }
    Ok(Value::Nil)
}

// ---------- keymaps ----------

pub(super) fn f_make_composed_keymap(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU subr.el:
    // `(keymap ,@(if (keymapp maps) (list maps) maps) ,@parent)' — MAPS
    // is spliced ELEMENT BY ELEMENT (strings and vectors yield their
    // elements, e.g. "a" → 97) unless it resolves to a keymap itself;
    // PARENT is appended verbatim as the (possibly dotted) tail.
    fn splice_seq(i: &mut Interp, v: &Value, into: &mut Vec<Value>) -> EvalResult {
        match v {
            Value::Str(s) => {
                // GNU string_to_char_array: a unibyte string yields its
                // raw bytes as chars 0-255; a multibyte string yields
                // real codepoints (eight-bit chars = 0x3FFF80+b).  The
                // PUA proxy chars map back via `eight_bit_byte' (byte
                // value) or `lisp_char_code' (full eight-bit code).
                if i.is_unibyte_str(s) {
                    for ch in s.borrow().chars() {
                        into.push(Value::Int(
                            crate::lisp::value::eight_bit_byte(ch)
                                .map(|b| b as i128)
                                .unwrap_or_else(|| (ch).code() as i128),
                        ));
                    }
                } else {
                    for ch in s.borrow().chars() {
                        into.push(Value::Int(crate::lisp::value::lisp_char_code(ch)));
                    }
                }
            }
            Value::Vec(vec) => {
                into.extend(vec.borrow().iter().cloned());
            }
            Value::Nil => {}
            // GNU `mapcar' over MAPS: non-list atoms (symbols, ints,
            // floats, buffers…) signal `sequencep'.
            Value::Int(_) | Value::Float(_) | Value::Sym(_) => {
                return Err(i.wrong_type_mut("sequencep", v));
            }
            other => match other.list_to_vec() {
                Ok(items) => into.extend(items),
                Err(crate::lisp::value::ListError::Circular(_)) => {
                    return Err(i.error("List contains a loop"));
                }
                Err(crate::lisp::value::ListError::Dotted(tail)) => {
                    return Err(i.wrong_type_mut("listp", &tail));
                }
            },
        }
        Ok(Value::Nil)
    }
    let m0 = crate::editor::keymap_def(i, a[0].clone())?;
    let mut elems = Vec::new();
    if is_keymap(i, &m0) {
        elems.push(m0);
    } else {
        splice_seq(i, &m0, &mut elems)?;
    }
    let mut tail = arg(&a, 1).clone();
    for m in elems.iter().rev() {
        tail = Value::cons(m.clone(), tail);
    }
    Ok(Value::cons(Value::Sym(i.q("keymap")), tail))
}

pub(super) fn f_current_active_maps(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU Fcurrent_active_maps: (otlp) (keymap-prop) minors local global;
    // with OLP set and no otlp, overriding-local-map REPLACES all of
    // the buffer-dependent maps.  POSITION goes through keymap.c's
    // `click_position': an integer/marker must sit inside BEGV..ZV of the
    // current buffer — out of range signals
    // (args-out-of-range BUFFER POSITION); other objects use PT.
    let olp = a.first().map(|v| v.truthy()).unwrap_or(false);
    if let Some(pos) = a.get(1) {
        let pt: Option<i128> = match pos {
            Value::Int(n) => Some(*n),
            Value::Marker(m) => {
                let m = m.borrow();
                if m.buffer.is_none() {
                    return Err(i.error("Marker does not point anywhere"));
                }
                Some(m.position as i128 + 1)
            }
            _ => None,
        };
        if let Some(p) = pt {
            let cur = crate::editor::cur(i);
            let bb = cur.borrow();
            if p < bb.begv as i128 + 1 || p > bb.zv as i128 + 1 {
                drop(bb);
                return Err(err_sym(
                    i,
                    "args-out-of-range",
                    vec![Value::Buffer(cur), pos.clone()],
                ));
            }
        }
    }
    let otlp = i.symbol_value(i.q_soft("overriding-terminal-local-map").unwrap_or(0));
    let olmap = i.symbol_value(i.q_soft("overriding-local-map").unwrap_or(0));
    let gmap = i.symbol_value(i.q_soft("global-map").unwrap_or(0));

    let mut maps = Vec::new();
    if is_keymap(i, &gmap) {
        maps.push(gmap);
    }
    if olp && !is_keymap(i, &otlp) && is_keymap(i, &olmap) {
        maps.insert(0, olmap);
        return Ok(Value::list(maps));
    }

    let local = {
        let b = crate::editor::cur(i);
        let lb = b.borrow();
        lb.locals
            .get(&i.q_soft("local-keymap").unwrap_or(u32::MAX))
            .cloned()
            .unwrap_or(Value::Nil)
    };
    if is_keymap(i, &local) {
        maps.insert(0, local);
    }
    // Minor-mode maps precede the local map.
    let minors = crate::editor::current_minor_maps(i)?
        .into_iter()
        .filter(|(_, m)| is_keymap(i, m))
        .map(|(_, m)| m)
        .collect::<Vec<_>>();
    for m in minors.into_iter().rev() {
        maps.insert(0, m);
    }
    if olp && is_keymap(i, &otlp) {
        maps.insert(0, otlp);
    }
    Ok(Value::list(maps))
}

pub(super) fn f_suppress_keymap(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    if !is_keymap(i, &a[0]) {
        return Err(i.wrong_type_mut("keymapp", &a[0]));
    }
    // GNU remaps self-insert-command, preserving explicit bindings.
    let nodigits = arg(&a, 1).truthy();
    let define_key = Value::Sym(i.q("define-key"));
    let keys = Value::vector(vec![
        Value::Sym(i.q("remap")),
        Value::Sym(i.q("self-insert-command")),
    ]);
    let undefined = Value::Sym(i.q("undefined"));
    i.apply(&define_key, vec![a[0].clone(), keys, undefined])?;
    if !nodigits {
        let dig = Value::Sym(i.q("digit-argument"));
        for code in '0' as i128..='9' as i128 {
            crate::editor::set_binding(i, &a[0], code, dig.clone());
        }
        let neg = Value::Sym(i.q("negative-argument"));
        crate::editor::set_binding(i, &a[0], '-' as i128, neg);
    }
    // GNU returns the NODIGITS argument (nil by default).
    Ok(arg(&a, 1))
}

pub(super) fn f_set_this_command_keys(_i: &mut Interp, _a: crate::lisp::Argv) -> EvalResult {
    Ok(Value::Nil)
}
