//! Hash table subrs.

use super::{S, arg};
use crate::lisp::Interp;
use crate::lisp::error::{EvalResult, Flow};
use crate::lisp::obarray::sym;
use crate::lisp::value::{HashKey, HashRef, HashTest, LispHash, Subr, Value};
use std::cell::RefCell;
use std::rc::Rc;

pub(crate) static SUBRS: &[Subr] = &[
    S!("make-hash-table", many 0, f_make_hash_table, "Create a hash table."),
    S!("gethash", 2, 3, f_gethash, "Look up KEY in TABLE."),
    S!("puthash", 3, 3, f_puthash, "Set KEY in TABLE to VALUE."),
    S!("remhash", 2, 2, f_remhash, "Remove KEY from TABLE."),
    S!("clrhash", 1, 1, f_clrhash, "Remove all entries from TABLE."),
    S!(
        "hash-table-count",
        1,
        1,
        f_hash_table_count,
        "Number of entries in TABLE."
    ),
    S!(
        "hash-table-keys",
        1,
        1,
        f_hash_table_keys,
        "Keys of TABLE as a list."
    ),
    S!(
        "hash-table-values",
        1,
        1,
        f_hash_table_values,
        "Values of TABLE as a list."
    ),
    S!(
        "hash-table-test",
        1,
        1,
        f_hash_table_test,
        "Test function of TABLE."
    ),
    S!(
        "hash-table-weakness",
        1,
        1,
        f_hash_table_weakness,
        "Weakness of TABLE."
    ),
    S!(
        "hash-table-rehash-size",
        1,
        1,
        f_hash_table_rehash_size,
        "Rehash size."
    ),
    S!(
        "hash-table-rehash-threshold",
        1,
        1,
        f_hash_table_rehash_threshold,
        "Rehash threshold."
    ),
    S!(
        "hash-table-size",
        1,
        1,
        f_hash_table_size,
        "Current size of TABLE."
    ),
    S!("copy-hash-table", 1, 1, f_copy_hash_table, "Copy TABLE."),
    S!(
        "define-hash-table-test",
        3,
        3,
        f_define_hash_table_test,
        "Define a hash table comparison and hash function."
    ),
    S!(
        "internal--hash-table-buckets",
        1,
        1,
        f_hash_table_buckets,
        "Internal: bucket vector of TABLE."
    ),
    S!(
        "internal--hash-table-index-size",
        1,
        1,
        f_hash_table_index_size,
        "Internal: index size of TABLE."
    ),
    S!(
        "internal--hash-table-histogram",
        1,
        1,
        f_hash_table_histogram,
        "Internal: bucket histogram."
    ),
];

/// Normalize a key for the table's test.
fn key_identity(value: &Value) -> usize {
    match value {
        Value::Cons(value) => Rc::as_ptr(value) as usize,
        Value::Str(value) => Rc::as_ptr(value) as usize,
        Value::Vec(value) | Value::Record(value) => Rc::as_ptr(value) as usize,
        Value::Lambda(value) => Rc::as_ptr(value) as usize,
        Value::Marker(value) => Rc::as_ptr(value) as usize,
        _ => unreachable!("a positioned key has a heap root"),
    }
}

fn list_spine_borrow_conflict(mut tail: Value) -> bool {
    let mut seen = std::collections::HashSet::new();
    loop {
        let Value::Cons(cell) = tail else {
            return false;
        };
        if !seen.insert(Rc::as_ptr(&cell) as usize) {
            return false;
        }
        let Ok(contents) = cell.try_borrow() else {
            return true;
        };
        tail = contents.cdr.clone();
    }
}

/// Whether reading the slots used by `equal_key' would conflict with a
/// live Rust mutable borrow. This is checked before the normalizer's fast
/// raw `borrow' accesses so table operations can signal instead of panic.
fn equal_key_borrow_conflict(value: &Value) -> bool {
    match value {
        Value::Cons(cell) => cell.try_borrow().is_err(),
        Value::Str(string) => string.try_borrow().is_err(),
        Value::Vec(vector) => vector.try_borrow().is_err(),
        Value::Record(record) => record.try_borrow().is_err(),
        Value::Marker(marker) => marker.try_borrow().is_err(),
        Value::Lambda(lambda) => {
            if let Some(items) = &lambda.bc_items {
                return items.try_borrow().is_err();
            }
            let environment_conflict = lambda.env_value.is_none()
                && lambda
                    .env
                    .as_ref()
                    .is_some_and(|frame| frame.raw.try_borrow().is_err());
            let interactive_conflict = lambda.interactive.as_ref().is_some_and(|form| match form {
                Value::Cons(cell) => cell
                    .try_borrow()
                    .map(|form| list_spine_borrow_conflict(form.cdr.clone()))
                    .unwrap_or(true),
                _ => false,
            });
            environment_conflict || interactive_conflict
        }
        _ => false,
    }
}

pub(crate) fn hash_key_for(interp: &Interp, v: &Value, test: HashTest) -> HashKey {
    hash_key_for_inner(interp, v, test).unwrap_or_else(|_| match test {
        HashTest::Eq | HashTest::Eql => HashKey::Ptr(key_identity(v)),
        HashTest::Equal => HashKey::EqualFallback(0, key_identity(v)),
    })
}

pub(crate) fn hash_key_for_checked(
    interp: &Interp,
    v: &Value,
    test: HashTest,
) -> Result<HashKey, Flow> {
    hash_key_for_inner(interp, v, test)
}

