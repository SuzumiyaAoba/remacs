use remacs_runtime::{editor::Frame, frontend::render_grid_mut, lisp::Interp};

fn conditional_display_specs_evaluate_with_buffer_context() {
    let mut interp = Interp::new();
    let buffer_id = interp.create_buffer("display-when-test");
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
            r#"(progn
                 (put-text-property
                  (point-min) (1+ (point-min)) 'display
                  '(when (and (eq object (current-buffer))
                              (= position 1)
                              (= buffer-position 1))
                     . "A"))
                 (put-text-property
                  (1+ (point-min)) (+ (point-min) 2) 'display
                  '(when nil . "B")))"#,
        )
        .unwrap();

    let grid = render_grid_mut(&mut interp, 8, 4);
    assert_eq!(&grid.rows[0][..2], "AY", "rendered row: {:?}", grid.rows[0]);
}

#[test]
fn conditional_display_spec_applies_only_when_its_condition_is_true() {
    std::thread::Builder::new()
        .name("conditional-display-spec".into())
        .stack_size(8 * 1024 * 1024)
        .spawn(conditional_display_specs_evaluate_with_buffer_context)
        .unwrap()
        .join()
        .expect("conditional display test thread panicked");
}
