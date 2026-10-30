use remacs_runtime::{
    editor::{Frame, Window},
    frontend::{
        InputEvent, KeyIo, MousePosition, render_frame_grid_with_input_cached,
        MouseButton, MouseInput, render_grid_with_input_cached, run_editor_with,
    },
    lisp::Interp,
};
use std::{collections::VecDeque, io, time::Duration};

struct DropFrontend {
    events: VecDeque<InputEvent>,
}

struct TempFile(std::path::PathBuf);

impl Drop for TempFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

impl KeyIo for DropFrontend {
    fn poll_key(&mut self, _dur: Duration) -> io::Result<Option<i128>> {
        Ok(None)
    }

    fn poll_input(&mut self, _dur: Duration) -> io::Result<Option<InputEvent>> {
        self.events.pop_front().map(Some).ok_or_else(|| {
            io::Error::new(io::ErrorKind::UnexpectedEof, "drop test exhausted its events")
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
fn nonselected_frame_grid_maps_input_to_its_selected_window() {
    let mut interp = Interp::new();
    let active_buffer = interp.create_buffer("active-frame-buffer");
    interp
        .buffers
        .get(active_buffer)
        .unwrap()
        .borrow_mut()
        .insert("current buffer\n");
    let active_minibuffer = interp.create_buffer(" *Minibuf-0*");
    let active_frame = Frame::new_tty(active_buffer, active_minibuffer, 80, 25);
    interp.selected_frame = Some(active_frame.clone());
    interp.frames.push(active_frame);
    interp.set_current_buffer(active_buffer);

    let target_buffer = interp.create_buffer("other-frame-buffer");
    interp
        .buffers
        .get(target_buffer)
        .unwrap()
        .borrow_mut()
        .insert("target buffer\n");
    let target_minibuffer = interp.create_buffer(" *Minibuf-1*");
    let target_frame = Frame::new_tty(target_buffer, target_minibuffer, 80, 25);
    target_frame.borrow().selected.borrow_mut().point = 7;
    interp.frames.push(target_frame.clone());

    let grid = render_frame_grid_with_input_cached(&interp, &target_frame, 80, 24, None);
    let input = grid
        .input
        .as_ref()
        .expect("nonselected frame should expose an input map for hit-testing");
    assert_eq!(input.buffer_id, target_buffer);
    assert_eq!(input.text.as_ref(), "target buffer\n");
    assert_eq!(input.cursor, 7);
    assert!(!input.rows.is_empty());
    assert_eq!(input.rows[0].buffer_start, 0);
    assert_eq!(input.rows[0].source_offsets.first(), Some(&0));
}

#[test]
fn split_window_hit_test_maps_to_the_nonselected_buffer_and_window() {
    let mut interp = Interp::new();
    let left_buffer = interp.create_buffer("left-frame-buffer");
    interp
        .buffers
        .get(left_buffer)
        .unwrap()
        .borrow_mut()
        .insert("left side\n");
    let minibuffer = interp.create_buffer(" *Minibuf-0*");
    let frame = Frame::new_tty(left_buffer, minibuffer, 80, 25);
    let left_window = frame.borrow().selected.clone();
    left_window.borrow_mut().width = 40;
    interp.selected_frame = Some(frame.clone());
    interp.frames.push(frame.clone());
    interp.set_current_buffer(left_buffer);

    let right_buffer = interp.create_buffer("right-frame-buffer");
    interp
        .buffers
        .get(right_buffer)
        .unwrap()
        .borrow_mut()
        .insert("target text\n");
    let right_window = Window::new(right_buffer);
    let root = Window::new(left_buffer);
    {
        let mut right = right_window.borrow_mut();
        right.left = 40;
        right.top = 0;
        right.width = 40;
        right.height = 24;
    }
    {
        let mut root = root.borrow_mut();
        root.horiz = Some(true);
        root.width = 80;
        root.height = 24;
        root.children = vec![left_window.clone(), right_window.clone()];
    }
    left_window.borrow_mut().parent = Some(std::rc::Rc::downgrade(&root));
    right_window.borrow_mut().parent = Some(std::rc::Rc::downgrade(&root));
    {
        let mut frame = frame.borrow_mut();
        frame.root = Some(root);
        frame.windows.push(right_window.clone());
    }

    let grid = render_grid_with_input_cached(&interp, 80, 24, None);
    let input = grid.input.as_ref().unwrap();
    let position = input
        .hit_test_position(0, 45, 45)
        .expect("pointer over the right split should map to its buffer text");
    assert_eq!(position.buffer_id, right_buffer);
    assert_eq!(position.buffer_position, 5);
    assert_eq!(position.utf16_offset, 5);
    assert_eq!(position.window_id, Some(right_window.borrow().id));
}

#[test]
fn same_buffer_hit_test_identifies_the_selected_split_window() {
    let mut interp = Interp::new();
    let buffer = interp.create_buffer("shared-split-buffer");
    interp
        .buffers
        .get(buffer)
        .unwrap()
        .borrow_mut()
        .insert("shared buffer text\n");
    let minibuffer = interp.create_buffer(" *Minibuf-0*");
    let frame = Frame::new_tty(buffer, minibuffer, 80, 25);
    let left_window = frame.borrow().selected.clone();
    left_window.borrow_mut().width = 40;
    let right_window = Window::new(buffer);
    {
        let mut right = right_window.borrow_mut();
        right.left = 40;
        right.top = 0;
        right.width = 40;
        right.height = 24;
    }
    let root = Window::new(buffer);
    {
        let mut root = root.borrow_mut();
        root.horiz = Some(true);
        root.width = 80;
        root.height = 24;
        root.children = vec![left_window.clone(), right_window.clone()];
    }
    left_window.borrow_mut().parent = Some(std::rc::Rc::downgrade(&root));
    right_window.borrow_mut().parent = Some(std::rc::Rc::downgrade(&root));
    {
        let mut frame = frame.borrow_mut();
        frame.root = Some(root);
        frame.windows.push(right_window.clone());
        frame.selected = right_window.clone();
    }
    interp.selected_frame = Some(frame.clone());
    interp.frames.push(frame);
    interp.set_current_buffer(buffer);

    let grid = render_grid_with_input_cached(&interp, 80, 24, None);
    let position = grid
        .input
        .as_ref()
        .unwrap()
        .hit_test_position(0, 45, 45)
        .expect("pointer over the selected right split should map to its window");
    assert_eq!(position.buffer_id, buffer);
    assert_eq!(position.window_id, None);

    interp
        .eval_str(
            "(global-set-key [mouse-1] (lambda (event) (interactive \"e\") (setq qa-same-buffer-posn-window (eq (posn-window (event-start event)) (selected-window)))))",
        )
        .unwrap();
    interp
        .eval_str("(global-set-key [f12] (lambda () (interactive) (kill-emacs)))")
        .unwrap();
    let mouse_position = MousePosition {
        buffer_id: position.buffer_id,
        buffer_position: position.buffer_position,
        utf16_offset: position.utf16_offset,
        x: 364,
        y: 10,
        row: 0,
        column: position.column,
        window_id: position.window_id,
        area: None,
        mode_line_buffer_name: false,
    };
    let frontend = DropFrontend {
        events: VecDeque::from([
            InputEvent::Mouse(MouseInput {
                button: MouseButton::Left,
                click_count: 1,
                modifiers: 0,
                start: mouse_position,
                end: mouse_position,
            }),
            InputEvent::Key(remacs_runtime::editor::event_code_for("f12")),
        ]),
    };
    run_editor_with(frontend, &mut interp).unwrap();
    let same_window_symbol = interp.intern("qa-same-buffer-posn-window");
    let same_window = interp.symbol_value(same_window_symbol);
    assert_eq!(interp.prin1_to_string(&same_window), "t");
}

#[test]
fn split_window_drops_use_the_target_buffer_and_window() {
    let file_path = std::env::temp_dir().join(format!(
        "remacs-split-drop-{}-{}.txt",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::write(&file_path, "file contents\n").unwrap();
    let _temp_file = TempFile(file_path.clone());

    let mut interp = Interp::new();
    let left_buffer = interp.create_buffer("left-frame-buffer");
    interp
        .buffers
        .get(left_buffer)
        .unwrap()
        .borrow_mut()
        .insert("left side\n");
    let minibuffer = interp.create_buffer(" *Minibuf-0*");
    let frame = Frame::new_tty(left_buffer, minibuffer, 80, 25);
    let left_window = frame.borrow().selected.clone();
    left_window.borrow_mut().width = 40;
    interp.selected_frame = Some(frame.clone());
    interp.frames.push(frame.clone());
    interp.set_current_buffer(left_buffer);

    let right_buffer = interp.create_buffer("right-frame-buffer");
    interp
        .buffers
        .get(right_buffer)
        .unwrap()
        .borrow_mut()
        .insert("target buffer\n");
    let right_window = Window::new(right_buffer);
    let root = Window::new(left_buffer);
    {
        let mut right = right_window.borrow_mut();
        right.left = 40;
        right.top = 0;
        right.width = 40;
        right.height = 24;
    }
    {
        let mut root = root.borrow_mut();
        root.horiz = Some(true);
        root.width = 80;
        root.height = 24;
        root.children = vec![left_window.clone(), right_window.clone()];
    }
    left_window.borrow_mut().parent = Some(std::rc::Rc::downgrade(&root));
    right_window.borrow_mut().parent = Some(std::rc::Rc::downgrade(&root));
    {
        let mut frame = frame.borrow_mut();
        frame.root = Some(root);
        frame.windows.push(right_window.clone());
    }
    interp
        .eval_str(
            "(global-set-key [f12] (lambda () (interactive) (setq qa-split-drop-color-changed (equal (face-attribute 'qa-split-drop-face :foreground nil) \"#ff0000\")) (kill-emacs)))",
        )
        .unwrap();
    interp
        .eval_str(
            "(progn (make-face 'qa-split-drop-face) (with-current-buffer \"right-frame-buffer\" (put-text-property 1 2 'face 'qa-split-drop-face)))",
        )
        .unwrap();

    let drop_position = MousePosition {
        buffer_id: right_buffer,
        buffer_position: 7,
        utf16_offset: 7,
        x: 364,
        y: 10,
        row: 0,
        column: 7,
        window_id: Some(right_window.borrow().id),
        area: None,
        mode_line_buffer_name: false,
    };
    let frontend = DropFrontend {
        events: VecDeque::from([
            InputEvent::ColorDrop {
                position: Some(MousePosition {
                    buffer_id: right_buffer,
                    buffer_position: 0,
                    utf16_offset: 0,
                    x: 4,
                    y: 10,
                    row: 0,
                    column: 0,
                    window_id: Some(right_window.borrow().id),
                    area: None,
                    mode_line_buffer_name: false,
                }),
                color: "ff0000".to_owned(),
                background: false,
            },
            InputEvent::Drop {
                position: Some(drop_position),
                urls: vec!["DROPPED".to_owned()],
                action: "copy".to_owned(),
            },
            InputEvent::Drop {
                position: Some(drop_position),
                urls: vec![format!("file://{}", file_path.display())],
                action: "copy".to_owned(),
            },
            InputEvent::Key(remacs_runtime::editor::event_code_for("f12")),
        ]),
    };
    run_editor_with(frontend, &mut interp).unwrap();

    let left_text = interp
        .buffers
        .get(left_buffer)
        .unwrap()
        .borrow()
        .text
        .text()
        .to_string();
    let right_text = interp
        .buffers
        .get(right_buffer)
        .unwrap()
        .borrow()
        .text
        .text()
        .to_string();
    let displayed_buffer_id = right_window.borrow().buffer;
    let displayed_buffer_ref = interp.buffers.get(displayed_buffer_id).unwrap();
    let displayed_buffer = displayed_buffer_ref.borrow();
    assert_eq!(left_text, "left side\n");
    assert_eq!(right_text, "target DROPPEDbuffer\n");
    assert_eq!(format!("{}", displayed_buffer.text.text()), "file contents\n");
    assert!(displayed_buffer.file_name.is_some());
    let face_changed_symbol = interp.intern("qa-split-drop-color-changed");
    let face_changed = interp.symbol_value(face_changed_symbol);
    assert_eq!(interp.prin1_to_string(&face_changed), "t");
}
