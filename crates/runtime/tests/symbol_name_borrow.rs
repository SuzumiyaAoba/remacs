use remacs_runtime::lisp::{Interp, Value};
use std::panic::{AssertUnwindSafe, catch_unwind};

#[test]
fn printing_symbol_with_mutably_borrowed_name_does_not_panic() {
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(|| {
            let mut interp = Interp::new();
            let name = Value::string("borrowed-symbol");
            let make_symbol = Value::Sym(interp.intern("make-symbol"));
            let symbol = interp.apply(&make_symbol, vec![name]).unwrap();
            let id = interp.sym_id(&symbol).expect("make-symbol returns a symbol");
            let name_string = interp.obarray.symbol(id).name_string.clone().unwrap();

            let held = name_string.borrow_mut();
            let printed = catch_unwind(AssertUnwindSafe(|| interp.prin1_to_string(&symbol)));
            drop(held);

            assert_eq!(
                printed.expect("printing must not panic on a borrowed symbol name"),
                "borrowed-symbol"
            );

            {
                let mut name = name_string.borrow_mut();
                name.clear();
                name.push_str("changed-symbol");
            }
            assert_eq!(interp.prin1_to_string(&symbol), "changed-symbol");
        })
        .unwrap()
        .join()
        .unwrap();
}

#[test]
fn make_symbol_reports_a_mutably_borrowed_name_without_panicking() {
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(|| {
            let mut interp = Interp::new();
            let name = Value::string("borrowed-make-symbol-name");
            let Value::Str(name_string) = &name else {
                unreachable!();
            };
            let make_symbol = Value::Sym(interp.intern("make-symbol"));

            let held = name_string.borrow_mut();
            let result = catch_unwind(AssertUnwindSafe(|| {
                interp.apply(&make_symbol, vec![name.clone()])
            }));
            drop(held);

            assert!(result.is_ok(), "make-symbol must not panic on a borrowed name");
            assert!(result.unwrap().is_err(), "make-symbol must report the borrow error");
        })
        .unwrap()
        .join()
        .unwrap();
}

#[test]
fn intern_reports_a_mutably_borrowed_name_without_panicking() {
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(|| {
            let mut interp = Interp::new();
            interp
                .eval_str("(setq read-symbol-shorthands '((\"src-\" . \"source-\")))")
                .unwrap();
            for function_name in ["intern", "intern-soft", "unintern"] {
                let name = Value::string("src-borrowed-intern-name");
                let Value::Str(name_string) = &name else {
                    unreachable!();
                };
                let function = Value::Sym(interp.intern(function_name));

                let held = name_string.borrow_mut();
                let result = catch_unwind(AssertUnwindSafe(|| {
                    interp.apply(&function, vec![name.clone()])
                }));
                drop(held);

                assert!(result.is_ok(), "{function_name} must not panic on a borrowed name");
                assert!(result.unwrap().is_err());
            }
        })
        .unwrap()
        .join()
        .unwrap();
}
