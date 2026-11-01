use remacs_runtime::{
    frontend::{
        GridMenu, InputEvent, KeyIo, prepare_native_menus, render_grid, run_editor_with_shared,
    },
    lisp::Interp,
};
use std::{cell::RefCell, collections::VecDeque, io, rc::Rc, time::Duration};

struct Frontend {
    menus: Vec<GridMenu>,
    select_item: bool,
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
        if std::mem::take(&mut self.select_item) {
            let menu = self
                .menus
                .iter()
                .find(|menu| menu.label == "QA Filter")
                .unwrap();
            let item = menu
                .items
                .iter()
                .find(|item| item.label == "Visit file")
                .unwrap();
            return Ok(Some(InputEvent::Command(item.command.clone().unwrap())));
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
fn native_menu_filter_replaces_the_binding_before_snapshot_and_dispatch() {
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
          (setq global-map (make-sparse-keymap)
                qa-filter-result 'find-file qa-filter-calls 0)
          (defun qa-menu-filter (binding)
            (setq qa-filter-input binding
                  qa-filter-calls (1+ qa-filter-calls))
            qa-filter-result)
          (defun qa-filter-selected (text)
            (interactive "sFilter value: ")
            (setq qa-selected-text text))
          (let ((map (make-sparse-keymap)))
            (define-key map [visit]
              '(menu-item "Visit file" ignore :filter qa-menu-filter))
            (define-key global-map [menu-bar qa-filter] (cons "QA Filter" map)))
          (menu-bar-mode 1))"#,
                )
                .unwrap();

            // The reported reproduction must advertise find-file, and the
            // filter receives the original symbol as data rather than a call.
            prepare_native_menus(&mut interp).unwrap();
            let grid = render_grid(&interp, 80, 24);
            let menu = grid
                .menus
                .iter()
                .find(|menu| menu.label == "QA Filter")
                .unwrap();
            assert_eq!(menu.items[0].command.as_deref(), Some("find-file"));
            assert!(
                interp
                    .eval_str("(and (eq qa-filter-input 'ignore) (= qa-filter-calls 1))")
                    .unwrap()
                    .truthy()
            );
            // Read-only rendering must not run the filter again.
            let _ = render_grid(&interp, 80, 24);
            assert!(interp.eval_str("(= qa-filter-calls 1)").unwrap().truthy());

            interp
                .eval_str("(setq qa-filter-result 'qa-filter-selected)")
                .unwrap();
            let frontend = Rc::new(RefCell::new(Frontend {
                menus: Vec::new(),
                select_item: true,
                keys: "chosen\r".chars().map(|ch| ch as i128).collect(),
            }));
            assert_eq!(
                run_editor_with_shared(frontend, &mut interp)
                    .unwrap_err()
                    .kind(),
                io::ErrorKind::BrokenPipe
            );
            assert!(
                interp
                    .eval_str(
                        "(and (equal qa-selected-text \"chosen\") (eq qa-filter-input 'ignore))"
                    )
                    .unwrap()
                    .truthy(),
                "selecting the menu row must run the replacement's interactive spec"
            );

            // A filter can also turn a command into an entire submenu.
            interp
                .eval_str(
                    r#"(setq qa-filter-result
          (let ((map (make-sparse-keymap)))
            (define-key map [child] '(menu-item "Filtered child" ignore))
            map))"#,
                )
                .unwrap();
            prepare_native_menus(&mut interp).unwrap();
            let grid = render_grid(&interp, 80, 24);
            let item = &grid
                .menus
                .iter()
                .find(|menu| menu.label == "QA Filter")
                .unwrap()
                .items[0];
            assert!(item.command.is_none());
            assert_eq!(item.items[0].label, "Filtered child");
            assert_eq!(item.items[0].command.as_deref(), Some("ignore"));
        })
        .unwrap()
        .join()
        .unwrap();
}
