use remacs_runtime::lisp::{Flow, Interp};

const CASES: &str = r#"
(progn
  (require 'cl-lib)
  (require 'saveplace)
  (setq save-place-loaded t)
  (let ((path "/private/tmp/ignored-place.txt"))
    (cl-labels ((run-case (file enabled ignored abbreviate &optional saved-name)
                  (let ((save-place-alist (list (cons (or saved-name file) 42)))
                        (save-place-ignore-files-regexp
                         (and ignored "ignored-place"))
                        (save-place-abbreviate-file-names abbreviate))
                    (with-temp-buffer
                      (insert "x")
                      (goto-char (point-max))
                      (setq buffer-file-name file)
                      (setq-local save-place-mode enabled)
                      (save-place-to-alist)
                      save-place-alist))))
      (unless (null (run-case path nil t nil))
        (error "Ignored file place remains after disabling save-place-mode"))
      (unless (null (run-case path nil nil nil))
        (error "Non-ignored file place remains after disabling save-place-mode"))
      (unless (equal (run-case path t t nil) (list (cons path 42)))
        (error "Ignoring an enabled file discarded its existing place"))
      (unless (equal (run-case path t nil nil)
                     (list (cons path 2)))
        (error "Enabled non-ignored file did not record its point"))
      (let* ((home-path (expand-file-name "ignored-place.txt" "~"))
             (abbreviated-path (abbreviate-file-name home-path)))
        (unless (null (run-case home-path nil t t abbreviated-path))
          (error "Disabling save-place-mode did not delete abbreviated entry")))))
  t)
"#;

#[test]
fn disabling_save_place_removes_entries_even_for_ignored_files() {
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
