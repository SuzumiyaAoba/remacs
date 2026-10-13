//! Hash table subrs.

use super::{S, arg};
use crate::lisp::Interp;
use crate::lisp::error::EvalResult;
use crate::lisp::obarray::sym;
use crate::lisp::value::{HashKey, HashTest, LispHash, Subr, Value};
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
        "Define a new test (ignored)."
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
pub(crate) fn hash_key_for(interp: &Interp, v: &Value, test: HashTest) -> HashKey {
    match test {
        HashTest::Eq => eq_key(v),
        HashTest::Eql => eql_key(v),
        HashTest::Equal => {
            let mut ancestors = std::collections::HashSet::new();
            equal_key(interp, v, &mut ancestors)
        }
    }
}

fn eq_key(v: &Value) -> HashKey {
    // `symbols-with-pos-enabled' objects hash as their bare symbol
    // (they are `eq' to it, so the hash must agree).
    if let Some((s, _)) = super::misc::as_sympos(v) {
        return HashKey::Sym(s);
    }
    match v {
        Value::Nil => HashKey::Nil,
        Value::Int(n) => HashKey::Int(*n),
        Value::Float(f) => HashKey::Float(f.to_bits()),
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
        Value::Float(f) => HashKey::Float(f.to_bits()),
        _ => eq_key(v),
    }
}

fn equal_key(
    interp: &Interp,
    v: &Value,
    ancestors: &mut std::collections::HashSet<usize>,
) -> HashKey {
    // Cyclic structures must not recurse forever (cl--generic method
    // tables embed self-referencing records — GNU accepts them in
    // `equal' tables).  Rather than GNU sxhash's depth cap — which is
    // unsound here because our HashKey IS the bucket entry, not a
    // collision-checked digest — track the containers on the current
    // descent path and emit a back-edge marker keyed by pointer.
    let ptr = match v {
        Value::Cons(c) => Some(Rc::as_ptr(c) as usize),
        Value::Vec(c) => Some(Rc::as_ptr(c) as usize),
        Value::Record(c) => Some(Rc::as_ptr(c) as usize),
        _ => None,
    };
    if let Some(p) = ptr {
        if !ancestors.insert(p) {
            // Already on the path: a cycle.  The pointer value keeps
            // equal keys equal and makes unrelated structures' back-
            // edges distinguishable (unlike a flat constant).
            return HashKey::Ptr(p | (1usize << 63));
        }
    }
    let out = match v {
        Value::Str(s) => HashKey::Str(s.borrow().clone()),
        // Enabled sympos are `equal' to their bare symbol — same key
        // as the plain symbol arm below.
        _ if super::misc::as_sympos(v).is_some() => {
            let (s, _) = super::misc::as_sympos(v).unwrap();
            HashKey::Sym(s)
        }
        Value::Cons(c) => {
            let b = c.borrow();
            HashKey::Cons(
                Box::new(equal_key(interp, &b.car, ancestors)),
                Box::new(equal_key(interp, &b.cdr, ancestors)),
            )
        }
        Value::Vec(vec) => HashKey::Vec(
            vec.borrow()
                .iter()
                .map(|x| equal_key(interp, x, ancestors))
                .collect(),
        ),
        // `equal' compares records elementwise — hash contents too,
        // tagged so records never collide with plain vectors.
        Value::Record(r) => HashKey::Tagged(
            1,
            r.borrow()
                .iter()
                .map(|x| equal_key(interp, x, ancestors))
                .collect(),
        ),
        Value::Lambda(l) => {
            if let Some(items) = &l.bc_items {
                let mut keys = vec![HashKey::Int(l.env.is_none() as i128)];
                keys.extend(
                    items
                        .borrow()
                        .iter()
                        .map(|x| equal_key(interp, x, ancestors)),
                );
                HashKey::Tagged(2, keys)
            } else {
                // Interpreted lambdas are `equal' by structure — cover
                // the same fields `equal_values' compares.
                let mut keys = vec![
                    HashKey::Int(l.is_macro as i128),
                    HashKey::Int(l.env.is_none() as i128),
                ];
                keys.extend(l.required.iter().map(|s| HashKey::Sym(*s)));
                keys.extend(l.rest.iter().map(|s| HashKey::Sym(*s)));
                keys.extend(l.optional.iter().map(|p| {
                    HashKey::Vec(vec![
                        HashKey::Sym(p.sym),
                        match &p.supplied {
                            Some(s) => HashKey::Sym(*s),
                            None => HashKey::Nil,
                        },
                        match &p.default {
                            Some(d) => equal_key(interp, d, ancestors),
                            None => HashKey::Nil,
                        },
                    ])
                }));
                keys.extend(l.body.iter().map(|x| equal_key(interp, x, ancestors)));
                HashKey::Tagged(3, keys)
            }
        }
        _ => eql_key(v),
    };
    if let Some(p) = ptr {
        ancestors.remove(&p);
    }
    out
}

