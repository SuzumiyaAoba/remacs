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
}
impl DialogBackend for Dialog {
    fn choose(
        &self,
        _: usize,
        _: &str,
        _: Option<&str>,
        _: &[String],
    ) -> Result<Option<usize>, String> {
        panic!("group information must be passed");
    }
    fn choose_with_items(
        &self,
        _: usize,
        _: &str,
        _: Option<&str>,
        items: &[DialogItem],
        _: bool,
    ) -> Result<Option<usize>, String> {
        *self.items.borrow_mut() = items.to_vec();
        Ok(self.selected.get())
    }
}
fn eval(interp: &mut Interp, source: &str) -> Value {
    interp
        .eval_str(source)
        .unwrap_or_else(|flow| panic!("{source}: {flow:?}"))
}
#[test]
fn dialog_boundaries_preserve_order_without_consuming_lisp_values() {
    std::thread::Builder::new().stack_size(64*1024*1024).spawn(|| {
        let mut interp = Interp::new();
        eval(&mut interp, "(set-frame-parameter nil 'window-system 'ns)");
        let dialog = Rc::new(Dialog::default());
        dialog.selected.set(Some(1));
        interp.set_dialog_backend(Some(dialog.clone()));
        let value = eval(&mut interp, r#"(x-popup-dialog t '("Save changes?" ("Save" . save) nil ("Discard" . discard) ("Cancel" . cancel)))"#);
        assert_eq!(interp.prin1_to_string(&value), "discard");
        assert_eq!(*dialog.items.borrow(), [DialogItem::Button("Save".into()),DialogItem::Separator,DialogItem::Button("Discard".into()),DialogItem::Button("Cancel".into())]);
        dialog.selected.set(Some(0));
        let identity = eval(&mut interp, r#"(progn
          (setq qa-dialog-group-value (list 'original))
          (eq qa-dialog-group-value
              (x-popup-dialog t (list "Groups" "Left text" (cons "Left" qa-dialog-group-value)
                                     nil "Right text" '("Right" . right) nil '("Last" . last)))))"#);
        assert_eq!(interp.prin1_to_string(&identity), "t");
        assert_eq!(*dialog.items.borrow(), [DialogItem::Text("Left text".into()),DialogItem::Button("Left".into()),DialogItem::Separator,DialogItem::Text("Right text".into()),DialogItem::Button("Right".into()),DialogItem::Separator,DialogItem::Button("Last".into())]);
        let nil_value = eval(&mut interp, r#"(x-popup-dialog t '("Groups" nil ("Nil value") nil))"#);
        assert!(matches!(nil_value, Value::Nil));
        assert_eq!(*dialog.items.borrow(),[DialogItem::Separator,DialogItem::Button("Nil value".into()),DialogItem::Separator]);
        eval(&mut interp, r#"(x-popup-dialog t '("Groups" ("Left" . left) boundary ("Right" . right)))"#);
        assert_eq!(*dialog.items.borrow(),[DialogItem::Button("Left".into()),DialogItem::Separator,DialogItem::Button("Right".into())]);
    }).unwrap().join().unwrap();
}