fn hash_key_for_inner(interp: &Interp, v: &Value, test: HashTest) -> Result<HashKey, Flow> {
    let _scope = interp.symbol_position_scope();
    let mut positioned = false;
    // `eq_key' still recognizes symbol positions through their record
    // wrapper. Check that wrapper before raw position inspection.
    if !matches!(test, HashTest::Equal)
        && matches!(v, Value::Record(_))
        && equal_key_borrow_conflict(v)
    {
        return Err(interp.error("Cannot hash a mutably borrowed Lisp object"));
    }
    let mut borrow_conflict = false;
    let normalized = match test {
        HashTest::Eq => {
            positioned = super::misc::raw_symbol_position(v).is_some();
            eq_key(v)
        }
        HashTest::Eql => {
            positioned = super::misc::raw_symbol_position(v).is_some();
            eql_key(v)
        }
        HashTest::Equal => {
            let mut ancestors = std::collections::HashSet::new();
            let normalized = equal_key(
                interp,
                v,
                &mut ancestors,
                &mut 4096,
                0,
                &mut positioned,
                &mut borrow_conflict,
            );
            if borrow_conflict {
                return Err(interp.error("Cannot hash a mutably borrowed Lisp object"));
            }
            match normalized {
                Some(normalized) => normalized,
                None => HashKey::EqualFallback(
                    super::strfn::equal_hash_code(interp, v)?,
                    key_identity(v),
                ),
            }
        }
    };
    Ok(if positioned {
        HashKey::Positioned(
            super::strfn::builtin_hash_code(interp, v, test)?,
            key_identity(v),
        )
    } else {
        normalized
    })
}

fn eq_key(v: &Value) -> HashKey {
    // `symbols-with-pos-enabled' objects hash as their bare symbol
    // (they are `eq' to it, so the hash must agree).
    if let Some((s, _)) = super::misc::as_sympos(v) {
        return if s == 0 {
            HashKey::Nil
        } else {
            HashKey::Sym(s)
        };
    }
    match v {
        Value::Nil => HashKey::Nil,
        Value::Int(n) => HashKey::Int(*n),
        Value::BigInt(n) => HashKey::Ptr(Rc::as_ptr(n) as usize),
        // GNU: floats are heap objects; `eq' is identity.
        // Two literal `1.0's must NOT hash equal — key on the Rc.
        Value::Float(f) => HashKey::Ptr(Rc::as_ptr(f) as usize),
        Value::Sym(s) => HashKey::Sym(*s),
        Value::Cons(c) => HashKey::Ptr(Rc::as_ptr(c) as usize),
        Value::Str(s) => HashKey::Ptr(Rc::as_ptr(s) as usize),
        Value::Vec(s) => HashKey::Ptr(Rc::as_ptr(s) as usize),
        Value::Record(s) => HashKey::Ptr(Rc::as_ptr(s) as usize),
        Value::Hash(s) => HashKey::Ptr(Rc::as_ptr(s) as usize),
        Value::Subr(s) => HashKey::Ptr(*s as *const _ as usize),
        Value::Lambda(s) => HashKey::Ptr(Rc::as_ptr(s) as usize),
        Value::Buffer(s) => HashKey::Ptr(Rc::as_ptr(s) as usize),
        Value::Marker(s) => HashKey::Ptr(Rc::as_ptr(s) as usize),
        Value::Window(s) => HashKey::Ptr(Rc::as_ptr(s) as usize),
        Value::Frame(s) => HashKey::Ptr(Rc::as_ptr(s) as usize),
        Value::Process(s) => HashKey::Ptr(Rc::as_ptr(s) as usize),
        Value::Thread(s) => HashKey::Ptr(Rc::as_ptr(s) as usize),
        Value::Mutex(s) => HashKey::Ptr(Rc::as_ptr(s) as usize),
        Value::CondVar(s) => HashKey::Ptr(Rc::as_ptr(s) as usize),
        Value::Finalizer(s) => HashKey::Ptr(Rc::as_ptr(s) as usize),
    }
}

fn eql_key(v: &Value) -> HashKey {
    // eql: numbers compare by value, everything else like eq.
    match v {
        Value::Int(n) => HashKey::Int(*n),
        Value::BigInt(n) => match v.int() {
            Some(n) => HashKey::Int(n),
            None => HashKey::BigInt((**n).clone()),
        },
        Value::Float(f) => HashKey::Float(f.to_bits()),
        _ => eq_key(v),
    }
}

