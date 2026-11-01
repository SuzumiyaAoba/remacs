use remacs_runtime::lisp::{Flow, Interp};

const CASES: &str = r##"
;;; -*- lexical-binding: t; -*-
(progn
  (with-temp-buffer
    (insert-file-contents (locate-library "icalendar-parser.el"))
    (emacs-lisp-mode)
    (goto-char (point-min))
    (let (definitions)
      (while (progn (forward-comment (point-max)) (not (eobp)))
        (let ((form (read (current-buffer))))
          (when (and (eq (car-safe form) 'defun)
                     (memq (car-safe (cdr-safe form))
                           '(icalendar-read-dur-value icalendar-print-dur-value
                             icalendar-read-period icalendar-parse-and-index)))
            (push (car-safe (cdr-safe form)) definitions))
          (when (equal form '(provide 'icalendar-parser))
            (push 'provide definitions))))
      (unless (equal (nreverse definitions)
                     '(icalendar-read-dur-value icalendar-print-dur-value
                       icalendar-read-period icalendar-parse-and-index provide))
        (error "The duration reader swallowed later top-level forms"))))
  (require 'icalendar-parser)
  (unless (and (featurep 'icalendar-parser) (fboundp 'icalendar-read-dur-value)
               (fboundp 'icalendar-read-period) (fboundp 'icalendar-parse-and-index))
    (error "The parser did not finish loading all definitions"))
  (dolist (case '(("P2W" 2) ("-P3W" -3)))
    (unless (equal (icalendar-read-dur-value (car case)) (cadr case))
      (error "The week duration reader ignored its string: %S" case)))
  (dolist (case '(("P1DT2H3M4S" (1 2 3 4)) ("-PT45M" (0 0 -45 0)) ("PT0S" (0 0 0 0))))
    (string-match "a\\(m\\)b" "ambient")
    (let* ((before (match-data))
           (value (icalendar-read-dur-value (car case))))
      (unless (equal (list (decoded-time-day value) (decoded-time-hour value)
                           (decoded-time-minute value) (decoded-time-second value)) (cadr case))
        (error "The duration reader used ambient match data: %S" value))
      (unless (equal before (match-data)) (error "Duration parsing clobbered caller match data"))))
  t)
"##;

fn check(interp: &mut Interp, source: &str) {
    match interp.eval_str(source) {
        Ok(value) => assert!(value.truthy()),
        Err(Flow::Signal(signal)) => panic!(
            "{} {}",
            interp.prin1_to_string(&signal.symbol()),
            interp.prin1_to_string(&signal.data())
        ),
        Err(other) => panic!("{other:?}"),
    }
}

#[test]
fn parser_source_load_preserves_top_level_forms_and_duration_strings() {
    let mut interp = Interp::new();
    check(&mut interp, CASES);
    check(&mut interp, r#"(load "icalendar-parser.el" nil t)"#);
    check(&mut interp, CASES);
}
