use super::{EditorView, frame_minimum_size};
use gpui::{TestAppContext, px, size};
use std::sync::mpsc;

#[test]
fn minimum_width_defaults_to_ten_cells_and_never_undercuts_the_grid() {
    assert_eq!(
        frame_minimum_size(None, None, 8.0, 19.0, 0.0),
        Some(size(px(80.0), px(0.0)))
    );
    assert_eq!(
        frame_minimum_size(Some(1), None, 7.25, 18.5, 6.0),
        Some(size(px(79.0), px(0.0)))
    );
    assert_eq!(
        frame_minimum_size(None, Some(2), 7.25, 18.5, 6.0),
        Some(size(px(79.0), px(43.0)))
    );
    assert_eq!(
        frame_minimum_size(Some(20), Some(2), 7.25, 18.5, 6.0),
        Some(size(px(151.0), px(43.0)))
    );
}

#[test]
fn minimum_width_tracks_measured_fonts_borders_and_changed_parameters() {
    let mut app = TestAppContext::single();
    let (tx, _rx) = mpsc::channel();
    let handle = app.add_window(|_, cx| EditorView::new(tx, cx));
    handle
        .update(&mut app, |view, window, _| {
            view.set_internal_border_width(3);
            let minimum = view.update_minimum_size(window, px(9.25), px(19.0));
            assert_eq!(minimum, size(px(99.0), px(0.0)));
        })
        .unwrap();
    app.simulate_window_resize(handle.into(), size(px(1.0), px(100.0)));
    handle
        .update(&mut app, |view, window, _| {
            assert_eq!(window.viewport_size(), size(px(99.0), px(100.0)));
            assert!(view.set_minimum_cells(Some(20), Some(5)));
            view.update_minimum_size(window, px(9.25), px(19.0));
        })
        .unwrap();
    app.simulate_window_resize(handle.into(), size(px(50.0), px(50.0)));
    handle
        .update(&mut app, |view, window, _| {
            assert_eq!(window.viewport_size(), size(px(191.0), px(101.0)));
            // No grid-size or parameter change accompanies this font-metric change.
            view.update_minimum_size(window, px(14.5), px(24.0));
        })
        .unwrap();
    app.simulate_window_resize(handle.into(), size(px(50.0), px(50.0)));
    handle
        .update(&mut app, |view, window, _| {
            assert_eq!(window.viewport_size(), size(px(296.0), px(126.0)));
            view.set_minimum_cells(None, None);
            view.update_minimum_size(window, px(14.5), px(24.0));
        })
        .unwrap();
    app.simulate_window_resize(handle.into(), size(px(1.0), px(100.0)));
    handle
        .update(&mut app, |_, window, _| {
            assert_eq!(window.viewport_size(), size(px(151.0), px(100.0)));
        })
        .unwrap();
}
