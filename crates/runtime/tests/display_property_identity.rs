use remacs_runtime::{frontend::render_grid, lisp::Interp};

fn check(interp: &mut Interp, source: &str, expected: &str) {
    interp.eval_str("(progn (widen) (erase-buffer))").unwrap();
    interp.eval_str(source).unwrap();
    interp.eval_str("(goto-char (point-min))").unwrap();
    let grid = render_grid(interp, 40, 10);
    assert_eq!(grid.rows[0].trim_end(), expected, "{source}");
}

#[test]
fn display_replacement_runs_follow_object_identity() {
    let mut interp = Interp::new();
    interp
        .eval_str(
            "(progn (set-frame-size nil 40 10)
              (setq-local mode-line-format nil)
              (setq-local header-line-format nil)
              (setq-local tab-line-format nil))",
        )
        .unwrap();
    check(
        &mut interp,
        "(let ((replacement (copy-sequence \"X\")))
           (insert \"ab\")
           (put-text-property 1 3 'display replacement)
           (put-text-property 2 3 'face 'bold))",
        "X",
    );
    check(
        &mut interp,
        "(let ((replacement (copy-sequence \"X\")))
           (insert \"abc\")
           (put-text-property 1 4 'display replacement)
           (put-text-property 2 3 'face 'bold)
           (put-text-property 3 4 'help-echo \"third\"))",
        "X",
    );
    // Equal strings in separate objects remain separate replacements.
    check(
        &mut interp,
        "(progn (insert \"ab\")
           (put-text-property 1 2 'display (copy-sequence \"X\"))
           (put-text-property 2 3 'display (copy-sequence \"X\")))",
        "XX",
    );
    check(
        &mut interp,
        "(let ((replacement (copy-sequence \"X\")))
           (insert \"abc\")
           (put-text-property 1 2 'display replacement)
           (put-text-property 3 4 'display replacement))",
        "XbX",
    );
    check(
        &mut interp,
        "(progn (insert \"ab\")
           (put-text-property 1 2 'display \"X\")
           (put-text-property 2 3 'display \"Y\"))",
        "XY",
    );
    check(
        &mut interp,
        "(let ((replacement (copy-sequence \"X\")))
           (insert \"ab\")
           (put-text-property 1 3 'display replacement)
           (put-text-property 2 3 'face 'bold)
           (narrow-to-region 2 3))",
        "X",
    );
}
