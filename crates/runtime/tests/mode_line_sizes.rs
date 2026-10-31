use remacs_runtime::{frontend::render_grid, lisp::Interp};

fn check(interp: &mut Interp, format: &str, expected: &str) {
    interp
        .eval_str(&format!(
            r#"(progn
              (setq-local mode-line-format '("M:" "{format}" "|"))
              (setq-local header-line-format '("H:" "{format}" "|"))
              (setq-local tab-line-format '("T:" "{format}" "|")))"#,
        ))
        .unwrap();
    let grid = render_grid(interp, 48, 12);
    for prefix in ["M:", "H:", "T:"] {
        let line = grid
            .rows
            .iter()
            .find(|row| row.starts_with(prefix))
            .unwrap_or_else(|| panic!("missing {prefix}: {:?}", grid.rows));
        assert_eq!(line.trim_end(), format!("{prefix}{expected}|"));
    }
}

fn replace(interp: &mut Interp, size: usize, character: char) {
    interp
        .eval_str(&format!(
            "(progn (widen) (erase-buffer) (insert (make-string {size} ?{character})) (goto-char 1))"
        ))
        .unwrap();
}

#[test]
fn readable_sizes_use_decimal_units_and_accessible_character_counts() {
    let mut interp = Interp::new();
    interp
        .eval_str("(progn (set-frame-size nil 48 12) (setq buffer-undo-list t) (setq-local truncate-lines t))")
        .unwrap();
    for (size, expected) in [
        (0, "0"),
        (999, "999"),
        (1000, "1.0k"),
        (1049, "1.0k"),
        (1050, "1.1k"),
        (9949, "9.9k"),
        (9950, "10k"),
        (10500, "11k"),
        (999499, "999k"),
        (999500, "1.0M"),
        (1000000, "1.0M"),
    ] {
        replace(&mut interp, size, 'a');
        check(&mut interp, "%I", expected);
    }
    replace(&mut interp, 1000, '日');
    check(&mut interp, "%I", "1.0k");
    check(&mut interp, "%6I", "   1.0k");
    replace(&mut interp, 10000, 'a');
    interp
        .eval_str("(progn (narrow-to-region 101 1101) (goto-char (point-min)) (set-window-start nil (point-min)))")
        .unwrap();
    check(&mut interp, "%I", "1.0k");
}
