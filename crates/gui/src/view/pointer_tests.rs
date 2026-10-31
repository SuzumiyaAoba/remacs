use super::{EditorView, Grid, GuiEvent};
use gpui::{Modifiers, point, px};
use std::sync::mpsc;

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
