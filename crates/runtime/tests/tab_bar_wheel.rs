use remacs_runtime::{
    editor::{CHAR_SHIFT, event_code_for},
    frontend::{
        InputEvent, KeyIo, MouseArea, MousePosition, MouseWheelDirection, run_editor_with,
    },
    lisp::Interp,
};
use std::{collections::VecDeque, io, time::Duration};

struct Frontend(VecDeque<InputEvent>);

impl KeyIo for Frontend {
    fn poll_key(&mut self, _: Duration) -> io::Result<Option<i128>> {
        Ok(None)
    }

    fn poll_input(&mut self, _: Duration) -> io::Result<Option<InputEvent>> {
        self.0
            .pop_front()
            .map(Some)
            .ok_or_else(|| io::Error::other("tab-bar wheel test exhausted input"))
    }

    fn unread(&mut self, _: i128) {}

    fn render(&mut self, _: &Interp) -> io::Result<()> {
        Ok(())
    }

    fn draw_echo(&mut self, _: &str) -> io::Result<()> {
        Ok(())
    }

    fn size(&self) -> (usize, usize) {
        (80, 16)
    }
}

#[test]
fn tab_bar_wheel_uses_tab_bar_map_with_shift_modifiers() {
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(tab_bar_wheel_uses_tab_bar_map_with_shift_modifiers_inner)
        .unwrap()
        .join()
        .unwrap();
}

fn tab_bar_wheel_uses_tab_bar_map_with_shift_modifiers_inner() {
    let mut interp = Interp::new();
    interp
        .eval_str(
            r#"(progn
              (provide 'cus-edit)
              (defmacro easy-menu-define (&rest _) nil)
              (require 'tab-bar)
              (set-frame-parameter nil 'window-system 'ns)
              (set-frame-parameter nil 'menu-bar-lines 0)
              (set-frame-parameter nil 'tool-bar-lines 0)
              (set-frame-parameter nil 'tab-bar-lines 1)
              (setq qa-tab-wheel-events nil)
              (defun qa-tab-wheel-record (event) (interactive "e")
                (setq qa-tab-wheel-events
                  (append qa-tab-wheel-events
                    (list (list (car event) (posn-area (event-start event)))))))
              (define-key tab-bar-map [wheel-up] #'qa-tab-wheel-record)
              (define-key tab-bar-map [wheel-down] #'qa-tab-wheel-record)
              (define-key tab-bar-map [S-wheel-up] #'qa-tab-wheel-record)
              (define-key tab-bar-map [S-wheel-down] #'qa-tab-wheel-record)
              (defun qa-tab-wheel-finish () (interactive)
                (setq qa-tab-wheel-result qa-tab-wheel-events)
                (kill-emacs))
              (global-set-key [f12] #'qa-tab-wheel-finish))"#,
        )
        .unwrap();
    let frame = interp.selected_frame.as_ref().unwrap().clone();
    let window = frame.borrow().selected.clone();
    let window_id = window.borrow().id;
    let position = MousePosition {
        buffer_id: window.borrow().buffer,
        buffer_position: 0,
        utf16_offset: 0,
        x: 8,
        y: 0,
        row: 0,
        column: 1,
        window_id: Some(window_id),
        area: Some(MouseArea::TabBar),
        object: None,
        timestamp: 0,
        mode_line_buffer_name: false,
    };
    let events = [
        (MouseWheelDirection::Up, 0),
        (MouseWheelDirection::Down, 0),
        (MouseWheelDirection::Up, CHAR_SHIFT),
        (MouseWheelDirection::Down, CHAR_SHIFT),
    ]
    .into_iter()
    .map(|(direction, modifiers)| InputEvent::MouseWheel {
        direction,
        modifiers,
        position,
        line_count: Some(1),
        pixel_delta: None,
    })
    .chain(std::iter::once(InputEvent::Key(event_code_for("f12"))))
    .collect();

    run_editor_with(Frontend(events), &mut interp).unwrap();
    let result = interp.intern("qa-tab-wheel-result");
    assert_eq!(
        interp.prin1_to_string(&interp.symbol_value(result)).to_unicode_lossy(),
        "((wheel-up tab-bar) (wheel-down tab-bar) (S-wheel-up tab-bar) (S-wheel-down tab-bar))"
    );
}
