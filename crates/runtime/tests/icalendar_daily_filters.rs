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
  (defun remacs-test-ical-daily-probes (rule start candidates expected &optional start-time end-time extras)
    (let* ((calendar-debug-sexp t)
           (generated (remacs-test-ical-daily-format rule start start-time end-time extras))
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
  ;; DAILY BYMONTH limits months while preserving all eligible dates within them (#1569).
  (dolist (calendar-date-style '(american european iso))
    (dolist (times '((nil nil) ("09:00" "10:00")))
      (remacs-test-ical-daily-hits
       "FREQ=DAILY;BYMONTH=1" '(1 1 2025) '(1 1 2025) '(2 3 2025)
       (mapcar (lambda (day) (list 1 day 2025)) (number-sequence 1 31)) (car times) (cadr times))
      (remacs-test-ical-daily-hits
       (concat "FREQ=DAILY;BYMONTH=1;UNTIL=" (if (car times) "20250117T235959" "20250117"))
       '(1 15 2025) '(1 1 2025) '(2 3 2025) '((1 15 2025) (1 16 2025) (1 17 2025))
       (car times) (cadr times))
      (remacs-test-ical-daily-probes
       "FREQ=DAILY;BYMONTH=1;COUNT=33" '(1 1 2025)
       '((1 1 2025) (1 31 2025) (2 1 2025) (1 1 2026) (1 2 2026) (1 3 2026))
       '((1 1 2025) (1 31 2025) (1 1 2026) (1 2 2026)) (car times) (cadr times))))
  (let ((calendar-date-style 'american))
    (remacs-test-ical-daily-probes
     "FREQ=DAILY;BYMONTH=7,1,7;COUNT=33" '(6 30 2025)
     '((6 30 2025) (7 1 2025) (7 31 2025) (8 1 2025) (1 1 2026) (1 2 2026))
     '((6 30 2025) (7 1 2025) (7 31 2025) (1 1 2026)))
    (remacs-test-ical-daily-probes
     "FREQ=DAILY;BYMONTH=2;BYDAY=MO;COUNT=5" '(1 6 2025)
     '((1 6 2025) (1 13 2025) (2 1 2025) (2 3 2025) (2 10 2025) (2 17 2025) (2 24 2025) (2 2 2026))
     '((1 6 2025) (2 3 2025) (2 10 2025) (2 17 2025) (2 24 2025)))
    (remacs-test-ical-daily-probes
     "FREQ=DAILY;BYMONTH=2;BYMONTHDAY=-1;COUNT=5" '(1 10 2025)
     '((1 10 2025) (2 28 2025) (3 31 2025) (2 28 2026) (2 28 2027) (2 29 2028) (2 28 2029))
     '((1 10 2025) (2 28 2025) (2 28 2026) (2 28 2027) (2 29 2028)))
    (remacs-test-ical-daily-probes
     "FREQ=DAILY;BYMONTH=2;BYMONTHDAY=1,15;BYDAY=MO,WE;COUNT=3" '(1 10 2025)
     '((1 10 2025) (1 15 2025) (2 1 2025) (2 1 2026) (2 1 2027) (2 15 2027) (2 1 2034))
     '((1 10 2025) (2 1 2027) (2 15 2027)))
    (remacs-test-ical-daily-probes
     "FREQ=DAILY;BYMONTH=1;INTERVAL=2;COUNT=18" '(1 1 2025)
     '((1 1 2025) (1 3 2025) (1 31 2025) (2 2 2025) (1 1 2026) (1 2 2026) (1 4 2026) (1 6 2026))
     '((1 1 2025) (1 3 2025) (1 31 2025) (1 2 2026) (1 4 2026)))
    (remacs-test-ical-daily-probes
     "FREQ=DAILY;BYMONTH=2;BYMONTHDAY=1,15;INTERVAL=5;COUNT=4" '(1 1 2025)
     '((1 1 2025) (2 1 2025) (2 15 2025) (2 15 2026) (2 15 2027) (2 15 2028))
     '((1 1 2025) (2 15 2025) (2 15 2026) (2 15 2027)))
    (remacs-test-ical-daily-probes
     "FREQ=DAILY;BYMONTH=2;BYMONTHDAY=31;INTERVAL=2;COUNT=100" '(1 31 2025)
     '((1 31 2025) (2 28 2025) (3 3 2025) (1 31 2026)) '((1 31 2025)))
    (remacs-test-ical-daily-probes
     "FREQ=DAILY;BYMONTH=2;INTERVAL=146097;COUNT=100" '(1 1 2025)
     '((1 1 2025) (1 1 2425) (2 1 2425)) '((1 1 2025)))
    (remacs-test-ical-daily-probes
     "FREQ=DAILY;BYMONTH=2;INTERVAL=146097;COUNT=3" '(2 1 2025)
     '((2 1 2025) (2 1 2425) (2 1 2825) (2 1 3225))
     '((2 1 2025) (2 1 2425) (2 1 2825)))
    ;; Independent enumeration covers the selected months as well as daily phase.
    (remacs-test-ical-daily-probes
     "FREQ=DAILY;BYMONTH=1,7;BYMONTHDAY=1,15;COUNT=1000000" '(1 1 2025)
     '((1 1 2025) (7 15 252024) (1 1 252025)) '((1 1 2025) (7 15 252024)))
    (remacs-test-ical-daily-probes
     "FREQ=DAILY;BYMONTH=1,7;BYMONTHDAY=1,15;INTERVAL=2;COUNT=1000000" '(1 1 2025)
     '((1 1 2025) (7 15 502024) (1 1 502025)) '((1 1 2025) (7 15 502024)))
    (remacs-test-ical-daily-probes
     "FREQ=DAILY;BYMONTH=1;COUNT=34" '(1 1 2025)
     '((1 1 2025) (1 31 2025) (1 1 2026) (1 2 2026) (1 3 2026) (1 4 2026))
     '((1 31 2025) (1 1 2026) (1 2 2026)) nil nil '((EXDATE nil "20250101") (EXDATE nil "20260103")))
    (let* ((calendar-debug-sexp t)
           (generated (remacs-test-ical-daily-format "FREQ=DAILY;BYMONTH=1;INTERVAL=2" '(1 1 2025)))
           (entry (diary-sexp-entry (cdr generated) "repeat %d" '(1 2 2026))))
      (unless (equal (if (consp entry) (cdr entry) entry) "repeat 183")
        (error "The month filter reset the cyclic phase or lost its formatted entry")))
    (remacs-test-ical-daily-probes
     "FREQ=DAILY;BYMONTH=1,7" '(1 1 2025)
     '((1 1 10001) (2 1 10001) (7 1 10001)) '((1 1 10001) (7 1 10001)))
    (let* ((start '(0 0 9 1 1 2025 nil -1 nil))
           (event '(VEVENT nil ((RRULE nil "FREQ=DAILY;BYMONTH=7,1,7;BYMONTHDAY=1,15;BYDAY=MO,WE;COUNT=4"))))
           (before (copy-tree (list event start))))
      (icalendar--convert-recurring-to-diary event start nil nil)
      (unless (equal (list event start) before) (error "Daily month conversion mutated its inputs")))
    (dolist (rule '("FREQ=DAILY;BYMONTH=0" "FREQ=DAILY;BYMONTH=13" "FREQ=DAILY;BYMONTH=1,NO"
                     "FREQ=DAILY;BYMONTH=1;COUNT=0" "FREQ=DAILY;BYMONTH=1;COUNT=1.5"
                     "FREQ=DAILY;BYMONTH=1;COUNT=2;INTERVAL=0"))
      (unless (condition-case nil
                  (progn (remacs-test-ical-daily-format rule '(1 1 2025)) nil)
                (error t))
        (error "An invalid daily month/count/interval was accepted: %S" rule))))
  ;; Daily BYMONTHDAY limits calendar dates without resetting the day-step phase (#1568).
  (dolist (calendar-date-style '(american european iso))
    (dolist (times '((nil nil) ("09:00" "10:00")))
      (remacs-test-ical-daily-hits
       "FREQ=DAILY;BYMONTHDAY=1,15" '(1 1 2025) '(1 1 2025) '(3 15 2025)
       '((1 1 2025) (1 15 2025) (2 1 2025) (2 15 2025) (3 1 2025) (3 15 2025))
       (car times) (cadr times))
      (remacs-test-ical-daily-hits
       (concat "FREQ=DAILY;BYMONTHDAY=1,15;UNTIL="
               (if (car times) "20250201T235959" "20250201"))
       '(1 1 2025) '(1 1 2025) '(3 15 2025)
       '((1 1 2025) (1 15 2025) (2 1 2025)) (car times) (cadr times))
      (remacs-test-ical-daily-hits
       "FREQ=DAILY;BYMONTHDAY=15,1,15;COUNT=3" '(1 10 2025) '(1 1 2025) '(3 15 2025)
       '((1 10 2025) (1 15 2025) (2 1 2025)) (car times) (cadr times))))
  (let ((calendar-date-style 'american))
    (remacs-test-ical-daily-hits
     "FREQ=DAILY;BYMONTHDAY=1,15;INTERVAL=2;COUNT=4" '(1 1 2025) '(1 1 2025) '(5 20 2025)
     '((1 1 2025) (1 15 2025) (4 1 2025) (4 15 2025)) "09:00" nil)
    (remacs-test-ical-daily-probes
     "FREQ=DAILY;BYMONTHDAY=1,15;INTERVAL=3;COUNT=4" '(1 10 2025)
     '((1 10 2025) (1 15 2025) (2 1 2025) (2 15 2025) (4 1 2025) (5 1 2025) (6 15 2025))
     '((1 10 2025) (2 15 2025) (4 1 2025) (5 1 2025)))
    (remacs-test-ical-daily-hits
     "FREQ=DAILY;BYMONTHDAY=31,-1,31;COUNT=4" '(1 10 2025) '(1 1 2025) '(5 1 2025)
     '((1 10 2025) (1 31 2025) (2 28 2025) (3 31 2025)))
    (remacs-test-ical-daily-hits
     "FREQ=DAILY;BYMONTHDAY=-31;COUNT=3" '(1 1 2025) '(1 1 2025) '(6 1 2025)
     '((1 1 2025) (3 1 2025) (5 1 2025)))
    (remacs-test-ical-daily-probes
     "FREQ=DAILY;BYMONTHDAY=1,15;BYDAY=MO,WE;COUNT=3" '(1 10 2025)
     '((1 1 2025) (1 10 2025) (1 15 2025) (2 1 2025) (9 1 2025) (9 15 2025))
     '((1 10 2025) (1 15 2025) (9 1 2025)))
    (remacs-test-ical-daily-probes
     "FREQ=DAILY;BYMONTHDAY=1,15;BYDAY=MO,WE;INTERVAL=2;COUNT=3" '(1 1 2025)
     '((1 1 2025) (1 15 2025) (4 1 2025) (9 1 2025) (10 1 2025) (12 1 2025) (12 15 2025))
     '((1 1 2025) (1 15 2025) (12 1 2025)))
    (remacs-test-ical-daily-probes
     "FREQ=DAILY;BYMONTHDAY=1;BYDAY=WE;INTERVAL=7;COUNT=100" '(1 6 2025)
     '((1 6 2025) (1 13 2025) (10 1 2025)) '((1 6 2025)))
    (remacs-test-ical-daily-probes
     "FREQ=DAILY;BYMONTHDAY=1;INTERVAL=146097;COUNT=3" '(1 1 2025)
     '((1 1 2025) (1 1 2425) (1 1 2825) (1 1 3225))
     '((1 1 2025) (1 1 2425) (1 1 2825)))
    (remacs-test-ical-daily-probes
     "FREQ=DAILY;BYMONTHDAY=1;INTERVAL=146097;COUNT=100" '(1 2 2025)
     '((1 2 2025) (1 1 2425) (1 2 2425)) '((1 2 2025)))
    (remacs-test-ical-daily-hits
     "FREQ=DAILY;BYMONTHDAY=29;COUNT=3" '(2 28 2024) '(2 27 2024) '(4 30 2024)
     '((2 28 2024) (2 29 2024) (3 29 2024)))
    (remacs-test-ical-daily-probes
     "FREQ=DAILY;BYMONTHDAY=29;INTERVAL=2;COUNT=3" '(2 28 2024)
     '((2 28 2024) (2 29 2024) (3 29 2024) (4 29 2024) (5 29 2024) (6 29 2024) (7 29 2024))
     '((2 28 2024) (3 29 2024) (6 29 2024)))
    (remacs-test-ical-daily-hits
     "FREQ=DAILY;BYMONTHDAY=1,15;COUNT=4" '(1 1 2025) '(1 1 2025) '(3 20 2025)
     '((1 15 2025) (2 1 2025)) nil nil '((EXDATE nil "20250101") (EXDATE nil "20250215")))
    (let* ((calendar-debug-sexp t)
           (generated (remacs-test-ical-daily-format "FREQ=DAILY;BYMONTHDAY=1,15;INTERVAL=2" '(1 1 2025)))
           (entry (diary-sexp-entry (cdr generated) "repeat %d" '(1 15 2025))))
      (unless (equal (if (consp entry) (cdr entry) entry) "repeat 7")
        (error "The month-day filter lost diary-cyclic's formatted entry")))
    ;; Independent month-by-month enumeration supplied these endpoints and next matches.
    (remacs-test-ical-daily-probes
     "FREQ=DAILY;BYMONTHDAY=1,15;COUNT=1000000" '(1 1 2025)
     '((1 1 2025) (8 15 43691) (9 1 43691)) '((1 1 2025) (8 15 43691)))
    (remacs-test-ical-daily-probes
     "FREQ=DAILY;BYMONTHDAY=1,15;INTERVAL=2;COUNT=1000000" '(1 1 2025)
     '((1 1 2025) (3 15 85358) (6 1 85358)) '((1 1 2025) (3 15 85358)))
    (remacs-test-ical-daily-probes
     "FREQ=DAILY;BYMONTHDAY=1,15;INTERVAL=37;COUNT=1000" '(1 1 2025)
     '((1 1 2025) (11 1 3543) (6 15 3545)) '((1 1 2025) (11 1 3543)))
    (let* ((start '(0 0 9 10 1 2025 nil -1 nil))
           (event '(VEVENT nil ((RRULE nil "FREQ=DAILY;BYMONTHDAY=15,1,15;INTERVAL=2;COUNT=4"))))
           (before (copy-tree (list event start))))
      (icalendar--convert-recurring-to-diary event start nil nil)
      (unless (equal (list event start) before) (error "Daily month-day conversion mutated its inputs")))
    (dolist (rule '("FREQ=DAILY;BYMONTHDAY=0" "FREQ=DAILY;BYMONTHDAY=32"
                     "FREQ=DAILY;BYMONTHDAY=-32" "FREQ=DAILY;BYMONTHDAY=NO"
                     "FREQ=DAILY;BYMONTHDAY=1;COUNT=0" "FREQ=DAILY;BYMONTHDAY=1;COUNT=1.5"
                     "FREQ=DAILY;BYMONTHDAY=1;COUNT=2;INTERVAL=0"))
      (unless (condition-case nil
                  (progn (remacs-test-ical-daily-format rule '(1 1 2025)) nil)
                (error t))
        (error "An invalid daily month-day/count/interval was accepted: %S" rule))))
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
fn daily_recurrences_apply_calendar_filters() {
    let mut interp = Interp::new();
    check(&mut interp, CASES);
    check(&mut interp, r#"(load "icalendar.el" nil t)"#);
    check(&mut interp, CASES);
}