fn equal_key(
    interp: &Interp,
    v: &Value,
    ancestors: &mut std::collections::HashSet<usize>,
    budget: &mut usize,
    depth: usize,
    positioned: &mut bool,
    borrow_conflict: &mut bool,
) -> Option<HashKey> {
    if equal_key_borrow_conflict(v) {
        *borrow_conflict = true;
        return None;
    }
    // Normalization is only a fast path, never proof that cyclic keys
    // differ. Bound both recursion and repeated expansion of shared DAGs.
    // A fallback hashes a bounded unfolding and collision-checks `equal'.
    if *budget == 0 || depth > 128 {
        return None;
    }
    *budget -= 1;
    if super::misc::raw_symbol_position(v).is_some() {
        *positioned = true;
        return Some(eq_key(v));
    }
    let ptr = match v {
        Value::Cons(c) => Some(Rc::as_ptr(c) as usize),
        Value::Vec(c) => Some(Rc::as_ptr(c) as usize),
        Value::Record(c) => Some(Rc::as_ptr(c) as usize),
        Value::Lambda(c) => Some(Rc::as_ptr(c) as usize),
        _ => None,
    };
    if let Some(p) = ptr {
        if !ancestors.insert(p) {
            return None;
        }
    }
    // Overlay contents live outside the handle vector and may be cyclic.
    // Use the existing bounded sxhash bucket + equal collision comparison.
    if interp.is_overlay(v) {
        if let Some(overlay) = interp.overlay_state(v) {
            let _ = equal_key(
                interp,
                &overlay.plist,
                ancestors,
                budget,
                depth + 1,
                positioned,
                borrow_conflict,
            );
            if *borrow_conflict {
                return None;
            }
        }
        return None;
    }
    if super::data::is_obarray(interp, v) {
        let identity = ptr?;
        return Some(HashKey::Tagged(4, vec![HashKey::Ptr(identity)]));
    }
    let out = match v {
        Value::Str(s) => HashKey::Str(
            s.borrow().clone(),
            super::strfn::string_storage_bytes(interp, s),
        ),
        Value::Marker(m) => {
            let m = m.borrow();
            // Match equal_values, including detached markers. Insertion
            // type is deliberately ignored; eq/eql retain pointer identity.
            HashKey::Marker(m.buffer, if m.buffer.is_some() { m.position } else { 0 })
        }
        // Enabled sympos are `equal' to their bare symbol — same key
        // as the plain symbol arm below.
        _ if super::misc::active_sympos_parts(interp, v).is_some() => {
            let (s, _) = super::misc::active_sympos_parts(interp, v).unwrap();
            HashKey::Sym(s)
        }
        Value::Cons(c) => {
            let b = c.borrow();
            HashKey::Cons(
                Box::new(equal_key(
                    interp,
                    &b.car,
                    ancestors,
                    budget,
                    depth + 1,
                    positioned,
                    borrow_conflict,
                )?),
                Box::new(equal_key(
                    interp,
                    &b.cdr,
                    ancestors,
                    budget,
                    depth + 1,
                    positioned,
                    borrow_conflict,
                )?),
            )
        }
        Value::Vec(vec) => HashKey::Vec(
            vec.borrow()
                .iter()
                .map(|x| {
                    equal_key(
                        interp,
                        x,
                        ancestors,
                        budget,
                        depth + 1,
                        positioned,
                        borrow_conflict,
                    )
                })
                .collect::<Option<Vec<_>>>()?,
        ),
        // `equal' compares records elementwise — hash contents too,
        // tagged so records never collide with plain vectors.
        Value::Record(_) if v.record_kind() == Some(crate::lisp::value::RecordKind::Obarray) => {
            eq_key(v)
        }
        Value::Record(r) => HashKey::Tagged(
            match v.record_kind() {
                Some(crate::lisp::value::RecordKind::BoolVector) => 4,
                Some(crate::lisp::value::RecordKind::CharTable) => 5,
                Some(crate::lisp::value::RecordKind::Obarray) => 6,
                Some(crate::lisp::value::RecordKind::FontSpec) => 7,
                Some(crate::lisp::value::RecordKind::SymbolWithPos) => 8,
                Some(crate::lisp::value::RecordKind::SubCharTable) => 9,
                None => 1,
            },
            r.borrow()
                .iter()
                .map(|x| {
                    equal_key(
                        interp,
                        x,
                        ancestors,
                        budget,
                        depth + 1,
                        positioned,
                        borrow_conflict,
                    )
                })
                .collect::<Option<Vec<_>>>()?,
        ),
        Value::Lambda(l) => {
            if let Some(items) = &l.bc_items {
                let mut keys = vec![HashKey::Int(l.env.is_none() as i128)];
                keys.extend(
                    items
                        .borrow()
                        .iter()
                        .map(|x| {
                            equal_key(
                                interp,
                                x,
                                ancestors,
                                budget,
                                depth + 1,
                                positioned,
                                borrow_conflict,
                            )
                        })
                        .collect::<Option<Vec<_>>>()?,
                );
                HashKey::Tagged(2, keys)
            } else {
                // Interpreted lambdas use the same vector-like slots as
                // `equal_values'. In particular, include captured values,
                // docstrings, and interactive specs rather than only the
                // presence of a lexical environment.
                let mut keys = vec![HashKey::Int(l.is_macro as i128)];
                let slots = match super::misc::lambda_slot_values_checked(l) {
                    Ok(slots) => slots,
                    Err(_) => {
                        *borrow_conflict = true;
                        return None;
                    }
                };
                keys.extend(
                    slots
                        .iter()
                        .map(|slot| {
                            equal_key(
                                interp,
                                slot,
                                ancestors,
                                budget,
                                depth + 1,
                                positioned,
                                borrow_conflict,
                            )
                        })
                        .collect::<Option<Vec<_>>>()?,
                );
                HashKey::Tagged(3, keys)
            }
        }
        _ => eql_key(v),
    };
    if let Some(p) = ptr {
        ancestors.remove(&p);
    }
    Some(out)
}

