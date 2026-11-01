use remacs_runtime::lisp::{Flow, Interp};

const CASES: &str = r##"
;;; -*- lexical-binding: t; -*-
(progn
  (require 'icalendar)
  (defvar remacs-test-ical-daily-phase nil)
  (defun remacs-test-ical-daily-format (rule start &optional start-time end-time extras)
    (setq remacs-test-ical-daily-phase (list calendar-date-style rule start start-time end-time))
    (let* ((event (list 'VEVENT nil (append (list (list 'RRULE nil rule)) extras)))
           (decoded (list 0 0 (if start-time 9 0) (cadr start) (car start) (nth 2 start) nil -1 nil))
           (line (icalendar--convert-recurring-to-diary event decoded start-time end-time))
           (parsed (read-from-string line 2)))
      (unless (string-prefix-p "%%(" line) (error "No diary expression: %S" line))
      (unless (equal (string-trim (substring line (cdr parsed)))
                     (concat (or start-time "") (if end-time "-" "") (or end-time "")))
        (error "Daily conversion changed the displayed time: %S" line))
      (cons line (substring line 2 (cdr parsed)))))
  (defun remacs-test-ical-daily-hits (rule start begin end expected &optional start-time end-time extras)
    (let* ((calendar-debug-sexp t)
           (generated (remacs-test-ical-daily-format rule start start-time end-time extras))
           (cursor (calendar-absolute-from-gregorian begin))
           (last (calendar-absolute-from-gregorian end))
           hits)
      (while (<= cursor last)
        (let ((day (calendar-gregorian-from-absolute cursor)))
          (when (diary-sexp-entry (cdr generated) "event" day) (push day hits)))
        (setq cursor (1+ cursor)))
      (setq hits (nreverse hits))
      (unless (equal hits expected)
        (error "Wrong daily occurrences: %S expected %S, generated %S"
               hits expected (car generated)))
      generated))
  (defun remacs-test-ical-daily-probes (rule start candidates expected)
    (let* ((calendar-debug-sexp t)
           (generated (remacs-test-ical-daily-format rule start))
           hits)
      (dolist (day candidates)
        (when (diary-sexp-entry (cdr generated) "event" day) (push day hits)))
      (setq hits (nreverse hits))
      (unless (equal hits expected)
        (error "Wrong daily probes: %S expected %S, generated %S" hits expected (car generated)))))
  (dolist (calendar-date-style '(american european iso))
    (dolist (times '((nil nil) ("09:00" "10:00")))
      (remacs-test-ical-daily-hits
       "FREQ=DAILY;BYDAY=MO,TU,WE,TH,FR" '(1 6 2025) '(1 1 2025) '(1 20 2025)
       '((1 6 2025) (1 7 2025) (1 8 2025) (1 9 2025) (1 10 2025)
         (1 13 2025) (1 14 2025) (1 15 2025) (1 16 2025) (1 17 2025) (1 20 2025))
       (car times) (cadr times))
      (remacs-test-ical-daily-hits
       (concat "FREQ=DAILY;BYDAY=MO,TU,WE,TH,FR;UNTIL="
               (if (car times) "20250115T235959" "20250115"))
       '(1 6 2025) '(1 1 2025) '(1 20 2025)
       '((1 6 2025) (1 7 2025) (1 8 2025) (1 9 2025) (1 10 2025)
         (1 13 2025) (1 14 2025) (1 15 2025)) (car times) (cadr times))
      (remacs-test-ical-daily-hits
       "FREQ=DAILY;BYDAY=MO,TU,WE,TH,FR;COUNT=6" '(1 6 2025) '(1 1 2025) '(1 20 2025)
       '((1 6 2025) (1 7 2025) (1 8 2025) (1 9 2025) (1 10 2025) (1 13 2025))
       (car times) (cadr times))))
  (let ((calendar-date-style 'american))
    (dolist (case '((1 ((1 10 2025))) (2 ((1 10 2025) (1 13 2025)))
                    (4 ((1 10 2025) (1 13 2025) (1 15 2025) (1 20 2025)))))
      (remacs-test-ical-daily-hits
       (format "FREQ=DAILY;BYDAY=WE,MO,WE;COUNT=%d" (car case))
       '(1 10 2025) '(1 1 2025) '(1 25 2025) (cadr case)))
    (remacs-test-ical-daily-hits
     "FREQ=DAILY;BYDAY=MO,WE;INTERVAL=2;COUNT=4" '(1 10 2025) '(1 1 2025) '(2 6 2025)
     '((1 10 2025) (1 20 2025) (1 22 2025) (2 3 2025)) "09:00" nil)
    (remacs-test-ical-daily-hits
     "FREQ=DAILY;BYDAY=MO;INTERVAL=3;COUNT=3" '(1 6 2025) '(1 1 2025) '(2 20 2025)
     '((1 6 2025) (1 27 2025) (2 17 2025)))
    (remacs-test-ical-daily-hits
     "FREQ=DAILY;BYDAY=WE;INTERVAL=7;COUNT=10" '(1 6 2025) '(1 1 2025) '(2 6 2025)
     '((1 6 2025)))
    (remacs-test-ical-daily-hits
     "FREQ=DAILY;BYDAY=MO;INTERVAL=7;COUNT=2" '(1 6 2025) '(1 1 2025) '(1 25 2025)
     '((1 6 2025) (1 13 2025)))
    (remacs-test-ical-daily-hits
     "FREQ=DAILY;BYDAY=SU;COUNT=3" '(1 5 2025) '(1 1 2025) '(1 25 2025)
     '((1 5 2025) (1 12 2025) (1 19 2025)))
    (remacs-test-ical-daily-hits
     "FREQ=DAILY;BYDAY=MO,WE;COUNT=3" '(1 6 2025) '(1 1 2025) '(1 25 2025)
     '((1 8 2025)) nil nil '((EXDATE nil "20250106") (EXDATE nil "20250113")))
    (remacs-test-ical-daily-hits
     "FREQ=DAILY;BYDAY=MO;COUNT=2" '(2 28 2020) '(2 27 2020) '(3 10 2020)
     '((2 28 2020) (3 2 2020)))
    (remacs-test-ical-daily-hits
     "FREQ=DAILY;BYDAY=TH,MO;COUNT=4" '(2 29 2024) '(2 28 2024) '(3 15 2024)
     '((2 29 2024) (3 4 2024) (3 7 2024) (3 11 2024)))
    (remacs-test-ical-daily-hits
     "FREQ=DAILY;BYDAY=MO,WE;COUNT=4" '(12 30 2024) '(12 29 2024) '(1 15 2025)
     '((12 30 2024) (1 1 2025) (1 6 2025) (1 8 2025)))
    (remacs-test-ical-daily-hits
     "FREQ=DAILY;INTERVAL=2;COUNT=3" '(1 6 2025) '(1 1 2025) '(1 15 2025)
     '((1 6 2025) (1 8 2025) (1 10 2025)))
    (let* ((calendar-debug-sexp t)
           (generated (remacs-test-ical-daily-format "FREQ=DAILY;BYDAY=MO" '(1 6 2025)))
           (entry (diary-sexp-entry (cdr generated) "repeat %d" '(1 13 2025))))
      (unless (equal (if (consp entry) (cdr entry) entry) "repeat 7")
        (error "The weekday filter lost diary-cyclic's formatted entry")))
    ;; Independent libc Gregorian conversion checked this millionth MO/WE instance.
    (remacs-test-ical-daily-probes
     "FREQ=DAILY;BYDAY=MO,WE;COUNT=1000000" '(1 6 2025)
     '((1 6 2025) (9 5 11607) (9 10 11607)) '((1 6 2025) (9 5 11607)))
    (let* ((start '(0 0 9 6 1 2025 nil -1 nil))
           (event '(VEVENT nil ((RRULE nil "FREQ=DAILY;BYDAY=WE,MO,WE;COUNT=4"))))
           (before (copy-tree (list event start))))
      (icalendar--convert-recurring-to-diary event start nil nil)
      (unless (equal (list event start) before) (error "Daily conversion mutated its inputs")))
    (dolist (rule '("FREQ=DAILY;BYDAY=2MO" "FREQ=DAILY;BYDAY=NO"
                     "FREQ=DAILY;BYDAY=MO;COUNT=0" "FREQ=DAILY;BYDAY=MO;COUNT=-1"
                     "FREQ=DAILY;BYDAY=MO;COUNT=1.5" "FREQ=DAILY;BYDAY=MO;COUNT=2;INTERVAL=0"))
      (unless (condition-case nil
                  (progn (remacs-test-ical-daily-format rule '(1 6 2025)) nil)
                (error t))
        (error "An invalid daily weekday/count/interval was accepted: %S" rule))))
  t)
"##;

fn check(interp: &mut Interp, source: &str) {
    match interp.eval_str(source) {
        Ok(value) => assert!(value.truthy()),
        Err(Flow::Signal(signal)) => {
            let symbol = interp.intern("remacs-test-ical-daily-phase");
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
fn daily_recurrences_apply_weekday_filters() {
    let mut interp = Interp::new();
    check(&mut interp, CASES);
    check(&mut interp, r#"(load "icalendar.el" nil t)"#);
    check(&mut interp, CASES);
}
