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
fn check(i: &Interp, frame: &FrameRef, title: &str, state: &str) {
    let current = i.current_buffer;
    let point = i.current_buffer_ref().unwrap().borrow().point();
    let selected = i.selected_frame.as_ref().unwrap().clone();
    let grid = render_frame_grid_with_input_cached(i, frame, 40, 12, None);
    assert_eq!(grid.title, title);
    for prefix in ["M", "H", "T"] {
        let expected = format!("{prefix}:{state}|");
        assert!(
            grid.rows.iter().any(|row| row.trim_end() == expected),
            "missing {expected:?}: {:?}",
            grid.rows
        );
    }
    assert_eq!(i.current_buffer, current);
    assert_eq!(i.current_buffer_ref().unwrap().borrow().point(), point);
    assert!(std::rc::Rc::ptr_eq(
        i.selected_frame.as_ref().unwrap(),
        &selected
    ));
}

#[test]
fn frame_title_state_markers_honor_read_only_priority_and_modified_ticks() {
    let mut i = Interp::new();
    eval(
        &mut i,
        r#"(progn
        (set-frame-parameter nil 'window-system 'ns)
        (set-frame-parameter nil 'menu-bar-lines 0)
        (set-frame-parameter nil 'tool-bar-lines 0)
        (set-frame-parameter nil 'tab-bar-lines 0)
        (set-frame-size nil 40 12)
        (setq frame-title-format "%*|%+|%&")
        (setq-default mode-line-format "M:%*|%+|%&|"
                      header-line-format "H:%*|%+|%&|" tab-line-format "T:%*|%+|%&|")
        (switch-to-buffer (get-buffer-create "state-primary"))
        (insert "primary") (set-buffer-modified-p nil))"#,
    );
    let primary = i.selected_frame.as_ref().unwrap().clone();
    let Value::Frame(other) = eval(
        &mut i,
        r#"(setq qa-state-other
        (make-frame '((window-system . ns) (width . 40) (height . 12)
                      (menu-bar-lines . 0) (tool-bar-lines . 0) (tab-bar-lines . 0))))"#,
    ) else {
        panic!("other frame")
    };
    eval(
        &mut i,
        r#"(progn
        (with-current-buffer (get-buffer-create "state-other")
          (insert "other") (setq buffer-read-only t) (set-buffer-modified-p t))
        (set-window-buffer (frame-selected-window qa-state-other) "state-other")
        (set-buffer (get-buffer-create "state-unrelated"))
        (setq buffer-read-only t))"#,
    );
    // GNU 31.1 interactive reference, including non-boolean read-only values.
    for (read_only, modified, expected) in [
        ("nil", "nil", "-|-|-"),
        ("nil", "t", "*|*|*"),
        ("t", "nil", "%|%|-"),
        ("t", "t", "%|*|*"),
        ("0", "nil", "%|%|-"),
        ("0", "t", "%|*|*"),
        ("locked", "nil", "%|%|-"),
        ("locked", "t", "%|*|*"),
    ] {
        eval(
            &mut i,
            &format!(
                r#"(with-current-buffer "state-primary"
            (setq buffer-read-only '{read_only}) (set-buffer-modified-p {modified}))"#
            ),
        );
        check(&i, &primary, expected, expected);
        check(&i, &other, "%|*|*", "%|*|*");
    }
    eval(&mut i, r#"(setq frame-title-format "%%*|%%+|%%&")"#);
    check(&i, &primary, "%*|%+|%&", "%|*|*");
    eval(&mut i, r#"(setq frame-title-format '(-2 "%*|%+|%&"))"#);
    check(&i, &primary, "%|", "%|*|*");
    eval(&mut i, r#"(setq frame-title-format '(8 "%*|%+|%&"))"#);
    check(&i, &primary, "%|*|*   ", "%|*|*");
    eval(
        &mut i,
        r#"(progn
        (setq frame-title-format "%*|%+|%&")
        (set-buffer "state-primary") (setq buffer-read-only nil) (set-buffer-modified-p nil))"#,
    );
    let mark = i.specbind_depth();
    let read_only = i.intern("buffer-read-only");
    i.specbind(read_only, Value::t()).unwrap();
    check(&i, &primary, "%|%|-", "%|%|-");
    check(&i, &other, "%|*|*", "%|*|*");
    i.unbind_to(mark).unwrap();
    check(&i, &primary, "-|-|-", "-|-|-");
    // Real editing and saving ticks drive the same markers as explicit flags.
    eval(
        &mut i,
        r#"(progn (goto-char (point-max)) (insert " edit"))"#,
    );
    check(&i, &primary, "*|*|*", "*|*|*");
    eval(&mut i, "(setq buffer-read-only t)");
    check(&i, &primary, "%|*|*", "%|*|*");
    eval(&mut i, "(set-buffer-modified-p nil)");
    check(&i, &primary, "%|%|-", "%|%|-");
    eval(&mut i, "(setq buffer-read-only nil)");
    check(&i, &primary, "-|-|-", "-|-|-");
    eval(
        &mut i,
        r#"(setq global-mode-string "%*|%+|%&" frame-title-format "global:%M")"#,
    );
    check(&i, &primary, "global:-|-|-", "-|-|-");
    check(&i, &other, "global:%|*|*", "%|*|*");
}
