use remacs_runtime::lisp::{Flow, Interp};

const CASES: &str = r##"
;;; -*- lexical-binding: t; -*-
(progn
  (require 'icalendar)
  (defvar remacs-test-ical-weekly-phase nil)
  (defun remacs-test-ical-weekly-format (rule start &optional start-time end-time extras)
    (setq remacs-test-ical-weekly-phase (list calendar-date-style rule start start-time end-time))
    (let* ((event (list 'VEVENT nil (append (list (list 'RRULE nil rule)) extras)))
           (decoded (list 0 0 (if start-time 9 0) (cadr start) (car start) (nth 2 start) nil -1 nil))
           (line (icalendar--convert-recurring-to-diary event decoded start-time end-time))
           (parsed (read-from-string line 2)))
      (unless (string-prefix-p "%%(" line) (error "No diary expression was generated: %S" line))
      (unless (equal (string-trim (substring line (cdr parsed)))
                     (concat (or start-time "") (if end-time "-" "") (or end-time "")))
        (error "Weekly conversion changed the displayed time: %S" line))
      (cons line (substring line 2 (cdr parsed)))))
  (defun remacs-test-ical-weekly-hits (rule start begin end expected &optional start-time end-time extras)
    (setq remacs-test-ical-weekly-phase (list calendar-date-style rule start start-time end-time))
    (let* ((calendar-debug-sexp t)
           (generated (remacs-test-ical-weekly-format rule start start-time end-time extras))
           (cursor (calendar-absolute-from-gregorian begin))
           (last (calendar-absolute-from-gregorian end))
           hits)
      (while (<= cursor last)
        (let ((day (calendar-gregorian-from-absolute cursor)))
          (when (diary-sexp-entry (cdr generated) "event" day) (push day hits)))
        (setq cursor (1+ cursor)))
      (setq hits (nreverse hits))
      (unless (equal hits expected)
        (error "Wrong weekly occurrences: %S generated %S" hits (car generated)))
      generated))
  (dolist (calendar-date-style '(american european iso))
    (dolist (times '((nil nil) ("09:00" "10:00")))
      (remacs-test-ical-weekly-hits
       "FREQ=WEEKLY;INTERVAL=2;BYDAY=MO,WE" '(1 6 2025) '(1 1 2025) '(2 5 2025)
       '((1 6 2025) (1 8 2025) (1 20 2025) (1 22 2025) (2 3 2025) (2 5 2025))
       (car times) (cadr times))
      (remacs-test-ical-weekly-hits
       (concat "FREQ=WEEKLY;INTERVAL=2;BYDAY=MO,WE;UNTIL="
               (if (car times) "20250122T235959" "20250122"))
       '(1 6 2025) '(1 1 2025) '(2 5 2025)
       '((1 6 2025) (1 8 2025) (1 20 2025) (1 22 2025))
       (car times) (cadr times))))
  (let ((calendar-date-style 'american))
    (remacs-test-ical-weekly-hits
     "FREQ=WEEKLY;INTERVAL=1;BYDAY=MO,WE" '(1 6 2025) '(1 1 2025) '(2 5 2025)
     '((1 6 2025) (1 8 2025) (1 13 2025) (1 15 2025) (1 20 2025)
       (1 22 2025) (1 27 2025) (1 29 2025) (2 3 2025) (2 5 2025)))
    (remacs-test-ical-weekly-hits
     "FREQ=WEEKLY;INTERVAL=3;BYDAY=WE,MO,WE" '(1 6 2025) '(1 1 2025) '(2 5 2025)
     '((1 6 2025) (1 8 2025) (1 27 2025) (1 29 2025)))
    ;; RFC 5545 section 3.8.5.3's TU/TH pattern, without a COUNT bound here.
    (remacs-test-ical-weekly-hits
     "FREQ=WEEKLY;INTERVAL=2;WKST=SU;BYDAY=TU,TH" '(9 2 1997) '(9 1 1997) '(10 31 1997)
     '((9 2 1997) (9 4 1997) (9 16 1997) (9 18 1997) (9 30 1997)
       (10 2 1997) (10 14 1997) (10 16 1997) (10 28 1997) (10 30 1997)) "09:00" "10:00")
    ;; The RFC's WKST example: changing only the week start changes Sundays.
    (remacs-test-ical-weekly-hits
     "FREQ=WEEKLY;INTERVAL=2;BYDAY=TU,SU;WKST=MO" '(8 5 1997) '(8 1 1997) '(8 31 1997)
     '((8 5 1997) (8 10 1997) (8 19 1997) (8 24 1997)))
    (remacs-test-ical-weekly-hits
     "FREQ=WEEKLY;INTERVAL=2;BYDAY=TU,SU;WKST=SU" '(8 5 1997) '(8 1 1997) '(8 31 1997)
     '((8 5 1997) (8 17 1997) (8 19 1997) (8 31 1997)))
    (dolist (calendar-week-start-day '(0 6))
      (remacs-test-ical-weekly-hits
       "FREQ=WEEKLY;INTERVAL=2;BYDAY=TU,SU" '(8 5 1997) '(8 1 1997) '(8 31 1997)
       '((8 5 1997) (8 10 1997) (8 19 1997) (8 24 1997))))
    (remacs-test-ical-weekly-hits
     "FREQ=WEEKLY;INTERVAL=2;BYDAY=TU,TH" '(2 22 2024) '(2 19 2024) '(3 21 2024)
     '((2 22 2024) (3 5 2024) (3 7 2024) (3 19 2024) (3 21 2024)))
    (remacs-test-ical-weekly-hits
     "FREQ=WEEKLY;INTERVAL=2;BYDAY=MO,WE" '(12 30 2024) '(12 16 2024) '(1 31 2025)
     '((12 30 2024) (1 1 2025) (1 13 2025) (1 15 2025) (1 27 2025) (1 29 2025)))
    (remacs-test-ical-weekly-hits
     "FREQ=WEEKLY;INTERVAL=2;BYDAY=MO,WE" '(1 6 2025) '(1 1 2025) '(2 5 2025)
     '((1 8 2025) (1 22 2025) (2 3 2025) (2 5 2025)) nil nil
     '((EXDATE nil "20250106") (EXDATE nil "20250120")))
    (remacs-test-ical-weekly-hits
     "FREQ=WEEKLY;INTERVAL=2" '(1 6 2025) '(1 1 2025) '(2 5 2025)
     '((1 6 2025) (1 20 2025) (2 3 2025)))
    (let* ((calendar-debug-sexp t)
           (generated (remacs-test-ical-weekly-hits
                       "FREQ=WEEKLY;INTERVAL=2;BYDAY=MO" '(1 6 2025) '(1 1 2025) '(2 5 2025)
                       '((1 6 2025) (1 20 2025) (2 3 2025))))
           (entry (diary-sexp-entry (cdr generated) "repeat %d" '(1 20 2025))))
      (unless (equal (if (consp entry) (cdr entry) entry) "repeat 1")
        (error "Aligned single weekdays lost their diary-cyclic repetition index")))
    (remacs-test-ical-weekly-hits
     "FREQ=WEEKLY;INTERVAL=2;BYDAY=WE" '(1 6 2025) '(1 1 2025) '(2 5 2025)
     '((1 8 2025) (1 22 2025) (2 5 2025)) "09:00" nil))
  ;; A single BYDAY must be used even when it differs from DTSTART (#1561).
  (dolist (calendar-date-style '(american european iso))
    (dolist (times '((nil nil) ("09:00" "10:00")))
      (remacs-test-ical-weekly-hits
       "FREQ=WEEKLY;BYDAY=WE" '(1 6 2025) '(1 1 2025) '(1 31 2025)
       '((1 8 2025) (1 15 2025) (1 22 2025) (1 29 2025)) (car times) (cadr times))
      (remacs-test-ical-weekly-hits
       (concat "FREQ=WEEKLY;BYDAY=WE;UNTIL="
               (if (car times) "20250115T235959" "20250115"))
       '(1 6 2025) '(1 1 2025) '(1 31 2025)
       '((1 8 2025) (1 15 2025)) (car times) (cadr times))))
  (let ((calendar-date-style 'american))
    (remacs-test-ical-weekly-hits
     "FREQ=WEEKLY;BYDAY=SU" '(1 6 2025) '(1 1 2025) '(2 2 2025)
     '((1 12 2025) (1 19 2025) (1 26 2025) (2 2 2025)))
    (remacs-test-ical-weekly-hits
     "FREQ=WEEKLY;BYDAY=MO;INTERVAL=2" '(1 9 2025) '(1 1 2025) '(2 10 2025)
     '((1 20 2025) (2 3 2025)))
    (remacs-test-ical-weekly-hits
     "FREQ=WEEKLY;BYDAY=MO;INTERVAL=2;WKST=TU" '(1 9 2025) '(1 1 2025) '(2 10 2025)
     '((1 13 2025) (1 27 2025) (2 10 2025)) "09:00" "10:00")
    (remacs-test-ical-weekly-hits
     "FREQ=WEEKLY;BYDAY=TU;INTERVAL=2" '(1 5 2025) '(1 1 2025) '(2 11 2025)
     '((1 14 2025) (1 28 2025) (2 11 2025))))
  ;; COUNT bounds individual instances, including a partial initial week (#1563).
  (dolist (calendar-date-style '(american european iso))
    (dolist (times '((nil nil) ("09:00" "10:00")))
      (dolist (case '((1 ((1 6 2025)))
                      (2 ((1 6 2025) (1 8 2025)))
                      (3 ((1 6 2025) (1 8 2025) (1 13 2025)))))
        (remacs-test-ical-weekly-hits
         (format "FREQ=WEEKLY;COUNT=%d;BYDAY=MO,WE" (car case))
         '(1 6 2025) '(1 1 2025) '(2 5 2025) (cadr case) (car times) (cadr times)))
      (remacs-test-ical-weekly-hits
       "FREQ=WEEKLY;COUNT=3;INTERVAL=2;BYDAY=MO,WE" '(1 6 2025) '(1 1 2025) '(2 5 2025)
       '((1 6 2025) (1 8 2025) (1 20 2025)) (car times) (cadr times))
      (remacs-test-ical-weekly-hits
       "FREQ=WEEKLY;COUNT=3;INTERVAL=2" '(1 6 2025) '(1 1 2025) '(3 3 2025)
       '((1 6 2025) (1 20 2025) (2 3 2025)) (car times) (cadr times))))
  (let ((calendar-date-style 'american))
    ;; Original RFC COUNT examples, now including their exact stopping point.
    (remacs-test-ical-weekly-hits
     "FREQ=WEEKLY;INTERVAL=2;COUNT=8;WKST=SU;BYDAY=TU,TH" '(9 2 1997) '(9 1 1997) '(11 15 1997)
     '((9 2 1997) (9 4 1997) (9 16 1997) (9 18 1997) (9 30 1997)
       (10 2 1997) (10 14 1997) (10 16 1997)) "09:00" "10:00")
    (remacs-test-ical-weekly-hits
     "FREQ=WEEKLY;INTERVAL=2;COUNT=4;BYDAY=TU,SU;WKST=MO" '(8 5 1997) '(8 1 1997) '(9 30 1997)
     '((8 5 1997) (8 10 1997) (8 19 1997) (8 24 1997)))
    (remacs-test-ical-weekly-hits
     "FREQ=WEEKLY;INTERVAL=2;COUNT=4;BYDAY=TU,SU;WKST=SU" '(8 5 1997) '(8 1 1997) '(9 30 1997)
     '((8 5 1997) (8 17 1997) (8 19 1997) (8 31 1997)) "09:00" "10:00")
    (remacs-test-ical-weekly-hits
     "FREQ=WEEKLY;COUNT=3;INTERVAL=2;BYDAY=MO,WE" '(1 8 2025) '(1 1 2025) '(2 5 2025)
     '((1 8 2025) (1 20 2025) (1 22 2025)))
    (remacs-test-ical-weekly-hits
     "FREQ=WEEKLY;COUNT=3;INTERVAL=2;BYDAY=SU,TU" '(1 5 2025) '(1 1 2025) '(2 5 2025)
     '((1 5 2025) (1 14 2025) (1 19 2025)))
    (remacs-test-ical-weekly-hits
     "FREQ=WEEKLY;COUNT=3;INTERVAL=2;BYDAY=SU,TU;WKST=SU" '(1 5 2025) '(1 1 2025) '(2 5 2025)
     '((1 5 2025) (1 7 2025) (1 19 2025)))
    (remacs-test-ical-weekly-hits
     "FREQ=WEEKLY;COUNT=10;INTERVAL=2;BYDAY=SU,MO,TU,WE,TH,FR,SA" '(1 10 2025) '(1 1 2025) '(2 5 2025)
     '((1 10 2025) (1 11 2025) (1 12 2025) (1 20 2025) (1 21 2025)
       (1 22 2025) (1 23 2025) (1 24 2025) (1 25 2025) (1 26 2025)))
    (remacs-test-ical-weekly-hits
     "FREQ=WEEKLY;COUNT=3;BYDAY=WE,MO,WE,MO" '(1 6 2025) '(1 1 2025) '(2 5 2025)
     '((1 6 2025) (1 8 2025) (1 13 2025)))
    (remacs-test-ical-weekly-hits
     "FREQ=WEEKLY;COUNT=3;BYDAY=MO" '(1 6 2025) '(1 1 2025) '(2 5 2025)
     '((1 6 2025) (1 13 2025) (1 20 2025)) "09:00" "10:00")
    (remacs-test-ical-weekly-hits
     "FREQ=WEEKLY;COUNT=2;BYDAY=WE" '(1 6 2025) '(1 1 2025) '(2 5 2025)
     '((1 8 2025) (1 15 2025)) "09:00" nil)
    (remacs-test-ical-weekly-hits
     "FREQ=WEEKLY;COUNT=3;BYDAY=MO,WE" '(1 6 2025) '(1 1 2025) '(2 5 2025)
     '((1 6 2025) (1 13 2025)) nil nil '((EXDATE nil "20250108")))
    (remacs-test-ical-weekly-hits
     "FREQ=WEEKLY;COUNT=1;BYDAY=MO,WE" '(1 6 2025) '(1 1 2025) '(2 5 2025)
     nil nil nil '((EXDATE nil "20250106")))
    (remacs-test-ical-weekly-hits
     "FREQ=WEEKLY;COUNT=3;INTERVAL=2;BYDAY=MO,WE" '(12 30 2024) '(12 16 2024) '(2 5 2025)
     '((12 30 2024) (1 1 2025) (1 13 2025)))
    ;; Large counts must use week arithmetic, not enumerate every occurrence.
    (let* ((calendar-debug-sexp t)
           (generated (remacs-test-ical-weekly-format
                       "FREQ=WEEKLY;COUNT=1000000;INTERVAL=2;BYDAY=MO,WE" '(1 6 2025)))
           (last (+ (calendar-absolute-from-gregorian '(1 6 2025)) (* 499999 14) 2)))
      (unless (and (diary-sexp-entry (cdr generated) "event" (calendar-gregorian-from-absolute last))
                   (null (diary-sexp-entry (cdr generated) "event"
                                          (calendar-gregorian-from-absolute (+ last 12)))))
        (error "A large COUNT did not preserve its exact endpoint")))
    (let* ((start '(0 0 9 6 1 2025 nil -1 nil))
           (event '(VEVENT nil ((RRULE nil "FREQ=WEEKLY;COUNT=3;BYDAY=MO,WE"))))
           (start-before (copy-tree start)) (event-before (copy-tree event)))
      (icalendar--convert-recurring-to-diary event start nil nil)
      (unless (and (equal start start-before) (equal event event-before))
        (error "COUNT calculation mutated its input event/start")))
    (dolist (rule '("FREQ=WEEKLY;COUNT=0;BYDAY=MO,WE"
                     "FREQ=WEEKLY;COUNT=-1;BYDAY=MO,WE"
                     "FREQ=WEEKLY;COUNT=1.5;BYDAY=MO,WE"
                     "FREQ=WEEKLY;COUNT=3;INTERVAL=0;BYDAY=MO,WE"))
      (unless (condition-case nil
                  (progn (remacs-test-ical-weekly-format rule '(1 6 2025)) nil)
                (error t))
        (error "An invalid weekly count/interval was accepted: %S" rule))))
  t)
"##;

fn check(interp: &mut Interp, source: &str) {
    match interp.eval_str(source) {
        Ok(value) => assert!(value.truthy()),
        Err(Flow::Signal(signal)) => {
            let symbol = interp.intern("remacs-test-ical-weekly-phase");
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
fn weekly_recurrences_preserve_days_intervals_and_counts() {
    let mut interp = Interp::new();
    eprintln!("default iCalendar weekly interval");
    check(&mut interp, CASES);
    check(&mut interp, r#"(load "icalendar.el" nil t)"#);
    eprintln!("source iCalendar weekly interval");
    check(&mut interp, CASES);
}
