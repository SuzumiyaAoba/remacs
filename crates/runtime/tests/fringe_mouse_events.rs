use remacs_runtime::{
    frontend::{InputEvent, KeyIo, MouseArea, MouseButton, MouseInput, MousePosition, run_editor_with},
    lisp::Interp,
};
use std::{collections::VecDeque, io, time::Duration};

struct FringeFrontend {
    events: VecDeque<InputEvent>,
}

impl KeyIo for FringeFrontend {
    fn poll_key(&mut self, _: Duration) -> io::Result<Option<i128>> {
        Ok(None)
    }

    fn poll_input(&mut self, _: Duration) -> io::Result<Option<InputEvent>> {
        self.events.pop_front().map(Some).ok_or_else(|| {
            io::Error::new(io::ErrorKind::UnexpectedEof, "fringe input exhausted")
        })
    }

    fn unread(&mut self, _: i128) {}

    fn draw_echo(&mut self, _: &str) -> io::Result<()> {
        Ok(())
    }

    fn size(&self) -> (usize, usize) {
        (40, 12)
    }

    fn render(&mut self, _: &Interp) -> io::Result<()> {
        Ok(())
    }
}

#[test]
fn fringe_click_area_selects_fringe_mouse_binding_and_posn_area() {
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(|| {
            let mut i = Interp::new();
            i.eval_str(
                r#"(progn
                  (setq qa-fringe-area nil)
                  (global-set-key [left-fringe mouse-1]
                    (lambda (event) (interactive "e")
                      (setq qa-fringe-area
                            (posn-area (event-start event)))))
                  t)"#,
            )
            .unwrap();
            let frame = i.selected_frame.as_ref().unwrap().borrow();
            let window_id = frame.selected.borrow().id;
            let buffer_id = i.current_buffer;
            drop(frame);
            let position = MousePosition {
                buffer_id,
                buffer_position: 0,
                utf16_offset: 0,
                x: 0,
                y: 0,
                row: 0,
                column: 0,
                window_id: Some(window_id),
                area: Some(MouseArea::LeftFringe),
                object: None,
                timestamp: 0,
                mode_line_buffer_name: false,
            };
            let input_error = run_editor_with(
                FringeFrontend {
                    events: VecDeque::from([InputEvent::Mouse(MouseInput {
                        button: MouseButton::Left,
                        click_count: 1,
                        modifiers: 0,
                        start: position,
                        end: position,
                    })]),
                },
                &mut i,
            )
            .unwrap_err();
            assert_eq!(input_error.kind(), io::ErrorKind::UnexpectedEof);
            let area_symbol = i.intern("qa-fringe-area");
            assert_eq!(
                i.prin1_to_string(&i.symbol_value(area_symbol))
                    .to_unicode_lossy(),
                "left-fringe"
            );
        })
        .unwrap()
        .join()
        .unwrap();
}
