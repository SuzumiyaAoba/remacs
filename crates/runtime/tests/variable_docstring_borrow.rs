use remacs_runtime::lisp::{Interp, Value};
use std::panic::{AssertUnwindSafe, catch_unwind};

#[test]
fn defvar_one_does_not_panic_or_define_variable_with_borrowed_docstring() {
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(|| {
            let mut interp = Interp::new();
            let variable = interp.intern("qa-borrowed-defvar-docstring");
            let defvar = Value::Sym(interp.intern("defvar-1"));
            let doc = Value::string("borrowed docstring");
            let Value::Str(docstring) = &doc else {
                unreachable!();
            };

            let held = docstring.borrow_mut();
            let result = catch_unwind(AssertUnwindSafe(|| {
                interp.apply(
                    &defvar,
                    vec![Value::Sym(variable), Value::Int(42), doc.clone()],
                )
            }));
            drop(held);

            assert!(result.is_ok(), "defvar-1 must not panic on a borrowed docstring");
            assert!(result.unwrap().is_err());
            assert!(!interp.bound_p(variable));
            assert!(!interp.obarray.symbol(variable).special);
        })
        .unwrap()
        .join()
        .unwrap();
}

#[test]
fn defconst_one_does_not_panic_or_define_constant_with_borrowed_docstring() {
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(|| {
            let mut interp = Interp::new();
            let variable = interp.intern("qa-borrowed-defconst-docstring");
            let defconst = Value::Sym(interp.intern("defconst-1"));
            let doc = Value::string("borrowed docstring");
            let Value::Str(docstring) = &doc else {
                unreachable!();
            };

            let held = docstring.borrow_mut();
            let result = catch_unwind(AssertUnwindSafe(|| {
                interp.apply(
                    &defconst,
                    vec![Value::Sym(variable), Value::Int(42), doc.clone()],
                )
            }));
            drop(held);

            assert!(result.is_ok(), "defconst-1 must not panic on a borrowed docstring");
            assert!(result.unwrap().is_err());
            assert!(!interp.bound_p(variable));
            assert!(!interp.obarray.symbol(variable).special);
            assert!(!interp.obarray.symbol(variable).constant);
        })
        .unwrap()
        .join()
        .unwrap();
}
