use remacs_runtime::lisp::{Flow, Interp};
use std::path::PathBuf;

const CASES: &str = r###"(progn
  (defvar compilation-arguments nil)
  (defvar compilation-error-case-fold-search nil)
  (defvar compilation-error-regexp-alist nil)
  (defvar compilation-error-regexp-alist-alist nil)
  (defvar remacs-2963-matches nil)
  (defun remacs-2963-check (label condition actual)
    (unless condition
      (error "Compilation OMake regexp scope #2963 failed: %S: %S" label actual))
    t)
  (defun remacs-2963-run (command)
    (let ((compilation-arguments (list command))
          (compilation-error-regexp-alist '(test-gcc omake))
          (compilation-error-regexp-alist-alist
           '((test-gcc . ("^\\([^ :]+\\):\\([0-9]+\\):\\([0-9]+\\): error" 1 2 3))
             (omake . ("^\\*\\*\\* omake: file \\(.*\\) changed" 1 nil nil nil nil))))
          (compilation-error-case-fold-search nil)
          (remacs-2963-matches nil)
          (had-properties (fboundp 'compilation-error-properties))
          (old-properties
           (and (fboundp 'compilation-error-properties)
                (symbol-function 'compilation-error-properties)))
          (buffer (generate-new-buffer " *compilation-omake-scope*")))
      (unwind-protect
          (with-current-buffer buffer
            (insert "      fake.c:7:3: error: ordinary output\n")
            (fset 'compilation-error-properties
                  (lambda (&rest args)
                    (setq remacs-2963-matches
                          (cons (nth 7 args) remacs-2963-matches))
                    nil))
            (compilation-parse-errors (point-min) (point-max))
            (list (not (null (compilation--omake-command-p)))
                  (nreverse remacs-2963-matches)))
        (if had-properties
            (fset 'compilation-error-properties old-properties)
          (fmakunbound 'compilation-error-properties))
        (kill-buffer buffer))))
  (let ((result (remacs-2963-run "gcc -c fake.c")))
    (remacs-2963-check
     'regular-compiler-does-not-accept-omake-indentation
     (equal result '(nil nil)) result))
  (let ((result (remacs-2963-run "omake -P")))
    (remacs-2963-check
     'omake-command-keeps-indented-compiler-matches
     (equal result '(t (test-gcc))) result))
  t)"###;

#[test]
fn omake_indentation_workaround_is_scoped_to_omake_commands() {
    let source = PathBuf::from(env!("REMACS_SOURCE_ROOT"))
        .join("lisp/compile.el")
        .canonicalize()
        .unwrap();
    let source = std::fs::read_to_string(source).unwrap();
    let start = source
        .find("(defun compilation--omake-command-p")
        .expect("OMake command predicate");
    let end = start
        + source[start..]
            .find("\n(defvar-local compilation--parsed")
            .expect("end of compilation-parse-errors");
    let definitions = &source[start..end];
    let code = format!("(progn {} {})", definitions, CASES);
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
