use remacs_runtime::lisp::{Flow, Interp};
use std::time::Instant;

const SETUP: &str = r##"
(progn
  (require 'icalendar-recur)
  (defun remacs-test-recur-component (start rule rdate-groups &optional exdates)
    (icalendar-make-ast-node
     'icalendar-vevent nil
     (append
      (list (icalendar-make-property icalendar-uid "duplicates@example.test")
            (icalendar-make-property icalendar-dtstamp
              (icalendar-read-date-time "20250101T000000Z"))
            (icalendar-make-property icalendar-dtstart start)
            (icalendar-make-property icalendar-rrule rule))
      (mapcar (lambda (dates)
                (icalendar-make-property icalendar-rdate dates))
              rdate-groups)
      (when exdates
        (list (icalendar-make-property icalendar-exdate exdates))))))
  (defun remacs-test-recur-expect (label actual expected)
    (unless (equal actual expected)
      (error "%s: got %S, wanted %S" label actual expected)))
  t)
"##;

const DAILY: &str = r##"
(progn
  (let* ((jan1 (icalendar-read-date "20250101"))
         (jan2 (icalendar-read-date "20250102"))
         (jan3 (icalendar-read-date "20250103"))
         (event (remacs-test-recur-component
                 jan1 '((FREQ DAILY)) (list (list (copy-sequence jan1))))))
    ;; The same overlap must disappear on the first query and a cache hit.
    (dotimes (_ 2)
      (remacs-test-recur-expect
       "Overlapping daily window"
       (icalendar-recur-recurrences-in-window jan1 jan3 event)
       (list jan1 jan2))))
  t)
"##;

const LIMITS: &str = r##"
(progn
  (let* ((jan1 (icalendar-read-date "20250101"))
         (jan2 (icalendar-read-date "20250102"))
         (jan3 (icalendar-read-date "20250103"))
         (jan4 (icalendar-read-date "20250104"))
         (feb1 (icalendar-read-date "20250201"))
         (rule (list '(FREQ MONTHLY) '(BYMONTHDAY (1 2)) (list 'UNTIL jan2)))
         (groups (list (list jan2 jan1 jan3 jan3) (list jan3 jan4)))
         (event (remacs-test-recur-component jan1 rule groups))
         (interval (icalendar-recur-find-interval jan1 jan1 rule)))
    ;; Repeated values across RDATE properties form one set.  A limited first
    ;; query must still cache every distinct recurrence in the interval.
    (remacs-test-recur-expect "First limited query"
      (icalendar-recur-recurrences-in-interval interval event nil 1) (list jan1))
    (remacs-test-recur-expect "Larger cached limit"
      (icalendar-recur-recurrences-in-interval interval event nil 3) (list jan1 jan2 jan3))
    (remacs-test-recur-expect "Complete cached interval"
      (icalendar-recur-recurrences-in-interval interval event) (list jan1 jan2 jan3 jan4))
    (remacs-test-recur-expect "Cached window"
      (icalendar-recur-recurrences-in-window jan1 feb1 event) (list jan1 jan2 jan3 jan4))
    (let ((excluded (remacs-test-recur-component jan1 rule groups (list jan1 jan3))))
      (dotimes (_ 2)
        (remacs-test-recur-expect "EXDATE removes the entire overlapping instance"
          (icalendar-recur-recurrences-in-interval interval excluded) (list jan2 jan4)))))
  t)
"##;

const COUNTED: &str = r##"
(progn
  (let* ((jan1 (icalendar-read-date "20250101"))
         (jan2 (icalendar-read-date "20250102"))
         (jan3 (icalendar-read-date "20250103"))
         (feb1 (icalendar-read-date "20250201"))
         (event (remacs-test-recur-component jan1 '((FREQ DAILY) (COUNT 2))
                                             (list (list jan1 jan3 jan3)))))
    (remacs-test-recur-expect "COUNT union remains distinct"
      (icalendar-recur-recurrences-in-window jan1 feb1 event) (list jan1 jan2 jan3)))
  t)
"##;

const DATE_TIMES: &str = r##"
(progn
  (let* ((start (icalendar-read-date-time "20250101T090000"))
         (upper (icalendar-read-date-time "20250103T090000"))
         (event (remacs-test-recur-component
                 start '((FREQ DAILY))
                 (list (list (copy-sequence start) (copy-sequence start))))))
    (dotimes (_ 2)
      (remacs-test-recur-expect "Floating date-time overlap"
        (mapcar #'icalendar-print-date-time
                (icalendar-recur-recurrences-in-window start upper event))
        '("20250101T090000" "20250102T090000"))))
  t)
"##;

fn check(interp: &mut Interp, label: &str, source: &str) {
    let started = Instant::now();
    println!("{label}: starting");
    match interp.eval_str(source) {
        Ok(value) => assert!(value.truthy()),
        Err(Flow::Signal(signal)) => panic!(
            "{} {}",
            interp.prin1_to_string(&signal.symbol()),
            interp.prin1_to_string(&signal.data())
        ),
        Err(other) => panic!("{other:?}"),
    }
    println!("{label}: passed in {:?}", started.elapsed());
}

#[test]
fn recurrence_union_discards_duplicates_before_limits_and_caching() {
    let started = Instant::now();
    let mut interp = Interp::new();
    println!("Interpreter initialized in {:?}", started.elapsed());
    for (label, source) in [
        ("Actual library and test helpers", SETUP),
        ("Daily overlap", DAILY),
        ("RDATE/EXDATE, limits, cache", LIMITS),
        ("COUNT", COUNTED),
        ("Date-times", DATE_TIMES),
    ] {
        check(&mut interp, label, source);
    }
}
