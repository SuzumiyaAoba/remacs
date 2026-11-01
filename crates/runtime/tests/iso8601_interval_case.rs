use remacs_runtime::lisp::{Flow, Interp};
use std::path::PathBuf;

const CASES: &str = r#"(progn
  (dolist (fold '(nil t))
    (let ((case-fold-search fold))
      (unless (equal (iso8601-parse-duration "p1d")
                     (iso8601-parse-duration "P1D"))
        (error "Duration parsing no longer accepts lowercase"))
      (dolist (input '("p1d/2001-01-01" "2001-01-01/p1d"
                       "pt2h/2001-01-02T03:04:05Z"
                       "2001-01-02T03:04:05Z/pt2h"
                       "p1Dt2h3M4s/2001-01-02T03:04:05Z"
                       "2001-01-02T03:04:05Z/p1Dt2h3M4s"
                       "p1d/2001-01-02T03:04:05.25Z"
                       "2001-01-02T03:04:05.25Z/p1d"))
        (let ((expected (iso8601-parse-interval (upcase input))))
          (unless (equal (iso8601-parse-interval input) expected)
            (error "Interval case mismatch: %S" input)))
        (unless (eq case-fold-search fold)
          (error "Interval parsing leaked case-fold-search")))
      (dolist (input '("p/2001-01-01" "2001-01-01/pt"
                       "p1d/not-a-date" "p1d/p1d"))
        (unless (condition-case nil
                    (progn (iso8601-parse-interval input) nil)
                  (wrong-type-argument t))
          (error "Accepted an invalid interval: %S" input)))
      (unless (eq case-fold-search fold)
        (error "Invalid interval parsing leaked case-fold-search"))))
  t)"#;

fn lisp_string(value: &str) -> String {
    format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
}

#[test]
fn iso8601_intervals_accept_duration_case_independent_of_search_settings() {
    let source = PathBuf::from(env!("REMACS_SOURCE_ROOT"))
        .join("lisp/iso8601.el")
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
    assert!(result.truthy());
}
