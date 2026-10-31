use remacs_runtime::{frontend::DialogBackend, lisp::Interp};
use std::{cell::RefCell, rc::Rc};

#[derive(Default)]
struct Dialog {
    headers: RefCell<Vec<bool>>,
}

impl DialogBackend for Dialog {
    fn choose(
        &self,
        _: usize,
        _: &str,
        _: Option<&str>,
        _: &[String],
    ) -> Result<Option<usize>, String> {
        panic!("x-popup-dialog must pass its header");
    }

    fn choose_with_header(
        &self,
        _: usize,
        message: &str,
        detail: Option<&str>,
        buttons: &[String],
        header: bool,
    ) -> Result<Option<usize>, String> {
        assert_eq!(message, "Body");
        assert_eq!(detail, None, "a string HEADER is not dialog detail text");
        assert_eq!(buttons, ["OK"]);
        self.headers.borrow_mut().push(header);
        Ok(Some(0))
    }
}

#[test]
fn dialog_header_uses_lisp_truth_including_string_values() {
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(|| {
            let mut interp = Interp::new();
            interp
                .eval_str("(set-frame-parameter nil 'window-system 'ns)")
                .unwrap();
            let dialog = Rc::new(Dialog::default());
            interp.set_dialog_backend(Some(dialog.clone()));
            for header in [
                "",
                " nil",
                " t",
                " 0",
                " \"Notice\"",
                " \"\"",
                " 'header",
                " '(value)",
            ] {
                let source = format!(r#"(x-popup-dialog t '("Body" ("OK" . ok)){header})"#);
                let value = interp
                    .eval_str(&source)
                    .unwrap_or_else(|flow| panic!("{source}: {flow:?}"));
                assert_eq!(interp.prin1_to_string(&value), "ok");
            }
            assert_eq!(
                *dialog.headers.borrow(),
                [false, false, true, true, true, true, true, true]
            );
        })
        .unwrap()
        .join()
        .unwrap();
}
