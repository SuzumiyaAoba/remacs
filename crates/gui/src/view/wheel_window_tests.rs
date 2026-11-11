use super::{EditorView, GuiEvent};
use crate::logic::ChanIo;
use gpui::{ScrollDelta, ScrollWheelEvent, point, px};
use remacs_runtime::{
    editor::{event_code_for, reflow_frame_windows},
    frontend::{MouseArea, render_grid_with_input_cached, run_editor_with},
    lisp::{Interp, Value},
};
use std::sync::mpsc;

fn check_nonselected_wheel(stacked: bool, shared: bool, mode_line: bool) {
    let mut i = Interp::new();
    let mode_line_format = if mode_line { "\"Mode\"" } else { "nil" };
    i.eval_str(&format!(
        r#"(progn
        (set-frame-parameter nil 'window-system 'ns)
        (set-frame-parameter nil 'menu-bar-lines 0)
        (set-frame-parameter nil 'tool-bar-lines 0)
        (set-frame-parameter nil 'tab-bar-lines 0)
        (set-frame-parameter nil 'vertical-scroll-bars nil)
        (set-frame-size nil 80 16) (set-window-fringes nil 0 0)
        (setq-default header-line-format nil tab-line-format nil mode-line-format {mode_line_format})
        (switch-to-buffer (get-buffer-create "*wheel-left*"))
        (erase-buffer) (dotimes (n 100) (insert (format "Left line %d\n" n))) (goto-char 1)
        (setq qa-wheel-selected (selected-window))
        (with-current-buffer (get-buffer-create "*wheel-right*")
          (erase-buffer) (dotimes (n 100) (insert (format "Right line %d\n" n))) (goto-char 1))
        (require 'mwheel) (mouse-wheel-mode 1)
        (setq mouse-wheel-follow-mouse t mouse-wheel-scroll-amount '(1)
              mouse-wheel-progressive-speed nil)
        (global-set-key [f12] (lambda () (interactive)
          (setq qa-wheel-result (list
            (> (window-start qa-wheel-target) 1)
            (= (window-start qa-wheel-selected) 1)
            (= (window-point qa-wheel-selected) 1)
            (eq (selected-window) qa-wheel-selected)
            (eq (current-buffer) (window-buffer qa-wheel-selected))))
        (kill-emacs))))"#
    ))
    .unwrap();
    let frame = i.selected_frame.as_ref().unwrap().clone();
    reflow_frame_windows(&frame, 80, 16);
    let split = if stacked {
        "(split-window-below 8)"
    } else {
        "(split-window-right 40)"
    };
    i.eval_str(&format!("(setq qa-wheel-target {split})"))
        .unwrap();
    if !shared {
        i.eval_str("(set-window-buffer qa-wheel-target \"*wheel-right*\")")
            .unwrap();
    }
    reflow_frame_windows(&frame, 80, 16);
    let selected_buffer = i.current_buffer;
    let Value::Window(target) = i.eval_str("qa-wheel-target").unwrap() else {
        panic!("target window")
    };
    let target_id = target.borrow().id;
    let target_buffer = target.borrow().buffer;
    assert_eq!(target_buffer == selected_buffer, shared);
    let mut grid = render_grid_with_input_cached(&i, 80, 16, None);
    assert_eq!(grid.input.as_ref().unwrap().buffer_id, selected_buffer);
    let area = *grid
        .window_text_areas
        .iter()
        .find(|area| area.window_id == target_id)
        .unwrap();
    let mode_line_area = grid
        .mode_lines
        .iter()
        .find(|line| line.window_id == target_id)
        .copied();
    if mode_line {
        assert!(mode_line_area.is_some());
    } else {
        grid.row_heights[area.top] *= 2;
    }
    let mut app = gpui::TestAppContext::single_with_native_text_system();
    let (tx, rx) = mpsc::channel();
    let handle = app.add_window(|_, cx| EditorView::new(tx, cx));
    handle
        .update(&mut app, |view, window, cx| {
            window.activate_window();
            view.set_internal_border_width(3);
            view.grid = grid;
            cx.notify();
        })
        .unwrap();
    app.run_until_parked();
    let position = handle
        .update(&mut app, |view, _, _| {
            if let Some(mode_line) = mode_line_area {
                point(
                    px(3.0) + view.cell_width * (mode_line.left + 2) as f32 + px(1.0),
                    px(3.0)
                        + super::grid_row_top(&view.grid, mode_line.row, view.line_height)
                        + px(5.0),
                )
            } else {
                point(
                    px(3.0) + view.cell_width * (area.left + 2) as f32 + px(1.0),
                    px(3.0)
                        + super::grid_row_top(&view.grid, area.top + 1, view.line_height)
                        + px(5.0),
                )
            }
        })
        .unwrap();
    rx.try_iter().for_each(drop);
    let mut visual = gpui::VisualTestContext::from_window(handle.into(), &app);
    visual.simulate_event(ScrollWheelEvent {
        position,
        delta: ScrollDelta::Lines(point(0.0, -3.0)),
        ..Default::default()
    });
    let events: Vec<_> = rx
        .try_iter()
        .filter(|event| matches!(event, GuiEvent::Scroll { .. }))
        .collect();
    assert_eq!(
        events.len(),
        1,
        "wheel event must reach the pointer target"
    );
    let (bridge_tx, bridge_rx) = mpsc::channel();
    bridge_tx.send(GuiEvent::Resize(80, 16)).unwrap();
    for event in events {
        let GuiEvent::Scroll { position, .. } = &event else {
            unreachable!()
        };
        assert_eq!(position.window_id, Some(target_id));
        assert_eq!(position.buffer_id, target_buffer);
        assert_eq!(
            position.area,
            if mode_line {
                Some(MouseArea::ModeLine)
            } else {
                None
            }
        );
        bridge_tx.send(event).unwrap();
    }
    bridge_tx
        .send(GuiEvent::Key(event_code_for("f12")))
        .unwrap();
    drop(bridge_tx);
    let (grid_tx, _grid_rx) = smol::channel::bounded(1);
    run_editor_with(ChanIo::new_for_test(bridge_rx, grid_tx), &mut i).unwrap();
    let result = i.intern("qa-wheel-result");
    assert_eq!(
        i.prin1_to_string(&i.symbol_value(result))
            .to_unicode_lossy(),
        "(t t t t t)"
    );
}

#[test]
fn wheel_follows_nonselected_side_by_side_buffer() {
    check_nonselected_wheel(false, false, false);
}

#[test]
fn wheel_follows_nonselected_stacked_buffer() {
    check_nonselected_wheel(true, false, false);
}

#[test]
fn wheel_follows_nonselected_shared_buffer() {
    check_nonselected_wheel(false, true, false);
}


#[test]
fn wheel_follows_nonselected_mode_line() {
    check_nonselected_wheel(false, false, true);
}
