use remacs_runtime::lisp::{Flow, Interp};

#[test]
fn format_seconds_rejects_incomplete_percent_specifiers() {
    let mut interp = Interp::new();
    let result = interp
        .eval_str(
            r#"(progn
                 (require 'time-date)
                 (list
                  (format-seconds "%%" 1)
                  (format-seconds "%%%s" 1)
                  (format-seconds "%%z" 1)
                  (format-seconds "%%q" 1)
                  (format-seconds "%04s" 2)
                  (format-seconds "%.4s" 1)
                  (format-seconds "%z%02s%x" 1)
                  (mapcar
                   (lambda (format)
                     (condition-case nil
                         (progn (format-seconds format 1) 'accepted)
                       (error 'error)))
                   '("%" "elapsed %" "%%%" "%,10" "%04" "%q"
                     "%." "%.s" "%.,2s"))))"#,
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
        "(\"%\" \"%1\" \"%z\" \"%q\" \"0002\" \"0001\" \"01\" (error error error error error error error error error))"
    );
}
