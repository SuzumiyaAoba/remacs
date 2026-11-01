use remacs_runtime::{
    frontend::{GridMenuItem, prepare_native_menus, render_grid},
    lisp::Interp,
};

struct KeysLibrary(std::path::PathBuf);

impl KeysLibrary {
    fn new() -> Self {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "remacs-menu-keys-{}-{nonce}.el",
            std::process::id()
        ));
        std::fs::write(
            &path,
            r#"(defun qa-autoload-keys ()
          (setq qa-key-autoloaded t) "Loaded \\[qa-key-command]")"#,
        )
        .unwrap();
        Self(path)
    }
}

impl Drop for KeysLibrary {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

fn hint(interp: &Interp, label: &str) -> Option<String> {
    fn find<'a>(items: &'a [GridMenuItem], label: &str) -> Option<&'a GridMenuItem> {
        items.iter().find_map(|item| {
            if item.label == label {
                Some(item)
            } else {
                find(&item.items, label)
            }
        })
    }
    let grid = render_grid(interp, 80, 24);
    grid.menus
        .iter()
        .find_map(|menu| find(&menu.items, label))
        .unwrap_or_else(|| panic!("missing menu item {label}"))
        .key_hint
        .clone()
}

#[test]
fn native_menu_key_hints_follow_bindings_properties_and_gnu_callback_order() {
    std::thread::Builder::new().stack_size(64 * 1024 * 1024).spawn(|| {
        let library = KeysLibrary::new();
        let mut interp = Interp::new();
        interp.eval_str("(set-frame-parameter nil 'window-system 'ns) (menu-bar-mode 1)").unwrap();
        // Exercise the shipped menu as well as isolated custom keymaps.
        prepare_native_menus(&mut interp).unwrap();
        assert_eq!(hint(&interp, "Save").as_deref(), Some("C-x C-s"));

        interp.eval_str(r#"(progn
          (setq global-map (make-sparse-keymap))
          (define-key global-map (kbd "C-c") (make-sparse-keymap))
          (defun qa-key-command () (interactive))
          (defun qa-other-command () (interactive))
          (defun qa-menu-only-command () (interactive))
          (global-set-key (kbd "C-c z") 'qa-key-command)
          (let ((map (make-sparse-keymap)))
            (define-key map [run] '(menu-item "Run" qa-key-command))
            (define-key global-map [menu-bar qa-keys] (cons "QA Keys" map))))"#).unwrap();
        prepare_native_menus(&mut interp).unwrap();
        assert_eq!(hint(&interp, "Run").as_deref(), Some("C-c z"));

        let source = interp.prin1_to_string(&remacs_runtime::lisp::Value::string(library.0.to_string_lossy().as_ref())).to_unicode_lossy();
        interp.eval_str(&format!("(autoload 'qa-autoload-keys {source} nil nil)")).unwrap();
        interp.eval_str(r#"(progn
          (global-set-key (kbd "C-c a") 'qa-key-command)
          (global-set-key (kbd "C-c x") 'qa-other-command)
          (defalias 'qa-key-alias 'qa-key-command)
          (setq qa-key-closure (lambda () (interactive)))
          (global-set-key (kbd "C-c l") qa-key-closure)
          (defalias 'qa-closure-alias qa-key-closure)
          (setq qa-key-log nil
                qa-keys-map
                '(keymap
                  (automatic menu-item "Automatic" qa-key-command)
                  (disabled menu-item "Disabled" qa-key-command :enable nil)
                  (literal menu-item "Literal" qa-key-command :keys "Use \\[qa-key-command]")
                  (empty menu-item "Empty" qa-key-command :keys "")
                  (function menu-item "Function" qa-key-command :keys (lambda () "\\[qa-key-command]"))
                  (autoload menu-item "Autoload" qa-key-command :keys qa-autoload-keys)
                  (valid menu-item "Valid" qa-key-command :keys "Ignored" :key-sequence [3 122])
                  (invalid menu-item "Invalid" qa-key-command :key-sequence [3 120])
                  (nil-hint menu-item "Nil hint" qa-key-command :keys "Ignored" :key-sequence nil)
                  (alias menu-item "Alias" qa-key-alias :key-sequence [3 122])
                  (closure menu-item "Closure alias" qa-closure-alias :key-sequence [3 108])
                  (alternate menu-item "Alternate" qa-key-command :keys (qa-other-command "<" . ">"))
                  (no-binding menu-item "No binding" qa-menu-only-command)
                  (invalid-value menu-item "Ignored value" qa-key-command :keys "Earlier" :keys 7)
                  (function-value menu-item "Function number" qa-key-command :keys (lambda () 7))
                  (last-valid menu-item "Last valid" qa-key-command :key-sequence [3 122] :key-sequence 7)
                  (text menu-item "Text" nil :keys "Text hint")
                  (branch menu-item "Branch" (keymap (child menu-item "Branch child" qa-key-command))
                    :keys "Branch \\[qa-key-command]")
                  (ordered menu-item (progn (push 'name qa-key-log) "Ordered") qa-key-command
                    :visible (progn (push (list 'visible inhibit-redisplay) qa-key-log) t)
                    :keys (lambda () (push (list 'keys inhibit-redisplay) qa-key-log) "__ORDER__")
                    :filter (lambda (binding) (push 'filter qa-key-log) binding)
                    :enable (progn (push (list 'enable inhibit-redisplay) qa-key-log) t)
                    :button (:toggle . (progn (push (list 'button inhibit-redisplay) qa-key-log) t)))))
          (advice-add 'substitute-command-keys :before
            (lambda (text &rest _) (when (equal text "__ORDER__") (push 'substitute qa-key-log))))
          (setq qa-live-hint
            (list 'menu-item '(progn (setcar (nthcdr 4 qa-live-hint) [3 122]) "Live hint")
                  'qa-key-command :key-sequence [3 120]))
          (define-key qa-keys-map [live] qa-live-hint)
          (define-key global-map [menu-bar qa-keys] (cons "QA Keys" qa-keys-map)))"#).unwrap();
        prepare_native_menus(&mut interp).unwrap();
        for (label, expected) in [
            ("Automatic", Some("C-c a")), ("Disabled", Some("C-c a")),
            ("Literal", Some("Use C-c a")), ("Empty", Some("")),
            ("Function", Some("C-c a")), ("Valid", Some("C-c z")),
            ("Autoload", Some("Loaded C-c a")),
            ("Invalid", Some("C-c a")), ("Nil hint", Some("C-c a")),
            ("Alias", Some("C-c z")), ("Closure alias", Some("C-c l")),
            ("Alternate", Some("<C-c x>")), ("No binding", None),
            ("Ignored value", Some("Earlier")), ("Function number", Some("C-c a")),
            ("Last valid", Some("C-c z")), ("Live hint", Some("C-c z")),
            ("Text", Some("Text hint")), ("Ordered", Some("__ORDER__")),
            ("Branch", Some(r"Branch \[qa-key-command]")),
        ] {
            assert_eq!(hint(&interp, label).as_deref(), expected, "hint of {label}");
        }
        assert!(interp.eval_str("qa-key-autoloaded").unwrap().truthy());
        assert!(interp.eval_str(
            "(equal (reverse qa-key-log) '((visible t) (keys nil) name filter (enable t) substitute (button t)))"
        ).unwrap().truthy(), "GNU's :keys callback is direct, before name/filter/enable and late substitution");
        // Read-only snapshots neither run key callbacks nor refresh key searches.
        interp.eval_str("(global-unset-key (kbd \"C-c a\")) (global-set-key (kbd \"C-c q\") 'qa-key-command)").unwrap();
        assert_eq!(hint(&interp, "Automatic").as_deref(), Some("C-c a"));
        interp.eval_str("(setq qa-key-log nil)").unwrap();
        prepare_native_menus(&mut interp).unwrap();
        assert_eq!(hint(&interp, "Automatic").as_deref(), Some("C-c q"));
        assert_eq!(hint(&interp, "Literal").as_deref(), Some("Use C-c q"));
        assert_eq!(hint(&interp, "Valid").as_deref(), Some("C-c z"));

        // GNU calls the C primitives directly, even when their Lisp cells change.
        interp.eval_str(r#"(progn
          (setq qa-old-key-binding (symbol-function 'key-binding)
                qa-old-where-is (symbol-function 'where-is-internal)
                qa-old-key-description (symbol-function 'key-description))
          (fset 'key-binding (lambda (&rest _) (error "Lisp key-binding called")))
          (fset 'where-is-internal (lambda (&rest _) (error "Lisp where-is called")))
          (fset 'key-description (lambda (&rest _) (error "Lisp key-description called")))
          (define-key global-map [menu-bar qa-keys]
            '("QA Keys" keymap (run menu-item "Direct" qa-key-command :key-sequence [3 122]))))"#).unwrap();
        prepare_native_menus(&mut interp).unwrap();
        assert_eq!(hint(&interp, "Direct").as_deref(), Some("C-c z"));
        interp.eval_str(r#"(fset 'key-binding qa-old-key-binding)
          (fset 'where-is-internal qa-old-where-is)
          (fset 'key-description qa-old-key-description)"#).unwrap();

        let previous = render_grid(&interp, 80, 24).menus;
        let state = interp.eval_str("(list inhibit-redisplay (boundp 'menu-updating-frame) (and (boundp 'menu-updating-frame) menu-updating-frame))").unwrap();
        let state = interp.prin1_to_string(&state).to_unicode_lossy();
        for binding in [
            "(menu-item \"Bad callback\" qa-key-command :keys (lambda () (error \"Bad keys\")))",
            "(menu-item \"Bad hint\" qa-key-command :key-sequence invalid-key-symbol)",
        ] {
            interp.eval_str(&format!("(define-key global-map [menu-bar qa-keys] '(\"QA Keys\" keymap (run . {binding})))")).unwrap();
            assert!(prepare_native_menus(&mut interp).is_err());
            assert_eq!(render_grid(&interp, 80, 24).menus, previous);
            let restored = interp.eval_str("(list inhibit-redisplay (boundp 'menu-updating-frame) (and (boundp 'menu-updating-frame) menu-updating-frame))").unwrap();
            assert_eq!(interp.prin1_to_string(&restored), state.as_str());
        }
        interp.eval_str("(define-key global-map [menu-bar qa-keys] '(\"QA Keys\" keymap (run menu-item \"Recovered\" qa-key-command)))").unwrap();
        prepare_native_menus(&mut interp).unwrap();
        assert_eq!(hint(&interp, "Recovered").as_deref(), Some("C-c q"));
    }).unwrap().join().unwrap();
}
