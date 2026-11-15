use gpui::Window;
use remacs_runtime::frontend::FrameVisibility;

pub(super) fn native_visibility(is_visible: bool, is_minimized: Option<bool>) -> FrameVisibility {
    match is_minimized {
        Some(true) => FrameVisibility::Iconified,
        Some(false) => {
            if is_visible {
                FrameVisibility::Visible
            } else {
                FrameVisibility::Hidden
            }
        }
        None => {
            if is_visible {
                FrameVisibility::Visible
            } else {
                FrameVisibility::Iconified
            }
        }
    }
}

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

#[cfg(test)]
mod tests {
    use super::native_visibility;
    use remacs_runtime::frontend::FrameVisibility;

    #[test]
    fn native_window_state_distinguishes_hidden_from_iconified() {
        assert_eq!(
            native_visibility(false, Some(true)),
            FrameVisibility::Iconified
        );
        assert_eq!(
            native_visibility(false, Some(false)),
            FrameVisibility::Hidden
        );
        assert_eq!(
            native_visibility(true, Some(false)),
            FrameVisibility::Visible
        );
    }

    #[test]
    fn platforms_without_minimize_state_keep_the_legacy_nonvisible_mapping() {
        assert_eq!(
            native_visibility(false, None),
            FrameVisibility::Iconified
        );
    }
}
