//! Hash table subrs.

use super::{S, arg};
use crate::lisp::Interp;
use crate::lisp::error::EvalResult;
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
        HashTest::Equal => equal_key(interp, v),
    }
}

fn eq_key(v: &Value) -> HashKey {
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

fn equal_key(interp: &Interp, v: &Value) -> HashKey {
    match v {
        Value::Str(s) => HashKey::Str(s.borrow().clone()),
        Value::Cons(c) => {
            let b = c.borrow();
            HashKey::Cons(
                Box::new(equal_key(interp, &b.car)),
                Box::new(equal_key(interp, &b.cdr)),
            )
        }
        Value::Vec(vec) => {
            HashKey::Vec(vec.borrow().iter().map(|x| equal_key(interp, x)).collect())
        }
        _ => eql_key(v),
    }
}

fn f_make_hash_table(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    // (make-hash-table &rest KEYWORD-ARGS) — GNU validates every
    // keyword/value pair: odd arg count, a non-symbol key, an unknown
    // keyword, or a bad value all signal `error'.
    let items = &args[..];
    if items.len() % 2 != 0 {
        return Err(i.error("Invalid keyword argument"));
    }
    let mut test = HashTest::Eql;
    let mut weakness: Option<Value> = None;
    let mut size: i128 = 0;
    let mut rehash_size = Value::float(1.5);
    let mut rehash_threshold = Value::float(0.8125);
    let mut k = 0;
    while k < items.len() {
        let name = match &items[k] {
            Value::Sym(s) => i.symbol_name(*s).to_string(),
            other => {
                let msg = i.prin1_to_string(other);
                return Err(i.error(&format!("Invalid keyword argument {}", msg)));
            }
        };
        let val = &items[k + 1];
        match name.as_str() {
            ":test" => {
                let tname = match val {
                    Value::Sym(id) => i.symbol_name(*id).to_string(),
                    other => {
                        let msg = i.prin1_to_string(other);
                        return Err(i.error(&format!("Invalid hash table test: {}", msg)));
                    }
                };
                test = match tname.as_str() {
                    "eq" => HashTest::Eq,
                    "eql" => HashTest::Eql,
                    "equal" => HashTest::Equal,
                    _ => {
                        // User-defined test from `define-hash-table-test'
                        // (a `hash-table-test' symbol property, as in
                        // GNU). remacs can't plug arbitrary test/hash
                        // functions into the table — accept the name and
                        // use `equal' semantics.
                        let Value::Sym(id) = val else { unreachable!() };
                        let prop = i.intern("hash-table-test");
                        if !i.get_prop(*id, prop).is_nil() {
                            HashTest::Equal
                        } else {
                            return Err(i.error(&format!("Invalid hash table test: {}", tname)));
                        }
                    }
                };
            }
            ":weakness" => match val {
                Value::Nil => {}
                Value::Sym(id) => {
                    let w = i.symbol_name(*id).to_string();
                    match w.as_str() {
                        // GNU: `t' is a synonym for `key-and-value'.
                        "t" => {
                            weakness = Some(Value::Sym(i.intern("key-and-value")));
                        }
                        "key" | "value" | "key-or-value" | "key-and-value" => {
                            weakness = Some(val.clone());
                        }
                        _ => {
                            return Err(i.error(&format!("Invalid hash table weakness: {}", w)));
                        }
                    }
                }
                other => {
                    let msg = i.prin1_to_string(other);
                    return Err(i.error(&format!("Invalid hash table weakness: {}", msg)));
                }
            },
            ":size" => match val {
                Value::Int(n) if *n >= 0 => size = *n,
                other => {
                    let msg = i.prin1_to_string(other);
                    return Err(i.error(&format!("Invalid hash table size: {}", msg)));
                }
            },
            // GNU stores numeric rehash values and silently ignores
            // anything else (a symbol falls back to the default).
            ":rehash-size" => match val {
                Value::Int(_) | Value::Float(_) => rehash_size = val.clone(),
                _ => {}
            },
            ":rehash-threshold" => match val {
                Value::Int(_) | Value::Float(_) => rehash_threshold = val.clone(),
                _ => {}
            },
            ":purecopy" => {}
            _ => return Err(i.error(&format!("Invalid keyword argument {}", name))),
        }
        k += 2;
    }
    let mut h = LispHash::new(test);
    h.weakness = weakness;
    h.size = size;
    h.rehash_size = rehash_size;
    h.rehash_threshold = rehash_threshold;
    Ok(Value::Hash(Rc::new(RefCell::new(h))))
}

fn f_gethash(i: &mut Interp, args: Vec<Value>) -> EvalResult {
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

fn f_puthash(i: &mut Interp, args: Vec<Value>) -> EvalResult {
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

fn f_remhash(i: &mut Interp, args: Vec<Value>) -> EvalResult {
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

fn f_clrhash(i: &mut Interp, args: Vec<Value>) -> EvalResult {
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

fn f_hash_table_count(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    match &args[0] {
        Value::Hash(h) => Ok(Value::Int(h.borrow().map.len() as i128)),
        other => Err(i.wrong_type_mut("hash-table-p", other)),
    }
}

fn f_hash_table_keys(i: &mut Interp, args: Vec<Value>) -> EvalResult {
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

fn f_hash_table_values(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    match &args[0] {
        Value::Hash(h) => Ok(Value::list(h.borrow().map.values().cloned().collect())),
        other => Err(i.wrong_type_mut("hash-table-p", other)),
    }
}

fn f_hash_table_test(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    match &args[0] {
        Value::Hash(h) => {
            let name = match h.borrow().test {
                HashTest::Eq => "eq",
                HashTest::Eql => "eql",
                HashTest::Equal => "equal",
            };
            Ok(Value::Sym(i.intern(name)))
        }
        other => Err(i.wrong_type_mut("hash-table-p", other)),
    }
}

fn f_hash_table_weakness(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    match &args[0] {
        Value::Hash(h) => Ok(h.borrow().weakness.clone().unwrap_or(Value::Nil)),
        other => Err(i.wrong_type_mut("hash-table-p", other)),
    }
}
fn f_hash_table_rehash_size(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    match &args[0] {
        Value::Hash(h) => Ok(h.borrow().rehash_size.clone()),
        other => Err(i.wrong_type_mut("hash-table-p", other)),
    }
}
fn f_hash_table_rehash_threshold(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    match &args[0] {
        Value::Hash(h) => Ok(h.borrow().rehash_threshold.clone()),
        other => Err(i.wrong_type_mut("hash-table-p", other)),
    }
}
fn f_hash_table_size(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    match &args[0] {
        Value::Hash(h) => Ok(Value::Int(h.borrow().size)),
        other => Err(i.wrong_type_mut("hash-table-p", other)),
    }
}
fn f_copy_hash_table(i: &mut Interp, args: Vec<Value>) -> EvalResult {
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
fn f_define_hash_table_test(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    // GNU: (put NAME 'hash-table-test (list TEST HASH)) — the test is
    // a symbol property, and the return value is (TEST HASH).
    let Value::Sym(name) = &args[0] else {
        return Err(i.wrong_type_mut("symbolp", &args[0]));
    };
    let entry = Value::list(vec![args[1].clone(), args[2].clone()]);
    let prop = i.intern("hash-table-test");
    i.put_prop(*name, prop, entry.clone());
    Ok(entry)
}

/// GNU returns a list of buckets; each bucket is a list of
/// (KEY . HASH) conses. Our HashMap has no buckets — expose one bucket
/// per key with a stand-in hash code.
fn f_hash_table_buckets(i: &mut Interp, args: Vec<Value>) -> EvalResult {
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

fn f_hash_table_index_size(i: &mut Interp, args: Vec<Value>) -> EvalResult {
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

fn f_hash_table_histogram(i: &mut Interp, args: Vec<Value>) -> EvalResult {
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
