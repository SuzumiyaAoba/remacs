use remacs_runtime::{
    frontend::{Grid, InputEvent, KeyIo, install_minibuf_reader, render_grid},
    lisp::Interp,
};
use std::{cell::RefCell, io, rc::Rc, time::Duration};

#[derive(Default)]
struct Input {
    grid: Option<Grid>,
    labels: Vec<String>,
}

impl KeyIo for Input {
    fn poll_key(&mut self, _: Duration) -> io::Result<Option<i128>> {
        panic!("popup must read a selection event");
    }
    fn poll_key_or_mouse(&mut self, _: Duration) -> io::Result<Option<InputEvent>> {
        let item = self
            .grid
            .as_ref()
            .and_then(|grid| {
                grid.popup_menu_items
                    .iter()
                    .find(|item| item.selection.is_some())
            })
            .ok_or_else(|| io::Error::other("popup has no selectable item"))?;
        Ok(Some(InputEvent::PopupMenuSelection {
            menu_id: item.menu_id,
            index: item.selection,
        }))
    }
    fn unread(&mut self, _: i128) {}
    fn render(&mut self, interp: &Interp) -> io::Result<()> {
        let grid = render_grid(interp, 80, 24);
        if !grid.popup_menu_items.is_empty() {
            self.labels = grid
                .popup_menu_items
                .iter()
                .map(|item| {
                    grid.rows[item.row]
                        .chars()
                        .skip(item.left)
                        .take(item.width)
                        .collect::<String>()
                        .trim()
                        .to_owned()
                })
                .collect();
        }
        self.grid = Some(grid);
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
fn unlabelled_bindings_keep_fallback_labels_and_return_their_key_paths() {
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(|| {
            let mut interp = Interp::new();
            interp
                .eval_str(
                    "(progn
                       (set-frame-parameter nil 'window-system 'ns)
                       (setq qa-bare-popup-commands 0)
                       (defun qa-bare-popup-command () (interactive)
                         (setq qa-bare-popup-commands (1+ qa-bare-popup-commands))))",
                )
                .unwrap();
            let input = Rc::new(RefCell::new(Input::default()));
            install_minibuf_reader(&input, &mut interp);
            let value = interp
                .eval_str(
                    r#"(let ((map (make-sparse-keymap "Choose")))
                       (define-key map [first] 'qa-bare-popup-command)
                       (condition-case nil
                           (list (x-popup-menu t map) qa-bare-popup-commands)
                         (quit 'dismissed)))"#,
                )
                .unwrap();
            assert_eq!(interp.prin1_to_string(&value), "((first) 0)");
            assert_eq!(input.borrow().labels, ["first"]);
            assert!(interp.gui_popup_menu.is_none());

            let value = interp
                .eval_str(
                    r#"(let ((map (make-sparse-keymap "Choose"))
                             (inner (make-sparse-keymap)))
                       (define-key inner [leaf] 'qa-bare-popup-command)
                       (define-key inner [absent] nil)
                       (define-key map [branch] inner)
                       (define-key map [disabled] '(menu-item "Disabled" nil))
                       (condition-case nil
                           (list (x-popup-menu t map) qa-bare-popup-commands)
                         (quit 'dismissed)))"#,
                )
                .unwrap();
            assert_eq!(interp.prin1_to_string(&value), "((branch leaf) 0)");
            assert_eq!(input.borrow().labels, ["Disabled", "branch", "leaf"]);
            assert!(interp.gui_popup_menu.is_none());
        })
        .unwrap()
        .join()
        .unwrap();
}
