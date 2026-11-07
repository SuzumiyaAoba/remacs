use remacs_runtime::{
    editor::FrameRef,
    frontend::{Grid, KeyIo, render_frame_grid_with_input_cached},
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
        (60, 12)
    }
}
fn eval(i: &mut Interp, text: &str) -> Value {
    i.eval_str(text)
        .unwrap_or_else(|flow| panic!("{text}: {flow:?}"))
}
fn redraw(i: &mut Interp) -> Result<(), Flow> {
    let windows = i
        .frames
        .iter()
        .flat_map(|frame| frame.borrow().windows.clone())
        .collect();
    Frontend.run_pre_redisplay_for_windows(i, windows)
}
fn grid(i: &Interp, frame: &FrameRef) -> Grid {
    render_frame_grid_with_input_cached(i, frame, 60, 12, None)
}
#[track_caller]
fn check(i: &mut Interp, a: &FrameRef, b: &FrameRef, titles: [&str; 2]) {
    let caller = i.current_buffer;
    let point = i.current_buffer_ref().unwrap().borrow().point();
    let frame = i.selected_frame.as_ref().unwrap().clone();
    let selected = frame.borrow().selected.clone();
    let matches = eval(i, "(match-data t)");
    redraw(i).unwrap();
    assert_eq!(grid(i, a).title, titles[0]);
    assert_eq!(grid(i, b).title, titles[1]);
    assert_eq!(i.current_buffer, caller);
    assert_eq!(i.current_buffer_ref().unwrap().borrow().point(), point);
    assert!(std::rc::Rc::ptr_eq(
        i.selected_frame.as_ref().unwrap(),
        &frame
    ));
    assert!(std::rc::Rc::ptr_eq(&frame.borrow().selected, &selected));
    let after = eval(i, "(match-data t)");
    assert_eq!(
        i.prin1_to_string(&matches).to_unicode_lossy(),
        i.prin1_to_string(&after).to_unicode_lossy()
    );
}

