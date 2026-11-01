use remacs_runtime::lisp::{Flow, Interp};
use std::path::PathBuf;

fn lisp_string(value: &str) -> String {
    format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
}

const CASES: &str = r#"(progn
  (dolist (case '(("12" 12 0 0)
                  ("1234" 12 34 0)
                  ("12:34" 12 34 0)
                  ("123456" 12 34 56)
                  ("12:34:56" 12 34 56)
                  ("12.5" 12 30 0)
                  ("1234.5" 12 34 30)
                  ("12:34.5" 12 34 30)))
    (let* ((parsed (iso8601-parse-time (car case) t))
           (actual (list (decoded-time-hour parsed)
                         (decoded-time-minute parsed)
                         (decoded-time-second parsed))))
      (unless (equal actual (cdr case))
        (error "Valid time %S: expected %S, got %S"
               (car case) (cdr case) actual))))
  (dolist (input '("123456.25" "12:34:56.25" "12:34:56,25"))
    (unless (and (iso8601-parse-time input t)
                 (equal (decoded-time-second (iso8601-parse-time input t))
                        '(5625 . 100)))
      (error "Fractional seconds changed for %S" input)))
  (unless (= (decoded-time-second (iso8601-parse-time "12:34:56.25")) 56)
    (error "FORM nil no longer drops subsecond precision"))
  (dolist (input '("00" "0000" "00:00" "000000" "00:00:00"
                   "235960" "23:59:60"))
    (unless (iso8601-parse-time input)
      (error "Rejected end-of-day/leap-second form %S" input)))
  (dolist (input '("12:3456" "1234:56" "12:" "12:34:" "1234:"
                   "1234:.5" "12:3456.25" "1234:56,25" "12::56"
                   "24:00:01" "240001" "23:59:61" "235961"))
    (unless (condition-case nil (progn (iso8601-parse-time input) nil)
              (wrong-type-argument t))
      (error "Accepted invalid time form %S" input)))
  (dolist (input '("2024-10-29T12:34:56+01:00"
                   "20241029T123456+0100"
                   "2024-W01-1T12:34:56"
                   "2024W011T123456"
                   "2024-001T12:34:56"
                   "2024001T123456"
                   "+002024-10-29T12:34:56"
                   "+0020241029T123456"))
    (unless (iso8601-valid-p input)
      (error "Rejected an existing basic/extended date/time form %S" input)))
  (dolist (input '("2024-10-29T12:3456" "20241029T1234:56"))
    (when (iso8601-valid-p input)
      (error "Accepted mixed time separators in %S" input))
    (unless (condition-case nil (progn (iso8601-parse input) nil)
              (wrong-type-argument t))
      (error "Parsed mixed time separators in %S" input)))
  (let ((input "20241029T123456+0100"))
    (unless (and (iso8601-valid-p input)
                 (equal (match-string 1 input) "20241029")
                 (equal (match-string 2 input) "123456")
                 (equal (match-string 3 input) "+0100"))
      (error "Combined date/time capture data changed")))
  (let ((basic (iso8601-parse "20241029T123456.25+0100" t))
        (extended (iso8601-parse "2024-10-29T12:34:56.25+01:00" t)))
    (unless (and (equal (decoded-time-year basic) 2024)
                 (equal (decoded-time-month basic) 10)
                 (equal (decoded-time-day basic) 29)
                 (equal (decoded-time-second basic) '(5625 . 100))
                 (equal (decoded-time-zone basic) 3600)
                 (equal (decoded-time-year extended) 2024)
                 (equal (decoded-time-second extended) '(5625 . 100))
                 (equal (decoded-time-zone extended) 3600))
      (error "Basic/extended FORM or zone parsing changed")))
  (let* ((parsed (iso8601-parse "+0020241029T123456"))
         (actual (list (decoded-time-year parsed) (decoded-time-month parsed)
                       (decoded-time-day parsed) (decoded-time-hour parsed)
                       (decoded-time-minute parsed) (decoded-time-second parsed))))
    (unless (equal actual '(20241029 nil nil 12 34 56))
      (error "Compact signed expanded year lost its existing parse: %S" actual)))
  t)"#;

#[test]
fn iso8601_time_parsing_rejects_mixed_colon_styles() {
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
