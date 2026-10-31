use remacs_runtime::{
    frontend::render_grid,
    lisp::{Interp, Value},
};

fn eval(interp: &mut Interp, source: &str) -> Value {
    interp
        .eval_str(source)
        .unwrap_or_else(|flow| panic!("{source}: {flow:?}"))
}

fn check(interp: &mut Interp, menu: &str, expected: &[(&str, &str)]) {
    eval(interp, &format!("(x-popup-menu t {menu})"));
    let grid = render_grid(interp, 80, 24);
    let choices = grid
        .popup_menu_items
        .iter()
        .filter_map(|item| {
            let command = item.command.as_deref()?;
            let label = grid.rows[item.row]
                .chars()
                .skip(item.left)
                .take(item.width)
                .collect::<String>();
            Some((label.trim().to_owned(), command))
        })
        .collect::<Vec<_>>();
    assert_eq!(
        choices
            .iter()
            .map(|(label, command)| (label.as_str(), *command))
            .collect::<Vec<_>>(),
        expected,
        "menu: {menu}"
    );
    if expected.is_empty() {
        assert_eq!(
            grid.popup_menu_items.len(),
            2,
            "only the title and separator remain"
        );
        assert!(
            grid.popup_menu_items
                .iter()
                .all(|item| item.selection.is_none())
        );
    }
}

#[test]
fn popup_renders_all_keymap_panes_in_order() {
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(|| {
            let mut interp = Interp::new();
            eval(
                &mut interp,
                r#"(progn
                   (set-frame-parameter nil 'window-system 'ns)
                   (set-frame-parameter nil 'tool-bar-lines 0)
                   (set-frame-parameter nil 'tab-bar-lines 0)
                   (set-frame-size nil 80 24)
                   (setq qa-popup-first (make-sparse-keymap "First")
                         qa-popup-second (make-sparse-keymap "Second")
                         qa-popup-third (make-sparse-keymap "Third")
                         qa-popup-empty (make-sparse-keymap "Empty"))
                   (define-key qa-popup-first [one] '(menu-item "First item" ignore))
                   (define-key qa-popup-second [two] '(menu-item "Second item" forward-char))
                   (define-key qa-popup-third [three] '(menu-item "Third item" backward-char)))"#,
            );
            let first = ("First item", "ignore");
            let second = ("Second item", "forward-char");
            let third = ("Third item", "backward-char");
            check(
                &mut interp,
                "(list qa-popup-first qa-popup-second)",
                &[first, second],
            );
            check(
                &mut interp,
                "(list qa-popup-empty qa-popup-first qa-popup-empty qa-popup-second qa-popup-third)",
                &[first, second, third],
            );
            check(
                &mut interp,
                "(list qa-popup-second qa-popup-first)",
                &[second, first],
            );
            check(&mut interp, "qa-popup-first", &[first]);
            check(&mut interp, "(list qa-popup-first)", &[first]);
            check(
                &mut interp,
                "(list qa-popup-empty qa-popup-empty)",
                &[],
            );

            eval(
                &mut interp,
                r#"(define-key qa-popup-second [nested]
                    (list 'menu-item "More" qa-popup-third))"#,
            );
            check(
                &mut interp,
                "(list qa-popup-first qa-popup-second)",
                &[first, third, second],
            );
        })
        .unwrap()
        .join()
        .unwrap();
}
