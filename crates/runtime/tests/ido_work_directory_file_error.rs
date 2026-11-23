use remacs_runtime::lisp::{Flow, Interp};
use std::path::PathBuf;

const CASES: &str = r###"(progn
  (defun remacs-2951-check (label condition)
    (unless condition
      (error "Ido work-directory file-error #2951 failed: %S" label))
    t)
  (defun remacs-2951-with-stubs (make-file-list body)
    (let ((saved (mapcar (lambda (symbol) (cons symbol (symbol-function symbol)))
                         '(file-directory-p ido-nonreadable-directory-p
                           ido-is-ftp-directory ido-make-file-list-1
                           ido-set-matches-1))))
      (unwind-protect
          (progn
            (fset 'file-directory-p (lambda (_) t))
            (fset 'ido-nonreadable-directory-p (lambda (_) nil))
            (fset 'ido-is-ftp-directory (lambda (_) nil))
            (fset 'ido-make-file-list-1 make-file-list)
            (fset 'ido-set-matches-1 #'identity)
            (funcall body))
        (dolist (entry saved)
          (fset (car entry) (cdr entry))))))
  (remacs-2951-check
   'get-work-directory
   (equal
    (let ((ido-work-directory-list '("/unreadable/" "/readable/"))
          (ido-work-directory-index -1)
          (ido-current-directory "/elsewhere/")
          (ido-text "candidate")
          (ido-cur-item 'file))
      (remacs-2951-with-stubs
       (lambda (dir &optional _)
         (if (equal dir "/unreadable/")
             (signal 'file-error (list "Permission denied" dir))
           (list (concat dir "match"))))
       (lambda () (ido-get-work-directory 1 t))))
    "/readable/"))
  (remacs-2951-check
   'make-merged-file-list
   (equal
    (let ((ido-work-directory-list '("/unreadable/" "/readable-after/"))
          (ido-current-directory "/elsewhere/")
          (ido-cur-item 'file)
          (ido-merge-ftp-work-directories nil))
      (remacs-2951-with-stubs
       (lambda (dir &optional _)
         (if (equal dir "/unreadable/")
             (signal 'file-error (list "Permission denied" dir))
           (list (concat dir "match"))))
       (lambda () (ido-make-merged-file-list-1 "candidate" nil nil))))
    '("/readable-after/match")))
  (remacs-2951-check
   'make-merged-file-list-clears-prior-results-after-error
   (equal
    (let ((ido-work-directory-list '("/readable-before/" "/unreadable/"))
          (ido-current-directory "/elsewhere/")
          (ido-cur-item 'file)
          (ido-merge-ftp-work-directories nil))
      (remacs-2951-with-stubs
       (lambda (dir &optional _)
         (if (equal dir "/unreadable/")
             (signal 'file-error (list "Permission denied" dir))
           (list (concat dir "match"))))
       (lambda () (list-length (ido-make-merged-file-list-1 "candidate" nil nil)))))
    1))
  t)"###;

fn lisp_string(value: &str) -> String {
    format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
}

#[test]
fn ido_work_directory_search_skips_directories_that_fail_to_list() {
    let source = PathBuf::from(env!("REMACS_SOURCE_ROOT"))
        .join("lisp/ido.el")
        .canonicalize()
        .unwrap();
    let code = format!(
        "(progn (setq noninteractive t hack-read-symbol-shorthands-function nil) (load {} nil t) {})",
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
    assert!(result.truthy());
}
