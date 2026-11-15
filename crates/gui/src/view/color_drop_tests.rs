use super::{LINE_HEIGHT, input::mode_line_mouse_position};
use gpui::{point, px};
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
