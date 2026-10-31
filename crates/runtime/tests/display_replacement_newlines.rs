use remacs_runtime::{
    frontend::{Grid, render_grid},
    lisp::{Interp, Value},
};

fn eval(interp: &mut Interp, source: &str) -> Value {
    interp
        .eval_str(source)
        .unwrap_or_else(|flow| panic!("{source}: {flow:?}"))
}

fn check(interp: &mut Interp, source: &str, rows: &[&str], cursor: Option<(usize, usize)>) -> Grid {
    eval(
        interp,
        "(progn (erase-buffer) (remove-overlays) (setq-local line-prefix nil display-line-numbers nil buffer-invisibility-spec t))",
    );
    eval(interp, source);
    let text = eval(interp, "(buffer-string)");
    let point = eval(interp, "(point)");
    let grid = render_grid(interp, 40, 12);
    for (index, expected) in rows.iter().enumerate() {
        assert_eq!(
            grid.rows[index].trim_end(),
            *expected,
            "row {index}: {source}"
        );
    }
    assert_eq!(grid.rows[rows.len()].trim_end(), "", "extra row: {source}");
    if let Some(cursor) = cursor {
        assert_eq!(grid.cursor, Some(cursor), "{source}");
    }
    assert!(
        grid.cursors.iter().filter(|cursor| cursor.selected).count() <= 1,
        "duplicate cursor: {source}"
    );
    let after_text = eval(interp, "(buffer-string)");
    let after_point = eval(interp, "(point)");
    assert_eq!(
        interp.prin1_to_string(&after_text),
        interp.prin1_to_string(&text)
    );
    assert_eq!(
        interp.prin1_to_string(&after_point),
        interp.prin1_to_string(&point)
    );
    grid
}

#[test]
fn replacement_and_overlay_newlines_create_rows_without_changing_source_positions() {
    let mut interp = Interp::new();
    eval(
        &mut interp,
        "(progn
      (set-frame-parameter nil 'window-system 'ns)
      (set-frame-parameter nil 'menu-bar-lines 0)
      (set-frame-parameter nil 'tool-bar-lines 0)
      (set-frame-parameter nil 'tab-bar-lines 0)
      (set-frame-parameter nil 'vertical-scroll-bars nil)
      (set-frame-parameter nil 'horizontal-scroll-bars nil)
      (set-frame-size nil 40 12) (set-window-fringes nil 0 0)
      (setq-local mode-line-format nil header-line-format nil tab-line-format nil
                  indicate-empty-lines nil indicate-buffer-boundaries nil truncate-lines nil))",
    );
    remacs_runtime::editor::reflow_frame_windows(interp.selected_frame.as_ref().unwrap(), 40, 12);
    check(
        &mut interp,
        r#"(progn (insert "original") (put-text-property 1 (point-max) 'display "line 1\nline 2") (goto-char 1))"#,
        &["line 1", "line 2"],
        Some((0, 0)),
    );
    check(
        &mut interp,
        r#"(progn (insert "original") (put-text-property 1 (point-max) 'display "line 1\nline 2"))"#,
        &["line 1", "line 2"],
        Some((6, 1)),
    );
    check(
        &mut interp,
        r#"(progn (insert "X") (put-text-property 1 2 'display "A\n"))"#,
        &["A", ""],
        Some((0, 1)),
    );
    check(
        &mut interp,
        r#"(progn (insert "abXYcd") (put-text-property 3 5 'display "up\ndown"))"#,
        &["abup", "downcd"],
        Some((6, 1)),
    );
    check(
        &mut interp,
        r#"(progn (insert "XZ") (put-text-property 1 2 'display "\nA\n\nB\n"))"#,
        &["", "A", "", "B", "Z"],
        Some((1, 4)),
    );
    check(
        &mut interp,
        r#"(progn (insert "X-Y") (put-text-property 1 2 'display "A\nB") (put-text-property 3 4 'display "C\nD"))"#,
        &["A", "B-C", "D"],
        Some((1, 2)),
    );
    check(
        &mut interp,
        r#"(progn (insert "X") (put-text-property 1 2 'display "日\n本語"))"#,
        &["日", "本語"],
        Some((4, 1)),
    );
    check(
        &mut interp,
        r#"(progn (insert "X\nnext") (put-text-property 1 2 'display "a\nb"))"#,
        &["a", "b", "next"],
        Some((4, 2)),
    );
    check(
        &mut interp,
        r#"(progn (insert "abQcd") (put-text-property 3 4 'display "ignored\ntext") (overlay-put (make-overlay 3 4) 'display "ONE\nTWO"))"#,
        &["abONE", "TWOcd"],
        Some((5, 1)),
    );
    check(
        &mut interp,
        r#"(progn (insert "body") (overlay-put (make-overlay 1 1) 'before-string "pre\n") (goto-char 1))"#,
        &["pre", "body"],
        Some((0, 1)),
    );
    check(
        &mut interp,
        r#"(progn (insert "body") (overlay-put (make-overlay 1 (point-max)) 'after-string "!\nend"))"#,
        &["body!", "end"],
        Some((3, 1)),
    );
    check(
        &mut interp,
        r#"(progn (insert "X") (put-text-property 1 2 'display "A\nB") (overlay-put (make-overlay 1 1) 'before-string "p\nq") (goto-char 1))"#,
        &["p", "qA", "B"],
        Some((1, 1)),
    );
    check(
        &mut interp,
        r#"(progn (insert "body") (let ((overlay (make-overlay 1 (point-max)))) (overlay-put overlay 'before-string "AA\nBB") (overlay-put overlay 'after-string "CC\nDD")))"#,
        &["AA", "BBbodyCC", "DD"],
        Some((2, 2)),
    );
    check(
        &mut interp,
        r#"(progn (insert "Xbody") (put-text-property 1 2 'display "hidden\ntext") (put-text-property 1 2 'invisible t))"#,
        &["body"],
        Some((4, 0)),
    );
    check(
        &mut interp,
        r#"(progn (insert "X") (put-text-property 1 2 'display "A\\nB"))"#,
        &["A\\nB"],
        None,
    );

    let grid = check(
        &mut interp,
        r##"(progn (insert "XX") (put-text-property 1 3 'face '(:foreground "#ff0000")) (put-text-property 1 3 'display "red\nred"))"##,
        &["red", "red"],
        Some((3, 1)),
    );
    for row in 0..2 {
        assert_eq!(
            grid.char_attrs[row][0].fg,
            Some(remacs_runtime::frontend::CellColor(255, 0, 0))
        );
    }

    eval(
        &mut interp,
        "(progn (erase-buffer) (remove-overlays) (insert \"X\") (put-text-property 1 2 'display (make-string 10000 10)) (goto-char 1))",
    );
    let grid = render_grid(&interp, 40, 12);
    assert_eq!(grid.rows.len(), 12);
    assert!(grid.rows.iter().all(|row| row.trim_end().is_empty()));
    assert_eq!(grid.cursor, Some((0, 0)));
    assert_eq!(
        grid.cursors.iter().filter(|cursor| cursor.selected).count(),
        1
    );

    eval(
        &mut interp,
        r#"(progn (erase-buffer) (remove-overlays) (insert "X") (put-text-property 1 2 'display "A\nB") (setq-local display-line-numbers t) (goto-char 1))"#,
    );
    let grid = render_grid(&interp, 40, 12);
    assert!(grid.rows[0].contains('1'));
    assert!(grid.rows[0].trim_end().ends_with('A'));
    assert_eq!(grid.rows[1].trim(), "B");
}
