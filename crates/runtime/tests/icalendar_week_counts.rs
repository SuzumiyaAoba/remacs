use remacs_runtime::lisp::{Flow, Interp};

const CASES: &str = r##"
(progn
  (require 'icalendar-utils)
  (require 'cal-iso)
  (dolist (case '((2020 53) (2021 52) (1900 52) (2000 52)
                  (2004 53) (2009 53) (2015 53) (2024 52) (2026 53) (2100 52)))
    (unless (= (icalendar-number-of-weeks (car case)) (cadr case))
      (error "Wrong ISO week count for %s" (car case))))
  ;; The distance between successive week-one starts gives the week's
  ;; count independently of the helper's January 1/leap-year shortcut.
  ;; January 4 always belongs to week one for every permitted WKST.
  (dotimes (offset 400)
    (let* ((year (+ 1900 offset))
           (jan4 (calendar-absolute-from-gregorian (list 1 4 year)))
           (next-jan4 (calendar-absolute-from-gregorian (list 1 4 (1+ year)))))
      (dotimes (weekstart 7)
        (let* ((first (calendar-dayname-on-or-before weekstart jan4))
               (next (calendar-dayname-on-or-before weekstart next-jan4))
               (expected (/ (- next first) 7))
               (actual (icalendar-number-of-weeks year weekstart)))
          (unless (= actual expected)
            (error "Wrong week count: year=%s WKST=%s got=%s expected=%s"
                   year weekstart actual expected))))
      (let ((iso-weeks
             (car (calendar-iso-from-absolute
                   (calendar-absolute-from-gregorian (list 12 28 year))))))
        (unless (and (= (icalendar-number-of-weeks year) iso-weeks)
                     (= (icalendar-number-of-weeks year nil) iso-weeks))
          (error "Default WKST disagrees with the ISO calendar for %s" year)))))
  t)
"##;

const NEGATIVE_CASES: &str = r##"
(progn
  (dolist (case '((2021 -1 (12 27 2021)) (2021 -2 (12 20 2021))
                  (2021 -52 (1 4 2021)) (2020 -1 (12 28 2020))
                  (2020 -53 (12 30 2019))))
    (let ((actual (icalendar-start-of-weekno (cadr case) (car case))))
      (unless (equal actual (caddr case))
        (error "Wrong negative week start: %S returned %S" case actual))))
  (dotimes (offset 400)
    (let* ((year (+ 1900 offset))
           (jan4 (calendar-absolute-from-gregorian (list 1 4 year)))
           (next-jan4 (calendar-absolute-from-gregorian (list 1 4 (1+ year)))))
      (dotimes (weekstart 7)
        (let* ((first (calendar-dayname-on-or-before weekstart jan4))
               (next (calendar-dayname-on-or-before weekstart next-jan4))
               (weeks (/ (- next first) 7)))
          (dolist (weekno (list -1 -2 (- weeks)))
            (let* ((actual (icalendar-start-of-weekno weekno year weekstart))
                   (expected (calendar-gregorian-from-absolute (+ next (* 7 weekno)))))
              (unless (and (equal actual expected)
                           (equal actual (icalendar-start-of-weekno
                                          (+ weeks 1 weekno) year weekstart)))
                (error "Wrong negative week start: year=%s WKST=%s week=%s got=%S expected=%S"
                       year weekstart weekno actual expected))))))
      (unless (equal (icalendar-start-of-weekno -1 year nil)
                     (icalendar-start-of-weekno -1 year 1))
        (error "Default WKST changed for negative weeks in %s" year))))
  t)
"##;

fn check(interp: &mut Interp, source: &str) {
    match interp.eval_str(source) {
        Ok(value) => assert!(value.truthy()),
        Err(Flow::Signal(signal)) => panic!(
            "{} {}",
            interp.prin1_to_string(&signal.symbol()),
            interp.prin1_to_string(&signal.data())
        ),
        Err(other) => panic!("{other:?}"),
    }
}

#[test]
fn week_counts_and_starts_follow_four_day_week_one_for_a_gregorian_cycle() {
    let mut interp = Interp::new();
    check(&mut interp, CASES);
    check(&mut interp, NEGATIVE_CASES);
}
