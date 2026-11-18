use remacs_runtime::lisp::{Interp, Value};
use std::panic::{AssertUnwindSafe, catch_unwind};

#[test]
fn random_reports_a_mutably_borrowed_string_seed_without_panicking() {
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(|| {
            let mut interp = Interp::new();
            let random = Value::Sym(interp.intern("random"));
            let seed = Value::string("borrowed-random-seed");
            let Value::Str(seed_string) = &seed else {
                unreachable!();
            };

            let held = seed_string.borrow_mut();
            let result = catch_unwind(AssertUnwindSafe(|| {
                interp.apply(&random, vec![seed.clone()])
            }));
            drop(held);

            assert!(result.is_ok(), "random must not panic on a borrowed string seed");
            assert!(result.unwrap().is_err());
        })
        .unwrap()
        .join()
        .unwrap();
}
