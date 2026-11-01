use remacs_runtime::{
    frontend::{prepare_native_menus, render_grid},
    lisp::Interp,
};

fn order(interp: &Interp) -> Vec<String> {
    render_grid(interp, 80, 24)
        .menus
        .into_iter()
        .map(|menu| menu.items[0].label.clone())
        .collect()
}

#[test]
fn final_menu_keys_move_to_the_end_in_requested_order() {
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(|| {
            let mut interp = Interp::new();
            // The reported reproduction against the shipped global map.
            interp
                .eval_str("(set-frame-parameter nil 'window-system 'ns) (menu-bar-mode 1) (setq menu-bar-final-items '(file))")
                .unwrap();
            prepare_native_menus(&mut interp).unwrap();
            assert_eq!(render_grid(&interp, 80, 24).menus.last().unwrap().label, "File");

            interp
                .eval_str(
                    r#"(progn
          (use-local-map nil)
          (setq minor-mode-map-alist nil minor-mode-overriding-map-alist nil
                emulation-mode-map-alists nil menu-bar-final-items nil
                global-map
                '(keymap (menu-bar keymap
                  (file menu-item "File" (keymap (run menu-item "primary-file" ignore)))
                  (edit menu-item "Edit" (keymap (run menu-item "edit" ignore)))
                  (help-menu menu-item "Help" (keymap (run menu-item "help" ignore)))
                  (other-file menu-item "File" (keymap (run menu-item "other-file" ignore)))))))"#,
                )
                .unwrap();
            prepare_native_menus(&mut interp).unwrap();
            assert_eq!(order(&interp), ["primary-file", "edit", "help", "other-file"]);
            interp.eval_str("(setq menu-bar-final-items '(file))").unwrap();
            assert_eq!(order(&interp), ["primary-file", "edit", "help", "other-file"],
                "a read-only snapshot is stable until the next preparation");
            prepare_native_menus(&mut interp).unwrap();
            assert_eq!(order(&interp), ["edit", "help", "other-file", "primary-file"]);

            for (keys, expected) in [
                ("nil", vec!["primary-file", "edit", "help", "other-file"]),
                ("'(help-menu file)", vec!["edit", "other-file", "help", "primary-file"]),
                ("'(file help-menu)", vec!["edit", "other-file", "primary-file", "help"]),
                ("'(missing file file help-menu)", vec!["edit", "other-file", "primary-file", "help"]),
                ("'(other-file file)", vec!["edit", "help", "other-file", "primary-file"]),
                ("'(missing \"File\" 7 edit)", vec!["primary-file", "help", "other-file", "edit"]),
                ("'(file . ignored-tail)", vec!["edit", "help", "other-file", "primary-file"]),
                ("'file", vec!["primary-file", "edit", "help", "other-file"]),
            ] {
                interp.eval_str(&format!("(setq menu-bar-final-items {keys})")).unwrap();
                prepare_native_menus(&mut interp).unwrap();
                assert_eq!(order(&interp), expected, "final keys: {keys}");
            }

            // Reordering runs after local entries have joined the menu bar.
            interp.eval_str(r#"(use-local-map
              '(keymap (menu-bar keymap
                 (mode menu-item "Mode" (keymap (run menu-item "mode" ignore))))))
              (setq menu-bar-final-items '(file))"#).unwrap();
            prepare_native_menus(&mut interp).unwrap();
            assert_eq!(order(&interp), ["edit", "help", "other-file", "mode", "primary-file"]);
        })
        .unwrap()
        .join()
        .unwrap();
}
