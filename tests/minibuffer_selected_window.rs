//! Focused tests for minibuffer window context.
mod common;

use remacs::lisp::{Flow, MinibufInput};
use std::{cell::RefCell, collections::VecDeque, rc::Rc};

#[test]
fn minibuffer_selected_window_reports_the_window_before_the_read() {
    common::on_big_stack(|| {
        let (mut interp, _) = common::interp();
        let answers = Rc::new(RefCell::new(VecDeque::from([MinibufInput::Text(
            "answer".into(),
        )])));
        interp.minibuf_reader = Some(Rc::new(move |_, _, _| {
            answers.borrow_mut().pop_front().ok_or(Flow::Quit)
        }));

        let value = interp
            .eval_str(
                r#"(let ((minibuffer-selected-window-probe nil)
                       (minibuffer-setup-hook nil)
                       (before (selected-window)))
                   (add-hook 'minibuffer-setup-hook
                             (lambda ()
                               (setq minibuffer-selected-window-probe
                                     (minibuffer-selected-window))))
                   (let ((input (read-from-minibuffer "P: ")))
                     (list (equal input "answer")
                           (eq minibuffer-selected-window-probe before)
                           (window-live-p minibuffer-selected-window-probe)
                           (minibuffer-selected-window))))"#,
            )
            .unwrap();

        assert_eq!(interp.prin1_to_string(&value).to_unicode_lossy(), "(t t t nil)");
    });
}
