use remacs_runtime::lisp::Interp;

#[test]
fn seconds_to_string_keeps_fractional_rounding_step_precision() {
    let mut interp = Interp::new();
    let result = interp
        .eval_str(
            "(progn (require 'time-date) (list (seconds-to-string 1.25 t nil 0.25) (seconds-to-string 1.25 t nil 0.125) (seconds-to-string 1.25 t nil 0.5)))",
        )
        .unwrap();
    assert_eq!(
        interp.prin1_to_string(&result),
        "(\"1.25 seconds\" \"1.250 seconds\" \"1 second\")"
    );
}
