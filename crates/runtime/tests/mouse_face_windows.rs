use remacs_runtime::{
    frontend::{MousePosition, render_grid_with_mouse_position_cached},
    lisp::{Interp, Value},
};

#[test]
fn shared_buffer_mouse_faces_follow_the_hovered_window() {
    let mut interp = Interp::new();
    interp
        .eval_str(
            r##"(progn
              (set-frame-size nil 80 12)
              (switch-to-buffer (get-buffer-create "*qa3301-shared*"))
              (erase-buffer) (insert "hover\n")
              (make-face 'qa3301-hover)
              (set-face-attribute 'qa3301-hover nil :background "#ff0000")
              (put-text-property 1 6 'mouse-face 'qa3301-hover)
              (setq-local mode-line-format nil)
              (setq-local header-line-format nil)
              (setq-local tab-line-format nil))"##,
        )
        .unwrap();
    let frame = interp.selected_frame.as_ref().unwrap().clone();
    remacs_runtime::editor::reflow_frame_windows(&frame, 80, 12);
    let left_id = frame.borrow().selected.borrow().id;
    let Value::Window(right) = interp.eval_str("(split-window-right 40)").unwrap() else {
        panic!("right window");
    };
    let right_id = right.borrow().id;
    let base = render_grid_with_mouse_position_cached(&interp, 80, 12, None, None);
    for (column, other, window_id) in [(1, 41, left_id), (41, 1, right_id)] {
        let mapped = base
            .input
            .as_ref()
            .unwrap()
            .hit_test_position(0, column, column)
            .unwrap();
        assert_eq!(mapped.buffer_position, 1);
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
        };
        let hovered = render_grid_with_mouse_position_cached(
            &interp,
            80,
            12,
            base.input.as_ref(),
            Some(position),
        );
        assert_ne!(hovered.attrs[0][column].bg, base.attrs[0][column].bg);
        assert_eq!(
            hovered.attrs[0][other].bg, base.attrs[0][other].bg,
            "highlight leaked from window {window_id} into its sibling"
        );
        assert_eq!(mapped.window_id, Some(window_id));
    }
}
