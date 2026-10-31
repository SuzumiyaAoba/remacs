use remacs_runtime::lisp::{Flow, Interp};

#[test]
fn format_seconds_accepts_multidigit_fraction_precision() {
    let mut interp = Interp::new();
    let result = interp
        .eval_str(
            r#"(progn
                 (require 'time-date)
                 (list
                  (format-seconds "%,1s" 1.23456789)
                  (format-seconds "%,2s" 1.23456789)
                  (format-seconds "%,10s" 1.23456789)
                  (format-seconds "%% %04s" 2)
                  (format-seconds "%z%02s%x" 1)))"#,
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
        "(\"1.2\" \"1.23\" \"1.2345678900\" \"% 0002\" \"01\")"
    );
}
