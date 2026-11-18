use remacs_runtime::lisp::{Flow, Interp, Value};

#[test]
fn dbus_validation_errors_precede_any_session_connection_attempt() {
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(|| {
            let mut interp = Interp::new();
            if interp.eval_str("(featurep 'dbusbind)").unwrap().is_nil() {
                return;
            }

            for (form, expected) in [
                ("(dbus--init-bus :bogus)", "dbus-error"),
                ("(dbus-get-unique-name :bogus)", "dbus-error"),
                (
                    "(dbus-message-internal 99 :session \"x\")",
                    "dbus-error",
                ),
                (
                    "(dbus-message-internal 1 :session)",
                    "wrong-number-of-arguments",
                ),
            ] {
                let Err(Flow::Signal(signal)) = interp.eval_str(form) else {
                    panic!("{form} did not signal {expected}");
                };
                let Value::Sym(symbol) = signal.symbol() else {
                    panic!("{form} signaled with a non-symbol condition");
                };
                assert_eq!(interp.symbol_name(symbol), expected, "{form}");
            }
        })
        .unwrap()
        .join()
        .unwrap();
}
