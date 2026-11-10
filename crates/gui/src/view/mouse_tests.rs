use super::{EditorView, GuiEvent};
use crate::logic::ChanIo;
use gpui::{Modifiers, MouseButton as GpuiMouseButton, NavigationDirection, point, px};
use remacs_runtime::{
    editor::{CHAR_CTL, CHAR_SHIFT, event_code_for, reflow_frame_windows},
    frontend::{
        Grid, MouseArea, MouseButton as Button, render_grid_with_input_cached, run_editor_with,
    },
    lisp::Interp,
};
use std::sync::mpsc;

#[test]
fn mouse_drag_events_keep_current_modifiers() {
    let mut app = gpui::TestAppContext::single();
    let (tx, rx) = mpsc::channel();
    let handle = app.add_window(|_, cx| EditorView::new(tx, cx));
    handle
        .update(&mut app, |view, window, cx| {
            window.activate_window();
            view.set_internal_border_width(0);
            let mut grid = Grid::blank(40, 8);
            grid.rows = vec![" ".repeat(40); 8];
            view.grid = grid;
            cx.notify();
        })
        .unwrap();
    app.run_until_parked();

    let mut visual = gpui::VisualTestContext::from_window(handle.into(), &app);
    let start = point(px(5.0), px(5.0));
    let end = point(px(25.0), px(5.0));
    visual.simulate_mouse_down(start, GpuiMouseButton::Left, Modifiers::none());
    visual.simulate_mouse_move(
        end,
        Some(GpuiMouseButton::Left),
        Modifiers {
            shift: true,
            ..Default::default()
        },
    );

    let events: Vec<_> = rx.try_iter().collect();
    assert!(events.iter().any(|event| matches!(
        event,
        GuiEvent::MouseMove { modifiers: CHAR_SHIFT, .. }
    )));
    assert!(events.iter().any(|event| matches!(
        event,
        GuiEvent::MouseDragMove { modifiers: CHAR_SHIFT, .. }
    )));
}

#[test]
fn navigation_buttons_reach_the_gui_channel_with_position_and_modifiers() {
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
      (erase-buffer) (insert "navigation button test")
      (setq context-menu-mode nil remacs-gui-navigation-events nil)
      (defun remacs-gui-navigation-record (event)
        (interactive "e")
        (setq remacs-gui-navigation-events
          (append remacs-gui-navigation-events (list (car event)))))
      (dolist (key '([C-S-down-mouse-2] [C-S-mouse-2]
                    [C-S-down-mouse-3] [C-S-mouse-3]
                    [C-S-down-mouse-4] [C-S-mouse-4]
                    [C-S-down-mouse-5] [C-S-mouse-5]))
        (global-set-key key #'remacs-gui-navigation-record))
      (global-set-key [f12] (lambda () (interactive) (kill-emacs))))"#,
    )
    .unwrap();
    let frame = i.selected_frame.as_ref().unwrap();
    reflow_frame_windows(frame, 40, 12);
    let window_id = frame.borrow().selected.borrow().id;
    let buffer_id = i.current_buffer;
    let grid = render_grid_with_input_cached(&i, 40, 12, None);
    let mut app = gpui::TestAppContext::single();
    let (tx, rx) = mpsc::channel();
    let handle = app.add_window(|_, cx| EditorView::new(tx, cx));
    handle
        .update(&mut app, |view, window, cx| {
            window.activate_window();
            view.grid = grid;
            cx.notify();
        })
        .unwrap();
    app.run_until_parked();
    let width = handle
        .update(&mut app, |view, _, _| {
            view.geometry.as_ref().unwrap().char_width as f32
        })
        .unwrap();
    let point = point(px(width * 2.0), px(5.0));
    let mut visual = gpui::VisualTestContext::from_window(handle.into(), &app);
    let buttons = [
        (gpui::MouseButton::Middle, Button::Middle),
        (gpui::MouseButton::Right, Button::Right),
        (
            gpui::MouseButton::Navigate(NavigationDirection::Back),
            Button::Back,
        ),
        (
            gpui::MouseButton::Navigate(NavigationDirection::Forward),
            Button::Forward,
        ),
    ];
    for (button, _) in buttons {
        visual.simulate_mouse_down(
            point,
            button,
            Modifiers {
                control: true,
                shift: true,
                ..Default::default()
            },
        );
        visual.simulate_mouse_up(point, button, Modifiers::none());
    }
    let events: Vec<_> = rx
        .try_iter()
        .filter(|event| matches!(event, GuiEvent::MouseDown { .. } | GuiEvent::MouseUp { .. }))
        .collect();
    assert_eq!(events.len(), buttons.len() * 2);
    for ((_, expected), pair) in buttons.into_iter().zip(events.chunks_exact(2)) {
        let GuiEvent::MouseDown {
            button,
            click_count,
            modifiers,
            position,
        } = &pair[0]
        else {
            panic!("down event");
        };
        assert_eq!(*button, expected);
        assert_eq!(*click_count, 1);
        assert_eq!(*modifiers, CHAR_CTL | CHAR_SHIFT);
        assert_eq!(position.buffer_id, buffer_id);
        assert_eq!(position.window_id, Some(window_id));
        assert_eq!(position.area, None);
        let GuiEvent::MouseUp {
            button,
            click_count,
            modifiers,
            position: end,
        } = &pair[1]
        else {
            panic!("up event");
        };
        assert_eq!(*button, expected);
        assert_eq!(*click_count, 1);
        assert_eq!(*modifiers, 0);
        assert_eq!(end, position);
    }

    let (bridge_tx, bridge_rx) = mpsc::channel();
    for event in events {
        bridge_tx.send(event).unwrap();
    }
    bridge_tx
        .send(GuiEvent::Key(event_code_for("f12")))
        .unwrap();
    drop(bridge_tx);
    let (grid_tx, _grid_rx) = smol::channel::bounded(1);
    run_editor_with(ChanIo::new_for_test(bridge_rx, grid_tx), &mut i).unwrap();
    let symbol = i.intern("remacs-gui-navigation-events");
    assert_eq!(
        i.prin1_to_string(&i.symbol_value(symbol))
            .to_unicode_lossy(),
        "(C-S-down-mouse-2 C-S-mouse-2 C-S-down-mouse-3 C-S-mouse-3 C-S-down-mouse-4 C-S-mouse-4 C-S-down-mouse-5 C-S-mouse-5)"
    );
}

