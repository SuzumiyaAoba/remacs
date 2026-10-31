use remacs_runtime::lisp::{Flow, Interp};

const CASES: &str = r#"
(progn
  (require 'filesets)
  (dolist (docile '(nil t))
    (dolist (readable '(nil t))
      (dolist (infer '(nil t))
        (let* ((filesets-be-docile-flag docile)
               (file (propertize "/mock/missing 日本.txt" 'face 'bold))
               (fileset (copy-sequence "test set"))
               (lookups 0) opened notices
               (opener (lambda (name) (setq opened name) :opened)))
          (cl-letf (((symbol-function 'file-readable-p)
                     (lambda (name) (unless (eq name file) (error "Changed the filename object")) readable))
                    ((symbol-function 'filesets-entry-get-open-fn)
                     (lambda (name &rest _)
                       (unless (eq name fileset) (error "Changed the fileset object"))
                       (setq lookups (1+ lookups)) opener))
                    ((symbol-function 'message) (lambda (&rest _) (setq notices t) :skipped)))
            (let* ((allowed (or readable (not docile)))
                   (result (filesets-file-open (unless infer opener) file fileset)))
              (unless (and (= lookups (if infer 1 0))
                           (eq opened (and allowed file))
                           (eq result (if allowed :opened :skipped))
                           (eq notices (not allowed)))
                (error "Filesets %S/%S/%S: result %S, opened %S, notice %S, lookups %S"
                       docile readable infer result opened notices lookups))))))))
  (let ((filesets-be-docile-flag nil))
    (cl-letf (((symbol-function 'file-readable-p) (lambda (_) nil)))
      (unless (equal (condition-case err
                         (filesets-file-open (lambda (_) (error "Open failed")) "/mock/missing")
                       (error err))
                     '(error "Open failed"))
        (error "Filesets swallowed the opener failure"))))
  ;; The real default opener can visit a new file without creating it on disk.
  (let* ((directory (make-temp-file "remacs-filesets-docile-" t))
         (file (concat (file-name-as-directory directory) "new.txt"))
         (filesets-be-docile-flag nil)
         (filesets-open-file-function #'filesets-find-file)
         (filesets-find-file-delay 0)
         (filesets-data nil))
    (unwind-protect
        (save-window-excursion
          (filesets-file-open nil file)
          (unless (and (equal buffer-file-name file) (not (file-exists-p file)))
            (error "Filesets did not visit the missing file: %S" buffer-file-name)))
      (let ((buffer (get-file-buffer file)))
        (when (buffer-live-p buffer)
          (with-current-buffer buffer (set-buffer-modified-p nil))
          (kill-buffer buffer)))
      (delete-directory directory t)))
  t)
"#;

#[test]
fn non_docile_filesets_can_open_missing_files() {
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
