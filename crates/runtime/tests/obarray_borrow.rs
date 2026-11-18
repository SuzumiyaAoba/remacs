use remacs_runtime::lisp::{Interp, Value};
use std::panic::{AssertUnwindSafe, catch_unwind};

#[test]
fn intern_reports_a_mutably_borrowed_custom_obarray_without_panicking() {
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(|| {
            let mut interp = Interp::new();
            interp
                .eval_str(
                    "(setq qa-borrowed-obarray (make-vector 1 0)) \
                     (intern \"seed\" qa-borrowed-obarray)",
                )
                .unwrap();
            let obarray = interp.eval_str("qa-borrowed-obarray").unwrap();
            let Value::Vec(entries) = &obarray else {
                panic!("legacy obarray is not a vector");
            };
            let Value::Record(record) = entries.borrow()[0].clone() else {
                panic!("legacy obarray was not promoted to a record");
            };
            let Value::Vec(symbols) = record.borrow()[1].clone() else {
                panic!("obarray symbol list is not a vector");
            };
            let intern = Value::Sym(interp.intern("intern"));
            let name = Value::string("new-entry");

            let held = symbols.borrow_mut();
            let result = catch_unwind(AssertUnwindSafe(|| {
                interp.apply(&intern, vec![name, obarray.clone()])
            }));
            drop(held);

            assert!(result.is_ok(), "intern must not panic on a borrowed custom obarray");
            assert!(result.unwrap().is_err());
        })
        .unwrap()
        .join()
        .unwrap();
}

#[test]
fn intern_reports_a_mutably_borrowed_legacy_obarray_without_promoting_it() {
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(|| {
            let mut interp = Interp::new();
            let obarray = interp.eval_str("(make-vector 1 0)").unwrap();
            let Value::Vec(entries) = &obarray else {
                panic!("legacy obarray is not a vector");
            };
            let intern = Value::Sym(interp.intern("intern"));

            let held = entries.borrow_mut();
            let result = catch_unwind(AssertUnwindSafe(|| {
                interp.apply(&intern, vec![Value::string("not-promoted"), obarray.clone()])
            }));
            drop(held);

            assert!(result.is_ok(), "intern must not panic on a borrowed legacy obarray");
            assert!(result.unwrap().is_err());
            assert!(matches!(entries.borrow().first(), Some(Value::Int(0))));
        })
        .unwrap()
        .join()
        .unwrap();
}

#[test]
fn unintern_reports_a_mutably_borrowed_custom_obarray_without_removing_symbol() {
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(|| {
            let mut interp = Interp::new();
            let obarray = interp.eval_str("(obarray-make)").unwrap();
            let intern = Value::Sym(interp.intern("intern"));
            let symbol = interp
                .apply(&intern, vec![Value::string("keep-entry"), obarray.clone()])
                .unwrap();
            let Value::Record(record) = &obarray else {
                panic!("obarray-make did not return a record");
            };
            let Value::Vec(symbols) = record.borrow()[1].clone() else {
                panic!("obarray symbol list is not a vector");
            };
            let unintern = Value::Sym(interp.intern("unintern"));

            let held = symbols.borrow_mut();
            let result = catch_unwind(AssertUnwindSafe(|| {
                interp.apply(&unintern, vec![symbol.clone(), obarray.clone()])
            }));
            drop(held);

            assert!(result.is_ok(), "unintern must not panic on a borrowed custom obarray");
            assert!(result.unwrap().is_err());
            let intern_soft = Value::Sym(interp.intern("intern-soft"));
            assert!(matches!(
                interp
                    .apply(&intern_soft, vec![Value::string("keep-entry"), obarray])
                    .unwrap(),
                Value::Sym(id) if interp.sym_id(&symbol) == Some(id)
            ));
        })
        .unwrap()
        .join()
        .unwrap();
}

#[test]
fn unintern_by_symbol_still_removes_custom_obarray_member() {
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(|| {
            let mut interp = Interp::new();
            let obarray = interp.eval_str("(obarray-make)").unwrap();
            let intern = Value::Sym(interp.intern("intern"));
            let symbol = interp
                .apply(&intern, vec![Value::string("remove-entry"), obarray.clone()])
                .unwrap();
            let unintern = Value::Sym(interp.intern("unintern"));
            let removed = interp
                .apply(&unintern, vec![symbol.clone(), obarray.clone()])
                .unwrap();
            assert!(removed.truthy());

            let intern_soft = Value::Sym(interp.intern("intern-soft"));
            let remaining = interp
                .apply(
                    &intern_soft,
                    vec![Value::string("remove-entry"), obarray],
                )
                .unwrap();
            assert!(remaining.is_nil());
        })
        .unwrap()
        .join()
        .unwrap();
}

#[test]
fn obarray_clear_reports_a_mutably_borrowed_custom_symbol_vector_without_clearing() {
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(|| {
            let mut interp = Interp::new();
            let obarray = interp.eval_str("(obarray-make)").unwrap();
            let intern = Value::Sym(interp.intern("intern"));
            let symbol = interp
                .apply(&intern, vec![Value::string("keep-after-clear-error"), obarray.clone()])
                .unwrap();
            let Value::Record(record) = &obarray else {
                panic!("obarray-make did not return a record");
            };
            let Value::Vec(symbols) = record.borrow()[1].clone() else {
                panic!("obarray symbol list is not a vector");
            };
            let clear = Value::Sym(interp.intern("obarray-clear"));

            let held = symbols.borrow_mut();
            let result = catch_unwind(AssertUnwindSafe(|| {
                interp.apply(&clear, vec![obarray.clone()])
            }));
            drop(held);

            assert!(result.is_ok(), "obarray-clear must not panic on a borrowed symbol vector");
            assert!(result.unwrap().is_err());
            let intern_soft = Value::Sym(interp.intern("intern-soft"));
            assert!(matches!(
                interp
                    .apply(&intern_soft, vec![Value::string("keep-after-clear-error"), obarray])
                    .unwrap(),
                Value::Sym(id) if interp.sym_id(&symbol) == Some(id)
            ));
        })
        .unwrap()
        .join()
        .unwrap();
}

#[test]
fn obarray_clear_removes_custom_obarray_members() {
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(|| {
            let mut interp = Interp::new();
            let obarray = interp.eval_str("(obarray-make)").unwrap();
            let intern = Value::Sym(interp.intern("intern"));
            interp
                .apply(&intern, vec![Value::string("clear-entry"), obarray.clone()])
                .unwrap();
            let clear = Value::Sym(interp.intern("obarray-clear"));
            assert!(interp
                .apply(&clear, vec![obarray.clone()])
                .unwrap()
                .is_nil());

            let intern_soft = Value::Sym(interp.intern("intern-soft"));
            assert!(interp
                .apply(&intern_soft, vec![Value::string("clear-entry"), obarray])
                .unwrap()
                .is_nil());
        })
        .unwrap()
        .join()
        .unwrap();
}
