use remacs_runtime::{
    editor::{FrameRef, reflow_frame_windows},
    frontend::{GridCursorShape as Shape, render_frame_grid_with_input_cached},
    lisp::{Interp, Value},
};

fn check(i: &Interp, active: &FrameRef, inactive: &FrameRef, expected: Option<(Shape, usize)>) {
    let current = i.current_buffer;
    let selected = i.selected_frame.as_ref().unwrap().clone();
    let grid = render_frame_grid_with_input_cached(i, active, 48, 12, None);
    assert_eq!(grid.cursors.len(), 1);
    assert_eq!(grid.cursors[0].shape, Shape::Block);
    assert!(grid.cursors[0].selected);
    assert!(grid.cursor.is_some());
    let other = render_frame_grid_with_input_cached(i, inactive, 48, 12, None);
    assert_eq!(
        other.input.as_ref().unwrap().cursor,
        3,
        "input caret remains available"
    );
    assert!(
        other.cursor.is_none(),
        "inactive frame has no active/system caret"
    );
    assert!(other.cursor_index.is_none());
    match expected {
        None => assert!(other.cursors.is_empty(), "nonselected cursor disabled"),
        Some((shape, thickness)) => {
            assert_eq!(other.cursors.len(), 1);
            let cursor = other.cursors[0];
            assert_eq!(cursor.shape, shape);
            assert_eq!(cursor.thickness, thickness);
            assert!(
                !cursor.selected,
                "inactive cursors must not participate in selected-cursor blinking"
            );
        }
    }
    assert_eq!(i.current_buffer, current);
    assert!(std::rc::Rc::ptr_eq(
        i.selected_frame.as_ref().unwrap(),
        &selected
    ));
}

#[test]
fn inactive_frame_cursors_obey_nonselected_policy_and_do_not_blink() {
    let mut i = Interp::new();
    i.eval_str(
        r#"(progn
        (set-frame-parameter nil 'window-system 'ns)
        (set-frame-parameter nil 'menu-bar-lines 0)
        (set-frame-parameter nil 'tool-bar-lines 0)
        (set-frame-parameter nil 'tab-bar-lines 0)
        (set-frame-size nil 48 12) (set-window-fringes nil 0 0)
        (switch-to-buffer (get-buffer-create "*cursor-frame-a*"))
        (erase-buffer) (insert "frame A\n") (goto-char 3)
        (setq-local mode-line-format nil header-line-format nil tab-line-format nil
          cursor-type 'box cursor-in-non-selected-windows nil)
        (setq blink-cursor-mode t blink-cursor-delay 0.0 blink-cursor-interval 1.0)
        (setq qa-cursor-frame-a (selected-frame)
              qa-cursor-frame-b (make-frame '((window-system . ns) (width . 48) (height . 12)
                (menu-bar-lines . 0) (tool-bar-lines . 0) (tab-bar-lines . 0))))
        (select-frame qa-cursor-frame-b)
        (switch-to-buffer (get-buffer-create "*cursor-frame-b*"))
        (erase-buffer) (insert "frame B\n") (goto-char 4)
        (setq-local mode-line-format nil header-line-format nil tab-line-format nil
          cursor-type 'box cursor-in-non-selected-windows t)
        (setq qa-cursor-window-b (selected-window))
        (select-frame qa-cursor-frame-a))"#,
    )
    .unwrap();
    let a = i.selected_frame.as_ref().unwrap().clone();
    let Value::Frame(b) = i.eval_str("qa-cursor-frame-b").unwrap() else {
        panic!("frame B")
    };
    for frame in [&a, &b] {
        reflow_frame_windows(frame, 48, 12);
    }
    for (normal, alternative, expected) in [
        ("'box", "t", Some((Shape::Hollow, 0))),
        ("'box", "nil", None),
        ("'box", "'(bar . 4)", Some((Shape::Bar, 4))),
        ("'(bar . 5)", "t", Some((Shape::Bar, 4))),
        ("'(bar . 1)", "t", Some((Shape::Bar, 1))),
        ("'(hbar . 5)", "t", Some((Shape::HBar, 5))),
        ("'hollow", "t", Some((Shape::Hollow, 0))),
        ("'box", "'box", Some((Shape::Block, 0))),
        ("nil", "t", None),
        ("nil", "'(bar . 4)", None),
    ] {
        i.eval_str(&format!("(with-current-buffer \"*cursor-frame-b*\" (setq cursor-type {normal} cursor-in-non-selected-windows {alternative}))")).unwrap();
        check(&i, &a, &b, expected);
    }
    i.eval_str(r#"(progn
        (with-current-buffer "*cursor-frame-b*" (setq cursor-type 'box cursor-in-non-selected-windows t))
        (set-window-cursor-type qa-cursor-window-b '(hbar . 3)))"#).unwrap();
    check(&i, &a, &b, Some((Shape::HBar, 3)));
    i.eval_str(r#"(progn
        (set-window-cursor-type qa-cursor-window-b t)
        (with-current-buffer "*cursor-frame-b*" (setq cursor-type '(bar . 5) cursor-in-non-selected-windows nil))
        (select-frame qa-cursor-frame-b))"#).unwrap();
    let selected_b = render_frame_grid_with_input_cached(&i, &b, 48, 12, None);
    assert_eq!(selected_b.cursors.len(), 1);
    assert_eq!(selected_b.cursors[0].shape, Shape::Bar);
    assert_eq!(selected_b.cursors[0].thickness, 5);
    assert!(selected_b.cursors[0].selected);
    let inactive_a = render_frame_grid_with_input_cached(&i, &a, 48, 12, None);
    assert!(inactive_a.cursors.is_empty());
    assert!(inactive_a.cursor.is_none());
    i.eval_str("(select-frame qa-cursor-frame-a)").unwrap();
    check(&i, &a, &b, None);
}
