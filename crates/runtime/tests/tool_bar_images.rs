use remacs_runtime::{
    frontend::{GridImageSource, prepare_native_menus, render_grid},
    lisp::Interp,
};

#[test]
fn toolbar_image_properties_are_evaluated_and_rendered() {
    let mut i = Interp::new();
    i.eval_str(
        r#"(progn
          (set-frame-parameter nil 'window-system 'ns)
          (set-frame-parameter nil 'char-width 8)
          (set-frame-parameter nil 'char-height 16)
          (set-frame-parameter nil 'menu-bar-lines 0)
          (set-frame-parameter nil 'tool-bar-lines 1)
          (set-frame-parameter nil 'tab-bar-lines 0)
          (setq menu-bar-mode nil tool-bar-map (make-sparse-keymap))
          (defvar qa-toolbar-image-calls 0)
          (defun qa-toolbar-image ()
            (setq qa-toolbar-image-calls (1+ qa-toolbar-image-calls))
            '(image :type svg :data
              "<svg xmlns='http://www.w3.org/2000/svg' width='12' height='24'/>"))
          (define-key tool-bar-map [icon]
            '(menu-item "Visible label" ignore :image (qa-toolbar-image))))"#,
    )
    .unwrap();

    prepare_native_menus(&mut i).unwrap();
    let calls = i.eval_str("qa-toolbar-image-calls").unwrap();
    assert_eq!(calls.int(), Some(1));

    let grid = render_grid(&i, 60, 12);
    assert_eq!(grid.images.len(), 1);
    let image = &grid.images[0];
    assert_eq!(image.row, 0);
    assert_eq!(image.width, Some(12));
    assert_eq!(image.height, Some(24));
    assert!(matches!(image.source, GridImageSource::Data { ref format, .. } if format == "svg"));
    assert!(grid.row_heights[image.row] >= 24);

    let button = &grid.tool_bar_items[0];
    let (start, end) = button.byte_range.expect("toolbar button byte range");
    assert!(grid.rows[button.row][start..end].trim().is_empty());
    assert!(!grid.rows[button.row].contains("Visible label"));
}