fn ensure_equal_key_borrowable(interp: &Interp, value: &Value) -> Result<(), Flow> {
    let mut pending = vec![value.clone()];
    let mut seen = std::collections::HashSet::new();
    while let Some(value) = pending.pop() {
        let identity = match &value {
            Value::Cons(cell) => Some(Rc::as_ptr(cell) as usize),
            Value::Str(string) => Some(Rc::as_ptr(string) as usize),
            Value::Vec(vector) | Value::Record(vector) => Some(Rc::as_ptr(vector) as usize),
            Value::Hash(hash) => Some(Rc::as_ptr(hash) as usize),
            Value::Lambda(function) => Some(Rc::as_ptr(function) as usize),
            Value::Buffer(buffer) => Some(Rc::as_ptr(buffer) as usize),
            Value::Marker(marker) => Some(Rc::as_ptr(marker) as usize),
            Value::Window(window) => Some(Rc::as_ptr(window) as usize),
            Value::Frame(frame) => Some(Rc::as_ptr(frame) as usize),
            Value::Process(process) => Some(Rc::as_ptr(process) as usize),
            Value::Thread(thread) => Some(Rc::as_ptr(thread) as usize),
            Value::Mutex(mutex) => Some(Rc::as_ptr(mutex) as usize),
            Value::CondVar(condition) => Some(Rc::as_ptr(condition) as usize),
            Value::Finalizer(finalizer) => Some(Rc::as_ptr(finalizer) as usize),
            _ => None,
        };
        if identity.is_some_and(|identity| !seen.insert(identity)) {
            continue;
        }

        match value {
            Value::Cons(cell) => {
                let (car, cdr) = cell
                    .try_borrow()
                    .map(|cell| (cell.car.clone(), cell.cdr.clone()))
                    .map_err(|_| interp.error("Cannot hash a mutably borrowed Lisp object"))?;
                pending.push(car);
                pending.push(cdr);
            }
            Value::Str(string) => {
                let _contents = string
                    .try_borrow()
                    .map_err(|_| interp.error("Cannot hash a mutably borrowed Lisp object"))?;
                for (_, _, plist) in interp.str_props(&string) {
                    pending.extend(plist.iter().cloned());
                }
            }
            Value::Vec(vector) | Value::Record(vector) => {
                let items = vector
                    .try_borrow()
                    .map(|items| items.clone())
                    .map_err(|_| interp.error("Cannot hash a mutably borrowed Lisp object"))?;
                pending.extend(items);
            }
            Value::Hash(hash) => {
                let (keys, values) = {
                    let table = hash
                        .try_borrow()
                        .map_err(|_| interp.error("Cannot hash a mutably borrowed Lisp object"))?;
                    (
                        table
                            .keys
                            .iter()
                            .map(|(_, original)| original.clone())
                            .collect::<Vec<_>>(),
                        table.map.values().cloned().collect::<Vec<_>>(),
                    )
                };
                pending.extend(keys);
                pending.extend(values);
            }
            Value::Lambda(function) => {
                if let Some(items) = &function.bc_items {
                    let items = items
                        .try_borrow()
                        .map(|items| items.clone())
                        .map_err(|_| interp.error("Cannot hash a mutably borrowed Lisp object"))?;
                    pending.extend(items);
                } else {
                    pending.extend(function.arglist.iter().cloned());
                    pending.extend(function.body.iter().cloned());
                    pending.extend(function.body_value.iter().cloned());
                    pending.extend(function.doc_value.iter().cloned());
                    pending.extend(function.interactive.iter().cloned());
                    for optional in &function.optional {
                        pending.extend(optional.default.iter().cloned());
                    }
                    if let Some(environment) = &function.env_value {
                        pending.push(environment.clone());
                    }
                }
                if let Some(environment) = &function.env {
                    let raw = environment
                        .raw
                        .try_borrow()
                        .map(|raw| raw.clone())
                        .map_err(|_| interp.error("Cannot hash a mutably borrowed Lisp object"))?;
                    pending.push(raw);
                }
            }
            Value::Marker(marker) => {
                let _marker = marker
                    .try_borrow()
                    .map_err(|_| interp.error("Cannot hash a mutably borrowed Lisp object"))?;
            }
            Value::Buffer(buffer) => {
                let _buffer = buffer
                    .try_borrow()
                    .map_err(|_| interp.error("Cannot hash a mutably borrowed Lisp object"))?;
            }
            Value::Window(window) => {
                let _window = window
                    .try_borrow()
                    .map_err(|_| interp.error("Cannot hash a mutably borrowed Lisp object"))?;
            }
            Value::Frame(frame) => {
                let _frame = frame
                    .try_borrow()
                    .map_err(|_| interp.error("Cannot hash a mutably borrowed Lisp object"))?;
            }
            Value::Process(process) => {
                let _process = process
                    .try_borrow()
                    .map_err(|_| interp.error("Cannot hash a mutably borrowed Lisp object"))?;
            }
            Value::Thread(thread) => {
                let _thread = thread
                    .try_borrow()
                    .map_err(|_| interp.error("Cannot hash a mutably borrowed Lisp object"))?;
            }
            Value::Mutex(mutex) => {
                let _mutex = mutex
                    .try_borrow()
                    .map_err(|_| interp.error("Cannot hash a mutably borrowed Lisp object"))?;
            }
            Value::CondVar(condition) => {
                let _condition = condition
                    .try_borrow()
                    .map_err(|_| interp.error("Cannot hash a mutably borrowed Lisp object"))?;
            }
            Value::Finalizer(finalizer) => {
                let function = finalizer
                    .try_borrow()
                    .map(|function| function.clone())
                    .map_err(|_| interp.error("Cannot hash a mutably borrowed Lisp object"))?;
                pending.push(function);
            }
            _ => {}
        }
    }
    Ok(())
}

