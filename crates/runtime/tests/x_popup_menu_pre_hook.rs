use remacs_runtime::lisp::Interp;

#[test]
fn graphical_popup_menu_runs_x_pre_popup_menu_hook_after_validation() {
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(|| {
            let mut i = Interp::new();
            let result = i
                .eval_str(
                    r#"(progn
                         (set-frame-parameter nil 'window-system 'ns)
                         (setq qa-popup-hook-count 0)
                         (add-hook 'x-pre-popup-menu-hook
                                   (lambda ()
                                     (setq qa-popup-hook-count
                                           (1+ qa-popup-hook-count))))
                         (condition-case nil
                             (x-popup-menu t '(nil))
                           (error nil))
                         (let ((map (make-sparse-keymap "Menu")))
                           (define-key map [item] '(menu-item "Item" ignore))
                           (x-popup-menu t map))
                         qa-popup-hook-count)"#,
                )
                .unwrap();

            assert_eq!(i.prin1_to_string(&result).to_unicode_lossy(), "1");
            assert!(i.gui_popup_menu.is_some());
        })
        .unwrap()
        .join()
        .unwrap();
}
