use remacs_runtime::lisp::{Flow, Interp};
use std::path::PathBuf;

const CASES: &str = r#"
(progn
  (require 'iso8601)
  ;; Every currently announced positive leap second must remain accepted.
  (unless (= (length iso8601--positive-leap-second-days) 27)
    (error "Leap-second table should have 27 announced insertions"))
  (dolist (day iso8601--positive-leap-second-days)
    (unless (iso8601-valid-p (concat day "T23:59:60Z"))
      (error "Rejected announced leap-second day %s" day)))
  (dolist (input '("2016-12-31T23:59:60Z"
                   "20161231T235960Z"
                   "2016-12-31t23:59:60z"
                   "2016-W52-6T23:59:60Z"
                   "2016W526T235960Z"
                   "2016-366T23:59:60Z"
                   "2016366T235960Z"
                   "2017-01-01T05:29:60+05:30"
                   "2016-12-31T18:59:60-05:00"
                   "1973-01-01T23:58:60+23:59"
                   "1972-12-31T00:00:60-23:59"
                   "1972-12-31T00:00:60"
                   "2016-12-31T23:59:60+00:00"
                   "2017-01-01T00:00:60+00:01"
                   "2017-01-01T05:29:60"
                   "2017-01-01T23:58:60"
                   "2016-12-31T23:59:60-00:00"
                   "2016-12-31T23:59:60.5Z"
                   "2016-12-31T23:59:60,25Z"
                   "2016T23:59:60Z"
                   "2016-12T23:59:60Z"
                   "--12-31T23:59:60Z"
                   "--12T23:59:60Z"
                   "---31T23:59:60Z"
                   "2016-W52T23:59:60Z"
                   "2016-W52T05:29:60+05:30"
                   "2017-001T05:29:60+05:30"
                   "20161231T235960+0000"))
    (unless (iso8601-valid-p input)
      (error "Rejected valid leap-second timestamp %s" input)))
  (dolist (input '("2024-01-01T12:00:60Z"
                   "2016-12-30T23:59:60Z"
                   "2016-12-31T23:58:60Z"
                   "2016-12-31T22:59:60Z"
                   "2016-12-31T23:59:60+05:30"
                   "2017-01-01T05:29:60Z"
                   "2016-12-31T23:59:60+00:01"
                   "2017-01-01T05:29:60-00:00"
                   "2017-01-01T05:29:60+23:59"
                   "2017-01-01T23:59:60"
                   "2016-W52-5T23:59:60Z"
                   "2016-W51T23:59:60Z"
                   "2016-365T23:59:60Z"
                   "2017-001T05:29:60Z"
                   "2024-12-31T23:59:60Z"
                   "2024T23:59:60Z"
                   "2026-12-31T23:59:60Z"
                   "2026-12-31T23:59:60.5Z"
                   "2024-01-01T00:00:60"
                   "2016-12-31T23:59:60+24:00"))
    (when (iso8601-valid-p input)
      (error "Accepted a non-existent leap-second timestamp %s" input))
    (unless (condition-case nil (progn (iso8601-parse input) nil)
              (wrong-type-argument t))
      (error "Parsed a non-existent leap-second timestamp %s" input)))
  ;; Time-only parsing has no date or offset with which to reject a possible
  ;; leap second, so retain its existing permissive behavior.
  (unless (= (decoded-time-second (iso8601-parse-time "23:59:60")) 60)
    (error "Time-only leap second was rejected"))
  (unless (and (iso8601-valid-p "2016-12-31t23:59:59z")
               (= (decoded-time-zone (iso8601-parse-time "23:59:59z")) 0))
    (error "Lowercase UTC designator was rejected"))
  ;; Partial date/year and unknown-offset forms are accepted when some
  ;; announced instant can match their known fields.
  (dolist (input '("2017T00:00:60+00:01"
                   "2016-W52T23:59:60Z"
                   "--01-01T05:29:60+05:30"
                   "---01T05:29:60+05:30"))
    (unless (iso8601-valid-p input)
      (error "Rejected compatible partial leap-second timestamp %s" input)))
  (unless (equal (iso8601-valid-p "+0020241029T123456") 0)
    (error "Compact signed-year validity return changed"))
  (let ((input "20161231T235960+0000"))
    (unless (and (iso8601-valid-p input)
                 (equal (match-string 1 input) "20161231")
                 (equal (match-string 2 input) "235960")
                 (equal (match-string 3 input) "+0000"))
      (error "Leap-second validation changed combined match data")))
  (let ((input "20261231T235960Z"))
    (unless (and (not (iso8601-valid-p input))
                 (equal (match-string 1 input) "20261231")
                 (equal (match-string 2 input) "235960")
                 (equal (match-string 3 input) "Z"))
      (error "Rejected leap-second validation changed combined match data")))
  t)
"#;

fn lisp_string(value: &str) -> String {
    format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
}

#[test]
fn iso8601_valid_p_accepts_only_announced_leap_second_instants() {
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
