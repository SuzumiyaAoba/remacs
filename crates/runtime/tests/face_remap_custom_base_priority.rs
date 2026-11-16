use remacs_runtime::{frontend::render_grid, lisp::Interp};

#[test]
fn relative_face_remap_overrides_a_custom_base() {
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(|| {
            let mut interp = Interp::new();
            interp
                .eval_str(
                    r#"(progn
                      (set-frame-parameter nil 'window-system 'gpui)
                      (set-frame-parameter nil 'menu-bar-lines 0)
                      (set-frame-parameter nil 'tool-bar-lines 0)
                      (set-frame-parameter nil 'tab-bar-lines 0)
                      (set-frame-parameter nil 'vertical-scroll-bars nil)
                      (setq-default mode-line-format nil header-line-format nil tab-line-format nil)
                      (setq-local face-remapping-alist nil)
                      (face-remap-set-base 'default '(:foreground "blue"))
                      (face-remap-add-relative 'default '(:foreground "red"))
                      (erase-buffer)
                      (insert "This should be red"))"#,
                )
                .unwrap();

            let grid = render_grid(&interp, 80, 12);
            let text_row = grid
                .rows
                .iter()
                .position(|row| row.contains("This"))
                .expect("rendered buffer text");
            let text_column = grid.rows[text_row].find("This").unwrap();
            let character_foreground = grid
                .char_attrs
                .get(text_row)
                .and_then(|styles| styles.get(text_column))
                .and_then(|style| style.fg);
            let cell_foreground = grid
                .attrs
                .get(text_row)
                .and_then(|styles| styles.get(text_column))
                .and_then(|style| style.fg);
            let foreground = character_foreground
                .or(cell_foreground)
                .or(grid.default_style.fg);
            let remappings = interp.eval_str("face-remapping-alist").unwrap();
            let remappings = interp.prin1_to_string(&remappings);
            let row = &grid.rows[text_row];
            assert_eq!(
                foreground,
                Some(remacs_runtime::frontend::CellColor(255, 0, 0)),
                "row={row:?}; remappings={remappings}; default={:?}; char={character_foreground:?}; cell={cell_foreground:?}",
                grid.default_style.fg
            );
        })
        .unwrap()
        .join()
        .unwrap();
}
