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
fn execute_reaches_gui_and_runs_extended_command_and_modified_binding() {
    let keys = [false, true].map(|modified| KeyDownEvent {
        keystroke: Keystroke {
            key: "execute".into(),
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
    assert_eq!(events.len(), 2, "each Execute key-down must arrive once");
    let (bridge_tx, bridge_rx) = mpsc::channel();
    for event in events {
        bridge_tx.send(event).unwrap();
    }
    let (grids, _receive_grids) = smol::channel::bounded(1);
    let mut io = ChanIo::new_for_test(bridge_rx, grids);
    let mut codes = Vec::new();
    for expected in [
        event_code_for("execute"),
        event_code_for("execute") | CHAR_CTL | CHAR_META | CHAR_SHIFT | CHAR_SUPER,
    ] {
        let Some(InputEvent::Key(code)) = io.poll_input(Duration::ZERO).unwrap() else {
            panic!("Execute must use the command input path");
        };
        assert_eq!(code, expected);
        codes.push(code);
    }

    let mut interp = Interp::new();
    let binding = interp
        .eval_str("(lookup-key global-map [execute])")
        .unwrap();
    assert_eq!(interp.prin1_to_string(&binding), "execute-extended-command");
    interp.eval_str("(progn
      (setq qa-execute-default 0)
      (defun qa-execute-key-default () (interactive) (setq qa-execute-default (1+ qa-execute-default))))").unwrap();
    let answered = std::rc::Rc::new(std::cell::Cell::new(false));
    let read_answered = answered.clone();
    interp.minibuf_reader = Some(std::rc::Rc::new(move |_, _, _| {
        if read_answered.replace(true) {
            Err(remacs_runtime::lisp::Flow::Quit)
        } else {
            Ok(remacs_runtime::lisp::MinibufInput::Text(
                "qa-execute-key-default".into(),
            ))
        }
    }));
    let mut sequence = Vec::new();
    let mut argument = false;
    dispatch_key_core(&mut interp, codes[0], &mut sequence, &mut argument, None).unwrap();
    assert!(answered.get());
    assert!(
        interp
            .eval_str("(= qa-execute-default 1)")
            .unwrap()
            .truthy()
    );
    interp.eval_str("(progn
        (setq qa-native-execute-keys nil)
        (global-set-key [execute] (lambda () (interactive) (push 'plain qa-native-execute-keys)))
        (global-set-key [C-M-S-s-execute] (lambda () (interactive) (push 'modified qa-native-execute-keys))))").unwrap();
    for code in codes {
        dispatch_key_core(&mut interp, code, &mut sequence, &mut argument, None).unwrap();
    }
    let received = interp.eval_str("qa-native-execute-keys").unwrap();
    assert_eq!(interp.prin1_to_string(&received), "(modified plain)");
    assert!(sequence.is_empty());
}
