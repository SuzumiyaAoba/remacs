use gpui::Window;
use remacs_runtime::frontend::FrameVisibility;

pub(super) fn apply_frame_visibility(
    window: &Window,
    visibility: FrameVisibility,
    focus_on_map: bool,
) {
    match visibility {
        FrameVisibility::Hidden => window.set_visible(false),
        FrameVisibility::Iconified => window.minimize_window(),
        FrameVisibility::Visible => show_frame_window(window, focus_on_map),
    }
}

pub(super) fn apply_initial_frame_visibility(
    window: &Window,
    visibility: FrameVisibility,
    focus_on_map: bool,
) {
    // A secondary native window is already visible after orderFront:, but
    // still needs activation after its no-accept-focus hooks are installed.
    if visibility != FrameVisibility::Visible || !window.is_visible() || focus_on_map {
        apply_frame_visibility(window, visibility, focus_on_map);
    }
}

pub(super) fn show_frame_window(window: &Window, focus_on_map: bool) {
    window.set_visible(true);
    if focus_on_map {
        window.activate_window();
    }
}
