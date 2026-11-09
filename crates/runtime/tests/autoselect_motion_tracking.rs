use remacs_runtime::{
    editor::event_code_for,
    frontend::{InputEvent, KeyIo, run_editor_with},
    lisp::Interp,
};
use std::{cell::Cell, collections::VecDeque, io, rc::Rc, time::Duration};

struct Frontend {
    events: VecDeque<(InputEvent, i128)>,
    counter: u32,
    expected: i128,
    observations: Rc<Cell<usize>>,
}
impl KeyIo for Frontend {
    fn poll_key(&mut self, _: Duration) -> io::Result<Option<i128>> {
        Ok(None)
    }
    fn poll_input(&mut self, _: Duration) -> io::Result<Option<InputEvent>> {
        let (event, expected) = self.events.pop_front().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "tracking test exhausted events",
            )
        })?;
        self.expected = expected;
        Ok(Some(event))
    }
    fn unread(&mut self, _: i128) {}
    fn draw_echo(&mut self, _: &str) -> io::Result<()> {
        Ok(())
    }
    fn size(&self) -> (usize, usize) {
        (40, 16)
    }
    fn render(&mut self, i: &Interp) -> io::Result<()> {
        assert_eq!(
            i.symbol_value(self.counter).int(),
            Some(self.expected),
            "motion count after event {}",
            self.observations.get()
        );
        self.observations.set(self.observations.get() + 1);
        Ok(())
    }
}

#[test]
fn window_autoselection_does_not_enable_lisp_motion_tracking() {
    let mut i = Interp::new();
    i.eval_str(r#"(progn
      (setq mouse-autoselect-window t track-mouse nil remacs-motion-count 0)
      (global-set-key [mouse-movement] (lambda () (interactive) (setq remacs-motion-count (1+ remacs-motion-count))))
      (defun remacs-track-on () (interactive) (setq track-mouse t))
      (defun remacs-track-off () (interactive) (setq track-mouse nil mouse-autoselect-window 0.5))
      (defun remacs-track-dragging () (interactive) (setq track-mouse 'dragging mouse-autoselect-window nil))
      (global-set-key [f12] (lambda () (interactive) (kill-emacs))))"#).unwrap();
    let frame = i.selected_frame.as_ref().unwrap().borrow();
    let frame_id = frame.id;
    let window = frame.selected.borrow().id;
    drop(frame);
    let motion = |x| InputEvent::MouseMoved {
        frame_id: Some(frame_id),
        text_window_id: Some(window),
        frame_position: Some((x, 0)),
        frame_pixel_position: None,
        position: None,
        modifiers: 0,
    };
    let events = VecDeque::from([
        (motion(0), 0),
        (motion(1), 0),
        (InputEvent::Command("remacs-track-on".into()), 0),
        (motion(2), 1),
        (motion(3), 2),
        (InputEvent::Command("remacs-track-off".into()), 2),
        (motion(4), 2),
        (InputEvent::Command("remacs-track-dragging".into()), 2),
        (motion(5), 3),
        (InputEvent::Key(event_code_for("f12")), 3),
    ]);
    let counter = i.intern("remacs-motion-count");
    let observations = Rc::new(Cell::new(0));
    run_editor_with(
        Frontend {
            events,
            counter,
            expected: 0,
            observations: observations.clone(),
        },
        &mut i,
    )
    .unwrap();
    assert_eq!(i.symbol_value(counter).int(), Some(3));
    assert!(observations.get() >= 9);
}
