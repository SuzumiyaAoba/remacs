use super::{EditorView, GuiEvent};
use crate::logic::ChanIo;
use gpui::{Modifiers, NavigationDirection, point, px};
use remacs_runtime::{
    editor::{event_code_for, reflow_frame_windows},
    frontend::{render_grid_with_input_cached, run_editor_with},
    lisp::Interp,
};
use std::sync::mpsc;

#[test]
fn popup_dismissal_consumes_the_gesture_before_and_after_the_menu_redraw() {
    let mut i = Interp::new();
    i.eval_str(
        r#"(progn
        (set-frame-parameter nil 'window-system 'ns)
        (set-frame-parameter nil 'menu-bar-lines 0)
        (set-frame-parameter nil 'tool-bar-lines 0)
        (set-frame-parameter nil 'tab-bar-lines 0)
        (set-frame-parameter nil 'vertical-scroll-bars nil)
        (set-frame-size nil 40 12) (set-window-fringes nil 0 0)
        (setq-local mode-line-format nil header-line-format nil tab-line-format nil)
        (erase-buffer) (dotimes (_ 12) (insert "background text for popup dismissal\n"))
        (goto-char 1)
        (setq context-menu-mode nil qa-popup-clicks nil qa-popup-map (make-sparse-keymap "Popup"))
        (define-key qa-popup-map [item] '(menu-item "Item" ignore))
        (defun qa-popup-background-click (event) (interactive "e")
          (setq qa-popup-clicks (append qa-popup-clicks (list (car event)))))
        (dolist (key '([down-mouse-1] [mouse-1] [down-mouse-2] [mouse-2]
                      [down-mouse-3] [mouse-3] [down-mouse-4] [mouse-4]
                      [down-mouse-5] [mouse-5]))
          (global-set-key key #'qa-popup-background-click))
        (global-set-key [f12] (lambda () (interactive) (kill-emacs))))"#,
    )
    .unwrap();
    reflow_frame_windows(i.selected_frame.as_ref().unwrap(), 40, 12);
    let mut app = gpui::TestAppContext::single();
    let (tx, rx) = mpsc::channel();
    let handle = app.add_window(|_, cx| EditorView::new(tx, cx));
    let (bridge_tx, bridge_rx) = mpsc::channel();
    for button in [
        gpui::MouseButton::Left,
        gpui::MouseButton::Middle,
        gpui::MouseButton::Right,
        gpui::MouseButton::Navigate(NavigationDirection::Back),
        gpui::MouseButton::Navigate(NavigationDirection::Forward),
    ] {
        // The headless popup reader publishes the real menu snapshot. Simulate
        // its synchronous dismissal repaint between the press and release.
        i.eval_str("(x-popup-menu t qa-popup-map)").unwrap();
        let grid = render_grid_with_input_cached(&i, 40, 12, None);
        let menu_id = grid.popup_menu_items[0].menu_id;
        assert!(grid.popup_menu_items.iter().all(|item| item.row != 8));
        assert!(
            grid.input
                .as_ref()
                .unwrap()
                .hit_test_position(8, 2, 2)
                .is_some()
        );
        handle
            .update(&mut app, |view, window, cx| {
                window.activate_window();
                view.grid = grid;
                cx.notify();
            })
            .unwrap();
        app.run_until_parked();
        rx.try_iter().for_each(drop);
        let (width, height) = handle
            .update(&mut app, |view, _, _| {
                let geometry = view.geometry.as_ref().unwrap();
                (geometry.char_width as f32, geometry.char_height as f32)
            })
            .unwrap();
        let outside = point(px(width * 2.0), px(height * 8.0 + 5.0));
        let mut visual = gpui::VisualTestContext::from_window(handle.into(), &app);
        visual.simulate_mouse_down(outside, button, Modifiers::none());
        i.gui_popup_menu = None;
        let background = render_grid_with_input_cached(&i, 40, 12, None);
        handle
            .update(&mut app, |view, _, cx| {
                view.grid = background;
                cx.notify();
            })
            .unwrap();
        app.run_until_parked();
        visual.simulate_mouse_move(
            outside + point(px(2.0), px(0.0)),
            Some(button),
            Modifiers::none(),
        );
        visual.simulate_mouse_up(outside, button, Modifiers::none());
        let events: Vec<_> = rx.try_iter().collect();
        assert_eq!(
            events
                .iter()
                .filter_map(|event| match event {
                    GuiEvent::PopupMenuSelection { menu_id, index } => Some((*menu_id, *index)),
                    _ => None,
                })
                .collect::<Vec<_>>(),
            [(menu_id, None)],
            "{button:?}"
        );
        assert!(
            !events.iter().any(|event| matches!(
                event,
                GuiEvent::MouseDown { .. }
                    | GuiEvent::MouseUp { .. }
                    | GuiEvent::MouseDragMove { .. }
                    | GuiEvent::MenuCommand(_)
                    | GuiEvent::CommandArgs { .. }
            )),
            "dismiss gesture leaked to the background: {button:?}"
        );
        for event in events {
            bridge_tx.send(event).unwrap();
        }
        // The next ordinary click must not be swallowed by stale capture state.
        visual.simulate_mouse_down(outside, button, Modifiers::none());
        visual.simulate_mouse_up(outside, button, Modifiers::none());
        for event in rx.try_iter() {
            bridge_tx.send(event).unwrap();
        }
    }
    bridge_tx
        .send(GuiEvent::Key(event_code_for("f12")))
        .unwrap();
    drop(bridge_tx);
    let (grid_tx, _grid_rx) = smol::channel::bounded(1);
    run_editor_with(ChanIo::new_for_test(bridge_rx, grid_tx), &mut i).unwrap();
    let clicks = i.intern("qa-popup-clicks");
    assert_eq!(
        i.prin1_to_string(&i.symbol_value(clicks))
            .to_unicode_lossy(),
        "(down-mouse-1 mouse-1 down-mouse-2 mouse-2 down-mouse-3 mouse-3 down-mouse-4 mouse-4 down-mouse-5 mouse-5)"
    );
}
