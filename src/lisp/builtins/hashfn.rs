//! Hash table subrs.

use super::{arg, S};
use crate::lisp::error::EvalResult;
use crate::lisp::value::{HashKey, HashTest, LispHash, Subr, Value};
use crate::lisp::Interp;
use std::rc::Rc;
use std::cell::RefCell;

pub(crate) static SUBRS: &[Subr] = &[
    S!("make-hash-table", many 0, f_make_hash_table, "Create a hash table."),
    S!("gethash", 2, 3, f_gethash, "Look up KEY in TABLE."),
    S!("puthash", 3, 3, f_puthash, "Set KEY in TABLE to VALUE."),
    S!("remhash", 2, 2, f_remhash, "Remove KEY from TABLE."),
    S!("clrhash", 1, 1, f_clrhash, "Remove all entries from TABLE."),
    S!("hash-table-count", 1, 1, f_hash_table_count, "Number of entries in TABLE."),
    S!("hash-table-keys", 1, 1, f_hash_table_keys, "Keys of TABLE as a list."),
    S!("hash-table-values", 1, 1, f_hash_table_values, "Values of TABLE as a list."),
    S!("hash-table-test", 1, 1, f_hash_table_test, "Test function of TABLE."),
    S!("hash-table-weakness", 1, 1, f_hash_table_weakness, "Weakness of TABLE."),
    S!("hash-table-rehash-size", 1, 1, f_hash_table_rehash_size, "Rehash size."),
    S!("hash-table-rehash-threshold", 1, 1, f_hash_table_rehash_threshold, "Rehash threshold."),
    S!("hash-table-size", 1, 1, f_hash_table_size, "Current size of TABLE."),
    S!("copy-hash-table", 1, 1, f_copy_hash_table, "Copy TABLE."),
    S!("define-hash-table-test", 3, 3, f_define_hash_table_test, "Define a new test (ignored)."),
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
    // (make-hash-table &key test size rehash-size rehash-threshold weakness)
    let mut test = HashTest::Eql;
    let items = &args[..];
    let mut k = 0;
    while k + 1 < items.len() {
        if let Value::Sym(s) = &items[k] {
            let name = i.symbol_name(*s);
            if name == ":test" {
                let tname = i
                    .sym_id(&items[k + 1])
                    .map(|id| i.symbol_name(id).to_string())
                    .unwrap_or_default();
                test = match tname.as_str() {
                    "eq" => HashTest::Eq,
                    "equal" => HashTest::Equal,
                    _ => HashTest::Eql,
                };
            }
        }
        k += 2;
    }
    Ok(Value::Hash(Rc::new(RefCell::new(LispHash::new(test)))))
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
            hh.keys.insert(key, args[0].clone());
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
            hh.keys.remove(&key);
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
            h.borrow().keys.values().cloned().collect(),
        )),
        other => Err(i.wrong_type_mut("hash-table-p", other)),
    }
}

fn f_hash_table_values(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    match &args[0] {
        Value::Hash(h) => Ok(Value::list(
            h.borrow().map.values().cloned().collect(),
        )),
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

fn f_hash_table_weakness(_i: &mut Interp, _args: Vec<Value>) -> EvalResult {
    Ok(Value::Nil)
}
fn f_hash_table_rehash_size(_i: &mut Interp, _args: Vec<Value>) -> EvalResult {
    Ok(Value::Float(1.5))
}
fn f_hash_table_rehash_threshold(_i: &mut Interp, _args: Vec<Value>) -> EvalResult {
    Ok(Value::Float(0.8125))
}
fn f_hash_table_size(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    match &args[0] {
        Value::Hash(h) => Ok(Value::Int(h.borrow().map.len().max(65) as i128)),
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
            Ok(Value::Hash(Rc::new(RefCell::new(nh))))
        }
        other => Err(i.wrong_type_mut("hash-table-p", other)),
    }
}
fn f_define_hash_table_test(_i: &mut Interp, _args: Vec<Value>) -> EvalResult {
    Ok(Value::Nil)
}
