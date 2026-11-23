use remacs_runtime::{frontend::render_grid, lisp::Interp};

#[test]
fn popup_menu_renders_button_help_and_key_properties() {
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(|| {
            let mut interp = Interp::new();
            interp
                .eval_str(
                    r#"(progn
                      (setq hack-read-symbol-shorthands-function nil)
                      (set-frame-parameter nil 'window-system 'ns)
                      (set-frame-parameter nil 'menu-bar-lines 0)
                      (defun qa-popup-property-command () (interactive))
                      (let ((map (make-sparse-keymap)))
                        (define-key map [toggle]
                          '(menu-item "Toggle" qa-popup-property-command
                                      :button (:toggle . t)
                                      :help "Toggle help text"
                                      :keys "C-c z"))
                        (define-key map [radio]
                          '(menu-item "Radio" qa-popup-property-command
                                      :button (:radio . t)
                                      :help "Radio help text"
                                      :keys "C-c r"))
                        (x-popup-menu t map)))"#,
                )
                .unwrap();

            let grid = render_grid(&interp, 80, 24);
            assert_eq!(grid.popup_menu_items.len(), 2);
            for (label, help, keys) in [
                ("Toggle", "Toggle help text", "C-c z"),
                ("Radio", "Radio help text", "C-c r"),
            ] {
                let item = grid
                    .popup_menu_items
                    .iter()
                    .find(|item| grid.rows[item.row].contains(label))
                    .unwrap_or_else(|| panic!("missing popup row {label}: {:?}", grid.rows));
                let row = &grid.rows[item.row];
                assert!(row.contains("✓"), "selected state missing from {row:?}");
                assert!(row.contains(keys), "key hint missing from {row:?}");
                assert!(
                    grid.help_echoes
                        .iter()
                        .any(|echo| echo.row == item.row && echo.text == help)
                );
            }
        })
        .unwrap()
        .join()
        .unwrap();
}