pub(crate) fn f_make_hash_table(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    // (make-hash-table &rest KEYWORD-ARGS)
    // — GNU scans the pairs backwards: the *last* pair's keyword is
    // validated first, and the *first* occurrence of a keyword wins.
    // Only after the scan does it validate :test, then :size, then
    // :weakness — so an unknown keyword signals before any value does.
    let items = &args[..];
    if items.len() % 2 != 0 {
        return Err(i.error("Odd number of arguments"));
    }
    let mut test_arg = Value::Nil;
    let mut size_arg = Value::Nil;
    let mut weakness_arg = Value::Nil;
    for pair in items.chunks_exact(2).rev() {
        let kw = &pair[0];
        let arg = &pair[1];
        let kw_id = i.sym_id(kw);
        if kw_id == Some(i.q(":test")) {
            test_arg = arg.clone();
        } else if kw_id == Some(i.q(":weakness")) {
            weakness_arg = arg.clone();
        } else if kw_id == Some(i.q(":size")) {
            size_arg = arg.clone();
        } else if kw_id == Some(i.q(":rehash-threshold"))
            || kw_id == Some(i.q(":rehash-size"))
            || kw_id == Some(i.q(":purecopy"))
        {
        } else {
            // GNU signal_error("Invalid keyword argument", KW):
            // a proper-list KW becomes the DATA tail (elements spliced
            // into the error list); anything else is wrapped in list1.
            let mut data = vec![Value::string("Invalid keyword argument")];
            match kw {
                Value::Nil => {} // nil is a proper empty list → splices to nothing
                Value::Cons(_) if kw.list_to_vec().is_ok() => {
                    data.extend(kw.list_to_vec().unwrap());
                }
                _ => data.push(kw.clone()),
            }
            return Err(i.signal_data(sym::ERROR, data));
        }
    }
    // GNU validates the collected :test value: non-symbols signal
    // `symbolp'; unknown symbols consult `define-hash-table-test'
    // (the `hash-table-test' property) and otherwise signal
    // "Invalid hash table test".
    let (test, test_name, user_test) = {
        let test_id = match &test_arg {
            Value::Nil => None,
            v => match i.sym_id(v) {
                Some(id) => Some(id),
                None => return Err(i.wrong_type_mut("symbolp", v)),
            },
        };
        let eql_test = i.intern_soft("eql");
        let eq_test = i.intern_soft("eq");
        let equal_test = i.intern_soft("equal");
        match test_id {
            None => (HashTest::Eql, Value::Nil, None),
            Some(id) if Some(id) == eql_test => (HashTest::Eql, Value::Nil, None),
            Some(id) if Some(id) == eq_test => (HashTest::Eq, Value::Nil, None),
            Some(id) if Some(id) == equal_test => (HashTest::Equal, Value::Nil, None),
            Some(id) => {
                let prop = i.q("hash-table-test");
                let definition = i.get_prop(id, prop);
                if let Value::Cons(first) = &definition
                    && let Value::Cons(second) = &first.borrow().cdr
                {
                    // Capture the two function objects, not the property's
                    // conses: redefining the test affects only new tables.
                    let functions = (first.borrow().car.clone(), second.borrow().car.clone());
                    (HashTest::Eql, test_arg.clone(), Some(functions))
                } else {
                    // GNU: (error "Invalid hash table test" VAL)
                    return Err(i.signal_data(
                        sym::ERROR,
                        vec![Value::string("Invalid hash table test"), test_arg.clone()],
                    ));
                }
            }
        }
    };
    let size: i128 = match &size_arg {
        Value::Nil => 0,
        Value::Int(n) if *n >= 0 => *n,
        // GNU: (error "Invalid hash table size" VAL)
        other => {
            return Err(i.signal_data(
                sym::ERROR,
                vec![Value::string("Invalid hash table size"), other.clone()],
            ));
        }
    };
    let weakness: Option<Value> = match &weakness_arg {
        Value::Nil => None,
        v => {
            let Some(id) = i.sym_id(v) else {
                return Err(i.signal_data(
                    sym::ERROR,
                    vec![Value::string("Invalid hash table weakness"), v.clone()],
                ));
            };
            let key = i.intern_soft("key");
            let value = i.intern_soft("value");
            let key_and_value = i.intern_soft("key-and-value");
            let key_or_value = i.intern_soft("key-or-value");
            // GNU: `t' is a synonym for `key-and-value'.
            if id == sym::T || Some(id) == key_and_value {
                Some(Value::Sym(i.q("key-and-value")))
            } else if Some(id) == key || Some(id) == value || Some(id) == key_or_value {
                Some(Value::Sym(id))
            } else {
                // GNU: (error "Invalid hash table weakness" VAL)
                return Err(i.signal_data(
                    sym::ERROR,
                    vec![Value::string("Invalid hash table weakness"), v.clone()],
                ));
            }
        }
    };
    let rehash_size = Value::float(1.5);
    let rehash_threshold = Value::float(0.8125);
    let mut h = LispHash::new(test);
    h.gc_weakness = crate::lisp::value::HashWeakness::from_value(i, &weakness);
    h.weakness = weakness;
    h.size = size;
    h.rehash_size = rehash_size;
    h.rehash_threshold = rehash_threshold;
    h.test_name = test_name;
    h.user_test = user_test;
    Ok(Value::Hash(Rc::new(RefCell::new(h))).tracked())
}

/// Never retain a RefCell borrow across Lisp: callbacks can inspect the
/// table or perform recursive lookups. Restore its mutability on every
/// Lisp exit path, including signals and throws, as GNU does.
fn user_call(i: &mut Interp, h: &HashRef, function: &Value, args: Vec<Value>) -> EvalResult {
    let mutable = h.borrow().mutable;
    h.borrow_mut().mutable = false;
    let result = i.apply(function, args);
    h.borrow_mut().mutable = mutable;
    result
}

fn check_mutable(i: &mut Interp, h: &HashRef) -> Result<(), Flow> {
    if h.borrow().mutable {
        Ok(())
    } else {
        Err(i.signal_data(
            sym::ERROR,
            vec![
                Value::string("hash table test modifies table"),
                Value::Hash(h.clone()),
            ],
        ))
    }
}

/// Return the matching entry's key, or a fresh key suitable for insertion.
/// A normalized key is sufficient for small acyclic built-in keys.
/// Larger/cyclic equal keys and custom tests need collision checking.
pub(crate) fn table_key(i: &mut Interp, h: &HashRef, key: &Value) -> Result<HashKey, Flow> {
    let mut scope = i.symbol_position_scope();
    let result = table_key_inner(i, h, key);
    scope.finish(i.positioned_symbols_enabled());
    result
}

fn hash_bucket(hash: u32, capacity: i128) -> u32 {
    let bits = ((capacity.max(0) as u128 + 1)
        .next_power_of_two()
        .trailing_zeros())
    .min(32);
    ((hash.wrapping_mul(2_654_435_769) as u64) >> (32 - bits)) as u32
}

