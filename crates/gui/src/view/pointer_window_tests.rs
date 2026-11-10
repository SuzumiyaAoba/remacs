use super::{EditorView, GuiEvent};
use gpui::{CursorStyle, Modifiers, point, px};
use remacs_runtime::{
    editor::reflow_frame_windows,
    frontend::render_grid_with_mouse_position_cached,
    lisp::{Interp, Value},
};
use std::sync::mpsc;

#[test]
fn hovering_nonselected_text_maps_its_window_and_updates_the_native_pointer() {
    let mut i = Interp::new();
    i.eval_str(
        r#"(progn
        (set-frame-parameter nil 'window-system 'ns)
        (set-frame-parameter nil 'menu-bar-lines 0)
        (set-frame-parameter nil 'tool-bar-lines 0)
        (set-frame-parameter nil 'tab-bar-lines 0)
        (set-frame-parameter nil 'vertical-scroll-bars nil)
        (set-frame-size nil 80 12) (set-window-fringes nil 0 0)
        (setq-default mode-line-format nil header-line-format nil tab-line-format nil)
        (switch-to-buffer (get-buffer-create "*ui-pointer-left*"))
        (erase-buffer) (insert (propertize "left text" 'pointer 'arrow)) (goto-char 1)
        (setq qa-ui-pointer-left (selected-window) qa-ui-pointer-right (split-window-right 40))
        (with-current-buffer (get-buffer-create "*ui-pointer-right*")
          (erase-buffer) (insert (propertize "hand" 'pointer 'hand) " plain text") (goto-char 1))
        (set-window-buffer qa-ui-pointer-right "*ui-pointer-right*"))"#,
    )
    .unwrap();
    let frame = i.selected_frame.as_ref().unwrap().clone();
    reflow_frame_windows(&frame, 80, 12);
    let selected = frame.borrow().selected.clone();
    let left = selected.borrow().id;
    let Value::Window(right) = i.eval_str("qa-ui-pointer-right").unwrap() else {
        panic!("right window")
    };
    let right = right.borrow().id;
    let current = i.current_buffer;
    let grid = render_grid_with_mouse_position_cached(&i, 80, 12, None, None);
    let mut app = gpui::TestAppContext::single_with_native_text_system();
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
    let cell = handle
        .update(&mut app, |view, _, _| f32::from(view.cell_width))
        .unwrap();
    let mut visual = gpui::VisualTestContext::from_window(handle.into(), &app);
    for (column, expected_window, expected) in [
        (41, right, CursorStyle::PointingHand),
        (47, right, CursorStyle::IBeam),
        (1, left, CursorStyle::Arrow),
    ] {
        rx.try_iter().for_each(drop);
        let diagnostic = handle
            .update(&mut app, |view, window, _| {
                let point = point(px(cell * column as f32 + 1.0), px(5.0));
                (
                    super::input::mouse_position_for_point_with_cell_width(
                        &view.grid,
                        point,
                        px(cell),
                        window,
                    ),
                    view.grid
                        .input
                        .as_ref()
                        .and_then(|input| input.hit_test_source_range(0, column)),
                )
            })
            .unwrap();
        visual.simulate_mouse_move(
            point(px(cell * column as f32 + 1.0), px(5.0)),
            None,
            Modifiers::none(),
        );
        let position = rx
            .try_iter()
            .filter_map(|event| match event {
                GuiEvent::MouseMove { position, .. } => position,
                _ => None,
            })
            .last()
            .unwrap_or_else(|| panic!("column {column}: no text hit, cell={cell}, {diagnostic:?}"));
        assert_eq!(position.window_id, Some(expected_window));
        let grid = render_grid_with_mouse_position_cached(&i, 80, 12, None, Some(position));
        handle
            .update(&mut app, |view, _, cx| {
                view.grid = grid;
                cx.notify();
            })
            .unwrap();
        app.run_until_parked();
        assert_eq!(app.active_cursor_style(), expected, "column {column}");
        assert_eq!(i.current_buffer, current);
        assert!(std::rc::Rc::ptr_eq(&selected, &frame.borrow().selected));
    }
}

fn assert_mouse_position_is_relative_to_split_window(split_call: &str) {
    let mut i = Interp::new();
    i.eval_str(&format!(
        r#"(progn
        (set-frame-parameter nil 'window-system 'ns)
        (set-frame-parameter nil 'menu-bar-lines 0)
        (set-frame-parameter nil 'tool-bar-lines 0)
        (set-frame-parameter nil 'tab-bar-lines 0)
        (set-frame-parameter nil 'vertical-scroll-bars nil)
        (set-frame-size nil 80 12) (set-window-fringes nil 0 0)
        (setq-default mode-line-format nil header-line-format nil tab-line-format nil)
        (switch-to-buffer (get-buffer-create "*ui-mouse-left*"))
        (erase-buffer) (insert "left text\nsecond line") (goto-char 1)
        (setq qa-ui-mouse-target ({split_call}))
        (with-current-buffer (get-buffer-create "*ui-mouse-target*")
          (erase-buffer) (insert "target text\nsecond line") (goto-char 1))
        (set-window-buffer qa-ui-mouse-target "*ui-mouse-target*")
        (set-window-fringes qa-ui-mouse-target 0 0)
        (select-window qa-ui-mouse-target))"#
    ))
    .unwrap();
    let frame = i.selected_frame.as_ref().unwrap().clone();
    reflow_frame_windows(&frame, 80, 12);
    let Value::Window(target) = i.eval_str("qa-ui-mouse-target").unwrap() else {
        panic!("split window")
    };
    let target_id = target.borrow().id;
    let grid = render_grid_with_mouse_position_cached(&i, 80, 12, None, None);
    let area = grid
        .window_text_areas
        .iter()
        .find(|area| area.window_id == target_id)
        .unwrap();
    let area_left = area.left;
    let area_top = area.top;

    let mut app = gpui::TestAppContext::single_with_native_text_system();
    let (tx, _rx) = mpsc::channel();
    let handle = app.add_window(|_, cx| EditorView::new(tx, cx));
    handle
        .update(&mut app, |view, window, cx| {
            window.activate_window();
            view.grid = grid;
            cx.notify();
        })
        .unwrap();
    app.run_until_parked();
    let (cell, position) = handle
        .update(&mut app, |view, window, _| {
            let cell = f32::from(view.cell_width);
            let origin_x = cell * area_left as f32;
            let origin_y = super::grid_row_top(&view.grid, area_top, px(super::LINE_HEIGHT));
            let point = point(px(origin_x + 2.0), origin_y + px(4.0));
            (
                cell,
                super::input::mouse_position_for_point_with_cell_width(
                    &view.grid,
                    point,
                    px(cell),
                    window,
                ),
            )
        })
        .unwrap();
    let position = position.unwrap_or_else(|| panic!("no text hit, cell={cell}"));
    assert_eq!(position.window_id, Some(target_id));
    assert_eq!((position.x, position.y), (2, 4));
}

#[test]
fn right_split_mouse_position_is_window_relative() {
    assert_mouse_position_is_relative_to_split_window("split-window-right 40");
}

#[test]
fn lower_split_mouse_position_is_window_relative() {
    assert_mouse_position_is_relative_to_split_window("split-window-below 6");
}
