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
fn check(i: &Interp, frame: &FrameRef, title: &str, coordinates: &str) {
    let caller = i.current_buffer;
    let point = i.current_buffer_ref().unwrap().borrow().point();
    let selected = i.selected_frame.as_ref().unwrap().clone();
    let grid = render_frame_grid_with_input_cached(i, frame, 40, 12, None);
    assert_eq!(grid.title, title);
    for prefix in ["M", "H", "T"] {
        let expected = format!("{prefix}:{coordinates}|");
        assert!(
            grid.rows.iter().any(|row| row.trim_end() == expected),
            "missing {expected:?}: {:?}",
            grid.rows
        );
    }
    assert_eq!(i.current_buffer, caller);
    assert_eq!(i.current_buffer_ref().unwrap().borrow().point(), point);
    assert!(std::rc::Rc::ptr_eq(
        i.selected_frame.as_ref().unwrap(),
        &selected
    ));
}

#[test]
fn frame_titles_ignore_coordinates_but_keep_mode_lines_escapes_and_padding() {
    let mut i = Interp::new();
    eval(
        &mut i,
        r#"(progn
        (set-frame-parameter nil 'window-system 'ns)
        (set-frame-parameter nil 'menu-bar-lines 0)
        (set-frame-parameter nil 'tool-bar-lines 0)
        (set-frame-parameter nil 'tab-bar-lines 0)
        (set-frame-size nil 40 12)
        (setq-default mode-line-format "M:%c|%C|%l|"
                      header-line-format "H:%c|%C|%l|" tab-line-format "T:%c|%C|%l|")
        (switch-to-buffer (get-buffer-create "coordinates-primary"))
        (insert "one\nabcd\nend\n") (goto-char 7)
        (setq frame-title-format "Before %c%C%l After"))"#,
    );
    let primary = i.selected_frame.as_ref().unwrap().clone();
    let Value::Frame(other) = eval(
        &mut i,
        r#"(setq qa-coordinate-other
        (make-frame '((window-system . ns) (width . 40) (height . 12)
                      (menu-bar-lines . 0) (tool-bar-lines . 0) (tab-bar-lines . 0))))"#,
    ) else {
        panic!("other frame")
    };
    eval(
        &mut i,
        r#"(progn
        (with-current-buffer (get-buffer-create "coordinates-other")
          (insert "x\nyz\n") (goto-char 4))
        (set-window-buffer (frame-selected-window qa-coordinate-other) "coordinates-other")
        (set-window-point (frame-selected-window qa-coordinate-other) 4)
        (set-buffer (get-buffer-create "coordinates-unrelated"))
        (insert "caller") (goto-char 3))"#,
    );
    // GNU xdisp.c uses empty strings specifically for MODE_LINE_TITLE.
    // format-mode-line's NOPROP target intentionally still shows coordinates.
    for (form, expected) in [
        (r#""Before %c%C%l After""#, "Before  After"),
        (r#""%c|%C|%l""#, "||"),
        (r#""%%c|%%C|%%l""#, "%c|%C|%l"),
        (r#""[%5c][%3C][%2l]""#, "[     ][   ][  ]"),
        (r#"(6 "%c%C%l")"#, "      "),
        (r#"(-1 "%cX")"#, "X"),
        (r#"(-2 "[%4c]")"#, "[    "),
    ] {
        eval(&mut i, &format!("(setq frame-title-format '{form})"));
        check(&i, &primary, expected, "2|3|2");
        check(&i, &other, expected, "1|2|2");
    }
    eval(
        &mut i,
        r#"(progn
        (setq frame-title-format "Before %c%C%l After")
        (with-current-buffer "coordinates-primary" (goto-char 11)))"#,
    );
    check(&i, &primary, "Before  After", "1|2|3");
    check(&i, &other, "Before  After", "1|2|2");
    eval(
        &mut i,
        r#"(setq global-mode-string "%c:%C:%l" frame-title-format "global:%M")"#,
    );
    check(&i, &primary, "global:::", "1|2|3");
    eval(
        &mut i,
        r#"(progn
        (defvar qa-coordinate-literal "%c%C%l")
        (setq frame-title-format '("" qa-coordinate-literal)))"#,
    );
    check(&i, &primary, "%c%C%l", "1|2|3");
    eval(
        &mut i,
        r#"(set-frame-parameter nil 'title "Pinned %c%C%l")"#,
    );
    check(&i, &primary, "Pinned %c%C%l", "1|2|3");
}
