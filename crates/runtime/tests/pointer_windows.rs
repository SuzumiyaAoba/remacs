use remacs_runtime::{
    editor::reflow_frame_windows,
    frontend::{GridPointerShape as Shape, MousePosition, render_grid_with_mouse_position_cached},
    lisp::{Interp, Value},
};

fn hover(i: &Interp, column: usize, expected_window: usize) -> MousePosition {
    let grid = render_grid_with_mouse_position_cached(i, 80, 12, None, None);
    let mapped = grid
        .input
        .as_ref()
        .unwrap()
        .hit_test_position(0, column, column)
        .unwrap();
    assert_eq!(mapped.window_id, Some(expected_window));
    MousePosition {
        buffer_id: mapped.buffer_id,
        buffer_position: mapped.buffer_position,
        utf16_offset: mapped.utf16_offset,
        x: column as i128,
        y: 0,
        row: 0,
        column: mapped.column,
        window_id: mapped.window_id,
        area: None,
        mode_line_buffer_name: false,
    }
}

fn shape(i: &Interp, position: MousePosition) -> Option<Shape> {
    render_grid_with_mouse_position_cached(i, 80, 12, None, Some(position)).hover_cursor
}

#[test]
fn split_window_pointers_follow_the_target_without_changing_selection() {
    let mut i = Interp::new();
    i.eval_str(
        r#"(progn
        (set-frame-parameter nil 'window-system 'ns)
        (set-frame-parameter nil 'menu-bar-lines 0)
        (set-frame-parameter nil 'tool-bar-lines 0)
        (set-frame-parameter nil 'tab-bar-lines 0)
        (set-frame-parameter nil 'vertical-scroll-bars nil)
        (set-frame-size nil 80 12) (set-window-fringes nil 0 0)
        (setq-default mode-line-format nil header-line-format nil tab-line-format nil)
        (switch-to-buffer (get-buffer-create "*pointer-left*"))
        (erase-buffer) (insert (propertize "left text" 'pointer 'arrow)) (goto-char 1)
        (setq qa-pointer-left (selected-window) qa-pointer-right (split-window-right 40))
        (with-current-buffer (get-buffer-create "*pointer-right*")
          (erase-buffer)
          (insert (propertize "hand" 'pointer 'hand) " plain "
                  (propertize "wait" 'pointer 'hourglass) " overlay")
          (goto-char 1)
          (let ((overlay (make-overlay 17 24)))
            (overlay-put overlay 'pointer 'nhdrag)
            (overlay-put overlay 'window qa-pointer-right)))
        (set-window-buffer qa-pointer-right "*pointer-right*"))"#,
    )
    .unwrap();
    let frame = i.selected_frame.as_ref().unwrap().clone();
    reflow_frame_windows(&frame, 80, 12);
    let selected = frame.borrow().selected.clone();
    let left = selected.borrow().id;
    let Value::Window(right) = i.eval_str("qa-pointer-right").unwrap() else {
        panic!("right window")
    };
    let right = right.borrow().id;
    let current = i.current_buffer;
    let point = i.buffers.get(current).unwrap().borrow().point();
    for (column, window, expected) in [
        (1, left, Shape::Arrow),
        (41, right, Shape::Hand),
        (46, right, Shape::Text),
        (52, right, Shape::Hourglass),
        (57, right, Shape::ResizeVertical),
    ] {
        assert_eq!(
            shape(&i, hover(&i, column, window)),
            Some(expected),
            "column {column}"
        );
    }
    // The same buffer can have different window-restricted overlay pointers.
    i.eval_str(
        r#"(progn
        (set-window-buffer qa-pointer-right (current-buffer))
        (let ((overlay (make-overlay 1 5)))
          (overlay-put overlay 'pointer 'hand)
          (overlay-put overlay 'window qa-pointer-right)))"#,
    )
    .unwrap();
    assert_eq!(shape(&i, hover(&i, 1, left)), Some(Shape::Arrow));
    assert_eq!(shape(&i, hover(&i, 41, right)), Some(Shape::Hand));
    let mut legacy = hover(&i, 1, left);
    legacy.window_id = None;
    assert_eq!(shape(&i, legacy), Some(Shape::Arrow));
    legacy.window_id = Some(usize::MAX);
    assert_eq!(shape(&i, legacy), None);
    assert!(std::rc::Rc::ptr_eq(&selected, &frame.borrow().selected));
    assert_eq!(i.current_buffer, current);
    assert_eq!(i.buffers.get(current).unwrap().borrow().point(), point);
}
