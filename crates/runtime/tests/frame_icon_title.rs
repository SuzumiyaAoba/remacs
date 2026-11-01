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
        (60, 12)
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
fn iconified_titles_use_icon_format_in_the_frame_buffer_and_restore_normal_titles() {
    let mut i = Interp::new();
    eval(
        &mut i,
        r#"(progn
      (set-frame-parameter nil 'window-system 'x)
      (set-frame-parameter nil 'menu-bar-lines 0)
      (set-frame-parameter nil 'tool-bar-lines 0)
      (set-frame-size nil 60 12)
      (setq-default mode-line-format nil header-line-format nil tab-line-format nil)
      (switch-to-buffer (get-buffer-create "icon-primary"))
      (fundamental-mode)
      (setq frame-title-format "normal:%b"))"#,
    );
    let primary = i.selected_frame.as_ref().unwrap().clone();
    check(&i, &primary, "normal:icon-primary");
    eval(&mut i, "(iconify-frame)");
    check(&i, &primary, "icon-primary");
    eval(
        &mut i,
        r#"(progn (make-frame-visible) (setq icon-title-format "icon:%b"))"#,
    );
    let Value::Frame(other) = eval(
        &mut i,
        r#"(setq qa-icon-other
      (make-frame '((window-system . x) (width . 60) (height . 12)
                    (menu-bar-lines . 0) (tool-bar-lines . 0))))"#,
    ) else {
        panic!("other frame")
    };
    eval(
        &mut i,
        r#"(progn
      (with-current-buffer (get-buffer-create "icon-other") (insert "other"))
      (set-window-buffer (frame-selected-window qa-icon-other) "icon-other")
      (set-buffer (get-buffer-create "icon-caller")) (insert "caller") (goto-char 3))"#,
    );
    check(&i, &primary, "normal:icon-primary");
    check(&i, &other, "normal:icon-other");
    eval(&mut i, "(iconify-frame)");
    check(&i, &primary, "icon:icon-primary");
    check(&i, &other, "normal:icon-other");
    for (format, expected) in [
        (r#""静止:%b""#, "静止:icon-primary"),
        ("t", "normal:icon-primary"),
        ("nil", ""),
        (r#"'(8 "%m")"#, "Fundamental"),
        (
            r#"'(multiple-frames "many:%b" "one:%b")"#,
            "many:icon-primary",
        ),
    ] {
        eval(&mut i, &format!("(setq icon-title-format {format})"));
        check(&i, &primary, expected);
        check(&i, &other, "normal:icon-other");
    }
    eval(
        &mut i,
        r#"(with-current-buffer "icon-primary" (setq-local icon-title-format "local:%b"))"#,
    );
    check(&i, &primary, "local:icon-primary");
    eval(&mut i, "(make-frame-visible)");
    check(&i, &primary, "normal:icon-primary");
    eval(&mut i, "(iconify-frame)");
    check(&i, &primary, "local:icon-primary");
    eval(&mut i, r#"(set-frame-parameter nil 'title "Pinned")"#);
    check(&i, &primary, "Pinned");
    eval(
        &mut i,
        r#"(progn
      (set-frame-parameter nil 'title nil)
      (with-current-buffer "icon-primary"
        (setq icon-title-format "%@:%b" default-directory "/claimed/path/"))
      (setq pre-redisplay-function nil qa-icon-remote-calls 0)
      (defun qa-icon-handler (operation _name &rest _)
        (when (eq operation 'file-remote-p)
          (setq qa-icon-remote-calls (1+ qa-icon-remote-calls)) "/mock:host:"))
      (setq file-name-handler-alist '(("\\`/claimed/" . qa-icon-handler))))"#,
    );
    let windows = primary.borrow().windows.clone();
    Frontend
        .run_pre_redisplay_for_windows(&mut i, windows)
        .unwrap();
    check(&i, &primary, "@:icon-primary");
    assert_eq!(eval(&mut i, "qa-icon-remote-calls").int(), Some(1));
}
