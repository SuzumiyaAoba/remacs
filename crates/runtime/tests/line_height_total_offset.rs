use remacs_runtime::{frontend::render_grid, lisp::Interp};

fn height_total_records_text_offset_and_total_row_height() {
    let mut interp = Interp::new();
    interp
        .eval_str(
            r#"(progn
              (set-frame-parameter nil 'window-system 'ns)
              (set-frame-parameter nil 'menu-bar-lines 0)
              (set-frame-parameter nil 'tool-bar-lines 0)
              (set-frame-parameter nil 'tab-bar-lines 0)
              (set-frame-size nil 40 12)
              (set-face-attribute 'default nil :height 140)
              (setq-local mode-line-format nil
                          header-line-format nil
                          tab-line-format nil)
              (insert "first" (propertize "\n" 'line-height '(40 60)) "second")
              (goto-char (point-min)))"#,
        )
        .unwrap();
    let frame = interp.selected_frame.as_ref().unwrap().clone();
    let window_id = frame.borrow().selected.borrow().id;

    let grid = render_grid(&interp, 40, 12);
    let metrics = grid
        .window_row_heights
        .iter()
        .find(|metrics| metrics.window_id == window_id)
        .unwrap();
    let expected_offset = 40usize.saturating_sub(grid.default_line_height);
    assert_eq!(grid.row_heights[0], 60);
    assert_eq!(metrics.heights[0], 60);
    assert_eq!(metrics.text_offsets[0], expected_offset);
    assert!(expected_offset > 0);
}

#[test]
fn height_total_creates_top_spacing_before_text() {
    std::thread::Builder::new()
        .name("line-height-total-offset".into())
        .stack_size(64 * 1024 * 1024)
        .spawn(height_total_records_text_offset_and_total_row_height)
        .unwrap()
        .join()
        .expect("line-height total-offset test thread panicked");
}
