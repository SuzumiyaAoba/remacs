use remacs_runtime::lisp::{Flow, Interp};
use std::time::Instant;

const SETUP: &str = r##"
(progn
  (with-temp-buffer
    (insert-file-contents (locate-library "org-capture.el"))
    (emacs-lisp-mode)
    (check-parens)
    (goto-char (point-min))
    (let (definitions)
      (while (progn (forward-comment (point-max)) (not (eobp)))
        (let ((form (read (current-buffer))))
          (when (and (eq (car-safe form) 'defun)
                     (memq (car-safe (cdr-safe form))
                           '(org-capture--list-item-empty-lines-before
                             org-capture-place-item org-capture-place-table-line
                             org-capture-fill-template)))
            (push (car-safe (cdr-safe form)) definitions))
          (when (equal form '(provide 'org-capture))
            (push 'provide definitions))))
      (unless (equal (nreverse definitions)
                     '(org-capture--list-item-empty-lines-before
                       org-capture-place-item org-capture-place-table-line
                       org-capture-fill-template provide))
        (error "The capture list helper swallowed later top-level forms"))))
  (load "org-capture.el" nil t)
  (require 'org-capture)
  (unless (and (featurep 'org-capture)
               (fboundp 'org-capture--list-item-empty-lines-before)
               (fboundp 'org-capture-place-item)
               (fboundp 'org-capture-fill-template))
    (error "Org capture source did not finish loading its definitions"))
  t)
"##;

const CASES: &str = r##"
(progn
  (dolist (case '((t "- one\n- two\n" nil "- one\n- two\n\n- new\n")
                  (nil "- one\n- two\n" nil "- one\n- two\n- new\n")
                  (auto "- one\n\n- two\n" nil "- one\n\n- two\n\n- new\n")
                  (auto "- one\n- two\n" nil "- one\n- two\n- new\n")
                  (t "- one\n- two\n" (:empty-lines-before 0) "- one\n- two\n- new\n")
                  (nil "- one\n- two\n" (:empty-lines-before 2) "- one\n- two\n\n- new\n")
                  (nil "" nil "- new\n")))
    (with-temp-buffer
      (org-mode)
      (insert (nth 1 case))
      (goto-char (point-min))
      (let ((org-capture-plist
             (append (list :type 'item :template "new" :unnarrowed t) (nth 2 case)))
            (org-blank-before-new-entry
             (list (cons 'heading nil) (cons 'plain-list-item (car case)))))
        (org-capture-place-item)
        (unless (equal (buffer-string) (nth 3 case))
          (error "Wrong captured list spacing: %S produced %S" case (buffer-string))))))
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
fn org_capture_source_loads_and_preserves_list_item_spacing() {
    let mut interp = Interp::new();
    check(&mut interp, "Actual Org capture source load", SETUP);
    check(&mut interp, "List spacing scenarios", CASES);
}
