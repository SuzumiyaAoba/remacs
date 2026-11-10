use super::{EditorView, GuiEvent};
use crate::logic::ChanIo;
use gpui::{KeyDownEvent, Keystroke, Modifiers};
use remacs_runtime::{
    editor::{CHAR_CTL, CHAR_META, CHAR_SHIFT, CHAR_SUPER, event_code_for},
    frontend::{Grid, InputEvent, KeyIo, dispatch_key_core},
    lisp::Interp,
};
use std::{sync::mpsc, time::Duration};

#[test]
fn undo_redo_reach_gui_and_execute_default_and_modified_bindings() {
    let keys = ["undo", "redo"].into_iter().flat_map(|name| {
        [false, true].map(move |modified| KeyDownEvent {
            keystroke: Keystroke {
                key: name.into(),
                key_char: None,
                modifiers: Modifiers {
                    control: modified,
                    alt: modified,
                    shift: modified,
                    platform: modified,
                    function: false,
                    hyper: false,
                },
            },
            is_held: false,
        })
    });
    let mut app = gpui::TestAppContext::single();
    let (tx, rx) = mpsc::channel();
    let handle = app.add_window(|_, cx| EditorView::new(tx, cx));
    handle
        .update(&mut app, |view, window, cx| {
            window.activate_window();
            view.focus.focus(window);
            view.grid = Grid::blank(40, 8);
            cx.notify();
        })
        .unwrap();
    app.run_until_parked();
    rx.try_iter().for_each(drop);
    let mut visual = gpui::VisualTestContext::from_window(handle.into(), &app);
    for key in keys {
        visual.simulate_event(key);
    }
    let events = rx
        .try_iter()
        .filter(|event| matches!(event, GuiEvent::Key(_) | GuiEvent::Text(_)))
        .collect::<Vec<_>>();
    assert_eq!(events.len(), 4, "each Undo/Redo key-down must arrive once");
    let (bridge_tx, bridge_rx) = mpsc::channel();
    for event in events {
        bridge_tx.send(event).unwrap();
    }
    let (grids, _receive_grids) = smol::channel::bounded(1);
    let mut io = ChanIo::new_for_test(bridge_rx, grids);
    let mut codes = Vec::new();
    for expected in [
        event_code_for("undo"),
        event_code_for("undo") | CHAR_CTL | CHAR_META | CHAR_SHIFT | CHAR_SUPER,
        event_code_for("redo"),
        event_code_for("redo") | CHAR_CTL | CHAR_META | CHAR_SHIFT | CHAR_SUPER,
    ] {
        let Some(InputEvent::Key(code)) = io.poll_input(Duration::ZERO).unwrap() else {
            panic!("Undo/Redo must use the command input path");
        };
        assert_eq!(code, expected);
        codes.push(code);
    }

    let mut interp = Interp::new();
    for (name, expected) in [("undo", "undo"), ("redo", "repeat-complex-command")] {
        let binding = interp
            .eval_str(&format!("(lookup-key global-map [{name}])"))
            .unwrap();
        assert_eq!(interp.prin1_to_string(&binding), expected);
    }
    let mut sequence = Vec::new();
    let mut argument = false;
    interp
        .eval_str(
            r#"(progn
      (buffer-enable-undo) (erase-buffer) (setq buffer-undo-list nil)
      (insert "before") (undo-boundary) (insert "after") (undo-boundary))"#,
        )
        .unwrap();
    dispatch_key_core(&mut interp, codes[0], &mut sequence, &mut argument, None).unwrap();
    let text = interp.eval_str("(buffer-string)").unwrap();
    assert_eq!(interp.princ_to_string(&text), "before");

    interp
        .eval_str(
            "(progn
      (setq qa-default-redo 0)
      (defun qa-repeat-native-command () (interactive) (setq qa-default-redo (1+ qa-default-redo)))
      (setq command-history '((qa-repeat-native-command))))",
        )
        .unwrap();
    let answered = std::rc::Rc::new(std::cell::Cell::new(false));
    let read_answered = answered.clone();
    interp.minibuf_reader = Some(std::rc::Rc::new(move |_, _, _| {
        if read_answered.replace(true) {
            Err(remacs_runtime::lisp::Flow::Quit)
        } else {
            Ok(remacs_runtime::lisp::MinibufInput::Text(
                "(qa-repeat-native-command)".into(),
            ))
        }
    }));
    dispatch_key_core(&mut interp, codes[2], &mut sequence, &mut argument, None).unwrap();
    assert!(answered.get());
    assert!(interp.eval_str("(= qa-default-redo 1)").unwrap().truthy());

    interp.eval_str("(progn
      (setq qa-native-history-keys nil)
      (global-set-key [undo] (lambda () (interactive) (push 'undo qa-native-history-keys)))
      (global-set-key [redo] (lambda () (interactive) (push 'redo qa-native-history-keys)))
      (global-set-key [C-M-S-s-undo] (lambda () (interactive) (push 'modified-undo qa-native-history-keys)))
      (global-set-key [C-M-S-s-redo] (lambda () (interactive) (push 'modified-redo qa-native-history-keys))))").unwrap();
    for code in codes {
        dispatch_key_core(&mut interp, code, &mut sequence, &mut argument, None).unwrap();
    }
    let received = interp.eval_str("qa-native-history-keys").unwrap();
    assert_eq!(
        interp.prin1_to_string(&received),
        "(modified-redo redo modified-undo undo)"
    );
    assert!(sequence.is_empty());
}
