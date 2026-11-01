use remacs_runtime::{
    frontend::{
        GridMenu, InputEvent, KeyIo, install_minibuf_reader, prepare_native_menus, render_grid,
        run_editor_with_shared,
    },
    lisp::Interp,
};
use std::{cell::RefCell, collections::VecDeque, io, rc::Rc, time::Duration};

#[derive(Default)]
struct Frontend {
    menus: Vec<GridMenu>,
    keys: VecDeque<i128>,
}

impl KeyIo for Frontend {
    fn poll_key(&mut self, _: Duration) -> io::Result<Option<i128>> {
        self.keys
            .pop_front()
            .map(Some)
            .ok_or_else(|| io::ErrorKind::BrokenPipe.into())
    }
    fn poll_input(&mut self, duration: Duration) -> io::Result<Option<InputEvent>> {
        self.poll_key(duration).map(|key| key.map(InputEvent::Key))
    }
    fn unread(&mut self, _: i128) {}
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
fn native_menu_conditions_are_prepared_before_the_readonly_snapshot() {
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(|| {
            let mut interp = Interp::new();
            interp
                .eval_str(
                    r#"(progn
          (set-frame-parameter nil 'window-system 'ns)
          (setq pre-redisplay-function nil)
          (setq-local fontification-functions nil)
          (setq global-map (make-sparse-keymap))
          (let ((map (make-sparse-keymap)))
            (define-key map [disabled] '(menu-item "Disabled" ignore :enable nil))
            (define-key map [hidden] '(menu-item "Hidden" ignore :visible nil))
            (define-key global-map [menu-bar qa-native] (cons "QA Test" map)))
          (menu-bar-mode 1))"#,
                )
                .unwrap();
            let frontend = Rc::new(RefCell::new(Frontend::default()));
            let error = run_editor_with_shared(frontend.clone(), &mut interp).unwrap_err();
            assert_eq!(error.kind(), io::ErrorKind::BrokenPipe);
            {
                let frontend = frontend.borrow();
                let menu = frontend
                    .menus
                    .iter()
                    .find(|menu| menu.label == "QA Test")
                    .expect("native menu must reach the rendered snapshot");
                assert_eq!(
                    menu.items
                        .iter()
                        .map(|item| (item.label.as_str(), item.command.as_deref()))
                        .collect::<Vec<_>>(),
                    [("Disabled", None)]
                );
            }

            interp
                .eval_str(
                    r#"(progn
          (rename-buffer "native-selected" t)
          (defvar qa-native-visible nil)
          (defvar qa-native-enabled nil)
          (setq-local qa-native-visible t qa-native-enabled nil)
          (with-current-buffer (get-buffer-create "native-other")
            (setq-local qa-native-visible nil qa-native-enabled t))
          (let ((map (make-sparse-keymap)))
            (define-key map [dynamic]
              '(menu-item (concat "Dynamic " (buffer-name)) ignore
                 :visible qa-native-visible :enable qa-native-enabled))
            (define-key global-map [menu-bar qa-native] (cons "QA Test" map)))
          (set-buffer "native-other")
          (string-match "ab" "zabz"))"#,
                )
                .unwrap();
            let buffer = interp.current_buffer;
            let matches = interp.eval_str("(match-data t)").unwrap();
            let matches = interp.prin1_to_string(&matches).to_unicode_lossy();
            prepare_native_menus(&mut interp).unwrap();
            assert_eq!(interp.current_buffer, buffer);
            let after = interp.eval_str("(match-data t)").unwrap();
            assert_eq!(interp.prin1_to_string(&after), matches.as_str());
            let grid = render_grid(&interp, 80, 24);
            let menu = grid
                .menus
                .iter()
                .find(|menu| menu.label == "QA Test")
                .unwrap();
            assert_eq!(menu.items[0].label, "Dynamic native-selected");
            assert!(menu.items[0].command.is_none());
            interp
                .eval_str("(with-current-buffer \"native-selected\" (setq qa-native-enabled t))")
                .unwrap();
            prepare_native_menus(&mut interp).unwrap();
            let grid = render_grid(&interp, 80, 24);
            assert_eq!(
                grid.menus
                    .iter()
                    .find(|menu| menu.label == "QA Test")
                    .unwrap()
                    .items[0]
                    .command
                    .as_deref(),
                Some("ignore")
            );
            interp
                .eval_str("(with-current-buffer \"native-selected\" (setq qa-native-visible nil))")
                .unwrap();
            prepare_native_menus(&mut interp).unwrap();
            assert!(
                !render_grid(&interp, 80, 24)
                    .menus
                    .iter()
                    .any(|menu| menu.label == "QA Test")
            );

            // Conditions may use nested input. Menu preparation must neither hold
            // the frontend borrow nor recursively recompute the same menu.
            interp
                .eval_str(
                    r#"(let ((map (make-sparse-keymap)))
          (define-key map [prompt]
            '(menu-item "Prompted" ignore :enable (equal (read-from-minibuffer "Enable? ") "y")))
          (define-key global-map [menu-bar qa-native] (cons "QA Test" map)))"#,
                )
                .unwrap();
            let frontend = Rc::new(RefCell::new(Frontend {
                menus: Vec::new(),
                keys: VecDeque::from(['y' as i128, 13]),
            }));
            install_minibuf_reader(&frontend, &mut interp);
            assert_eq!(
                run_editor_with_shared(frontend.clone(), &mut interp)
                    .unwrap_err()
                    .kind(),
                io::ErrorKind::BrokenPipe
            );
            let frontend = frontend.borrow();
            assert_eq!(
                frontend
                    .menus
                    .iter()
                    .find(|menu| menu.label == "QA Test")
                    .unwrap()
                    .items[0]
                    .command
                    .as_deref(),
                Some("ignore")
            );
        })
        .unwrap()
        .join()
        .unwrap();
}
