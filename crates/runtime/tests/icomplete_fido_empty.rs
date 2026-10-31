use remacs_runtime::lisp::{Flow, Interp};

const CASES: &str = r#"
(progn
  (require 'icomplete)
  (dolist (category '(file project-file buffer unsupported))
    (with-temp-buffer
      (insert "unchanged")
      (goto-char (point-max))
      (let ((original (current-buffer)) (queries 0) calls)
        (cl-letf (((symbol-function 'completion-all-sorted-completions)
                   (lambda (&rest _) (setq queries (1+ queries)) nil))
                  ((symbol-function 'icomplete--field-end) (lambda () (point-max)))
                  ((symbol-function 'icomplete--field-beg) (lambda () (push 'field-beg calls) 1))
                  ((symbol-function 'icomplete--category) (lambda () (push 'category calls) category))
                  ((symbol-function 'icomplete--field-string)
                   (lambda () (error "Read a file path with no candidates")))
                  ((symbol-function 'yes-or-no-p) (lambda (&rest _) (push 'prompt calls) t))
                  ((symbol-function 'delete-file) (lambda (&rest _) (push 'delete calls)))
                  ((symbol-function 'kill-buffer) (lambda (&rest _) (push 'kill calls) t))
                  ((symbol-function 'completion--cache-all-sorted-completions)
                   (lambda (&rest _) (push 'cache calls)))
                  ((symbol-function 'message) (lambda (&rest _) (push 'message calls) nil)))
          (unless (and (null (icomplete-fido-kill)) (null calls) (= queries 1)
                       (eq (current-buffer) original) (buffer-live-p original)
                       (equal (buffer-string) "unchanged") (= (point) (point-max)))
            (error "Empty %S candidates caused side effects: %S" category calls))))))
  (defun remacs-test-fido-kill-candidate (category answer success)
    (with-temp-buffer
      (insert "field")
      (goto-char (point-max))
      (let* ((tail (cons "remaining" 3))
             (all (cons "selected" tail))
             (prompts 0) victims caches)
        (cl-letf (((symbol-function 'completion-all-sorted-completions) (lambda (&rest _) all))
                  ((symbol-function 'icomplete--field-end) (lambda () (point-max)))
                  ((symbol-function 'icomplete--field-beg) (lambda () (point-min)))
                  ((symbol-function 'icomplete--category) (lambda () category))
                  ((symbol-function 'icomplete--field-string) (lambda () "/tmp/remacs/f"))
                  ((symbol-function 'yes-or-no-p)
                   (lambda (&rest _)
                     (unless (and enable-recursive-minibuffers (null icomplete-mode))
                       (error "Confirmation lost its minibuffer bindings"))
                     (setq prompts (1+ prompts)) answer))
                  ((symbol-function 'kill-buffer) (lambda (name) (push name victims) success))
                  ((symbol-function 'delete-file) (lambda (name) (push name victims) nil))
                  ((symbol-function 'completion--cache-all-sorted-completions)
                   (lambda (&rest arguments) (push arguments caches))))
          (icomplete-fido-kill))
        (unless (and (= prompts 1)
                     (equal victims (and answer (list (if (eq category 'buffer)
                                                         "selected" "/tmp/remacs/selected"))))
                     (= (length caches) (if (and answer success) 1 0)))
          (error "Candidate %S/%S/%S: victims %S, cache %S" category answer success victims caches))
        (when caches
          (unless (and (= (caar caches) (point-min))
                       (= (cadar caches) (point-max))
                       (eq (nth 2 (car caches)) tail))
            (error "Successful deletion lost the remaining completion tail"))))))
  (remacs-test-fido-kill-candidate 'buffer nil t)
  (remacs-test-fido-kill-candidate 'buffer t nil)
  (remacs-test-fido-kill-candidate 'buffer t t)
  (dolist (category '(file project-file))
    (remacs-test-fido-kill-candidate category nil t)
    (remacs-test-fido-kill-candidate category t t))
  ;; The editing branch still runs before querying candidates or categories.
  (with-temp-buffer
    (insert "abc def")
    (goto-char 1)
    (let ((kill-ring nil) (kill-ring-yank-pointer nil)
          (last-command nil) (current-prefix-arg nil))
      (cl-letf (((symbol-function 'icomplete--field-end) (lambda () (point-max)))
                ((symbol-function 'completion-all-sorted-completions)
                 (lambda (&rest _) (error "Queried candidates while killing a line")))
                ((symbol-function 'icomplete--category)
                 (lambda () (error "Queried category while killing a line"))))
        (icomplete-fido-kill))
      (unless (and (equal (buffer-string) "") (equal (car kill-ring) "abc def"))
        (error "Fido did not run the normal kill-line command"))))
  (with-temp-buffer
    (cl-letf (((symbol-function 'icomplete--field-end) (lambda () (point-max)))
              ((symbol-function 'completion-all-sorted-completions) (lambda (&rest _) '("selected")))
              ((symbol-function 'icomplete--category) (lambda () 'unsupported)))
      (unless (condition-case nil (progn (icomplete-fido-kill) nil) (error t))
        (error "Unsupported nonempty completion category stopped reporting an error"))))
  t)
"#;

#[test]
fn fido_kill_skips_empty_candidates_without_targeting_the_current_buffer() {
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
