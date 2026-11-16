use remacs_runtime::{
    editor::reflow_frame_windows,
    frontend::render_grid_with_input_cached,
    lisp::Interp,
};

#[test]
fn hit_test_prefers_visible_character_after_hidden_text() {
    let mut interp = Interp::new();
    interp
        .eval_str(
            r#"(progn
          (set-frame-parameter nil 'window-system 'ns)
          (set-frame-parameter nil 'menu-bar-lines 0)
          (set-frame-parameter nil 'tool-bar-lines 0)
          (set-frame-parameter nil 'tab-bar-lines 0)
          (set-frame-parameter nil 'vertical-scroll-bars nil)
          (set-frame-parameter nil 'horizontal-scroll-bars nil)
          (set-frame-size nil 40 12)
          (setq-local mode-line-format nil header-line-format nil tab-line-format nil
                      buffer-invisibility-spec '(t))
          (set-window-fringes nil 0 0)
          (erase-buffer)
          (insert "leftHIDDENright")
          (put-text-property 5 11 'invisible t)
          (goto-char (point-min))
          (set-window-start nil (point-min) t))"#,
        )
        .unwrap();
    reflow_frame_windows(interp.selected_frame.as_ref().unwrap(), 40, 12);

    let grid = render_grid_with_input_cached(&interp, 40, 12, None);
    let area = grid.window_text_areas.first().unwrap();
    let row = area.top;
    assert_eq!(grid.rows[row].trim_end(), "leftright");

    let source_byte = grid.rows[row].find('r').unwrap();
    let hit = grid
        .input
        .as_ref()
        .unwrap()
        .hit_test_position(row, area.text_left + source_byte, source_byte)
        .unwrap();
    assert_eq!(hit.buffer_position, 10, "Lisp position should be 11");
}
