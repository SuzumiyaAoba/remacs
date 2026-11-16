use remacs_runtime::{
    frontend::render_frame_grid_with_mouse_position_cached,
    lisp::{Interp, Value},
};

fn eval(interp: &mut Interp, source: &str) {
    interp
        .eval_str(source)
        .unwrap_or_else(|flow| panic!("{source}: {flow:?}"));
}

#[test]
fn tab_bar_fallback_uses_the_buffer_of_the_frame_being_rendered() {
    let mut interp = Interp::new();
    eval(
        &mut interp,
        r#"(progn
          (set-frame-parameter nil 'window-system 'ns)
          (set-frame-parameter nil 'menu-bar-lines 0)
          (set-frame-parameter nil 'tool-bar-lines 0)
          (set-frame-parameter nil 'tab-bar-lines 1)
          (set-frame-parameter nil 'tabs nil)
          (switch-to-buffer (get-buffer-create "qa-tab-fallback-frame-a")))"#,
    );
    let frame_a = interp.selected_frame.as_ref().unwrap().clone();
    let buffer_a = interp.current_buffer;

    eval(
        &mut interp,
        r#"(setq qa-tab-fallback-frame-b-buffer
                 (get-buffer-create "qa-tab-fallback-frame-b"))"#,
    );
    let Value::Frame(frame_b) = interp
        .eval_str(
            r#"(setq qa-tab-fallback-frame-b
                   (make-frame '((window-system . ns)
                                 (menu-bar-lines . 0)
                                 (tool-bar-lines . 0)
                                 (tab-bar-lines . 1)
                                 (tabs . nil))))"#,
        )
        .unwrap()
    else {
        panic!("second frame");
    };
    eval(
        &mut interp,
        "(set-window-buffer (frame-selected-window qa-tab-fallback-frame-b) \
         qa-tab-fallback-frame-b-buffer)",
    );
    let Value::Buffer(buffer_b) = interp
        .eval_str("qa-tab-fallback-frame-b-buffer")
        .unwrap()
    else {
        panic!("second frame buffer");
    };
    assert_ne!(buffer_a, buffer_b.borrow().id);
    assert_eq!(
        frame_b.borrow().selected.borrow().buffer,
        buffer_b.borrow().id
    );

    // Render the nonselected frame while the interpreter still has frame A's
    // current buffer, matching the GUI's multi-frame render path.
    interp.selected_frame = Some(frame_a.clone());
    interp.set_current_buffer(buffer_a);
    let grid_a = render_frame_grid_with_mouse_position_cached(&interp, &frame_a, 60, 12, None, None);
    let grid_b = render_frame_grid_with_mouse_position_cached(&interp, &frame_b, 60, 12, None, None);
    let tab_row_a = grid_a.tab_bar_items.first().expect("frame A tab").row;
    let tab_row_b = grid_b.tab_bar_items.first().expect("frame B tab").row;

    assert!(grid_a.rows[tab_row_a].contains("qa-tab-fallback-frame-a"));
    assert!(grid_b.rows[tab_row_b].contains("qa-tab-fallback-frame-b"));
    assert!(!grid_b.rows[tab_row_b].contains("qa-tab-fallback-frame-a"));
}