#[test]
fn frame_title_eval_runs_once_in_frame_context_and_preserves_order_and_unwinds() {
    let mut i = Interp::new();
    eval(
        &mut i,
        r#"(progn
      (set-frame-parameter nil 'window-system 'ns)
      (set-frame-parameter nil 'menu-bar-lines 0)
      (set-frame-parameter nil 'tool-bar-lines 0)
      (set-frame-size nil 60 12)
      (setq pre-redisplay-function nil)
      (setq-default mode-line-format nil header-line-format nil tab-line-format nil)
      (switch-to-buffer (get-buffer-create "eval-primary")) (insert "abcdef") (goto-char 2)
      (setq frame-title-format '(:eval (concat "Current: " (buffer-name)))))"#,
    );
    let a = i.selected_frame.as_ref().unwrap().clone();
    let Value::Frame(b) = eval(
        &mut i,
        r#"(setq qa-title-other
      (make-frame '((window-system . ns) (width . 60) (height . 12)
                    (menu-bar-lines . 0) (tool-bar-lines . 0))))"#,
    ) else {
        panic!("frame")
    };
    eval(
        &mut i,
        r#"(progn
      (with-current-buffer (get-buffer-create "eval-other") (insert "uvwxyz") (goto-char 5))
      (set-window-buffer (frame-selected-window qa-title-other) "eval-other")
      (set-window-point (frame-selected-window qa-title-other) 4)
      (set-buffer (get-buffer-create "eval-caller")) (insert "caller") (goto-char 3)
      (string-match "aa" "baaz"))"#,
    );
    check(
        &mut i,
        &a,
        &b,
        ["Current: eval-primary", "Current: eval-other"],
    );
    eval(
        &mut i,
        r#"(progn (setq qa-title-count 0)
      (setq frame-title-format '(:eval (format "%s:%s:%s" (buffer-name) (point)
             (setq qa-title-count (1+ qa-title-count))))))"#,
    );
    check(&mut i, &a, &b, ["eval-primary:2:1", "eval-other:4:2"]);
    assert_eq!(grid(&i, &a).title, "eval-primary:2:1");
    assert_eq!(grid(&i, &b).title, "eval-other:4:2");
    assert_eq!(eval(&mut i, "qa-title-count").int(), Some(2));
    check(&mut i, &a, &b, ["eval-primary:2:3", "eval-other:4:4"]);

    for (format, expected, calls) in [
        (
            r#"'(:eval '("nested:" (:eval "%b")))"#,
            ["nested:eval-primary", "nested:eval-other"],
            0,
        ),
        (
            r#"'(qa-title-on (:eval (progn (setq qa-title-count (1+ qa-title-count)) "on")) "off")"#,
            ["off", "off"],
            0,
        ),
        (
            r#"'(-3 ("abc" (:eval (setq qa-title-count 99))))"#,
            ["abc", "abc"],
            0,
        ),
        (
            r#"'(-4 ("ab" (:eval (progn (setq qa-title-count (1+ qa-title-count)) "cdef")) (:eval (setq qa-title-count 99))))"#,
            ["abcd", "abcd"],
            2,
        ),
        (r#"'(8 (:eval "x"))"#, ["x       ", "x       "], 0),
        (
            r#"'(#1=(:eval (number-to-string (setq qa-title-count (1+ qa-title-count)))) #1#)"#,
            ["12", "34"],
            4,
        ),
        (
            r#"'(:propertize (:eval "%b") face bold)"#,
            ["eval-primary", "eval-other"],
            0,
        ),
    ] {
        eval(
            &mut i,
            &format!("(setq qa-title-count 0 qa-title-on nil frame-title-format {format})"),
        );
        check(&mut i, &a, &b, expected);
        assert_eq!(eval(&mut i, "qa-title-count").int(), Some(calls));
    }
    eval(
        &mut i,
        r#"(progn
      (setq qa-title-label "before")
      (setq frame-title-format '("" qa-title-label (:eval (progn (setq qa-title-label "after") "|")) qa-title-label)))"#,
    );
    check(&mut i, &a, &b, ["before|after", "after|after"]);
    eval(
        &mut i,
        r#"(progn
      (setq global-mode-string '(:eval "%b") frame-title-format "global:%M")
      (setq qa-title-form '(:eval "trusted"))
      (put 'qa-title-form 'risky-local-variable nil))"#,
    );
    check(&mut i, &a, &b, ["global:eval-primary", "global:eval-other"]);
    eval(&mut i, r#"(setq frame-title-format '("" qa-title-form))"#);
    check(&mut i, &a, &b, ["", ""]);
    eval(&mut i, "(put 'qa-title-form 'risky-local-variable t)");
    check(&mut i, &a, &b, ["trusted", "trusted"]);
    eval(
        &mut i,
        r#"(progn
      (setq debug-on-error t debug-on-signal t qa-title-debug-count 0)
      (setq debugger (lambda (&rest _) (setq qa-title-debug-count (1+ qa-title-debug-count))))
      (setq frame-title-format '(:eval (error "title failure"))))"#,
    );
    check(&mut i, &a, &b, ["", ""]);
    assert_eq!(eval(&mut i, "qa-title-debug-count").int(), Some(0));
    eval(
        &mut i,
        r#"(setq frame-title-format '(:eval (if (and inhibit-redisplay inhibit-quit debug-on-error) "guarded" "bad")))"#,
    );
    check(&mut i, &a, &b, ["guarded", "guarded"]);
    eval(&mut i, r#"(setq inhibit-eval-during-redisplay t)"#);
    check(&mut i, &a, &b, ["", ""]);
    eval(
        &mut i,
        r#"(progn (setq inhibit-eval-during-redisplay nil debug-on-error nil debug-on-signal nil)
      (setq frame-title-format '(:eval (progn (goto-char (point-max)) (string-match "z" "z") "safe"))))"#,
    );
    check(&mut i, &a, &b, ["safe", "safe"]);
    // A point excursion in an evaluator must not scroll the actual viewport
    // while expanding a position construct returned by that evaluator.
    eval(
        &mut i,
        r#"(progn
      (with-current-buffer "eval-primary"
        (erase-buffer) (dotimes (_ 100) (insert "aaaa\n"))
        (goto-char 211) (set-window-start nil 201))
      (setq frame-title-format '(:eval (progn (goto-char (point-max)) "%p"))))"#,
    );
    check(&mut i, &a, &b, ["40%", "All"]);
    assert_eq!(a.borrow().selected.borrow().start, 200);
    let point = eval(&mut i, r#"(with-current-buffer "eval-primary" (point))"#);
    assert_eq!(point.int(), Some(211));
    eval(
        &mut i,
        r#"(progn
      (setq frame-title-format "visible" icon-title-format '(:eval (concat "icon:" (buffer-name))))
      (iconify-frame))"#,
    );
    check(&mut i, &a, &b, ["icon:eval-primary", "visible"]);
    eval(
        &mut i,
        r#"(progn
      (make-frame-visible)
      (setq frame-title-format '(:eval (throw 'qa-title-escape "escaped"))))"#,
    );
    let caller = i.current_buffer;
    check(&mut i, &a, &b, ["", ""]);
    let tag = Value::Sym(i.intern("qa-title-escape"));
    i.catch_tags.push(tag);
    let thrown = redraw(&mut i);
    i.catch_tags.pop();
    let Err(Flow::Throw(_, value)) = thrown else {
        panic!("expected outer throw: {thrown:?}")
    };
    assert_eq!(i.prin1_to_string(&value).to_unicode_lossy(), "\"escaped\"");
    assert_eq!(i.current_buffer, caller);
    assert!(std::rc::Rc::ptr_eq(i.selected_frame.as_ref().unwrap(), &a));
    let flags = eval(&mut i, "(list inhibit-redisplay inhibit-quit)");
    assert_eq!(i.prin1_to_string(&flags).to_unicode_lossy(), "(nil nil)");
    eval(
        &mut i,
        r#"(setq frame-title-format '(:eval
      (if (eq (selected-frame) qa-title-other)
          (progn (delete-frame qa-title-other t) "deleted") "live")))"#,
    );
    let deleted = redraw(&mut i);
    assert!(matches!(deleted, Err(Flow::Signal(_))), "{deleted:?}");
    assert!(b.borrow().dead);
    assert_eq!(i.current_buffer, caller);
    assert!(std::rc::Rc::ptr_eq(i.selected_frame.as_ref().unwrap(), &a));
}
