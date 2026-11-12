use remacs_runtime::{
    frontend::{GridPointerShape, MousePosition, render_grid_with_mouse_position_cached},
    lisp::Interp,
};

#[test]
fn hourglass_text_and_overlay_properties_request_a_busy_pointer() {
    let mut i = Interp::new();
    i.eval_str(
        r#"(progn
        (set-frame-parameter nil 'window-system 'ns)
        (set-frame-parameter nil 'menu-bar-lines 0)
        (set-frame-parameter nil 'tool-bar-lines 0)
        (set-frame-parameter nil 'tab-bar-lines 0)
        (set-window-fringes nil 0 0)
        (setq-local mode-line-format nil header-line-format nil tab-line-format nil)
        (erase-buffer)
        (insert (propertize "text" 'pointer 'hourglass) " overlay plain")
        (let ((overlay (make-overlay 6 13)))
          (overlay-put overlay 'pointer 'hourglass))
        (goto-char 1))"#,
    )
    .unwrap();
    remacs_runtime::editor::reflow_frame_windows(i.selected_frame.as_ref().unwrap(), 40, 12);
    let base = render_grid_with_mouse_position_cached(&i, 40, 12, None, None);
    for (column, expected) in [
        (1, GridPointerShape::Hourglass),
        (6, GridPointerShape::Hourglass),
        (14, GridPointerShape::Text),
    ] {
        let mapped = base
            .input
            .as_ref()
            .unwrap()
            .hit_test_position(0, column, column)
            .unwrap();
        let position = MousePosition {
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
            object: None,
        };
        let grid =
            render_grid_with_mouse_position_cached(&i, 40, 12, base.input.as_ref(), Some(position));
        assert_eq!(grid.hover_cursor, Some(expected), "column {column}");
    }
}
