use super::{EditorView, frame_minimum_size, frame_size_dimension, frame_window_options};
use super::logic::FrameSizeSpec;
use gpui::{TestAppContext, WindowBounds, px, size};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex, atomic::AtomicUsize, mpsc},
};

#[test]
fn minimum_size_defaults_to_ten_columns_and_three_rows() {
    assert_eq!(
        frame_minimum_size(None, None, 8.0, 19.0, 0.0),
        Some(size(px(80.0), px(57.0)))
    );
    assert_eq!(
        frame_minimum_size(Some(1), None, 7.25, 18.5, 6.0),
        Some(size(px(79.0), px(62.0)))
    );
    assert_eq!(
        frame_minimum_size(None, Some(2), 7.25, 18.5, 6.0),
        Some(size(px(79.0), px(62.0)))
    );
    assert_eq!(
        frame_minimum_size(Some(20), Some(2), 7.25, 18.5, 6.0),
        Some(size(px(151.0), px(62.0)))
    );
}

#[test]
fn frame_size_parameters_resolve_workarea_ratios_and_text_pixels() {
    assert_eq!(
        frame_size_dimension(
            Some(FrameSizeSpec::Ratio(0.5)),
            px(320.0),
            Some(px(1600.0)),
            px(4.0),
        ),
        px(804.0)
    );
    assert_eq!(
        frame_size_dimension(
            Some(FrameSizeSpec::TextPixels(720)),
            px(320.0),
            Some(px(1600.0)),
            px(4.0),
        ),
        px(724.0)
    );
    assert_eq!(
        frame_size_dimension(
            Some(FrameSizeSpec::Ratio(0.5)),
            px(320.0),
            None,
            px(4.0),
        ),
        px(320.0)
    );
}

#[test]
fn native_frame_options_keep_the_viewport_at_least_three_rows_high() {
    let frame = super::logic::FrameSnapshot {
        frame_id: 1,
        primary_frame_id: Some(1),
        image_cache_entries: Arc::new(Mutex::new(HashMap::new())),
        image_cache_size: Arc::new(AtomicUsize::new(0)),
        name: "F1".to_owned(),
        icon_name: None,
        parent_frame: None,
        visibility: remacs_runtime::frontend::FrameVisibility::Visible,
        modifier_mapping: super::keys::ModifierMapping::default(),
        alpha_active: 255,
        alpha_inactive: 255,
        alpha_background: 255,
        width: 40,
        height: 12,
        width_size: None,
        height_size: None,
        workarea_size: None,
        internal_border_width: 0,
        left: None,
        top: None,
        user_position: false,
        user_size: false,
        override_redirect: false,
        keep_ratio: false,
        undecorated: false,
        ns_transparent_titlebar: false,
        ns_appearance: None,
        represented_file: None,
        fullscreen: None,
        auto_raise: false,
        auto_lower: false,
        z_group: None,
        no_accept_focus: false,
        no_focus_on_map: false,
        skip_taskbar: false,
        sticky: false,
        shaded: false,
        min_width: None,
        min_height: None,
        drag_with_header_line: false,
        drag_with_mode_line: false,
        drag_internal_border: false,
        grid: remacs_runtime::frontend::Grid::blank(40, 12),
    };
    let app = TestAppContext::single();
    let mut async_app = app.to_async();

    let options = frame_window_options(&frame, &HashMap::new(), &mut async_app);

    assert_eq!(options.window_min_size, Some(size(px(80.0), px(57.0))));

    let mut pixel_frame = frame.clone();
    pixel_frame.width_size = Some(FrameSizeSpec::TextPixels(720));
    pixel_frame.height_size = Some(FrameSizeSpec::TextPixels(480));
    let pixel_options = frame_window_options(&pixel_frame, &HashMap::new(), &mut async_app);
    let WindowBounds::Windowed(bounds) = pixel_options.window_bounds.unwrap() else {
        panic!("text-pixel size should open a windowed frame");
    };
    assert_eq!(bounds.size, size(px(720.0), px(480.0)));

    let mut ratio_frame = frame.clone();
    ratio_frame.width_size = Some(FrameSizeSpec::Ratio(0.5));
    ratio_frame.height_size = Some(FrameSizeSpec::Ratio(0.5));
    ratio_frame.workarea_size = Some((1600, 900));
    let ratio_options = frame_window_options(&ratio_frame, &HashMap::new(), &mut async_app);
    let WindowBounds::Windowed(bounds) = ratio_options.window_bounds.unwrap() else {
        panic!("ratio size should open a windowed frame");
    };
    assert_eq!(bounds.size, size(px(800.0), px(450.0)));
}

#[test]
fn minimum_size_tracks_measured_fonts_borders_and_changed_parameters() {
    let mut app = TestAppContext::single();
    let (tx, _rx) = mpsc::channel();
    let handle = app.add_window(|_, cx| EditorView::new(tx, cx));
    handle
        .update(&mut app, |view, window, _| {
            view.set_internal_border_width(3);
            let minimum = view.update_minimum_size(window, px(9.25), px(19.0));
            assert_eq!(minimum, size(px(99.0), px(63.0)));
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
    app.simulate_window_resize(handle.into(), size(px(1.0), px(50.0)));
    handle
        .update(&mut app, |_, window, _| {
            assert_eq!(window.viewport_size(), size(px(151.0), px(78.0)));
        })
        .unwrap();
}
