use std::path::PathBuf;

use remacs_runtime::lisp::{Flow, Interp};

const CASES: &str = r#"(progn
  (let ((directory (make-temp-file "remacs-wildcard-3227-" t)))
    (unwind-protect
        (progn
          (with-temp-file (expand-file-name "[!]" directory)
            (insert "literal"))
          (with-temp-file (expand-file-name "b" directory)
            (insert "letter"))
          (list
           (not (null (string-match-p (wildcard-to-regexp "[!]") "[!]")))
           (not (string-match-p (wildcard-to-regexp "[!]") "b"))
           (not (null (string-match-p (wildcard-to-regexp "[!a]") "b")))
           (not (string-match-p (wildcard-to-regexp "[!a]") "a"))
           (not (null (string-match-p (wildcard-to-regexp "[!]]") "b")))
           (not (string-match-p (wildcard-to-regexp "[!]]") "]"))
           (not (null (string-match-p (wildcard-to-regexp "[!]\\]]") "a")))
           (not (string-match-p (wildcard-to-regexp "[!]\\]]") "]"))
           (equal (file-expand-wildcards
                   (expand-file-name "[!]" directory))
                  (list (expand-file-name "[!]" directory)))))
      (delete-directory directory t))))"#;

fn lisp_string(value: &str) -> String {
    format!(
        "\"{}\"",
        value.replace('\\', "\\\\").replace('"', "\\\"")
    )
}

fn eval(interp: &mut Interp, source: &str) -> remacs_runtime::lisp::Value {
    interp.eval_str(source).unwrap_or_else(|flow| match flow {
        Flow::Signal(signal) => panic!(
            "{} {}",
            interp.prin1_to_string(&signal.symbol()),
            interp.prin1_to_string(&signal.data())
        ),
        other => panic!("{other:?}"),
    })
}

#[test]
fn empty_negated_class_is_literal_and_does_not_break_valid_negation() {
    let source = PathBuf::from(env!("REMACS_SOURCE_ROOT"))
        .join("lisp/files.el")
        .canonicalize()
        .unwrap();
    let source_eval = format!(
        r#"(progn
          (setq noninteractive t)
          (load {} nil t)
          {})"#,
        lisp_string(source.to_str().unwrap()),
        CASES
    );
    let mut interp = Interp::new();
    let result = eval(&mut interp, CASES);
    assert_eq!(
        interp.prin1_to_string(&result),
        "(t t t t t t t t t)"
    );
    let result = eval(&mut interp, &source_eval);
    assert_eq!(
        interp.prin1_to_string(&result),
        "(t t t t t t t t t)"
    );
}
