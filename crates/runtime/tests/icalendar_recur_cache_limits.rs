use remacs_runtime::lisp::{Flow, Interp};

const CASES: &str = r##"
(progn
  (require 'icalendar-recur)
  (let* ((start (icalendar-read-date "20250101"))
         (rule '((FREQ YEARLY) (BYMONTH (1 2))))
         (event
          (icalendar-make-ast-node
           'icalendar-vevent nil
           (list (icalendar-make-property icalendar-uid "yearly-cache@example.test")
                 (icalendar-make-property icalendar-dtstamp
                   (icalendar-read-date-time "20250101T000000Z"))
                 (icalendar-make-property icalendar-dtstart start)
                 (icalendar-make-property icalendar-rrule rule))))
         (interval (icalendar-recur-find-interval start start rule))
         (expected '((1 1 2025) (2 1 2025))))
    (unless (equal (icalendar-recur-recurrences-in-interval interval event nil 1)
                   '((1 1 2025)))
      (error "The initial NMAX=1 query did not return just January"))
    (unless (equal (icalendar-recur-recurrences-in-interval interval event) expected)
      (error "The cached yearly interval was truncated by the initial NMAX"))
    (dolist (limit '(2 3 1))
      (unless (equal (icalendar-recur-recurrences-in-interval interval event nil limit)
                     (take limit expected))
        (error "A cached interval query returned the wrong prefix for NMAX=%s" limit)))
    (unless (equal (icalendar-recur-recurrences-in-interval interval event) expected)
      (error "Later limited queries damaged the complete cached interval")))
  t)
"##;

#[test]
fn nmax_does_not_truncate_cached_yearly_recurrences() {
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
