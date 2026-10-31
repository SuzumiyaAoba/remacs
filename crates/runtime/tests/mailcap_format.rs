use remacs_runtime::lisp::{Flow, Interp};

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
        .unwrap_or_else(|flow| match flow {
            Flow::Signal(signal) => panic!(
                "{} {}",
                interp.prin1_to_string(&signal.symbol()),
                interp.prin1_to_string(&signal.data())
            ),
            other => panic!("{other:?}"),
        });
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

#[test]
fn mailcap_quotes_untrusted_content_type_in_test_and_viewer_commands() {
    let mut interp = Interp::new();
    let result = interp
        .eval_str(
            r#"(progn
                 (require 'mailcap)
                 (require 'mail-parse)
                 (let* ((content-type
                         (concat "application/x-" (string 96) "id" (string 96)))
                        (parsed (mail-header-parse-content-type content-type))
                        (type-info (cons (cons 'type (car parsed)) (cdr parsed)))
                        (mailcap-viewer-test-cache nil)
                        captured)
                   (cl-letf (((symbol-function 'call-process)
                              (lambda (&rest args) (setq captured args) 0)))
                     (mailcap-viewer-passes-test
                      '((viewer . "cat") (test . "test %t")) type-info))
                   (list
                    (equal (car parsed) content-type)
                    (equal (nth 5 captured)
                           (format "test %s" (shell-quote-argument content-type)))
                    (equal (mailcap--format-command
                            (mailcap-unescape-mime-test "viewer %t %s" type-info)
                            "/tmp/mailcap-file")
                           (format "viewer %s %s"
                                   (shell-quote-argument content-type)
                                   (shell-quote-argument "/tmp/mailcap-file")))
                    (equal (mailcap-unescape-mime-test "test %t" nil)
                           (format "test %s" (shell-quote-argument ""))))))"#,
        )
        .unwrap_or_else(|flow| match flow {
            Flow::Signal(signal) => panic!(
                "{} {}",
                interp.prin1_to_string(&signal.symbol()),
                interp.prin1_to_string(&signal.data())
            ),
            other => panic!("{other:?}"),
        });
    assert_eq!(interp.prin1_to_string(&result), "(t t t t)");
}
