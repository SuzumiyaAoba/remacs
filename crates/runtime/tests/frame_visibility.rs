use remacs_runtime::{
    frontend::{FrameVisibility, FrameWindowBackend},
    lisp::Interp,
};
use std::{cell::RefCell, rc::Rc};

#[derive(Default)]
struct WindowRecorder(RefCell<Vec<(usize, FrameVisibility)>>);

impl FrameWindowBackend for WindowRecorder {
    fn set_visible(&self, frame: usize, visible: bool) -> Result<(), String> {
        self.set_visibility(
            frame,
            if visible {
                FrameVisibility::Visible
            } else {
                FrameVisibility::Iconified
            },
        )
    }

    fn set_visibility(&self, frame: usize, visibility: FrameVisibility) -> Result<(), String> {
        self.0.borrow_mut().push((frame, visibility));
        Ok(())
    }

    fn raise(&self, _: usize) -> Result<(), String> {
        Ok(())
    }
    fn lower(&self, _: usize) -> Result<(), String> {
        Ok(())
    }
    fn warp_pointer_in_frame(&self, _: usize, _: i128, _: i128) -> Result<(), String> {
        Ok(())
    }
    fn warp_pointer_absolute(&self, _: usize, _: i128, _: i128) -> Result<(), String> {
        Ok(())
    }
}

#[test]
fn parameter_setters_send_distinct_native_visibility_requests() {
    let mut interp = Interp::new();
    let backend = Rc::new(WindowRecorder::default());
    interp.set_frame_window_backend(Some(backend.clone()));
    let tty = interp
        .eval_str(
            "(progn (set-frame-parameter nil 'visibility nil) (frame-visible-p (selected-frame)))",
        )
        .unwrap();
    assert_eq!(interp.prin1_to_string(&tty), "t");
    assert!(backend.0.borrow().is_empty());

    interp
        .eval_str("(set-frame-parameter nil 'window-system 'ns)")
        .unwrap();
    let values = interp
        .eval_str(
            "(let ((frame (selected-frame)))
           (list
            (progn (set-frame-parameter frame 'visibility nil) (frame-visible-p frame))
            (progn (modify-frame-parameters frame '((visibility . icon))) (frame-visible-p frame))
            (progn (set-frame-parameter frame 'visibility t) (frame-visible-p frame))
            (progn (set-frame-parameter frame 'visibility 'other) (frame-visible-p frame))
            (progn (set-frame-parameter frame 'visibility (make-symbol \"icon\")) (frame-visible-p frame))
            (progn (modify-frame-parameters frame '((visibility) (visibility . icon)))
                   (frame-visible-p frame))))",
        )
        .unwrap();
    assert_eq!(interp.prin1_to_string(&values), "(nil icon t t t nil)");
    let states: Vec<_> = backend.0.borrow().iter().map(|(_, state)| *state).collect();
    assert_eq!(
        states,
        vec![
            FrameVisibility::Hidden,
            FrameVisibility::Iconified,
            FrameVisibility::Visible,
            FrameVisibility::Visible,
            FrameVisibility::Visible,
            FrameVisibility::Iconified,
            FrameVisibility::Hidden,
        ]
    );

    // An invalid alist must be rejected before issuing any native request.
    assert!(
        interp
            .eval_str("(modify-frame-parameters nil '((visibility . t) invalid-element))",)
            .is_err()
    );
    assert_eq!(backend.0.borrow().len(), states.len());
    let visible = interp
        .eval_str("(frame-visible-p (selected-frame))")
        .unwrap();
    assert!(visible.is_nil());

    backend.0.borrow_mut().clear();
    interp
        .eval_str(
            "(let ((frame (selected-frame)))
               (iconify-frame frame)
               (make-frame-visible frame)
               (make-frame-invisible frame t))",
        )
        .unwrap();
    assert_eq!(
        backend
            .0
            .borrow()
            .iter()
            .map(|(_, state)| *state)
            .collect::<Vec<_>>(),
        vec![
            FrameVisibility::Iconified,
            FrameVisibility::Visible,
            FrameVisibility::Hidden
        ],
    );
}
