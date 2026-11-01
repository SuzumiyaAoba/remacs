use remacs_runtime::{
    frontend::{prepare_native_menus, render_grid},
    lisp::{Interp, Value},
};

fn check_item(interp: &mut Interp, label: &str, separator: bool) {
    prepare_native_menus(interp).unwrap();
    let grid = render_grid(interp, 80, 24);
    let menu = grid
        .menus
        .iter()
        .find(|menu| menu.label == "QA Separators")
        .unwrap();
    assert_eq!(menu.items.len(), 1);
    let item = &menu.items[0];
    assert_eq!(item.label, label);
    assert_eq!(item.separator, separator, "native separator: {label:?}");
    assert_eq!(item.command.as_deref(), (!separator).then_some("ignore"));

    interp
        .eval_str("(x-popup-menu t qa-separator-map)")
        .unwrap();
    let menu = interp.gui_popup_menu.as_ref().unwrap();
    assert_eq!(menu.entries.len(), 1);
    let item = &menu.entries[0];
    assert_eq!(item.separator, separator, "popup separator: {label:?}");
    assert_eq!(item.command.as_deref(), (!separator).then_some("ignore"));
    assert_eq!(item.value.is_some(), !separator);
}

#[test]
fn native_and_popup_separators_use_gnu_names_instead_of_event_name_heuristics() {
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(|| {
            let mut interp = Interp::new();
            interp
                .eval_str(
                    r#"(progn
          (set-frame-parameter nil 'window-system 'ns)
          (setq global-map (make-sparse-keymap)
                qa-separator-map (make-sparse-keymap))
          (define-key qa-separator-map [line] '(menu-item "--single-line"))
          (define-key qa-separator-map [label] '(menu-item "separator options" ignore))
          (define-key global-map [menu-bar qa-separators]
            (cons "QA Separators" qa-separator-map))
          (menu-bar-mode 1))"#,
                )
                .unwrap();
            prepare_native_menus(&mut interp).unwrap();
            let grid = render_grid(&interp, 80, 24);
            let menu = grid
                .menus
                .iter()
                .find(|menu| menu.label == "QA Separators")
                .unwrap();
            assert!(
                menu.items
                    .iter()
                    .find(|item| item.label == "--single-line")
                    .unwrap()
                    .separator
            );
            let item = menu
                .items
                .iter()
                .find(|item| item.label == "separator options")
                .unwrap();
            assert!(!item.separator);
            assert_eq!(item.command.as_deref(), Some("ignore"));

            let separators = [
                "",
                "-",
                "--",
                "-----",
                "--space",
                "--no-line",
                "--single-line",
                "--double-line",
                "--single-dashed-line",
                "--double-dashed-line",
                "--shadow-etched-in",
                "--shadow-etched-out",
                "--shadow-etched-in-dash",
                "--shadow-etched-out-dash",
                "--shadow-double-etched-in",
                "--shadow-double-etched-out",
                "--shadow-double-etched-in-dash",
                "--shadow-double-etched-out-dash",
                "--single-line\0ignored",
                "\0ignored",
            ];
            let labels = [
                "separator options",
                "separator",
                "Separator",
                "--unknown",
                "--x",
                "---single-line",
                "--SINGLE-LINE",
                "--space extra",
                "x--single-line",
                "-single-line",
            ];
            for (label, separator) in separators
                .into_iter()
                .map(|label| (label, true))
                .chain(labels.into_iter().map(|label| (label, false)))
            {
                let literal = interp
                    .prin1_to_string(&Value::string(label))
                    .to_unicode_lossy();
                // Constant and dynamic extended names, plus the old dotted form,
                // all describe the same native label and separator state.
                for binding in [
                    format!("'(menu-item {literal} ignore)"),
                    format!("'(menu-item (identity {literal}) ignore)"),
                    format!("(cons {literal} 'ignore)"),
                ] {
                    interp
                        .eval_str(&format!(
                            "(setq qa-separator-map (make-sparse-keymap)) \
                         (define-key qa-separator-map [item] {binding}) \
                         (define-key global-map [menu-bar qa-separators] \
                           (cons \"QA Separators\" qa-separator-map))"
                        ))
                        .unwrap();
                    check_item(&mut interp, label, separator);
                }
            }

            interp
                .eval_str(
                    "(setq qa-separator-map (make-sparse-keymap)) \
                 (define-key qa-separator-map [separator-command] 'ignore) \
                 (define-key global-map [menu-bar qa-separators] \
                   (cons \"QA Separators\" qa-separator-map))",
                )
                .unwrap();
            check_item(&mut interp, "separator-command", false);
        })
        .unwrap()
        .join()
        .unwrap();
}
