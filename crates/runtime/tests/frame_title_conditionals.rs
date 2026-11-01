use remacs_runtime::{
    editor::{Frame, FrameRef},
    frontend::render_frame_grid_with_input_cached,
    lisp::{Interp, Value},
};

#[track_caller]
fn check(i: &Interp, frame: &FrameRef, expected: &str) {
    let selected = i.selected_frame.as_ref().unwrap().clone();
    let current = i.current_buffer;
    assert_eq!(
        render_frame_grid_with_input_cached(i, frame, 48, 12, None).title,
        expected
    );
    assert!(std::rc::Rc::ptr_eq(
        i.selected_frame.as_ref().unwrap(),
        &selected
    ));
    assert_eq!(i.current_buffer, current);
}

#[test]
fn frame_title_conditionals_choose_one_branch_and_track_eligible_frames() {
    let mut i = Interp::new();
    i.eval_str(
        r#"(progn
        (set-frame-parameter nil 'window-system 'ns)
        (set-frame-parameter nil 'menu-bar-lines 0)
        (set-frame-parameter nil 'tool-bar-lines 0)
        (set-frame-parameter nil 'tab-bar-lines 0)
        (set-frame-size nil 48 12)
        (setq-default mode-line-format nil header-line-format nil tab-line-format nil)
        (switch-to-buffer (get-buffer-create "primary-title"))
        (defvar qa-title-flag nil)
        (defvaralias 'qa-title-flag-alias 'qa-title-flag)
        (setq invocation-name "test-remacs" system-name "test-host"))"#,
    )
    .unwrap();
    let primary = i.selected_frame.as_ref().unwrap().clone();
    for (form, expected) in [
        (r#"(qa-title-flag "yes" "no")"#, "no"),
        (r#"(t "yes" "no")"#, "yes"),
        (r#"(nil "yes" "no")"#, "no"),
        (r#"(qa-title-unbound "yes" "no")"#, "no"),
        (r#"(qa-title-unbound "yes")"#, ""),
        ("(t)", ""),
        (r#"(5 (qa-title-unbound "yes"))"#, "     "),
    ] {
        i.eval_str(&format!("(setq frame-title-format '{form})"))
            .unwrap();
        check(&i, &primary, expected);
    }
    i.eval_str(
        r#"(progn (setq-local qa-title-flag 0)
        (setq frame-title-format '(qa-title-flag-alias "yes" "no")))"#,
    )
    .unwrap();
    check(&i, &primary, "yes");
    let mark = i.specbind_depth();
    let flag = i.intern("qa-title-flag");
    i.specbind(flag, Value::Nil).unwrap();
    check(&i, &primary, "no");
    i.unbind_to(mark).unwrap();
    i.eval_str(r#"(setq frame-title-format '(8 (qa-title-flag (-3 "%b") "no")))"#)
        .unwrap();
    check(&i, &primary, "pri     ");
    i.eval_str(r#"(setq frame-title-format '(multiple-frames "%b" ("" invocation-name "@" system-name)) multiple-frames t)"#).unwrap();
    check(&i, &primary, "test-remacs@test-host");
    let Value::Frame(other) = i
        .eval_str(r#"(make-frame '((window-system . ns) (width . 48) (height . 12)))"#)
        .unwrap()
    else {
        panic!("other frame")
    };
    i.eval_str(
        r#"(progn
        (with-current-buffer (get-buffer-create "other-title") (setq-local qa-title-flag nil))
        (setq qa-title-other-frame (car (delq (selected-frame) (frame-list))))
        (set-window-buffer (frame-selected-window qa-title-other-frame) "other-title")
        (setq multiple-frames nil))"#,
    )
    .unwrap();
    check(&i, &primary, "primary-title");
    check(&i, &other, "other-title");
    i.eval_str(r#"(setq frame-title-format '(qa-title-flag-alias "yes" "no"))"#)
        .unwrap();
    check(&i, &primary, "yes");
    check(&i, &other, "no");
    i.eval_str(r#"(setq frame-title-format '(multiple-frames "many" "one"))"#)
        .unwrap();
    i.eval_str("(make-frame-invisible qa-title-other-frame t)")
        .unwrap();
    check(&i, &primary, "one");
    i.eval_str("(iconify-frame qa-title-other-frame)").unwrap();
    check(&i, &primary, "many");
    i.eval_str("(make-frame-visible qa-title-other-frame)")
        .unwrap();
    check(&i, &primary, "many");
    i.eval_str("(set-frame-parameter qa-title-other-frame 'parent-frame (selected-frame))")
        .unwrap();
    check(&i, &primary, "one");
    i.eval_str("(progn (set-frame-parameter qa-title-other-frame 'parent-frame nil) (set-frame-parameter qa-title-other-frame 'tooltip t))").unwrap();
    check(&i, &primary, "one");
    i.eval_str("(set-frame-parameter qa-title-other-frame 'tooltip nil)")
        .unwrap();
    check(&i, &primary, "many");
    i.eval_str("(delete-frame qa-title-other-frame)").unwrap();
    check(&i, &primary, "one");
    // Frontend-neutral minibuffer-only frame metadata. The public constructor
    // currently cannot change minibuffer kind after creating the base frame.
    let buffer = i.create_buffer(" *title-mini-only*");
    let mini = Frame::new_tty(buffer, buffer, 48, 12);
    let only = i.intern("only");
    let minibuffer = i.intern("minibuffer");
    {
        let mut frame = mini.borrow_mut();
        let window = frame.minibuffer.as_ref().unwrap().clone();
        frame.selected = window.clone();
        frame.root = Some(window.clone());
        frame.windows = vec![window];
        frame.params = Value::list(vec![Value::Sym(minibuffer), Value::Sym(only)]);
    }
    i.frames.push(mini);
    check(&i, &primary, "one");
}
