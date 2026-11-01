use remacs_runtime::lisp::{Flow, Interp};
use std::path::PathBuf;

fn lisp_string(value: &str) -> String {
    format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
}

const CASES: &str = r#"(progn
  (dolist (case '(("2024-10-29T12:34:56+01:00" 2024 10 29 12 34 56 3600)
                  ("20241029T123456+0100" 2024 10 29 12 34 56 3600)
                  ("2024-W01-1T12:34:56" 2024 1 1 12 34 56 nil)
                  ("2024W011T123456" 2024 1 1 12 34 56 nil)
                  ("2024-001T12:34:56" 2024 1 1 12 34 56 nil)
                  ("2024001T123456" 2024 1 1 12 34 56 nil)
                  ("+002024-W01-1T12:34:56" 2024 1 1 12 34 56 nil)
                  ("+002024W011T123456" 2024 1 1 12 34 56 nil)
                  ("+002024-001T12:34:56" 2024 1 1 12 34 56 nil)
                  ("+002024-10-29T12:34:56" 2024 10 29 12 34 56 nil)))
    (unless (iso8601-valid-p (car case))
      (error "Rejected a consistent combined representation: %S" (car case)))
    (let* ((parsed (iso8601-parse (car case) t))
           (actual (list (decoded-time-year parsed)
                         (decoded-time-month parsed)
                         (decoded-time-day parsed)
                         (decoded-time-hour parsed)
                         (decoded-time-minute parsed)
                         (decoded-time-second parsed)
                         (decoded-time-zone parsed))))
      (unless (equal actual (cdr case))
        (error "Combined representation %S: expected %S, got %S"
               (car case) (cdr case) actual))))
  (dolist (input '("2024-10-29T12:34:56Z" "20241029T123456Z"
                   "2024-10-29T12:34:56+01" "20241029T123456+01"
                   "2024-10-29T12" "20241029T12"
                   "2024-10T12:34" "2024-10T12"
                   "2024-W01T12:34" "2024W01T1234"
                   "2024T12:34" "2024T1234"
                   "+002024T12:34" "+002024T1234"))
    (unless (iso8601-valid-p input)
      (error "Rejected a partial or unambiguous combined form: %S" input)))
  (dolist (input '("2024-10-29T123456"
                   "20241029T12:34:56"
                   "2024-10-29T12:34:56+0100"
                   "20241029T123456+01:00"
                   "2024-W01-1T123456"
                   "2024W011T12:34:56"
                   "2024-001T123456"
                   "2024001T12:34:56"
                   "+002024-W01-1T123456"
                   "+002024W011T12:34:56"
                   "+002024-001T123456"
                   "+002024-10-29T123456"
                   "2024-10-29T12:34:56.25+0100"
                   "20241029T123456,25+01:00"
                   "2024-10-29T12:3456"))
    (when (iso8601-valid-p input)
      (error "Accepted mixed date/time/zone notation: %S" input))
    (unless (condition-case nil (progn (iso8601-parse input) nil)
              (wrong-type-argument t))
      (error "Parsed mixed date/time/zone notation: %S" input)))
  (dolist (case '(("+0020241029T123456" 20241029 12 34 56)
                  ("+0020241029T12:34:56" 20241029 12 34 56)
                  ("+002024001T123456" 2024001 12 34 56)
                  ("+002024001T12:34:56" 2024001 12 34 56)))
    (unless (iso8601-valid-p (car case))
      (error "Rejected an ambiguous compact signed year token: %S" (car case)))
    (unless (equal (iso8601-valid-p (car case)) 0)
      (error "Compact signed year validity value changed for %S" (car case)))
    (let* ((parsed (iso8601-parse (car case)))
           (actual (list (decoded-time-year parsed)
                         (decoded-time-hour parsed)
                         (decoded-time-minute parsed)
                         (decoded-time-second parsed))))
      (unless (equal actual (cdr case))
        (error "Ambiguous signed token %S: expected year-only parse %S, got %S"
               (car case) (cdr case) actual))))
  (let ((input "20241029T123456+0100"))
    (unless (and (iso8601-valid-p input)
                 (equal (match-string 1 input) "20241029")
                 (equal (match-string 2 input) "123456")
                 (equal (match-string 3 input) "+0100"))
      (error "Combined match data changed after notation checking")))
  (let* ((input "20241029T123456,25+0100")
         (integral (iso8601-parse input))
         (fractional (iso8601-parse input t)))
    (unless (and (= (decoded-time-second integral) 56)
                 (equal (decoded-time-second fractional) '(5625 . 100))
                 (= (decoded-time-zone fractional) 3600))
      (error "FORM or basic fractional parsing changed: %S / %S"
             integral fractional)))
  t)"#;

#[test]
fn iso8601_combined_date_time_uses_one_notation_style() {
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
