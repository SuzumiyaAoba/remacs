use remacs_runtime::{
    editor::event_code_for,
    frontend::{InputEvent, KeyIo, MouseButton, MouseInput, MousePosition, run_editor_with},
    lisp::Interp,
};
use std::{collections::VecDeque, io, time::Duration};

struct Frontend(VecDeque<InputEvent>);

impl KeyIo for Frontend {
    fn poll_key(&mut self, _: Duration) -> io::Result<Option<i128>> {
        match self.0.pop_front() {
            Some(InputEvent::Key(key)) => Ok(Some(key)),
            Some(input) => {
                self.0.push_front(input);
                Ok(None)
            }
            None => Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "isearch mouse test exhausted input",
            )),
        }
    }

    fn poll_input(&mut self, _: Duration) -> io::Result<Option<InputEvent>> {
        self.next_input()
    }

    fn poll_key_or_mouse(&mut self, _: Duration) -> io::Result<Option<InputEvent>> {
        self.next_input()
    }

    fn unread(&mut self, key: i128) {
        self.0.push_front(InputEvent::Key(key));
    }

    fn render(&mut self, _: &Interp) -> io::Result<()> {
        Ok(())
    }

    fn draw_echo(&mut self, _: &str) -> io::Result<()> {
        Ok(())
    }

    fn size(&self) -> (usize, usize) {
        (80, 24)
    }
}

impl Frontend {
    fn next_input(&mut self) -> io::Result<Option<InputEvent>> {
        self.0
            .pop_front()
            .map(Some)
            .ok_or_else(|| io::Error::new(io::ErrorKind::UnexpectedEof, "input exhausted"))
    }
}

#[test]
fn middle_click_during_isearch_dispatches_the_mouse2_binding() {
    let mut i = Interp::new();
    i.eval_str(
        r#"(progn
              (require 'isearch)
              (setq mouse-yank-at-point nil)
              (setq qa-isearch-mouse2-called nil)
              (global-set-key
               [mouse-2]
               (lambda (_event)
                 (interactive "e")
                 (setq qa-isearch-mouse2-called t)))
              (global-set-key [f12]
               (lambda () (interactive) (kill-emacs)))
              (erase-buffer)
              (insert "search target\n")
              (goto-char (point-min)))"#,
    )
    .unwrap();

    let window_id = i
        .selected_frame
        .as_ref()
        .unwrap()
        .borrow()
        .selected
        .borrow()
        .id;
    let position = MousePosition {
        buffer_id: i.current_buffer,
        buffer_position: 1,
        utf16_offset: 1,
        x: 8,
        y: 0,
        row: 0,
        column: 1,
        window_id: Some(window_id),
        area: None,
        mode_line_buffer_name: false,
    };
    let mouse = MouseInput {
        button: MouseButton::Middle,
        click_count: 1,
        modifiers: 0,
        start: position,
        end: position,
    };
    let events = VecDeque::from([
        InputEvent::Key(19), // C-s starts I-search.
        InputEvent::MouseDown(mouse),
        InputEvent::Mouse(mouse),
        InputEvent::Key(13), // RET ends I-search.
        InputEvent::Key(event_code_for("f12")),
    ]);

    run_editor_with(Frontend(events), &mut i).unwrap();

    let called = i.intern("qa-isearch-mouse2-called");
    assert!(i.symbol_value(called).truthy());
}
