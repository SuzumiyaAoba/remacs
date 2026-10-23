//! Remacs's defined rejection of malformed constant references (#121).
//! Do not run these programs in GNU: its unchecked accesses are undefined.
mod common;

use common::{batch_interp, flow_str, on_big_stack};

#[test]
fn out_of_bounds_constants_signal_catchable_errors() {
    on_big_stack(|| {
        let (mut interp, _) = batch_interp();
        // Inline/wide constant loads, then each varref/varset/varbind operand
        // encoding. Both raw execution and compiled calls use the same policy.
        let mut programs = vec![
            (vec![192, 135], "[]"),
            (vec![197, 135], "[1 2]"),
            (vec![255, 135], "(make-vector 63 nil)"),
            (vec![129, 0, 1, 135], "(make-vector 256 nil)"),
            (vec![129, 255, 255, 135], "[]"),
        ];
        for base in [8, 16, 24] {
            programs.extend([
                (vec![base, 135], "[]"),
                (vec![base + 6, 255, 135], "[]"),
                (vec![base + 7, 255, 255, 135], "[]"),
            ]);
        }
        for (bytes, constants) in programs {
            let bytes = bytes
                .iter()
                .map(u8::to_string)
                .collect::<Vec<_>>()
                .join(" ");
            for compiled in [false, true] {
                let call = if compiled {
                    format!("(funcall (make-byte-code 0 (unibyte-string {bytes}) {constants} 1))")
                } else {
                    format!("(byte-code (unibyte-string {bytes}) {constants} 1)")
                };
                let source = format!("(condition-case err {call} (error err))");
                let value = interp
                    .eval_str(&source)
                    .unwrap_or_else(|flow| panic!("{source}: {}", flow_str(&mut interp, &flow)));
                assert_eq!(
                    interp.prin1_to_string(&value),
                    "(error \"Invalid byte-code\")",
                    "{source}"
                );
            }
        }
        let value = interp
            .eval_str("(byte-code (unibyte-string 192 135) [still-ok] 1)")
            .unwrap();
        assert_eq!(interp.prin1_to_string(&value), "still-ok");
    });
}

#[test]
fn corrupt_bytecode_unwinds_bindings_and_can_be_repaired() {
    on_big_stack(|| {
        let (mut interp, _) = batch_interp();
        let value = interp
            .eval_str(
                r#"
            (defvar qa121-bound 'outer)
            (defvar qa121-code nil)
            (let* ((unwound
                    (list
                     (condition-case err
                         (byte-code (unibyte-string 193 24 194 135)
                                    [qa121-bound 42] 1)
                       (error err))
                     qa121-bound))
                   (qa121-code (unibyte-string 192 32 136 193 135))
                   (f (make-byte-code
                       0 qa121-code
                       (vector (lambda () (aset qa121-code 3 194)) 11) 1))
                   (bad (condition-case err (funcall f) (error err))))
              (aset qa121-code 0 193)
              (aset qa121-code 1 135)
              (list unwound bad (funcall f)))
        "#,
            )
            .unwrap_or_else(|flow| panic!("{}", flow_str(&mut interp, &flow)));
        assert_eq!(
            interp.prin1_to_string(&value),
            "(((error \"Invalid byte-code\") outer) (error \"Invalid byte-code\") 11)"
        );
    });
}