#[test]
fn mode_line_middle_clicks_send_mode_line_mouse_events() {
    let mut interp = Interp::new();
    interp
        .eval_str(
            r#"(progn
          (set-frame-parameter nil 'menu-bar-lines 0)
          (set-frame-parameter nil 'tool-bar-lines 0)
          (set-frame-parameter nil 'tab-bar-lines 0)
          (set-frame-parameter nil 'vertical-scroll-bars nil)
          (set-frame-size nil 40 12) (set-window-fringes nil 0 0)
          (setq-local mode-line-format "Mode"
                      header-line-format nil tab-line-format nil)
          (erase-buffer))"#,
        )
        .unwrap();
    let frame = interp.selected_frame.as_ref().unwrap();
    reflow_frame_windows(frame, 40, 12);
    let grid = render_grid_with_input_cached(&interp, 40, 12, None);
    let mode_line = *grid.mode_lines.first().expect("mode line should be rendered");
    assert!(mode_line.width > 0);

    let mut app = gpui::TestAppContext::single();
    let (tx, rx) = mpsc::channel();
    let handle = app.add_window(|_, cx| EditorView::new(tx, cx));
    handle
        .update(&mut app, |view, window, cx| {
            window.activate_window();
            view.set_internal_border_width(0);
            view.grid = grid.clone();
            cx.notify();
        })
        .unwrap();
    app.run_until_parked();
    let (cell_width, cell_height) = handle
        .update(&mut app, |view, _, _| {
            let geometry = view.geometry.as_ref().unwrap();
            (geometry.char_width as f32, geometry.char_height as f32)
        })
        .unwrap();
    let column = mode_line.left + 1;
    let row_top = super::grid_row_top(&grid, mode_line.row, px(cell_height));
    let row_height = super::grid_row_height(&grid, mode_line.row, px(cell_height));
    let point = point(
        px((column as f32 + 0.5) * cell_width),
        px(f32::from(row_top) + f32::from(row_height) / 2.0),
    );
    let mut visual = gpui::VisualTestContext::from_window(handle.into(), &app);
    visual.simulate_mouse_down(point, GpuiMouseButton::Middle, Modifiers::none());
    visual.simulate_mouse_up(point, GpuiMouseButton::Middle, Modifiers::none());

    let events: Vec<_> = rx
        .try_iter()
        .filter(|event| matches!(event, GuiEvent::MouseDown { .. } | GuiEvent::MouseUp { .. }))
        .collect();
    assert_eq!(events.len(), 2);
    for event in events {
        let position = match event {
            GuiEvent::MouseDown {
                button: Button::Middle,
                position,
                ..
            }
            | GuiEvent::MouseUp {
                button: Button::Middle,
                position,
                ..
            } => position,
            _ => panic!("unexpected non-middle mode-line mouse event"),
        };
        assert_eq!(position.area, Some(MouseArea::ModeLine));
        assert_eq!(position.window_id, Some(mode_line.window_id));
    }
}
