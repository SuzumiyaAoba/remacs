use std::path::PathBuf;

use remacs_runtime::lisp::{Flow, Interp};

const CASES: &str = r#"(list
  (not (null (string-match-p (wildcard-to-regexp "foo\\*") "foo*")))
  (not (string-match-p (wildcard-to-regexp "foo\\*") "foox"))
  (not (null (string-match-p (wildcard-to-regexp "foo\\?") "foo?")))
  (not (string-match-p (wildcard-to-regexp "foo\\?") "fooa"))
  (not (null (string-match-p (wildcard-to-regexp "foo\\[bar\\]")
                             "foo[bar]")))
  (not (null (string-match-p (wildcard-to-regexp "foo\\a") "fooa")))
  (not (null (string-match-p (wildcard-to-regexp "foo[\\]]") "foo]")))
  (not (string-match-p (wildcard-to-regexp "foo[\\]]") "fooa"))
  (not (string-match-p (wildcard-to-regexp "[a-\\]]") "a"))
  (not (string-match-p (wildcard-to-regexp "[a-\\]]") "]"))
  (not (string-match-p (wildcard-to-regexp "[a-\\]]") "-"))
  (not (null (string-match-p (wildcard-to-regexp "[\\]-a]") "_")))
  (not (null (string-match-p (wildcard-to-regexp "[\\]-a]") "]")))
  (not (string-match-p (wildcard-to-regexp "[\\]-a]") "b"))
  (not (null (string-match-p (wildcard-to-regexp "[x\\]-a]") "_")))
  (not (null (string-match-p (wildcard-to-regexp "[x\\]-a]") "x")))
  (not (string-match-p (wildcard-to-regexp "[x\\]-a]") "b"))
  (not (null (string-match-p (wildcard-to-regexp "foo\\\\") "foo\\"))))"#;

fn lisp_string(value: &str) -> String {
    format!(
        "\"{}\"",
        value.replace('\\', "\\\\").replace('"', "\\\"")
    )
}

#[test]
fn wildcard_backslash_quotes_and_consumes_the_next_character() {
    let mut interp = Interp::new();
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
    let result = interp.eval_str(CASES).unwrap_or_else(|flow| match flow {
        Flow::Signal(signal) => panic!(
            "{} {}",
            interp.prin1_to_string(&signal.symbol()),
            interp.prin1_to_string(&signal.data())
        ),
        other => panic!("{other:?}"),
    });
    assert_eq!(interp.prin1_to_string(&result), "(t t t t t t t t t t t t t t t t t t)");

    let result = interp.eval_str(&source_eval).unwrap_or_else(|flow| match flow {
        Flow::Signal(signal) => panic!(
            "{} {}",
            interp.prin1_to_string(&signal.symbol()),
            interp.prin1_to_string(&signal.data())
        ),
        other => panic!("{other:?}"),
    });
    assert_eq!(interp.prin1_to_string(&result), "(t t t t t t t t t t t t t t t t t t)");
}
