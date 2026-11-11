use super::{EditorView, GuiEvent};
use crate::logic::ChanIo;
use gpui::{
    Modifiers, MouseButton as GpuiMouseButton, NavigationDirection, ScrollDelta, ScrollWheelEvent,
    TouchPhase, point, px,
};
use remacs_runtime::{
    editor::{CHAR_CTL, CHAR_SHIFT, event_code_for, reflow_frame_windows},
    frontend::{
        CellStyle, Grid, MouseArea, MouseButton as Button, render_grid_with_input_cached,
        run_editor_with,
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
fn input_geometry_uses_default_and_per_run_font_metrics() {
    let mut base = Grid::blank(4, 1);
    base.rows[0] = "iiii".to_owned();
    base.font_families = vec!["Menlo".to_owned(), "Arial".to_owned()];
    base.default_style.font_family = Some(0);
    base.default_style.font_height = Some(140);
    base.default_style.font_width = Some(100);

    let mut tall_default = base.clone();
    tall_default.default_style.font_height = Some(280);

    let mut expanded_run = base.clone();
    expanded_run.char_attrs = vec![vec![
        CellStyle {
            font_family: Some(1),
            font_height: Some(280),
            font_width: Some(200),
            ..Default::default()
        };
        4
    ]];

    let mut app = gpui::TestAppContext::single();
    let (tx, _rx) = mpsc::channel();
    let handle = app.add_window(|_, cx| EditorView::new(tx, cx));
    handle
        .update(&mut app, |view, window, cx| {
            window.activate_window();
            view.grid = base.clone();
            cx.notify();
        })
        .unwrap();
    app.run_until_parked();

    let widths = handle
        .update(&mut app, |_, window, _| {
            let base_width = f32::from(
                super::input::shape_grid_row(&base, 0, window, &base.rows[0]).x_for_index(4),
            );
            let tall_width = f32::from(
                super::input::shape_grid_row(
                    &tall_default,
                    0,
                    window,
                    &tall_default.rows[0],
                )
                .x_for_index(4),
            );
            let run_width = f32::from(
                super::input::shape_grid_row(
                    &expanded_run,
                    0,
                    window,
                    &expanded_run.rows[0],
                )
                .x_for_index(4),
            );
            (base_width, tall_width, run_width)
        })
        .unwrap();
    assert!(widths.1 > widths.0 * 1.5, "default face height affects shaping: {widths:?}");
    assert!(widths.2 > widths.0 * 1.5, "run face metrics affect shaping: {widths:?}");
}

#[cfg(target_os = "macos")]
#[test]
fn zwj_family_emoji_keeps_following_text_at_grid_column() {
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
          (set-face-attribute 'default nil :family "Menlo" :height 140)
          (erase-buffer) (insert "👨‍👩‍👧‍👦X"))"#,
    )
    .unwrap();
    assert_eq!(
        i.eval_str("(string-width \"👨‍👩‍👧‍👦\")")
            .unwrap()
            .int(),
        Some(8)
    );
    let frame = i.selected_frame.as_ref().unwrap().clone();
    reflow_frame_windows(&frame, 40, 12);
    let grid = render_grid_with_input_cached(&i, 40, 12, None);
    assert!(grid.rows[0].contains('\u{200d}'), "grid row lost the ZWJ joiners");

    let mut app = gpui::TestAppContext::single_with_native_text_system();
    let (tx, _rx) = mpsc::channel();
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

    let x_byte = grid.rows[0].find('X').unwrap();
    let (actual, expected) = handle
        .update(&mut app, |view, window, _| {
            let line = super::input::shape_grid_row(&grid, 0, window, &grid.rows[0]);
            (
                f32::from(line.x_for_index(x_byte)),
                f32::from(view.cell_width) * 8.0,
            )
        })
        .unwrap();
    assert!(
        (actual - expected).abs() < 1.0,
        "X starts at {actual}px, expected grid column 8 at {expected}px"
    );
}

#[test]
fn trackpad_touch_end_reaches_the_lisp_keymap() {
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
          (erase-buffer) (insert "trackpad touch end")
          (setq qa-touch-end nil)
          (global-set-key [touch-end]
            (lambda (event) (interactive "e") (setq qa-touch-end (car event))))
          (global-set-key [f12] (lambda () (interactive) (kill-emacs))))"#,
    )
    .unwrap();
    let frame = i.selected_frame.as_ref().unwrap().clone();
    reflow_frame_windows(&frame, 40, 12);
    let window_id = frame.borrow().selected.borrow().id;
    let buffer_id = i.current_buffer;
    let grid = render_grid_with_input_cached(&i, 40, 12, None);

    let mut app = gpui::TestAppContext::single();
    let (tx, rx) = mpsc::channel();
    let view_tx = tx.clone();
    let handle = app.add_window(|_, cx| EditorView::new(view_tx, cx));
    handle
        .update(&mut app, |view, window, cx| {
            window.activate_window();
            view.set_internal_border_width(0);
            view.grid = grid;
            cx.notify();
        })
        .unwrap();
    app.run_until_parked();
    let cell_width = handle
        .update(&mut app, |view, _, _| {
            view.geometry.as_ref().unwrap().char_width as f32
        })
        .unwrap();
    rx.try_iter().for_each(drop);

    let mut visual = gpui::VisualTestContext::from_window(handle.into(), &app);
    visual.simulate_event(ScrollWheelEvent {
        position: point(px(cell_width * 2.0), px(5.0)),
        delta: ScrollDelta::Pixels(point(px(0.0), px(0.0))),
        touch_phase: TouchPhase::Ended,
        ..Default::default()
    });
    let events: Vec<_> = rx.try_iter().collect();
    let touch_end = events.iter().find_map(|event| match event {
        GuiEvent::TouchEnd {
            modifiers,
            position,
        } => Some((*modifiers, *position)),
        _ => None,
    });
    let (modifiers, position) = touch_end.expect("touch-end should reach the GUI channel");
    assert_eq!(position.buffer_id, buffer_id);
    assert_eq!(position.window_id, Some(window_id));
    assert_eq!(modifiers, 0);

    tx.send(GuiEvent::TouchEnd {
        modifiers,
        position,
    })
    .unwrap();
    tx.send(GuiEvent::Key(event_code_for("f12"))).unwrap();
    let (grid_tx, _grid_rx) = smol::channel::bounded(1);
    let result = run_editor_with(ChanIo::new_for_test(rx, grid_tx), &mut i);
    if let Err(error) = result {
        assert_eq!(error.to_string(), "Exit(0)");
    }
    let touch_end_symbol = i.intern("qa-touch-end");
    let touch_end = i.symbol_value(touch_end_symbol);
    assert_eq!(i.prin1_to_string(&touch_end).to_unicode_lossy(), "touch-end");
}

