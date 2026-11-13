use remacs_runtime::lisp::{Flow, Interp};

#[test]
fn break_line_continuing_a_multicharacter_token_adds_leading_ampersand() {
    let mut interp = Interp::new();
    let result = interp
        .eval_str(
            r#"(progn
              (provide 'cus-edit)
              (defmacro easy-menu-define (&rest _) nil)
              (require 'f90)
              (with-temp-buffer
                (f90-mode)
                (setq f90-beginning-ampersand nil)
                (insert "if a == b then\n")
                (goto-char (point-min))
                (search-forward "==")
                (backward-char 1)
                (f90-break-line t)
                (buffer-string)))"#,
        )
        .unwrap_or_else(|flow| match flow {
            Flow::Signal(signal) => panic!(
                "{} {}",
                interp.prin1_to_string(&signal.symbol()),
                interp.prin1_to_string(&signal.data())
            ),
            other => panic!("{other:?}"),
        });

    assert_eq!(
        interp.prin1_to_string(&result).to_string(),
        "\"if a =&\n     &= b then\n\""
    );
}
