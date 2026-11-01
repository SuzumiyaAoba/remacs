use remacs_runtime::lisp::{Flow, Interp};
use std::time::Instant;

const SETUP: &str = r##"
(progn
  (require 'icalendar-recur)
  (defun remacs-test-week-event (start weekno weekday &optional weekstart)
    (icalendar-make-ast-node
     'icalendar-vevent nil
     (list (icalendar-make-property icalendar-uid "week-bounds@example.test")
           (icalendar-make-property icalendar-dtstamp
             (icalendar-read-date-time "20190101T000000Z"))
           (icalendar-make-property icalendar-dtstart (icalendar-read-date start))
           (icalendar-make-property icalendar-rrule
             (append (list '(FREQ YEARLY) (list 'BYWEEKNO (list weekno))
                           (list 'BYDAY (list weekday)))
                     (when weekstart (list (list 'WKST weekstart))))))))
  (defun remacs-test-week-expect (label actual expected)
    (unless (equal actual expected)
      (error "%s: got %S, expected %S" label actual expected)))
  t)
"##;

const NEGATIVE: &str = r##"
(progn
  (let ((event (remacs-test-week-event "20200105" -53 0)))
    (remacs-test-week-expect "Nonexistent week -53 in 2021"
      (icalendar-recur-recurrences-in-window
       (icalendar-read-date "20210101") (icalendar-read-date "20220101") event) nil)
    (remacs-test-week-expect "Existing week -53 in 2020"
      (icalendar-recur-recurrences-in-window
       (icalendar-read-date "20200101") (icalendar-read-date "20200110") event)
      '((1 5 2020)))
    (remacs-test-week-expect "Cached nonexistent week -53"
      (icalendar-recur-recurrences-in-window
       (icalendar-read-date "20210101") (icalendar-read-date "20220101") event) nil))
  (let* ((low (icalendar-make-date-time :year 2020 :month 1 :day 1
                                       :hour 0 :minute 0 :second 0))
         (high (icalendar-make-date-time :year 2022 :month 1 :day 1
                                        :hour 0 :minute 0 :second 0))
         (intervals (icalendar-recur-refine-byweekno
                     (icalendar-recur-make-interval low high) '(-53))))
    (remacs-test-week-expect "Validate week bounds separately in each year"
      (mapcar (lambda (interval)
                (list (icalendar-date/time-to-date (icalendar-recur-interval-low interval))
                      (icalendar-date/time-to-date (icalendar-recur-interval-high interval))))
              intervals)
      '(((1 1 2020) (1 6 2020)))))
  t)
"##;

const POSITIVE: &str = r##"
(progn
  (let ((event (remacs-test-week-event "20201230" 53 3)))
    (remacs-test-week-expect "Nonexistent week 53 in 2025"
      (icalendar-recur-recurrences-in-window
       (icalendar-read-date "20250101") (icalendar-read-date "20260101") event) nil)
    (remacs-test-week-expect "Existing week 53 in 2020"
      (icalendar-recur-recurrences-in-window
       (icalendar-read-date "20201220") (icalendar-read-date "20210101") event)
      '((12 30 2020))))
  t)
"##;

const WEEKSTART: &str = r##"
(progn
  (let ((event (remacs-test-week-event "20190103" -53 4 6)))
    ;; With Saturday as WKST, 2019 has 53 weeks and 2020 has only 52.
    (remacs-test-week-expect "WKST-specific nonexistent week -53 in 2020"
      (icalendar-recur-recurrences-in-window
       (icalendar-read-date "20200101") (icalendar-read-date "20200110") event) nil)
    (remacs-test-week-expect "WKST-specific existing week -53 in 2019"
      (icalendar-recur-recurrences-in-window
       (icalendar-read-date "20190101") (icalendar-read-date "20190110") event)
      '((1 3 2019))))
  (remacs-test-week-expect "Valid week -52 remains available in 2021"
    (icalendar-recur-recurrences-in-window
     (icalendar-read-date "20210101") (icalendar-read-date "20210120")
     (remacs-test-week-event "20210110" -52 0))
    '((1 10 2021)))
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
fn yearly_recurrences_ignore_weeks_that_do_not_exist_for_their_wkst() {
    let mut interp = Interp::new();
    for (label, source) in [
        ("Actual recurrence library", SETUP),
        ("Negative week bounds", NEGATIVE),
        ("Positive week bounds", POSITIVE),
        ("Custom week start and valid boundary", WEEKSTART),
    ] {
        check(&mut interp, label, source);
    }
}
