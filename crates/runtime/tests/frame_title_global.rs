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
fn check(i: &Interp, frame: &FrameRef, expected: &str) {
    let selected = i.selected_frame.as_ref().unwrap().clone();
    let current = i.current_buffer;
    assert_eq!(
        render_frame_grid_with_input_cached(i, frame, 48, 12, None).title,
        expected
    );
    assert_eq!(i.current_buffer, current);
    assert!(std::rc::Rc::ptr_eq(
        i.selected_frame.as_ref().unwrap(),
        &selected
    ));
}

#[test]
fn frame_title_global_mode_string_formats_values_live_with_gnu_limits() {
    let mut i = Interp::new();
    eval(
        &mut i,
        r#"(progn
        (set-frame-parameter nil 'window-system 'ns)
        (set-frame-parameter nil 'menu-bar-lines 0)
        (set-frame-parameter nil 'tool-bar-lines 0)
        (set-frame-parameter nil 'tab-bar-lines 0)
        (set-frame-size nil 48 12)
        (setq-default mode-line-format nil header-line-format nil tab-line-format nil)
        (switch-to-buffer (get-buffer-create "global-primary"))
        (defvar qa-title-piece "clock %b")
        (defvar qa-title-wide "漢a")
        (defvar qa-title-flag t))"#,
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
        (set-window-buffer (frame-selected-window qa-title-other)
                           (get-buffer-create "global-other"))
        (set-buffer (get-buffer-create "global-unrelated")))"#,
    );
    // GNU 31.1 interactive integer-FACE reference: %M formats another element,
    // so nested variables, fields and recursion retain their own semantics.
    for (value, expected, clipped, padded) in [
        ("nil", "", "", "            "),
        (
            r#""Global status""#,
            "Global status",
            "Glo",
            "Global status",
        ),
        (r#"("" "A" "B")"#, "AB", "AB", "AB          "),
        (r#"("" qa-title-piece)"#, "clock %b", "clo", "clock %b    "),
        (
            r#"(qa-title-flag "yes" "no")"#,
            "yes",
            "yes",
            "yes         ",
        ),
        (r#"(6 "ok")"#, "ok    ", "ok ", "ok          "),
        (r#""漢a""#, "漢a", "漢a", "漢a         "),
        ("t", "", "", "            "),
        ("7", "*invalid*", "*in", "*invalid*   "),
        ("qa-title-unbound", "", "", "            "),
        (r#""%M""#, "*too-deep*", "*to", "*too-deep*  "),
        (r#"("" "%M")"#, "*too-deep*", "*to", "*too-deep*  "),
    ] {
        eval(
            &mut i,
            &format!("(setq global-mode-string '{value} frame-title-format \"%M\")"),
        );
        check(&i, &primary, expected);
        check(&i, &other, expected);
        eval(&mut i, r#"(setq frame-title-format "<%M>|%%M")"#);
        check(&i, &primary, &format!("<{expected}>|%M"));
        eval(&mut i, r#"(setq frame-title-format '(-3 "%M"))"#);
        check(&i, &primary, clipped);
        eval(&mut i, r#"(setq frame-title-format '(12 "%M"))"#);
        check(&i, &primary, padded);
    }
    for (value, expected) in [(r#""漢a""#, "漢a"), (r#"("" qa-title-wide)"#, "")] {
        eval(
            &mut i,
            &format!("(setq global-mode-string '{value} frame-title-format '(-1 \"%M\"))"),
        );
        check(&i, &primary, expected);
    }
    eval(
        &mut i,
        r#"(setq global-mode-string "%%:%b" frame-title-format "%M")"#,
    );
    check(&i, &primary, "%:global-primary");
    check(&i, &other, "%:global-other");
    eval(
        &mut i,
        r#"(progn
        (setq global-mode-string '("[" qa-title-piece "]"))
        (setq qa-title-piece "10:00"))"#,
    );
    check(&i, &primary, "[10:00]");
    eval(&mut i, r#"(setq qa-title-piece "10:01")"#);
    check(&i, &primary, "[10:01]");
    eval(&mut i, r#"(aset qa-title-piece 4 ?2)"#);
    check(&i, &other, "[10:02]");

    eval(
        &mut i,
        r#"(progn
        (with-current-buffer "global-primary" (setq-local global-mode-string "local A"))
        (with-current-buffer "global-other" (setq-local global-mode-string "local B")))"#,
    );
    check(&i, &primary, "local A");
    check(&i, &other, "local B");
    eval(&mut i, r#"(set-buffer "global-primary")"#);
    let mark = i.specbind_depth();
    let symbol = i.intern("global-mode-string");
    i.specbind(symbol, Value::string("bound A")).unwrap();
    check(&i, &primary, "bound A");
    check(&i, &other, "local B");
    i.unbind_to(mark).unwrap();
    check(&i, &primary, "local A");

    // The existing remote dependency probe must also see %@ inside %M.
    eval(
        &mut i,
        r#"(progn
        (setq pre-redisplay-function nil)
        (with-current-buffer "global-primary"
          (setq global-mode-string "%@" default-directory "/claimed/path/"))
        (with-current-buffer "global-other"
          (setq global-mode-string "%%@" default-directory "/claimed/other/"))
        (setq qa-title-remote-calls 0)
        (defun qa-title-remote-handler (operation _name &rest _)
          (when (eq operation 'file-remote-p)
            (setq qa-title-remote-calls (1+ qa-title-remote-calls))
            "/mock:host:"))
        (setq file-name-handler-alist '(("\\`/claimed/" . qa-title-remote-handler))))"#,
    );
    let windows = i
        .frames
        .iter()
        .flat_map(|frame| frame.borrow().windows.clone())
        .collect();
    Frontend
        .run_pre_redisplay_for_windows(&mut i, windows)
        .unwrap();
    check(&i, &primary, "@");
    check(&i, &other, "%@");
    assert_eq!(eval(&mut i, "qa-title-remote-calls").int(), Some(1));
}
