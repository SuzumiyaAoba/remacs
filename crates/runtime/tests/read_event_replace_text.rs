use remacs_runtime::{
    frontend::{InputEvent, KeyIo, run_editor_with},
    lisp::Interp,
};
use std::{collections::VecDeque, io, time::Duration};

struct InputFrontend {
    events: VecDeque<InputEvent>,
}

impl KeyIo for InputFrontend {
    fn poll_key(&mut self, _dur: Duration) -> io::Result<Option<i128>> {
        Ok(None)
    }

    fn poll_input(&mut self, _dur: Duration) -> io::Result<Option<InputEvent>> {
        self.events.pop_front().map(Some).ok_or_else(|| {
            io::Error::new(io::ErrorKind::UnexpectedEof, "read-event test exhausted its events")
        })
    }

    fn unread(&mut self, _key: i128) {}

    fn render(&mut self, _interp: &Interp) -> io::Result<()> {
        Ok(())
    }

    fn draw_echo(&mut self, _text: &str) -> io::Result<()> {
        Ok(())
    }

    fn size(&self) -> (usize, usize) {
        (80, 25)
    }
}

#[test]
fn read_event_and_read_char_skip_empty_replace_text() {
    let mut interp = Interp::new();
    interp
        .eval_str(
            "(progn (erase-buffer) (insert \"selected\") (goto-char (point-min)) (set-mark (point)) (goto-char (point-max)) (setq transient-mark-mode t mark-active t) (global-set-key [f8] (lambda () (interactive) (setq qa-read-event-result (read-event)) (setq qa-read-char-result (read-char)) (kill-emacs))) (global-set-key [f12] (lambda () (interactive) (kill-emacs))))",
        )
        .unwrap();
    let buffer_id = interp.current_buffer_ref().unwrap().borrow().id;
    let replace = |text: &str| InputEvent::ReplaceText {
        range: 0..8,
        text: text.to_owned(),
    };
    let frontend = InputFrontend {
        events: VecDeque::from([
            InputEvent::Key(remacs_runtime::editor::event_code_for("f8")),
            replace(""),
            replace("x"),
            replace(""),
            replace("y"),
            InputEvent::Key(remacs_runtime::editor::event_code_for("f12")),
            InputEvent::Key(remacs_runtime::editor::event_code_for("f12")),
        ]),
    };
    run_editor_with(frontend, &mut interp).unwrap();

    let read_event_id = interp.intern("qa-read-event-result");
    let read_event = interp.symbol_value(read_event_id);
    assert_eq!(interp.prin1_to_string(&read_event), "120");
    let read_char_id = interp.intern("qa-read-char-result");
    let read_char = interp.symbol_value(read_char_id);
    assert_eq!(interp.prin1_to_string(&read_char), "121");
    let text = interp.buffers.get(buffer_id).unwrap().borrow().text.text();
    assert_eq!(format!("{text}"), "selected");
}
