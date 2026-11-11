use super::{EditorView, GuiEvent};
use crate::logic::ChanIo;
use gpui::{Modifiers, ScrollDelta, ScrollWheelEvent, point, px};
use remacs_runtime::{
    editor::{CHAR_CTL, event_code_for, reflow_frame_windows},
    frontend::{MouseArea, render_grid_with_input_cached, run_editor_with},
    lisp::{Interp, Value},
};
use std::sync::mpsc;

#[test]
fn header_wheel_hit_testing_uses_row_heights_and_reaches_its_lisp_prefix() {
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(header_wheel_hit_testing_uses_row_heights_and_reaches_its_lisp_prefix_inner)
        .expect("spawn header wheel test thread")
        .join()
        .expect("header wheel test thread");
}

fn header_wheel_hit_testing_uses_row_heights_and_reaches_its_lisp_prefix_inner() {
    let mut i = Interp::new();
    i.eval_str(
        r#"(progn
        (set-frame-parameter nil 'window-system 'ns)
        (set-frame-parameter nil 'menu-bar-lines 0)
        (set-frame-parameter nil 'tool-bar-lines 0)
        (set-frame-parameter nil 'tab-bar-lines 0)
        (set-frame-parameter nil 'vertical-scroll-bars nil)
        (set-frame-size nil 80 16) (set-window-fringes nil 0 0)
        (setq-default header-line-format "Header" tab-line-format "Tab" mode-line-format "Mode")
        (switch-to-buffer (get-buffer-create "*ui-header-left*"))
        (erase-buffer) (insert "left text for wheel\n") (goto-char 1)
        (setq qa-ui-header-left (selected-window) qa-ui-header-right (split-window-right 40))
        (with-current-buffer (get-buffer-create "*ui-header-right*")
          (erase-buffer) (insert "right text for wheel\n") (goto-char 1))
        (set-window-buffer qa-ui-header-right "*ui-header-right*")
        (setq qa-ui-header-wheels nil)
        (defun qa-ui-header-record (event) (interactive "e")
          (setq qa-ui-header-wheels (append qa-ui-header-wheels
            (list (list (car event) (posn-area (event-start event))
              (eq (posn-window (event-start event)) qa-ui-header-right)
              (event-line-count event))))))
        (global-set-key [header-line wheel-down] #'qa-ui-header-record)
        (global-set-key [header-line C-wheel-up] #'qa-ui-header-record)
        (global-set-key [header-line wheel-left] #'qa-ui-header-record)
        (global-set-key [header-line wheel-right] #'qa-ui-header-record)
        (global-set-key [wheel-down] #'qa-ui-header-record)
        (global-set-key [f12] (lambda () (interactive)
          (setq qa-ui-header-result (list qa-ui-header-wheels
            (eq (selected-window) qa-ui-header-left)))
          (kill-emacs))))"#,
    )
    .unwrap();
    let frame = i.selected_frame.as_ref().unwrap().clone();
    reflow_frame_windows(&frame, 80, 16);
    let left = frame.borrow().selected.borrow().id;
    let Value::Window(right) = i.eval_str("qa-ui-header-right").unwrap() else {
        panic!("right window")
    };
    let right = right.borrow().id;
    let mut grid = render_grid_with_input_cached(&i, 80, 16, None);
    let left_header = *grid
        .header_lines
        .iter()
        .find(|header| header.window_id == left)
        .unwrap();
    let right_header = *grid
        .header_lines
        .iter()
        .find(|header| header.window_id == right)
        .unwrap();
    grid.row_heights[0] *= 2;
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
    let targets = handle
        .update(&mut app, |view, _, _| {
            [right_header, left_header, right_header]
                .into_iter()
                .enumerate()
                .map(|(index, header)| {
                    let row = header.row + usize::from(index == 2);
                    point(
                        px(3.0) + view.cell_width * (header.left + 2) as f32 + px(1.0),
                        px(3.0) + super::grid_row_top(&view.grid, row, view.line_height) + px(5.0),
                    )
                })
                .collect::<Vec<_>>()
        })
        .unwrap();
    let horizontal_target = targets[0].clone();
    rx.try_iter().for_each(drop);
    let mut visual = gpui::VisualTestContext::from_window(handle.into(), &app);
    for ((position, amount), modifiers) in targets.into_iter().zip([-2.0, 1.0, -1.0]).zip([
        Modifiers::none(),
        Modifiers {
            control: true,
            ..Default::default()
        },
        Modifiers::none(),
    ]) {
        visual.simulate_event(ScrollWheelEvent {
            position,
            delta: ScrollDelta::Lines(point(0.0, amount)),
            modifiers,
            ..Default::default()
        });
    }
    for amount in [1.0, -1.0] {
        visual.simulate_event(ScrollWheelEvent {
            position: horizontal_target,
            delta: ScrollDelta::Lines(point(amount, 0.0)),
            modifiers: Modifiers::none(),
            ..Default::default()
        });
    }
    visual.simulate_event(ScrollWheelEvent {
        position: horizontal_target,
        delta: ScrollDelta::Pixels(point(px(0.0), px(5.0))),
        modifiers: Modifiers::none(),
        ..Default::default()
    });
    let events: Vec<_> = rx
        .try_iter()
        .filter(|event| matches!(event, GuiEvent::Scroll { .. }))
        .collect();
    assert_eq!(events.len(), 6);
    let (bridge_tx, bridge_rx) = mpsc::channel();
    for (event, (window, area, modifiers, horizontal, expected_pixel_delta)) in
        events.into_iter().zip([
            (right, Some(MouseArea::HeaderLine), 0, false, None),
            (left, Some(MouseArea::HeaderLine), CHAR_CTL, false, None),
            (right, None, 0, false, None),
            (right, Some(MouseArea::HeaderLine), 0, true, None),
            (right, Some(MouseArea::HeaderLine), 0, true, None),
            (right, Some(MouseArea::HeaderLine), 0, false, Some((0, 5))),
        ])
    {
        let GuiEvent::Scroll {
            horizontal: actual_horizontal,
            pixel_delta: actual_pixel_delta,
            position,
            modifiers: actual,
            ..
        } = &event
        else {
            unreachable!()
        };
        assert_eq!(position.window_id, Some(window));
        assert_eq!(position.area, area);
        assert_eq!(*actual, modifiers);
        assert_eq!(*actual_horizontal, horizontal);
        assert_eq!(*actual_pixel_delta, expected_pixel_delta);
        bridge_tx.send(event).unwrap();
    }
    bridge_tx
        .send(GuiEvent::Key(event_code_for("f12")))
        .unwrap();
    drop(bridge_tx);
    let (grid_tx, _grid_rx) = smol::channel::bounded(1);
    run_editor_with(ChanIo::new_for_test(bridge_rx, grid_tx), &mut i).unwrap();
    let result = i.intern("qa-ui-header-result");
    assert_eq!(
        i.prin1_to_string(&i.symbol_value(result))
            .to_unicode_lossy(),
        "(((wheel-down header-line t 2) (C-wheel-up header-line nil 1) (wheel-down nil t 1) (wheel-left header-line t 1) (wheel-right header-line t 1)) t)"
    );
}
