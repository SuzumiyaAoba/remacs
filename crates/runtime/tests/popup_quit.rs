use remacs_runtime::{
    frontend::{KeyIo, QuitRequest, install_minibuf_reader, render_grid},
    lisp::Interp,
};
use std::{cell::RefCell, io, rc::Rc, sync::Arc, time::Duration};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Cancel {
    Escape,
    ControlG,
    AsyncControlG,
}

struct Input {
    cancel: Cancel,
    quit: Arc<QuitRequest>,
    polls: usize,
    visible: Vec<bool>,
}

impl KeyIo for Input {
    fn poll_key(&mut self, _: Duration) -> io::Result<Option<i128>> {
        self.polls += 1;
        match self.cancel {
            Cancel::Escape => Ok(Some(27)),
            Cancel::ControlG => Ok(Some(7)),
            Cancel::AsyncControlG => {
                // The GUI can deliver quit before its queued key event.
                self.quit.request();
                Ok(None)
            }
        }
    }
    fn unread(&mut self, _: i128) {}
    fn render(&mut self, interp: &Interp) -> io::Result<()> {
        self.visible
            .push(!render_grid(interp, 80, 24).popup_menu_items.is_empty());
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
fn escape_and_both_quit_delivery_paths_remove_the_popup() {
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(|| {
            let mut interp = Interp::new();
            interp
                .eval_str(
                    r#"(progn
                       (set-frame-parameter nil 'window-system 'ns)
                       (setq qa-popup-cancel-commands 0
                             qa-popup-cancel-map (make-sparse-keymap "Cancel test"))
                       (defun qa-popup-cancel-command () (interactive)
                         (setq qa-popup-cancel-commands (1+ qa-popup-cancel-commands)))
                       (define-key qa-popup-cancel-map [answer]
                         '(menu-item "Answer" qa-popup-cancel-command)))"#,
                )
                .unwrap();
            let quit = Arc::new(QuitRequest::default());
            interp.set_quit_request(Some(quit.clone()));
            let input = Rc::new(RefCell::new(Input {
                cancel: Cancel::Escape,
                quit: quit.clone(),
                polls: 0,
                visible: Vec::new(),
            }));
            install_minibuf_reader(&input, &mut interp);
            for position in ["t", "(list 'mouse-3 (list (selected-window) 1 '(0 . 0) 0))"] {
                for cancel in [Cancel::Escape, Cancel::ControlG, Cancel::AsyncControlG] {
                    input.borrow_mut().cancel = cancel;
                    let old_polls = input.borrow().polls;
                    let source = format!(
                        "(condition-case nil
                            (x-popup-menu {position} qa-popup-cancel-map)
                          (quit 'cancelled))"
                    );
                    let value = interp
                        .eval_str(&source)
                        .unwrap_or_else(|flow| panic!("{cancel:?}, {position}: {flow:?}"));
                    assert_eq!(
                        interp.prin1_to_string(&value),
                        if position != "t" && cancel == Cancel::Escape {
                            "nil"
                        } else {
                            "cancelled"
                        },
                        "{cancel:?}, {position}"
                    );
                    assert!(interp.gui_popup_menu.is_none());
                    assert_eq!(input.borrow().polls, old_polls + 1);
                    assert_eq!(
                        std::mem::take(&mut input.borrow_mut().visible),
                        [true, false],
                        "the visible overlay must be repainted away: {cancel:?}, {position}"
                    );
                    assert_eq!(
                        quit.discard_queued_key(),
                        cancel == Cancel::AsyncControlG,
                        "only a consumed asynchronous request discards a queued key"
                    );
                    assert!(!quit.discard_queued_key());
                    let state = interp
                        .eval_str("(list qa-popup-cancel-commands quit-flag)")
                        .unwrap();
                    assert_eq!(interp.prin1_to_string(&state), "(0 nil)");
                }
            }
        })
        .unwrap()
        .join()
        .unwrap();
}