fn table_key_inner(i: &mut Interp, h: &HashRef, key: &Value) -> Result<HashKey, Flow> {
    let (test, user_test) = {
        let table = h.borrow();
        (table.test, table.user_test.clone())
    };
    let Some((compare, hash_function)) = user_test else {
        let normalized = hash_key_for_checked(i, key, test)?;
        if matches!(normalized, HashKey::Positioned(..)) || h.borrow().positioned_keys != 0 {
            let hash = super::strfn::builtin_hash_code(i, key, test)?;
            let capacity = h.borrow().size;
            let bucket = hash_bucket(hash, capacity);
            let candidates: Vec<(HashKey, Value)> = {
                let table = h.borrow();
                table
                    .collision_order
                    .iter()
                    .rev()
                    .filter_map(|slot| table.keys.0[*slot].as_ref())
                    .cloned()
                    .collect()
            };
            for (stored_key, original) in candidates {
                let stored_hash = match &stored_key {
                    HashKey::Positioned(hash, _) | HashKey::EqualFallback(hash, _) => *hash,
                    _ => super::strfn::builtin_hash_code(i, &original, test)?,
                };
                if hash_bucket(stored_hash, capacity) != bucket {
                    continue;
                }
                // GNU tests EQ before requiring identical full hashes.
                if super::eq_values(key, &original) {
                    return Ok(stored_key);
                }
                if stored_hash != hash {
                    continue;
                }
                let matches = match test {
                    HashTest::Eq => false,
                    HashTest::Eql => super::eql_values(key, &original),
                    HashTest::Equal => super::equal_values(i, key, &original)?,
                };
                if matches {
                    return Ok(stored_key);
                }
            }
            return Ok(normalized);
        }
        if let HashKey::EqualFallback(hash, _) = &normalized {
            let candidates: Vec<(HashKey, Value)> = {
                let table = h.borrow();
                table
                    .collision_order
                    .iter()
                    .rev()
                    .filter_map(|slot| table.keys.0[*slot].as_ref())
                    .filter(|(k, _)| {
                        matches!(k, HashKey::EqualFallback(code, _)
                        if hash_bucket(*code, table.size) == hash_bucket(*hash, table.size))
                    })
                    .cloned()
                    .collect()
            };
            for (stored_key, original) in candidates {
                let HashKey::EqualFallback(stored_hash, _) = &stored_key else {
                    unreachable!()
                };
                if super::eq_values(key, &original)
                    || (*stored_hash == *hash && super::equal_values(i, key, &original)?)
                {
                    return Ok(stored_key);
                }
            }
        }
        return Ok(normalized);
    };
    // User-provided hash/comparison functions may read arbitrary parts of
    // KEY through Lisp primitives. Validate the reachable equal structure
    // before entering those callbacks so a Rust RefCell borrow cannot panic
    // inside user code.
    ensure_equal_key_borrowable(i, key)?;
    let hash_result = user_call(i, h, &hash_function, vec![key.clone()])?;
    let hash = super::strfn::user_hash_code(i, &hash_result)?;
    let candidates: Vec<(HashKey, Value)> = {
        let table = h.borrow();
        table
            .collision_order
            .iter()
            .rev()
            .filter_map(|slot| table.keys.0[*slot].as_ref())
            .filter(|(k, _)| {
                matches!(k, HashKey::Custom(code, _)
                if hash_bucket(*code, table.size) == hash_bucket(hash, table.size))
            })
            .cloned()
            .collect()
    };
    for (stored_key, original) in candidates {
        let HashKey::Custom(stored_hash, _) = &stored_key else {
            unreachable!()
        };
        if super::eq_values(key, &original)
            || (*stored_hash == hash && {
                ensure_equal_key_borrowable(i, &original)?;
                user_call(i, h, &compare, vec![key.clone(), original])?.truthy()
            })
        {
            return Ok(stored_key);
        }
    }
    let table = h.borrow();
    let slot = table
        .free_slots
        .last()
        .copied()
        .unwrap_or(table.keys.0.len());
    Ok(HashKey::Custom(hash, slot))
}

/// Look up a hash entry without using a Lisp default value as a sentinel.
/// `Some(nil)` is a present nil entry; `None` means the key is absent.
pub(crate) fn gethash_entry(
    i: &mut Interp,
    key: &Value,
    table: &Value,
) -> Result<Option<Value>, Flow> {
    let Value::Hash(hash) = table else {
        return Err(i.wrong_type_mut("hash-table-p", table));
    };
    let key = table_key(i, hash, key)?;
    hash.try_borrow()
        .map(|table| table.map.get(&key).cloned())
        .map_err(|_| i.error("Cannot inspect a mutably borrowed hash table"))
}

pub(crate) fn f_gethash(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    let value = gethash_entry(i, &args[0], &args[1])?;
    Ok(value.unwrap_or_else(|| arg(&args, 2)))
}

pub(crate) fn f_puthash(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    // Emacs signature: (puthash KEY VALUE TABLE).
    match &args[2] {
        Value::Hash(h) => {
            check_mutable(i, h)?;
            let key = table_key(i, h, &args[0])?;
            let mut hh = h.borrow_mut();
            hh.map.insert(key.clone(), args[1].clone());
            hh.put_key(key, args[0].clone());
            Ok(args[1].clone())
        }
        other => Err(i.wrong_type_mut("hash-table-p", other)),
    }
}

pub(crate) fn f_remhash(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    match &args[1] {
        Value::Hash(h) => {
            check_mutable(i, h)?;
            let key = table_key(i, h, &args[0])?;
            let mut hh = h.borrow_mut();
            hh.map.remove(&key);
            hh.remove_key(&key);
            Ok(Value::Nil)
        }
        other => Err(i.wrong_type_mut("hash-table-p", other)),
    }
}

fn f_clrhash(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    match &args[0] {
        Value::Hash(h) => {
            check_mutable(i, h)?;
            let mut hh = h.borrow_mut();
            hh.map.clear();
            hh.keys.clear();
            hh.collision_order.clear();
            hh.free_slots.clear();
            hh.positioned_keys = 0;
            Ok(args[0].clone())
        }
        other => Err(i.wrong_type_mut("hash-table-p", other)),
    }
}

fn f_hash_table_count(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    match &args[0] {
        Value::Hash(h) => Ok(Value::Int(h.borrow().map.len() as i128)),
        other => Err(i.wrong_type_mut("hash-table-p", other)),
    }
}

