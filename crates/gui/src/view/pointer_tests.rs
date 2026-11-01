use super::{EditorView, Grid, GuiEvent};
use gpui::{Modifiers, point, px};
use std::sync::mpsc;

#[test]
fn hourglass_pointer_reaches_the_platform_and_respects_pointer_visibility() {
    use remacs_runtime::frontend::{GridPointerRegion, GridPointerShape};

    let mut app = gpui::TestAppContext::single();
    let (tx, _rx) = mpsc::channel();
    let handle = app.add_window(|_, cx| EditorView::new(tx, cx));
    handle
        .update(&mut app, |view, window, cx| {
            window.activate_window();
            view.grid = Grid::blank(40, 12);
            view.grid.rows = vec![" ".repeat(40); 12];
            view.grid.hover_cursor = Some(GridPointerShape::Hourglass);
            view.grid.pointer_regions = vec![GridPointerRegion {
                row: 1,
                left: 0,
                width: 40,
                shape: GridPointerShape::Text,
            }];
            cx.notify();
        })
        .unwrap();
    app.run_until_parked();
    let line_height = handle
        .update(&mut app, |view, _, _| {
            view.geometry.as_ref().unwrap().char_height as f32
        })
        .unwrap();
    let mut visual = gpui::VisualTestContext::from_window(handle.into(), &app);
    visual.simulate_mouse_move(point(px(5.0), px(5.0)), None, Modifiers::none());
    assert_eq!(app.active_cursor_style(), gpui::CursorStyle::Wait);
    visual.simulate_mouse_move(
        point(px(5.0), px(line_height + 5.0)),
        None,
        Modifiers::none(),
    );
    assert_eq!(app.active_cursor_style(), gpui::CursorStyle::IBeam);
    handle
        .update(&mut app, |view, _, cx| {
            view.grid.pointer_visible = false;
            cx.notify();
        })
        .unwrap();
    app.run_until_parked();
    visual.simulate_mouse_move(point(px(5.0), px(5.0)), None, Modifiers::none());
    assert_eq!(app.active_cursor_style(), gpui::CursorStyle::None);
    handle
        .update(&mut app, |view, _, cx| {
            view.grid.pointer_visible = true;
            cx.notify();
        })
        .unwrap();
    app.run_until_parked();
    visual.simulate_mouse_move(point(px(5.0), px(5.0)), None, Modifiers::none());
    assert_eq!(app.active_cursor_style(), gpui::CursorStyle::Wait);
}

#[test]
fn auxiliary_pointer_regions_set_native_cursor_without_swallowing_mouse_motion() {
    use remacs_runtime::frontend::{GridPointerRegion, GridPointerShape};

    let mut app = gpui::TestAppContext::single();
    let (tx, rx) = mpsc::channel();
    let handle = app.add_window(|_, cx| EditorView::new(tx, cx));
    handle
        .update(&mut app, |view, window, cx| {
            window.activate_window();
            view.set_internal_border_width(3);
            view.grid = Grid::blank(40, 12);
            view.grid.rows = vec![" ".repeat(40); 12];
            view.grid.pointer_regions = vec![
                GridPointerRegion {
                    row: 0,
                    left: 0,
                    width: 6,
                    shape: GridPointerShape::Hand,
                },
                GridPointerRegion {
                    row: 0,
                    left: 6,
                    width: 34,
                    shape: GridPointerShape::Arrow,
                },
                GridPointerRegion {
                    row: 1,
                    left: 0,
                    width: 40,
                    shape: GridPointerShape::Text,
                },
            ];
            cx.notify();
        })
        .unwrap();
    app.run_until_parked();
    let (cell_width, line_height) = handle
        .update(&mut app, |view, _, _| {
            let geometry = view.geometry.as_ref().unwrap();
            (geometry.char_width as f32, geometry.char_height as f32)
        })
        .unwrap();
    let mut visual = gpui::VisualTestContext::from_window(handle.into(), &app);
    visual.simulate_mouse_move(
        point(px(3.0 + cell_width * 2.0), px(8.0)),
        None,
        Modifiers::none(),
    );
    assert_eq!(app.active_cursor_style(), gpui::CursorStyle::PointingHand);
    visual.simulate_mouse_move(
        point(px(3.0 + cell_width * 8.0), px(8.0)),
        None,
        Modifiers::none(),
    );
    assert_eq!(app.active_cursor_style(), gpui::CursorStyle::Arrow);
    visual.simulate_mouse_move(
        point(px(3.0 + cell_width * 2.0), px(3.0 + line_height + 5.0)),
        None,
        Modifiers::none(),
    );
    assert_eq!(app.active_cursor_style(), gpui::CursorStyle::IBeam);
    assert!(
        rx.try_iter()
            .filter(|event| matches!(event, GuiEvent::MouseMove { .. }))
            .count()
            >= 3
    );

    handle
        .update(&mut app, |view, _, cx| {
            view.grid.pointer_visible = false;
            cx.notify();
        })
        .unwrap();
    app.run_until_parked();
    visual.simulate_mouse_move(
        point(px(3.0 + cell_width * 2.0), px(8.0)),
        None,
        Modifiers::none(),
    );
    assert_eq!(app.active_cursor_style(), gpui::CursorStyle::None);
}

