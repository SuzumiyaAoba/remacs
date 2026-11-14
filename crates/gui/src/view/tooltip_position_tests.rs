use super::{TooltipPlacement, tooltip_placement};
use gpui::{point, px, size};
use remacs_runtime::frontend::GridTooltipPosition;

#[test]
fn tip_position_uses_pointer_offsets_and_absolute_frame_edges() {
    let viewport = size(px(640.0), px(480.0));
    let pointer = point(px(100.0), px(120.0));
    assert_eq!(
        tooltip_placement(
            GridTooltipPosition {
                dx: 5,
                dy: -10,
                ..Default::default()
            },
            40,
            50,
            30,
            viewport,
            pointer,
        ),
        TooltipPlacement {
            left: Some(px(105.0)),
            right: None,
            top: Some(px(110.0)),
            bottom: None,
        }
    );

    assert_eq!(
        tooltip_placement(
            GridTooltipPosition {
                left: Some(200),
                top: Some(300),
                ..Default::default()
            },
            40,
            50,
            30,
            viewport,
            pointer,
        ),
        TooltipPlacement {
            left: Some(px(160.0)),
            right: None,
            top: Some(px(220.0)),
            bottom: None,
        }
    );

    assert_eq!(
        tooltip_placement(
            GridTooltipPosition {
                right: Some(600),
                bottom: Some(500),
                ..Default::default()
            },
            40,
            50,
            30,
            viewport,
            pointer,
        ),
        TooltipPlacement {
            left: None,
            right: Some(px(80.0)),
            top: None,
            bottom: Some(px(60.0)),
        }
    );
}
