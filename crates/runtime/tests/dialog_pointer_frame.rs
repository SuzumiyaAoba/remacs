use remacs_runtime::{
    frontend::DialogBackend,
    lisp::{Interp, Value},
};
use std::{cell::Cell, rc::Rc};

#[derive(Default)]
struct Dialog {
    parent: Cell<Option<usize>>,
}

impl DialogBackend for Dialog {
    fn choose(
        &self,
        frame_id: usize,
        message: &str,
        detail: Option<&str>,
        buttons: &[String],
    ) -> Result<Option<usize>, String> {
        assert_eq!(message, "Question");
        assert_eq!(detail, None);
        assert_eq!(buttons, ["OK"]);
        self.parent.set(Some(frame_id));
        Ok(Some(0))
    }
}

fn eval(interp: &mut Interp, source: &str) -> Value {
    interp
        .eval_str(source)
        .unwrap_or_else(|flow| panic!("{source}: {flow:?}"))
}

fn frame_id(interp: &mut Interp, source: &str) -> usize {
    let Value::Frame(frame) = eval(interp, source) else {
        panic!("{source} must return a frame");
    };
    frame.borrow().id
}

fn popup(interp: &mut Interp, dialog: &Dialog, position: &str, parent: usize, selected: usize) {
    dialog.parent.set(None);
    let value = eval(
        interp,
        &format!(r#"(x-popup-dialog {position} '("Question" ("OK" . ok)))"#),
    );
    assert_eq!(interp.prin1_to_string(&value), "ok");
    assert_eq!(dialog.parent.get(), Some(parent), "position: {position}");
    assert_eq!(frame_id(interp, "(selected-frame)"), selected);
}

#[test]
fn dialog_uses_pointer_frame_without_changing_selection() {
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(|| {
            let mut interp = Interp::new();
            eval(
                &mut interp,
                "(progn
                   (set-frame-parameter nil 'window-system 'ns)
                   (setq qa-dialog-selected (selected-frame)
                         qa-dialog-other (make-frame '((window-system . ns))))
                   (select-frame qa-dialog-selected))",
            );
            let selected = frame_id(&mut interp, "qa-dialog-selected");
            let other = frame_id(&mut interp, "qa-dialog-other");
            assert_ne!(selected, other);
            let dialog = Rc::new(Dialog::default());
            interp.set_dialog_backend(Some(dialog.clone()));

            interp.gui_mouse_frame_position = Some((other, 4, 5));
            interp.gui_mouse_frame_pixel_position = Some((other, 40, 50));
            popup(&mut interp, &dialog, "t", other, selected);
            // Explicit positions and menu-bar events keep their own parent.
            for position in [
                "qa-dialog-selected",
                "(frame-selected-window qa-dialog-selected)",
                "'(menu-bar)",
                "'(tab-bar)",
                "'(tool-bar)",
            ] {
                popup(&mut interp, &dialog, position, selected, selected);
            }
            popup(&mut interp, &dialog, "qa-dialog-other", other, selected);

            // A backend may supply just character or just pixel coordinates.
            interp.gui_mouse_frame_pixel_position = None;
            popup(&mut interp, &dialog, "t", other, selected);
            interp.gui_mouse_frame_position = None;
            interp.gui_mouse_frame_pixel_position = Some((other, 40, 50));
            popup(&mut interp, &dialog, "t", other, selected);

            // Missing, unknown, and deleted pointer frames fall back safely.
            interp.gui_mouse_frame_pixel_position = None;
            popup(&mut interp, &dialog, "t", selected, selected);
            interp.gui_mouse_frame_pixel_position = Some((usize::MAX, 40, 50));
            popup(&mut interp, &dialog, "t", selected, selected);
            eval(&mut interp, "(delete-frame qa-dialog-other t)");
            interp.gui_mouse_frame_position = Some((other, 4, 5));
            interp.gui_mouse_frame_pixel_position = Some((other, 40, 50));
            popup(&mut interp, &dialog, "t", selected, selected);
        })
        .unwrap()
        .join()
        .unwrap();
}