#[test]
fn dragging_tracking_retains_shape_across_hover_redraw_and_visibility_changes() {
    use remacs_runtime::frontend::{GridPointerRegion, GridPointerShape};
    let mut app = gpui::TestAppContext::single();
    let (tx, rx) = mpsc::channel();
    let handle = app.add_window(|_, cx| EditorView::new(tx, cx));
    handle
        .update(&mut app, |view, window, cx| {
            window.activate_window();
            view.grid = Grid::blank(40, 12);
            view.grid.rows = vec![" ".repeat(40); 12];
            view.grid.hover_cursor = Some(GridPointerShape::Text);
            view.grid.pointer_regions = vec![GridPointerRegion {
                row: 0,
                left: 6,
                width: 34,
                shape: GridPointerShape::Hand,
            }];
            cx.notify();
        })
        .unwrap();
    app.run_until_parked();
    let width = handle
        .update(&mut app, |view, _, _| {
            view.geometry.as_ref().unwrap().char_width as f32
        })
        .unwrap();
    let mut visual = gpui::VisualTestContext::from_window(handle.into(), &app);
    visual.simulate_mouse_move(point(px(width * 2.0), px(5.0)), None, Modifiers::none());
    assert_eq!(app.active_cursor_style(), gpui::CursorStyle::IBeam);
    handle
        .update(&mut app, |view, _, cx| {
            view.grid.freeze_pointer_shape = true;
            cx.notify();
        })
        .unwrap();
    app.run_until_parked();
    visual.simulate_mouse_move(
        point(px(width * 8.0), px(5.0)),
        Some(gpui::MouseButton::Left),
        Modifiers::none(),
    );
    assert_eq!(app.active_cursor_style(), gpui::CursorStyle::IBeam);
    handle
        .update(&mut app, |view, _, cx| {
            view.grid.hover_cursor = Some(GridPointerShape::Arrow);
            cx.notify();
        })
        .unwrap();
    app.run_until_parked();
    visual.simulate_mouse_move(
        point(px(width * 2.0), px(5.0)),
        Some(gpui::MouseButton::Left),
        Modifiers::none(),
    );
    assert_eq!(app.active_cursor_style(), gpui::CursorStyle::IBeam);
    assert!(
        rx.try_iter()
            .filter(|event| matches!(event, GuiEvent::MouseDragMove { .. }))
            .count()
            >= 2
    );
    handle
        .update(&mut app, |view, _, cx| {
            view.grid.freeze_pointer_shape = false;
            cx.notify();
        })
        .unwrap();
    app.run_until_parked();
    assert_eq!(app.active_cursor_style(), gpui::CursorStyle::Arrow);

    visual.simulate_mouse_move(point(px(width * 8.0), px(5.0)), None, Modifiers::none());
    assert_eq!(app.active_cursor_style(), gpui::CursorStyle::PointingHand);
    handle
        .update(&mut app, |view, _, cx| {
            view.grid.freeze_pointer_shape = true;
            view.grid.pointer_visible = false;
            cx.notify();
        })
        .unwrap();
    app.run_until_parked();
    assert_eq!(app.active_cursor_style(), gpui::CursorStyle::None);
    handle
        .update(&mut app, |view, _, cx| {
            view.grid.pointer_visible = true;
            cx.notify();
        })
        .unwrap();
    app.run_until_parked();
    assert_eq!(app.active_cursor_style(), gpui::CursorStyle::PointingHand);
    visual.simulate_mouse_move(point(px(width * 2.0), px(5.0)), None, Modifiers::none());
    assert_eq!(app.active_cursor_style(), gpui::CursorStyle::PointingHand);
    handle
        .update(&mut app, |view, _, cx| {
            view.grid.freeze_pointer_shape = false;
            cx.notify();
        })
        .unwrap();
    app.run_until_parked();
    assert_eq!(app.active_cursor_style(), gpui::CursorStyle::Arrow);
}
