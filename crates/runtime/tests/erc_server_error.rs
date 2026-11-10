use remacs_runtime::lisp::{Flow, Interp, Value};

fn eval(i: &mut Interp, source: &str) -> Value {
    i.eval_str(source).unwrap_or_else(|flow| match flow {
        Flow::Signal(signal) => panic!(
            "{} {}",
            i.prin1_to_string(&signal.symbol()).to_unicode_lossy(),
            i.prin1_to_string(&signal.data()).to_unicode_lossy()
        ),
        other => panic!("{other:?}"),
    })
}

#[test]
fn erc_server_command_keeps_the_connection_error_reason() {
    std::thread::Builder::new()
        .stack_size(128 << 20)
        .spawn(|| {
            let mut i = Interp::new();
            let erc_path = concat!(env!("REMACS_SOURCE_ROOT"), "/lisp/erc.el");
            let erc_path = i
                .prin1_to_string(&Value::string(erc_path))
                .to_unicode_lossy();
            let definition = format!(
                r#"(with-temp-buffer
                     (insert-file-contents {erc_path})
                     (let* ((search "(defun erc-cmd-SERVER")
                            (end (search-forward search nil t)))
                       (unless end (error "erc-cmd-SERVER not found"))
                       (goto-char (- end (length search)))
                       (eval (read (current-buffer)))))"#
            );
            eval(&mut i, &definition);
            let message = eval(
                &mut i,
                r#"(progn
                     (fset 'erc-log (lambda (&rest _args) nil))
                     (fset 'erc-current-nick (lambda () "remacs-test"))
                     (fset 'erc (lambda (&rest _args) (error "Connection refused")))
                     (fset 'erc-error
                           (lambda (format-string &rest args)
                             (setq qa-erc-server-error-message
                                   (apply #'format format-string args))))
                     (erc-cmd-SERVER "irc.example.test")
                     qa-erc-server-error-message)"#,
            );
            let Value::Str(message) = message else {
                panic!("expected an error message string")
            };
            assert_eq!(
                message.borrow().to_unicode_lossy(),
                "Could not connect to irc.example.test: Connection refused"
            );
        })
        .unwrap()
        .join()
        .unwrap();
}
