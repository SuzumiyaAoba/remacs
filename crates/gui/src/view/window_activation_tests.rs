use super::{FrameActivationUpdate, frame_activation_update};

#[test]
fn initially_inactive_window_does_not_lower_or_emit_focus_out() {
    assert_eq!(
        frame_activation_update(None, false, false, true),
        FrameActivationUpdate::default()
    );
}

#[test]
fn actual_focus_loss_lowers_and_emits_focus_out() {
    assert_eq!(
        frame_activation_update(Some(true), false, false, true),
        FrameActivationUpdate {
            raise: false,
            lower: true,
            focus: Some(false),
        }
    );
}

#[test]
fn an_active_transition_keeps_auto_raise_and_focus_in() {
    assert_eq!(
        frame_activation_update(Some(false), true, true, false),
        FrameActivationUpdate {
            raise: true,
            lower: false,
            focus: Some(true),
        }
    );
}
