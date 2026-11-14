use super::{EditorView, TooltipPlacement, tooltip_placement};
use gpui::{TestAppContext, point, px, size};
use remacs_runtime::frontend::{
    CellColor, CellStyle, Grid, GridTooltip, GridTooltipPosition, GridTooltipRun,
};
use std::sync::mpsc;

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

#[test]
fn explicit_tooltip_renders_styled_text_runs() {
    let mut app = TestAppContext::single_with_native_text_system();
    let (tx, _rx) = mpsc::channel();
    let handle = app.add_window(|_, cx| EditorView::new(tx, cx));
    handle
        .update(&mut app, |view, window, cx| {
            let mut grid = Grid::blank(40, 6);
            grid.tooltip = Some(GridTooltip {
                text: "styled tip".to_owned(),
                style: CellStyle {
                    fg: Some(CellColor(0, 0, 255)),
                    bg: Some(CellColor(255, 255, 0)),
                    ..Default::default()
                },
                font_families: Vec::new(),
                runs: vec![GridTooltipRun {
                    len: "styled tip".len(),
                    style: CellStyle {
                        fg: Some(CellColor(255, 0, 0)),
                        bg: Some(CellColor(255, 255, 0)),
                        bold: true,
                        ..Default::default()
                    },
                }],
            });
            grid.tooltip_position = Some(GridTooltipPosition::default());
            view.set_grid(grid, window);
            cx.notify();
        })
        .unwrap();
    app.run_until_parked();

    let rendered_tooltip = handle
        .update(&mut app, |view, _, _| view.grid.tooltip.is_some())
        .unwrap();
    assert!(rendered_tooltip);
}
