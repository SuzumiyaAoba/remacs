use remacs_runtime::{
    frontend::render_frame_grid_with_input_cached,
    lisp::{Interp, Value},
};

#[test]
fn frame_title_integer_fields_match_gnu_column_padding_and_precision() {
    let mut i = Interp::new();
    i.eval_str(
        r#"(progn
        (set-frame-parameter nil 'window-system 'ns)
        (set-frame-parameter nil 'menu-bar-lines 0)
        (set-frame-parameter nil 'tool-bar-lines 0)
        (set-frame-parameter nil 'tab-bar-lines 0)
        (set-frame-size nil 48 12)
        (setq-default mode-line-format nil header-line-format nil tab-line-format nil)
        (switch-to-buffer (get-buffer-create "abcdef")))"#,
    )
    .unwrap();
    let frame = i.selected_frame.as_ref().unwrap().clone();
    // GNU 31.1 format-mode-line with integer FACE uses MODE_LINE_NOPROP,
    // whose column-based string sink is shared with MODE_LINE_TITLE.
    for (form, expected) in [
        (r#"(3 "a")"#, "a  "),
        (r#"(3 "abcdef")"#, "abcdef"),
        (r#"(-3 "abcdef")"#, "abc"),
        (r#"(-3 "a")"#, "a"),
        (r#"(0 "ab" "cd")"#, "abcd"),
        ("(5)", "     "),
        ("(-3)", ""),
        ("(0)", ""),
        (r#"(6 (-3 "abcdef"))"#, "abc   "),
        (r#"(-3 (6 "a"))"#, "a  "),
        // Preserve GNU's literal-span zero-precision behavior when no
        // character fits, as well as wide and combining character widths.
        (r#"(-1 "漢a")"#, "漢a"),
        (r#"(-2 "漢a")"#, "漢"),
        (r#"(-3 "漢a")"#, "漢a"),
        (r#"(5 "漢a")"#, "漢a  "),
        ("(-1 \"e\u{301}x\")", "e\u{301}"),
        ("(-1 \"e\" \"\u{301}x\")", "e"),
        ("(-2 \"a\u{301}b\")", "a\u{301}b"),
        ("(-2 \"a\" \"\u{301}b\")", "a\u{301}b"),
        (r#"(-4 "a\tb")"#, "a"),
        (r#"(10 "a\tb")"#, "a\tb"),
        (r#"(-3 "a\nb")"#, "a\nb"),
        (r#"(4294967299 "a")"#, "a  "),
        (r#"(-4294967299 "abcdef")"#, "abc"),
        (r#"(-3 "%b")"#, "abc"),
        (r#"(9 "%b")"#, "abcdef   "),
        (r#"(5 "%b")"#, "abcdef"),
        (r#"(0 "%b")"#, "abcdef"),
    ] {
        i.eval_str(&format!("(setq frame-title-format '{form})"))
            .unwrap();
        let grid = render_frame_grid_with_input_cached(&i, &frame, 48, 12, None);
        assert_eq!(grid.title, expected, "{form}");
    }
    // Width evaluation uses the rendered frame buffer's active settings.
    let Value::Frame(other) = i
        .eval_str(r#"(make-frame '((window-system . ns) (width . 48) (height . 12)))"#)
        .unwrap()
    else {
        panic!("second frame")
    };
    i.eval_str(r#"(progn
        (with-current-buffer (get-buffer-create "*width-other*")
          (setq-local tab-width 4 char-width-table (copy-sequence char-width-table))
          (set-char-table-range char-width-table ?漢 1))
        (set-window-buffer (frame-selected-window (car (delq (selected-frame) (frame-list)))) "*width-other*"))"#).unwrap();
    for (form, first, second) in [
        (r#"(10 "a\tb")"#, "a\tb", "a\tb    "),
        (r#"(4 "漢a")"#, "漢a ", "漢a  "),
    ] {
        i.eval_str(&format!(
            "(setq-default frame-title-format '{form}) (kill-local-variable 'frame-title-format)"
        ))
        .unwrap();
        assert_eq!(
            render_frame_grid_with_input_cached(&i, &frame, 48, 12, None).title,
            first
        );
        assert_eq!(
            render_frame_grid_with_input_cached(&i, &other, 48, 12, None).title,
            second
        );
    }
    let ordinary = i.eval_str("(string-width \"a\\tb\")").unwrap();
    assert_eq!(ordinary.int(), Some(10));
}
