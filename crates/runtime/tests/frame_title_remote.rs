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
        (48, 12)
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
fn check(i: &mut Interp, primary: &FrameRef, other: &FrameRef, first: &str, second: &str) {
    let current = i.current_buffer;
    let selected = i.selected_frame.as_ref().unwrap().clone();
    let windows = i
        .frames
        .iter()
        .flat_map(|frame| frame.borrow().windows.clone())
        .collect();
    Frontend.run_pre_redisplay_for_windows(i, windows).unwrap();
    assert_eq!(
        render_frame_grid_with_input_cached(i, primary, 48, 12, None).title,
        first
    );
    assert_eq!(
        render_frame_grid_with_input_cached(i, other, 48, 12, None).title,
        second
    );
    assert_eq!(i.current_buffer, current);
    assert!(std::rc::Rc::ptr_eq(
        i.selected_frame.as_ref().unwrap(),
        &selected
    ));
}

#[test]
fn frame_title_remote_marker_uses_handlers_and_only_live_title_dependencies() {
    let mut i = Interp::new();
    eval(
        &mut i,
        r#"(progn
        (set-frame-parameter nil 'window-system 'ns)
        (set-frame-parameter nil 'menu-bar-lines 0)
        (set-frame-parameter nil 'tool-bar-lines 0)
        (set-frame-parameter nil 'tab-bar-lines 0)
        (set-frame-size nil 48 12)
        (setq pre-redisplay-function nil)
        (setq-default mode-line-format nil header-line-format nil tab-line-format nil)
        (switch-to-buffer (get-buffer-create "remote-primary"))
        (setq default-directory "/tmp/")
        (setq frame-title-format "[%@] %b")
        (setq qa-title-original-handlers file-name-handler-alist)
        (setq qa-title-calls 0 qa-title-enabled t qa-title-error nil)
        (defun qa-title-handler (operation name &rest options)
          (when (eq operation 'file-remote-p)
            (setq qa-title-calls (1+ qa-title-calls))
            (setq qa-title-argument-ok
              (and (eq name default-directory)
                   (eq (get-text-property 0 'remote-test name) 'kept)
                   (equal options '(nil nil)) inhibit-redisplay))
            (string-match "x" "x")
            (when qa-title-error (error "remote callback error"))
            (when qa-title-enabled "/mock:host:")))
        (setq file-name-handler-alist
          '(("\\`/mock:" . qa-title-handler) ("\\`/claimed/" . qa-title-handler))))"#,
    );
    let primary = i.selected_frame.as_ref().unwrap().clone();
    let Value::Frame(other) = eval(
        &mut i,
        r#"(setq qa-title-other
        (make-frame '((window-system . ns) (width . 48) (height . 12))))"#,
    ) else {
        panic!("other frame")
    };
    eval(
        &mut i,
        r#"(progn
        (with-current-buffer (get-buffer-create "remote-other")
          (setq default-directory (propertize "/mock:host:/other/" 'remote-test 'kept)))
        (set-window-buffer (frame-selected-window qa-title-other) "remote-other")
        (set-buffer (get-buffer-create "remote-unrelated"))
        (setq default-directory "/tmp/")
        (string-match "aa" "baaz"))"#,
    );
    check(
        &mut i,
        &primary,
        &other,
        "[-] remote-primary",
        "[@] remote-other",
    );
    assert!(eval(&mut i, "qa-title-argument-ok").truthy());
    assert!(eval(&mut i, "(equal (match-data t) '(1 3))").truthy());
    eval(
        &mut i,
        r#"(with-current-buffer "remote-primary"
        (setq default-directory (propertize "/claimed/path/" 'remote-test 'kept)))"#,
    );
    check(
        &mut i,
        &primary,
        &other,
        "[@] remote-primary",
        "[@] remote-other",
    );
    eval(&mut i, "(setq qa-title-enabled nil)");
    check(
        &mut i,
        &primary,
        &other,
        "[-] remote-primary",
        "[-] remote-other",
    );
    eval(
        &mut i,
        "(setq qa-title-enabled t inhibit-file-name-handlers '(qa-title-handler) inhibit-file-name-operation 'file-remote-p)",
    );
    check(
        &mut i,
        &primary,
        &other,
        "[-] remote-primary",
        "[-] remote-other",
    );
    eval(
        &mut i,
        "(setq inhibit-file-name-handlers nil inhibit-file-name-operation nil)",
    );
    eval(
        &mut i,
        "(put 'qa-title-handler 'operations '(file-exists-p))",
    );
    check(
        &mut i,
        &primary,
        &other,
        "[-] remote-primary",
        "[-] remote-other",
    );
    eval(&mut i, "(put 'qa-title-handler 'operations nil)");
    eval(&mut i, "(setq qa-title-error t debug-on-error t)");
    check(
        &mut i,
        &primary,
        &other,
        "[-] remote-primary",
        "[-] remote-other",
    );
    eval(
        &mut i,
        "(setq qa-title-error nil debug-on-error nil inhibit-eval-during-redisplay t)",
    );
    let calls = eval(&mut i, "qa-title-calls").int();
    check(
        &mut i,
        &primary,
        &other,
        "[-] remote-primary",
        "[-] remote-other",
    );
    assert_eq!(eval(&mut i, "qa-title-calls").int(), calls);
    eval(&mut i, "(setq inhibit-eval-during-redisplay nil)");
    for (form, expected) in [
        (r#""literal %%@""#, "literal %@"),
        (r#"(nil "%@" "local")"#, "local"),
        (r#"(-1 "x" "%@")"#, "x"),
    ] {
        eval(&mut i, &format!("(setq frame-title-format '{form})"));
        let before = eval(&mut i, "qa-title-calls").int();
        check(&mut i, &primary, &other, expected, expected);
        assert_eq!(eval(&mut i, "qa-title-calls").int(), before);
    }
    eval(
        &mut i,
        r#"(progn
        (defvar qa-title-format '("" "%@"))
        (setq frame-title-format 'qa-title-format)
        (set-frame-parameter nil 'title "literal %@"))"#,
    );
    let before = eval(&mut i, "qa-title-calls").int().unwrap();
    check(&mut i, &primary, &other, "literal %@", "@");
    assert_eq!(eval(&mut i, "qa-title-calls").int(), Some(before + 1));
    eval(
        &mut i,
        r#"(progn (set-frame-parameter nil 'title nil)
        (setq frame-title-format '(4 "%@")))"#,
    );
    check(&mut i, &primary, &other, "@   ", "@   ");
    eval(&mut i, r#"(setq frame-title-format "[%@] %b")"#);
    eval(
        &mut i,
        r#"(with-current-buffer "remote-primary" (aset default-directory 1 ?x))"#,
    );
    assert_eq!(
        render_frame_grid_with_input_cached(&i, &primary, 48, 12, None).title,
        "[-] remote-primary"
    );
    check(
        &mut i,
        &primary,
        &other,
        "[-] remote-primary",
        "[@] remote-other",
    );

    eval(
        &mut i,
        r#"(progn
        (with-current-buffer "remote-primary" (setq default-directory "/tmp/"))
        (with-current-buffer "remote-other" (setq default-directory "/tmp/"))
        (setq file-name-handler-alist qa-title-original-handlers)
        (require 'tramp)
        (setq qa-title-connection-attempts 0)
        (dolist (function '(make-network-process open-network-stream start-process make-process call-process))
          (fset function (lambda (&rest _)
            (setq qa-title-connection-attempts (1+ qa-title-connection-attempts))
            (error "Unexpected remote connection")))))"#,
    );
    // GNU 31.1 interactive reference; these are name-only Tramp queries.
    for (directory, marker) in [
        ("/tmp/", '-'),
        ("/tmp/a:b/", '-'),
        ("/ssh:example.invalid:/tmp/", '@'),
        ("/sudo::/tmp/", '@'),
        ("/ssh:jump.invalid|sudo:root@example.invalid:/var/", '@'),
        ("/:/ssh:example.invalid:/tmp/", '-'),
    ] {
        eval(
            &mut i,
            &format!(
                "(with-current-buffer \"remote-primary\" (setq default-directory {directory:?}))"
            ),
        );
        check(
            &mut i,
            &primary,
            &other,
            &format!("[{marker}] remote-primary"),
            "[-] remote-other",
        );
    }
    assert_eq!(eval(&mut i, "qa-title-connection-attempts").int(), Some(0));
    eval(
        &mut i,
        r#"(progn (setq file-name-handler-alist nil)
        (with-current-buffer "remote-primary" (setq default-directory "/ssh:example.invalid:/tmp/")))"#,
    );
    check(
        &mut i,
        &primary,
        &other,
        "[-] remote-primary",
        "[-] remote-other",
    );
}
