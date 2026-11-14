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
fn tab_bar_position_controls_the_order_of_tab_and_tool_bars() {
    let mut interp = Interp::new();
    eval(
        &mut interp,
        r#"(progn
          (provide 'cus-edit)
          (defmacro easy-menu-define (&rest _) nil)
          (require 'tab-bar)
          (set-frame-parameter nil 'window-system 'ns)
          (set-frame-parameter nil 'menu-bar-lines 0)
          (set-frame-parameter nil 'tool-bar-lines 1)
          (set-frame-parameter nil 'tab-bar-lines 1)
          (set-frame-parameter nil 'char-width 8)
          (set-frame-parameter nil 'char-height 16)
          (defun qa-position-toolbar-command () (interactive))
          (define-key tool-bar-map [qa-position]
            '(menu-item "Toolbar" qa-position-toolbar-command))
          (defun qa-position-tab-command () (interactive))
          (defun qa-position-tab-format ()
            '((qa-position menu-item "Tabbar" qa-position-tab-command)))
          (setq tab-bar-format '(qa-position-tab-format)))"#,
    );

    prepare_native_menus(&mut interp).unwrap();
    let top = render_grid(&interp, 60, 12);
    assert_eq!(top.tool_bar_items[0].row, 1);
    assert_eq!(top.tab_bar_items[0].row, 0);

    eval(
        &mut interp,
        "(set-frame-parameter nil 'tab-bar-position 'bottom)",
    );
    prepare_native_menus(&mut interp).unwrap();
    let bottom = render_grid(&interp, 60, 12);
    assert_eq!(bottom.tool_bar_items[0].row, 0);
    assert_eq!(bottom.tab_bar_items[0].row, 1);
}