#[test]
fn extended_tail_padding_uses_default_face_font_metrics() {
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
          (make-face 'qa-extend-font)
          (set-face-attribute 'qa-extend-font nil :family "Arial" :width 'expanded
                              :weight 'bold :height 280 :background "yellow" :extend t)
          (erase-buffer)
          (insert (propertize "x\n" 'face 'qa-extend-font)))"#,
    )
    .unwrap();
    let frame = i.selected_frame.as_ref().unwrap();
    reflow_frame_windows(frame, 40, 12);
    let grid = render_grid_with_input_cached(&i, 40, 12, None);
    let text_style = grid.char_attrs[0][0];
    assert!(text_style.font_family.is_some());
    assert!(text_style.font_width.is_some());
    assert!(text_style.font_weight.is_some());
    assert_eq!(text_style.font_height, Some(280));

    assert!(grid.char_attrs[0][1..].iter().all(|style| {
        style.font_family.is_none()
            && style.font_width.is_none()
            && style.font_weight.is_none()
            && style.font_height.is_none()
    }));
    assert!(grid.attrs[0][1..].iter().all(|style| {
        style.font_family.is_none()
            && style.font_width.is_none()
            && style.font_weight.is_none()
            && style.font_height.is_none()
    }));
}

#[test]
fn negative_single_box_width_splits_vertical_and_horizontal_signs() {
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
          (erase-buffer) (insert "boxed")
          (set-face-attribute 'default nil :box '(:line-width -2)))"#,
    )
    .unwrap();
    let frame = i.selected_frame.as_ref().unwrap();
    reflow_frame_windows(frame, 40, 12);
    let grid = render_grid_with_input_cached(&i, 40, 12, None);
    let box_style = grid.char_attrs[0][0]
        .box_style
        .expect("default face box should be rendered");
    assert_eq!(box_style.vertical_width, 2);
    assert_eq!(box_style.horizontal_width, -2);
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
                    [C-S-down-mouse-8] [C-S-mouse-8]
                    [C-S-down-mouse-9] [C-S-mouse-9]))
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
        "(C-S-down-mouse-2 C-S-mouse-2 C-S-down-mouse-3 C-S-mouse-3 C-S-down-mouse-8 C-S-mouse-8 C-S-down-mouse-9 C-S-mouse-9)"
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
