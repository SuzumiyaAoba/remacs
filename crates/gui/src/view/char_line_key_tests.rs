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
fn character_and_line_keys_reach_gui_defaults_and_custom_bindings() {
    let names = ["insertline", "deleteline", "insertchar", "deletechar"];
    let keys = names.into_iter().flat_map(|name| {
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
    assert_eq!(
        events.len(),
        8,
        "each character/line key-down must arrive once"
    );
    let (bridge_tx, bridge_rx) = mpsc::channel();
    for event in events {
        bridge_tx.send(event).unwrap();
    }
    let (grids, _receive_grids) = smol::channel::bounded(1);
    let mut io = ChanIo::new_for_test(bridge_rx, grids);
    let mut codes = Vec::new();
    for name in names {
        for expected in [
            event_code_for(name),
            event_code_for(name) | CHAR_CTL | CHAR_META | CHAR_SHIFT | CHAR_SUPER,
        ] {
            let Some(InputEvent::Key(code)) = io.poll_input(Duration::ZERO).unwrap() else {
                panic!("character/line key must use the command path");
            };
            assert_eq!(code, expected);
            codes.push(code);
        }
    }

    let mut interp = Interp::new();
    let cases = [
        ("insertline", "open-line", "\nalpha\nbeta\n"),
        ("deleteline", "kill-line", "\nbeta\n"),
        ("insertchar", "overwrite-mode", "alpha\nbeta\n"),
        ("deletechar", "delete-forward-char", "lpha\nbeta\n"),
    ];
    let mut sequence = Vec::new();
    let mut argument = false;
    for (index, (name, command, expected)) in cases.into_iter().enumerate() {
        let binding = interp
            .eval_str(&format!("(lookup-key global-map [{name}])"))
            .unwrap();
        assert_eq!(interp.prin1_to_string(&binding), command);
        interp.eval_str(r#"(progn (erase-buffer) (insert "alpha\nbeta\n") (goto-char (point-min)) (setq overwrite-mode nil))"#).unwrap();
        dispatch_key_core(
            &mut interp,
            codes[index * 2],
            &mut sequence,
            &mut argument,
            None,
        )
        .unwrap();
        let text = interp.eval_str("(buffer-string)").unwrap();
        assert_eq!(interp.princ_to_string(&text), expected, "{name}");
        if name == "insertchar" {
            assert!(interp.eval_str("overwrite-mode").unwrap().truthy());
        }
        assert!(sequence.is_empty());
    }
    interp
        .eval_str("(progn (setq overwrite-mode nil qa-native-char-line-keys nil))")
        .unwrap();
    for name in names {
        for (event, marker) in [
            (name.to_owned(), name.to_owned()),
            (format!("C-M-S-s-{name}"), format!("modified-{name}")),
        ] {
            interp.eval_str(&format!("(global-set-key [{event}] (lambda () (interactive) (push '{marker} qa-native-char-line-keys)))")).unwrap();
        }
    }
    for code in codes {
        dispatch_key_core(&mut interp, code, &mut sequence, &mut argument, None).unwrap();
    }
    let received = interp.eval_str("qa-native-char-line-keys").unwrap();
    assert_eq!(
        interp.prin1_to_string(&received),
        "(modified-deletechar deletechar modified-insertchar insertchar modified-deleteline deleteline modified-insertline insertline)"
    );
    assert!(sequence.is_empty());
}
