use remacs_runtime::lisp::{Flow, Interp};
use std::path::PathBuf;

fn lisp_string(value: &str) -> String {
    format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
}

const CASES: &str = r#"(progn
  (dolist (case '(("20241029" 2024 10 29)
                  ("2024-10-29" 2024 10 29)
                  ("2024101" 2024 4 10)
                  ("2024-101" 2024 4 10)
                  ("+002024-10-29" 2024 10 29)
                  ("+002024-001" 2024 1 1)
                  ("+002024W011" 2024 1 1)
                  ("+002024-W01-1" 2024 1 1)))
    (unless (iso8601-valid-p (car case))
      (error "Rejected valid basic/extended date %S" (car case)))
    (let* ((parsed (iso8601-parse-date (car case)))
           (actual (list (decoded-time-year parsed)
                         (decoded-time-month parsed)
                         (decoded-time-day parsed))))
      (unless (equal actual (cdr case))
        (error "Date %S: expected %S, got %S" (car case) (cdr case) actual))))
  (dolist (case '(("2024" 2024 nil nil)
                  ("+002024" 2024 nil nil)
                  ;; These compact signed numerals remain expanded years:
                  ;; without an agreed year width, they are ambiguous with
                  ;; basic calendar and ordinal dates.
                  ("+0020241029" 20241029 nil nil)
                  ("+002024001" 2024001 nil nil)))
    (unless (iso8601-valid-p (car case))
      (error "Rejected a supported year token %S" (car case)))
    (let* ((parsed (iso8601-parse-date (car case)))
           (actual (list (decoded-time-year parsed)
                         (decoded-time-month parsed)
                         (decoded-time-day parsed))))
      (unless (equal actual (cdr case))
        (error "Year token %S: expected %S, got %S"
               (car case) (cdr case) actual))))
  (unless (equal (iso8601-valid-p "+0020241029") 0)
    (error "Compact signed year validity return changed"))
  (let ((input "20241029"))
    (unless (and (iso8601-valid-p input)
                 (equal (match-string 1 input) input)
                 (null (match-string 2 input))
                 (null (match-string 3 input)))
      (error "Basic date match data changed")))
  (dolist (case '(("20241029T123456" 2024 10 29 12 34 56)
                  ("2024101T123456" 2024 4 10 12 34 56)
                  ("+0020241029T123456" 20241029 nil nil 12 34 56)
                  ("+002024001T123456" 2024001 nil nil 12 34 56)))
    (unless (iso8601-valid-p (car case))
      (error "Rejected a supported combined date/time %S" (car case)))
    (let* ((parsed (iso8601-parse (car case)))
           (actual (list (decoded-time-year parsed)
                         (decoded-time-month parsed)
                         (decoded-time-day parsed)
                         (decoded-time-hour parsed)
                         (decoded-time-minute parsed)
                         (decoded-time-second parsed))))
      (unless (equal actual (cdr case))
        (error "Combined date/time %S: expected %S, got %S"
               (car case) (cdr case) actual))))
  (dolist (input '("20240230" "20241340" "2024367" "2024399"))
    (when (iso8601-valid-p input)
      (error "Accepted an out-of-range basic date %S" input))
    (unless (condition-case nil (progn (iso8601-parse input) nil)
              (wrong-type-argument t))
      (error "Parsed an out-of-range basic date %S" input)))
  t)"#;

#[test]
fn iso8601_basic_calendar_and_ordinal_dates_are_not_shadowed_by_years() {
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