fn f_make_hash_table(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
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
        if kw_id == Some(i.intern(":test")) {
            test_arg = arg.clone();
        } else if kw_id == Some(i.intern(":weakness")) {
            weakness_arg = arg.clone();
        } else if kw_id == Some(i.intern(":size")) {
            size_arg = arg.clone();
        } else if kw_id == Some(i.intern(":rehash-threshold"))
            || kw_id == Some(i.intern(":rehash-size"))
            || kw_id == Some(i.intern(":purecopy"))
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
    // `symbolp'; unknown symbol names consult `define-hash-table-test'
    // (the `hash-table-test' property) and otherwise signal
    // "Invalid hash table test".
    let (test, test_name) = {
        let tname = match &test_arg {
            Value::Nil => String::new(),
            v => match i.sym_id(v) {
                Some(id) => i.symbol_name(id).to_string(),
                None => return Err(i.wrong_type_mut("symbolp", v)),
            },
        };
        match tname.as_str() {
            "" | "eql" => (HashTest::Eql, Value::Nil),
            "eq" => (HashTest::Eq, Value::Nil),
            "equal" => (HashTest::Equal, Value::Nil),
            _ => {
                let id = i.sym_id(&test_arg).unwrap();
                let prop = i.intern("hash-table-test");
                if !i.get_prop(id, prop).is_nil() {
                    // remacs can't plug arbitrary test/hash functions
                    // into the table — accept the name and use `equal'
                    // semantics, keeping the name for `hash-table-test'.
                    (HashTest::Equal, test_arg.clone())
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
            let w = i.sym_id(v).map(|id| i.symbol_name(id).to_string());
            match w.as_deref() {
                // GNU: `t' is a synonym for `key-and-value'.
                Some("t") | Some("key-and-value") => Some(Value::Sym(i.intern("key-and-value"))),
                Some("key") | Some("value") | Some("key-or-value") => {
                    Some(Value::Sym(i.sym_id(v).unwrap()))
                }
                // GNU: (error "Invalid hash table weakness" VAL)
                _ => {
                    return Err(i.signal_data(
                        sym::ERROR,
                        vec![Value::string("Invalid hash table weakness"), v.clone()],
                    ));
                }
            }
        }
    };
    let rehash_size = Value::float(1.5);
    let rehash_threshold = Value::float(0.8125);
    let mut h = LispHash::new(test);
    h.weakness = weakness;
    h.size = size;
    h.rehash_size = rehash_size;
    h.rehash_threshold = rehash_threshold;
    h.test_name = test_name;
    Ok(Value::Hash(Rc::new(RefCell::new(h))))
}

fn f_gethash(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    match &args[1] {
        Value::Hash(h) => {
            let key = hash_key_for(i, &args[0], h.borrow().test);
            Ok(h.borrow()
                .map
                .get(&key)
                .cloned()
                .unwrap_or_else(|| arg(&args, 2)))
        }
        other => Err(i.wrong_type_mut("hash-table-p", other)),
    }
}

fn f_puthash(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    // Emacs signature: (puthash KEY VALUE TABLE).
    match &args[2] {
        Value::Hash(h) => {
            let key = hash_key_for(i, &args[0], h.borrow().test);
            let mut hh = h.borrow_mut();
            hh.map.insert(key.clone(), args[1].clone());
            hh.put_key(key, args[0].clone());
            Ok(args[1].clone())
        }
        other => Err(i.wrong_type_mut("hash-table-p", other)),
    }
}

fn f_remhash(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    match &args[1] {
        Value::Hash(h) => {
            let key = hash_key_for(i, &args[0], h.borrow().test);
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
            let mut hh = h.borrow_mut();
            hh.map.clear();
            hh.keys.clear();
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
            let hh = h.borrow();
            let mut nh = LispHash::new(hh.test);
            nh.map = hh.map.clone();
            nh.keys = hh.keys.clone();
            nh.weakness = hh.weakness.clone();
            nh.size = hh.size;
            nh.rehash_size = hh.rehash_size.clone();
            nh.rehash_threshold = hh.rehash_threshold.clone();
            Ok(Value::Hash(Rc::new(RefCell::new(nh))))
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
    let prop = i.intern("hash-table-test");
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
            // GNU: empty table → 1, otherwise smallest power-of-2 >= 8.
            let n = h.borrow().keys.len();
            if n == 0 {
                return Ok(Value::Int(1));
            }
            Ok(Value::Int(n.next_power_of_two().max(8) as i128))
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
