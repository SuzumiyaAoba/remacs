use remacs_runtime::lisp::{Interp, Value};
use std::panic::{AssertUnwindSafe, catch_unwind};

#[test]
fn type_of_reports_a_mutably_borrowed_record_without_panicking() {
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(|| {
            let mut interp = Interp::new();
            let record = interp.eval_str("(record 'qa-borrowed-record-type 1)").unwrap();
            let Value::Record(cell) = &record else {
                panic!("record constructor did not return a record");
            };
            let type_of = Value::Sym(interp.intern("type-of"));

            let held = cell.borrow_mut();
            let result = catch_unwind(AssertUnwindSafe(|| {
                interp.apply(&type_of, vec![record.clone()])
            }));
            drop(held);

            assert!(result.is_ok(), "type-of must not panic on a borrowed record");
            assert!(result.unwrap().is_err());

            let inner = Value::record(vec![
                Value::Sym(interp.intern("qa-record-class")),
                Value::Sym(interp.intern("qa-record-type-name")),
            ]);
            let outer = Value::record(vec![inner.clone(), Value::Int(1)]);
            let Value::Record(inner_cell) = &inner else {
                unreachable!();
            };
            let held = inner_cell.borrow_mut();
            let nested_result = catch_unwind(AssertUnwindSafe(|| {
                interp.apply(&type_of, vec![outer])
            }));
            drop(held);

            assert!(nested_result.is_ok(), "type-of must not panic on a borrowed record type");
            assert!(nested_result.unwrap().is_err());
        })
        .unwrap()
        .join()
        .unwrap();
}
