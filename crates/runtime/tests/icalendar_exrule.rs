use remacs_runtime::lisp::{Flow, Interp};

const CASES: &str = r##"
;;; -*- lexical-binding: t; -*-
(progn
  (require 'icalendar)
  (defvar remacs-test-ical-exrule-phase nil)
  (let ((calendar-date-style 'iso)
        (calendar-debug-sexp t)
        (candidates '((1 1 2025) (1 2 2025) (1 3 2025) (1 4 2025) (9 1 2025) (9 7 2026))))
    (dolist (rule-case '(("FREQ=DAILY;COUNT=3" ((1 1 2025) (1 2 2025) (1 3 2025)))
                         ("FREQ=YEARLY;BYMONTH=9;BYDAY=1MO;COUNT=2" ((1 1 2025) (9 1 2025)))))
      (dolist (times '((nil nil) ("09:00" "10:00")))
        (let* ((decoded (list 0 0 (if (car times) 9 0) 1 1 2025 nil -1 nil))
               (base-event (list 'VEVENT nil (list (list 'RRULE nil (car rule-case)))))
               (base (icalendar--convert-recurring-to-diary base-event decoded (car times) (cadr times))))
          (dolist (case '((((EXRULE nil "FREQ=DAILY;COUNT=1"))
                           "\n Exception rules: FREQ=DAILY;COUNT=1")
                          (((EXRULE nil "FREQ=WEEKLY;BYDAY=MO,WE")
                            (EXRULE nil "FREQ=MONTHLY;BYMONTHDAY=1,-1"))
                           "\n Exception rules: FREQ=WEEKLY;BYDAY=MO,WE\n                  FREQ=MONTHLY;BYMONTHDAY=1,-1")))
            (setq remacs-test-ical-exrule-phase (list (car rule-case) times (car case)))
            (let* ((event (list 'VEVENT nil (append (nth 2 base-event) (car case))))
                   (before (copy-tree (list event decoded)))
                   (line (icalendar--convert-recurring-to-diary event decoded (car times) (cadr times)))
                   (parsed (read-from-string line 2))
                   (sexp (substring line 2 (cdr parsed))) hits)
              (unless (equal line (concat base (cadr case)))
                (error "EXRULE note lost values, commas, order, or time: %S" line))
              (unless (equal (list event decoded) before) (error "EXRULE conversion mutated input"))
              (dolist (day candidates)
                (when (diary-sexp-entry sexp "event" day) (push day hits)))
              (unless (equal (nreverse hits) (cadr rule-case))
                (error "EXRULE annotation changed the RRULE diary expression: %S" line))))))))
  t)
"##;

fn check(interp: &mut Interp, source: &str) {
    match interp.eval_str(source) {
        Ok(value) => assert!(value.truthy()),
        Err(Flow::Signal(signal)) => {
            let symbol = interp.intern("remacs-test-ical-exrule-phase");
            let phase = interp.symbol_value(symbol);
            panic!(
                "{} {} phase={}",
                interp.prin1_to_string(&signal.symbol()),
                interp.prin1_to_string(&signal.data()),
                interp.prin1_to_string(&phase)
            );
        }
        Err(other) => panic!("{other:?}"),
    }
}

#[test]
fn exrule_annotations_preserve_complete_rule_values() {
    let mut interp = Interp::new();
    check(&mut interp, CASES);
    check(&mut interp, r#"(load "icalendar.el" nil t)"#);
    check(&mut interp, CASES);
}
