use remacs_runtime::lisp::{Flow, Interp};
use std::path::PathBuf;

const CASES: &str = r#"
(progn
  (require 'cl-lib)
  (require 'abbrev)
  (defvar remacs-inverse-add-abbrev-prompt nil)
  (defun remacs-inverse-add-abbrev-prompt-for (text position arg)
    (let ((table (make-abbrev-table)))
      (with-temp-buffer
        (insert text)
        (goto-char position)
        (cl-letf (((symbol-function 'read-string)
                   (lambda (prompt &rest _)
                     (setq remacs-inverse-add-abbrev-prompt prompt)
                     "expansion"))
                  ((symbol-function 'define-abbrev) (lambda (&rest _) nil))
                  ((symbol-function 'expand-abbrev) (lambda (&rest _) nil)))
          (inverse-add-abbrev table "Global" arg)
          remacs-inverse-add-abbrev-prompt))))
  (unless (equal (remacs-inverse-add-abbrev-prompt-for "foo bar baz" 1 -1)
                 "Expansion for Global abbrev \"foo\": ")
    (error "ARG=-1 should select the first word after point"))
  (unless (equal (remacs-inverse-add-abbrev-prompt-for "foo bar baz" 1 -2)
                 "Expansion for Global abbrev \"bar\": ")
    (error "ARG=-2 should select the second word after point"))
  (unless (equal (remacs-inverse-add-abbrev-prompt-for "foo bar baz" 12 1)
                 "Expansion for Global abbrev \"baz\": ")
    (error "Positive ARG behavior changed"))
  t)
"#;

fn lisp_string(value: &str) -> String {
    format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
}

fn eval(interp: &mut Interp, source: &str) {
    interp.eval_str(source).unwrap_or_else(|flow| match flow {
        Flow::Signal(signal) => panic!(
            "{} {}",
            interp.prin1_to_string(&signal.symbol()),
            interp.prin1_to_string(&signal.data())
        ),
        other => panic!("{other:?}"),
    });
}

fn load_source_abbrev(interp: &mut Interp) {
    let source = PathBuf::from(env!("REMACS_SOURCE_ROOT")).join("lisp/abbrev.el");
    eval(interp, &format!("(load {} nil t)", lisp_string(source.to_str().unwrap())));
}

#[test]
fn inverse_add_abbrev_negative_arguments_count_words_after_point() {
    let mut interp = Interp::new();
    eval(&mut interp, CASES);
    load_source_abbrev(&mut interp);
    eval(&mut interp, CASES);
}
