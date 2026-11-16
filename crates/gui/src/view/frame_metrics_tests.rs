use super::{frame_font_metrics, measure_frame_font_metrics};
use gpui::{Context, IntoElement, Render, TestAppContext, Window, div, font, px};
use remacs_runtime::frontend::{CellStyle, FontAtStyle, FontGlyphKey, Grid};

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

#[test]
fn graphical_font_metrics_publish_real_proportional_glyph_advances() {
    let mut app = TestAppContext::single_with_native_text_system();
    let handle = app.add_window(|_, _cx| MetricsView);
    let (family, metrics) = handle
        .update(&mut app, |_, window, _cx| {
            let mut family = None;
            for candidate in window.text_system().all_font_names() {
                let font_id = window.text_system().resolve_font(&font(candidate.clone()));
                let wide = window
                    .text_system()
                    .advance(font_id, px(super::FONT_SIZE), 'W');
                let narrow = window
                    .text_system()
                    .advance(font_id, px(super::FONT_SIZE), 'i');
                if let (Ok(wide), Ok(narrow)) = (wide, narrow)
                    && f32::from(wide.width) > f32::from(narrow.width) * 1.25
                {
                    family = Some(candidate);
                    break;
                }
            }
            let family = family.expect("system should provide a proportional font");
            let mut grid = Grid::blank(2, 1);
            grid.font_families.push(family.clone());
            grid.default_style.font_family = Some(0);
            let style = CellStyle {
                font_family: Some(0),
                ..CellStyle::default()
            };
            grid.rows[0] = "Wi".to_owned();
            grid.char_attrs[0] = vec![style; 2];
            let (default_font, default_font_size, cell_width, _) = frame_font_metrics(&grid, window);
            let metrics = measure_frame_font_metrics(
                &grid,
                &default_font,
                default_font_size,
                cell_width,
                window,
            );
            (family, metrics)
        })
        .unwrap();

    assert!(metrics.default_cell_width_64 > 0);
    let style = FontAtStyle {
        family: Some(family),
        weight: None,
        width: None,
        height: None,
        bold: false,
        italic: false,
    };
    let width_of = |character| {
        metrics
            .glyph_widths_64
            .get(&FontGlyphKey {
                style: style.clone(),
                character,
            })
            .copied()
            .unwrap()
    };
    assert!(width_of('W') > width_of('i'));
}
