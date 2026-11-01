use super::super::{EditorView, raise_frame_window};
use std::sync::mpsc;

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
