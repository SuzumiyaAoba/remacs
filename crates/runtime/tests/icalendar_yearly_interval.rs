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
fn yearly_recurrences_respect_start_year_and_interval() {
    let mut interp = Interp::new();
    check(&mut interp, CASES);
    check(&mut interp, r#"(load "icalendar.el" nil t)"#);
    check(&mut interp, CASES);
}
