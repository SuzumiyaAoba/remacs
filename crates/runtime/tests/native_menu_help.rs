use remacs_runtime::{
    frontend::{GridMenuItem, prepare_native_menus, render_grid},
    lisp::Interp,
};

fn rows(interp: &Interp) -> Vec<GridMenuItem> {
    render_grid(interp, 80, 24)
        .menus
        .into_iter()
        .find(|menu| menu.label == "QA Help")
        .unwrap()
        .items
}

#[test]
fn native_menu_help_is_constant_prepared_text_and_updates_with_the_menu() {
    std::thread::Builder::new().stack_size(64 * 1024 * 1024).spawn(|| {
        let mut interp = Interp::new();
        interp.eval_str(r#"(progn
          (set-frame-parameter nil 'window-system 'ns)
          (setq global-map (make-sparse-keymap) menu-bar-update-hook nil qa-help-evals 0)
          (defun qa-menu-help-command () (interactive) (message "ran"))
          (global-set-key (kbd "C-c h") 'qa-menu-help-command)
          (setq qa-help-map
            '(keymap
              (run menu-item "Run" qa-menu-help-command :help "Runs the test command")
              (disabled menu-item "Disabled" ignore :enable nil :help "Still described")
              (heading menu-item "Heading" nil :help "Section description")
              (keys menu-item "Keys" qa-menu-help-command :help "Use \\[qa-menu-help-command]")
              (ignored menu-item "Ignored" ignore :help (progn (setq qa-help-evals (1+ qa-help-evals)) "No"))
              (duplicate menu-item "Duplicate" ignore :help "Old" :help nil)
              (legacy "Legacy" "Legacy help" . ignore)
              (more menu-item "More" (keymap (child menu-item "Child" ignore :help "Child help")) :help "Submenu help")))
          (setq qa-literal-help (propertize "Literal \\[qa-menu-help-command]" 'help-echo-inhibit-substitution t))
          (define-key qa-help-map [literal] (list 'menu-item "Literal" 'ignore :help qa-literal-help))
          (define-key global-map [menu-bar qa-help] (cons "QA Help" qa-help-map))
          (menu-bar-mode 1))"#).unwrap();
        prepare_native_menus(&mut interp).unwrap();
        let items = rows(&interp);
        let item = |name: &str| items.iter().find(|item| item.label == name).unwrap();
        assert_eq!(item("Run").help.as_deref(), Some("Runs the test command"));
        assert_eq!(item("Disabled").help.as_deref(), Some("Still described"));
        assert!(item("Disabled").command.is_none() && item("Disabled").command_id.is_none());
        assert_eq!(item("Heading").help.as_deref(), Some("Section description"));
        assert_eq!(item("Keys").help.as_deref(), Some("Use C-c h"));
        assert_eq!(item("Literal").help.as_deref(), Some("Literal \\[qa-menu-help-command]"));
        assert!(item("Ignored").help.is_none());
        assert!(item("Duplicate").help.is_none());
        assert_eq!(item("Legacy").help.as_deref(), Some("Legacy help"));
        assert_eq!(item("More").help.as_deref(), Some("Submenu help"));
        assert_eq!(item("More").items[0].help.as_deref(), Some("Child help"));
        assert!(interp.eval_str("(= qa-help-evals 0)").unwrap().truthy());
        interp.eval_str(r#"(define-key qa-help-map [run]
          '(menu-item "Run" qa-menu-help-command :help "Updated description"))"#).unwrap();
        prepare_native_menus(&mut interp).unwrap();
        assert_eq!(rows(&interp).iter().find(|item| item.label == "Run").unwrap().help.as_deref(), Some("Updated description"));
    }).unwrap().join().unwrap();
}
