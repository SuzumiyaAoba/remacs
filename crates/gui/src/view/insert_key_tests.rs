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
fn insert_reaches_the_gui_channel_and_default_overwrite_binding() {
    let keys = [false, true].map(|modified| KeyDownEvent {
        keystroke: Keystroke {
            key: "insert".into(),
            key_char: None,
            modifiers: Modifiers {
                control: modified,
                alt: modified,
                shift: modified,
                platform: modified,
                function: false,
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
    assert_eq!(events.len(), 2, "each Insert key-down must arrive once");
    let (bridge_tx, bridge_rx) = mpsc::channel();
    for event in events {
        bridge_tx.send(event).unwrap();
    }
    let (grids, _receive_grids) = smol::channel::bounded(1);
    let mut io = ChanIo::new_for_test(bridge_rx, grids);
    let mut codes = Vec::new();
    for expected in [
        event_code_for("insert"),
        event_code_for("insert") | CHAR_CTL | CHAR_META | CHAR_SHIFT | CHAR_SUPER,
    ] {
        let Some(InputEvent::Key(code)) = io.poll_input(Duration::ZERO).unwrap() else {
            panic!("Insert must use the command input path");
        };
        assert_eq!(code, expected);
        codes.push(code);
    }

    let mut interp = Interp::new();
    let binding = interp.eval_str("(lookup-key global-map [insert])").unwrap();
    assert_eq!(interp.prin1_to_string(&binding), "overwrite-mode");
    let mut sequence = Vec::new();
    let mut argument = false;
    for enabled in [true, false] {
        dispatch_key_core(&mut interp, codes[0], &mut sequence, &mut argument, None).unwrap();
        assert_eq!(interp.eval_str("overwrite-mode").unwrap().truthy(), enabled);
    }
    interp.eval_str("(progn
        (setq qa-native-insert-keys nil)
        (global-set-key [insert] (lambda () (interactive) (push 'plain qa-native-insert-keys)))
        (global-set-key [C-M-S-s-insert] (lambda () (interactive) (push 'modified qa-native-insert-keys))))").unwrap();
    for code in codes {
        dispatch_key_core(&mut interp, code, &mut sequence, &mut argument, None).unwrap();
    }
    let received = interp.eval_str("qa-native-insert-keys").unwrap();
    assert_eq!(interp.prin1_to_string(&received), "(modified plain)");
    assert!(sequence.is_empty());
}
