use remacs_runtime::lisp::{Flow, Interp};
use std::time::Instant;

const SETUP: &str = r#"(progn (require 'gnus-icalendar) t)"#;

const CASES: &str = r##"
(progn
  (defvar remacs-test-icalendar-action nil)
  (defun remacs-test-icalendar-callback (data)
    (setq remacs-test-icalendar-action data))
  (with-temp-buffer
    (dolist (label '("Accept" "Export to Org"))
      (let* ((start (point))
             (data (list 'action label)))
        (gnus-icalendar-insert-button label #'remacs-test-icalendar-callback data)
        (let ((map (get-text-property start 'keymap)))
          (unless (and (eq map gnus-icalendar-button-map)
                       (not (eq map gnus-mime-button-map)))
            (error "An iCalendar action button used the MIME part map"))
          (dolist (entry gnus-mime-button-commands)
            (unless (eq (car entry) 'gnus-article-press-button)
              (when (lookup-key map (cadr entry))
                (error "An iCalendar button inherited MIME shortcut %S" (cadr entry)))))
          (when (eq (lookup-key map [down-mouse-3]) 'gnus-mime-button-menu)
            (error "An iCalendar button inherited the MIME part menu"))
          (unless (and (eq (lookup-key map "\r") 'gnus-article-press-button)
                       (eq (lookup-key map [mouse-2]) 'gnus-article-push-button))
            (error "iCalendar activation bindings are missing"))
          (goto-char (+ start 2))
          (setq remacs-test-icalendar-action nil)
          (funcall (lookup-key map "\r") nil)
          (unless (eq remacs-test-icalendar-action data)
            (error "Return did not invoke the iCalendar callback with its data"))
          (let* ((window (selected-window))
                 (previous (window-buffer window))
                 (position (point))
                 (button-buffer (current-buffer)))
            (unwind-protect
                (progn
                  (set-window-buffer window (current-buffer))
                  (setq remacs-test-icalendar-action nil)
                  (funcall (lookup-key map [mouse-2])
                           (list 'mouse-2 (list window position '(0 . 0) 0)))
                  (unless (eq remacs-test-icalendar-action data)
                    (error "Mouse activation did not invoke the iCalendar callback")))
              (set-window-buffer window previous)
              (set-buffer button-buffer))))
        (goto-char (point-max))
        (insert "\n"))))
  ;; Real MIME buttons retain their own commands.
  (dolist (binding '(("o" . gnus-mime-save-part)
                     ("d" . gnus-mime-delete-part)
                     ("r" . gnus-mime-replace-part)))
    (unless (eq (lookup-key gnus-mime-button-map (car binding)) (cdr binding))
      (error "A real MIME shortcut changed: %S" binding)))
  t)
"##;

fn check(interp: &mut Interp, label: &str, source: &str) {
    let started = Instant::now();
    println!("{label}: starting");
    match interp.eval_str(source) {
        Ok(value) => assert!(value.truthy()),
        Err(Flow::Signal(signal)) => panic!(
            "{} {}",
            interp.prin1_to_string(&signal.symbol()),
            interp.prin1_to_string(&signal.data())
        ),
        Err(other) => panic!("{other:?}"),
    }
    println!("{label}: passed in {:?}", started.elapsed());
}

#[test]
fn calendar_action_buttons_have_only_their_own_activation_keys() {
    let mut interp = Interp::new();
    check(&mut interp, "Actual Gnus iCalendar load", SETUP);
    check(&mut interp, "Button keys and callbacks", CASES);
}
