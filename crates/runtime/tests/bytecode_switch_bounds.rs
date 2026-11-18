use remacs_runtime::lisp::{Flow, Interp, sym};
use std::panic::{AssertUnwindSafe, catch_unwind};

#[test]
fn switch_lookup_preserves_misses_and_rejects_invalid_targets() {
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(|| {
            let mut interp = Interp::new();
            for (target, expected_data) in [
                ("-1", "(\"Invalid byte opcode: op=0, ptr=-1\")"),
                (
                    "18446744073709551615",
                    "(\"Invalid byte opcode: op=0, ptr=18446744073709551615\")",
                ),
                ("'bad-target", "(\"Invalid byte-code\")"),
                ("'remacs--switch-miss", "(\"Invalid byte-code\")"),
                ("nil", "(\"Invalid byte-code\")"),
            ] {
                let source = format!(
                    "(let ((table (make-hash-table :test 'eq))) \
                       (puthash 'key {target} table) \
                       (funcall (make-byte-code '(x) \
                         (unibyte-string 8 193 183 130 10 0 194 135 195 135 196 135) \
                         (vector 'x table 1 2 42) 2) 'key))"
                );
                let result = catch_unwind(AssertUnwindSafe(|| interp.eval_str(&source)));

                assert!(result.is_ok(), "switch target {target} must not panic");
                let Err(Flow::Signal(signal)) = result.unwrap() else {
                    panic!("switch target {target} should signal invalid bytecode");
                };
                assert!(signal.is_symbol(sym::ERROR));
                assert_eq!(
                    interp.prin1_to_string(&signal.data()).to_unicode_lossy(),
                    expected_data
                );
            }

            let case_entry = interp
                .eval_str(
                    "(let ((table (make-hash-table :test 'eq))) \
                       (puthash 'key 6 table) \
                       (funcall (make-byte-code '(x) \
                         (unibyte-string 8 193 183 130 10 0 194 135 195 135 196 135) \
                         (vector 'x table 1 2 42) 2) 'key))",
                )
                .unwrap();
            assert_eq!(interp.prin1_to_string(&case_entry).to_unicode_lossy(), "1");

            let missing_entry = interp
                .eval_str(
                    "(let ((table (make-hash-table :test 'eq))) \
                       (puthash 'other 6 table) \
                       (funcall (make-byte-code '(x) \
                         (unibyte-string 8 193 183 130 10 0 194 135 195 135 196 135) \
                         (vector 'x table 1 2 42) 2) 'key))",
                )
                .unwrap();
            assert_eq!(
                interp.prin1_to_string(&missing_entry).to_unicode_lossy(),
                "42"
            );
        })
        .unwrap()
        .join()
        .unwrap();
}
