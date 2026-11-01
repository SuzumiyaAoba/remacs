use remacs_runtime::lisp::{Flow, Interp};
use std::path::PathBuf;

const CASES: &str = r##"(progn
  (defun remacs-shell-check (actual expected label)
    (unless (equal actual expected)
      (error "%s: got %S, expected %S" label actual expected)))
  (defvar remacs-shell-3041-captured-directories nil)
(defun remacs-shell-3041-capture-directory (directory)
  (setq remacs-shell-3041-captured-directories
        (cons directory remacs-shell-3041-captured-directories)))
(defun remacs-shell-3041-track (command)
  (let ((shell-dirtrack-mode t)
        (remacs-shell-3041-captured-directories nil)
        (original (symbol-function 'shell-process-cd)))
    (unwind-protect
        (progn
          (fset 'shell-process-cd #'remacs-shell-3041-capture-directory)
          (shell-directory-tracker command)
          (nreverse remacs-shell-3041-captured-directories))
      (fset 'shell-process-cd original))))
  (cl-letf (((symbol-function 'process-command)
             (lambda (&rest _) '("/bin/sh"))))
    (let ((shell-dumb-shell-regexp "sh"))
    (remacs-shell-check (shell-unquote-argument "'foo\\bar'") "foo\\bar" "dumb-shell keeps quoted backslash")
    (remacs-shell-check (shell-unquote-argument "foo\\ bar") "foo\\ bar" "dumb-shell keeps backslash escapes"))
    (remacs-shell-check (shell-unquote-argument "'foo\\bar'") "foo\\bar" "single-quoted backslash is literal")
    (remacs-shell-check (shell-unquote-argument "\"foo\\q\"") "foo\\q" "double-quoted nonspecial backslash is literal")
    (remacs-shell-check (shell-unquote-argument "\"foo\\$bar\"") "foo$bar" "double-quoted dollar escape removes backslash")
    (remacs-shell-check (shell-unquote-argument "\"foo\\`bar\"") "foo`bar" "double-quoted backtick escape removes backslash")
    (remacs-shell-check (shell-unquote-argument "\"foo\\\"bar\"") "foo\"bar" "double-quoted quote escape removes backslash")
    (remacs-shell-check (shell-unquote-argument "\"foo\\\\bar\"") "foo\\bar" "double-quoted backslash escape collapses pair")
    (remacs-shell-check (shell-unquote-argument "foo\\ bar") "foo bar" "unquoted backslash escape remains")
    (remacs-shell-check (shell-unquote-argument "foo\\\nbar") "foobar" "unquoted backslash-newline is removed")
    (remacs-shell-check (shell-unquote-argument "\"foo\\\nbar\"") "foobar" "double-quoted backslash-newline is removed")
    (remacs-shell-check (shell-unquote-argument "'foo\\\nbar'") "foo\\\nbar" "single-quoted backslash-newline stays literal")
    (remacs-shell-check (shell-unquote-argument "''") "" "empty single-quoted word is preserved")
    (remacs-shell-check (shell-unquote-argument "\"\"") "" "empty double-quoted word is preserved")
    (unless (equal (remacs-shell-3041-track "cd '/tmp/foo\\bar'") (list "/tmp/foo\\bar")) (error "directory tracker lost quoted backslash"))
    t))"##;

fn lisp_string(value: &str) -> String {
    format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
}

#[test]
fn shell_unquote_preserves_quoted_backslashes() {
    let source = PathBuf::from(env!("REMACS_SOURCE_ROOT"))
        .join("lisp/shell.el")
        .canonicalize()
        .unwrap();
    let code = format!(
        "(progn (setq noninteractive t) (require 'cl-lib) (load {} nil t t) {})",
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
    assert_eq!(interp.prin1_to_string(&result), "t");
}
