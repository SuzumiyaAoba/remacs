use remacs_runtime::lisp::{Flow, Interp};
use std::path::PathBuf;

const CASES: &str = r##"(progn
  (defun remacs-shell-check (actual expected label)
    (unless (equal actual expected)
      (error "%s: got %S, expected %S" label actual expected)))
  (cl-letf (((symbol-function 'process-command)
             (lambda (&rest _) '("/bin/sh"))))
    (with-temp-buffer
      (let ((shell-file-name-quote-list (list ?\\)))
        (shell-completion-vars)
        (unless (eq comint-unquote-function #'shell--unquote-argument)
          (error "shell completion unquote function changed"))
        (unless (eq comint-requote-function #'shell--requote-argument)
          (error "shell completion requote function changed"))
        (remacs-shell-check (funcall comint-unquote-function "\"foo\\q\"") "foo\\q" "double-quoted nonspecial escape retained")
        (remacs-shell-check (funcall comint-unquote-function "\"foo\\$HOME\"") "foo$HOME" "escaped dollar is not expanded")
        (remacs-shell-check (funcall comint-unquote-function "\"foo\\`bar\"") "foo`bar" "escaped backtick is literal")
        (remacs-shell-check (funcall comint-unquote-function "\"foo\\\"bar\"") "foo\"bar" "escaped quote is retained")
        (remacs-shell-check (funcall comint-unquote-function "\"foo\\\\bar\"") "foo\\bar" "escaped backslash is reduced")
        (remacs-shell-check (funcall comint-unquote-function "\"foo\\\nbar\"") "foobar" "double-quoted line continuation is removed")
        (remacs-shell-check (funcall comint-unquote-function "foo\\\nbar") "foobar" "unquoted line continuation is removed")
        (remacs-shell-check (funcall comint-unquote-function "foo\\q") "fooq" "outside-quote escape behavior is unchanged")
        (remacs-shell-check (funcall comint-unquote-function "'foo\\q'") "foo\\q" "single-quoted backslash remains literal")
        (remacs-shell-check (funcall comint-unquote-function "'foo\\\nbar'") "foo\\\nbar" "single-quoted line continuation stays literal")
        (remacs-shell-check (funcall comint-unquote-function "\"\"") "" "empty double-quoted word remains empty")
        (remacs-shell-check (funcall comint-unquote-function "''") "" "empty single-quoted word remains empty")
        (let* ((input "\"foo\\q\"")
               (unquoted (funcall comint-unquote-function input))
               (before-q (funcall comint-requote-function 4 input))
               (after-q (funcall comint-requote-function 5 input))
               (shell-quoted (funcall (cdr after-q) unquoted)))
          (unless (equal unquoted "foo\\q")
            (error "completion unquote lost literal backslash: %S" unquoted))
          (unless (and (= (car before-q) 5) (= (car after-q) 6))
            (error "completion cursor mapping changed: %S / %S"
                   before-q after-q))
          (remacs-shell-check (funcall (cdr after-q) "foobar" t) "foobar\""
                              "final completion preserves the closing quote")
          (unless (equal (funcall comint-unquote-function shell-quoted)
                         unquoted)
            (error "comint quote/unquote round trip changed: %S" shell-quoted)))
        (let ((table (completion-table-with-quoting
                      '("foobar") #'shell--unquote-argument
                      #'shell--requote-argument)))
          (remacs-shell-check (try-completion "\"fo\"" table) "\"foobar\""
                              "quoted completion retains the closing quote")
          (remacs-shell-check (car (all-completions "\"fo\"" table nil))
                              "\"foobar\""
                              "listed completion retains the closing quote"))
        (remacs-shell-check (completion--twq-requote #'identity "foobar" t)
                            "foobar" "one-argument requote callbacks remain supported")
        ;; With no configured escape quoting, preserve the existing branch.
        (let ((comint-file-name-quote-list nil))
          (remacs-shell-check (shell--unquote-argument "\"foo\\q\"") "foo\\q"
                              "no-quote-list behavior remains literal"))
        t))))"##;

fn lisp_string(value: &str) -> String {
    format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
}

#[test]
fn shell_completion_preserves_nonspecial_double_quote_backslashes() {
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
