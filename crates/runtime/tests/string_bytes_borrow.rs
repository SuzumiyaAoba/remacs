use remacs_runtime::lisp::{Interp, Value};
use std::panic::{AssertUnwindSafe, catch_unwind};

#[test]
fn byte_string_comparisons_report_mutably_borrowed_strings_without_panicking() {
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(|| {
            let mut interp = Interp::new();
            let left = Value::string("borrowed-left");
            let right = Value::string("borrowed-right");
            let Value::Str(left_string) = &left else {
                unreachable!();
            };
            let string_version_lessp = Value::Sym(interp.intern("string-version-lessp"));
            let string_distance = Value::Sym(interp.intern("string-distance"));
            let send_string_to_terminal = Value::Sym(interp.intern("send-string-to-terminal"));
            assert!(interp
                .eval_str("(fboundp 'send-string-to-terminal)")
                .unwrap()
                .truthy());

            let held = left_string.borrow_mut();
            let version_result = catch_unwind(AssertUnwindSafe(|| {
                interp.apply(
                    &string_version_lessp,
                    vec![left.clone(), right.clone()],
                )
            }));
            let distance_chars_result = catch_unwind(AssertUnwindSafe(|| {
                interp.apply(&string_distance, vec![left.clone(), right.clone()])
            }));
            let distance_bytes_result = catch_unwind(AssertUnwindSafe(|| {
                interp.apply(&string_distance, vec![left.clone(), right.clone(), Value::t()])
            }));
            let send_result = catch_unwind(AssertUnwindSafe(|| {
                interp.apply(&send_string_to_terminal, vec![left.clone()])
            }));
            drop(held);

            assert!(version_result.is_ok(), "version comparison must not panic");
            assert!(version_result.unwrap().is_err());
            assert!(distance_chars_result.is_ok(), "character distance must not panic");
            assert!(distance_chars_result.unwrap().is_err());
            assert!(distance_bytes_result.is_ok(), "byte distance must not panic");
            assert!(distance_bytes_result.unwrap().is_err());
            assert!(send_result.is_ok(), "terminal output must not panic");
            assert!(send_result.unwrap().is_err());
        })
        .unwrap()
        .join()
        .unwrap();
}
