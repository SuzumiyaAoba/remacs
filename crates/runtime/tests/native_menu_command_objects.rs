use remacs_runtime::{
    frontend::{
        GridMenu, InputEvent, KeyIo, NativeMenuCommandId, WeakNativeMenuCommandId,
        prepare_native_menus, render_grid, run_editor_with_shared,
    },
    lisp::Interp,
};
use std::{cell::RefCell, collections::VecDeque, io, rc::Rc, time::Duration};

enum Step {
    Select(&'static str),
    Replace,
    Saved,
    Key(i128),
}

struct Frontend {
    menus: Vec<GridMenu>,
    steps: VecDeque<Step>,
    saved: Option<NativeMenuCommandId>,
    old_action: Option<WeakNativeMenuCommandId>,
}

impl Frontend {
    fn command(&self, label: &str) -> NativeMenuCommandId {
        for menu in &self.menus {
            if menu.label == label {
                return menu.command_id.clone().expect("root command object");
            }
            if let Some(item) = menu.items.iter().find(|item| item.label == label) {
                assert!(
                    item.command.is_none(),
                    "objects must not acquire symbol aliases"
                );
                return item.command_id.clone().expect("leaf command object");
            }
        }
        panic!("menu item missing: {label}");
    }
}

impl KeyIo for Frontend {
    fn poll_key(&mut self, duration: Duration) -> io::Result<Option<i128>> {
        match self.poll_input(duration)? {
            Some(InputEvent::Key(key)) => Ok(Some(key)),
            _ => panic!("expected a minibuffer key"),
        }
    }
    fn poll_input(&mut self, _: Duration) -> io::Result<Option<InputEvent>> {
        let event = match self.steps.pop_front().ok_or(io::ErrorKind::BrokenPipe)? {
            Step::Key(key) => InputEvent::Key(key),
            Step::Select(label) => InputEvent::NativeMenuCommand(self.command(label)),
            Step::Replace => {
                let token = self.command("Run captured");
                self.old_action = Some(token.downgrade());
                self.saved = Some(token);
                InputEvent::Command("qa-replace-menu".into())
            }
            Step::Saved => {
                let old = self.saved.take().unwrap();
                assert_ne!(old, self.command("Run captured"));
                InputEvent::NativeMenuCommand(old)
            }
        };
        Ok(Some(event))
    }
    fn unread(&mut self, key: i128) {
        self.steps.push_front(Step::Key(key));
    }
    fn render(&mut self, interp: &Interp) -> io::Result<()> {
        self.menus = render_grid(interp, 80, 24).menus;
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
fn command_objects_keep_their_environment_and_survive_menu_replacement() {
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(|| {
            let mut interp = Interp::new();
            interp
                .eval_str(
                    r#"(progn
          (set-frame-parameter nil 'window-system 'ns)
          (setq global-map (make-sparse-keymap) menu-bar-update-hook nil
                qa-lambda-runs 0 qa-root-runs 0 qa-symbol-runs 0
                qa-results nil qa-pre-runs 0 qa-post-runs 0)
          (defun qa-build-menu-command (initial)
            (eval `(let ((counter ,initial))
                     (lambda (prefix text)
                       (interactive "p\nsMenu value: ")
                       (setq counter (+ counter prefix))
                       (setq qa-results
                         (append qa-results (list (list counter text prefix)))))) t))
          (setq qa-first-command (qa-build-menu-command 40)
                qa-second-command (qa-build-menu-command 100)
                qa-map (make-sparse-keymap))
          (define-key qa-map [run]
            '(menu-item "Run lambda" (lambda () (interactive)
                                      (setq qa-lambda-runs (1+ qa-lambda-runs)))))
          (define-key qa-map [captured] (list 'menu-item "Run captured" qa-first-command))
          (define-key qa-map [disabled]
            '(menu-item "Disabled lambda" (lambda () (interactive)) :enable nil))
          (define-key qa-map [hidden]
            '(menu-item "Hidden lambda" (lambda () (interactive)) :visible nil))
          (let ((command (make-symbol "ignore")))
            (fset command (lambda () (interactive) (setq qa-symbol-runs (1+ qa-symbol-runs))))
            (define-key qa-map [uninterned] (list 'menu-item "Uninterned command" command)))
          (define-key global-map [menu-bar qa-lambda] (cons "QA Lambda" qa-map))
          (define-key global-map [menu-bar qa-root]
            '(menu-item "Root lambda" (lambda () (interactive)
                                       (setq qa-root-runs (1+ qa-root-runs)))))
          (defun qa-replace-menu () (interactive)
            (define-key qa-map [captured] (list 'menu-item "Run captured" qa-second-command))
            (setq prefix-arg 7))
          (setq pre-command-hook
            (list (lambda () (when (memq this-command (list qa-first-command qa-second-command))
                              (setq qa-pre-runs (1+ qa-pre-runs))))))
          (setq post-command-hook
            (list (lambda () (when (memq this-command (list qa-first-command qa-second-command))
                              (setq qa-post-runs (1+ qa-post-runs))))))
          (menu-bar-mode 1))"#,
                )
                .unwrap();
            prepare_native_menus(&mut interp).unwrap();
            let first = render_grid(&interp, 80, 24);
            let menu = first
                .menus
                .iter()
                .find(|menu| menu.label == "QA Lambda")
                .unwrap();
            let disabled = menu
                .items
                .iter()
                .find(|item| item.label == "Disabled lambda")
                .unwrap();
            assert!(disabled.command.is_none() && disabled.command_id.is_none());
            assert!(!menu.items.iter().any(|item| item.label == "Hidden lambda"));
            let token = menu
                .items
                .iter()
                .find(|item| item.label == "Run captured")
                .unwrap()
                .command_id
                .clone()
                .unwrap();
            prepare_native_menus(&mut interp).unwrap();
            let second = render_grid(&interp, 80, 24);
            assert_eq!(
                token,
                second
                    .menus
                    .iter()
                    .find(|menu| menu.label == "QA Lambda")
                    .unwrap()
                    .items
                    .iter()
                    .find(|item| item.label == "Run captured")
                    .unwrap()
                    .command_id
                    .clone()
                    .unwrap()
            );
            drop((token, first, second));

            let mut steps =
                VecDeque::from([Step::Select("Run lambda"), Step::Replace, Step::Saved]);
            steps.extend("chosen\r".chars().map(|ch| Step::Key(ch as i128)));
            steps.push_back(Step::Select("Run captured"));
            steps.extend("next\r".chars().map(|ch| Step::Key(ch as i128)));
            steps.extend([
                Step::Select("Uninterned command"),
                Step::Select("Root lambda"),
            ]);
            let frontend = Rc::new(RefCell::new(Frontend {
                menus: Vec::new(),
                steps,
                saved: None,
                old_action: None,
            }));
            let error = run_editor_with_shared(frontend.clone(), &mut interp).unwrap_err();
            assert_eq!(error.kind(), io::ErrorKind::BrokenPipe);
            let state = interp.eval_str("(list qa-lambda-runs qa-root-runs qa-symbol-runs qa-pre-runs qa-post-runs qa-results)").unwrap();
            let state = interp.prin1_to_string(&state);
            assert!(
                interp
                    .eval_str(
                        r#"(and (= qa-lambda-runs 1) (= qa-root-runs 1) (= qa-symbol-runs 1)
          (= qa-pre-runs 2) (= qa-post-runs 2)
          (equal qa-results '((47 "chosen" 7) (101 "next" 1))))"#
                    )
                    .unwrap()
                    .truthy(),
                "command results: {state}; message: {}", interp.echo_message
            );
            assert!(
                frontend
                    .borrow()
                    .old_action
                    .as_ref()
                    .unwrap()
                    .upgrade()
                    .is_none(),
                "a retired native action must not keep the old command alive"
            );
        })
        .unwrap()
        .join()
        .unwrap();
}
