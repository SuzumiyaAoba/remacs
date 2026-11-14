use remacs_runtime::{
    frontend::{prepare_native_menus, render_grid},
    lisp::Interp,
};

#[test]
fn toolbar_help_is_registered_for_hover_dispatch() {
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(|| {
            let mut i = Interp::new();
            i.eval_str(
                r#"(progn
                     (set-frame-parameter nil 'window-system 'ns)
                     (set-frame-parameter nil 'menu-bar-lines 0)
                     (set-frame-parameter nil 'tool-bar-lines 1)
                     (set-frame-parameter nil 'tab-bar-lines 0)
                     (setq menu-bar-mode nil tool-bar-map (make-sparse-keymap))
                     (defun qa-toolbar-help-command () (interactive))
                     (define-key tool-bar-map [qa-help]
                       '(menu-item "Help item" qa-toolbar-help-command
                                   :help "Toolbar help text")))"#,
            )
            .unwrap();
            prepare_native_menus(&mut i).unwrap();

            let grid = render_grid(&i, 80, 24);
            assert_eq!(grid.tool_bar_items.len(), 1);
            assert!(grid.help_echoes.iter().any(|help| {
                help.row == 0 && help.text == "Toolbar help text" && help.width > 0
            }));
        })
        .unwrap()
        .join()
        .unwrap();
}
