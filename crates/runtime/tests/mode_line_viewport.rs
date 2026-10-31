use remacs_runtime::{
    frontend::{Grid, render_grid},
    lisp::{Flow, Interp, Value},
};

fn eval(interp: &mut Interp, source: &str) -> Value {
    interp.eval_str(source).unwrap_or_else(|flow| match flow {
        Flow::Signal(signal) => panic!(
            "{} {}",
            interp.prin1_to_string(&signal.symbol()),
            interp.prin1_to_string(&signal.data())
        ),
        other => panic!("{other:?}"),
    })
}

fn mode_line(grid: &Grid, window: Option<usize>) -> String {
    let line = grid
        .mode_lines
        .iter()
        .find(|line| window.is_none_or(|window| line.window_id == window))
        .expect("rendered mode line");
    grid.rows[line.row]
        .chars()
        .skip(line.left)
        .take(line.width)
        .collect::<String>()
        .trim_end()
        .to_owned()
}

fn check_single(interp: &Interp, expected: &str, start: usize, end: usize) {
    let grid = render_grid(interp, 20, 8);
    assert_eq!(grid.mode_lines.len(), 1);
    assert_eq!(mode_line(&grid, None), expected);
    let frame = interp.selected_frame.as_ref().unwrap().borrow();
    let window = frame.selected.borrow();
    assert_eq!(window.start, start);
    assert_eq!(window.redisplay_end.unwrap().3, end);
}

#[test]
fn mode_line_percentages_follow_each_windows_displayed_range() {
    let mut interp = Interp::new();
    eval(
        &mut interp,
        r#"(progn
      (set-frame-size nil 20 8)
      (erase-buffer) (dotimes (_ 100) (insert "aaaa\n"))
      (setq-local mode-line-format "%p / %P")
      (setq-local header-line-format nil)
      (setq-local tab-line-format nil)
      (setq-local truncate-lines t)
      (setq-local display-line-numbers nil)
      (setq-local scroll-margin 0)
      (goto-char 1) (set-window-start nil 1))"#,
    );
    check_single(&interp, "Top / Top 6%", 0, 30);

    // Moving point inside the same displayed range must not change percentages.
    eval(&mut interp, "(goto-char 21)");
    check_single(&interp, "Top / Top 6%", 0, 30);
    eval(
        &mut interp,
        "(setq-local mode-line-format \"%l:%c|%p / %P\")",
    );
    check_single(&interp, "5:0|Top / Top 6%", 0, 30);

    eval(
        &mut interp,
        "(progn (setq-local mode-line-format \"%p / %P\") (set-window-start nil 11))",
    );
    check_single(&interp, " 2% /  8%", 10, 40);
    eval(
        &mut interp,
        "(progn (goto-char 221) (set-window-start nil 201))",
    );
    check_single(&interp, "40% / 46%", 200, 230);
    // Scrolling while point stays put changes both bounds.
    eval(&mut interp, "(set-window-start nil 211)");
    check_single(&interp, "42% / 48%", 210, 240);
    eval(
        &mut interp,
        "(progn (goto-char 486) (set-window-start nil 476))",
    );
    check_single(&interp, "Bottom / Bottom", 475, 500);

    eval(
        &mut interp,
        "(progn (narrow-to-region 101 401) (goto-char 221) (set-window-start nil 201))",
    );
    check_single(&interp, "34% / 44%", 200, 230);

    eval(
        &mut interp,
        r#"(progn (widen) (erase-buffer)
      (dotimes (_ 3) (insert "aaaa\n"))
      (goto-char 6) (set-window-start nil 1))"#,
    );
    check_single(&interp, "All / All", 0, 15);
    eval(
        &mut interp,
        "(progn (erase-buffer) (set-window-start nil 1))",
    );
    check_single(&interp, "All / All", 0, 0);

    // A wrapped line's visible end is a character position, not its physical EOL.
    eval(
        &mut interp,
        "(progn (insert (make-string 121 ?x)) (setq-local truncate-lines nil) (goto-char 1) (set-window-start nil 1))",
    );
    check_single(&interp, "Top / Top99%", 0, 120);
    eval(
        &mut interp,
        "(progn (erase-buffer) (insert (make-string 121 ?日)) (goto-char 1) (set-window-start nil 1))",
    );
    check_single(&interp, "Top / Top50%", 0, 60);

    // Separate windows on the same buffer have independent display ranges.
    eval(
        &mut interp,
        r#"(progn (erase-buffer)
      (dotimes (_ 100) (insert "aaaa\n"))
      (setq-local truncate-lines t)
      (set-frame-size nil 20 12)
      (goto-char 1) (set-window-start nil 1))"#,
    );
    // Apply the same layout update as the frontend before splitting.
    remacs_runtime::editor::reflow_frame_windows(interp.selected_frame.as_ref().unwrap(), 20, 12);
    let upper = interp
        .selected_frame
        .as_ref()
        .unwrap()
        .borrow()
        .selected
        .borrow()
        .id;
    let bottom = eval(&mut interp, "(split-window-below 6)");
    let Value::Window(bottom) = bottom else {
        panic!("new window");
    };
    let bottom_id = bottom.borrow().id;
    eval(
        &mut interp,
        "(let ((window (next-window))) (set-window-start window 201) (set-window-point window 211))",
    );
    let grid = render_grid(&interp, 20, 12);
    assert_eq!(grid.mode_lines.len(), 2);
    assert_eq!(mode_line(&grid, Some(upper)), "Top / Top 5%");
    assert_eq!(mode_line(&grid, Some(bottom_id)), "40% / 44%");
    let frame = interp.selected_frame.as_ref().unwrap().borrow();
    let upper_window = frame
        .windows
        .iter()
        .find(|window| window.borrow().id == upper)
        .unwrap();
    assert_eq!(upper_window.borrow().redisplay_end.unwrap().3, 25);
    assert_eq!(bottom.borrow().redisplay_end.unwrap().3, 220);
}
