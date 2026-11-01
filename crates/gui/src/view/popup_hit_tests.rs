use super::{EditorView, GuiEvent};
use gpui::{Modifiers, point, px};
use remacs_runtime::frontend::{CellStyle, Grid, GridPopupMenuItem};
use std::sync::mpsc;

#[test]
fn popup_click_uses_the_native_painted_glyph_extent() {
    let mut app = gpui::TestAppContext::single_with_native_text_system();
    let (tx, rx) = mpsc::channel();
    let handle = app.add_window(|_, cx| EditorView::new(tx, cx));
    handle
        .update(&mut app, |view, window, cx| {
            window.activate_window();
            view.set_internal_border_width(3);
            let mut grid = Grid::blank(32, 6);
            grid.rows = vec![" ".repeat(32); 6];
            grid.rows[1] = "    WWWWWW                      ".into();
            grid.rows[2] = "    e\u{301}漢字WW                      ".into();
            grid.default_style.font_height = Some(140);
            grid.row_heights = vec![19, 38, 38, 19, 19, 19];
            let menu = CellStyle {
                font_width: Some(125),
                font_height: Some(280),
                ..Default::default()
            };
            for row in [1, 2] {
                grid.char_attrs[row] = grid.rows[row]
                    .chars()
                    .enumerate()
                    .map(|(index, _)| {
                        if (4..10).contains(&index) {
                            menu
                        } else {
                            CellStyle::default()
                        }
                    })
                    .collect();
            }
            grid.popup_menu_items = vec![
                GridPopupMenuItem {
                    menu_id: 41,
                    selection: Some(0),
                    row: 1,
                    left: 4,
                    width: 6,
                    byte_range: Some((4, 10)),
                    command: None,
                },
                GridPopupMenuItem {
                    menu_id: 41,
                    selection: Some(1),
                    row: 2,
                    left: 4,
                    width: 7,
                    byte_range: Some((4, "    e\u{301}漢字WW".len())),
                    command: None,
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
                view.popup_hit_regions.borrow().clone(),
                view.geometry.as_ref().unwrap().char_width as f32,
            )
        })
        .unwrap();
    assert_eq!(regions.len(), 2);
    assert!(
        f32::from(regions[0].bounds.right()) > 10.0 * cell + 2.0,
        "fixture must extend beyond the old standard-cell hit box"
    );
    let mut visual = gpui::VisualTestContext::from_window(handle.into(), &app);
    for (index, region) in regions.iter().enumerate() {
        let position = point(
            px(3.0) + region.bounds.right() - px(0.25),
            px(3.0) + region.bounds.top() + region.bounds.size.height / 2.0,
        );
        visual.simulate_click(position, Modifiers::none());
        let events: Vec<_> = rx
            .try_iter()
            .filter_map(|event| match event {
                GuiEvent::PopupMenuSelection { menu_id, index } => Some((menu_id, index)),
                _ => None,
            })
            .collect();
        assert_eq!(events, vec![(41, Some(index))]);
    }
    visual.simulate_click(
        point(
            px(3.0) + regions[0].bounds.right() + px(1.0),
            px(3.0) + regions[0].bounds.top() + px(1.0),
        ),
        Modifiers::none(),
    );
    assert!(rx.try_iter().any(|event| matches!(
        event,
        GuiEvent::PopupMenuSelection {
            menu_id: 41,
            index: None
        }
    )));
}
