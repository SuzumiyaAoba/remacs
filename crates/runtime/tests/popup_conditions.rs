use remacs_runtime::{
    frontend::{Grid, InputEvent, KeyIo, install_minibuf_reader, render_grid},
    lisp::{Interp, Value},
};
use std::{cell::RefCell, io, rc::Rc, time::Duration};

fn eval(interp: &mut Interp, source: &str) -> Value {
    interp
        .eval_str(source)
        .unwrap_or_else(|flow| panic!("{source}: {flow:?}"))
}

fn rows(interp: &Interp) -> Vec<(String, bool)> {
    let grid = render_grid(interp, 80, 24);
    grid.popup_menu_items
        .iter()
        .map(|item| {
            let label = grid.rows[item.row]
                .chars()
                .skip(item.left)
                .take(item.width)
                .collect::<String>()
                .trim()
                .to_owned();
            (label, item.selection.is_some())
        })
        .collect()
}

#[derive(Default)]
struct Input {
    grid: Option<Grid>,
    polls: usize,
}

impl KeyIo for Input {
    fn poll_key(&mut self, _: Duration) -> io::Result<Option<i128>> {
        unreachable!()
    }
    fn poll_key_or_mouse(&mut self, _: Duration) -> io::Result<Option<InputEvent>> {
        let grid = self.grid.as_ref().unwrap();
        let label = if self.polls == 0 {
            "Disabled"
        } else {
            "Enabled"
        };
        let (index, item) = grid
            .popup_menu_items
            .iter()
            .enumerate()
            .find(|(_, item)| {
                grid.rows[item.row]
                    .chars()
                    .skip(item.left)
                    .take(item.width)
                    .collect::<String>()
                    .trim()
                    == label
            })
            .unwrap();
        self.polls += 1;
        Ok(Some(InputEvent::PopupMenuSelection {
            menu_id: item.menu_id,
            index: Some(index),
        }))
    }
    fn unread(&mut self, _: i128) {}
    fn render(&mut self, interp: &Interp) -> io::Result<()> {
        self.grid = Some(render_grid(interp, 80, 24));
        Ok(())
    }
    fn draw_echo(&mut self, _: &str) -> io::Result<()> {
        Ok(())
    }
    fn size(&self) -> (usize, usize) {
        (80, 24)
    }
}

