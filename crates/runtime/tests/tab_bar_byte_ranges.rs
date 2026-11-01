use remacs_runtime::{frontend::render_grid, lisp::Interp};

#[test]
fn tab_select_close_and_new_buttons_keep_their_composed_text_ranges() {
    let mut i = Interp::new();
    i.eval_str(r#"(progn
      (set-frame-parameter nil 'window-system 'ns)
      (set-frame-parameter nil 'menu-bar-lines 0)
      (set-frame-parameter nil 'tool-bar-lines 0)
      (set-frame-parameter nil 'tab-bar-lines 1)
      (set-frame-parameter nil 'tabs '((current-tab (name . "é漢")) (tab (name . "Wide"))))
      (setq tab-bar-close-button-show t tab-bar-close-button " ×" tab-bar-new-button-show t tab-bar-new-button " + ")
      (set-face-attribute 'tab-bar-tab nil :width 'expanded)
      (set-face-attribute 'tab-bar-tab-inactive nil :width 'condensed))"#).unwrap();
    let grid = render_grid(&i, 60, 12);
    let actual: Vec<_> = grid
        .tab_bar_items
        .iter()
        .map(|item| {
            let (start, end) = item.byte_range.expect("composed tab item range");
            (
                item.command.as_str(),
                item.argument,
                grid.rows[item.row][start..end].trim(),
            )
        })
        .collect();
    assert_eq!(
        actual,
        vec![
            ("tab-bar-select-tab", Some(1), "é漢"),
            ("tab-bar-close-tab", Some(1), "×"),
            ("tab-bar-select-tab", Some(2), "Wide"),
            ("tab-bar-close-tab", Some(2), "×"),
            ("tab-bar-new-tab", None, "+"),
        ]
    );
}
