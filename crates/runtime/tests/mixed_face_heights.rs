use remacs_runtime::{
    frontend::{Grid, render_grid},
    lisp::Interp,
};

fn row(i: &mut Interp, source: &str) -> Grid {
    i.eval_str("(erase-buffer)").unwrap();
    i.eval_str(source).unwrap();
    i.eval_str("(goto-char 1)").unwrap();
    render_grid(i, 40, 12)
}

#[test]
fn mixed_face_heights_reserve_the_tallest_font_on_each_row() {
    let mut i = Interp::new();
    i.eval_str(
        "(progn
      (set-frame-parameter nil 'window-system 'ns)
      (set-frame-parameter nil 'menu-bar-lines 0)
      (set-frame-parameter nil 'tool-bar-lines 0)
      (set-frame-parameter nil 'tab-bar-lines 0)
      (set-frame-size nil 40 12) (set-window-fringes nil 0 0)
      (set-face-attribute 'default nil :height 140)
      (setq-local mode-line-format nil header-line-format nil tab-line-format nil))",
    )
    .unwrap();
    remacs_runtime::editor::reflow_frame_windows(i.selected_frame.as_ref().unwrap(), 40, 12);
    let normal = row(&mut i, "(insert \"normal\")");
    let large = row(
        &mut i,
        "(insert (propertize \"large\" 'face '(:height 240)))",
    );
    assert!(large.row_heights[0] > normal.row_heights[0]);
    for text in [
        "(insert \"normal \" (propertize \"large\" 'face '(:height 240)) \" normal\")",
        "(insert (propertize \"large\" 'face '(:height 240)) \" normal\")",
        "(insert (propertize \"small\" 'face '(:height 100)) (propertize \"large\" 'face '(:height 240)))",
        "(insert \"a\" (propertize \" \" 'face '(:height 240)) \"b\")",
    ] {
        let mixed = row(&mut i, text);
        assert_eq!(mixed.row_heights[0], large.row_heights[0], "{text}");
        assert!(
            mixed.char_attrs[0]
                .iter()
                .any(|style| style.font_height == Some(240))
        );
    }
    let relative = row(
        &mut i,
        "(insert \"normal \" (propertize \"relative\" 'face '(:height 1.5)))",
    );
    let uniform_relative = row(
        &mut i,
        "(insert (propertize \"relative\" 'face '(:height 210)))",
    );
    assert_eq!(relative.row_heights[0], uniform_relative.row_heights[0]);
    assert!(
        relative.char_attrs[0]
            .iter()
            .any(|style| style.font_height == Some(210))
    );
    i.eval_str("(progn (make-face 'mixed-height-parent) (set-face-attribute 'mixed-height-parent nil :height 180))").unwrap();
    let inherited = row(
        &mut i,
        "(insert \"normal \" (propertize \"inherited\" 'face '(:inherit mixed-height-parent :height 1.5)))",
    );
    let uniform_inherited = row(
        &mut i,
        "(insert (propertize \"inherited\" 'face '(:height 270)))",
    );
    assert_eq!(inherited.row_heights[0], uniform_inherited.row_heights[0]);
    assert!(
        inherited.char_attrs[0]
            .iter()
            .any(|style| style.font_height == Some(270))
    );
    let layers = row(
        &mut i,
        "(insert \"normal \" (propertize \"layers\" 'face '((:height 1.5) (:height 180))))",
    );
    assert_eq!(layers.row_heights[0], uniform_inherited.row_heights[0]);
    assert!(
        layers.char_attrs[0]
            .iter()
            .any(|style| style.font_height == Some(270))
    );
}