#[test]
fn popup_visibility_and_enable_forms_control_rendering_and_selection() {
    std::thread::Builder::new().stack_size(64 * 1024 * 1024).spawn(|| {
        let mut interp = Interp::new();
        eval(&mut interp, r#"(progn
          (set-frame-parameter nil 'window-system 'ns)
          (setq qa-condition-map (make-sparse-keymap)
                qa-condition-log nil qa-condition-commands 0)
          (defun qa-condition-command () (interactive)
            (setq qa-condition-commands (1+ qa-condition-commands)))
          (define-key qa-condition-map [hidden]
            '(menu-item "Hidden" qa-condition-command :visible nil))
          (define-key qa-condition-map [disabled]
            '(menu-item "Disabled" qa-condition-command :enable nil))
          (x-popup-menu t qa-condition-map))"#);
        assert_eq!(rows(&interp), [("Disabled".into(), false)]);

        eval(&mut interp, r#"(progn
          (setq qa-condition-log nil qa-condition-visible nil qa-condition-enabled nil
                qa-condition-map (make-sparse-keymap))
          (define-key qa-condition-map [item]
            '(menu-item "Dynamic" ignore
               :enable (progn (push 'old-enable qa-condition-log) t)
               :visible (progn (push 'visible qa-condition-log) qa-condition-visible)
               :enable (progn (push (list 'enable inhibit-redisplay) qa-condition-log)
                              qa-condition-enabled)))
          (x-popup-menu t qa-condition-map))"#);
        assert!(rows(&interp).is_empty());
        let log = eval(&mut interp, "qa-condition-log");
        assert_eq!(interp.prin1_to_string(&log), "(visible)");
        eval(&mut interp, "(setq qa-condition-log nil qa-condition-visible t) (x-popup-menu t qa-condition-map)");
        assert_eq!(rows(&interp), [("Dynamic".into(), false)]);
        let log = eval(&mut interp, "qa-condition-log");
        assert_eq!(interp.prin1_to_string(&log), "((enable t) visible)");
        eval(&mut interp, "(setq qa-condition-enabled 0)");
        assert_eq!(rows(&interp), [("Dynamic".into(), false)], "an open snapshot is stable");
        eval(&mut interp, "(x-popup-menu t qa-condition-map)");
        assert_eq!(rows(&interp), [("Dynamic".into(), true)]);

        eval(&mut interp, r#"(progn
          (setq qa-condition-global t)
          (eval '(let ((qa-condition-global nil))
                   (x-popup-menu t '(keymap (item menu-item "Global" ignore
                                                   :visible qa-condition-global
                                                   :enable qa-condition-global)))) t))"#);
        assert_eq!(rows(&interp), [("Global".into(), true)], "properties use dynamic eval, not the caller's lexical environment");

        eval(&mut interp, r#"(progn
          (setq qa-condition-binding
                (list 'menu-item "Live properties" 'ignore
                      :visible '(progn (setcar (nthcdr 6 qa-condition-binding) nil) t)
                      :enable t))
          (x-popup-menu t (list 'keymap (cons 'item qa-condition-binding))))"#);
        assert_eq!(rows(&interp), [("Live properties".into(), false)]);

        eval(&mut interp, r#"(progn
          (setq qa-condition-log nil qa-condition-parent (make-sparse-keymap)
                qa-condition-child (make-sparse-keymap))
          (define-key qa-condition-child [child]
            '(menu-item "Child" ignore :visible (progn (push 'child qa-condition-log) t)))
          (define-key qa-condition-parent [parent]
            (list 'menu-item "Parent" qa-condition-child :enable nil))
          (x-popup-menu t qa-condition-parent))"#);
        assert_eq!(rows(&interp), [("Parent".into(), false)]);
        assert!(eval(&mut interp, "qa-condition-log").is_nil());

        eval(&mut interp, r#"(progn
          (setq qa-condition-debugged nil)
          (put 'qa-menu-condition 'error-conditions '(qa-menu-condition))
          (let ((debug-on-error t)
                (debugger (lambda (&rest _) (setq qa-condition-debugged t))))
            (x-popup-menu t '(keymap
              (hidden menu-item "Error hidden" ignore :visible (signal 'qa-menu-condition nil))
              (disabled menu-item "Error disabled" ignore :enable (error "disabled"))))))"#);
        assert_eq!(rows(&interp), [("Error disabled".into(), false)]);
        assert!(eval(&mut interp, "qa-condition-debugged").is_nil());
        eval(&mut interp, r#"(let ((enable-disabled-menus-and-buttons t))
          (x-popup-menu t '(keymap
            (enabled menu-item "Forced" ignore :enable (error "must not evaluate"))
            (hidden menu-item "Still hidden" ignore :visible nil))))"#);
        assert_eq!(rows(&interp), [("Forced".into(), true)]);
        let restored = eval(&mut interp, "(list inhibit-redisplay enable-disabled-menus-and-buttons)");
        assert_eq!(interp.prin1_to_string(&restored), "(nil nil)");

        interp.gui_popup_menu = None;
        let quit = eval(&mut interp, r#"(condition-case nil
          (x-popup-menu t '(keymap (item menu-item "Quit" ignore :visible (signal 'quit nil))))
          (quit 'quit))"#);
        assert_eq!(interp.prin1_to_string(&quit), "quit");
        assert!(interp.gui_popup_menu.is_none());
        let restored = eval(&mut interp, "inhibit-redisplay");
        assert!(restored.is_nil());

        let input = Rc::new(RefCell::new(Input::default()));
        install_minibuf_reader(&input, &mut interp);
        let choice = eval(&mut interp, r#"(let ((map (make-sparse-keymap)))
          (define-key map [enabled] '(menu-item "Enabled" qa-condition-command))
          (define-key map [disabled] '(menu-item "Disabled" qa-condition-command :enable nil))
          (list (x-popup-menu t map) qa-condition-commands))"#);
        assert_eq!(interp.prin1_to_string(&choice), "((enabled) 0)");
        assert_eq!(input.borrow().polls, 2, "even a stale disabled index cannot select the command");
        assert!(interp.gui_popup_menu.is_none());
    }).unwrap().join().unwrap();
}
