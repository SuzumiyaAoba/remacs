use super::super::{apply_initial_frame_visibility, frame_snapshot_visibility};
use gpui::{Context, IntoElement, Render, TestAppContext, Window, div};
use remacs_runtime::frontend::FrameVisibility;
use std::collections::HashMap;

struct VisibilityView;

impl Render for VisibilityView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
    }
}

#[test]
fn pending_iconify_request_overrides_initial_visible_snapshot() {
    let mut app = TestAppContext::single();
    let handle = app.add_window(|_, _| VisibilityView);
    let frame_id = 2274;
    let mut pending = HashMap::from([(frame_id, FrameVisibility::Iconified)]);
    let visibility = frame_snapshot_visibility(
        frame_id,
        FrameVisibility::Visible,
        &pending,
    );

    assert_eq!(visibility, FrameVisibility::Iconified);
    assert_eq!(pending.get(&frame_id), Some(&FrameVisibility::Iconified));
    handle
        .update(&mut app, |_, window, _| {
            apply_initial_frame_visibility(window, visibility, false);
            assert!(!window.is_visible());
        })
        .unwrap();
    pending.remove(&frame_id);
    assert!(pending.is_empty());
}
