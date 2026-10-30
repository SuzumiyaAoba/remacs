use remacs_runtime::lisp::Interp;

#[test]
fn seconds_to_string_accepts_float_zero_precision() {
    let mut interp = Interp::new();
    let result = interp
        .eval_str(
            "(progn (require 'time-date) (list (seconds-to-string 1.25 t nil 0) (seconds-to-string 1.25 t nil 0.0)))",
        )
        .unwrap();
    assert_eq!(interp.prin1_to_string(&result), "(\"1 second\" \"1 second\")");
}
