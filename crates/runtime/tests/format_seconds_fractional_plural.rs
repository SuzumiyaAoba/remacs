use remacs_runtime::lisp::{Flow, Interp};

#[test]
fn format_seconds_uses_exact_value_for_pluralization() {
    let mut interp = Interp::new();
    let result = interp
        .eval_str(
            r#"(progn
                 (require 'time-date)
                 (list
                  (format-seconds "%,2S" 0.25)
                  (format-seconds "%,2S" 1.0)
                  (format-seconds "%,2S" 1.25)
                  (format-seconds "%,2S" 1.0001)
                  (format-seconds "%,2S" -1.25)))"#,
        )
        .unwrap_or_else(|flow| match flow {
            Flow::Signal(signal) => panic!(
                "{} {}",
                interp.prin1_to_string(&signal.symbol()),
                interp.prin1_to_string(&signal.data())
            ),
            other => panic!("{other:?}"),
        });
    assert_eq!(
        interp.prin1_to_string(&result),
        "(\"0.25 seconds\" \"1.00 second\" \"1.25 seconds\" \"1.00 seconds\" \"-1.25 seconds\")"
    );
}
