use remacs_runtime::lisp::{Flow, Interp};

const CASES: &str = r##"
(progn
  (require 'icalendar-parser)
  (when (icalendar-rrule-value-p
         '((FREQ YEARLY) (BYWEEKNO (1)) (BYDAY (1 (2 . 1)))))
    (error "YEARLY/BYWEEKNO accepted an ordinal after an ordinary weekday"))
  ;; Each frequency states whether it permits ordinal BYDAY without
  ;; BYWEEKNO, and whether it permits BYWEEKNO with plain weekdays.
  (dolist (frequency '((SECONDLY nil nil) (MINUTELY nil nil) (HOURLY nil nil)
                       (DAILY nil nil) (WEEKLY nil nil) (MONTHLY t nil) (YEARLY t t)))
    (dolist (with-weekno '(nil t))
      ;; Each case explicitly states whether an ordinal is present, covering
      ;; first, middle, and last positions, mixed signs, and plain controls.
      (dolist (days '(((1 2) nil) ((0 3 6) nil)
                      (((2 . 1) 1) t) ((1 (2 . 1)) t)
                      ((1 (2 . 1) 3) t) ((1 3 (2 . 1)) t)
                      ((1 (2 . -1)) t) ((1 (2 . -1) 3) t)
                      (((1 . 1) (2 . -1)) t)))
        (let* ((ordinal (cadr days))
               (expected (if with-weekno
                             (and (caddr frequency) (not ordinal))
                           (or (not ordinal) (cadr frequency))))
               (rule (append (list (list 'FREQ (car frequency))
                                   (list 'BYDAY (car days)))
                             (when with-weekno '((BYWEEKNO (1)))))))
          (unless (and (eq (not (null (icalendar-rrule-value-p rule))) expected)
                       (eq (not (null (cl-typep rule 'icalendar-rrule-value))) expected))
            (error "Wrong ordinal BYDAY validation for %S; expected %S" rule expected))))))
  t)
"##;

#[test]
fn ordinal_byday_constraints_apply_to_every_weekday_in_a_rule() {
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
