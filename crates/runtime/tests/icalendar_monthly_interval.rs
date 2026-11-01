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
  ;; Plain BYDAY selects every such weekday within an active month (#1565).
  (dolist (calendar-date-style '(american european iso))
    (dolist (times '((nil nil) ("09:00" "10:00")))
      (dolist (limited '(nil t))
        (remacs-test-ical-monthly-hits
         (concat "FREQ=MONTHLY;BYDAY=MO"
                 (if limited (if (car times) ";UNTIL=20250120T235959" ";UNTIL=20250120") ""))
         '(1 6 2025) '(1 1 2025) '(3 1 2025)
         (if limited '((1 6 2025) (1 13 2025) (1 20 2025))
           '((1 6 2025) (1 13 2025) (1 20 2025) (1 27 2025)
             (2 3 2025) (2 10 2025) (2 17 2025) (2 24 2025))) (car times) (cadr times)))))
  (let ((calendar-date-style 'american))
    ;; RFC 5545's every-Tuesday, every-other-month example.
    (remacs-test-ical-monthly-hits
     "FREQ=MONTHLY;INTERVAL=2;BYDAY=TU;UNTIL=19980131" '(9 2 1997) '(9 1 1997) '(2 28 1998)
     '((9 2 1997) (9 9 1997) (9 16 1997) (9 23 1997) (9 30 1997)
       (11 4 1997) (11 11 1997) (11 18 1997) (11 25 1997)
       (1 6 1998) (1 13 1998) (1 20 1998) (1 27 1998)))
    (remacs-test-ical-monthly-hits
     "FREQ=MONTHLY;INTERVAL=2;BYDAY=MO,WE" '(1 6 2025) '(1 1 2025) '(3 5 2025)
     '((1 6 2025) (1 8 2025) (1 13 2025) (1 15 2025) (1 20 2025) (1 22 2025)
       (1 27 2025) (1 29 2025) (3 3 2025) (3 5 2025)))
    (remacs-test-ical-monthly-hits
     "FREQ=MONTHLY;BYDAY=SU" '(1 5 2025) '(1 1 2025) '(2 2 2025)
     '((1 5 2025) (1 12 2025) (1 19 2025) (1 26 2025) (2 2 2025)))
    (remacs-test-ical-monthly-hits
     "FREQ=MONTHLY;BYDAY=MO" '(1 6 2025) '(1 1 2025) '(1 31 2025)
     '((1 6 2025) (1 20 2025)) nil nil '((EXDATE nil "20250113") (EXDATE nil "20250127")))
    (remacs-test-ical-monthly-hits
     "FREQ=MONTHLY;INTERVAL=2;BYDAY=MO" '(1 20 2025) '(1 1 2025) '(3 10 2025)
     '((1 20 2025) (1 27 2025) (3 3 2025) (3 10 2025)) "09:00" nil))
  (defun remacs-test-ical-monthly-probes (rule start candidates expected)
    (let* ((calendar-debug-sexp t)
           (generated (remacs-test-ical-monthly-format rule start))
           hits)
      (dolist (day candidates)
        (when (diary-sexp-entry (cdr generated) "event" day) (push day hits)))
      (setq hits (nreverse hits))
      (unless (equal hits expected)
        (error "Wrong monthly probes: %S expected %S, generated %S" hits expected (car generated)))))
  ;; BYMONTHDAY selects a set of days and retains the initial DTSTART (#1566).
  (dolist (calendar-date-style '(american european iso))
    (dolist (times '((nil nil) ("09:00" "10:00")))
      (remacs-test-ical-monthly-hits
       "FREQ=MONTHLY;BYMONTHDAY=1,15" '(1 10 2025) '(1 1 2025) '(4 1 2025)
       '((1 10 2025) (1 15 2025) (2 1 2025) (2 15 2025) (3 1 2025) (3 15 2025) (4 1 2025))
       (car times) (cadr times))
      (remacs-test-ical-monthly-hits
       (concat "FREQ=MONTHLY;INTERVAL=2;BYMONTHDAY=15,1,15;UNTIL="
               (if (car times) "20250315T235959" "20250315"))
       '(1 10 2025) '(1 1 2025) '(4 1 2025)
       '((1 10 2025) (1 15 2025) (3 1 2025) (3 15 2025)) (car times) (cadr times))
      (remacs-test-ical-monthly-hits
       "FREQ=MONTHLY;BYMONTHDAY=1,15;COUNT=3" '(1 10 2025) '(1 1 2025) '(4 1 2025)
       '((1 10 2025) (1 15 2025) (2 1 2025)) (car times) (cadr times))))
  (let ((calendar-date-style 'american))
    (remacs-test-ical-monthly-hits
     "FREQ=MONTHLY;BYMONTHDAY=31,-1,31;COUNT=4" '(1 10 2025) '(1 1 2025) '(5 1 2025)
     '((1 10 2025) (1 31 2025) (2 28 2025) (3 31 2025)))
    (remacs-test-ical-monthly-hits
     "FREQ=MONTHLY;BYMONTHDAY=+1,-1;COUNT=5" '(1 1 2025) '(1 1 2025) '(4 1 2025)
     '((1 1 2025) (1 31 2025) (2 1 2025) (2 28 2025) (3 1 2025)))
    (remacs-test-ical-monthly-hits
     "FREQ=MONTHLY;BYMONTHDAY=31;COUNT=4" '(1 31 2025) '(1 1 2025) '(8 1 2025)
     '((1 31 2025) (3 31 2025) (5 31 2025) (7 31 2025)))
    (remacs-test-ical-monthly-hits
     "FREQ=MONTHLY;BYMONTHDAY=1,15;COUNT=4" '(1 10 2025) '(1 1 2025) '(4 1 2025)
     '((1 15 2025) (2 1 2025)) nil nil '((EXDATE nil "20250110") (EXDATE nil "20250215")))
    (remacs-test-ical-monthly-hits
     "FREQ=MONTHLY;BYMONTHDAY=8,9,10,11,12,13,14;BYDAY=2MO;COUNT=4"
     '(1 10 2025) '(1 1 2025) '(5 1 2025)
     '((1 10 2025) (1 13 2025) (2 10 2025) (3 10 2025)))
    (remacs-test-ical-monthly-hits
     "FREQ=MONTHLY;BYMONTHDAY=-1,-2,-3,-4,-5,-6,-7;BYDAY=-1FR;COUNT=3"
     '(1 10 2025) '(1 1 2025) '(4 1 2025) '((1 10 2025) (1 31 2025) (2 28 2025)))
    (remacs-test-ical-monthly-probes
     "FREQ=MONTHLY;BYMONTHDAY=1,15;BYDAY=MO,WE;COUNT=3" '(1 10 2025)
     '((1 1 2025) (1 10 2025) (1 15 2025) (2 1 2025) (9 1 2025) (9 15 2025))
     '((1 10 2025) (1 15 2025) (9 1 2025)))
    ;; A fifth weekday must belong to the queried month, never spill from the previous one.
    (remacs-test-ical-monthly-probes
     "FREQ=MONTHLY;BYMONTHDAY=3,29;BYDAY=5MO" '(1 10 2025)
     '((1 10 2025) (3 3 2025) (6 2 2025) (9 29 2025)) '((1 10 2025) (9 29 2025)))
    (remacs-test-ical-monthly-probes
     "FREQ=MONTHLY;BYMONTHDAY=3;BYDAY=5MO;COUNT=100" '(1 10 2025)
     '((1 10 2025) (3 3 2025) (9 29 2025)) '((1 10 2025)))
    (remacs-test-ical-monthly-probes
     "FREQ=MONTHLY;BYMONTHDAY=1,15;COUNT=1000000" '(1 1 2025)
     '((1 1 2025) (8 1 43691) (8 15 43691) (9 1 43691))
     '((1 1 2025) (8 1 43691) (8 15 43691)))
    (remacs-test-ical-monthly-probes
     "FREQ=MONTHLY;INTERVAL=4800;BYMONTHDAY=1,15;COUNT=3" '(1 10 2025)
     '((1 10 2025) (1 15 2025) (1 1 2425) (1 15 2425))
     '((1 10 2025) (1 15 2025) (1 1 2425)))
    (remacs-test-ical-monthly-probes
     "FREQ=MONTHLY;INTERVAL=18;BYMONTHDAY=31;COUNT=3" '(12 31 2023)
     '((12 31 2023) (6 30 2025) (12 31 2026) (6 30 2028) (12 31 2029) (12 31 2032))
     '((12 31 2023) (12 31 2026) (12 31 2029)))
    (remacs-test-ical-monthly-probes
     "FREQ=MONTHLY;INTERVAL=12;BYMONTHDAY=29;COUNT=3" '(2 28 2023)
     '((2 28 2023) (2 29 2024) (3 1 2025) (2 29 2028) (2 29 2032))
     '((2 28 2023) (2 29 2024) (2 29 2028)))
    (remacs-test-ical-monthly-probes
     "FREQ=MONTHLY;INTERVAL=4800;BYMONTHDAY=29;COUNT=100" '(2 28 2025)
     '((2 28 2025) (3 1 2025) (2 28 2425) (3 1 2425)) '((2 28 2025)))
    (remacs-test-ical-monthly-probes
     "FREQ=MONTHLY;BYMONTHDAY=1,15" '(1 10 2025)
     '((1 1 10001) (1 15 10001)) '((1 1 10001) (1 15 10001)))
    (let* ((start '(0 0 9 10 1 2025 nil -1 nil))
           (event '(VEVENT nil ((RRULE nil "FREQ=MONTHLY;BYMONTHDAY=15,1,15;COUNT=3"))))
           (before (copy-tree (list event start))))
      (icalendar--convert-recurring-to-diary event start nil nil)
      (unless (equal (list event start) before) (error "BYMONTHDAY conversion mutated its inputs")))
    (dolist (rule '("FREQ=MONTHLY;BYMONTHDAY=0" "FREQ=MONTHLY;BYMONTHDAY=32"
                     "FREQ=MONTHLY;BYMONTHDAY=-32" "FREQ=MONTHLY;BYMONTHDAY=1,NO"
                     "FREQ=MONTHLY;BYMONTHDAY=1;COUNT=0" "FREQ=MONTHLY;BYMONTHDAY=1;COUNT=-1"
                     "FREQ=MONTHLY;BYMONTHDAY=1;COUNT=1.5"
                     "FREQ=MONTHLY;BYMONTHDAY=1;COUNT=2;INTERVAL=0"))
      (unless (condition-case nil
                  (progn (remacs-test-ical-monthly-format rule '(1 10 2025)) nil)
                (error t))
        (error "An invalid monthly day/count/interval was accepted: %S" rule))))
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
fn monthly_recurrences_preserve_days_intervals_and_counts() {
    let mut interp = Interp::new();
    check(&mut interp, CASES);
    check(&mut interp, r#"(load "icalendar.el" nil t)"#);
    check(&mut interp, CASES);
}
