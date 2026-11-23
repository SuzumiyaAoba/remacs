use remacs_runtime::{
    frontend::{Grid, prepare_native_menus, render_grid},
    lisp::Interp,
};

fn assert_wrapped_toolbar_rows(grid: &Grid) {
    let labels = grid
        .tool_bar_items
        .iter()
        .map(|item| {
            let (start, end) = item.byte_range.expect("toolbar button range");
            (grid.rows[item.row][start..end].trim().to_owned(), item.row)
        })
        .collect::<Vec<_>>();

    assert_eq!(labels.len(), 2);
    let first = labels
        .iter()
        .find(|(label, _)| label == "First")
        .expect("first toolbar button");
    let second = labels
        .iter()
        .find(|(label, _)| label == "Second")
        .expect("second toolbar button");
    assert_eq!(first.1.min(second.1), 0);
    assert_eq!(first.1.max(second.1), 1);
    assert!(grid.rows.iter().all(|row| !row.contains("Wrap marker")));
}

#[test]
fn toolbar_wrap_menu_items_start_a_new_row_without_a_button() {
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(|| {
            let mut interp = Interp::new();
            interp
                .eval_str(
                    r#"(progn
                      (set-frame-parameter nil 'window-system 'ns)
                      (set-frame-parameter nil 'menu-bar-lines 0)
                      (set-frame-parameter nil 'tab-bar-lines 0)
                      (set-frame-parameter nil 'tool-bar-lines 1)
                      (setq menu-bar-mode nil tool-bar-map (make-sparse-keymap))
                      (defun qa-tool-bar-wrap-first () (interactive))
                      (defun qa-tool-bar-wrap-second () (interactive))
                      (define-key tool-bar-map [first]
                        '(menu-item "First" qa-tool-bar-wrap-first))
                      (define-key tool-bar-map [wrap]
                        '(menu-item "Wrap marker" ignore :wrap t))
                      (define-key tool-bar-map [second]
                        '(menu-item "Second" qa-tool-bar-wrap-second))
                      (tool-bar-mode 1))"#,
                )
                .unwrap();

            assert_wrapped_toolbar_rows(&render_grid(&interp, 80, 24));
            prepare_native_menus(&mut interp).unwrap();
            assert_wrapped_toolbar_rows(&render_grid(&interp, 80, 24));
        })
        .unwrap()
        .join()
        .unwrap();
}
