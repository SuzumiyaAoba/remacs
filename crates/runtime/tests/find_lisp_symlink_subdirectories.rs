use remacs_runtime::lisp::{Flow, Interp};
use std::path::PathBuf;

const CASES: &str = r#"(progn
  (defvar remacs-3061-public-results nil)
  (defun remacs-3061-capture-public-dired (directory file-predicate
                                                   directory-predicate
                                                   _buffer-name
                                                   &optional _other-window)
    (setq remacs-3061-public-results
          (sort
           (mapcar #'file-name-nondirectory
                   (find-lisp-find-files-internal
                    directory file-predicate directory-predicate))
           #'string<)))
  (defun remacs-3061-subdirectories-via-public-api (directory)
    (let ((original (symbol-function 'find-lisp-find-dired-internal))
          (remacs-3061-public-results nil))
      (unwind-protect
          (progn
            (fset 'find-lisp-find-dired-internal
                  #'remacs-3061-capture-public-dired)
            (find-lisp-find-dired-subdirectories directory)
            remacs-3061-public-results)
        (fset 'find-lisp-find-dired-internal original))))
  (let* ((root (make-temp-file "remacs-find-lisp-3061-" t))
         (real (expand-file-name "real" root))
         (nested (expand-file-name "nested" real))
         (dir-link (expand-file-name "dir-link" root))
         (regular (expand-file-name "plain.txt" root))
         (file-link (expand-file-name "file-link" root))
         (dangling (expand-file-name "dangling" root))
         (cycle (expand-file-name "cycle" real)))
    (unwind-protect
        (progn
          (make-directory nested t)
          (with-temp-file regular (insert "file"))
          (make-symbolic-link real dir-link)
          (make-symbolic-link regular file-link)
          (make-symbolic-link (expand-file-name "missing" root) dangling)
          (make-symbolic-link root cycle)
          (let ((expected '("nested" "real"))
                (direct
                 (sort
                  (mapcar #'file-name-nondirectory
                          (find-lisp-find-files-internal
                           root #'find-lisp-file-predicate-is-directory
                           #'find-lisp-default-directory-predicate))
                  #'string<))
                (public (remacs-3061-subdirectories-via-public-api root)))
            (list
             (equal direct expected)
             (equal public expected)
             (find-lisp-default-directory-predicate "real" root)
             (not (find-lisp-default-directory-predicate "dir-link" root))
             (not (find-lisp-default-directory-predicate "cycle" real))
             (find-lisp-file-predicate-is-directory "real" root)
             (not (find-lisp-file-predicate-is-directory "dir-link" root))
             (not (find-lisp-file-predicate-is-directory "plain.txt" root))
             (not (find-lisp-file-predicate-is-directory "file-link" root))
             (not (find-lisp-file-predicate-is-directory "dangling" root)))))
      (dolist (link (list cycle file-link dangling dir-link))
        (when (file-symlink-p link)
          (delete-file link)))
      (when (file-directory-p root)
        (delete-directory root t)))))"#;

fn lisp_string(value: &str) -> String {
    format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
}

#[test]
fn find_lisp_subdirectory_results_exclude_directory_symlinks() {
    let source = PathBuf::from(env!("REMACS_SOURCE_ROOT"))
        .join("lisp/find-lisp.el")
        .canonicalize()
        .unwrap();
    let code = format!(
        "(progn (setq noninteractive t) (load {} nil t t) {})",
        lisp_string(source.to_str().unwrap()),
        CASES
    );
    let mut interp = Interp::new();
    let result = interp.eval_str(&code).unwrap_or_else(|flow| match flow {
        Flow::Signal(signal) => panic!(
            "{} {}",
            interp.prin1_to_string(&signal.symbol()),
            interp.prin1_to_string(&signal.data())
        ),
        other => panic!("{other:?}"),
    });
    assert_eq!(interp.prin1_to_string(&result), "(t t t t t t t t t t)");
}
