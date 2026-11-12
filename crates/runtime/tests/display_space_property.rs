use remacs_runtime::{
    editor::Frame,
    frontend::render_grid,
    lisp::Interp,
};

fn display_space_property_replaces_source_with_requested_width() {
    let mut interp = Interp::new();
    let buffer_id = interp.create_buffer("display-space-test");
    {
        let buffer = interp.buffers.get(buffer_id).unwrap();
        buffer.borrow_mut().insert("XY\n");
        buffer.borrow_mut().set_point(0);
    }
    let minibuffer_id = interp.create_buffer(" *Minibuf-0*");
    let frame = Frame::new_tty(buffer_id, minibuffer_id, 80, 25);
    interp.selected_frame = Some(frame.clone());
    interp.frames.push(frame);
    interp.set_current_buffer(buffer_id);

    interp
        .eval_str(
            "(put-text-property (point-min) (1+ (point-min)) 'display '(space :width 3))",
        )
        .unwrap();
    let grid = render_grid(&interp, 8, 4);

    assert_eq!(&grid.rows[0][..4], "   Y", "rendered row: {:?}", grid.rows[0]);
}

#[test]
fn display_space_spec_uses_its_width_and_hides_the_source_character() {
    std::thread::Builder::new()
        .name("display-space-property".into())
        .stack_size(8 * 1024 * 1024)
        .spawn(display_space_property_replaces_source_with_requested_width)
        .unwrap()
        .join()
        .expect("display space test thread panicked");
}
