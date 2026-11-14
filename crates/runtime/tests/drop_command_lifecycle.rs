use remacs_runtime::{
    frontend::{InputEvent, KeyIo, run_editor_with_shared},
    lisp::{Flow, Interp},
};
use std::{cell::RefCell, collections::VecDeque, io, rc::Rc, time::Duration};

const SETUP: &str = r#"(progn
  (setq this-command 'previous-command
        last-command 'older-command
        buffer-undo-list '((1 . 2))
        qa-drop-pre-command nil
        qa-drop-post-command nil
        qa-drop-handler-command nil
        qa-drop-handler-last nil
        qa-drop-args nil)
  (add-hook 'pre-command-hook
            (lambda () (setq qa-drop-pre-command this-command)))
  (add-hook 'post-command-hook
            (lambda () (setq qa-drop-post-command this-command)))
  (defun dnd-handle-multiple-urls (window urls action)
    (setq qa-drop-handler-command this-command
          qa-drop-handler-last last-command
          qa-drop-args (list window urls action)))
  (provide 'dnd))"#;

struct Frontend {
    events: VecDeque<InputEvent>,
}

impl KeyIo for Frontend {
    fn poll_key(&mut self, _: Duration) -> io::Result<Option<i128>> {
        Ok(None)
    }

    fn poll_input(&mut self, _: Duration) -> io::Result<Option<InputEvent>> {
        self.events
            .pop_front()
            .map(Some)
            .ok_or_else(|| io::ErrorKind::BrokenPipe.into())
    }

    fn unread(&mut self, _: i128) {}

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

#[test]
fn gui_drop_uses_the_command_lifecycle() {
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(|| {
            let mut interp = Interp::new();
            interp.eval_str(SETUP).unwrap_or_else(|flow| match flow {
                Flow::Signal(signal) => panic!(
                    "{} {}",
                    interp.prin1_to_string(&signal.symbol()),
                    interp.prin1_to_string(&signal.data())
                ),
                other => panic!("{other:?}"),
            });

            let events = VecDeque::from([InputEvent::Drop {
                position: None,
                urls: vec!["file:///tmp/remacs-drop-probe".to_owned()],
                action: "copy".to_owned(),
            }]);
            let frontend = Rc::new(RefCell::new(Frontend { events }));
            let error = run_editor_with_shared(frontend, &mut interp).unwrap_err();
            assert_eq!(error.kind(), io::ErrorKind::BrokenPipe);

            let expected_command = if cfg!(target_os = "macos") {
                "ns-drag-n-drop"
            } else {
                "dnd-handle-multiple-urls"
            };
            let assertion = format!(
                "(and (eq this-command '{expected_command})                     (eq last-command 'previous-command)                     (eq qa-drop-pre-command '{expected_command})                     (eq qa-drop-post-command '{expected_command})                     (eq qa-drop-handler-command '{expected_command})                     (eq qa-drop-handler-last 'previous-command)                     (windowp (car qa-drop-args))                     (equal (cadr qa-drop-args) '(\"file:///tmp/remacs-drop-probe\"))                     (eq (caddr qa-drop-args) 'copy)                     (equal buffer-undo-list '(nil (1 . 2))))"
            );
            let state = interp.eval_str(&assertion).unwrap_or_else(|flow| match flow {
                Flow::Signal(signal) => panic!(
                    "{} {}",
                    interp.prin1_to_string(&signal.symbol()),
                    interp.prin1_to_string(&signal.data())
                ),
                other => panic!("{other:?}"),
            });
            assert!(
                state.truthy(),
                "drop command state: {}",
                interp.eval_str(
                    "(list this-command last-command qa-drop-pre-command qa-drop-post-command qa-drop-handler-command qa-drop-handler-last qa-drop-args buffer-undo-list)"
                )
                .map(|value| interp.prin1_to_string(&value))
                .unwrap_or_default()
            );
        })
        .unwrap()
        .join()
        .unwrap();
}

