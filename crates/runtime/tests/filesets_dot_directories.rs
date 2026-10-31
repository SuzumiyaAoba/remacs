use remacs_runtime::lisp::{Flow, Interp};

const CASES: &str = r#"
(progn
  (require 'filesets)
  (let ((names '("./" "../" ".../" ".... /" ".hidden/" "..." "plain" ".hidden")))
    (unless (equal (filesets-filter-dir-names names) '(".../" ".... /" ".hidden/"))
      (error "Filesets dropped valid dot directories"))
    (unless (equal (filesets-filter-dir-names names t) '("..." "plain" ".hidden"))
      (error "Filesets changed the non-directory filter")))
  (let* ((name (propertize ".../" 'face 'bold))
         (result (filesets-filter-dir-names (list "./" name "../"))))
    (unless (and (= (length result) 1) (eq (car result) name))
      (error "Filesets changed a retained filename object")))
  (unless (null (filesets-filter-dir-names nil)) (error "Filesets changed the empty list"))
  (defun remacs-test-filesets-names (label actual expected)
    (unless (equal (sort (copy-sequence actual) #'string<)
                   (sort (copy-sequence expected) #'string<))
      (error "Filesets %S: expected %S, got %S" label expected actual)))
  ;; Dot-only directory names are ordinary filesystem entries on Unix.
  (unless (memq system-type '(windows-nt ms-dos))
    (let* ((directory (make-temp-file "remacs-filesets-dots-" t))
           (prefix (file-name-as-directory directory))
           (directories '(".../" "..../" ".hidden/" "normal/"))
           (files '("....." ".plain" "plain"))
           (filesets-sort-menu-flag nil)
           (filesets-verbosity 0))
      (unwind-protect
          (progn
            (dolist (name directories) (make-directory (concat prefix name)))
            (dolist (name files) (with-temp-file (concat prefix name) (insert "x")))
            (remacs-test-filesets-names 'directories
              (filesets-directory-files directory nil :dirs) directories)
            (remacs-test-filesets-names 'files
              (filesets-directory-files directory nil :files) files)
            (remacs-test-filesets-names 'all
              (filesets-directory-files directory) (append files directories))
            (remacs-test-filesets-names 'full-paths
              (filesets-directory-files directory nil :dirs t)
              (mapcar (lambda (name) (concat prefix name)) directories))
            (remacs-test-filesets-names 'directory-pattern
              (filesets-directory-files directory "\\`\\.\\.\\./\\'" :dirs nil t) '(".../"))
            (remacs-test-filesets-names 'unfiltered-directories
              (filesets-directory-files directory "\\`\\.\\.\\./\\'" :dirs nil nil) directories))
        (delete-directory directory t))))
  t)
"#;

#[test]
fn filesets_keep_valid_dot_only_directories_and_exclude_only_dot_entries() {
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
