use remacs_runtime::{
    frontend::{DialogBackend, DialogItem},
    lisp::{Interp, Value},
};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

#[derive(Default)]
struct Dialog {
    items: RefCell<Vec<DialogItem>>,
    selected: Cell<Option<usize>>,
    header: Cell<bool>,
}
impl DialogBackend for Dialog {
    fn choose(
        &self,
        _: usize,
        _: &str,
        _: Option<&str>,
        _: &[String],
    ) -> Result<Option<usize>, String> {
        panic!("structured items must reach the GUI backend");
    }
    fn choose_with_items(
        &self,
        _: usize,
        message: &str,
        detail: Option<&str>,
        items: &[DialogItem],
        header: bool,
    ) -> Result<Option<usize>, String> {
        assert_eq!(message, "Choose one");
        assert_eq!(detail, None);
        *self.items.borrow_mut() = items.to_vec();
        self.header.set(header);
        Ok(self.selected.get())
    }
}
fn eval(interp: &mut Interp, source: &str) -> Value {
    interp
        .eval_str(source)
        .unwrap_or_else(|flow| panic!("{source}: {flow:?}"))
}

#[test]
fn dialog_text_is_preserved_without_changing_selectable_values() {
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(|| {
            let mut interp = Interp::new();
            eval(&mut interp, "(set-frame-parameter nil 'window-system 'ns)");
            let dialog = Rc::new(Dialog::default());
            dialog.selected.set(Some(0));
            interp.set_dialog_backend(Some(dialog.clone()));
            let value = eval(
                &mut interp,
                r#"(x-popup-dialog t '("Choose one" "Select an operation:" ("OK" . ok)))"#,
            );
            assert_eq!(interp.prin1_to_string(&value), "ok");
            assert_eq!(
                *dialog.items.borrow(),
                [
                    DialogItem::Text("Select an operation:".into()),
                    DialogItem::Button("OK".into())
                ]
            );
            assert!(!dialog.header.get());

            dialog.selected.set(Some(1));
            let identity = eval(
                &mut interp,
                r#"(progn
          (setq qa-dialog-item-value (list 'original))
          (eq qa-dialog-item-value
              (x-popup-dialog t (list "Choose one" "Before" '("One" . one) "Between"
                                     (cons "Two" qa-dialog-item-value) "After") t)))"#,
            );
            assert_eq!(interp.prin1_to_string(&identity), "t");
            assert_eq!(
                *dialog.items.borrow(),
                [
                    DialogItem::Text("Before".into()),
                    DialogItem::Button("One".into()),
                    DialogItem::Text("Between".into()),
                    DialogItem::Button("Two".into()),
                    DialogItem::Text("After".into())
                ]
            );
            assert!(dialog.header.get());

            dialog.selected.set(None);
            let cancelled = eval(
                &mut interp,
                r#"(condition-case nil
            (x-popup-dialog t '("Choose one" "Only information"))
          (quit 'quit))"#,
            );
            assert_eq!(interp.prin1_to_string(&cancelled), "quit");
            assert_eq!(
                *dialog.items.borrow(),
                [DialogItem::Text("Only information".into())]
            );
        })
        .unwrap()
        .join()
        .unwrap();
}
