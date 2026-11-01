use remacs_runtime::{
    editor::{FrameRef, reflow_frame_windows},
    frontend::render_frame_grid_with_input_cached,
    lisp::{Interp, Value},
};

fn check(i: &Interp, frame: &FrameRef, row: usize, column: usize, start_row: usize) {
    let selected = i.selected_frame.as_ref().unwrap().clone();
    let current = i.current_buffer;
    let window = frame.borrow().selected.clone();
    let window_id = window.borrow().id;
    let saved_point = window.borrow().point;
    let buffer = i.buffers.get(window.borrow().buffer).unwrap();
    let (expected, expected_start, live_point) = {
        let buffer = buffer.borrow();
        (
            buffer.text.line_start(row) + column,
            buffer.text.line_start(start_row),
            buffer.point(),
        )
    };
    let grid = render_frame_grid_with_input_cached(i, frame, 48, 12, None);
    let input = grid.input.as_ref().unwrap();
    assert_eq!(input.cursor, expected, "native input caret");
    assert_eq!(
        window.borrow().start,
        expected_start,
        "viewport must follow this window's point"
    );
    assert_eq!(input.rows.first().unwrap().buffer_start, expected_start);
    let cursor = grid.cursors.first().expect("window cursor");
    let mapped = input
        .hit_test_position(cursor.row, cursor.column, cursor.index)
        .unwrap();
    assert_eq!(mapped.window_id, Some(window_id));
    assert_eq!(
        mapped.buffer_position, expected,
        "painted cursor must match the input caret"
    );
    let mode = grid
        .mode_lines
        .iter()
        .find(|line| line.window_id == window_id)
        .unwrap();
    assert_eq!(
        grid.rows[mode.row].trim(),
        format!("L{}:C{column}", row + 1)
    );
    assert_eq!(buffer.borrow().point(), live_point);
    assert_eq!(window.borrow().point, saved_point);
    assert!(std::rc::Rc::ptr_eq(
        i.selected_frame.as_ref().unwrap(),
        &selected
    ));
    assert_eq!(i.current_buffer, current);
}

#[test]
fn frame_redisplay_uses_independent_saved_window_points_and_live_selected_point() {
    let mut i = Interp::new();
    i.eval_str(r#"(progn
        (set-frame-parameter nil 'window-system 'ns)
        (set-frame-parameter nil 'menu-bar-lines 0)
        (set-frame-parameter nil 'tool-bar-lines 0)
        (set-frame-parameter nil 'tab-bar-lines 0)
        (set-frame-size nil 48 12) (set-window-fringes nil 0 0)
        (switch-to-buffer (get-buffer-create "*shared-frame-points*"))
        (erase-buffer) (dotimes (n 100) (insert (format "row-%02d %s\n" n (make-string 100 ?x))))
        (setq-local mode-line-format "L%l:C%c" header-line-format nil tab-line-format nil
          truncate-lines t cursor-in-non-selected-windows t scroll-margin 0 auto-hscroll-mode t)
        (defun qa-frame-point-at (line column)
          (save-excursion (goto-char (point-min)) (forward-line line) (forward-char column) (point)))
        (goto-char (qa-frame-point-at 1 1))
        (setq qa-point-frame-a (selected-frame) qa-point-window-a (selected-window)
              qa-point-frame-b (make-frame '((window-system . ns) (width . 48) (height . 12)
                (menu-bar-lines . 0) (tool-bar-lines . 0) (tab-bar-lines . 0))))
        (setq qa-point-window-b (frame-selected-window qa-point-frame-b))
        (set-window-buffer qa-point-window-b (current-buffer))
        (set-window-point qa-point-window-b (qa-frame-point-at 47 3))
        (set-window-start qa-point-window-a 1 t)
        (set-window-start qa-point-window-b (qa-frame-point-at 43 0) t))"#).unwrap();
    let a = i.selected_frame.as_ref().unwrap().clone();
    let Value::Frame(b) = i.eval_str("qa-point-frame-b").unwrap() else {
        panic!("frame B")
    };
    for frame in [&a, &b] {
        reflow_frame_windows(frame, 48, 12);
    }
    check(&i, &b, 47, 3, 43);
    check(&i, &a, 1, 1, 0);
    i.eval_str("(set-window-point qa-point-window-b (qa-frame-point-at 47 75))")
        .unwrap();
    check(&i, &b, 47, 75, 43);
    assert!(b.borrow().selected.borrow().hscroll > 0);
    check(&i, &a, 1, 1, 0);
    assert_eq!(a.borrow().selected.borrow().hscroll, 0);
    i.eval_str(
        r#"(progn
        (select-frame qa-point-frame-b)
        (goto-char (qa-frame-point-at 60 5))
        (set-window-start qa-point-window-b (qa-frame-point-at 57 0) t))"#,
    )
    .unwrap();
    check(&i, &a, 1, 1, 0);
    check(&i, &b, 60, 5, 57);
    i.eval_str("(progn (select-frame qa-point-frame-a) (goto-char (qa-frame-point-at 5 2)))")
        .unwrap();
    check(&i, &b, 60, 5, 57);
    check(&i, &a, 5, 2, 0);
}
