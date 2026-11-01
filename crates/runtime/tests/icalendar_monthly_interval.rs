use remacs_runtime::lisp::{Flow, Interp};

const CASES: &str = r##"
;;; -*- lexical-binding: t; -*-
(progn
  (require 'icalendar)
  (defvar remacs-test-ical-monthly-phase nil)
  (defun remacs-test-ical-monthly-format (rule start &optional start-time end-time extras)
    (setq remacs-test-ical-monthly-phase (list calendar-date-style rule start start-time end-time))
    (let* ((event (list 'VEVENT nil (append (list (list 'RRULE nil rule)) extras)))
           (decoded (list 0 0 (if start-time 9 0) (cadr start) (car start) (nth 2 start) nil -1 nil))
           (line (icalendar--convert-recurring-to-diary event decoded start-time end-time))
           (parsed (read-from-string line 2)))
      (unless (string-prefix-p "%%(" line) (error "No diary expression: %S" line))
      (unless (equal (string-trim (substring line (cdr parsed)))
                     (concat (or start-time "") (if end-time "-" "") (or end-time "")))
        (error "Monthly conversion changed the displayed time: %S" line))
      (cons line (substring line 2 (cdr parsed)))))
  (defun remacs-test-ical-monthly-hits (rule start begin end expected &optional start-time end-time extras)
    (let* ((calendar-debug-sexp t)
           (generated (remacs-test-ical-monthly-format rule start start-time end-time extras))
           (cursor (calendar-absolute-from-gregorian begin))
           (last (calendar-absolute-from-gregorian end))
           hits)
      (while (<= cursor last)
        (let ((day (calendar-gregorian-from-absolute cursor)))
          (when (diary-sexp-entry (cdr generated) "event" day) (push day hits)))
        (setq cursor (1+ cursor)))
      (setq hits (nreverse hits))
      (unless (equal hits expected)
        (error "Wrong monthly occurrences: %S expected %S, generated %S"
               hits expected (car generated)))
      generated))
  (dolist (calendar-date-style '(american european iso))
    (dolist (times '((nil nil) ("09:00" "10:00")))
      (dolist (limited '(nil t))
        (let ((until (if limited
                         (if (car times) ";UNTIL=20250313T235959" ";UNTIL=20250313")
                       "")))
          (remacs-test-ical-monthly-hits
           (concat "FREQ=MONTHLY;INTERVAL=2;BYDAY=2MO" until)
           '(1 13 2025) '(1 1 2025) '(6 15 2025)
           (if limited '((1 13 2025) (3 10 2025))
             '((1 13 2025) (3 10 2025) (5 12 2025))) (car times) (cadr times))
          (remacs-test-ical-monthly-hits
           (concat "FREQ=MONTHLY;INTERVAL=2" until)
           '(1 10 2025) '(1 1 2025) '(6 15 2025)
           (if limited '((1 10 2025) (3 10 2025))
             '((1 10 2025) (3 10 2025) (5 10 2025))) (car times) (cadr times))))))
  ;; A monthly BYDAY list is a union, including every ordinal selector (#1562).
  (dolist (calendar-date-style '(american european iso))
    (dolist (times '((nil nil) ("09:00" "10:00")))
      (remacs-test-ical-monthly-hits
       "FREQ=MONTHLY;BYDAY=1MO,-1FR" '(1 6 2025) '(1 1 2025) '(3 31 2025)
       '((1 6 2025) (1 31 2025) (2 3 2025) (2 28 2025) (3 3 2025) (3 28 2025))
       (car times) (cadr times))
      (remacs-test-ical-monthly-hits
       (concat "FREQ=MONTHLY;INTERVAL=2;BYDAY=1MO,-1FR;UNTIL="
               (if (car times) "20250328T235959" "20250328"))
       '(1 6 2025) '(1 1 2025) '(5 31 2025)
       '((1 6 2025) (1 31 2025) (3 3 2025) (3 28 2025)) (car times) (cadr times))))
  (let ((calendar-date-style 'american))
    ;; RFC 5545's every-other-month first/last Sunday pattern, bounded by UNTIL.
    (remacs-test-ical-monthly-hits
     "FREQ=MONTHLY;INTERVAL=2;BYDAY=1SU,-1SU;UNTIL=19980531"
     '(9 7 1997) '(9 1 1997) '(6 30 1998)
     '((9 7 1997) (9 28 1997) (11 2 1997) (11 30 1997) (1 4 1998) (1 25 1998)
       (3 1 1998) (3 29 1998) (5 3 1998) (5 31 1998)))
    (remacs-test-ical-monthly-hits
     "FREQ=MONTHLY;BYDAY=-1FR,1MO,-1FR,1MO;UNTIL=20250228" '(1 6 2025) '(1 1 2025) '(3 31 2025)
     '((1 6 2025) (1 31 2025) (2 3 2025) (2 28 2025)))
    (remacs-test-ical-monthly-hits
     "FREQ=MONTHLY;BYDAY=2MO,-2MO;UNTIL=20250331" '(1 13 2025) '(1 1 2025) '(3 31 2025)
     '((1 13 2025) (1 20 2025) (2 10 2025) (2 17 2025) (3 10 2025) (3 24 2025)))
    (remacs-test-ical-monthly-hits
     "FREQ=MONTHLY;BYDAY=1MO,FR;UNTIL=20250214" '(1 6 2025) '(1 1 2025) '(3 1 2025)
     '((1 6 2025) (1 10 2025) (1 17 2025) (1 24 2025) (1 31 2025)
       (2 3 2025) (2 7 2025) (2 14 2025)))
    ;; Distinct ordinals can select the same day; the entry still appears once.
    (remacs-test-ical-monthly-hits
     "FREQ=MONTHLY;BYDAY=2MO,-3MO;UNTIL=20250228" '(1 13 2025) '(1 1 2025) '(2 28 2025)
     '((1 13 2025) (2 10 2025)))
    (remacs-test-ical-monthly-hits
     "FREQ=MONTHLY;BYDAY=1MO,-1FR;UNTIL=20250228" '(1 6 2025) '(1 1 2025) '(3 31 2025)
     '((1 6 2025) (2 3 2025)) nil nil '((EXDATE nil "20250131") (EXDATE nil "20250228")))
    (remacs-test-ical-monthly-hits
     "FREQ=MONTHLY;BYDAY=+2MO,-2FR;UNTIL=20250228" '(1 13 2025) '(1 1 2025) '(2 28 2025)
     '((1 13 2025) (1 24 2025) (2 10 2025) (2 21 2025))))
  (let ((calendar-date-style 'american))
    ;; Missing INTERVAL and explicit INTERVAL=1 retain the original diary rule.
    (dolist (byday '("" ";BYDAY=2MO"))
      (unless (equal (car (remacs-test-ical-monthly-format (concat "FREQ=MONTHLY" byday) '(1 13 2025)))
                     (car (remacs-test-ical-monthly-format (concat "FREQ=MONTHLY;INTERVAL=1" byday) '(1 13 2025))))
        (error "Explicit monthly INTERVAL=1 changed the diary format")))
    (remacs-test-ical-monthly-hits
     "FREQ=MONTHLY;INTERVAL=1;BYDAY=2MO" '(1 13 2025) '(1 1 2025) '(4 15 2025)
     '((1 13 2025) (2 10 2025) (3 10 2025) (4 14 2025)))
    (remacs-test-ical-monthly-hits
     "FREQ=MONTHLY;INTERVAL=3;BYDAY=-1SU" '(12 29 2024) '(12 1 2024) '(7 1 2025)
     '((12 29 2024) (3 30 2025) (6 29 2025)) "09:00" nil)
    ;; Invalid dates are skipped without moving the active-month phase.
    (remacs-test-ical-monthly-hits
     "FREQ=MONTHLY;INTERVAL=2" '(12 31 2023) '(12 1 2023) '(7 1 2024)
     '((12 31 2023)))
    (remacs-test-ical-monthly-hits
     "FREQ=MONTHLY;INTERVAL=2" '(2 29 2024) '(2 1 2024) '(9 1 2024)
     '((2 29 2024) (4 29 2024) (6 29 2024) (8 29 2024)))
    (remacs-test-ical-monthly-hits
     "FREQ=MONTHLY;INTERVAL=2;COUNT=3" '(1 10 2025) '(1 1 2025) '(7 15 2025)
     '((1 10 2025) (3 10 2025) (5 10 2025)))
    (remacs-test-ical-monthly-hits
     "FREQ=MONTHLY;INTERVAL=2;BYDAY=2MO;UNTIL=20250310" '(1 13 2025) '(1 1 2025) '(5 15 2025)
     '((1 13 2025) (3 10 2025)))
    (remacs-test-ical-monthly-hits
     "FREQ=MONTHLY;INTERVAL=2;BYDAY=2MO;UNTIL=20250309" '(1 13 2025) '(1 1 2025) '(5 15 2025)
     '((1 13 2025)))
    (remacs-test-ical-monthly-hits
     "FREQ=MONTHLY;INTERVAL=2" '(1 10 2025) '(1 1 2025) '(6 15 2025)
     '((3 10 2025)) nil nil '((EXDATE nil "20250110") (EXDATE nil "20250510")))
    ;; Non-divisors of twelve and intervals longer than a year keep their phase.
    (dolist (case '(("FREQ=MONTHLY;INTERVAL=5" ((11 10 2024) (4 10 2025) (9 10 2025) (2 10 2026)
                                              (7 10 2026) (12 10 2026) (5 10 2027) (10 10 2027)
                                              (3 10 2028) (8 10 2028)))
                    ("FREQ=MONTHLY;INTERVAL=18" ((11 10 2024) (5 10 2026) (11 10 2027)))))
      (let* ((calendar-debug-sexp t)
             (generated (remacs-test-ical-monthly-format (car case) '(11 10 2024)))
             hits)
        (dotimes (i 49)
          (let ((day (list (1+ (mod (+ 9 i) 12)) 10 (+ 2024 (/ (+ 9 i) 12)))))
            (when (diary-sexp-entry (cdr generated) "event" day) (push day hits))))
        (unless (equal (nreverse hits) (cadr case))
          (error "Monthly interval lost its phase across years: %S" (car generated)))))
    (let* ((start '(0 0 9 13 1 2025 nil -1 nil))
           (event '(VEVENT nil ((RRULE nil "FREQ=MONTHLY;INTERVAL=2;BYDAY=2MO"))))
           (before (copy-tree (list event start))))
      (icalendar--convert-recurring-to-diary event start nil nil)
      (unless (equal (list event start) before) (error "Monthly conversion mutated its inputs"))))
  t)
"##;

fn check(interp: &mut Interp, source: &str) {
    match interp.eval_str(source) {
        Ok(value) => assert!(value.truthy()),
        Err(Flow::Signal(signal)) => {
            let symbol = interp.intern("remacs-test-ical-monthly-phase");
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
fn monthly_recurrences_respect_start_month_and_interval() {
    let mut interp = Interp::new();
    check(&mut interp, CASES);
    check(&mut interp, r#"(load "icalendar.el" nil t)"#);
    check(&mut interp, CASES);
}
