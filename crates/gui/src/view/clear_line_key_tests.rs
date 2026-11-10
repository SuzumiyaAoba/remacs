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
fn clear_line_reaches_named_clear_without_killing_text() {
    let keys = [false, true].map(|modified| KeyDownEvent {
        keystroke: Keystroke {
            key: "clear".into(),
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
        let release = gpui::KeyUpEvent {
            keystroke: key.keystroke.clone(),
        };
        visual.simulate_event(key);
        visual.simulate_event(release);
    }
    let events = rx
        .try_iter()
        .filter(|event| matches!(event, GuiEvent::Key(_) | GuiEvent::Text(_)))
        .collect::<Vec<_>>();
    assert_eq!(events.len(), 2, "each Clear key-down must arrive once");
    let (bridge_tx, bridge_rx) = mpsc::channel();
    for event in events {
        bridge_tx.send(event).unwrap();
    }
    let (grids, _receive_grids) = smol::channel::bounded(1);
    let mut io = ChanIo::new_for_test(bridge_rx, grids);
    let mut codes = Vec::new();
    for expected in [
        event_code_for("clear"),
        event_code_for("clear") | CHAR_CTL | CHAR_META | CHAR_SHIFT | CHAR_SUPER,
    ] {
        let Some(InputEvent::Key(code)) = io.poll_input(Duration::ZERO).unwrap() else {
            panic!("Clear must use the command input path");
        };
        assert_eq!(code, expected);
        codes.push(code);
    }

    assert_ne!(
        event_code_for("clear"),
        11,
        "named Clear must be distinct from ASCII C-k"
    );
    let mut interp = Interp::new();
    interp.eval_str("(progn
        (erase-buffer) (insert \"text must survive Clear\") (goto-char (point-min)) (setq qa-native-clear-keys nil)
        (global-set-key [clear] (lambda () (interactive) (push 'plain qa-native-clear-keys)))
        (global-set-key [C-M-S-s-clear] (lambda () (interactive) (push 'modified qa-native-clear-keys))))").unwrap();
    let mut sequence = Vec::new();
    let mut argument = false;
    for code in codes {
        dispatch_key_core(&mut interp, code, &mut sequence, &mut argument, None).unwrap();
    }
    let received = interp.eval_str("qa-native-clear-keys").unwrap();
    assert_eq!(interp.prin1_to_string(&received), "(modified plain)");
    assert!(
        interp
            .eval_str("(equal (buffer-string) \"text must survive Clear\")")
            .unwrap()
            .truthy()
    );
    assert!(sequence.is_empty());
}
