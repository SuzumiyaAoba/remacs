use remacs_runtime::{
    frontend::{
        GridMenu, InputEvent, KeyIo, prepare_native_menus, render_grid, run_editor_with_shared,
    },
    lisp::Interp,
};
use std::{cell::RefCell, collections::VecDeque, io, rc::Rc, time::Duration};

struct Frontend {
    menus: Vec<GridMenu>,
    select: bool,
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
        if std::mem::take(&mut self.select) {
            let root = self
                .menus
                .iter()
                .find(|menu| menu.label == "Run interactive")
                .unwrap();
            assert!(root.items.is_empty());
            return Ok(Some(InputEvent::Command(root.command.clone().unwrap())));
        }
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
fn top_level_menu_commands_are_visible_without_a_submenu() {
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(|| {
            let mut interp = Interp::new();
            interp
                .eval_str(
                    r#"(progn
          (set-frame-parameter nil 'window-system 'ns)
          (setq global-map (make-sparse-keymap) minor-mode-map-alist nil
                minor-mode-overriding-map-alist nil emulation-mode-map-alists nil)
          (use-local-map nil)
          (define-key global-map [menu-bar qa-run-now] '(menu-item "Run now" ignore))
          (menu-bar-mode 1))"#,
                )
                .unwrap();
            prepare_native_menus(&mut interp).unwrap();
            let grid = render_grid(&interp, 80, 24);
            let root = grid
                .menus
                .iter()
                .find(|menu| menu.label == "Run now")
                .expect("a top-level command must remain in the menu bar");
            assert!(root.items.is_empty());
            assert_eq!(root.command.as_deref(), Some("ignore"));

            interp.eval_str(r#"(progn
              (setq qa-root-on t qa-root-visible t qa-root-runs 0)
              (defun qa-root-command (text) (interactive "sTop level value: ")
                (setq qa-root-runs (1+ qa-root-runs) qa-root-text text))
              (define-key global-map [menu-bar qa-run-now]
                '(menu-item "Run interactive" qa-root-command :enable qa-root-on :visible qa-root-visible))
              (define-key global-map [menu-bar qa-one]
                '("One item" keymap (child menu-item "Child" ignore))))"#).unwrap();
            prepare_native_menus(&mut interp).unwrap();
            let menus = render_grid(&interp, 80, 24).menus;
            let submenu = menus.iter().find(|menu| menu.label == "One item").unwrap();
            assert!(submenu.command.is_none());
            assert_eq!(submenu.items.len(), 1);
            for variable in ["qa-root-on", "qa-root-visible"] {
                interp.eval_str(&format!("(setq {variable} nil)")).unwrap();
                prepare_native_menus(&mut interp).unwrap();
                assert!(!render_grid(&interp, 80, 24).menus.iter().any(|menu| menu.label == "Run interactive"));
                interp.eval_str(&format!("(setq {variable} t)")).unwrap();
            }

            // Higher-priority maps can replace an action with a submenu and back.
            interp.eval_str(r#"(use-local-map '(keymap (menu-bar keymap
              (qa-run-now menu-item "Ignored local label" (keymap (local menu-item "Local child" ignore))))))"#).unwrap();
            prepare_native_menus(&mut interp).unwrap();
            let menus = render_grid(&interp, 80, 24).menus;
            let root = menus.iter().find(|menu| menu.label == "Run interactive").unwrap();
            assert!(root.command.is_none());
            assert_eq!(root.items[0].label, "Local child");
            interp.eval_str(r#"(use-local-map '(keymap (menu-bar keymap
              (qa-run-now menu-item "Ignored local label" qa-root-command))))"#).unwrap();
            prepare_native_menus(&mut interp).unwrap();
            let menus = render_grid(&interp, 80, 24).menus;
            let root = menus.iter().find(|menu| menu.label == "Run interactive").unwrap();
            assert_eq!(root.command.as_deref(), Some("qa-root-command"));
            assert!(root.items.is_empty());
            interp.eval_str("(use-local-map nil)").unwrap();

            let frontend = Rc::new(RefCell::new(Frontend {
                menus: Vec::new(), select: true,
                keys: "chosen\r".chars().map(|ch| ch as i128).collect(),
            }));
            let error = run_editor_with_shared(frontend, &mut interp).unwrap_err();
            assert_eq!(error.kind(), io::ErrorKind::BrokenPipe);
            assert!(interp.eval_str("(and (= qa-root-runs 1) (equal qa-root-text \"chosen\"))").unwrap().truthy(),
                "one root selection must call the command's interactive specification exactly once");
        })
        .unwrap()
        .join()
        .unwrap();
}
