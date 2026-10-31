use remacs_runtime::lisp::{Flow, Interp};

const CASES: &str = r#"
(progn
  (require 'icomplete)
  (defun remacs-test-fido-kill-path (field thing expected category answer)
    (with-temp-buffer
      (insert field)
      (goto-char (point-max))
      (setq buffer-read-only t)
      (let* ((default-directory "/tmp/Remacs default/")
             (tail (cons "remaining" 5))
             (all (cons thing tail)) calls)
        (cl-letf (((symbol-function 'completion-all-sorted-completions) (lambda (&rest _) all))
                  ((symbol-function 'icomplete--field-end) (lambda () (point-max)))
                  ((symbol-function 'icomplete--field-beg) (lambda () (point-min)))
                  ((symbol-function 'icomplete--category) (lambda () category))
                  ((symbol-function 'icomplete--field-string) (lambda () field))
                  ((symbol-function 'yes-or-no-p)
                   (lambda (prompt) (push (cons 'prompt prompt) calls) answer))
                  ((symbol-function 'delete-file) (lambda (path) (push (cons 'delete path) calls) nil))
                  ((symbol-function 'completion--cache-all-sorted-completions)
                   (lambda (beg end remaining)
                     (unless (and (= beg (point-min)) (= end (point-max)) (eq remaining tail))
                       (error "Fido changed the remaining completion cache"))
                     (push 'cache calls)))
                  ((symbol-function 'message) #'ignore))
          (icomplete-fido-kill))
        (setq calls (nreverse calls))
        (unless (and (equal calls
                            (append (list (cons 'prompt (concat "Delete file " expected "? ")))
                                    (and answer (list (cons 'delete expected) 'cache))))
                     (equal (buffer-string) field) (= (point) (point-max)) buffer-read-only)
          (error "Fido %S/%S/%S/%S: expected %S, got %S"
                 field thing category answer expected calls)))))
  (let ((process-environment (copy-sequence process-environment)))
    (setenv "REMACS_FIDO_DIR" "/tmp/Remacs env 日本")
    (setenv "REMACS_FIDO_FILE" "/tmp/Remacs env 日本/full-file")
    (setenv "REMACS_FIDO_LITERAL" "/tmp/$REMACS_FIDO_DIR")
    (setenv "REMACS_FIDO_EMPTY" "")
    (setenv "REMACS_FIDO_UNDEFINED" nil)
    (dolist (case '(("$REMACS_FIDO_DIR/se" "selected" "/tmp/Remacs env 日本/selected")
                    ("${REMACS_FIDO_DIR}/se" "selected" "/tmp/Remacs env 日本/selected")
                    ("$REMACS_FIDO_FILE" "selected" "/tmp/Remacs env 日本/selected")
                    ("$REMACS_FIDO_DIR" "selected" "/tmp/selected")
                    ("$$REMACS_FIDO_DIR/se" "selected" "/tmp/Remacs default/$REMACS_FIDO_DIR/selected")
                    ("$REMACS_FIDO_LITERAL/se" "selected" "/tmp/$REMACS_FIDO_DIR/selected")
                    ("$REMACS_FIDO_EMPTY/se" "selected" "/selected")
                    ("$REMACS_FIDO_UNDEFINED/se" "selected" "/selected")
                    ("relative/se" "selected" "/tmp/Remacs default/relative/selected")
                    ("file" "selected" "/tmp/Remacs default/selected")
                    ("$REMACS_FIDO_DIR/se" "src/selected" "/tmp/Remacs env 日本/src/selected")
                    ("$REMACS_FIDO_DIR/se" "日本 file.txt" "/tmp/Remacs env 日本/日本 file.txt")
                    ("$REMACS_FIDO_DIR/se" "$REMACS_FIDO_DIR" "/tmp/Remacs env 日本/$REMACS_FIDO_DIR")
                    ("$REMACS_FIDO_DIR/se" "/absolute/selected" "/absolute/selected")))
      (dolist (category '(file project-file))
        (dolist (answer '(nil t))
          (remacs-test-fido-kill-path (nth 0 case) (nth 1 case) (nth 2 case) category answer)))))
  ;; A failed deletion must retain the candidate in the cache.
  (with-temp-buffer
    (let ((process-environment (copy-sequence process-environment)) deleted cached)
      (setenv "REMACS_FIDO_DIR" "/tmp/Remacs env 日本")
      (cl-letf (((symbol-function 'icomplete--field-end) (lambda () (point-max)))
                ((symbol-function 'icomplete--field-string) (lambda () "$REMACS_FIDO_DIR/se"))
                ((symbol-function 'icomplete--category) (lambda () 'file))
                ((symbol-function 'completion-all-sorted-completions) (lambda (&rest _) '("selected")))
                ((symbol-function 'yes-or-no-p) (lambda (&rest _) t))
                ((symbol-function 'delete-file)
                 (lambda (path) (setq deleted path) (signal 'file-error '("Delete refused"))))
                ((symbol-function 'completion--cache-all-sorted-completions)
                 (lambda (&rest _) (setq cached t))))
        (unless (equal (condition-case err (icomplete-fido-kill) (error err))
                       '(file-error "Delete refused"))
          (error "Fido did not propagate deletion failure")))
      (unless (and (equal deleted "/tmp/Remacs env 日本/selected") (null cached))
        (error "Failed deletion changed the completion cache"))))
  t)
"#;

#[test]
fn fido_kill_expands_input_directories_before_confirming_file_deletion() {
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
