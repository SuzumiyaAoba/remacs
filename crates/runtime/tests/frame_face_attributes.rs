use remacs_runtime::{
    editor::reflow_frame_windows,
    frontend::{CellColor, render_frame_grid_with_input_cached},
    lisp::{Interp, Value},
};

#[test]
fn face_attributes_are_isolated_and_rendered_per_frame() {
    let mut i = Interp::new();
    i.eval_str(
        r#"(progn
          (set-frame-parameter nil 'window-system 'ns)
          (set-frame-parameter nil 'menu-bar-lines 0)
          (set-frame-parameter nil 'tool-bar-lines 0)
          (set-frame-parameter nil 'tab-bar-lines 0)
          (set-frame-size nil 40 12)
          (setq-default mode-line-format nil header-line-format nil tab-line-format nil)
          (set-face-attribute 'default nil :height 140 :foreground "white")
          (setq qa-face-attribute-frame-a (selected-frame)
                qa-face-attribute-frame-b
                (make-frame '((window-system . ns) (width . 40) (height . 12)
                  (menu-bar-lines . 0) (tool-bar-lines . 0) (tab-bar-lines . 0))))
          (set-face-attribute 'default qa-face-attribute-frame-a
                              :height 120 :foreground "red" :underline t)
          (set-face-attribute 'default qa-face-attribute-frame-b
                              :height 200 :foreground "blue" :underline nil)
          (with-current-buffer (get-buffer-create "*frame-face-attributes*")
            (erase-buffer)
            (insert "frame face"))
          (dolist (frame (list qa-face-attribute-frame-a qa-face-attribute-frame-b))
            (set-window-buffer (frame-selected-window frame) "*frame-face-attributes*"))
          (select-frame qa-face-attribute-frame-a))"#,
    )
    .unwrap();

    let frame_a = i.selected_frame.as_ref().unwrap().clone();
    let Value::Frame(frame_b) = i.eval_str("qa-face-attribute-frame-b").unwrap() else {
        panic!("frame B")
    };
    for frame in [&frame_a, &frame_b] {
        reflow_frame_windows(frame, 40, 12);
    }

    for (frame, frame_name, expected_height, expected_color, expected_underline) in [
        (
            &frame_a,
            "qa-face-attribute-frame-a",
            120,
            CellColor(255, 0, 0),
            true,
        ),
        (
            &frame_b,
            "qa-face-attribute-frame-b",
            200,
            CellColor(0, 0, 255),
            false,
        ),
    ] {
        let face_height = i
            .eval_str(&format!(
                "(face-attribute 'default :height {frame_name})"
            ))
            .unwrap();
        assert!(matches!(
            face_height,
            Value::Int(height) if height == expected_height
        ));
        let face_underline = i
            .eval_str(&format!("(face-underline-p 'default {frame_name})"))
            .unwrap();
        assert_eq!(face_underline.truthy(), expected_underline);

        let grid = render_frame_grid_with_input_cached(&i, frame, 40, 12, None);
        assert_eq!(grid.default_style.font_height, Some(expected_height as u16));
        assert_eq!(grid.default_style.fg, Some(expected_color));
        assert_eq!(grid.default_style.underline, expected_underline);
        assert!(std::rc::Rc::ptr_eq(
            i.selected_frame.as_ref().unwrap(),
            &frame_a
        ));
    }

    i.eval_str(
        "(set-face-attribute 'default nil :height 160 :foreground \"green\" :underline nil)",
    )
    .unwrap();
    for frame in [&frame_a, &frame_b] {
        let grid = render_frame_grid_with_input_cached(&i, frame, 40, 12, None);
        assert_eq!(grid.default_style.font_height, Some(160));
        assert_eq!(grid.default_style.fg, Some(CellColor(0, 255, 0)));
        assert!(!grid.default_style.underline);
    }
}
