use remacs_runtime::lisp::{Flow, Interp};
use std::path::PathBuf;

const CASES: &str = r###"(progn
  (defvar remacs-2953-listed nil)
  (defun remacs-2953-check (label condition)
    (unless condition
      (error "Ido work-directory size guard #2953 failed: %S" label))
    t)
  (defun remacs-2953-with-stubs (make-file-list body)
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
  (let ((ido-big-directories '("\\`/large/\\'"))
        (ido-max-directory-size nil)
        (ido-work-directory-list '("/large/" "/small/"))
        (ido-work-directory-index -1)
        (ido-current-directory "/elsewhere/")
        (ido-text "candidate")
        (ido-cur-item 'file)
        (remacs-2953-listed nil))
    (let ((result
           (remacs-2953-with-stubs
            (lambda (dir &optional _)
              (setq remacs-2953-listed (cons dir remacs-2953-listed))
              (list (concat dir "match")))
            (lambda () (ido-get-work-directory 1 t)))))
      (remacs-2953-check
       'get-work-directory
       (and (equal result "/small/")
            (equal (nreverse remacs-2953-listed) '("/small/"))))))
  (let ((ido-big-directories '("\\`/large/\\'"))
        (ido-max-directory-size nil)
        (ido-work-directory-list '("/large/" "/small/"))
        (ido-current-directory "/elsewhere/")
        (ido-cur-item 'file)
        (ido-merge-ftp-work-directories nil)
        (remacs-2953-listed nil))
    (let ((result
           (remacs-2953-with-stubs
            (lambda (dir &optional _)
              (setq remacs-2953-listed (cons dir remacs-2953-listed))
              (list (concat dir "match")))
            (lambda () (ido-make-merged-file-list-1 "candidate" nil nil)))))
      (remacs-2953-check
       'make-merged-file-list
       (and (equal result '("/small/match"))
            (equal (nreverse remacs-2953-listed) '("/small/"))))))
  t)"###;

fn lisp_string(value: &str) -> String {
    format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
}

#[test]
fn ido_work_directory_search_skips_directories_over_the_size_limit() {
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
