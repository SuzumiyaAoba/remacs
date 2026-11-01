use super::{EditorView, GuiEvent};
use gpui::{Modifiers, point, px};
use remacs_runtime::frontend::{CellStyle, Grid, GridToolBarItem};
use std::sync::mpsc;

#[test]
fn adjacent_tool_buttons_use_their_native_painted_glyph_bounds() {
    let mut app = gpui::TestAppContext::single_with_native_text_system();
    let (tx, rx) = mpsc::channel();
    let handle = app.add_window(|_, cx| EditorView::new(tx, cx));
    handle
        .update(&mut app, |view, window, cx| {
            window.activate_window();
            view.set_internal_border_width(3);
            let mut grid = Grid::blank(32, 5);
            grid.rows = vec![" ".repeat(32); 5];
            grid.rows[0] = "WWWWWW iiiiii                   ".into();
            grid.default_style.font_height = Some(140);
            grid.row_heights = vec![38, 19, 19, 19, 19];
            let tool = CellStyle {
                font_width: Some(125),
                font_height: Some(280),
                ..Default::default()
            };
            grid.char_attrs[0] = grid.rows[0]
                .chars()
                .enumerate()
                .map(|(index, _)| {
                    if index < 6 || (7..13).contains(&index) {
                        tool
                    } else {
                        CellStyle::default()
                    }
                })
                .collect();
            grid.tool_bar_items = vec![
                GridToolBarItem {
                    row: 0,
                    left: 0,
                    width: 6,
                    byte_range: Some((0, 6)),
                    command: "first-command".into(),
                    command_id: None,
                },
                GridToolBarItem {
                    row: 0,
                    left: 7,
                    width: 6,
                    byte_range: Some((7, 13)),
                    command: "second-command".into(),
                    command_id: None,
                },
            ];
            view.grid = grid;
            cx.notify();
        })
        .unwrap();
    app.run_until_parked();
    let (regions, cell) = handle
        .update(&mut app, |view, _, _| {
            (
                view.tool_hit_regions.borrow().clone(),
                view.geometry.as_ref().unwrap().char_width as f32,
            )
        })
        .unwrap();
    assert_eq!(regions.len(), 2);
    assert!(
        f32::from(regions[0].bounds.right()) > 6.0 * cell + 2.0,
        "first tool must extend beyond its old standard-cell hit box"
    );
    assert!(regions[1].bounds.left() > regions[0].bounds.right());
    let mut visual = gpui::VisualTestContext::from_window(handle.into(), &app);
    for (region, expected) in regions.iter().zip(["first-command", "second-command"]) {
        visual.simulate_click(
            point(
                px(3.0) + region.bounds.right() - px(0.25),
                px(3.0) + region.bounds.size.height / 2.0,
            ),
            Modifiers::none(),
        );
        let commands: Vec<_> = rx
            .try_iter()
            .filter_map(|event| match event {
                GuiEvent::MenuCommand(command) => Some(command),
                _ => None,
            })
            .collect();
        assert_eq!(commands, vec![expected]);
    }
    let gap = (regions[0].bounds.right() + regions[1].bounds.left()) / 2.0;
    visual.simulate_click(
        point(px(3.0) + gap, px(3.0) + regions[0].bounds.size.height / 2.0),
        Modifiers::none(),
    );
    assert!(
        !rx.try_iter()
            .any(|event| matches!(event, GuiEvent::MenuCommand(_)))
    );
}
