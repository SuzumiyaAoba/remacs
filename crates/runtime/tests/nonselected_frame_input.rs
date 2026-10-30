use remacs_runtime::{
    editor::Frame,
    frontend::render_frame_grid_with_input_cached,
    lisp::Interp,
};

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
}
