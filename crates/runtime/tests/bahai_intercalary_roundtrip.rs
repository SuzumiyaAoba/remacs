use remacs_runtime::lisp::{Flow, Interp};

const CASES: &str = r##"
;;; -*- lexical-binding: t; -*-
(progn
  (require 'cal-bahai)
  (defvar remacs-test-bahai-phase nil)
  (unless (and (= (calendar-bahai-to-absolute '(19 -3 1)) 673564)
               (equal (calendar-bahai-from-absolute 673564) '(19 -3 1)))
    (error "Pre-reform first intercalary day moved to month 18"))
  (unless (and (equal (calendar-bahai-from-absolute 736020) '(19 -3 172))
               (= (calendar-bahai-to-absolute '(19 -3 172)) 736020))
    (error "Post-reform first intercalary day failed its absolute-date round trip"))
  ;; Real Naw-Ruz calculations: pre/post reform, 4/5 days, Gregorian century rules.
  (dolist (case '((1 . 4) (4 . 5) (56 . 4) (156 . 5) (157 . 4) (171 . 4)
                  (172 . 4) (174 . 5)))
    (let* ((year (car case)) (days (cdr case))
           (start (calendar-bahai-nawruz year))
           (end (calendar-bahai-nawruz (1+ year)))
           (first (+ start (* 19 18)))
           (final-month (+ first days)))
      (setq remacs-test-bahai-phase year)
      (unless (and (= (- end start) (+ (* 19 19) days))
                   (eq (not (null (calendar-bahai-leap-year-p year))) (= days 5)))
        (error "Selected year does not exercise the expected leap/reform case: %S" case))
      (dotimes (index days)
        (let* ((date (list 19 (+ 1 index (- days)) year))
               (absolute (+ first index)))
          (unless (and (= (calendar-bahai-to-absolute date) absolute)
                       (equal (calendar-bahai-from-absolute absolute) date)
                       (= (calendar-bahai-to-absolute (calendar-bahai-from-absolute absolute)) absolute))
            (error "Intercalary day does not occupy the correct consecutive slot: %S %S"
                   date absolute))))
      (dolist (case '((1 1) (2 7) (18 19)))
        (let* ((date (append case (list year)))
               (absolute (+ start (* 19 (1- (car case))) (1- (cadr case)))))
          (unless (and (= (calendar-bahai-to-absolute date) absolute)
                       (equal (calendar-bahai-from-absolute absolute) date))
            (error "An ordinary Bahai date changed: %S" date))))
      (unless (and (= (calendar-bahai-to-absolute (list 18 19 year)) (1- first))
                   (= (calendar-bahai-to-absolute (list 19 0 year)) (1- final-month))
                   (= (calendar-bahai-to-absolute (list 19 1 year)) final-month)
                   (= (calendar-bahai-to-absolute (list 19 19 year)) (1- end))
                   (equal (calendar-bahai-from-absolute final-month) (list 19 1 year))
                   (equal (calendar-bahai-from-absolute (1- end)) (list 19 19 year))
                   (equal (calendar-bahai-from-absolute end) (list 1 1 (1+ year)))
                   (= (calendar-bahai-to-absolute (list 1 1 (1+ year))) end))
        (error "Ayyam-i-Ha/final-month/New-Year boundaries are discontinuous: %S" year))))
  t)
"##;

fn check(interp: &mut Interp, source: &str) {
    match interp.eval_str(source) {
        Ok(value) => assert!(value.truthy()),
        Err(Flow::Signal(signal)) => {
            let symbol = interp.intern("remacs-test-bahai-phase");
            let phase = interp.symbol_value(symbol);
            panic!(
                "{} {} year={}",
                interp.prin1_to_string(&signal.symbol()),
                interp.prin1_to_string(&signal.data()),
                interp.prin1_to_string(&phase)
            );
        }
        Err(other) => panic!("{other:?}"),
    }
}

#[test]
fn bahai_intercalary_dates_round_trip_across_leap_and_reform_boundaries() {
    let mut interp = Interp::new();
    eprintln!("default Bahai intercalary conversion");
    check(&mut interp, CASES);
    check(&mut interp, r#"(load "cal-bahai.el" nil t)"#);
    eprintln!("source Bahai intercalary conversion");
    check(&mut interp, CASES);
}
