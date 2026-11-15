use super::{LINE_HEIGHT, input::mode_line_mouse_position};
use super::{GuiEvent, external_color_drop_event, external_text_drop_event};
use gpui::{ExternalColor, ExternalText, point, px};
use remacs_runtime::frontend::{Grid, GridModeLine, MouseArea};

#[test]
fn color_drop_can_resolve_a_mode_line_without_buffer_text() {
    let mut grid = Grid::blank(80, 24);
    grid.mode_lines.push(GridModeLine {
        window_id: 7,
        buffer_id: 42,
        row: 20,
        left: 0,
        width: 80,
        buffer_name_range: None,
    });
    let point = point(px(40.0), px(LINE_HEIGHT * 20.0 + 1.0));

    let position = mode_line_mouse_position(&grid, point, px(8.0), 1.0)
        .expect("mode-line drop coordinates should have a frame position");

    assert_eq!(position.area, Some(MouseArea::ModeLine));
    assert_eq!(position.window_id, Some(7));
    assert_eq!(position.buffer_id, 42);
}

#[test]
fn sentinel_text_is_distinct_from_a_native_color_payload() {
    let payload = "\u{1f}remacs-color-drop:foreground:ff0000".to_owned();
    let text_event = external_text_drop_event(None, &ExternalText(payload.clone()));
    assert!(matches!(
        text_event,
        GuiEvent::Drop { urls, .. } if urls == vec![payload.clone()]
    ));

    let color_event = external_color_drop_event(None, &ExternalColor(payload)).unwrap();
    assert!(matches!(
        color_event,
        GuiEvent::ColorDrop {
            color,
            background: false,
            ..
        } if color == "ff0000"
    ));
}
