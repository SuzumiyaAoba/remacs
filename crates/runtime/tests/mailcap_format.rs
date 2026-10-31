use remacs_runtime::lisp::Interp;

#[test]
fn mailcap_replaces_each_file_placeholder_with_the_same_path() {
    let mut interp = Interp::new();
    let result = interp
        .eval_str(
            r#"(progn
                 (require 'mailcap)
                 (list (mailcap--format-command "cat %s; cat %s" "/tmp/mailcap-file")
                       (mailcap--format-command "echo %%s; cat %s" "/tmp/mailcap-file")))"#,
        )
        .unwrap();
    assert_eq!(
        interp.prin1_to_string(&result),
        "(\"cat /tmp/mailcap-file; cat /tmp/mailcap-file\" \"echo %s; cat /tmp/mailcap-file\")"
    );
}
