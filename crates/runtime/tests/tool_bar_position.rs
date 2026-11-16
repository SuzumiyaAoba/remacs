use remacs_runtime::{
    frontend::{prepare_native_menus, render_grid},
    lisp::Interp,
};

fn eval(interp: &mut Interp, source: &str) {
    interp
        .eval_str(source)
        .unwrap_or_else(|flow| panic!("{source}: {flow:?}"));
}

#[test]
fn tool_bar_position_bottom_places_toolbar_after_text_area() {
    std::thread::Builder::new()
        .stack_size(128 << 20)
        .spawn(|| {
            let mut interp = Interp::new();
            eval(
                &mut interp,
                r#"(progn
          (set-frame-parameter nil 'window-system 'gpui)
          (set-frame-parameter nil 'menu-bar-lines 0)
          (set-frame-parameter nil 'tool-bar-lines 1)
          (set-frame-parameter nil 'tab-bar-lines 0)
          (set-frame-parameter nil 'char-width 8)
          (set-frame-parameter nil 'char-height 16)
          (setq tool-bar-map (make-sparse-keymap))
          (defun qa-tool-bar-position-command () (interactive))
          (define-key tool-bar-map [qa-tool-bar-position]
            '(menu-item "Toolbar" qa-tool-bar-position-command))
          (set-frame-parameter nil 'tool-bar-position 'top))"#,
            );

            prepare_native_menus(&mut interp).unwrap();
            let top = render_grid(&interp, 60, 12);
            assert_eq!(top.tool_bar_items.len(), 1);
            assert_eq!(top.tool_bar_items[0].row, 0);

            eval(
                &mut interp,
                "(set-frame-parameter nil 'tool-bar-position 'bottom)",
            );
            prepare_native_menus(&mut interp).unwrap();
            let bottom = render_grid(&interp, 60, 12);
            assert_eq!(bottom.tool_bar_items.len(), 1);
            assert_eq!(bottom.tool_bar_items[0].row, 11);
        })
        .unwrap()
        .join()
        .unwrap();
}
