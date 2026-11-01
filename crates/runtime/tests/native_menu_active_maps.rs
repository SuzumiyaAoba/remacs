use remacs_runtime::{
    frontend::{GridMenu, GridMenuItem, prepare_native_menus, render_grid},
    lisp::{Interp, Value},
};

fn roots(interp: &Interp) -> Vec<String> {
    render_grid(interp, 80, 24)
        .menus
        .into_iter()
        .map(|menu| menu.label)
        .collect()
}

fn menu(interp: &Interp, name: &str) -> GridMenu {
    render_grid(interp, 80, 24)
        .menus
        .into_iter()
        .find(|menu| menu.label == name)
        .unwrap_or_else(|| panic!("missing menu {name}"))
}

fn labels(items: &[GridMenuItem]) -> Vec<&str> {
    items.iter().map(|item| item.label.as_str()).collect()
}

#[test]
fn native_menu_collects_the_selected_buffers_active_maps() {
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(|| {
            let mut interp = Interp::new();
            let canonical = interp
                .eval_str("(keymap-canonicalize '(keymap (a . ignore) (b . next-line)))")
                .unwrap();
            assert_eq!(interp.prin1_to_string(&canonical), "(keymap (a . ignore) (b . next-line))");
            interp
                .eval_str(
                    r#"(progn
          (set-frame-parameter nil 'window-system 'ns)
          (setq global-map (make-sparse-keymap)
                minor-mode-map-alist nil minor-mode-overriding-map-alist nil
                emulation-mode-map-alists nil)
          (let ((map (make-sparse-keymap)))
            (define-key map [menu-bar qa-local]
              (cons "Local" (make-sparse-keymap "Local")))
            (define-key map [menu-bar qa-local run]
              '(menu-item "Run Local" ignore))
            (use-local-map map))
          (menu-bar-mode 1))"#,
                )
                .unwrap();
            prepare_native_menus(&mut interp).unwrap();
            let grid = render_grid(&interp, 80, 24);
            let local_menu = grid
                .menus
                .iter()
                .find(|menu| menu.label == "Local")
                .expect("local menu must be visible");
            assert_eq!(local_menu.items[0].label, "Run Local");
            assert_eq!(local_menu.items[0].command.as_deref(), Some("ignore"));

            // Canonicalization retains high-priority bindings and their order.
            assert!(interp.eval_str(r#"(and
              (equal (keymap-canonicalize '(keymap (a . ignore) (b . next-line) (a . previous-line) (c . forward-char)))
                     '(keymap (a . ignore) (b . next-line) (c . forward-char)))
              (let ((child '(keymap (c . ignore) (dup . next-line)))
                    (parent '(keymap (dup . previous-line) (p . forward-char))))
                (set-keymap-parent child parent)
                (equal (keymap-canonicalize child) '(keymap (c . ignore) (dup . next-line) (p . forward-char)))))"#).unwrap().truthy());
            assert!(interp.eval_str(r#"(let ((child (make-keymap)) (parent (make-keymap)))
              (define-key child [(99 . 102)] 'qa-high)
              (define-key parent [(97 . 100)] 'qa-low)
              (set-keymap-parent child parent)
              (let ((canonical (keymap-canonicalize child)))
                (equal (mapcar (lambda (key) (lookup-key canonical (vector key))) '(97 98 99 100 101 102))
                       '(qa-low qa-low qa-high qa-high qa-high qa-high))))"#).unwrap().truthy());
            interp.eval_str(r#"(progn
              (defun qa-menu-source (key label menu)
                (let ((map (make-sparse-keymap)))
                  (define-key map (vector 'menu-bar key) (cons label menu)) map))
              (setq qa-global-label (copy-sequence "QA shared")
                    qa-global '(keymap "Global pane"
                                (dup menu-item "Global duplicate" previous-line)
                                (global menu-item "Global action" ignore))
                    qa-local '(keymap "Local pane"
                               (dup menu-item "Local duplicate" next-line)
                               (local menu-item "Local action" ignore))
                    qa-minor '(keymap "Minor pane" (minor menu-item "Minor action" ignore))
                    global-map (qa-menu-source 'qa-shared qa-global-label qa-global)
                    qa-local-map (qa-menu-source 'qa-shared "Ignored local label" qa-local)
                    qa-minor-map (qa-menu-source 'qa-shared "Ignored minor label" qa-minor)
                    qa-minor-on t minor-mode-map-alist (list (cons 'qa-minor-on qa-minor-map)))
              (define-key qa-local-map [menu-bar qa-shared]
                (list 'menu-item '(progn (aset qa-global-label 3 ?S) "Ignored local label") qa-local))
              (use-local-map qa-local-map))"#).unwrap();
            prepare_native_menus(&mut interp).unwrap();
            let panes = menu(&interp, "QA Shared").items;
            assert_eq!(labels(&panes), ["Minor pane", "Local pane", "Global pane"]);
            assert_eq!(labels(&panes[0].items), ["Minor action"]);
            assert_eq!(labels(&panes[1].items), ["Local duplicate", "Local action"]);
            assert_eq!(labels(&panes[2].items), ["Global duplicate", "Global action"]);
            assert_eq!(panes[1].items[0].command.as_deref(), Some("next-line"));
            assert_eq!(panes[2].items[0].command.as_deref(), Some("previous-line"));

            interp.eval_str("(setq qa-minor-on nil)").unwrap();
            prepare_native_menus(&mut interp).unwrap();
            assert_eq!(labels(&menu(&interp, "QA Shared").items), ["Local pane", "Global pane"]);
            interp.eval_str("(use-local-map nil)").unwrap();
            prepare_native_menus(&mut interp).unwrap();
            assert_eq!(labels(&menu(&interp, "QA Shared").items), ["Global duplicate", "Global action"]);

            // Inheritance composes within one source instead of adding panes.
            interp.eval_str(r#"(progn
              (setq qa-parent '(keymap "Parent"
                                 (dup menu-item "Parent duplicate" previous-line)
                                 (parent menu-item "Parent action" ignore))
                    qa-child '(keymap "Child"
                                (dup menu-item "Child duplicate" next-line)
                                (child menu-item "Child action" ignore)))
              (set-keymap-parent qa-child qa-parent)
              (setq global-map (make-sparse-keymap)
                    qa-parent-top (qa-menu-source 'qa-inherited "Inherited" qa-parent)
                    qa-child-top (qa-menu-source 'qa-inherited "Inherited child" qa-child))
              (set-keymap-parent qa-child-top qa-parent-top)
              (fset 'qa-menu-alias qa-child-top) (fset 'qa-menu-alias-outer 'qa-menu-alias)
              (setq qa-minor-on t minor-mode-map-alist '((qa-minor-on . qa-menu-alias-outer)))
              (use-local-map nil))"#).unwrap();
            prepare_native_menus(&mut interp).unwrap();
            let inherited = menu(&interp, "Inherited child").items;
            assert_eq!(labels(&inherited), ["Child duplicate", "Child action", "Parent action"]);
            assert_eq!(inherited[0].command.as_deref(), Some("next-line"));

            interp.eval_str(r#"(progn
              (fset 'qa-pane-alias qa-local) (fset 'qa-pane-alias-outer 'qa-pane-alias)
              (setq global-map (qa-menu-source 'qa-alias "Aliased pane" 'qa-pane-alias-outer)
                    minor-mode-map-alist nil))"#).unwrap();
            prepare_native_menus(&mut interp).unwrap();
            assert_eq!(labels(&menu(&interp, "Aliased pane").items), ["Local duplicate", "Local action"]);

            // Point, emulation, overriding-minor, major and global sources.
            interp.eval_str(r#"(progn
              (setq global-map (qa-menu-source 'qa-global "Global root" '(keymap (run menu-item "Global" ignore)))
                    qa-major (qa-menu-source 'qa-major "Major root" '(keymap (run menu-item "Major" ignore)))
                    qa-regular (qa-menu-source 'qa-regular "Regular root" '(keymap (run menu-item "Regular" ignore)))
                    qa-minor-override (qa-menu-source 'qa-over "Minor override root" '(keymap (run menu-item "Override" ignore)))
                    qa-emulation (qa-menu-source 'qa-emul "Emulation root" '(keymap (run menu-item "Emulation" ignore)))
                    qa-point (qa-menu-source 'qa-point "Point root" '(keymap (run menu-item "Point" ignore)))
                    qa-property-local (qa-menu-source 'qa-prop-local "Property local root" '(keymap (run menu-item "Property local" ignore)))
                    qa-terminal (qa-menu-source 'qa-terminal "Terminal root" '(keymap (run menu-item "Terminal" ignore)))
                    qa-overriding (qa-menu-source 'qa-overriding "Overriding root" '(keymap (run menu-item "Overriding" ignore)))
                    qa-minor-on t qa-emulation-on t
                    minor-mode-map-alist (list (cons 'qa-minor-on qa-regular))
                    minor-mode-overriding-map-alist (list (cons 'qa-minor-on qa-minor-override))
                    qa-emulation-alist (list (cons 'qa-emulation-on qa-emulation))
                    emulation-mode-map-alists '(qa-emulation-alist))
              (use-local-map qa-major) (erase-buffer) (insert "abc") (goto-char 2)
              (put-text-property 1 4 'keymap qa-point))"#).unwrap();
            prepare_native_menus(&mut interp).unwrap();
            assert_eq!(roots(&interp), ["Global root", "Major root", "Minor override root", "Emulation root", "Point root"]);
            interp.eval_str("(put-text-property 1 4 'local-map qa-property-local)").unwrap();
            prepare_native_menus(&mut interp).unwrap();
            assert_eq!(roots(&interp), ["Global root", "Property local root", "Minor override root", "Emulation root", "Point root"]);
            // get_local_map reads sticky properties outside an empty narrowing.
            interp.eval_str("(remove-text-properties 1 4 '(keymap nil local-map nil)) (put-text-property 1 2 'keymap qa-point) (setq-local text-property-default-nonsticky nil) (narrow-to-region 2 2)").unwrap();
            prepare_native_menus(&mut interp).unwrap();
            assert!(roots(&interp).contains(&"Point root".to_owned()));
            assert!(interp.eval_str("(and (= (point-min) 2) (= (point-max) 2))").unwrap().truthy());
            interp.eval_str("(widen) (remove-text-properties 1 4 '(keymap nil))").unwrap();

            interp.eval_str(r#"(define-key global-map [menu-bar qa-global]
              '(menu-item "Global root" (keymap (run menu-item "Global" ignore))
                 :visible (or overriding-local-map-menu-flag
                              (and (null overriding-local-map) (null overriding-terminal-local-map)))))"#).unwrap();
            interp.eval_str("(setq overriding-terminal-local-map qa-terminal overriding-local-map qa-overriding overriding-local-map-menu-flag nil)").unwrap();
            prepare_native_menus(&mut interp).unwrap();
            assert_eq!(roots(&interp), ["Global root", "Major root", "Minor override root", "Emulation root"]);
            assert!(interp.eval_str("(and (eq overriding-terminal-local-map qa-terminal) (eq overriding-local-map qa-overriding))").unwrap().truthy());
            interp.eval_str("(setq overriding-local-map-menu-flag t)").unwrap();
            prepare_native_menus(&mut interp).unwrap();
            assert_eq!(roots(&interp), ["Global root", "Overriding root", "Terminal root"]);
            interp.eval_str("(setq overriding-local-map nil)").unwrap();
            prepare_native_menus(&mut interp).unwrap();
            assert_eq!(roots(&interp), ["Global root", "Major root", "Minor override root", "Emulation root", "Terminal root"]);

            // Global menus precede mode additions, with Help moved to the end.
            interp.eval_str(r#"(progn
              (setq overriding-local-map nil overriding-terminal-local-map nil
                    overriding-local-map-menu-flag nil emulation-mode-map-alists nil
                    minor-mode-map-alist nil minor-mode-overriding-map-alist nil
                    menu-bar-final-items '(help-menu)
                    global-map (qa-menu-source 'file "File" '(keymap (run menu-item "File action" ignore))))
              (define-key global-map [menu-bar help-menu]
                '("Help" keymap (run menu-item "Help action" ignore)))
              (use-local-map qa-major))"#).unwrap();
            prepare_native_menus(&mut interp).unwrap();
            assert_eq!(roots(&interp), ["File", "Major root", "Help"]);

            // undefined cancels the global entry; a higher source can add it back.
            interp.eval_str(r#"(progn
              (setq overriding-terminal-local-map nil overriding-local-map-menu-flag nil
                    emulation-mode-map-alists nil minor-mode-overriding-map-alist nil
                    global-map (qa-menu-source 'qa-shared "Global label" qa-global))
              (use-local-map qa-local-map)
              (define-key qa-local-map [menu-bar qa-shared] 'undefined)
              (setq minor-mode-map-alist nil))"#).unwrap();
            prepare_native_menus(&mut interp).unwrap();
            assert!(roots(&interp).is_empty());
            interp.eval_str("(setq minor-mode-map-alist (list (cons 'qa-minor-on qa-minor-map)))").unwrap();
            prepare_native_menus(&mut interp).unwrap();
            assert_eq!(roots(&interp), ["Ignored minor label"]);
            assert_eq!(labels(&menu(&interp, "Ignored minor label").items), ["Minor action"]);

            // Canonicalization is observable and safe_funcall suppresses errors/quit.
            interp.eval_str(r#"(progn
              (setq qa-old-canonical (symbol-function 'keymap-canonicalize)
                    qa-debugged nil debugger (lambda (&rest _) (setq qa-debugged t))
                    debug-on-error t debug-on-quit t)
              (fset 'keymap-canonicalize (lambda (_) (error "Bad canonicalizer"))))"#).unwrap();
            prepare_native_menus(&mut interp).unwrap();
            assert!(roots(&interp).is_empty());
            assert!(!interp.eval_str("qa-debugged").unwrap().truthy());
            interp.eval_str("(fset 'keymap-canonicalize (lambda (_) (signal 'quit nil)))").unwrap();
            prepare_native_menus(&mut interp).unwrap();
            assert!(roots(&interp).is_empty());
            interp.eval_str("(fset 'keymap-canonicalize qa-old-canonical)").unwrap();
            prepare_native_menus(&mut interp).unwrap();
            assert_eq!(roots(&interp), ["Ignored minor label"]);
            let debug = interp.intern("debug-on-next-call");
            interp.set_symbol(debug, Value::t()).unwrap();
            prepare_native_menus(&mut interp).unwrap();
            assert!(interp.symbol_value(debug).truthy(), "menu construction must preserve the caller's step request");
            interp.set_symbol(debug, Value::Nil).unwrap();
            assert!(!interp.eval_str("qa-debugged").unwrap().truthy());

            // Evaluate the selected window's maps, then restore the caller.
            interp.eval_str(r#"(progn
              (define-key qa-minor-map [menu-bar qa-shared]
                (list 'menu-item '(progn (string-match "x" "x") "Ignored minor label") qa-minor))
              (rename-buffer "menu-selected" t)
              (setq-local qa-minor-on t)
              (with-current-buffer (get-buffer-create "menu-other")
                (setq-local qa-minor-on nil)
                (use-local-map (qa-menu-source 'qa-other "Other root" '(keymap (run menu-item "Other" ignore)))))
              (set-buffer "menu-other") (string-match "before" "before"))"#).unwrap();
            let caller = interp.current_buffer;
            let matches = interp.eval_str("(match-data t)").unwrap();
            let matches = interp.prin1_to_string(&matches).to_unicode_lossy();
            prepare_native_menus(&mut interp).unwrap();
            assert_eq!(roots(&interp), ["Ignored minor label"]);
            assert_eq!(interp.current_buffer, caller);
            let after = interp.eval_str("(match-data t)").unwrap();
            assert_eq!(interp.prin1_to_string(&after), matches.as_str());


        })
        .unwrap()
        .join()
        .unwrap();
}
