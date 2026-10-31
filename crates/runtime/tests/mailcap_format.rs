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

#[test]
fn mailcap_quotes_the_file_path_before_shell_expansion() {
    let mut interp = Interp::new();
    let result = interp
        .eval_str(
            r#"(progn
                 (require 'mailcap)
                 (let* ((path "/tmp/mailcap space/file")
                        (command "cat %s; cat %s")
                        (quoted (shell-quote-argument path))
                        (expected (format "cat %s; cat %s" quoted quoted))
                        captured)
                   (cl-letf (((symbol-function 'async-shell-command)
                              (lambda (command &rest _) (setq captured command)))
                             ((symbol-function 'get-buffer-process)
                              (lambda (_) 'fake-process))
                             ((symbol-function 'process-sentinel) (lambda (_) nil))
                             ((symbol-function 'set-process-sentinel)
                              (lambda (&rest _) nil)))
                     (mailcap--async-shell command path))
                   (list (equal (mailcap--format-command command path) expected)
                         (equal captured expected))))"#,
        )
        .unwrap();
    assert_eq!(interp.prin1_to_string(&result), "(t t)");
}
