//! Non-destructive filtering of trusted bootstrap property lists.

use crate::lisp::value::{SymId, Value};

/// Remove every occurrence of KEY, retaining property order and value
/// identity. Like the original bootstrap loops, an odd final key receives
/// a nil value and an improper tail is discarded.
pub(super) fn without(plist: &Value, key: SymId) -> Value {
    let mut kept = Vec::new();
    let mut cur = plist.clone();
    while let Value::Cons(cell) = cur {
        let (property, rest) = {
            let cell = cell.borrow();
            (cell.car.clone(), cell.cdr.clone())
        };
        let (value, next) = match rest {
            Value::Cons(cell) => {
                let cell = cell.borrow();
                (cell.car.clone(), cell.cdr.clone())
            }
            _ => (Value::Nil, Value::Nil),
        };
        if !matches!(&property, Value::Sym(id) if *id == key) {
            kept.push(property);
            kept.push(value);
        }
        cur = next;
    }
    Value::list(kept)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::rc::Rc;

    #[test]
    fn removal_keeps_duplicate_order_nil_values_and_shared_objects() {
        let value = Value::string("shared");
        let plist = Value::list(vec![
            Value::Sym(1),
            Value::Int(10),
            Value::Sym(2),
            value.clone(),
            Value::Sym(1),
            Value::Int(20),
            Value::Sym(3),
            Value::Nil,
        ]);
        let result = without(&plist, 1).list_to_vec().unwrap();
        assert_eq!(result.len(), 4);
        assert!(matches!(result[0], Value::Sym(2)));
        let (Value::Str(original), Value::Str(copy)) = (&value, &result[1]) else {
            panic!("property value was replaced")
        };
        assert!(Rc::ptr_eq(original, copy));
        assert!(matches!(result[2], Value::Sym(3)));
        assert!(result[3].is_nil());
        assert_eq!(plist.list_to_vec().unwrap().len(), 8);
    }

    #[test]
    fn odd_property_lists_keep_the_bootstrap_nil_padding_rule() {
        let result = without(&Value::cons(Value::Sym(2), Value::Int(99)), 1)
            .list_to_vec()
            .unwrap();
        assert!(matches!(result[0], Value::Sym(2)));
        assert!(result[1].is_nil());
    }
}
