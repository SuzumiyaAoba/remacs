use remacs_runtime::{frontend::render_grid, lisp::Interp};

fn check(interp: &mut Interp, construct: &str, expected: &str) {
    interp
        .eval_str(&format!(
            r#"(progn
              (setq-local mode-line-format '("M:" {construct}))
              (setq-local header-line-format '("H:" {construct}))
              (setq-local tab-line-format '("T:" {construct})))"#,
        ))
        .unwrap();
    let grid = render_grid(interp, 48, 12);
    for prefix in ["M:", "H:", "T:"] {
        let line = grid
            .rows
            .iter()
            .find(|row| row.starts_with(prefix))
            .unwrap_or_else(|| panic!("missing {prefix} line: {:?}", grid.rows));
        assert_eq!(
            line.trim_end(),
            format!("{prefix}{expected}"),
            "{construct}"
        );
    }
}

#[test]
fn conditional_lines_render_only_the_selected_branch() {
    let mut interp = Interp::new();
    interp
        .eval_str("(progn (set-frame-size nil 48 12) (setq-default qa3305-condition t))")
        .unwrap();
    check(&mut interp, "(qa3305-condition \"yes\" \"no\")", "yes");
    check(&mut interp, "(qa3305-condition \"yes\")", "yes");
    check(&mut interp, "(qa3305-condition nil \"no\")", "");
    check(&mut interp, "(qa3305-condition (\"a\" \"b\") \"no\")", "ab");
    check(
        &mut interp,
        "(\"before \" (qa3305-condition \"yes\" \"%-\") \" after\")",
        "before yes after",
    );

    interp
        .eval_str("(setq-local qa3305-condition nil)")
        .unwrap();
    check(&mut interp, "(qa3305-condition \"yes\" \"no\")", "no");
    check(&mut interp, "(qa3305-condition \"yes\")", "");
    check(
        &mut interp,
        "(qa3305-condition \"yes\" (\"n\" \"o\"))",
        "no",
    );
    check(&mut interp, "(qa3305-unbound \"yes\" \"no\")", "no");
    check(
        &mut interp,
        "(\"before \" (qa3305-condition \"%-\" \"no\") \" after\")",
        "before no after",
    );

    // Every non-nil Lisp value, including zero, selects THEN.
    interp.eval_str("(setq-local qa3305-condition 0)").unwrap();
    check(&mut interp, "(qa3305-condition \"yes\" \"no\")", "yes");
    check(
        &mut interp,
        "(qa3305-condition (:propertize \"yes\" face bold) \"no\")",
        "yes",
    );
}
