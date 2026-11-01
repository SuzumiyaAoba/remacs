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
    let current = i.current_buffer;
    let point = i.current_buffer_ref().unwrap().borrow().point();
    let selected = i.selected_frame.as_ref().unwrap().clone();
    assert_eq!(
        render_frame_grid_with_input_cached(i, frame, 48, 12, None).title,
        expected
    );
    assert_eq!(i.current_buffer, current);
    assert_eq!(i.current_buffer_ref().unwrap().borrow().point(), point);
    assert!(std::rc::Rc::ptr_eq(
        i.selected_frame.as_ref().unwrap(),
        &selected
    ));
}

#[test]
fn frame_title_sizes_count_accessible_characters_and_share_gnu_decimal_units() {
    let mut i = Interp::new();
    eval(
        &mut i,
        r#"(progn
        (set-frame-parameter nil 'window-system 'ns)
        (set-frame-parameter nil 'menu-bar-lines 0)
        (set-frame-parameter nil 'tool-bar-lines 0)
        (set-frame-parameter nil 'tab-bar-lines 0)
        (set-frame-size nil 48 12)
        (setq-default mode-line-format nil header-line-format nil tab-line-format nil truncate-lines t)
        (setq frame-title-format "%i|%I|%%i|%%I")
        (switch-to-buffer (get-buffer-create "size-primary"))
        (setq buffer-undo-list t))"#,
    );
    let primary = i.selected_frame.as_ref().unwrap().clone();
    let Value::Frame(other) = eval(
        &mut i,
        r#"(setq qa-size-other
        (make-frame '((window-system . ns) (width . 48) (height . 12))))"#,
    ) else {
        panic!("other frame")
    };
    eval(
        &mut i,
        r#"(progn
        (with-current-buffer (get-buffer-create "size-other") (insert (make-string 37 ?z)))
        (set-window-buffer (frame-selected-window qa-size-other) "size-other")
        (set-buffer (get-buffer-create "size-unrelated"))
        (insert "caller!"))"#,
    );
    // GNU 31.1 interactive reference covers decimal half-up rounding and
    // promotion to the next unit, independently of the buffer's byte length.
    for (size, readable) in [
        (0, "0"),
        (999, "999"),
        (1000, "1.0k"),
        (1049, "1.0k"),
        (1050, "1.1k"),
        (9949, "9.9k"),
        (9950, "10k"),
        (10500, "11k"),
        (999499, "999k"),
        (999500, "1.0M"),
        (1000000, "1.0M"),
    ] {
        eval(
            &mut i,
            &format!(
                r#"(with-current-buffer "size-primary"
            (widen) (erase-buffer) (insert (make-string {size} ?a)) (goto-char 1))"#
            ),
        );
        check(&i, &primary, &format!("{size}|{readable}|%i|%I"));
        check(&i, &other, "37|37|%i|%I");
    }
    eval(
        &mut i,
        r#"(with-current-buffer "size-primary"
        (erase-buffer) (insert (make-string 1000 ?日)) (goto-char 1))"#,
    );
    check(&i, &primary, "1000|1.0k|%i|%I");
    eval(&mut i, r#"(setq frame-title-format '(-2 "%I"))"#);
    check(&i, &primary, "1.");
    eval(&mut i, r#"(setq frame-title-format '(8 "%I"))"#);
    check(&i, &primary, "1.0k    ");
    eval(&mut i, r#"(setq frame-title-format "%i|%I|%%i|%%I")"#);
    eval(
        &mut i,
        r#"(with-current-buffer "size-primary" (narrow-to-region 101 201))"#,
    );
    check(&i, &primary, "100|100|%i|%I");
    eval(
        &mut i,
        r#"(with-current-buffer "size-primary" (narrow-to-region 101 101))"#,
    );
    check(&i, &primary, "0|0|%i|%I");
    eval(&mut i, r#"(with-current-buffer "size-primary" (widen))"#);
    check(&i, &primary, "1000|1.0k|%i|%I");
    eval(
        &mut i,
        r#"(with-current-buffer "size-primary" (erase-buffer) (insert "日🙂é\n"))"#,
    );
    check(&i, &primary, "5|5|%i|%I");
    eval(
        &mut i,
        r#"(with-current-buffer "size-primary"
        (erase-buffer) (set-buffer-multibyte nil) (insert (unibyte-string 128 97 255)))"#,
    );
    check(&i, &primary, "3|3|%i|%I");
    eval(
        &mut i,
        r#"(setq global-mode-string "%i/%I" frame-title-format "global:%M")"#,
    );
    check(&i, &primary, "global:3/3");
    check(&i, &other, "global:37/37");
    eval(
        &mut i,
        r#"(progn
        (setq frame-title-format "%i|%I|%%i|%%I")
        (with-current-buffer (get-buffer-create "size-side") (insert (make-string 41 ?s)))
        (setq qa-size-side (split-window-right))
        (set-window-buffer qa-size-side "size-side")
        (select-window qa-size-side)
        (set-buffer "size-unrelated"))"#,
    );
    check(&i, &primary, "41|41|%i|%I");
    check(&i, &other, "37|37|%i|%I");
}
