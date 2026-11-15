use remacs_runtime::{
    editor::Frame,
    frontend::{CellColor, render_grid},
    lisp::Interp,
};

fn string_line_prefix_inherits_the_default_face_style() {
    let mut interp = Interp::new();
    let buffer_id = interp.create_buffer("line-prefix-face-style");
    let minibuffer_id = interp.create_buffer(" *Minibuf-0*");
    let frame = Frame::new_tty(buffer_id, minibuffer_id, 40, 12);
    interp.selected_frame = Some(frame.clone());
    interp.frames.push(frame);
    interp.set_current_buffer(buffer_id);
    interp
        .eval_str(
            r#"(progn
              (set-face-attribute 'default nil :foreground "red" :background "blue")
              (setq-local line-prefix "> ")
              (insert "body")
              (goto-char (point-min)))"#,
        )
        .unwrap();

    let grid = render_grid(&interp, 20, 6);
    let default = grid.default_style;
    assert_eq!(default.fg, Some(CellColor(255, 0, 0)));
    assert_eq!(default.bg, Some(CellColor(0, 0, 255)));
    assert_eq!(grid.char_attrs[0][0].fg, default.fg);
    assert_eq!(grid.char_attrs[0][0].bg, default.bg);
    assert_eq!(grid.char_attrs[0][2].fg, default.fg);
    assert_eq!(grid.char_attrs[0][2].bg, default.bg);
}

#[test]
fn string_line_prefix_inherits_default_face_style() {
    std::thread::Builder::new()
        .name("line-prefix-face-style".into())
        .stack_size(64 * 1024 * 1024)
        .spawn(string_line_prefix_inherits_the_default_face_style)
        .unwrap()
        .join()
        .expect("line-prefix style test thread panicked");
}
