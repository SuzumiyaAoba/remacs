use remacs_runtime::{
    frontend::DialogBackend,
    lisp::{Interp, Value},
};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

struct Dialog {
    reply: RefCell<Result<Option<usize>, String>>,
    calls: Cell<usize>,
}

impl DialogBackend for Dialog {
    fn choose(
        &self,
        _frame_id: usize,
        message: &str,
        detail: Option<&str>,
        buttons: &[String],
    ) -> Result<Option<usize>, String> {
        assert_eq!(message, "Question");
        assert_eq!(detail, None);
        assert_eq!(buttons, ["Yes", "No"]);
        self.calls.set(self.calls.get() + 1);
        self.reply.borrow().clone()
    }
}

fn eval(interp: &mut Interp, source: &str) -> Value {
    interp
        .eval_str(source)
        .unwrap_or_else(|flow| panic!("{source}: {flow:?}"))
}

#[test]
fn dialog_dismissal_quits_but_a_selected_nil_is_a_value() {
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(|| {
            let mut interp = Interp::new();
            eval(&mut interp, "(set-frame-parameter nil 'window-system 'ns)");
            let dialog = Rc::new(Dialog {
                reply: RefCell::new(Ok(None)),
                calls: Cell::new(0),
            });
            interp.set_dialog_backend(Some(dialog.clone()));

            let result =
                interp.eval_str(r#"(x-popup-dialog t '("Question" ("Yes" . yes) ("No" . no)))"#);
            assert_eq!(dialog.calls.get(), 1, "the graphical backend was called");
            assert!(
                result.as_ref().is_err_and(|flow| flow.is_quit()),
                "dismissed dialog: {result:?}"
            );

            let caught = eval(
                &mut interp,
                r#"(progn
          (setq qa-dialog-cleanup nil)
          (condition-case err
              (unwind-protect
                  (x-popup-dialog t '("Question" ("Yes" . yes) ("No" . no)))
                (setq qa-dialog-cleanup t))
            (quit (list (car err) qa-dialog-cleanup))))"#,
            );
            assert_eq!(interp.prin1_to_string(&caught), "(quit t)");

            *dialog.reply.borrow_mut() = Ok(Some(0));
            let selected_nil = eval(
                &mut interp,
                r#"(x-popup-dialog t '("Question" ("Yes") ("No" . no)))"#,
            );
            assert!(matches!(selected_nil, Value::Nil));

            *dialog.reply.borrow_mut() = Ok(Some(1));
            let same_value = eval(
                &mut interp,
                r#"(progn
          (setq qa-dialog-value (list 'original 'value))
          (eq qa-dialog-value
              (x-popup-dialog t (list "Question" '("Yes")
                                     (cons "No" qa-dialog-value)))))"#,
            );
            assert_eq!(interp.prin1_to_string(&same_value), "t");

            *dialog.reply.borrow_mut() = Err("native failure".into());
            let error = eval(
                &mut interp,
                r#"(condition-case err
            (x-popup-dialog t '("Question" ("Yes" . yes) ("No" . no)))
          (error (error-message-string err)))"#,
            );
            assert_eq!(
                interp.prin1_to_string(&error),
                r#""Dialog failed: native failure""#
            );
            assert_eq!(dialog.calls.get(), 5);
        })
        .unwrap()
        .join()
        .unwrap();
}
