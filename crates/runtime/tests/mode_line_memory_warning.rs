use remacs_runtime::{frontend::render_grid, lisp::Interp};

#[test]
fn memory_warning_tracks_the_lisp_warning_state_in_all_bars() {
    let mut interp = Interp::new();
    interp
        .eval_str(
            r#"(progn
              (set-frame-size nil 48 12)
              (setq-local mode-line-format '("M:Memory:%e|"))
              (setq-local header-line-format '("H:Memory:%e|"))
              (setq-local tab-line-format '("T:Memory:%e|")))"#,
        )
        .unwrap();
    // Simulate the existing warning state without exhausting real memory.
    for (state, warning) in [
        ("nil", ""),
        ("t", "!MEM FULL! "),
        ("0", "!MEM FULL! "),
        ("nil", ""),
    ] {
        interp
            .eval_str(&format!(
                "(progn (setq memory-full {state}) (force-mode-line-update))"
            ))
            .unwrap();
        let grid = render_grid(&interp, 48, 12);
        for prefix in ["M:", "H:", "T:"] {
            let line = grid
                .rows
                .iter()
                .find(|row| row.starts_with(prefix))
                .unwrap_or_else(|| panic!("missing {prefix}: {:?}", grid.rows));
            assert_eq!(
                line.trim_end(),
                format!("{prefix}Memory:{warning}|"),
                "memory-full={state}"
            );
        }
    }
}
