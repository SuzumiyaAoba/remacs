use remacs_runtime::{
    editor::{CHAR_CTL, CHAR_SHIFT, event_code_for},
    frontend::{InputEvent, KeyIo, MouseButton, MouseInput, MousePosition, run_editor_with},
    lisp::Interp,
};
use std::{collections::VecDeque, io, time::Duration};

struct Frontend(VecDeque<InputEvent>);
impl KeyIo for Frontend {
    fn poll_key(&mut self, _: Duration) -> io::Result<Option<i128>> {
        Ok(None)
    }
    fn poll_input(&mut self, _: Duration) -> io::Result<Option<InputEvent>> {
        self.0.pop_front().map(Some).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "navigation test exhausted events",
            )
        })
    }
    fn unread(&mut self, _: i128) {}
    fn render(&mut self, _: &Interp) -> io::Result<()> {
        Ok(())
    }
    fn draw_echo(&mut self, _: &str) -> io::Result<()> {
        Ok(())
    }
    fn size(&self) -> (usize, usize) {
        (40, 12)
    }
}

#[test]
fn navigation_buttons_run_lisp_bindings_with_click_drag_and_modifier_data() {
    let mut i = Interp::new();
    i.eval_str(
        r#"(progn
      (erase-buffer) (insert "abcdef") (goto-char 1)
      (setq remacs-navigation-events nil)
      (defun remacs-navigation-record (event)
        (interactive "e")
        (setq remacs-navigation-events
          (append remacs-navigation-events
            (list (list (car event) (posn-point (nth 1 event))
              (if (consp (nth 2 event)) (posn-point (nth 2 event)) (nth 2 event)))))))
      (dolist (key '([down-mouse-1] [mouse-1]
                    [down-mouse-4] [mouse-4] [down-mouse-5] [mouse-5]
                    [C-S-double-down-mouse-4] [C-S-double-mouse-4]
                    [triple-down-mouse-5] [triple-mouse-5] [drag-mouse-5]
                    [wheel-up] [wheel-down]))
        (global-set-key key #'remacs-navigation-record))
      (global-set-key [f12] (lambda () (interactive) (kill-emacs))))"#,
    )
    .unwrap();
    let window = i
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
        window_id: Some(window),
        area: None,
        mode_line_buffer_name: false,
    };
    let mut events = VecDeque::new();
    for (button, click_count, modifiers) in [
        (MouseButton::Left, 1, 0),
        (MouseButton::Back, 1, 0),
        (MouseButton::Forward, 1, 0),
        (MouseButton::Back, 2, CHAR_CTL | CHAR_SHIFT),
        (MouseButton::Forward, 3, 0),
    ] {
        let mouse = MouseInput {
            button,
            click_count,
            modifiers,
            start: position,
            end: position,
        };
        events.push_back(InputEvent::MouseDown(mouse));
        events.push_back(InputEvent::Mouse(mouse));
    }
    let drag = MouseInput {
        button: MouseButton::Forward,
        click_count: 1,
        modifiers: 0,
        start: position,
        end: MousePosition {
            buffer_position: 3,
            utf16_offset: 3,
            x: 24,
            column: 3,
            ..position
        },
    };
    events.push_back(InputEvent::MouseDown(MouseInput {
        end: position,
        ..drag
    }));
    events.push_back(InputEvent::Mouse(drag));
    for up in [true, false] {
        events.push_back(InputEvent::MouseWheel {
            up,
            modifiers: 0,
            position,
            line_count: 1,
        });
    }
    events.push_back(InputEvent::Key(event_code_for("f12")));
    run_editor_with(Frontend(events), &mut i).unwrap();
    let symbol = i.intern("remacs-navigation-events");
    let actual = i
        .prin1_to_string(&i.symbol_value(symbol))
        .to_unicode_lossy();
    assert_eq!(
        actual,
        "((down-mouse-1 2 1) (mouse-1 2 1) (down-mouse-4 2 1) (mouse-4 2 1) (down-mouse-5 2 1) (mouse-5 2 1) (C-S-double-down-mouse-4 2 2) (C-S-double-mouse-4 2 2) (triple-down-mouse-5 2 3) (triple-mouse-5 2 3) (down-mouse-5 2 1) (drag-mouse-5 2 4) (wheel-up 2 nil) (wheel-down 2 nil))"
    );
}
