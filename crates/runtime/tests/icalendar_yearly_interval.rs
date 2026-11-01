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
        (when (diary-sexp-entry (cdr generated) "event" day) (push day hits)))
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
fn yearly_recurrences_preserve_months_intervals_and_counts() {
    let mut interp = Interp::new();
    check(&mut interp, CASES);
    check(&mut interp, r#"(load "icalendar.el" nil t)"#);
    check(&mut interp, CASES);
}
