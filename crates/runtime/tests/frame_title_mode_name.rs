use remacs_runtime::{
    editor::FrameRef,
    frontend::render_frame_grid_with_input_cached,
    lisp::{Flow, Interp, Value},
};

fn eval(i: &mut Interp, source: &str) -> Value {
    i.eval_str(source).unwrap_or_else(|flow| match flow {
        Flow::Signal(signal) => panic!(
            "{source}: {} {}",
            i.prin1_to_string(&signal.symbol()),
            i.prin1_to_string(&signal.data())
        ),
        other => panic!("{source}: {other:?}"),
    })
}

#[track_caller]
fn check(i: &Interp, frame: &FrameRef, expected: &str) {
    let caller = i.current_buffer;
    let point = i.current_buffer_ref().unwrap().borrow().point();
    let selected = i.selected_frame.as_ref().unwrap().clone();
    assert_eq!(
        render_frame_grid_with_input_cached(i, frame, 60, 12, None).title,
        expected
    );
    assert_eq!(i.current_buffer, caller);
    assert_eq!(i.current_buffer_ref().unwrap().borrow().point(), point);
    assert!(std::rc::Rc::ptr_eq(
        i.selected_frame.as_ref().unwrap(),
        &selected
    ));
}

#[test]
fn frame_title_mode_name_uses_target_buffer_literal_name_and_tracks_mode_changes() {
    let mut i = Interp::new();
    eval(
        &mut i,
        r#"(progn
        (set-frame-parameter nil 'window-system 'ns)
        (set-frame-parameter nil 'menu-bar-lines 0)
        (set-frame-parameter nil 'tool-bar-lines 0)
        (set-frame-size nil 60 12)
        (setq-default mode-line-format nil header-line-format nil tab-line-format nil)
        (setq frame-title-format "%m — %b")
        (switch-to-buffer (get-buffer-create "mode-primary"))
        (fundamental-mode))"#,
    );
    let primary = i.selected_frame.as_ref().unwrap().clone();
    let Value::Frame(other) = eval(
        &mut i,
        r#"(setq qa-mode-other
          (make-frame '((window-system . ns) (width . 60) (height . 12)
                        (menu-bar-lines . 0) (tool-bar-lines . 0))))"#,
    ) else {
        panic!("other frame")
    };
    eval(
        &mut i,
        r#"(progn
        (with-current-buffer (get-buffer-create "mode-other")
          (setq mode-name "Other"))
        (set-window-buffer (frame-selected-window qa-mode-other) "mode-other")
        (set-buffer (get-buffer-create "mode-caller"))
        (insert "caller") (goto-char 3) (setq mode-name "Caller"))"#,
    );
    check(&i, &primary, "Fundamental — mode-primary");
    check(&i, &other, "Other — mode-other");
    eval(
        &mut i,
        r#"(with-current-buffer "mode-primary" (text-mode))"#,
    );
    check(&i, &primary, "Text — mode-primary");
    check(&i, &other, "Other — mode-other");
    eval(&mut i, r#"(set-buffer "mode-primary")"#);
    let mark = i.specbind_depth();
    let mode_name = i.intern("mode-name");
    i.specbind(mode_name, Value::string("Bound")).unwrap();
    check(&i, &primary, "Bound — mode-primary");
    check(&i, &other, "Other — mode-other");
    i.unbind_to(mark).unwrap();
    check(&i, &primary, "Text — mode-primary");
    eval(&mut i, r#"(set-buffer "mode-caller")"#);
    // GNU decode_mode_spec returns only string mode-names, literally. It
    // neither expands percent constructs nor evaluates non-string values.
    for (form, expected) in [
        (r#""Custom""#, "Custom — mode-primary"),
        (r#""Mode %b/%m/%@""#, "Mode %b/%m/%@ — mode-primary"),
        (
            r#"(propertize "Styled" 'face 'bold)"#,
            "Styled — mode-primary",
        ),
        ("nil", " — mode-primary"),
        ("42", " — mode-primary"),
        ("'mode-caller", " — mode-primary"),
        (r#"'("List")"#, " — mode-primary"),
        ("'(:eval (error \"must not evaluate\"))", " — mode-primary"),
    ] {
        eval(
            &mut i,
            &format!("(with-current-buffer \"mode-primary\" (setq mode-name {form}))"),
        );
        check(&i, &primary, expected);
        check(&i, &other, "Other — mode-other");
    }
    eval(
        &mut i,
        r#"(progn
        (with-current-buffer "mode-primary" (setq mode-name "日本語"))
        (setq frame-title-format "[%8m]|%%m"))"#,
    );
    check(&i, &primary, "[日本語  ]|%m");
    check(&i, &other, "[Other   ]|%m");
    eval(
        &mut i,
        r#"(setq global-mode-string "%m" frame-title-format "global:%M")"#,
    );
    check(&i, &primary, "global:日本語");
    check(&i, &other, "global:Other");
    eval(
        &mut i,
        r#"(progn
        (setq frame-title-format "%m")
        (with-current-buffer (get-buffer-create "mode-side") (setq mode-name "Side"))
        (setq qa-mode-side (split-window-right))
        (set-window-buffer qa-mode-side "mode-side")
        (select-window qa-mode-side) (set-buffer "mode-caller"))"#,
    );
    check(&i, &primary, "Side");
    check(&i, &other, "Other");
}
