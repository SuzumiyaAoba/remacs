use remacs_runtime::{
    editor::FrameRef,
    frontend::{KeyIo, render_frame_grid_with_input_cached},
    lisp::{Flow, Interp, Value},
};
use std::{io, time::Duration};

struct Frontend;
impl KeyIo for Frontend {
    fn poll_key(&mut self, _: Duration) -> io::Result<Option<i128>> {
        Ok(None)
    }
    fn unread(&mut self, _: i128) {}
    fn render(&mut self, _: &Interp) -> io::Result<()> {
        Ok(())
    }
    fn draw_echo(&mut self, _: &str) -> io::Result<()> {
        Ok(())
    }
    fn size(&self) -> (usize, usize) {
        (40, 8)
    }
}

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
fn check(i: &Interp, frame: &FrameRef, expected: &str, start: usize, end: usize) {
    let caller = i.current_buffer;
    let point = i.current_buffer_ref().unwrap().borrow().point();
    let selected = i.selected_frame.as_ref().unwrap().clone();
    let window = frame.borrow().selected.clone();
    let grid = render_frame_grid_with_input_cached(i, frame, 40, 8, None);
    assert_eq!(window.borrow().start, start);
    assert_eq!(window.borrow().redisplay_end.unwrap().3, end);
    // The first render after a scroll/resize/edit must use this new endpoint.
    assert_eq!(grid.title, expected);
    let line = grid
        .mode_lines
        .iter()
        .find(|line| line.window_id == window.borrow().id)
        .unwrap();
    let text: String = grid.rows[line.row]
        .chars()
        .skip(line.left)
        .take(line.width)
        .collect();
    let visible: String = expected.chars().take(line.width).collect();
    assert_eq!(text.trim_end(), visible.trim_end());
    assert_eq!(i.current_buffer, caller);
    assert_eq!(i.current_buffer_ref().unwrap().borrow().point(), point);
    assert!(std::rc::Rc::ptr_eq(
        i.selected_frame.as_ref().unwrap(),
        &selected
    ));
}

