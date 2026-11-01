use remacs_runtime::{frontend::render_grid, lisp::Interp};

#[test]
fn tool_buttons_keep_their_unicode_composed_ranges_and_commands() {
    let mut i = Interp::new();
    i.eval_str(
        r#"(progn
      (set-frame-parameter nil 'window-system 'ns)
      (set-frame-parameter nil 'menu-bar-lines 0)
      (set-frame-parameter nil 'tool-bar-lines 1)
      (set-frame-parameter nil 'tab-bar-lines 0)
      (defun remacs-tool-first () (interactive))
      (defun remacs-tool-second () (interactive))
      (setq tool-bar-map (make-sparse-keymap))
      (define-key tool-bar-map [first] '(menu-item "é漢" remacs-tool-first))
      (define-key tool-bar-map [second] '(menu-item "Wide" remacs-tool-second))
      (set-face-attribute 'tool-bar nil :width 'expanded))"#,
    )
    .unwrap();
    let grid = render_grid(&i, 60, 12);
    assert_eq!(grid.tool_bar_items.len(), 2);
    for (command, expected) in [("remacs-tool-first", "é漢"), ("remacs-tool-second", "Wide")] {
        let item = grid
            .tool_bar_items
            .iter()
            .find(|item| item.command == command)
            .unwrap();
        let (start, end) = item.byte_range.expect("composed tool button range");
        assert_eq!(grid.rows[item.row][start..end].trim(), expected);
    }
}
