use super::frame_font_metrics;
use gpui::{Context, IntoElement, Render, TestAppContext, Window, div, px};
use remacs_runtime::frontend::Grid;

struct MetricsView;

impl Render for MetricsView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
    }
}

#[test]
fn custom_face_height_scales_initial_frame_pixels() {
    let mut app = TestAppContext::single();
    let window = app.add_window(|_, _| MetricsView);
    let mut grid = Grid::blank(90, 30);
    grid.default_style.font_height = Some(220);

    let (cell_width, line_height) = window
        .update(&mut app, |_, window, _| {
            let (_, _, cell_width, line_height) = frame_font_metrics(&grid, window);
            (cell_width, line_height)
        })
        .unwrap();
    let size = crate::frame_pixel_size(90, 30, &grid, cell_width, line_height, 0);

    assert!(f32::from(line_height) > 19.0);
    assert!(f32::from(size.height) > 30.0 * 19.0);
    assert_eq!(size.width, (cell_width * 90.0).max(px(80.0)));
}
