use remacs_runtime::{
    frontend::{KeyIo, install_minibuf_reader, prepare_native_menus, render_grid},
    lisp::{Interp, Value, error::Flow},
};
use std::{cell::RefCell, collections::VecDeque, io, rc::Rc, time::Duration};

struct Input(VecDeque<i128>);

impl KeyIo for Input {
    fn poll_key(&mut self, _: Duration) -> io::Result<Option<i128>> {
        self.0
            .pop_front()
            .map(Some)
            .ok_or_else(|| io::ErrorKind::BrokenPipe.into())
    }
    fn unread(&mut self, key: i128) {
        self.0.push_front(key);
    }
    fn render(&mut self, _: &Interp) -> io::Result<()> {
        Ok(())
    }
    fn draw_echo(&mut self, _: &str) -> io::Result<()> {
        Ok(())
    }
    fn size(&self) -> (usize, usize) {
        (80, 24)
    }
}

fn assert_lisp(interp: &mut Interp, expression: &str) {
    assert!(
        interp.eval_str(expression).unwrap().truthy(),
        "{expression}"
    );
}

#[test]
fn hooks_update_native_menus_before_the_readonly_snapshot() {
    std::thread::Builder::new().stack_size(64 * 1024 * 1024).spawn(|| {
        let mut interp = Interp::new();
        interp.eval_str(r#"(progn
          (set-frame-parameter nil 'window-system 'ns)
          (setq global-map (make-sparse-keymap) activate-menubar-hook nil
                menu-bar-update-hook nil qa-menu-update-called nil)
          (menu-bar-mode 1)
          (add-hook 'menu-bar-update-hook
            (lambda ()
              (setq qa-menu-update-called t)
              (define-key global-map [menu-bar qa-dynamic] '("Dynamic" . ignore)))))"#).unwrap();
        prepare_native_menus(&mut interp).unwrap();
        assert_lisp(&mut interp, "qa-menu-update-called");
        let menus = render_grid(&interp, 80, 24).menus;
        assert!(menus.iter().any(|menu| menu.label == "Dynamic" && menu.command.as_deref() == Some("ignore")));

        // Activation precedes updating, in the selected window's buffer.
        // Lisp changes reach this snapshot, while readonly renders run no hooks.
        interp.eval_str(r#"(progn
          (rename-buffer "hook-selected" t)
          (setq qa-hook-order nil qa-hook-count 0)
          (defun qa-activate-menu () (setq qa-hook-order '(activate)))
          (defun qa-update-menu ()
            (setq qa-hook-order (append qa-hook-order '(update))
                  qa-hook-count (1+ qa-hook-count)
                  qa-hook-context (list (buffer-name) inhibit-quit debug-on-next-call
                                        overriding-local-map overriding-terminal-local-map))
            (string-match "x" "x")
            (define-key global-map [menu-bar qa-dynamic]
              (cons (format "Dynamic %d" qa-hook-count) 'ignore)))
          (setq activate-menubar-hook '(qa-activate-menu)
                menu-bar-update-hook '(qa-update-menu)
                overriding-local-map '(keymap) overriding-terminal-local-map '(keymap)
                debug-on-next-call nil)
          (set-buffer (get-buffer-create "hook-other"))
          (string-match "ab" "zabz"))"#).unwrap();
        let previous_buffer = interp.current_buffer;
        let matches = interp.eval_str("(match-data t)").unwrap();
        let matches = interp.prin1_to_string(&matches);
        let debug = interp.intern("debug-on-next-call");
        interp.set_symbol(debug, Value::t()).unwrap();
        prepare_native_menus(&mut interp).unwrap();
        assert_eq!(interp.current_buffer, previous_buffer);
        assert!(interp.symbol_value(debug).truthy());
        interp.set_symbol(debug, Value::Nil).unwrap();
        let after = interp.eval_str("(match-data t)").unwrap();
        assert_eq!(interp.prin1_to_string(&after), matches);
        assert_lisp(&mut interp, r#"(and (equal qa-hook-order '(activate update))
          (equal qa-hook-context '("hook-selected" t nil nil nil))
          (equal overriding-local-map '(keymap))
          (equal overriding-terminal-local-map '(keymap)))"#);
        for _ in 0..2 {
            assert!(render_grid(&interp, 80, 24).menus.iter().any(|menu| menu.label == "Dynamic 1"));
        }
        assert_lisp(&mut interp, "(= qa-hook-count 1)");
        prepare_native_menus(&mut interp).unwrap();
        assert!(render_grid(&interp, 80, 24).menus.iter().any(|menu| menu.label == "Dynamic 2"));

        // A failing callback is removed, later callbacks still update menus.
        interp.eval_str(r#"(progn
          (setq debug-on-next-call nil)
          (defun qa-bad-menu () (error "Broken menu updater"))
          (setq menu-bar-update-hook '(qa-bad-menu qa-update-menu)))"#).unwrap();
        prepare_native_menus(&mut interp).unwrap();
        assert_lisp(&mut interp, "(and (= qa-hook-count 3) (equal menu-bar-update-hook '(qa-update-menu)))");
        assert!(render_grid(&interp, 80, 24).menus.iter().any(|menu| menu.label == "Dynamic 3"));

        // Nonlocal exits propagate and unwind bindings and the reentry guard.
        interp.eval_str(r#"(progn
          (defun qa-exit-menu () (throw 'qa-menu-exit 'done))
          (setq menu-bar-update-hook '(qa-exit-menu))
          (string-match "ab" "zabz"))"#).unwrap();
        let tag = Value::Sym(interp.intern("qa-menu-exit"));
        interp.catch_tags.push(tag);
        let result = prepare_native_menus(&mut interp);
        interp.catch_tags.pop();
        assert!(matches!(result, Err(Flow::Throw(_, _))));
        assert_eq!(interp.current_buffer, previous_buffer);
        assert_lisp(&mut interp, "(and (equal (match-data t) '(1 3)) (not inhibit-quit) (equal overriding-local-map '(keymap)))");
        interp.eval_str("(setq menu-bar-update-hook '(qa-update-menu))").unwrap();
        prepare_native_menus(&mut interp).unwrap();
        assert_lisp(&mut interp, "(= qa-hook-count 4)");

        // Minibuffer redisplay during a hook must not recursively run it.
        interp.eval_str(r#"(progn
          (defun qa-prompt-menu ()
            (setq qa-hook-count (1+ qa-hook-count)
                  qa-hook-input (read-from-minibuffer "Menu value: ")))
          (setq menu-bar-update-hook '(qa-prompt-menu)))"#).unwrap();
        let input = Rc::new(RefCell::new(Input("ok\r".chars().map(|ch| ch as i128).collect())));
        install_minibuf_reader(&input, &mut interp);
        prepare_native_menus(&mut interp).unwrap();
        assert_lisp(&mut interp, r#"(and (= qa-hook-count 5) (equal qa-hook-input "ok"))"#);
        interp.eval_str("(menu-bar-mode -1)").unwrap();
        prepare_native_menus(&mut interp).unwrap();
        assert_lisp(&mut interp, "(= qa-hook-count 5)");
        assert!(render_grid(&interp, 80, 24).menus.is_empty());
    }).unwrap().join().unwrap();
}
