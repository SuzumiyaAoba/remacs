use remacs_runtime::lisp::{Flow, Interp};

const CASES: &str = r#"(progn
  (require 'uniquify)
  (defvar remacs-2642-file-buffer nil)
  (defvar remacs-2642-ignored-buffer nil)
  (defvar remacs-2642-ignored-items nil)
  (let ((uniquify-buffer-name-style 'post-forward-angle-brackets)
        (uniquify-min-dir-content 0)
        (uniquify-strip-common-suffix nil)
        (uniquify-ignore-buffers-re "^ignored$")
        (uniquify-after-kill-buffer-flag nil))
    (setq remacs-2642-file-buffer (get-buffer-create "same")
          remacs-2642-ignored-buffer (get-buffer-create "ignored"))
    (unwind-protect
        (progn
          (with-current-buffer remacs-2642-file-buffer
            (setq buffer-file-name "/a/same")
            (setq-local uniquify-managed
                        (list (uniquify-make-item
                               "same" "/a" remacs-2642-file-buffer))))
          (with-current-buffer remacs-2642-ignored-buffer
            (setq buffer-file-name "/b/same")
            (setq-local uniquify-managed
                        (list (uniquify-make-item
                               "same" "/b" remacs-2642-ignored-buffer))))
          (setq remacs-2642-ignored-items
                (buffer-local-value 'uniquify-managed
                                    remacs-2642-ignored-buffer))
          (let ((names (mapcar #'substring-no-properties
                               (uniquify-get-unique-names
                                (list remacs-2642-file-buffer
                                      remacs-2642-ignored-buffer)))))
            (unless (and (equal names '("same" "ignored"))
                         (equal (buffer-name remacs-2642-file-buffer) "same")
                         (equal (buffer-name remacs-2642-ignored-buffer) "ignored")
                         (eq remacs-2642-ignored-items
                             (buffer-local-value 'uniquify-managed
                                                 remacs-2642-ignored-buffer)))
              (error "Ignored buffer was changed in stateless uniquification: %S"
                     names))))
      (dolist (buffer (list remacs-2642-file-buffer
                            remacs-2642-ignored-buffer))
        (when (buffer-live-p buffer)
          (kill-buffer buffer))))
  t))"#;

#[test]
fn stateless_names_leave_ignored_buffers_unchanged() {
    let mut interp = Interp::new();
    match interp.eval_str(CASES) {
        Ok(value) => assert!(value.truthy()),
        Err(Flow::Signal(signal)) => panic!(
            "{} {}",
            interp.prin1_to_string(&signal.symbol()),
            interp.prin1_to_string(&signal.data())
        ),
        Err(other) => panic!("{other:?}"),
    }
}
