use remacs_runtime::{editor::Frame, frontend::render_grid, lisp::Interp};

fn line_prefix_display_specs_reach_the_frontend_grid() {
    let mut interp = Interp::new();
    let buffer_id = interp.create_buffer("line-prefix-display-specs");
    let minibuffer_id = interp.create_buffer(" *Minibuf-0*");
    let frame = Frame::new_tty(buffer_id, minibuffer_id, 40, 12);
    interp.selected_frame = Some(frame.clone());
    interp.frames.push(frame);
    interp.set_current_buffer(buffer_id);

    interp
        .eval_str(
            r#"(progn
              (setq-local line-prefix '(space :width 2))
              (insert "first\nsecond\n")
              (put-text-property (point-min) (1+ (point-min))
                                 'line-prefix '(space :width 4))
              (goto-char (point-min)))"#,
        )
        .unwrap();
    let grid = render_grid(&interp, 20, 6);
    assert!(grid.rows[0].starts_with("    first"), "{:?}", grid.rows[0]);
    assert!(grid.rows[1].starts_with("  second"), "{:?}", grid.rows[1]);

    interp
        .eval_str(
            r#"(progn
              (erase-buffer)
              (setq-local line-prefix
                '(image :type svg
                        :data "<svg xmlns='http://www.w3.org/2000/svg' width='16' height='10'><rect width='16' height='10' fill='red'/></svg>"))
              (insert "image\nnext")
              (goto-char (point-min)))"#,
        )
        .unwrap();
    let grid = render_grid(&interp, 20, 6);
    for row in [0, 1] {
        assert!(
            grid.images
                .iter()
                .any(|image| image.row == row && image.column == 0),
            "missing line-prefix image on row {row}: {:?}",
            grid.images
        );
    }
    assert!(grid.rows[0].contains("image"));
    assert!(grid.rows[1].contains("next"));
}

#[test]
fn line_prefix_display_specs_render_spaces_and_images() {
    std::thread::Builder::new()
        .name("line-prefix-display-specs".into())
        .stack_size(64 * 1024 * 1024)
        .spawn(line_prefix_display_specs_reach_the_frontend_grid)
        .unwrap()
        .join()
        .expect("line-prefix display test thread panicked");
}
