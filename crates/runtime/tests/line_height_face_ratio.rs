use remacs_runtime::{
    frontend::{Grid, render_grid},
    lisp::Interp,
};

fn row(i: &mut Interp, source: &str) -> Grid {
    i.eval_str("(erase-buffer)").unwrap();
    i.eval_str(source).unwrap();
    i.eval_str("(goto-char (point-min))").unwrap();
    render_grid(i, 40, 12)
}

fn line_height_face_ratio_uses_the_named_faces_height() {
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
                          tab-line-format nil))"#,
        )
        .unwrap();
    let normal = row(&mut interp, r#"(insert "first\nsecond")"#);

    interp
        .eval_str(
            r#"(progn
              (make-face 'qa-line-height-large)
              (set-face-attribute 'qa-line-height-large nil :height 240))"#,
        )
        .unwrap();
    let large = row(
        &mut interp,
        r#"(insert "first"
                (propertize "\n" 'line-height '(qa-line-height-large . 1.0))
                "second")"#,
    );

    assert!(
        large.row_heights[0] > normal.row_heights[0],
        "specified face height was ignored: normal row={}, line-height row={}",
        normal.row_heights[0],
        large.row_heights[0]
    );
}

#[test]
fn line_height_face_ratio_uses_the_named_faces_height_on_graphical_frames() {
    std::thread::Builder::new()
        .name("line-height-face-ratio".into())
        .stack_size(64 * 1024 * 1024)
        .spawn(line_height_face_ratio_uses_the_named_faces_height)
        .unwrap()
        .join()
        .expect("line-height face ratio test thread panicked");
}
