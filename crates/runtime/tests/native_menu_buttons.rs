use remacs_runtime::{
    frontend::{prepare_native_menus, render_grid},
    lisp::Interp,
};

fn check_rows(interp: &Interp, selected: bool) {
    let grid = render_grid(interp, 80, 24);
    let menu = grid
        .menus
        .iter()
        .find(|menu| menu.label == "QA Button")
        .unwrap();
    for (label, checked, command) in [
        ("Toggle", true, Some("ignore")),
        ("Dynamic", selected, Some("ignore")),
        ("Radio", selected, Some("ignore")),
        ("Disabled", selected, None),
        ("Plain", false, Some("ignore")),
        ("Invalid", false, Some("ignore")),
        ("Broken", false, Some("ignore")),
        ("List state", false, Some("ignore")),
        ("Duplicate", true, Some("ignore")),
        ("Last", false, Some("ignore")),
        ("Ordered", selected, Some("ignore")),
        ("Text", false, None),
        ("Submenu", false, None),
    ] {
        let item = menu.items.iter().find(|item| item.label == label).unwrap();
        assert_eq!(item.checked, checked, "check state of {label}");
        assert_eq!(item.command.as_deref(), command, "command of {label}");
    }
    assert!(!menu.items.iter().any(|item| item.label == "Hidden"));
    let submenu = menu
        .items
        .iter()
        .find(|item| item.label == "Submenu")
        .unwrap();
    assert_eq!(submenu.items[0].label, "Child");
}

#[test]
fn native_menu_buttons_evaluate_leaf_states_and_refresh_the_snapshot() {
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(|| {
            let mut interp = Interp::new();
            interp
                .eval_str(
                    r#"(progn
          (set-frame-parameter nil 'window-system 'ns)
          (setq global-map (make-sparse-keymap)
                qa-button-state t qa-button-trace nil qa-button-errors 0)
          (defun qa-button-filter (binding)
            (push 'filter qa-button-trace) binding)
          (let ((map (make-sparse-keymap)) (child (make-sparse-keymap)))
            (define-key child [child] '(menu-item "Child" ignore))
            (define-key map [toggle] '(menu-item "Toggle" ignore :button (:toggle . t)))
            (define-key map [dynamic] '(menu-item "Dynamic" ignore :button (:toggle . qa-button-state)))
            (define-key map [radio] '(menu-item "Radio" ignore :button (:radio . qa-button-state)))
            (define-key map [disabled]
              '(menu-item "Disabled" ignore :enable nil :button (:toggle . qa-button-state)))
            (define-key map [plain] '(menu-item "Plain" ignore))
            (define-key map [invalid]
              '(menu-item "Invalid" ignore :button (:other . (push 'invalid qa-button-trace))))
            (define-key map [broken]
              '(menu-item "Broken" ignore
                 :button (:radio . (progn (setq qa-button-errors (1+ qa-button-errors))
                                         (error "Bad selection state")))))
            (define-key map [list] '(menu-item "List state" ignore :button (:toggle t)))
            (define-key map [duplicate]
              '(menu-item "Duplicate" ignore :button (:toggle . t)
                 :button (:other . (push 'duplicate qa-button-trace))))
            (define-key map [last]
              '(menu-item "Last" ignore :button (:toggle . t) :button (:radio . nil)))
            (define-key map [ordered]
              '(menu-item (progn (push 'name qa-button-trace) "Ordered") ignore
                 :filter qa-button-filter
                 :enable (progn (push 'enable qa-button-trace) t)
                 :button (:toggle . (progn (push 'button qa-button-trace) qa-button-state))))
            (define-key map [hidden]
              '(menu-item "Hidden" ignore :visible nil
                 :button (:toggle . (push 'hidden qa-button-trace))))
            (define-key map [text]
              '(menu-item "Text" nil :button (:toggle . (push 'text qa-button-trace))))
            (define-key map [submenu]
              (list 'menu-item "Submenu" child
                    :button '(:toggle . (push 'submenu qa-button-trace))))
            (define-key global-map [menu-bar qa-button]
              (list 'menu-item "QA Button" map
                    :button '(:toggle . (push 'top qa-button-trace)))))
          (menu-bar-mode 1))"#,
                )
                .unwrap();

            prepare_native_menus(&mut interp).unwrap();
            check_rows(&interp, true);
            assert!(
                interp
                    .eval_str(
                        "(and (equal (reverse qa-button-trace) '(name filter enable button)) (= qa-button-errors 1))"
                    )
                    .unwrap()
                    .truthy(),
                "evaluate only eligible states, after the other menu properties"
            );

            interp.eval_str("(setq qa-button-state nil qa-button-trace nil)").unwrap();
            prepare_native_menus(&mut interp).unwrap();
            check_rows(&interp, false);
            assert!(
                interp
                    .eval_str(
                        "(and (equal (reverse qa-button-trace) '(name filter enable button)) (= qa-button-errors 2))"
                    )
                    .unwrap()
                    .truthy(),
                "read-only snapshots must not evaluate selection state again"
            );
        })
        .unwrap()
        .join()
        .unwrap();
}
