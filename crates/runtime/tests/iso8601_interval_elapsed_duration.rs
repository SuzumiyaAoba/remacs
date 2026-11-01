use remacs_runtime::lisp::{Flow, Interp};
use std::path::PathBuf;

fn lisp_string(value: &str) -> String {
    format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
}

const CASES: &str = r#"(progn
  ;; The issue's reproduction: elapsed time is P2D, not a 1970 date.
  (let ((duration
         (nth 2 (iso8601-parse-interval
                 "2024-01-01T00:00:00Z/2024-01-03T00:00:00Z"))))
    (unless (equal duration (iso8601-parse-duration "P2D"))
      (error "Two-day interval duration mismatch: %S" duration)))
  (let ((duration
         (nth 2 (iso8601-parse-interval
                 "2024-01-01T00:00:00Z/2024-01-01T00:00:00Z"))))
    (unless (equal duration (iso8601-parse-duration "P0D"))
      (error "Zero interval duration mismatch: %S" duration)))
  ;; Crossing a month boundary still produces elapsed days, not months.
  (let ((duration
         (nth 2 (iso8601-parse-interval
                 "2024-01-31T00:00:00Z/2024-03-01T01:02:03Z"))))
    (unless (equal duration (iso8601-parse-duration "P30DT1H2M3S"))
      (error "Multi-unit elapsed duration mismatch: %S" duration)))
  (let ((duration
         (nth 2 (iso8601-parse-interval
                 "2024-01-01T00:00:00Z/2024-01-01T01:00:00+01:00"))))
    (unless (equal duration (iso8601-parse-duration "P0D"))
      (error "Equal instants with different zones had nonzero duration: %S"
             duration)))
  ;; A date change across offset transitions need not take 24 hours.
  (let ((duration
         (nth 2 (iso8601-parse-interval
                 "2024-03-30T12:00:00+01:00/2024-03-31T12:00:00+02:00"))))
    (unless (equal duration (iso8601-parse-duration "PT23H"))
      (error "Offset transition changed elapsed hours: %S" duration)))
  (let ((duration
         (nth 2 (iso8601-parse-interval
                 "2024-01-03T00:00:00Z/2024-01-01T00:00:00Z"))))
    (unless (equal duration '(0 0 0 -2 0 0 nil -1 nil))
      (error "Negative two-day duration was not normalized: %S" duration)))
  (let ((duration
         (nth 2 (iso8601-parse-interval
                 "2024-01-01T00:00:01.5Z/2024-01-01T00:00:00Z"))))
    (unless (and (= (decoded-time-year duration) 0)
                 (= (decoded-time-month duration) 0)
                 (= (decoded-time-day duration) 0)
                 (= (decoded-time-hour duration) 0)
                 (= (decoded-time-minute duration) 0)
                 (time-equal-p (decoded-time-second duration) '(-3 . 2))
                 (null (decoded-time-weekday duration))
                 (= (decoded-time-dst duration) -1)
                 (null (decoded-time-zone duration)))
      (error "Negative fractional duration fields are wrong: %S" duration)))
  ;; #719's endpoints and their difference retain fractional seconds.
  (let* ((interval
          (iso8601-parse-interval
           "2024-01-01T00:00:00.1Z/2024-01-01T00:00:00.9Z"))
         (start (car interval))
         (end (cadr interval))
         (duration (nth 2 interval)))
    (unless (and (equal (decoded-time-second start) '(1 . 10))
                 (equal (decoded-time-second end) '(9 . 10))
                 (time-equal-p (decoded-time-second duration) '(8 . 10)))
      (error "Subsecond endpoint/duration precision changed: %S" interval)))
  ;; Fractions remain exact when crossing midnight.
  (let ((duration
         (nth 2 (iso8601-parse-interval
                 "2024-01-01T23:59:59.9Z/2024-01-02T00:00:00.1Z"))))
    (unless (and (= (decoded-time-day duration) 0)
                 (= (decoded-time-hour duration) 0)
                 (= (decoded-time-minute duration) 0)
                 (time-equal-p (decoded-time-second duration) '(2 . 10)))
      (error "Fractional midnight rollover changed duration: %S" duration)))
  ;; These endpoints differ below picosecond resolution.
  (let* ((interval
          (iso8601-parse-interval
           "2024-01-01T00:00:00.0000000000000001Z/2024-01-01T00:00:00.0000000000000002Z"))
         (start (car interval))
         (end (cadr interval))
         (duration (nth 2 interval))
         (tiny '(1 . 10000000000000000)))
    (unless (and (equal (decoded-time-second start) tiny)
                 (equal (decoded-time-second end) '(2 . 10000000000000000))
                 (time-equal-p (decoded-time-second duration) tiny))
      (error "Sub-picosecond interval precision changed: %S" interval)))
  t)"#;

#[test]
fn iso8601_start_end_intervals_return_exact_elapsed_durations() {
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
