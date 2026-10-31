use remacs_runtime::lisp::{Flow, Interp};

#[test]
fn sequences_report_only_terms_that_failed_at_the_rightmost_position() {
    let mut interp = Interp::new();
    let result = interp
        .eval_str(
            r#"(progn
                 (require 'peg)
                 (defun remacs-test-peg-failure (text form &optional at-end)
                   (with-temp-buffer
                     (insert text)
                     (unless at-end (goto-char (point-min)))
                     (condition-case err
                         (eval form t)
                       (peg-search-failed
                        (list (car err) (cadr err)
                              (sort (copy-sequence (nth 2 err)) #'string-lessp)
                              (point))))))
                 (list
                  (remacs-test-peg-failure "ax" '(peg-parse "a" "b") t)
                  (remacs-test-peg-failure "xx" '(peg-parse "a" "b"))
                  (remacs-test-peg-failure "ax" '(peg-parse "a" "b"))
                  (remacs-test-peg-failure "abx" '(peg-parse "a" "b" "c"))
                  (remacs-test-peg-failure
                   "ax" '(peg-parse (or (and "a" "b") (and "a" "c"))))
                  (remacs-test-peg-failure
                   "abx" '(peg-parse (or (and "a" "b" "c") (and "a" "d"))))
                  (remacs-test-peg-failure "x" '(peg-parse (guard t) "b"))
                  (remacs-test-peg-failure "日x" '(peg-parse "日" "本"))
                  (remacs-test-peg-failure "alphaX" '(peg-parse "alpha" "beta"))
                  (remacs-test-peg-failure "ab" '(peg-parse "a" "b"))))"#,
        )
        .unwrap_or_else(|flow| match flow {
            Flow::Signal(signal) => panic!(
                "{} {}",
                interp.prin1_to_string(&signal.symbol()),
                interp.prin1_to_string(&signal.data())
            ),
            other => panic!("{other:?}"),
        });
    let expected = [
        r#"(peg-search-failed 3 ("a") 3)"#,
        r#"(peg-search-failed 1 ("a") 1)"#,
        r#"(peg-search-failed 2 ("b") 2)"#,
        r#"(peg-search-failed 3 ("c") 3)"#,
        r#"(peg-search-failed 2 ("b" "c") 2)"#,
        r#"(peg-search-failed 3 ("c") 3)"#,
        r#"(peg-search-failed 1 ("b") 1)"#,
        r#"(peg-search-failed 2 ("本") 2)"#,
        r#"(peg-search-failed 6 ("beta") 6)"#,
        "t",
    ]
    .join(" ");
    assert_eq!(interp.prin1_to_string(&result), format!("({expected})"));
}