fn f_hash_table_keys(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    match &args[0] {
        Value::Hash(h) => Ok(Value::list(
            h.borrow()
                .keys
                .iter()
                .map(|(_, k)| k.clone())
                .collect::<Vec<Value>>(),
        )),
        other => Err(i.wrong_type_mut("hash-table-p", other)),
    }
}

fn f_hash_table_values(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    match &args[0] {
        Value::Hash(h) => Ok(Value::list(h.borrow().map.values().cloned().collect())),
        other => Err(i.wrong_type_mut("hash-table-p", other)),
    }
}

fn f_hash_table_test(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    match &args[0] {
        Value::Hash(h) => {
            let hh = h.borrow();
            // A named test (define-hash-table-test) reports its symbol.
            if hh.test_name.truthy() {
                return Ok(hh.test_name.clone());
            }
            let name = match hh.test {
                HashTest::Eq => "eq",
                HashTest::Eql => "eql",
                HashTest::Equal => "equal",
            };
            Ok(Value::Sym(i.intern(name)))
        }
        other => Err(i.wrong_type_mut("hash-table-p", other)),
    }
}

fn f_hash_table_weakness(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    match &args[0] {
        Value::Hash(h) => Ok(h.borrow().weakness.clone().unwrap_or(Value::Nil)),
        other => Err(i.wrong_type_mut("hash-table-p", other)),
    }
}
fn f_hash_table_rehash_size(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    match &args[0] {
        Value::Hash(h) => Ok(h.borrow().rehash_size.clone()),
        other => Err(i.wrong_type_mut("hash-table-p", other)),
    }
}
fn f_hash_table_rehash_threshold(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    match &args[0] {
        Value::Hash(h) => Ok(h.borrow().rehash_threshold.clone()),
        other => Err(i.wrong_type_mut("hash-table-p", other)),
    }
}
fn f_hash_table_size(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    match &args[0] {
        Value::Hash(h) => Ok(Value::Int(h.borrow().size)),
        other => Err(i.wrong_type_mut("hash-table-p", other)),
    }
}
fn f_copy_hash_table(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    match &args[0] {
        Value::Hash(h) => {
            let mut nh = h.borrow().clone();
            // A copy made inside a callback is independently mutable.
            nh.mutable = true;
            Ok(Value::Hash(Rc::new(RefCell::new(nh))).tracked())
        }
        other => Err(i.wrong_type_mut("hash-table-p", other)),
    }
}
fn f_define_hash_table_test(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    // GNU: (put NAME 'hash-table-test (list TEST HASH)) — the test is
    // a symbol property, and the return value is (TEST HASH).
    let Some(name) = i.sym_id(&args[0]) else {
        return Err(i.wrong_type_mut("symbolp", &args[0]));
    };
    let entry = Value::list(vec![args[1].clone(), args[2].clone()]);
    let prop = i.q("hash-table-test");
    i.put_prop(name, prop, entry.clone());
    Ok(entry)
}

/// GNU returns a list of buckets; each bucket is a list of
/// (KEY . HASH) conses. Our HashMap has no buckets — expose one bucket
/// per key with a stand-in hash code.
fn f_hash_table_buckets(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    match &args[0] {
        Value::Hash(h) => {
            let hh = h.borrow();
            if hh.keys.is_empty() {
                return Ok(Value::Nil);
            }
            let buckets: Vec<Value> = hh
                .keys
                .iter()
                .map(|(_, k)| k)
                .enumerate()
                .map(|(n, k)| {
                    Value::list(vec![Value::cons(k.clone(), Value::Int(1000 + n as i128))])
                })
                .collect();
            Ok(Value::list(buckets))
        }
        other => Err(i.wrong_type_mut("hash-table-p", other)),
    }
}

fn f_hash_table_index_size(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    match &args[0] {
        Value::Hash(h) => {
            // GNU chooses the smallest power-of-two index strictly larger
            // than entry capacity, which survives remhash and clrhash.
            let capacity = h.borrow().size as u128;
            let minimum = capacity + 1;
            Ok(Value::Int(minimum.next_power_of_two() as i128))
        }
        other => Err(i.wrong_type_mut("hash-table-p", other)),
    }
}

fn f_hash_table_histogram(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    match &args[0] {
        Value::Hash(h) => {
            let n = h.borrow().keys.len();
            if n == 0 {
                return Ok(Value::Nil);
            }
            // One key per bucket → ((1 . N)).
            Ok(Value::list(vec![Value::cons(
                Value::Int(1),
                Value::Int(n as i128),
            )]))
        }
        other => Err(i.wrong_type_mut("hash-table-p", other)),
    }
}

#[cfg(test)]
mod checked_equal_key_tests {
    use super::*;

