use remacs_runtime::{
    editor::{CHAR_CTL, event_code_for, reflow_frame_windows},
    frontend::{
        GridHeaderLine, InputEvent, KeyIo, MouseArea, MousePosition, MouseWheelDirection,
        render_grid_with_input_cached, run_editor_with,
    },
    lisp::{Interp, Value},
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
            .ok_or_else(|| io::Error::other("header wheel test exhausted input"))
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

fn position(header: GridHeaderLine) -> MousePosition {
    MousePosition {
        buffer_id: header.buffer_id,
        buffer_position: 0,
        utf16_offset: 0,
        x: (header.left + 2) as i128 * 8,
        y: header.row as i128 * 19,
        row: header.row,
        column: 2,
        window_id: Some(header.window_id),
        area: Some(MouseArea::HeaderLine),
        mode_line_buffer_name: false,
    }
}

#[test]
fn header_wheel_uses_prefixed_bindings_and_real_mwheel_window_selection() {
    let mut i = Interp::new();
    i.eval_str(
        r#"(progn
        (set-frame-parameter nil 'window-system 'ns)
        (set-frame-parameter nil 'menu-bar-lines 0)
        (set-frame-parameter nil 'tool-bar-lines 0)
        (set-frame-parameter nil 'tab-bar-lines 0)
        (set-frame-parameter nil 'vertical-scroll-bars nil)
        (set-frame-size nil 80 16) (set-window-fringes nil 0 0)
        (setq-default header-line-format "Header" tab-line-format "Tab" mode-line-format "Mode")
        (switch-to-buffer (get-buffer-create "*header-left*"))
        (erase-buffer) (dotimes (n 100) (insert (format "Left %d\n" n))) (goto-char 1)
        (setq qa-header-left (selected-window) qa-header-right (split-window-right 40))
        (with-current-buffer (get-buffer-create "*header-right*")
          (erase-buffer) (dotimes (n 100) (insert (format "Right %d\n" n))) (goto-char 1))
        (set-window-buffer qa-header-right "*header-right*")
        (require 'mwheel) (mouse-wheel-mode 1)
        (setq qa-header-events nil mouse-wheel-scroll-amount '(1) mouse-wheel-progressive-speed nil)
        (defun qa-header-record (event) (interactive "e")
          (setq qa-header-events (append qa-header-events
            (list (list (car event) (posn-area (event-start event))
              (eq (posn-window (event-start event)) qa-header-right)
              (event-line-count event) (aref (this-command-keys-vector) 0))))))
        (global-set-key [header-line wheel-down] #'qa-header-record)
        (global-set-key [header-line C-wheel-up] #'qa-header-record)
        (defun qa-header-default () (interactive)
          (global-set-key [header-line wheel-down] #'mwheel-scroll)
          (setq mouse-wheel-follow-mouse t))
        (defun qa-header-follow-off () (interactive)
          (setq qa-header-follow-left (window-start qa-header-left)
                qa-header-follow-right (window-start qa-header-right)
                mouse-wheel-follow-mouse nil))
        (global-set-key [f12] (lambda () (interactive)
          (setq qa-header-result (list qa-header-events
            (= qa-header-follow-left 1) (> qa-header-follow-right 1)
            (= (window-start qa-header-left) qa-header-follow-left)
            (> (window-start qa-header-right) qa-header-follow-right)
            (eq (selected-window) qa-header-left)))
          (kill-emacs))))"#,
    )
    .unwrap();
    let frame = i.selected_frame.as_ref().unwrap().clone();
    reflow_frame_windows(&frame, 80, 16);
    let left = frame.borrow().selected.borrow().id;
    let Value::Window(right) = i.eval_str("qa-header-right").unwrap() else {
        panic!("right window")
    };
    let right = right.borrow().id;
    let grid = render_grid_with_input_cached(&i, 80, 16, None);
    assert_eq!(grid.header_lines.len(), 2);
    let left = *grid
        .header_lines
        .iter()
        .find(|line| line.window_id == left)
        .unwrap();
    let right = *grid
        .header_lines
        .iter()
        .find(|line| line.window_id == right)
        .unwrap();
    assert_eq!((left.row, right.row), (1, 1));
    assert!(
        grid.input
            .as_ref()
            .unwrap()
            .hit_test_position(right.row, right.left + 2, right.left + 2)
            .is_none()
    );
    let events = [
        InputEvent::MouseWheel {
            direction: MouseWheelDirection::Down,
            modifiers: 0,
            position: position(right),
            line_count: Some(2),
            pixel_delta: None,
        },
        InputEvent::MouseWheel {
            direction: MouseWheelDirection::Up,
            modifiers: CHAR_CTL,
            position: position(left),
            line_count: Some(1),
            pixel_delta: None,
        },
        InputEvent::Command("qa-header-default".into()),
        InputEvent::MouseWheel {
            direction: MouseWheelDirection::Down,
            modifiers: 0,
            position: position(right),
            line_count: Some(3),
            pixel_delta: None,
        },
        InputEvent::Command("qa-header-follow-off".into()),
        // GNU 31.1 falls back to the event window without mouse-wheel-frame
        // redirection, including when mouse-wheel-follow-mouse is nil.
        InputEvent::MouseWheel {
            direction: MouseWheelDirection::Down,
            modifiers: 0,
            position: position(right),
            line_count: Some(2),
            pixel_delta: None,
        },
        InputEvent::Key(event_code_for("f12")),
    ];
    run_editor_with(Frontend(events.into()), &mut i).unwrap();
    let result = i.intern("qa-header-result");
    assert_eq!(
        i.prin1_to_string(&i.symbol_value(result))
            .to_unicode_lossy(),
        "(((wheel-down header-line t 2 header-line) (C-wheel-up header-line nil 1 header-line)) t t t t t)"
    );
}

#[test]
fn pixel_scroll_precision_uses_precise_wheel_delta() {
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(pixel_scroll_precision_uses_precise_wheel_delta_inner)
        .expect("spawn pixel wheel test thread")
        .join()
        .expect("pixel wheel test thread");
}

fn pixel_scroll_precision_uses_precise_wheel_delta_inner() {
    let mut i = Interp::new();
    i.eval_str(
        r#"(progn
        (set-frame-parameter nil 'window-system 'ns)
        (set-frame-parameter nil 'menu-bar-lines 0)
        (set-frame-parameter nil 'tool-bar-lines 0)
        (set-frame-parameter nil 'tab-bar-lines 0)
        (set-frame-parameter nil 'vertical-scroll-bars nil)
        (set-frame-size nil 80 16) (set-window-fringes nil 0 0)
        (setq-default header-line-format nil tab-line-format nil mode-line-format nil)
        (switch-to-buffer (get-buffer-create "*pixel-wheel*"))
        (erase-buffer) (dotimes (n 100) (insert (format "pixel line %d\n" n)))
        (goto-char 1)
        (require 'pixel-scroll)
        (pixel-scroll-precision-mode 1)
        (setq qa-pixel-scroll-event nil)
        (defun qa-pixel-scroll-observer (event)
          (setq qa-pixel-scroll-event (list (nth 3 event) (nth 4 event))))
        (advice-add 'pixel-scroll-precision :before #'qa-pixel-scroll-observer)
        (global-set-key [f12] (lambda () (interactive)
          (setq qa-pixel-scroll-result
            (list qa-pixel-scroll-event (window-vscroll nil t)))
          (kill-emacs))))"#,
    )
    .unwrap();
    let frame = i.selected_frame.as_ref().unwrap().clone();
    reflow_frame_windows(&frame, 80, 16);
    let window_id = frame.borrow().selected.borrow().id;
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
    let events = [
        InputEvent::MouseWheel {
            direction: MouseWheelDirection::Down,
            modifiers: 0,
            position,
            line_count: None,
            pixel_delta: Some((0, -5)),
        },
        InputEvent::Key(event_code_for("f12")),
    ];
    run_editor_with(Frontend(events.into()), &mut i).unwrap();
    let result = i.intern("qa-pixel-scroll-result");
    assert_eq!(
        i.prin1_to_string(&i.symbol_value(result))
            .to_unicode_lossy(),
        "((nil (0 . -5)) 5)"
    );
}
