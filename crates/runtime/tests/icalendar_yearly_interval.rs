use remacs_runtime::lisp::{Flow, Interp};

const CASES: &str = r##"
;;; -*- lexical-binding: t; -*-
(progn
  (require 'icalendar)
  (defvar remacs-test-ical-yearly-phase nil)
  (defun remacs-test-ical-yearly-format (rule start &optional start-time end-time extras)
    (setq remacs-test-ical-yearly-phase (list calendar-date-style rule start start-time end-time))
    (let* ((event (list 'VEVENT nil (append (list (list 'RRULE nil rule)) extras)))
           (decoded (list 0 0 (if start-time 9 0) (cadr start) (car start) (nth 2 start) nil -1 nil))
           (line (icalendar--convert-recurring-to-diary event decoded start-time end-time))
           (parsed (read-from-string line 2)))
      (unless (string-prefix-p "%%(" line) (error "No diary expression: %S" line))
      (unless (equal (string-trim (substring line (cdr parsed)))
                     (concat (or start-time "") (if end-time "-" "") (or end-time "")))
        (error "Yearly conversion changed the displayed time: %S" line))
      (cons line (substring line 2 (cdr parsed)))))
  (defun remacs-test-ical-yearly-hits (rule start candidates expected &optional start-time end-time extras)
    (let* ((calendar-debug-sexp t)
           (generated (remacs-test-ical-yearly-format rule start start-time end-time extras))
           hits)
      (dolist (day candidates)
        (let ((entry (diary-sexp-entry (cdr generated) "event" day)))
          (when entry
            (unless (equal (if (consp entry) (cdr entry) entry) "event")
              (error "Yearly predicate returned invalid entry data: %S" entry))
            (push day hits))))
      (setq hits (nreverse hits))
      (unless (equal hits expected)
        (error "Wrong yearly occurrences: %S expected %S, generated %S"
               hits expected (car generated)))
      generated))
  (let (candidates)
    ;; Probe matching/adjacent days and other months before and after DTSTART.
    (dotimes (i 10)
      (let ((year (+ 2023 i)))
        (setq candidates
              (append candidates (list (list 1 5 year) (list 1 6 year) (list 1 7 year)
                                       (list 2 6 year) (list 12 6 year))))))
    (dolist (calendar-date-style '(american european iso))
      (dolist (times '((nil nil) ("09:00" "10:00")))
        (dolist (limited '(nil t))
          (remacs-test-ical-yearly-hits
           (concat "FREQ=YEARLY;INTERVAL=2"
                   (if limited (if (car times) ";UNTIL=20290106T235959" ";UNTIL=20290106") ""))
           '(1 6 2025) candidates
           (if limited '((1 6 2025) (1 6 2027) (1 6 2029))
             '((1 6 2025) (1 6 2027) (1 6 2029) (1 6 2031)))
           (car times) (cadr times)))))
    (let ((calendar-date-style 'american))
      (remacs-test-ical-yearly-hits
       "FREQ=YEARLY;INTERVAL=3" '(1 6 2025) candidates
       '((1 6 2025) (1 6 2028) (1 6 2031)) "09:00" nil)
      (remacs-test-ical-yearly-hits
       "FREQ=YEARLY;INTERVAL=2;COUNT=3" '(1 6 2025) candidates
       '((1 6 2025) (1 6 2027) (1 6 2029)))
      (remacs-test-ical-yearly-hits
       "FREQ=YEARLY;INTERVAL=2;UNTIL=20290105" '(1 6 2025) candidates
       '((1 6 2025) (1 6 2027)))
      (remacs-test-ical-yearly-hits
       "FREQ=YEARLY;INTERVAL=2;UNTIL=20280106" '(1 6 2025) candidates
       '((1 6 2025) (1 6 2027)))
      (remacs-test-ical-yearly-hits
       "FREQ=YEARLY;INTERVAL=2" '(1 6 2025) candidates
       '((1 6 2027) (1 6 2031)) nil nil '((EXDATE nil "20250106") (EXDATE nil "20290106")))
      (remacs-test-ical-yearly-hits
       "FREQ=YEARLY;INTERVAL=1" '(1 6 2025) candidates
       '((1 6 2025) (1 6 2026) (1 6 2027) (1 6 2028)
         (1 6 2029) (1 6 2030) (1 6 2031) (1 6 2032)))
      ;; The interval filter must preserve diary-anniversary's formatted entry.
      (let* ((calendar-debug-sexp t)
             (generated (remacs-test-ical-yearly-format "FREQ=YEARLY;INTERVAL=2" '(1 6 2025)))
             (entry (diary-sexp-entry (cdr generated) "anniversary %d%s" '(1 6 2027))))
        (unless (equal (if (consp entry) (cdr entry) entry) "anniversary 3rd")
          (error "The yearly interval filter lost the anniversary's formatted entry")))))
  (defun remacs-test-ical-yearly-range (rule start begin end expected &optional start-time end-time)
    (let ((cursor (calendar-absolute-from-gregorian begin))
          (last (calendar-absolute-from-gregorian end)) candidates)
      (while (<= cursor last)
        (push (calendar-gregorian-from-absolute cursor) candidates)
        (setq cursor (1+ cursor)))
      (remacs-test-ical-yearly-hits rule start (nreverse candidates) expected start-time end-time)))
  ;; Without BYMONTH, yearly BYMONTHDAY expands across the year's months (#1571).
  (dolist (calendar-date-style '(american european iso))
    (dolist (times '((nil nil) ("09:00" "10:00")))
      (remacs-test-ical-yearly-range
       "FREQ=YEARLY;BYMONTHDAY=1" '(6 10 2025) '(6 1 2025) '(1 3 2026)
       '((6 10 2025) (7 1 2025) (8 1 2025) (9 1 2025) (10 1 2025) (11 1 2025) (12 1 2025) (1 1 2026))
       (car times) (cadr times))
      (remacs-test-ical-yearly-range
       (concat "FREQ=YEARLY;BYMONTHDAY=1;UNTIL=" (if (car times) "20251001T235959" "20251001"))
       '(6 10 2025) '(6 1 2025) '(1 3 2026)
       '((6 10 2025) (7 1 2025) (8 1 2025) (9 1 2025) (10 1 2025)) (car times) (cadr times))
      (remacs-test-ical-yearly-range
       "FREQ=YEARLY;BYMONTHDAY=1;COUNT=3" '(6 10 2025) '(6 1 2025) '(1 3 2026)
       '((6 10 2025) (7 1 2025) (8 1 2025)) (car times) (cadr times))))
  (let ((calendar-date-style 'american))
    (remacs-test-ical-yearly-hits
     "FREQ=YEARLY;INTERVAL=2;BYMONTHDAY=1;COUNT=8" '(6 10 2025)
     '((6 1 2025) (6 10 2025) (7 1 2025) (12 1 2025) (1 1 2026) (6 1 2026) (1 1 2027) (2 1 2027))
     '((6 10 2025) (7 1 2025) (12 1 2025) (1 1 2027)))
    (remacs-test-ical-yearly-hits
     "FREQ=YEARLY;BYMONTH=12,6,12;BYMONTHDAY=15,1,15;COUNT=5" '(6 10 2025)
     '((6 1 2025) (6 10 2025) (6 15 2025) (7 1 2025) (12 1 2025) (12 15 2025) (6 1 2026) (6 15 2026))
     '((6 10 2025) (6 15 2025) (12 1 2025) (12 15 2025) (6 1 2026)))
    (remacs-test-ical-yearly-range
     "FREQ=YEARLY;BYMONTHDAY=31,-1,31;COUNT=4" '(1 10 2025) '(1 1 2025) '(5 1 2025)
     '((1 10 2025) (1 31 2025) (2 28 2025) (3 31 2025)))
    (remacs-test-ical-yearly-range
     "FREQ=YEARLY;BYMONTHDAY=-31;COUNT=3" '(1 1 2025) '(1 1 2025) '(8 1 2025)
     '((1 1 2025) (3 1 2025) (5 1 2025)))
    (remacs-test-ical-yearly-range
     "FREQ=YEARLY;BYMONTHDAY=1,-1;COUNT=5" '(1 1 2025) '(1 1 2025) '(4 1 2025)
     '((1 1 2025) (1 31 2025) (2 1 2025) (2 28 2025) (3 1 2025)))
    (remacs-test-ical-yearly-hits
     "FREQ=YEARLY;BYMONTH=2;BYMONTHDAY=29;COUNT=3" '(1 31 2023)
     '((1 31 2023) (3 1 2023) (2 29 2024) (3 1 2025) (2 29 2028) (2 29 2032))
     '((1 31 2023) (2 29 2024) (2 29 2028)))
    (remacs-test-ical-yearly-hits
     "FREQ=YEARLY;INTERVAL=100;BYMONTH=2;BYMONTHDAY=29;COUNT=3" '(2 29 2000)
     '((2 29 2000) (3 1 2100) (3 1 2200) (3 1 2300) (2 29 2400) (2 29 2800) (2 29 3200))
     '((2 29 2000) (2 29 2400) (2 29 2800)))
    (remacs-test-ical-yearly-hits
     "FREQ=YEARLY;INTERVAL=400;BYMONTH=2;BYMONTHDAY=29;COUNT=3" '(2 29 2000)
     '((2 29 2000) (2 29 2400) (2 29 2800) (2 29 3200))
     '((2 29 2000) (2 29 2400) (2 29 2800)))
    (remacs-test-ical-yearly-hits
     "FREQ=YEARLY;INTERVAL=400;BYMONTH=2;BYMONTHDAY=31;COUNT=100" '(1 31 2025)
     '((1 31 2025) (2 28 2025) (3 3 2025) (1 31 2425)) '((1 31 2025)))
    (remacs-test-ical-yearly-hits
     "FREQ=YEARLY;BYMONTHDAY=1;COUNT=4" '(6 10 2025)
     '((6 10 2025) (7 1 2025) (8 1 2025) (9 1 2025) (10 1 2025))
     '((7 1 2025) (8 1 2025)) nil nil '((EXDATE nil "20250610") (EXDATE nil "20250901")))
    (remacs-test-ical-yearly-hits
     "FREQ=YEARLY;BYMONTHDAY=1,-31;BYDAY=SU;COUNT=3" '(1 1 2023)
     '((1 1 2023) (7 1 2023) (10 1 2023) (9 1 2024) (12 1 2024) (3 1 2026))
     '((1 1 2023) (10 1 2023) (9 1 2024)))
    (remacs-test-ical-yearly-hits
     "FREQ=YEARLY;BYMONTHDAY=1" '(6 10 2025)
     '((1 1 10001) (1 10 10001) (2 1 10001)) '((1 1 10001) (2 1 10001)))
    ;; Count endpoints were independently enumerated without Gregorian cycle jumps.
    (remacs-test-ical-yearly-hits
     "FREQ=YEARLY;BYMONTHDAY=1;COUNT=1000000" '(6 10 2025)
     '((6 10 2025) (9 1 85358) (10 1 85358)) '((6 10 2025) (9 1 85358)))
    (remacs-test-ical-yearly-hits
     "FREQ=YEARLY;INTERVAL=2;BYMONTHDAY=1,-1;COUNT=1000000" '(6 10 2025)
     '((6 10 2025) (1 31 85359) (2 1 85359)) '((6 10 2025) (1 31 85359)))
    (remacs-test-ical-yearly-hits
     "FREQ=YEARLY;BYMONTHDAY=31;COUNT=1000000" '(6 10 2025)
     '((6 10 2025) (5 31 144882) (7 31 144882)) '((6 10 2025) (5 31 144882)))
    (let* ((start '(0 0 9 10 6 2025 nil -1 nil))
           (event '(VEVENT nil ((RRULE nil "FREQ=YEARLY;BYMONTH=12,6;BYMONTHDAY=15,1,15;COUNT=4"))))
           (before (copy-tree (list event start))))
      (icalendar--convert-recurring-to-diary event start nil nil)
      (unless (equal (list event start) before) (error "Yearly month-day conversion mutated its inputs")))
    (dolist (rule '("FREQ=YEARLY;BYMONTHDAY=0" "FREQ=YEARLY;BYMONTHDAY=32"
                     "FREQ=YEARLY;BYMONTHDAY=-32" "FREQ=YEARLY;BYMONTHDAY=1,NO"
                     "FREQ=YEARLY;BYMONTHDAY=1;COUNT=0" "FREQ=YEARLY;BYMONTHDAY=1;COUNT=1.5"
                     "FREQ=YEARLY;BYMONTHDAY=1;COUNT=2;INTERVAL=0"))
      (unless (condition-case nil
                  (progn (remacs-test-ical-yearly-format rule '(6 10 2025)) nil)
                (error t))
        (error "An invalid yearly month-day/count/interval was accepted: %S" rule))))
  ;; YEARLY BYDAY uses month ordinals with BYMONTH, year ordinals otherwise (#1570).
  (let ((candidates '((1 14 2025) (1 15 2025) (1 16 2025) (8 31 2025)
                       (9 1 2025) (9 2 2025) (9 8 2025) (9 15 2025) (1 15 2026)
                       (9 1 2026) (9 7 2026) (9 14 2026) (9 6 2027))))
    (dolist (calendar-date-style '(american european iso))
      (dolist (times '((nil nil) ("09:00" "10:00")))
        (remacs-test-ical-yearly-hits
         "FREQ=YEARLY;BYMONTH=9;BYDAY=1MO" '(1 15 2025) candidates
         '((1 15 2025) (9 1 2025) (9 7 2026) (9 6 2027)) (car times) (cadr times))
        (remacs-test-ical-yearly-hits
         (concat "FREQ=YEARLY;BYMONTH=9;BYDAY=1MO;UNTIL="
                 (if (car times) "20260907T235959" "20260907"))
         '(1 15 2025) candidates '((1 15 2025) (9 1 2025) (9 7 2026)) (car times) (cadr times))
        (remacs-test-ical-yearly-hits
         "FREQ=YEARLY;BYMONTH=9;BYDAY=1MO;COUNT=2" '(1 15 2025) candidates
         '((1 15 2025) (9 1 2025)) (car times) (cadr times)))))
  (dolist (calendar-date-style '(american european iso))
    (dolist (times '((nil nil) ("09:00" "10:00")))
      (remacs-test-ical-yearly-range
       "FREQ=YEARLY;BYDAY=MO,WE" '(1 15 2025) '(1 1 2025) '(2 6 2025)
       '((1 15 2025) (1 20 2025) (1 22 2025) (1 27 2025) (1 29 2025) (2 3 2025) (2 5 2025))
       (car times) (cadr times))
      (remacs-test-ical-yearly-range
       "FREQ=YEARLY;BYDAY=WE,MO,WE;COUNT=5" '(1 15 2025) '(1 1 2025) '(2 6 2025)
       '((1 15 2025) (1 20 2025) (1 22 2025) (1 27 2025) (1 29 2025)) (car times) (cadr times))))
  (let ((calendar-date-style 'american))
    (remacs-test-ical-yearly-hits
     "FREQ=YEARLY;INTERVAL=2;BYMONTH=9,1,9;BYDAY=-1FR,1MO,-1FR;COUNT=5" '(1 15 2025)
     '((1 6 2025) (1 15 2025) (1 31 2025) (9 1 2025) (9 26 2025) (1 5 2026)
       (1 4 2027) (1 29 2027))
     '((1 15 2025) (1 31 2025) (9 1 2025) (9 26 2025) (1 4 2027)))
    (remacs-test-ical-yearly-hits
     "FREQ=YEARLY;BYMONTH=9;BYDAY=MO,1MO;COUNT=4" '(1 15 2025)
     '((1 15 2025) (9 1 2025) (9 8 2025) (9 15 2025) (9 22 2025))
     '((1 15 2025) (9 1 2025) (9 8 2025) (9 15 2025)))
    (remacs-test-ical-yearly-hits
     "FREQ=YEARLY;BYMONTH=9;BYDAY=5MO;COUNT=2" '(1 15 2025)
     '((1 15 2025) (9 1 2025) (9 22 2025) (9 29 2025) (10 6 2025))
     '((1 15 2025) (9 29 2025)))
    (remacs-test-ical-yearly-hits
     "FREQ=YEARLY;BYMONTH=9;BYDAY=6MO;COUNT=10" '(1 15 2025)
     '((1 15 2025) (9 1 2025) (9 29 2025) (9 7 2026)) '((1 15 2025)))
    ;; RFC 5545's twentieth Monday of each year.
    (remacs-test-ical-yearly-hits
     "FREQ=YEARLY;BYDAY=20MO;COUNT=3" '(5 19 1997)
     '((5 12 1997) (5 19 1997) (5 18 1998) (5 17 1999) (5 15 2000))
     '((5 19 1997) (5 18 1998) (5 17 1999)))
    (remacs-test-ical-yearly-hits
     "FREQ=YEARLY;BYDAY=-1FR;COUNT=3" '(1 15 2025)
     '((1 15 2025) (1 31 2025) (12 19 2025) (12 26 2025) (12 31 2025) (12 25 2026) (12 31 2027))
     '((1 15 2025) (12 26 2025) (12 25 2026)))
    (remacs-test-ical-yearly-hits
     "FREQ=YEARLY;BYDAY=53MO;COUNT=3" '(1 1 2024)
     '((1 1 2024) (12 30 2024) (12 29 2025) (12 31 2029) (12 31 2035))
     '((1 1 2024) (12 30 2024) (12 31 2029)))
    (remacs-test-ical-yearly-hits
     "FREQ=YEARLY;INTERVAL=2;BYDAY=MO,WE" '(1 15 2025)
     '((1 15 2025) (1 20 2025) (1 5 2026) (1 4 2027) (1 5 2027))
     '((1 15 2025) (1 20 2025) (1 4 2027)))
    ;; A February29 DTSTART must not restrict the expanded rule to leap years.
    (remacs-test-ical-yearly-hits
     "FREQ=YEARLY;BYDAY=1MO" '(2 29 2024)
     '((1 1 2024) (2 29 2024) (1 6 2025) (1 5 2026) (1 3 2028))
     '((2 29 2024) (1 6 2025) (1 5 2026) (1 3 2028)))
    (remacs-test-ical-yearly-hits
     "FREQ=YEARLY;BYMONTH=3;BYDAY=TH;COUNT=10" '(3 13 1997)
     '((3 6 1997) (3 13 1997) (3 20 1997) (3 27 1997) (4 3 1997)
       (3 5 1998) (3 12 1998) (3 19 1998) (3 26 1998)
       (3 4 1999) (3 11 1999) (3 18 1999) (3 25 1999))
     '((3 13 1997) (3 20 1997) (3 27 1997) (3 5 1998) (3 12 1998) (3 19 1998)
       (3 26 1998) (3 4 1999) (3 11 1999) (3 18 1999)))
    ;; BYMONTHDAY limits BYDAY; a year ordinal stays year-scoped without BYMONTH.
    (remacs-test-ical-yearly-hits
     "FREQ=YEARLY;BYMONTHDAY=1,15;BYDAY=MO;COUNT=3" '(6 10 2025)
     '((6 10 2025) (6 15 2025) (9 1 2025) (9 15 2025) (12 1 2025))
     '((6 10 2025) (9 1 2025) (9 15 2025)))
    (remacs-test-ical-yearly-hits
     "FREQ=YEARLY;BYMONTHDAY=1,15;BYDAY=1MO;COUNT=3" '(6 10 2025)
     '((6 10 2025) (9 1 2025) (1 5 2026) (1 1 2029) (1 1 2035) (1 1 2046))
     '((6 10 2025) (1 1 2029) (1 1 2035)))
    (remacs-test-ical-yearly-hits
     "FREQ=YEARLY;BYMONTH=9;BYDAY=1MO;COUNT=3" '(1 15 2025)
     '((1 15 2025) (9 1 2025) (9 7 2026) (9 6 2027)) '((9 1 2025)) nil nil
     '((EXDATE nil "20250115") (EXDATE nil "20260907")))
    ;; Independently enumerated endpoints and the following eligible occurrences.
    (remacs-test-ical-yearly-hits
     "FREQ=YEARLY;BYDAY=MO,WE;COUNT=1000000" '(1 6 2025)
     '((1 6 2025) (9 5 11607) (9 10 11607)) '((1 6 2025) (9 5 11607)))
    (remacs-test-ical-yearly-hits
     "FREQ=YEARLY;INTERVAL=2;BYDAY=MO,WE;COUNT=1000000" '(1 6 2025)
     '((1 6 2025) (12 7 21205) (12 12 21205)) '((1 6 2025) (12 7 21205)))
    (remacs-test-ical-yearly-hits
     "FREQ=YEARLY;BYDAY=20MO;COUNT=1000000" '(1 15 2025)
     '((1 15 2025) (5 15 1002023) (5 13 1002024)) '((1 15 2025) (5 15 1002023)))
    (let* ((start '(0 0 9 15 1 2025 nil -1 nil))
           (event '(VEVENT nil ((RRULE nil "FREQ=YEARLY;BYMONTH=9,1,9;BYDAY=-1FR,1MO;COUNT=4"))))
           (before (copy-tree (list event start))))
      (icalendar--convert-recurring-to-diary event start nil nil)
      (unless (equal (list event start) before) (error "Yearly weekday conversion mutated its inputs")))
    (dolist (rule '("FREQ=YEARLY;BYDAY=54MO" "FREQ=YEARLY;BYDAY=0MO" "FREQ=YEARLY;BYDAY=NO"
                     "FREQ=YEARLY;BYDAY=MO;COUNT=0" "FREQ=YEARLY;BYDAY=MO;COUNT=1.5"
                     "FREQ=YEARLY;BYDAY=MO;COUNT=2;INTERVAL=0"))
      (unless (condition-case nil
                  (progn (remacs-test-ical-yearly-format rule '(1 15 2025)) nil)
                (error t))
        (error "An invalid yearly weekday/count/interval was accepted: %S" rule))))
  ;; February 29 instances disappear in common years; they never shift (#1580).
  (let (candidates)
    (dotimes (i 17)
      (let ((year (+ 2020 i)))
        (push (list 2 28 year) candidates)
        (when (calendar-leap-year-p year) (push (list 2 29 year) candidates))
        (push (list 3 1 year) candidates)
        (push (list 3 2 year) candidates)))
    (setq candidates (nreverse candidates))
    (dolist (calendar-date-style '(american european iso))
      (dolist (times '((nil nil) ("09:00" "10:00")))
        (remacs-test-ical-yearly-hits
         "FREQ=YEARLY" '(2 29 2024) candidates
         '((2 29 2024) (2 29 2028) (2 29 2032) (2 29 2036)) (car times) (cadr times))
        (remacs-test-ical-yearly-hits
         (concat "FREQ=YEARLY;UNTIL=" (if (car times) "20280229T235959" "20280229"))
         '(2 29 2024) candidates '((2 29 2024) (2 29 2028)) (car times) (cadr times))
        (remacs-test-ical-yearly-hits
         "FREQ=YEARLY;COUNT=2" '(2 29 2024) candidates
         '((2 29 2024) (2 29 2028)) (car times) (cadr times))))
    (let ((calendar-date-style 'american))
      (remacs-test-ical-yearly-hits
       "FREQ=YEARLY;COUNT=1" '(2 29 2024) candidates '((2 29 2024)))
      (remacs-test-ical-yearly-hits
       "FREQ=YEARLY;INTERVAL=2" '(2 29 2024) candidates
       '((2 29 2024) (2 29 2028) (2 29 2032) (2 29 2036)))
      (remacs-test-ical-yearly-hits
       "FREQ=YEARLY;INTERVAL=3;COUNT=2" '(2 29 2024) candidates
       '((2 29 2024) (2 29 2036)))
      (remacs-test-ical-yearly-hits
       "FREQ=YEARLY;UNTIL=20280228" '(2 29 2024) candidates '((2 29 2024)))
      (remacs-test-ical-yearly-hits
       "FREQ=YEARLY;COUNT=3" '(2 29 2024) candidates '((2 29 2032)) nil nil
       '((EXDATE nil "20240229") (EXDATE nil "20280229")))
      (let* ((calendar-debug-sexp t)
             (generated (remacs-test-ical-yearly-format "FREQ=YEARLY" '(2 29 2024)))
             (entry (diary-sexp-entry (cdr generated) "anniversary %d%s" '(2 29 2028))))
        (unless (equal (if (consp entry) (cdr entry) entry) "anniversary 5th")
          (error "The leap-year guard lost the formatted anniversary entry")))))
  (let ((calendar-date-style 'iso))
    (remacs-test-ical-yearly-hits
     "FREQ=YEARLY;COUNT=3" '(2 29 2096)
     '((2 29 2096) (3 1 2100) (2 29 2104) (2 29 2108) (2 29 2112))
     '((2 29 2096) (2 29 2104) (2 29 2108)))
    (remacs-test-ical-yearly-hits
     "FREQ=YEARLY;COUNT=2;INTERVAL=100" '(2 29 2000)
     '((2 29 2000) (3 1 2100) (3 1 2200) (3 1 2300) (2 29 2400) (2 29 2800))
     '((2 29 2000) (2 29 2400)))
    (remacs-test-ical-yearly-hits
     "FREQ=YEARLY" '(2 29 2396)
     '((2 29 2396) (3 1 2397) (3 1 2398) (3 1 2399) (2 29 2400) (2 29 2404))
     '((2 29 2396) (2 29 2400) (2 29 2404)))
    (remacs-test-ical-yearly-hits
     "FREQ=YEARLY;COUNT=3" '(2 28 2024)
     '((2 28 2024) (2 29 2024) (2 28 2025) (3 1 2025) (2 28 2026) (2 28 2027))
     '((2 28 2024) (2 28 2025) (2 28 2026)))
    (remacs-test-ical-yearly-hits
     "FREQ=YEARLY" '(3 1 2024)
     '((2 29 2024) (3 1 2024) (3 1 2025) (3 1 2026) (3 1 2027) (2 29 2028) (3 1 2028))
     '((3 1 2024) (3 1 2025) (3 1 2026) (3 1 2027) (3 1 2028))))
  ;; BYMONTH expands a year into selected months while retaining DTSTART (#1564).
  (let (candidates)
    (dotimes (i 6)
      (let ((year (+ 2024 i)))
        (dotimes (m 12)
          (dolist (day '(9 10 11))
            (push (list (1+ m) day year) candidates)))))
    (setq candidates (nreverse candidates))
    (dolist (calendar-date-style '(american european iso))
      (dolist (times '((nil nil) ("09:00" "10:00")))
        (remacs-test-ical-yearly-hits
         "FREQ=YEARLY;BYMONTH=1" '(6 10 2025) candidates
         '((6 10 2025) (1 10 2026) (1 10 2027) (1 10 2028) (1 10 2029))
         (car times) (cadr times))
        (remacs-test-ical-yearly-hits
         (concat "FREQ=YEARLY;INTERVAL=2;BYMONTH=7,1,7;UNTIL="
                 (if (car times) "20270710T235959" "20270710"))
         '(6 10 2025) candidates
         '((6 10 2025) (7 10 2025) (1 10 2027) (7 10 2027)) (car times) (cadr times))
        (remacs-test-ical-yearly-hits
         "FREQ=YEARLY;BYMONTH=1,7;COUNT=3" '(6 10 2025) candidates
         '((6 10 2025) (7 10 2025) (1 10 2026)) (car times) (cadr times))))
    (let ((calendar-date-style 'american))
      (dolist (case '((1 ((6 10 2025)))
                      (2 ((6 10 2025) (7 10 2025)))
                      (4 ((6 10 2025) (7 10 2025) (1 10 2027) (7 10 2027)))))
        (remacs-test-ical-yearly-hits
         (format "FREQ=YEARLY;BYMONTH=1,7;INTERVAL=2;COUNT=%d" (car case))
         '(6 10 2025) candidates (cadr case)))
      (remacs-test-ical-yearly-hits
       "FREQ=YEARLY;BYMONTH=6,7;COUNT=4" '(6 10 2025) candidates
       '((6 10 2025) (7 10 2025) (6 10 2026) (7 10 2026)))
      (remacs-test-ical-yearly-hits
       "FREQ=YEARLY;BYMONTH=1;COUNT=2" '(6 10 2025) candidates
       '((6 10 2025) (1 10 2026)))
      (remacs-test-ical-yearly-hits
       "FREQ=YEARLY;BYMONTH=1,7;UNTIL=20260709" '(6 10 2025) candidates
       '((6 10 2025) (7 10 2025) (1 10 2026)))
      (remacs-test-ical-yearly-hits
       "FREQ=YEARLY;BYMONTH=7,1;COUNT=4" '(6 10 2025) candidates
       '((7 10 2025) (1 10 2026)) nil nil '((EXDATE nil "20250610") (EXDATE nil "20260710")))))
  (let ((calendar-date-style 'iso))
    ;; RFC 5545 section 3.8.5.3: ten June/July occurrences, not ten years.
    (remacs-test-ical-yearly-hits
     "FREQ=YEARLY;COUNT=10;BYMONTH=6,7" '(6 10 1997)
     '((6 10 1997) (7 10 1997) (6 10 1998) (7 10 1998) (6 10 1999) (7 10 1999)
       (6 10 2000) (7 10 2000) (6 10 2001) (7 10 2001) (6 10 2002) (7 10 2002))
     '((6 10 1997) (7 10 1997) (6 10 1998) (7 10 1998) (6 10 1999) (7 10 1999)
       (6 10 2000) (7 10 2000) (6 10 2001) (7 10 2001)))
    (remacs-test-ical-yearly-hits
     "FREQ=YEARLY;INTERVAL=2;COUNT=10;BYMONTH=1,2,3" '(3 10 1997)
     '((1 10 1997) (2 10 1997) (3 10 1997) (1 10 1998)
       (1 10 1999) (2 10 1999) (3 10 1999) (1 10 2001) (2 10 2001) (3 10 2001)
       (1 10 2003) (2 10 2003) (3 10 2003) (1 10 2005))
     '((3 10 1997) (1 10 1999) (2 10 1999) (3 10 1999) (1 10 2001) (2 10 2001)
       (3 10 2001) (1 10 2003) (2 10 2003) (3 10 2003)))
    (remacs-test-ical-yearly-hits
     "FREQ=YEARLY;BYMONTH=2,3;COUNT=3" '(1 31 2025)
     '((1 31 2025) (2 28 2025) (3 1 2025) (3 31 2025) (1 31 2026) (2 28 2026)
       (3 31 2026) (3 31 2027))
     '((1 31 2025) (3 31 2025) (3 31 2026)))
    (remacs-test-ical-yearly-hits
     "FREQ=YEARLY;BYMONTH=2;COUNT=20" '(1 31 2025)
     '((1 31 2025) (2 28 2025) (3 3 2025) (1 31 2026) (2 28 2026)) '((1 31 2025)))
    (remacs-test-ical-yearly-hits
     "FREQ=YEARLY;BYMONTH=2;COUNT=3;INTERVAL=100" '(2 29 2000)
     '((2 29 2000) (3 1 2100) (3 1 2200) (3 1 2300)
       (2 29 2400) (2 29 2800) (2 29 3200))
     '((2 29 2000) (2 29 2400) (2 29 2800)))
    (remacs-test-ical-yearly-hits
     "FREQ=YEARLY;BYMONTH=2;COUNT=3;INTERVAL=400" '(2 29 2000)
     '((2 29 2000) (2 29 2400) (2 29 2800) (2 29 3200))
     '((2 29 2000) (2 29 2400) (2 29 2800)))
    (remacs-test-ical-yearly-hits
     "FREQ=YEARLY;BYMONTH=2;COUNT=100" '(2 29 2096)
     '((2 29 2096) (3 1 2100) (2 29 2104) (2 29 2496) (3 1 2500)
       (2 29 2504) (2 29 2508) (2 29 2512))
     '((2 29 2096) (2 29 2104) (2 29 2496) (2 29 2504) (2 29 2508)))
    ;; A million instances should use cycle arithmetic and support large years.
    (remacs-test-ical-yearly-hits
     "FREQ=YEARLY;BYMONTH=1,7;COUNT=1000000" '(1 10 2025)
     '((1 10 2025) (7 10 2025) (1 10 502024) (7 10 502024) (1 10 502025))
     '((1 10 2025) (7 10 2025) (1 10 502024) (7 10 502024)))
    (let* ((start '(0 0 9 10 6 2025 nil -1 nil))
           (event '(VEVENT nil ((RRULE nil "FREQ=YEARLY;BYMONTH=7,1,7;COUNT=3"))))
           (before (copy-tree (list event start))))
      (icalendar--convert-recurring-to-diary event start nil nil)
      (unless (equal (list event start) before) (error "BYMONTH conversion mutated its inputs")))
    (dolist (rule '("FREQ=YEARLY;BYMONTH=0" "FREQ=YEARLY;BYMONTH=13"
                     "FREQ=YEARLY;BYMONTH=1,NO"
                     "FREQ=YEARLY;BYMONTH=1;COUNT=0" "FREQ=YEARLY;BYMONTH=1;COUNT=-1"
                     "FREQ=YEARLY;BYMONTH=1;COUNT=1.5"
                     "FREQ=YEARLY;BYMONTH=1;COUNT=2;INTERVAL=0"))
      (unless (condition-case nil
                  (progn (remacs-test-ical-yearly-format rule '(6 10 2025)) nil)
                (error t))
        (error "An invalid yearly month/count/interval was accepted: %S" rule))))
  (dolist (calendar-date-style '(american european iso))
    (dolist (suffix '("" ";UNTIL=20300106"))
      (unless (equal (car (remacs-test-ical-yearly-format (concat "FREQ=YEARLY" suffix) '(1 6 2025)))
                     (car (remacs-test-ical-yearly-format (concat "FREQ=YEARLY;INTERVAL=1" suffix) '(1 6 2025))))
        (error "Explicit yearly INTERVAL=1 changed the diary format"))))
  (let ((calendar-date-style 'iso))
    (remacs-test-ical-yearly-hits
     "FREQ=YEARLY;INTERVAL=4" '(2 29 2024)
     '((2 28 2024) (2 29 2024) (3 1 2024) (3 1 2025) (3 1 2026) (3 1 2027)
       (2 28 2028) (2 29 2028) (3 1 2028) (3 1 2029) (2 29 2032))
     '((2 29 2024) (2 29 2028) (2 29 2032)))
    (remacs-test-ical-yearly-hits
     "FREQ=YEARLY;INTERVAL=2;UNTIL=20280229" '(2 29 2024)
     '((2 29 2024) (3 1 2024) (3 1 2025) (2 28 2026) (3 1 2026) (2 29 2028) (2 29 2032))
     '((2 29 2024) (2 29 2028)))
    (remacs-test-ical-yearly-hits
     "FREQ=YEARLY;INTERVAL=2" '(12 31 2024)
     '((12 31 2022) (1 1 2024) (12 31 2024) (1 1 2025) (12 31 2025)
       (12 31 2026) (1 1 2027) (12 31 2027) (12 31 2028))
     '((12 31 2024) (12 31 2026) (12 31 2028)))
    (let* ((start '(0 0 9 6 1 2025 nil -1 nil))
           (event '(VEVENT nil ((RRULE nil "FREQ=YEARLY;INTERVAL=2"))))
           (before (copy-tree (list event start))))
      (icalendar--convert-recurring-to-diary event start nil nil)
      (unless (equal (list event start) before) (error "Yearly conversion mutated its inputs"))))
  t)
"##;

fn check(interp: &mut Interp, source: &str) {
    match interp.eval_str(source) {
        Ok(value) => assert!(value.truthy()),
        Err(Flow::Signal(signal)) => {
            let symbol = interp.intern("remacs-test-ical-yearly-phase");
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
fn yearly_recurrences_preserve_selectors_intervals_and_counts() {
    let mut interp = Interp::new();
    check(&mut interp, CASES);
    check(&mut interp, r#"(load "icalendar.el" nil t)"#);
    check(&mut interp, CASES);
}
