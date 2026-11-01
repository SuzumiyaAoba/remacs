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
fn check(i: &Interp, frame: &FrameRef, narrowed: bool) {
    let caller = i.current_buffer;
    let point = i.current_buffer_ref().unwrap().borrow().point();
    let selected = i.selected_frame.as_ref().unwrap().clone();
    let buffer_id = frame.borrow().selected.borrow().buffer;
    let buffer_ref = i.buffers.get(buffer_id).unwrap();
    let buffer = buffer_ref.borrow();
    let bounds = (buffer.begv, buffer.zv, buffer.size());
    let name = buffer.name.to_unicode_lossy();
    drop(buffer);
    let grid = render_frame_grid_with_input_cached(i, frame, 60, 12, None);
    let expected = if narrowed {
        "[ Narrow]|%n|[ Narrow ]"
    } else {
        "[]|%n|[        ]"
    };
    assert_eq!(grid.title, expected, "buffer {name:?}, bounds {bounds:?}");
    for prefix in ["M", "H", "T"] {
        let row = format!("{prefix}:[{}]", if narrowed { " Narrow" } else { "" });
        assert!(
            grid.rows.iter().any(|rendered| rendered.trim_end() == row),
            "missing {row:?}: {:?}",
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
fn frame_title_narrowing_uses_target_buffer_bounds_and_updates_after_widening() {
    let mut i = Interp::new();
    eval(
        &mut i,
        r#"(progn
        (set-frame-parameter nil 'window-system 'ns)
        (set-frame-parameter nil 'menu-bar-lines 0)
        (set-frame-parameter nil 'tool-bar-lines 0)
        (set-frame-parameter nil 'tab-bar-lines 0)
        (set-frame-size nil 60 12)
        (setq-default mode-line-format "M:[%n]"
                      header-line-format "H:[%n]" tab-line-format "T:[%n]")
        (setq frame-title-format "[%n]|%%n|[%8n]")
        (switch-to-buffer (get-buffer-create "narrow-primary"))
        (insert "a日本語\nxyz\n"))"#,
    );
    let primary = i.selected_frame.as_ref().unwrap().clone();
    let Value::Frame(other) = eval(
        &mut i,
        r#"(setq qa-narrow-other
          (make-frame '((window-system . ns) (width . 60) (height . 12)
                        (menu-bar-lines . 0) (tool-bar-lines . 0) (tab-bar-lines . 0))))"#,
    ) else {
        panic!("other frame")
    };
    eval(
        &mut i,
        r#"(progn
        (with-current-buffer (get-buffer-create "narrow-other")
          (insert "other buffer\n"))
        (set-window-buffer (frame-selected-window qa-narrow-other) "narrow-other")
        (set-buffer (get-buffer-create "narrow-caller"))
        (insert "caller") (narrow-to-region 2 4) (goto-char 3))"#,
    );
    // GNU xdisp.c compares BEGV/ZV with the full buffer and includes a
    // leading space. Narrowing to the entire buffer has no indicator.
    for (operation, narrowed) in [
        ("(narrow-to-region (point-min) (point-max))", false),
        ("(narrow-to-region 2 (point-max))", true),
        ("(widen)", false),
        ("(narrow-to-region 1 5)", true),
        ("(widen)", false),
        ("(narrow-to-region 3 3)", true),
        ("(widen)", false),
    ] {
        eval(
            &mut i,
            &format!("(with-current-buffer \"narrow-primary\" {operation})"),
        );
        check(&i, &primary, narrowed);
        check(&i, &other, false);
    }
    eval(
        &mut i,
        r#"(with-current-buffer "narrow-other" (narrow-to-region 2 5))"#,
    );
    check(&i, &primary, false);
    check(&i, &other, true);
}
