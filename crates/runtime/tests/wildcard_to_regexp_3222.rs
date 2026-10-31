use std::path::PathBuf;

use remacs_runtime::lisp::{Flow, Interp};

const CASES: &str = r#"(list
  (not (null (string-match-p (wildcard-to-regexp "foo[bar") "foo[bar")))
  (not (null (string-match-p (wildcard-to-regexp "foo[bar*") "foo[bar-tail")))
  (not (null (string-match-p (wildcard-to-regexp "foo[bar*") "foo[bar")))
  (not (string-match-p (wildcard-to-regexp "foo[bar") "fooXbar"))
  (let ((directory (make-temp-file "remacs-wildcard-3222-" t)))
    (unwind-protect
        (progn
          (with-temp-file (expand-file-name "foo[bar" directory)
            (insert ""))
          (with-temp-file (expand-file-name "foo[bar-tail" directory)
            (insert ""))
          (list
           (equal (file-expand-wildcards
                   (expand-file-name "foo[bar" directory))
                  (list (expand-file-name "foo[bar" directory)))
           (equal (file-expand-wildcards
                   (expand-file-name "foo[bar*" directory))
                  (sort (list (expand-file-name "foo[bar" directory)
                              (expand-file-name "foo[bar-tail" directory))
                        #'string<))))
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
fn unmatched_open_brackets_are_literal_and_later_wildcards_still_work() {
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
    assert_eq!(interp.prin1_to_string(&result), "(t t t t (t t))");

    let result = eval(&mut interp, &source_eval);
    assert_eq!(interp.prin1_to_string(&result), "(t t t t (t t))");
}
