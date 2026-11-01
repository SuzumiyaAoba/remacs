use super::{EditorView, GuiEvent};
use crate::logic::ChanIo;
use gpui::{Modifiers, point, px};
use remacs_runtime::{
    editor::{event_code_for, reflow_frame_windows},
    frontend::{render_grid_with_input_cached, run_editor_with},
    lisp::{Interp, Value},
};
use std::sync::mpsc;

#[test]
fn autoselect_only_uses_visible_text_areas_even_without_a_buffer_hit() {
    let mut i = Interp::new();
    i.eval_str(r#"(progn
      (set-frame-parameter nil 'window-system 'ns)
      (set-frame-parameter nil 'menu-bar-lines 0)
      (set-frame-parameter nil 'tool-bar-lines 0)
      (set-frame-parameter nil 'tab-bar-lines 0)
      (set-frame-parameter nil 'vertical-scroll-bars nil)
      (set-frame-parameter nil 'horizontal-scroll-bars nil)
      (set-frame-size nil 40 16) (set-window-fringes nil 0 0)
      (erase-buffer) (setq-default fontification-functions nil)
      (setq-local fontification-functions nil mode-line-format "Mode" header-line-format "Header" tab-line-format "Tab")
      (setq remacs-ui-upper (selected-window) remacs-ui-lower (split-window-below 8))
      (setq mouse-autoselect-window t remacs-ui-selection nil)
      (defun remacs-ui-reset () (interactive) (select-window remacs-ui-upper))
      (defun remacs-ui-record () (interactive)
        (setq remacs-ui-selection (append remacs-ui-selection (list (eq (selected-window) remacs-ui-lower)))))
      (global-set-key [f12] (lambda () (interactive) (kill-emacs))))"#).unwrap();
    let frame = i.selected_frame.as_ref().unwrap().clone();
    reflow_frame_windows(&frame, 40, 16);
    let frame_id = frame.borrow().id;
    let Value::Window(lower) = i.eval_str("remacs-ui-lower").unwrap() else {
        panic!("lower window");
    };
    let lower_id = lower.borrow().id;
    let mut grid = render_grid_with_input_cached(&i, 40, 16, None);
    grid.row_heights[1] *= 2;
    let area = *grid
        .window_text_areas
        .iter()
        .find(|area| area.window_id == lower_id)
        .unwrap();
    let mode_row = grid
        .mode_lines
        .iter()
        .find(|line| line.window_id == lower_id)
        .unwrap()
        .row;
    let mut app = gpui::TestAppContext::single();
    let (tx, rx) = mpsc::channel();
    let handle = app.add_window(|_, cx| EditorView::new_for_frame(tx, Some(frame_id), cx));
    handle
        .update(&mut app, |view, window, cx| {
            window.activate_window();
            view.grid = grid;
            cx.notify();
        })
        .unwrap();
    app.run_until_parked();
    let (cell, line) = handle
        .update(&mut app, |view, _, _| {
            let geometry = view.geometry.as_ref().unwrap();
            (geometry.char_width as f32, geometry.char_height as f32)
        })
        .unwrap();
    let (bridge_tx, bridge_rx) = mpsc::channel();
    for event in rx.try_iter() {
        bridge_tx.send(event).unwrap();
    }
    let mut visual = gpui::VisualTestContext::from_window(handle.into(), &app);
    let targets = [
        (mode_row, None),
        (area.top - 1, None),
        (area.top - 2, None),
        (area.top, Some(lower_id)),
        (area.top + area.rows - 1, Some(lower_id)),
    ];
    let points = handle
        .update(&mut app, |view, _, _| {
            targets
                .iter()
                .map(|(row, _)| {
                    point(
                        px((area.left + 1) as f32 * cell),
                        super::grid_row_top(&view.grid, *row, px(line)) + px(5.0),
                    )
                })
                .collect::<Vec<_>>()
        })
        .unwrap();
    let mut saw_unmapped_text = false;
    for ((_, expected), position) in targets.into_iter().zip(points) {
        visual.simulate_mouse_move(position, None, Modifiers::none());
        let mut count = 0;
        for event in rx.try_iter() {
            if let GuiEvent::MouseMove {
                text_window_id,
                position,
                ..
            } = &event
            {
                assert_eq!(*text_window_id, expected);
                saw_unmapped_text |= expected.is_some() && position.is_none();
                bridge_tx
                    .send(GuiEvent::MenuCommand("remacs-ui-reset".into()))
                    .unwrap();
                bridge_tx.send(event).unwrap();
                bridge_tx
                    .send(GuiEvent::MenuCommand("remacs-ui-record".into()))
                    .unwrap();
                count += 1;
            } else {
                bridge_tx.send(event).unwrap();
            }
        }
        assert_eq!(count, 1);
    }
    assert!(
        saw_unmapped_text,
        "blank body must not need a mapped character"
    );
    bridge_tx
        .send(GuiEvent::Key(event_code_for("f12")))
        .unwrap();
    drop(bridge_tx);
    let (grid_tx, _grid_rx) = smol::channel::bounded(1);
    run_editor_with(ChanIo::new_for_test(bridge_rx, grid_tx), &mut i).unwrap();
    let symbol = i.intern("remacs-ui-selection");
    assert_eq!(
        i.prin1_to_string(&i.symbol_value(symbol))
            .to_unicode_lossy(),
        "(nil nil nil t t)"
    );
}
