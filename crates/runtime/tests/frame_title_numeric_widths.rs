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
        (40, 12)
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
    let current = i.current_buffer;
    let selected = i.selected_frame.as_ref().unwrap().clone();
    assert_eq!(
        render_frame_grid_with_input_cached(i, frame, 40, 12, None).title,
        expected
    );
    assert_eq!(i.current_buffer, current);
    assert!(std::rc::Rc::ptr_eq(
        i.selected_frame.as_ref().unwrap(),
        &selected
    ));
}

#[test]
fn frame_title_decimal_modifiers_match_gnu_padding_and_nested_field_rules() {
    let mut i = Interp::new();
    eval(
        &mut i,
        r#"(progn
        (set-frame-parameter nil 'window-system 'ns)
        (set-frame-parameter nil 'menu-bar-lines 0)
        (set-frame-parameter nil 'tool-bar-lines 0)
        (set-frame-parameter nil 'tab-bar-lines 0)
        (set-frame-size nil 40 12)
        (setq-default mode-line-format nil header-line-format nil tab-line-format nil truncate-lines t)
        (switch-to-buffer (get-buffer-create "abc"))
        (insert (make-string 1000 ?x))
        (setq buffer-read-only t global-mode-string "xy"))"#,
    );
    let primary = i.selected_frame.as_ref().unwrap().clone();
    let Value::Frame(other) = eval(
        &mut i,
        r#"(setq qa-width-other
        (make-frame '((window-system . ns) (width . 40) (height . 12)
                      (menu-bar-lines . 0) (tool-bar-lines . 0) (tab-bar-lines . 0))))"#,
    ) else {
        panic!("other frame")
    };
    eval(
        &mut i,
        r#"(progn
        (set-window-buffer (frame-selected-window qa-width-other) (get-buffer-create "long-buffer"))
        (set-buffer (get-buffer-create "width-unrelated")))"#,
    );
    // GNU 31.1 MODE_LINE_NOPROP shares the title's column-based sink.
    for (form, expected) in [
        (r#""[%10b]""#, "[abc       ]"),
        (r#""%2b""#, "abc"),
        (r#""%0b""#, "abc"),
        (r#""%0006b""#, "abc   "),
        (r#""%6f""#, "      "),
        (r#""%6i""#, "  1000"),
        (r#""%6I""#, "   1.0k"),
        (r#""%5*""#, "%    "),
        (r#""%5+""#, "*    "),
        (r#""%5&""#, "*    "),
        (r#""%5%""#, "%    "),
        (r#""%5e""#, "     "),
        (r#""%6M""#, "xy    "),
        (r#""%6P""#, "All   "),
        (r#""%6o""#, "All   "),
        (r#""%8q""#, "All     "),
        (r#""%10""#, ""),
        (r#""end%""#, "end"),
        (r#""%5漢""#, "     "),
        (r#""%4294967302b""#, "abc   "),
        (r#""%2147483648b""#, "abc"),
        (r#""%%10b""#, "%10b"),
        (r#"(-3 "%10b")"#, "abc       "),
        (r#"(-3 "%10i")"#, "          "),
        (r#"(-3 "%10I")"#, "          "),
        (r#"(-3 "%10M")"#, "xy        "),
        (r#"(5 "%10b")"#, "abc  "),
        (r#"(5 "xx%10b")"#, "xxabc"),
        (r#"(2 "%10b")"#, "abc"),
        (r#"(5 "%10i")"#, " 1000"),
        (r#"(5 "%10I")"#, "  1.0k"),
        (r#"(5 "%10M")"#, "xy   "),
        (r#"(-3 "%2b")"#, "abc"),
        (r#"("" "%6b" "%6i")"#, "abc     1000"),
    ] {
        eval(&mut i, &format!("(setq frame-title-format '{form})"));
        check(&i, &primary, expected);
    }
    eval(&mut i, r#"(setq frame-title-format "[%10b]")"#);
    check(&i, &other, "[long-buffer]");
    eval(&mut i, r#"(setq frame-title-format "%400i")"#);
    check(
        &i,
        &primary,
        &format!("{}1000{}", " ".repeat(156), " ".repeat(240)),
    );
    eval(
        &mut i,
        r#"(progn
        (with-current-buffer "abc" (setq buffer-read-only nil buffer-file-name "/tmp/example"))
        (setq frame-title-format "[%16f]"))"#,
    );
    check(&i, &primary, "[/tmp/example    ]");
    eval(
        &mut i,
        r#"(with-current-buffer "abc" (rename-buffer "漢a"))"#,
    );
    for (form, expected) in [
        (r#""[%6b]""#, "[漢a   ]"),
        (r#"(-1 "%6b")"#, "      "),
        (r#"(-3 "%6b")"#, "漢a   "),
        (r#"(3 "%6b")"#, "漢a"),
    ] {
        eval(&mut i, &format!("(setq frame-title-format '{form})"));
        check(&i, &primary, expected);
    }
    eval(
        &mut i,
        r#"(progn
        (with-current-buffer "漢a"
          (setq-local char-width-table (copy-sequence char-width-table))
          (set-char-table-range char-width-table ?漢 1))
        (setq frame-title-format "[%6b]"))"#,
    );
    check(&i, &primary, "[漢a    ]");
    // Decimal modifiers must stay visible to the existing dependency probe.
    eval(
        &mut i,
        r#"(progn
        (setq pre-redisplay-function nil frame-title-format "%4@")
        (with-current-buffer "漢a" (setq default-directory "/claimed/path/"))
        (setq qa-width-calls 0)
        (defun qa-width-handler (operation _name &rest _)
          (when (eq operation 'file-remote-p)
            (setq qa-width-calls (1+ qa-width-calls)) "/mock:host:"))
        (setq file-name-handler-alist '(("\\`/claimed/" . qa-width-handler))))"#,
    );
    let windows = primary.borrow().windows.clone();
    Frontend
        .run_pre_redisplay_for_windows(&mut i, windows)
        .unwrap();
    check(&i, &primary, "@   ");
    assert_eq!(eval(&mut i, "qa-width-calls").int(), Some(1));
    eval(&mut i, r#"(setq frame-title-format '(-4 "%10b%@"))"#);
    let windows = primary.borrow().windows.clone();
    Frontend
        .run_pre_redisplay_for_windows(&mut i, windows)
        .unwrap();
    check(&i, &primary, "漢a        ");
    assert_eq!(eval(&mut i, "qa-width-calls").int(), Some(1));
}
