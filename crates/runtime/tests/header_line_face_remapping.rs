use remacs_runtime::{
    frontend::{CellColor, render_grid},
    lisp::Interp,
};

#[test]
fn header_line_uses_the_buffer_face_remapping() {
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
                      (set-frame-size nil 80 12)
                      (setq-default mode-line-format nil tab-line-format nil)
                      (setq-local header-line-format "Header")
                      (face-remap-add-relative 'header-line '(:foreground "red"))
                      (erase-buffer)
                      (insert "body"))"#,
                )
                .unwrap();

            let grid = render_grid(&interp, 80, 12);
            let header_row = grid
                .header_lines
                .first()
                .map(|header| header.row)
                .expect("rendered header line");
            let header_text = grid.rows.get(header_row).expect("header row text");
            let header_column = header_text.find("Header").expect("header line content");
            let foreground = grid
                .char_attrs
                .get(header_row)
                .filter(|styles| !styles.is_empty())
                .and_then(|styles| styles.get(header_column))
                .and_then(|style| style.fg)
                .or_else(|| {
                    grid.attrs
                        .get(header_row)
                        .and_then(|styles| styles.get(header_column))
                        .and_then(|style| style.fg)
                });
            assert_eq!(foreground, Some(CellColor(255, 0, 0)));
        })
        .unwrap()
        .join()
        .unwrap();
}
