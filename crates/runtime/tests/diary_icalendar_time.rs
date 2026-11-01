use remacs_runtime::lisp::{Flow, Interp};

const CASES: &str = r##"
;;; -*- lexical-binding: t; -*-
(progn
  (require 'diary-icalendar)
  (defvar remacs-test-diary-time-phase nil)
  (defun remacs-test-diary-time-triple (value)
    (when value (list (decoded-time-hour value) (decoded-time-minute value) (decoded-time-second value))))
  (defun remacs-test-diary-time (text expected-start expected-end &optional linewise)
    (setq remacs-test-diary-time-phase (list text linewise))
    (with-temp-buffer
      (insert (concat text " event"))
      (let* ((diary-icalendar-export-linewise linewise)
             (original (buffer-string))
             (times (diary-icalendar-parse-time)))
        (unless (and (equal (remacs-test-diary-time-triple (car times)) expected-start)
                     (equal (remacs-test-diary-time-triple (cadr times)) expected-end))
          (error "Wrong diary time: %S from %S" times text))
        (unless (equal (buffer-string) "event")
          (error "Diary time parser consumed the wrong restriction: %S" (buffer-string)))
        (unless (equal original (save-restriction (widen) (buffer-string)))
          (error "Diary time parser changed buffer contents")))))
  (defun remacs-test-diary-time-reject (text &optional linewise)
    (setq remacs-test-diary-time-phase (list text linewise))
    (with-temp-buffer
      (insert (concat text " event"))
      (goto-char (point-min))
      (let ((diary-icalendar-export-linewise linewise)
            (original (buffer-string)) (begin (point-min)) (end (point-max)))
        (unless (null (diary-icalendar-parse-time)) (error "Accepted invalid diary time: %S" text))
        (unless (and (= (point-min) begin) (= (point-max) end) (= (point) begin)
                     (equal original (buffer-string)))
          (error "Invalid time consumed input: %S" text)))))
  ;; Reject complete invalid hours, including invalid end fields (#1586).
  (dolist (text '("25:00" "24:00" "29:59" "30:00" "99:00" "100:00" "123:45" "001:00"
                   "0AM" "00:30am" "13PM" "23:00AM" "25h" "25h00" "25.00h" "25.00am"
                   "09:00-25:00" "24:00-09:00" "12AM-13PM" "13AM-12PM"
                   "11PM-00AM" "23:59-24:00"))
    (remacs-test-diary-time-reject text)
    (remacs-test-diary-time-reject (concat "  " text) t))
  (remacs-test-diary-time "00:00-23:59" '(0 0 0) '(23 59 0))
  (remacs-test-diary-time "01AM-11PM" '(1 0 0) '(23 0 0))
  (with-temp-buffer
    (insert "prefix09:00-25:00 eventTAIL")
    (narrow-to-region 7 (- (point-max) 4))
    (goto-char (point-min))
    (let ((diary-icalendar-export-linewise nil)
          (before (buffer-string)) (begin (point-min)) (end (point-max)))
      (unless (and (null (diary-icalendar-parse-time))
                   (= (point-min) begin) (= (point-max) end) (= (point) begin)
                   (equal (buffer-string) before))
        (error "Invalid end time changed an existing restriction"))))
  ;; Midnight/noon conversion must work for both start and end fields (#1582).
  (dolist (case '(("12:30AM" (0 30 0) nil)
                  ("12:30PM" (12 30 0) nil)
                  ("12AM" (0 0 0) nil) ("12PM" (12 0 0) nil)
                  ("01am" (1 0 0) nil) ("11pm" (23 0 0) nil)
                  ("1:05Am" (1 5 0) nil) ("00:00" (0 0 0) nil)
                  ("12:00" (12 0 0) nil) ("23:59" (23 59 0) nil)
                  ("12.30am" (0 30 0) nil) ("9.00pm" (21 0 0) nil) ("12h30AM" (0 30 0) nil)
                  ("12.30h" (12 30 0) nil) ("12h" (12 0 0) nil)
                  ("0h" (0 0 0) nil)
                  ("11PM-12AM" (23 0 0) (0 0 0))
                  ("12AM-1AM" (0 0 0) (1 0 0))
                  ("12PM-1PM" (12 0 0) (13 0 0))
                  ("11:15PM-12:45AM" (23 15 0) (0 45 0))
                  ("12am--12pm" (0 0 0) (12 0 0))
                  ("23:45-00:15" (23 45 0) (0 15 0))
                  ("12h30AM-12.45am" (0 30 0) (0 45 0))))
    (remacs-test-diary-time (car case) (cadr case) (nth 2 case)))
  (remacs-test-diary-time "  12:30AM-12:45PM" '(0 30 0) '(12 45 0) t)
  (with-temp-buffer
    (insert "prefix12AM-12PM eventTAIL")
    (narrow-to-region 7 (- (point-max) 4))
    (let* ((diary-icalendar-export-linewise nil)
           (times (diary-icalendar-parse-time)))
      (unless (and (equal (remacs-test-diary-time-triple (car times)) '(0 0 0))
                   (equal (remacs-test-diary-time-triple (cadr times)) '(12 0 0))
                   (equal (buffer-string) "event"))
        (error "Midnight parsing did not respect the existing restriction"))))
  (with-temp-buffer
    (insert "no time here")
    (let ((diary-icalendar-export-linewise nil))
      (unless (and (null (diary-icalendar-parse-time)) (equal (buffer-string) "no time here"))
        (error "A missing time changed the restriction"))))
  t)
"##;

fn check(interp: &mut Interp, source: &str) {
    match interp.eval_str(source) {
        Ok(value) => assert!(value.truthy()),
        Err(Flow::Signal(signal)) => {
            let symbol = interp.intern("remacs-test-diary-time-phase");
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
fn diary_time_parser_validates_clock_fields() {
    let mut interp = Interp::new();
    check(&mut interp, CASES);
    check(&mut interp, r#"(load "diary-icalendar.el" nil t)"#);
    check(&mut interp, CASES);
}
