use super::{EditorView, GuiEvent};
use gpui::{Modifiers, point, px};
use remacs_runtime::frontend::{CellStyle, Grid, GridTabBarItem};
use std::sync::mpsc;

#[test]
fn adjacent_tab_clicks_follow_native_face_glyph_boundaries() {
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
            grid.char_attrs[0] = grid.rows[0]
                .chars()
                .enumerate()
                .map(|(index, _)| {
                    if index < 6 {
                        CellStyle {
                            font_width: Some(125),
                            font_height: Some(280),
                            ..Default::default()
                        }
                    } else if (7..13).contains(&index) {
                        CellStyle {
                            font_width: Some(75),
                            font_height: Some(140),
                            ..Default::default()
                        }
                    } else {
                        CellStyle::default()
                    }
                })
                .collect();
            grid.tab_bar_items = vec![
                GridTabBarItem {
                    row: 0,
                    left: 0,
                    width: 6,
                    byte_range: Some((0, 6)),
                    command: "tab-bar-select-tab".into(),
                    argument: Some(1),
                },
                GridTabBarItem {
                    row: 0,
                    left: 7,
                    width: 6,
                    byte_range: Some((7, 13)),
                    command: "tab-bar-select-tab".into(),
                    argument: Some(2),
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
                view.tab_hit_regions.borrow().clone(),
                view.geometry.as_ref().unwrap().char_width as f32,
            )
        })
        .unwrap();
    assert_eq!(regions.len(), 2);
    assert!(
        f32::from(regions[0].bounds.right()) > 6.0 * cell + 2.0,
        "first tab must extend beyond its old cell hit box"
    );
    assert!(regions[1].bounds.left() > regions[0].bounds.right());
    let mut visual = gpui::VisualTestContext::from_window(handle.into(), &app);
    for (index, region) in regions.iter().enumerate() {
        visual.simulate_click(
            point(
                px(3.0) + region.bounds.right() - px(0.25),
                px(3.0) + region.bounds.size.height / 2.0,
            ),
            Modifiers::none(),
        );
        let events: Vec<_> = rx
            .try_iter()
            .filter_map(|event| match event {
                GuiEvent::CommandArgs { command, args } => Some((command, args)),
                _ => None,
            })
            .collect();
        assert_eq!(
            events,
            vec![("tab-bar-select-tab".into(), vec![index as i128 + 1])]
        );
    }
    let gap = (regions[0].bounds.right() + regions[1].bounds.left()) / 2.0;
    visual.simulate_click(
        point(px(3.0) + gap, px(3.0) + regions[0].bounds.size.height / 2.0),
        Modifiers::none(),
    );
    assert!(
        !rx.try_iter()
            .any(|event| matches!(event, GuiEvent::CommandArgs { .. }))
    );
}
