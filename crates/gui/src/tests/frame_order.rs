use super::super::{
    apply_pending_frame_order, lower_frame_window, raise_frame_window, request_frame_order,
};
use gpui::{Context, IntoElement, Render, TestAppContext, Window, WindowHandle, div};
use std::collections::HashMap;

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

#[test]
fn lowering_another_frame_preserves_keyboard_focus() {
    // Regression for #2268: lower-frame must not activate its target.
    let mut app = TestAppContext::single();
    let source = app.add_window(|_, _| FrameOrderView);
    let target = app.add_window(|_, _| FrameOrderView);
    source
        .update(&mut app, |_, window, _| window.activate_window())
        .unwrap();
    app.run_until_parked();

    target
        .update(&mut app, |_, window, _| {
            lower_frame_window(window);
            assert!(window.is_visible());
        })
        .unwrap();
    app.run_until_parked();

    assert!(source
        .update(&mut app, |_, window, _| window.is_window_active())
        .unwrap());
    assert!(!target
        .update(&mut app, |_, window, _| window.is_window_active())
        .unwrap());
}

#[test]
fn setting_always_on_top_preserves_keyboard_focus() {
    // Regression for #2269: changing z-group must not activate the target frame.
    let mut app = TestAppContext::single();
    let source = app.add_window(|_, _| FrameOrderView);
    let target = app.add_window(|_, _| FrameOrderView);
    source
        .update(&mut app, |_, window, _| window.activate_window())
        .unwrap();
    app.run_until_parked();

    target
        .update(&mut app, |_, window, _| window.set_always_on_top(true))
        .unwrap();
    app.run_until_parked();

    assert!(source
        .update(&mut app, |_, window, _| window.is_window_active())
        .unwrap());
    assert!(!target
        .update(&mut app, |_, window, _| window.is_window_active())
        .unwrap());
}

#[test]
fn lower_frame_order_requested_before_window_creation_is_applied() {
    // Regression for #2273: a lower request survives until the native window exists.
    let mut app = TestAppContext::single();
    let source = app.add_window(|_, _| FrameOrderView);
    source
        .update(&mut app, |_, window, _| window.activate_window())
        .unwrap();
    app.run_until_parked();

    let frame_id = 2273;
    let windows = HashMap::<usize, WindowHandle<FrameOrderView>>::new();
    let mut pending = HashMap::new();
    let mut async_app = app.update(|app| app.to_async());
    request_frame_order(frame_id, false, &windows, &mut pending, &mut async_app);
    assert_eq!(pending.get(&frame_id), Some(&false));

    let target = app.add_window(|_, _| FrameOrderView);
    source
        .update(&mut app, |_, window, _| window.activate_window())
        .unwrap();
    app.run_until_parked();

    apply_pending_frame_order(frame_id, &target, &mut pending, &mut async_app);
    app.run_until_parked();
    assert!(pending.is_empty());
    assert!(source
        .update(&mut app, |_, window, _| window.is_window_active())
        .unwrap());
    assert!(!target
        .update(&mut app, |_, window, _| window.is_window_active())
        .unwrap());
}
