use remacs_runtime::lisp::Interp;

#[test]
fn sgml_skip_tag_forward_ignores_closing_tags_in_attributes() {
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(|| {
            let mut interp = Interp::new();
            let value = interp
                .eval_str(
                    r#"(progn
                      (require 'sgml-mode)
                      (with-temp-buffer
                        (sgml-mode)
                        (insert "<p><a title=\"</p>\">text</a></p> tail")
                        (goto-char (point-min))
                        (sgml-skip-tag-forward 1)
                        (buffer-substring-no-properties (point) (point-max))))"#,
                )
                .unwrap_or_else(|flow| panic!("SGML tag skip failed: {flow:?}"));
            assert_eq!(
                interp.prin1_to_string(&value).to_unicode_lossy(),
                "\" tail\""
            );
        })
        .unwrap()
        .join()
        .unwrap();
}
