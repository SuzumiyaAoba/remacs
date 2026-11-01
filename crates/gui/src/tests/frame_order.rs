use super::super::{EditorView, raise_frame_window};
use std::sync::mpsc;

#[test]
fn initial_visible_frame_becomes_active_only_when_mapping_allows_focus() {
    use super::super::apply_initial_frame_visibility;
    use remacs_runtime::frontend::FrameVisibility;
    for (no_focus_on_map, no_accept_focus) in
        [(false, false), (true, false), (false, true), (true, true)]
    {
        let mut app = gpui::TestAppContext::single();
        let (tx, _rx) = mpsc::channel();
        let source = app.add_window(|_, cx| EditorView::new(tx.clone(), cx));
        let target = app.add_window(|_, cx| EditorView::new(tx, cx));
        source
            .update(&mut app, |_, window, _| window.activate_window())
            .unwrap();
        app.run_until_parked();
        let focus_on_map = !no_focus_on_map && !no_accept_focus;
        target
            .update(&mut app, |_, window, _| {
                // MacWindow::open uses orderFront for focus=false/show=true.
                assert!(window.is_visible());
                apply_initial_frame_visibility(window, FrameVisibility::Visible, focus_on_map);
            })
            .unwrap();
        app.run_until_parked();
        assert_eq!(
            source
                .update(&mut app, |_, window, _| window.is_window_active())
                .unwrap(),
            !focus_on_map
        );
        assert_eq!(
            target
                .update(&mut app, |_, window, _| window.is_window_active())
                .unwrap(),
            focus_on_map
        );
    }
}

#[test]
fn raising_another_frame_preserves_keyboard_focus_until_explicit_activation() {
    let mut app = gpui::TestAppContext::single();
    let (tx, _rx) = mpsc::channel();
    let source = app.add_window(|_, cx| EditorView::new(tx.clone(), cx));
    let target = app.add_window(|_, cx| EditorView::new(tx, cx));
    source
        .update(&mut app, |_, window, _| window.activate_window())
        .unwrap();
    app.run_until_parked();
    let focus = |app: &mut gpui::TestAppContext| {
        (
            source
                .update(app, |_, window, _| window.is_window_active())
                .unwrap(),
            target
                .update(app, |_, window, _| window.is_window_active())
                .unwrap(),
        )
    };
    assert_eq!(focus(&mut app), (true, false));
    // Both a visible and a hidden target must be raised without becoming key.
    for hide in [false, true] {
        target
            .update(&mut app, |_, window, _| {
                if hide {
                    window.set_visible(false);
                }
                raise_frame_window(window);
                assert!(window.is_visible());
            })
            .unwrap();
        app.run_until_parked();
        assert_eq!(focus(&mut app), (true, false));
    }
    // Explicit input-focus requests still activate the target normally.
    target
        .update(&mut app, |_, window, _| window.activate_window())
        .unwrap();
    app.run_until_parked();
    assert_eq!(focus(&mut app), (false, true));
}
