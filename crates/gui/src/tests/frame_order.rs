use super::super::raise_frame_window;
use gpui::{Context, IntoElement, Render, TestAppContext, Window, div};

struct FrameOrderView;

impl Render for FrameOrderView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
    }
}

#[test]
fn initial_visible_frame_becomes_active_only_when_mapping_allows_focus() {
    use super::super::apply_initial_frame_visibility;
    use remacs_runtime::frontend::FrameVisibility;
    for (no_focus_on_map, no_accept_focus) in
        [(false, false), (true, false), (false, true), (true, true)]
    {
        let mut app = gpui::TestAppContext::single();
        let source = app.add_window(|_, _| FrameOrderView);
        let target = app.add_window(|_, _| FrameOrderView);
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
    // Regression for #2267: raise-frame changes stacking order without changing focus.
    let mut app = TestAppContext::single();
    let source = app.add_window(|_, _| FrameOrderView);
    let target = app.add_window(|_, _| FrameOrderView);
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