#[test]
fn frame_title_positions_follow_actual_window_viewports_and_remote_dependencies() {
    let mut i = Interp::new();
    eval(
        &mut i,
        r#"(progn
        (set-frame-parameter nil 'window-system 'ns)
        (set-frame-parameter nil 'menu-bar-lines 0)
        (set-frame-parameter nil 'tool-bar-lines 0)
        (set-frame-parameter nil 'tab-bar-lines 0)
        (set-frame-parameter nil 'left-fringe 0)
        (set-frame-parameter nil 'right-fringe 0)
        (set-frame-size nil 40 8)
        (set-window-fringes nil 0 0)
        (setq-default mode-line-format "%p|%P|%o|%q|" header-line-format nil tab-line-format nil
                      truncate-lines t scroll-margin 0)
        (setq frame-title-format "%p|%P|%o|%q|")
        (switch-to-buffer (get-buffer-create "position-primary"))
        (set-window-scroll-bars nil 0 nil 0 nil t)
        (setq buffer-undo-list t)
        (dotimes (_ 100) (insert "aaaa\n"))
        (goto-char 1) (set-window-start nil 1)
        (setq qa-position-primary-window (selected-window)))"#,
    );
    let primary = i.selected_frame.as_ref().unwrap().clone();
    let Value::Frame(other) = eval(
        &mut i,
        r#"(setq qa-position-other
        (make-frame '((window-system . ns) (width . 40) (height . 8)
                      (menu-bar-lines . 0) (tool-bar-lines . 0) (tab-bar-lines . 0)
                      (left-fringe . 0) (right-fringe . 0))))"#,
    ) else {
        panic!("other frame")
    };
    eval(
        &mut i,
        r#"(progn
        (set-window-buffer (frame-selected-window qa-position-other) "position-primary")
        (set-window-start (frame-selected-window qa-position-other) 201)
        (set-window-point (frame-selected-window qa-position-other) 211)
        (set-buffer (get-buffer-create "position-unrelated")) (insert "caller"))"#,
    );
    check(&i, &primary, "Top|Top 6%|Top|0-6%|", 0, 30);
    check(&i, &other, "40%|46%|43%|40-46%|", 200, 230);
    // GNU 31.1 interactive reference with six visible text rows.
    for (forms, expected, start, end) in [
        (
            "(goto-char 21) (set-window-start nil 11)",
            " 2%| 8%| 3%|2-8%|",
            10,
            40,
        ),
        (
            "(goto-char 221) (set-window-start nil 201)",
            "40%|46%|43%|40-46%|",
            200,
            230,
        ),
        (
            "(set-window-start nil 211)",
            "42%|48%|45%|42-48%|",
            210,
            240,
        ),
        (
            "(goto-char 486) (set-window-start nil 476)",
            "Bottom|Bottom|Bottom|95-100%|",
            475,
            500,
        ),
        (
            "(narrow-to-region 101 401) (goto-char 221) (set-window-start nil 201)",
            "34%|44%|38%|34-44%|",
            200,
            230,
        ),
        (
            r#"(widen) (erase-buffer) (dotimes (_ 3) (insert "aaaa\n")) (goto-char 6) (set-window-start nil 1)"#,
            "All|All|All|All   |",
            0,
            15,
        ),
        (
            "(erase-buffer) (set-window-start nil 1)",
            "All|All|All|All   |",
            0,
            0,
        ),
    ] {
        eval(
            &mut i,
            &format!("(with-current-buffer \"position-primary\" {forms})"),
        );
        check(&i, &primary, expected, start, end);
    }
    eval(
        &mut i,
        r#"(with-current-buffer "position-primary"
        (insert (make-string 241 ?x)) (setq truncate-lines nil) (goto-char 1) (set-window-start nil 1))"#,
    );
    check(&i, &primary, "Top|Top99%|Top|0-99%|", 0, 240);
    eval(
        &mut i,
        r#"(with-current-buffer "position-primary"
        (erase-buffer) (insert (make-string 241 ?日)) (goto-char 1) (set-window-start nil 1))"#,
    );
    check(&i, &primary, "Top|Top50%|Top|0-50%|", 0, 120);
    eval(
        &mut i,
        r#"(with-current-buffer "position-primary"
        (erase-buffer) (insert (make-string 10000 ?\n)) (setq truncate-lines t)
        (goto-char 1007) (set-window-start nil 1006))"#,
    );
    check(&i, &primary, "11%|11%|11%|11%|", 1005, 1011);

    eval(
        &mut i,
        r#"(progn
        (with-current-buffer "position-primary"
          (erase-buffer) (dotimes (_ 100) (insert "aaaa\n"))
          (goto-char 1) (set-window-start nil 1))
        (set-window-start (frame-selected-window qa-position-other) 201)
        (set-window-point (frame-selected-window qa-position-other) 211)
        (with-current-buffer (get-buffer-create "position-side")
          (dotimes (_ 200) (insert "aaaa\n")))
        (setq qa-position-side (split-window-right))
        (set-window-buffer qa-position-side "position-side")
        (set-window-start qa-position-side 301) (set-window-point qa-position-side 311)
        (select-window qa-position-side) (set-buffer "position-unrelated"))"#,
    );
    check(&i, &primary, "30%|33%|31%|30-33%|", 300, 330);
    check(&i, &other, "40%|46%|43%|40-46%|", 200, 230);

    // A position construct's width determines whether the following %@ is
    // reachable. Preparation must use the newly rendered range, not old data.
    eval(
        &mut i,
        r#"(progn
        (select-window qa-position-primary-window)
        (with-current-buffer "position-primary"
          (goto-char 1) (set-window-start nil 1) (setq default-directory "/claimed/path/"))
        (set-buffer "position-unrelated")
        (setq pre-redisplay-function nil frame-title-format '(-4 "%P%@") qa-position-calls 0)
        (defun qa-position-handler (operation _name &rest _)
          (when (eq operation 'file-remote-p)
            (setq qa-position-calls (1+ qa-position-calls)) "/mock:host:"))
        (setq file-name-handler-alist '(("\\`/claimed/" . qa-position-handler))))"#,
    );
    let windows = primary.borrow().windows.clone();
    Frontend
        .run_pre_redisplay_for_windows(&mut i, windows)
        .unwrap();
    assert_eq!(
        render_frame_grid_with_input_cached(&i, &primary, 40, 8, None).title,
        "Top "
    );
    assert_eq!(eval(&mut i, "qa-position-calls").int(), Some(0));
    eval(
        &mut i,
        r#"(with-current-buffer "position-primary" (goto-char 221) (set-window-start nil 201))"#,
    );
    let windows = primary.borrow().windows.clone();
    Frontend
        .run_pre_redisplay_for_windows(&mut i, windows)
        .unwrap();
    assert_eq!(
        render_frame_grid_with_input_cached(&i, &primary, 40, 8, None).title,
        "46%@"
    );
    assert_eq!(eval(&mut i, "qa-position-calls").int(), Some(1));
    eval(
        &mut i,
        r#"(setq global-mode-string "%p|%P|%o|%q|" frame-title-format "[%M]")"#,
    );
    assert_eq!(
        render_frame_grid_with_input_cached(&i, &primary, 40, 8, None).title,
        "[40%|46%|43%|40-46%|]"
    );
    eval(&mut i, r#"(setq frame-title-format "[%p]|%%p|[%5p]")"#);
    assert_eq!(
        render_frame_grid_with_input_cached(&i, &primary, 40, 8, None).title,
        "[40%]|%p|[40%  ]"
    );
    eval(
        &mut i,
        r#"(progn
        (with-current-buffer "position-primary"
          (goto-char 486) (set-window-start nil 476))
        (setq frame-title-format '(-4 "%p%@") qa-position-calls 0))"#,
    );
    let windows = primary.borrow().windows.clone();
    Frontend
        .run_pre_redisplay_for_windows(&mut i, windows)
        .unwrap();
    assert_eq!(
        render_frame_grid_with_input_cached(&i, &primary, 40, 8, None).title,
        "Bott"
    );
    assert_eq!(eval(&mut i, "qa-position-calls").int(), Some(0));
}
