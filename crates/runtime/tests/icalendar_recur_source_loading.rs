use remacs_runtime::lisp::{Flow, Interp};

const CASES: &str = r##"
;;; -*- lexical-binding: t; -*-
(progn
  (with-temp-buffer
    (insert-file-contents (locate-library "icalendar-recur.el"))
    (emacs-lisp-mode)
    (goto-char (point-min))
    (let (definitions)
      (while (progn (forward-comment (point-max)) (not (eobp)))
        (let ((form (read (current-buffer))))
          (when (and (eq (car-safe form) 'defun)
                     (memq (car-safe (cdr-safe form))
                           '(icalendar-recur-recurrences-in-window-w/end-times
                             icalendar-recur-recurrences-to-count)))
            (push (car-safe (cdr-safe form)) definitions))
          (when (equal form '(provide 'icalendar-recur))
            (push 'provide definitions))))
      (unless (equal (nreverse definitions)
                     '(icalendar-recur-recurrences-in-window-w/end-times
                       icalendar-recur-recurrences-to-count provide))
        (error "The window/end-time form swallowed later top-level forms"))))
  (require 'icalendar-recur)
  (unless (and (featurep 'icalendar-recur)
               (fboundp 'icalendar-recur-recurrences-in-window-w/end-times)
               (fboundp 'icalendar-recur-recurrences-to-count))
    (error "The recurrence source did not provide its trailing definitions"))
  (let* ((component
          (icalendar-make-ast-node
           'icalendar-vevent nil
           (list (icalendar-make-property icalendar-uid "source-load@example.test")
                 (icalendar-make-property icalendar-dtstamp (icalendar-read-date-time "20250101T000000Z"))
                 (icalendar-make-property icalendar-dtstart (icalendar-read-date "20250101"))
                 (icalendar-make-property icalendar-duration (icalendar-read-dur-value "P1D"))
                 (icalendar-make-property icalendar-rrule '((FREQ DAILY) (COUNT 2))))))
         (pairs (icalendar-recur-recurrences-in-window-w/end-times
                 (icalendar-read-date "20250101") (icalendar-read-date "20250104") component)))
    (unless (equal pairs '(((1 1 2025) (1 2 2025)) ((1 2 2025) (1 3 2025))))
      (error "Wrong actual recurrence start/end pairs: %S" pairs)))
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
fn recurrence_source_load_preserves_top_level_forms_and_window_end_bindings() {
    let mut interp = Interp::new();
    check(&mut interp, CASES);
    check(&mut interp, r#"(load "icalendar-recur.el" nil t)"#);
    check(&mut interp, CASES);
}
