use super::EditorView;
use std::sync::mpsc;

#[test]
fn initial_minimum_cells_reach_native_window_before_its_first_render() {
    let mut app = gpui::TestAppContext::single();
    let (tx, _rx) = mpsc::channel();
    let window = app.add_window(|_, cx| EditorView::new(tx, cx));

    window
        .update(&mut app, |view, window, _| {
            let expected = crate::frame_minimum_size(
                Some(120),
                Some(50),
                f32::from(view.cell_width),
                f32::from(view.line_height),
                0.0,
            )
            .unwrap();
            view.apply_minimum_cells(window, Some(120), Some(50));
            assert_eq!(
                view.native_minimum_size,
                Some((expected, window.scale_factor()))
            );
        })
        .unwrap();
}
