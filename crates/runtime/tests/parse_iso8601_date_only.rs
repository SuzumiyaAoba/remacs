use remacs_runtime::lisp::{Flow, Interp};
use std::path::PathBuf;

const CASES: &str = r#"(progn
  (require 'parse-time)
  (let ((date-only (parse-iso8601-time-string "2024-01-01"))
        (date-only-form (parse-iso8601-time-string "2024-01-01" t))
        (week-date (parse-iso8601-time-string "2024-W01-1"))
        (ordinal-date (parse-iso8601-time-string "2024-001"))
        (complete (parse-iso8601-time-string "2024-01-02T03:04:05+02:00"))
        (fractional (parse-iso8601-time-string "2024-01-02T03:04:05.25Z"))
        (fractional-form (parse-iso8601-time-string
                          "2024-01-02T03:04:05.25Z" t))
        (partial-date-error
         (condition-case nil
             (progn (parse-iso8601-time-string "2024-01") nil)
           (error t)))
        (rfc-date-only-error
         (condition-case nil
             (progn (parse-iso8601-time-string "1 Jan 2024") nil)
           (error t))))
    (list (time-equal-p date-only (encode-time 0 0 0 1 1 2024))
          (time-equal-p date-only-form (encode-time 0 0 0 1 1 2024))
          (time-equal-p week-date (encode-time 0 0 0 1 1 2024))
          (time-equal-p ordinal-date (encode-time 0 0 0 1 1 2024))
          (time-equal-p complete (encode-time 5 4 3 2 1 2024 7200))
          (time-equal-p fractional (encode-time 5.25 4 3 2 1 2024 0))
          (time-equal-p fractional-form (encode-time 5.25 4 3 2 1 2024 0))
          partial-date-error
          rfc-date-only-error)))"#;

fn lisp_string(value: &str) -> String {
    format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
}

#[test]
fn iso8601_date_only_uses_local_midnight_without_changing_other_forms() {
    let source = PathBuf::from(env!("REMACS_SOURCE_ROOT"))
        .join("lisp/parse-time.el")
        .canonicalize()
        .unwrap();
    let code = format!(
        "(progn (load {} nil t t) {})",
        lisp_string(source.to_str().unwrap()),
        CASES
    );
    let mut interp = Interp::new();
    let result = interp.eval_str(&code).unwrap_or_else(|flow| match flow {
        Flow::Signal(signal) => panic!(
            "{} {}",
            interp.prin1_to_string(&signal.symbol()),
            interp.prin1_to_string(&signal.data())
        ),
        other => panic!("{other:?}"),
    });
    assert_eq!(interp.prin1_to_string(&result), "(t t t t t t t t t)");
}