    #[test]
    fn equal_hash_lookup_and_insertion_reject_borrowed_key_strings() {
        std::thread::Builder::new()
            .stack_size(64 * 1024 * 1024)
            .spawn(|| {
                let mut interp = Interp::new();
                let table = Value::Hash(Rc::new(RefCell::new(LispHash::new(HashTest::Equal))));
                let key = Value::string("key");
                let Value::Str(key_string) = &key else {
                    unreachable!();
                };
                let held_borrow = key_string.borrow_mut();

                let insert_result = f_puthash(
                    &mut interp,
                    vec![key.clone(), Value::Int(7), table.clone()].into(),
                );
                let lookup_result = f_gethash(&mut interp, vec![key.clone(), table.clone()].into());

                drop(held_borrow);
                assert!(matches!(
                    insert_result,
                    Err(Flow::Signal(ref signal)) if signal.is_symbol(sym::ERROR)
                ));
                assert!(matches!(
                    lookup_result,
                    Err(Flow::Signal(ref signal)) if signal.is_symbol(sym::ERROR)
                ));
                let Value::Hash(hash) = &table else {
                    unreachable!();
                };
                assert_eq!(hash.borrow().map.len(), 0);
                assert!(matches!(
                    f_puthash(
                        &mut interp,
                        vec![key.clone(), Value::Int(7), table.clone()].into()
                    ),
                    Ok(Value::Int(7))
                ));
                assert!(matches!(
                    f_gethash(&mut interp, vec![key, table].into()),
                    Ok(Value::Int(7))
                ));

                let nested_table =
                    Value::Hash(Rc::new(RefCell::new(LispHash::new(HashTest::Equal))));
                let nested_string = Value::string("nested");
                let nested_key = Value::cons(nested_string.clone(), Value::Nil);
                let Value::Str(nested_cell) = &nested_string else {
                    unreachable!();
                };
                let nested_borrow = nested_cell.borrow_mut();
                let nested_result = f_puthash(
                    &mut interp,
                    vec![nested_key, Value::Int(8), nested_table.clone()].into(),
                );
                drop(nested_borrow);
                assert!(matches!(
                    nested_result,
                    Err(Flow::Signal(ref signal)) if signal.is_symbol(sym::ERROR)
                ));
                let Value::Hash(hash) = &nested_table else {
                    unreachable!();
                };
                assert_eq!(hash.borrow().map.len(), 0);
            })
            .unwrap()
            .join()
            .unwrap();
    }

    #[test]
    fn custom_hash_callbacks_are_not_run_with_borrowed_key_strings() {
        std::thread::Builder::new()
            .stack_size(64 * 1024 * 1024)
            .spawn(|| {
                let mut interp = Interp::new();
                interp
                    .eval_str(
                        "(defvar qa-borrow-hash-calls 0)\
                         (defun qa-borrow-hash-function (key)\
                           (setq qa-borrow-hash-calls (1+ qa-borrow-hash-calls))\
                           (sxhash-equal key))\
                         (define-hash-table-test 'qa-borrow-hash-test\
                                                 #'equal #'qa-borrow-hash-function)",
                    )
                    .unwrap();
                let table = interp
                    .eval_str("(make-hash-table :test 'qa-borrow-hash-test)")
                    .unwrap();
                let key = Value::string("custom-key");
                let Value::Str(cell) = &key else {
                    unreachable!();
                };
                let held_borrow = cell.borrow_mut();

                let result = f_puthash(
                    &mut interp,
                    vec![key.clone(), Value::Int(9), table.clone()].into(),
                );

                drop(held_borrow);
                assert!(matches!(
                    result,
                    Err(Flow::Signal(ref signal)) if signal.is_symbol(sym::ERROR)
                ));
                let calls_symbol = interp.q("qa-borrow-hash-calls");
                assert!(matches!(interp.symbol_value(calls_symbol), Value::Int(0)));
                let Value::Hash(hash) = &table else {
                    unreachable!();
                };
                assert_eq!(hash.borrow().map.len(), 0);

                assert!(matches!(
                    f_puthash(&mut interp, vec![key, Value::Int(9), table.clone()].into()),
                    Ok(Value::Int(9))
                ));
                assert!(matches!(interp.symbol_value(calls_symbol), Value::Int(1)));

                let nested_string = Value::string("nested");
                let nested_key = Value::cons(nested_string.clone(), Value::Nil);
                let Value::Str(nested_cell) = &nested_string else {
                    unreachable!();
                };
                let nested_borrow = nested_cell.borrow_mut();
                let nested_result = f_puthash(
                    &mut interp,
                    vec![nested_key, Value::Int(10), table.clone()].into(),
                );
                drop(nested_borrow);
                assert!(matches!(
                    nested_result,
                    Err(Flow::Signal(ref signal)) if signal.is_symbol(sym::ERROR)
                ));
                assert!(matches!(interp.symbol_value(calls_symbol), Value::Int(1)));
                let Value::Hash(hash) = &table else {
                    unreachable!();
                };
                assert_eq!(hash.borrow().map.len(), 1);
            })
            .unwrap()
            .join()
            .unwrap();
    }

    #[test]
    fn custom_hash_callbacks_check_borrowed_values_past_normalizer_depth() {
        std::thread::Builder::new()
            .stack_size(64 * 1024 * 1024)
            .spawn(|| {
                let mut interp = Interp::new();
                interp
                    .eval_str(
                        "(defvar qa-borrow-deep-hash-calls 0)\
                         (defun qa-borrow-deep-hash-function (key)\
                           (setq qa-borrow-deep-hash-calls\
                                 (1+ qa-borrow-deep-hash-calls))\
                           (if (consp key)\
                               (length (car (last key)))\
                             (sxhash-equal key)))\
                         (define-hash-table-test 'qa-borrow-deep-hash-test\
                                                 #'equal #'qa-borrow-deep-hash-function)",
                    )
                    .unwrap();
                let table = interp
                    .eval_str("(make-hash-table :test 'qa-borrow-deep-hash-test)")
                    .unwrap();
                let borrowed_string = Value::string("last");
                let Value::Str(cell) = &borrowed_string else {
                    unreachable!();
                };
                let held_borrow = cell.borrow_mut();
                let mut items = vec![Value::Nil; 5000];
                items[4999] = borrowed_string.clone();
                let key = Value::list(items);
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    f_puthash(&mut interp, vec![key, Value::Int(11), table.clone()].into())
                }));

                drop(held_borrow);
                assert!(
                    result.is_ok(),
                    "custom hash callback panicked on a borrowed nested value"
                );
                assert!(matches!(
                    result.unwrap(),
                    Err(Flow::Signal(ref signal)) if signal.is_symbol(sym::ERROR)
                ));
                let calls = interp.q("qa-borrow-deep-hash-calls");
                assert!(matches!(interp.symbol_value(calls), Value::Int(0)));
                let Value::Hash(hash) = &table else {
                    unreachable!();
                };
                assert_eq!(hash.borrow().map.len(), 0);
            })
            .unwrap()
            .join()
            .unwrap();
    }
}
