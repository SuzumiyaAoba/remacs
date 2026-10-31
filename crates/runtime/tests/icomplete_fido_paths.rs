use remacs_runtime::lisp::{Flow, Interp};

const CASES: &str = r#"
(progn
  (require 'icomplete)
  (defun remacs-test-fido-dired (field expected)
    (with-temp-buffer
      (insert field)
      (goto-char (point-max))
      (setq buffer-read-only t)
      (let (calls)
        (cl-letf (((symbol-function 'icomplete--category) (lambda () 'file))
                  ((symbol-function 'icomplete--field-string) (lambda () field))
                  ((symbol-function 'icomplete--field-end) (lambda () (point-max)))
                  ((symbol-function 'dired) (lambda (directory &rest _)
                                             (push (cons 'dired directory) calls)))
                  ((symbol-function 'exit-minibuffer)
                   (lambda () (push 'exit calls) (throw 'remacs-test-fido-exit t))))
          (unless (catch 'remacs-test-fido-exit (icomplete-fido-delete-char) nil)
            (error "Fido did not exit the minibuffer")))
        (unless (and (equal (nreverse calls) (list (cons 'dired expected) 'exit))
                     (equal (buffer-string) field)
                     (= (point) (point-max)) buffer-read-only)
          (error "Fido %S: expected directory %S, got calls %S" field expected calls)))))
  (let ((process-environment (copy-sequence process-environment)))
    (setenv "REMACS_FIDO_DIR" "/tmp/Remacs env 日本")
    (setenv "REMACS_FIDO_FILE" "/tmp/Remacs env 日本/full-file")
    (setenv "REMACS_FIDO_LITERAL" "/tmp/$REMACS_FIDO_DIR")
    (setenv "REMACS_FIDO_EMPTY" "")
    (setenv "REMACS_FIDO_UNDEFINED" nil)
    (dolist (case '(("$REMACS_FIDO_DIR/fo" "/tmp/Remacs env 日本/")
                    ("${REMACS_FIDO_DIR}/fo" "/tmp/Remacs env 日本/")
                    ("$REMACS_FIDO_FILE" "/tmp/Remacs env 日本/")
                    ("$REMACS_FIDO_DIR" "/tmp/")
                    ("$REMACS_FIDO_DIR/" "/tmp/Remacs env 日本/")
                    ("relative/$REMACS_FIDO_EMPTY/fo" "relative//")
                    ("$$REMACS_FIDO_DIR/fo" "$REMACS_FIDO_DIR/")
                    ("$REMACS_FIDO_LITERAL/fo" "/tmp/$REMACS_FIDO_DIR/")
                    ("$REMACS_FIDO_UNDEFINED/fo" "/")
                    ("/plain/path/fo" "/plain/path/")
                    ("relative/fo" "relative/")
                    ("~/fo" "~/")
                    ("" nil) ("fo" nil)))
      (remacs-test-fido-dired (car case) (cadr case))))
  ;; Other categories and positions still run the real editing command.
  (dolist (category '(file buffer))
    (dolist (count '(1 2))
      (with-temp-buffer
        (insert "abcd")
        (goto-char 2)
        (let ((current-prefix-arg count))
          (cl-letf (((symbol-function 'icomplete--category) (lambda () category))
                    ((symbol-function 'icomplete--field-end)
                     (lambda () (if (eq category 'file) (point-max) (point))))
                    ((symbol-function 'icomplete--field-string)
                     (lambda () (error "Read a file field while deleting characters")))
                    ((symbol-function 'dired) (lambda (&rest _) (error "Opened Dired while editing")))
                    ((symbol-function 'exit-minibuffer) (lambda () (error "Exited while editing"))))
            (icomplete-fido-delete-char)))
        (unless (and (= (point) 2) (equal (buffer-string) (if (= count 1) "acd" "ad")))
          (error "Fido changed the editing branch for %S/%S" category count)))))
  (with-temp-buffer
    (insert "field")
    (goto-char (point-max))
    (let (exited)
      (cl-letf (((symbol-function 'icomplete--category) (lambda () 'file))
                ((symbol-function 'icomplete--field-end) (lambda () (point-max)))
                ((symbol-function 'icomplete--field-string) (lambda () "/plain/field"))
                ((symbol-function 'dired) (lambda (&rest _) (error "Dired failed")))
                ((symbol-function 'exit-minibuffer) (lambda () (setq exited t))))
        (unless (equal (condition-case err (icomplete-fido-delete-char) (error err))
                       '(error "Dired failed"))
          (error "Fido did not propagate the Dired error")))
      (when exited (error "Fido exited after Dired failed"))))
  t)
"#;

#[test]
fn fido_delete_char_expands_environment_paths_before_opening_dired() {
    let mut interp = Interp::new();
    match interp.eval_str(CASES) {
        Ok(result) => assert!(result.truthy()),
        Err(Flow::Signal(signal)) => panic!(
            "{} {}",
            interp.prin1_to_string(&signal.symbol()),
            interp.prin1_to_string(&signal.data())
        ),
        Err(other) => panic!("{other:?}"),
    }
}
