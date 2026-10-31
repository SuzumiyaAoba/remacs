use remacs_runtime::lisp::{Flow, Interp};

const CASES: &str = r#"
(progn
  (require 'iso8601)
  (dolist (input '("2024W01-1" "2024-W011" "2024W01-" "2024-W01-"
                   "2024--W01-1" "+02024W01-1" "+02024-W011"
                   "-00001W01-1" "-00001-W011"
                   "2024W01-1T123400Z" "2024-W011T12:34:00Z"))
    (when (iso8601-valid-p input) (error "Accepted mixed week separators: %S" input))
    (unless (condition-case nil (progn (iso8601-parse input) nil) (wrong-type-argument t))
      (error "Parsed an invalid week date: %S" input)))
  (dolist (input '("2024W011" "2024-W01-1" "2024W01" "2024-W01"
                   "+02024W011" "+02024-W01-1" "+02024W01" "+02024-W01"
                   "-00001W011" "-00001-W01-1" "-00001W01" "-00001-W01"
                   "2024W011T123400Z" "2024-W01-1T12:34:00Z"))
    (unless (iso8601-valid-p input) (error "Rejected a consistent week date: %S" input)))
  (dolist (input '("2024W001" "2024-W54-1" "2021W531" "2024W010" "2024-W01-8"))
    (when (iso8601-valid-p input) (error "Accepted an out-of-range week date: %S" input)))
  (dolist (case '(("2024W011" 2024 1 1) ("2024-W01-1" 2024 1 1)
                  ("2019W011" 2018 12 31) ("2019-W01-1" 2018 12 31)
                  ("2020W537" 2021 1 3) ("2020-W53-7" 2021 1 3)
                  ("+02024W011" 2024 1 1) ("+02024-W01-1" 2024 1 1)))
    (let* ((parsed (iso8601-parse (car case)))
           (actual (list (decoded-time-year parsed) (decoded-time-month parsed)
                         (decoded-time-day parsed))))
      (unless (equal actual (cdr case))
        (error "Week date %S: expected %S, got %S" (car case) (cdr case) actual))))
  (dolist (pair '(("2024W01" "2024-W01") ("+02024W01" "+02024-W01")
                  ("-00001W01" "-00001-W01")))
    (unless (equal (iso8601-parse (car pair)) (iso8601-parse (cadr pair)))
      (error "Reduced week styles parsed differently: %S" pair)))
  (dolist (input '("2024W011T123400Z" "2024-W01-1T12:34:00Z"))
    (let ((date (car (split-string input "T"))))
      (unless (and (iso8601-valid-p input)
                   (equal (match-string 1 input) date)
                   (equal (match-string 3 input) "Z"))
        (error "Week validation lost combined match data")))
    (let ((parsed (iso8601-parse input)))
      (unless (equal (list (decoded-time-second parsed) (decoded-time-minute parsed)
                           (decoded-time-hour parsed) (decoded-time-zone parsed))
                     '(0 34 12 0))
        (error "Week date/time parse lost time or zone: %S" input))))
  t)
"#;

#[test]
fn iso_week_dates_require_consistent_separators_and_keep_capture_fields() {
    let mut interp = Interp::new();
    match interp.eval_str(CASES) {
        Ok(value) => assert!(value.truthy()),
        Err(Flow::Signal(signal)) => panic!(
            "{} {}",
            interp.prin1_to_string(&signal.symbol()),
            interp.prin1_to_string(&signal.data())
        ),
        Err(other) => panic!("{other:?}"),
    }
}
