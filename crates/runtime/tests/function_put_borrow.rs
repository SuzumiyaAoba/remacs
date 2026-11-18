use remacs_runtime::lisp::{Interp, Value};
use std::panic::{AssertUnwindSafe, catch_unwind};

#[test]
fn function_put_reports_a_mutably_borrowed_lambda_form_without_panicking() {
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(|| {
            let mut interp = Interp::new();
            let function_put = Value::Sym(interp.intern("function-put"));
            let property = Value::Sym(interp.intern("qa-function-put-borrow-property"));
            let function = Value::list(vec![
                Value::Sym(interp.intern("lambda")),
                Value::Nil,
                Value::Nil,
            ]);
            let Value::Cons(cell) = &function else {
                unreachable!();
            };

            let held = cell.borrow_mut();
            let result = catch_unwind(AssertUnwindSafe(|| {
                interp.apply(
                    &function_put,
                    vec![function.clone(), property, Value::Int(9)],
                )
            }));
            drop(held);

            assert!(result.is_ok(), "function-put must not panic on a borrowed function form");
            assert!(result.unwrap().is_err());
        })
        .unwrap()
        .join()
        .unwrap();
}
