use remacs_runtime::{
    frontend::{Grid, InputEvent, KeyIo, install_minibuf_reader, render_grid},
    lisp::Interp,
};
use std::{cell::RefCell, io, rc::Rc, sync::mpsc, time::Duration};

#[derive(Default)]
struct Input {
    polls: usize,
    renders: usize,
}

impl KeyIo for Input {
    fn poll_key(&mut self, _: Duration) -> io::Result<Option<i128>> {
        self.polls += 1;
        Ok(Some(27))
    }
    fn unread(&mut self, _: i128) {}
    fn render(&mut self, _: &Interp) -> io::Result<()> {
        self.renders += 1;
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
fn popup_waits_for_frontend_input_and_unwinds_dismissal() {
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(|| {
            let mut interp = Interp::new();
            interp
                .eval_str("(set-frame-parameter nil 'window-system 'ns)")
                .unwrap();
            let input = Rc::new(RefCell::new(Input::default()));
            install_minibuf_reader(&input, &mut interp);
            let value = interp
                .eval_str(
                    r#"(let ((menu (make-sparse-keymap "Popup")))
                       (define-key menu [answer] '(menu-item "Answer" ignore))
                       (condition-case nil (x-popup-menu t menu) (quit 'dismissed)))"#,
                )
                .unwrap();
            assert_eq!(interp.prin1_to_string(&value), "dismissed");
            assert_eq!(input.borrow().polls, 1);
            assert!(input.borrow().renders >= 2);
            assert!(interp.gui_popup_menu.is_none());
        })
        .unwrap()
        .join()
        .unwrap();
}

struct ChannelInput {
    events: mpsc::Receiver<io::Result<InputEvent>>,
    grids: mpsc::Sender<Grid>,
}

impl KeyIo for ChannelInput {
    fn poll_key(&mut self, _: Duration) -> io::Result<Option<i128>> {
        panic!("popup input must retain selection events");
    }
    fn poll_key_or_mouse(&mut self, duration: Duration) -> io::Result<Option<InputEvent>> {
        match self.events.recv_timeout(duration) {
            Ok(event) => event.map(Some),
            Err(mpsc::RecvTimeoutError::Timeout) => Ok(None),
            Err(mpsc::RecvTimeoutError::Disconnected) => Err(io::ErrorKind::BrokenPipe.into()),
        }
    }
    fn unread(&mut self, _: i128) {}
    fn render(&mut self, interp: &Interp) -> io::Result<()> {
        self.grids
            .send(render_grid(interp, 80, 24))
            .map_err(|_| io::ErrorKind::BrokenPipe.into())
    }
    fn draw_echo(&mut self, _: &str) -> io::Result<()> {
        Ok(())
    }
    fn size(&self) -> (usize, usize) {
        (80, 24)
    }
}

#[test]
fn popup_returns_choices_to_the_blocked_caller_without_executing_them() {
    let (input_tx, input_rx) = mpsc::channel();
    let (grid_tx, grid_rx) = mpsc::channel();
    let (result_tx, result_rx) = mpsc::channel();
    let worker = std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(move || {
            let mut interp = Interp::new();
            interp.eval_str(r#"(progn
              (set-frame-parameter nil 'window-system 'ns)
              (setq qa-popup-commands 0 qa-popup-map (make-sparse-keymap "Popup"))
              (defun qa-popup-command () (interactive) (setq qa-popup-commands (1+ qa-popup-commands)))
              (define-key qa-popup-map [answer] '(menu-item "Answer" qa-popup-command))
              (define-key qa-popup-map [disabled] '(menu-item "Disabled" nil)))"#).unwrap();
            let input = Rc::new(RefCell::new(ChannelInput { events: input_rx, grids: grid_tx }));
            install_minibuf_reader(&input, &mut interp);
            let sources = [
                "(list (x-popup-menu t qa-popup-map) qa-popup-commands)",
                r#"(let ((outer (make-sparse-keymap "Outer"))
                         (inner (make-sparse-keymap "Inner"))
                         (first (make-sparse-keymap "First")))
                     (define-key inner [leaf] '(menu-item "Leaf" qa-popup-command))
                     (define-key outer [branch] (list 'menu-item "Branch" inner))
                     (define-key first [other] '(menu-item "Other" ignore))
                     (list (x-popup-menu t (list first outer)) qa-popup-commands))"#,
                r#"(progn
                     (setq qa-popup-original (list 'original))
                     (eq qa-popup-original
                         (x-popup-menu t (list "Legacy" (list "Pane" "Description"
                                            (cons "Original" qa-popup-original))))))"#,
                r#"(list (x-popup-menu t '("Legacy" ("Pane" ("Nil" . nil)))) 'selected)"#,
                "(condition-case nil (x-popup-menu t qa-popup-map) (quit 'dismissed))",
                "(condition-case nil (x-popup-menu (list 'mouse-3 (list (selected-window) 1 '(0 . 0) 0)) qa-popup-map) (quit 'unexpected-quit))",
                "(progn (popup-menu qa-popup-map t) qa-popup-commands)",
                "(condition-case err (x-popup-menu t qa-popup-map) (error (car err)))",
                "(condition-case nil (x-popup-menu t qa-popup-map) (quit 'quit))",
            ];
            for source in sources {
                let value = interp.eval_str(source).unwrap_or_else(|flow| panic!("{source}: {flow:?}"));
                assert!(interp.gui_popup_menu.is_none(), "popup must unwind its display state");
                result_tx.send(interp.prin1_to_string(&value).to_unicode_lossy()).unwrap();
            }
        }).unwrap();

    let outcomes = (|| -> Result<Vec<(bool, String)>, String> {
        let actions = [
            Some("Answer"),
            Some("Leaf"),
            Some("Original"),
            Some("Nil"),
            None,
            None,
            Some("Answer"),
            None,
            None,
        ];
        let mut outcomes = Vec::new();
        for (case, label) in actions.into_iter().enumerate() {
            let grid = loop {
                let grid = grid_rx
                    .recv_timeout(Duration::from_secs(30))
                    .map_err(|error| format!("case {case}: {error}"))?;
                if !grid.popup_menu_items.is_empty() {
                    break grid;
                }
            };
            let pending = matches!(
                result_rx.recv_timeout(Duration::from_millis(30)),
                Err(mpsc::RecvTimeoutError::Timeout)
            );
            let menu_id = grid.popup_menu_items[0].menu_id;
            if let Some(label) = label {
                let item = grid
                    .popup_menu_items
                    .iter()
                    .find(|item| {
                        grid.rows[item.row]
                            .chars()
                            .skip(item.left)
                            .take(item.width)
                            .collect::<String>()
                            .trim()
                            == label
                    })
                    .ok_or_else(|| format!("missing {label:?} in case {case}"))?;
                if case == 0 {
                    // Ignore delayed responses for a different menu and disabled rows.
                    input_tx
                        .send(Ok(InputEvent::PopupMenuSelection {
                            menu_id: menu_id + 1000,
                            index: item.selection,
                        }))
                        .unwrap();
                    if let Some((index, _)) = grid
                        .popup_menu_items
                        .iter()
                        .enumerate()
                        .find(|(_, item)| item.selection.is_none())
                    {
                        input_tx
                            .send(Ok(InputEvent::PopupMenuSelection {
                                menu_id,
                                index: Some(index),
                            }))
                            .unwrap();
                    }
                }
                input_tx
                    .send(Ok(InputEvent::PopupMenuSelection {
                        menu_id,
                        index: item.selection,
                    }))
                    .unwrap();
            } else if case == 7 {
                input_tx
                    .send(Err(io::Error::other("test input disconnected")))
                    .unwrap();
            } else {
                input_tx
                    .send(Ok(InputEvent::Key(if case == 8 { 7 } else { 27 })))
                    .unwrap();
            }
            let result = result_rx
                .recv_timeout(Duration::from_secs(30))
                .map_err(|error| format!("case {case}: {error}"))?;
            outcomes.push((pending, result));
        }
        Ok(outcomes)
    })();
    // Disconnect before joining even when a driver assertion fails.
    drop(input_tx);
    drop(grid_rx);
    let joined = worker.join();
    let outcomes = outcomes.unwrap();
    joined.unwrap();
    assert!(
        outcomes.iter().all(|(pending, _)| *pending),
        "caller returned before selection"
    );
    assert_eq!(
        outcomes
            .into_iter()
            .map(|(_, value)| value)
            .collect::<Vec<_>>(),
        [
            "((answer) 0)",
            "((branch leaf) 0)",
            "t",
            "(nil selected)",
            "dismissed",
            "nil",
            "1",
            "error",
            "quit",
        ]
    );
}
