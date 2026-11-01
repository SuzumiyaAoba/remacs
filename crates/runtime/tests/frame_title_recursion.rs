use remacs_runtime::{
    editor::FrameRef,
    frontend::render_frame_grid_with_input_cached,
    lisp::{Interp, Value},
};

#[track_caller]
fn check(i: &Interp, frame: &FrameRef, expected: &str) {
    let selected = i.selected_frame.as_ref().unwrap().clone();
    let current = i.current_buffer;
    let depth = i.recursion_depth;
    let mini = i.minibuf_level;
    assert_eq!(
        render_frame_grid_with_input_cached(i, frame, 48, 12, None).title,
        expected
    );
    assert!(std::rc::Rc::ptr_eq(
        i.selected_frame.as_ref().unwrap(),
        &selected
    ));
    assert_eq!(i.current_buffer, current);
    assert_eq!(i.recursion_depth, depth);
    assert_eq!(i.minibuf_level, mini);
}

#[test]
fn frame_title_brackets_use_recursive_edit_depth_and_gnu_abbreviation() {
    let mut i = Interp::new();
    i.eval_str(
        r#"(progn
        (set-frame-parameter nil 'window-system 'ns)
        (set-frame-parameter nil 'menu-bar-lines 0)
        (set-frame-parameter nil 'tool-bar-lines 0)
        (set-frame-parameter nil 'tab-bar-lines 0)
        (set-frame-size nil 48 12)
        (setq-default mode-line-format nil header-line-format nil tab-line-format nil)
        (setq frame-title-format "%[Edit%]"))"#,
    )
    .unwrap();
    let primary = i.selected_frame.as_ref().unwrap().clone();
    let Value::Frame(other) = i
        .eval_str(r#"(make-frame '((window-system . ns) (width . 48) (height . 12)))"#)
        .unwrap()
    else {
        panic!("other frame")
    };
    // GNU 31.1 decode_mode_spec uses command_loop_level, rather than
    // recursion-depth's command-loop-plus-minibuffer total. Feed the
    // renderer those interpreter counters without running a nested loop.
    for (depth, expected, clipped) in [
        (0, "Edit", "Edit"),
        (1, "[Edit]", "[Edi"),
        (2, "[[Edit]]", "[[Ed"),
        (3, "[[[Edit]]]", "[[[E"),
        (4, "[[[[Edit]]]]", "[[[["),
        (5, "[[[[[Edit]]]]]", "[[[["),
        (6, "[[[... Edit ...]]]", "[[[."),
        (99, "[[[... Edit ...]]]", "[[[."),
        (usize::MAX - 3, "[[[... Edit ...]]]", "[[[."),
        (1, "[Edit]", "[Edi"),
        (0, "Edit", "Edit"),
    ] {
        i.recursion_depth = depth;
        for mini in [0, 3] {
            i.minibuf_level = mini;
            assert_eq!(
                i.eval_str("(recursion-depth)").unwrap().int(),
                Some((depth as u128 + mini as u128) as i128)
            );
            for form in [r#""%[Edit%]""#, r#"("" "%[" "Edit" "%]")"#] {
                i.eval_str(&format!("(setq frame-title-format '{form})"))
                    .unwrap();
                check(&i, &primary, expected);
                check(&i, &other, expected);
            }
            i.eval_str(r#"(setq frame-title-format '(-4 "%[Edit%]"))"#)
                .unwrap();
            check(&i, &primary, clipped);
            i.eval_str(r#"(setq frame-title-format '(12 "%[Edit%]"))"#)
                .unwrap();
            check(&i, &primary, &format!("{expected:<12}"));
            i.eval_str(r#"(setq frame-title-format "%%[Edit%%]")"#)
                .unwrap();
            check(&i, &primary, "%[Edit%]");
        }
    }
    i.eval_str(r#"(set-frame-parameter nil 'title "%[literal%]")"#)
        .unwrap();
    i.recursion_depth = 2;
    check(&i, &primary, "%[literal%]");
    i.eval_str(
        r#"(progn (set-frame-parameter nil 'title nil) (setq frame-title-format "%[Edit%]"))"#,
    )
    .unwrap();
    check(&i, &primary, "[[Edit]]");
}
