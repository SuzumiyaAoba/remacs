use remacs_runtime::{
    frontend::{prepare_native_menus, render_grid},
    lisp::Interp,
};

fn eval(interp: &mut Interp, source: &str) {
    interp
        .eval_str(source)
        .unwrap_or_else(|flow| panic!("{source}: {flow:?}"));
}

fn graphical_tab_bar() -> Interp {
    let mut interp = Interp::new();
    eval(
        &mut interp,
        r#"(progn
          (provide 'cus-edit)
          (defmacro easy-menu-define (&rest _) nil)
          (require 'tab-bar)
          (set-frame-parameter nil 'window-system 'ns)
          (set-frame-parameter nil 'menu-bar-lines 0)
          (set-frame-parameter nil 'tool-bar-lines 0)
          (set-frame-parameter nil 'tab-bar-lines 1)
          (set-frame-parameter nil 'tabs
            '((tab (name . "Other")) (current-tab (name . "Current")))))"#,
    );
    interp
}

#[test]
fn default_tab_bar_format_keeps_tabs_and_add_button() {
    let mut interp = graphical_tab_bar();
    prepare_native_menus(&mut interp).unwrap();

    let grid = render_grid(&interp, 60, 12);
    assert!(grid.rows[0].contains("Current"));
    assert!(grid.rows[0].contains("Other"));
    assert!(grid.rows[0].contains("+"));
    assert!(grid.tab_bar_items.iter().any(|item| {
        (item.command == "tab-bar-new-tab" || item.command_id.is_some()) && item.argument.is_none()
    }));
    assert!(
        grid.tab_bar_items
            .iter()
            .any(|item| item.command == "tab-bar-select-tab" && item.argument == Some(1))
    );
    assert!(
        grid.tab_bar_items
            .iter()
            .any(|item| item.command == "tab-bar-close-tab" && item.argument == Some(1))
    );
}

#[test]
fn nil_tab_bar_format_hides_the_bar_buttons() {
    let mut interp = graphical_tab_bar();
    eval(&mut interp, "(setq tab-bar-format nil)");
    prepare_native_menus(&mut interp).unwrap();

    let grid = render_grid(&interp, 60, 12);
    assert!(grid.tab_bar_items.is_empty());
    assert!(!grid.rows[0].contains("Current"));
    assert!(!grid.rows[0].contains("+"));
}

#[test]
fn tab_bar_format_renders_items_in_function_order() {
    let mut interp = graphical_tab_bar();
    eval(
        &mut interp,
        r#"(progn
          (defun qa-tab-format-command () (interactive))
          (defun qa-tab-format-first ()
            '((first menu-item "First" ignore)))
          (defun qa-tab-format-second ()
            '((second menu-item "Second" qa-tab-format-command)))
          (setq tab-bar-format '(qa-tab-format-second qa-tab-format-first)))"#,
    );
    prepare_native_menus(&mut interp).unwrap();

    let grid = render_grid(&interp, 60, 12);
    let row = &grid.rows[0];
    let second = row.find("Second").expect("second format item");
    let first = row.find("First").expect("first format item");
    assert!(
        second < first,
        "items should follow tab-bar-format order: {row:?}"
    );
    assert_eq!(grid.tab_bar_items.len(), 1);
    assert!(grid.tab_bar_items[0].command_id.is_some());
}
