use super::{EditorView, GuiEvent};
use crate::logic::ChanIo;
use gpui::{Modifiers, ScrollDelta, ScrollWheelEvent, point};
use remacs_runtime::frontend::{Grid, GridPopupMenuItem, InputEvent, KeyIo};
use std::{sync::mpsc, time::Duration};

#[test]
fn popup_hover_and_wheel_preserve_the_painted_submenu_column() {
    let mut app = gpui::TestAppContext::single();
    let (tx, rx) = mpsc::channel();
    let handle = app.add_window(|_, cx| EditorView::new(tx, cx));
    handle
        .update(&mut app, |view, window, cx| {
            window.activate_window();
            view.set_internal_border_width(0);
            view.grid = Grid::blank(48, 6);
            view.grid.rows = vec![" ".repeat(48); 6];
            view.grid.rows[1] = "    More            First                       ".into();
            view.grid.popup_menu_items = vec![
                GridPopupMenuItem {
                    menu_id: 41,
                    parent: None,
                    selection: Some(2),
                    row: 1,
                    left: 4,
                    width: 4,
                    byte_range: Some((4, 8)),
                    command: None,
                },
                GridPopupMenuItem {
                    menu_id: 41,
                    parent: Some(2),
                    selection: Some(3),
                    row: 1,
                    left: 20,
                    width: 5,
                    byte_range: Some((20, 25)),
                    command: None,
                },
            ];
            cx.notify();
        })
        .unwrap();
    app.run_until_parked();
    rx.try_iter().for_each(drop);
    let regions = handle
        .update(&mut app, |view, _, _| {
            view.popup_hit_regions.borrow().clone()
        })
        .unwrap();
    let mut visual = gpui::VisualTestContext::from_window(handle.into(), &app);
    let (bridge_tx, bridge_rx) = mpsc::channel();
    for (region, parent, index) in [(&regions[0], None, 2), (&regions[1], Some(2), 3)] {
        visual.simulate_mouse_move(region.bounds.center(), None, Modifiers::none());
        let hovers = rx
            .try_iter()
            .filter(|event| matches!(event, GuiEvent::PopupMenuHover { .. }))
            .collect::<Vec<_>>();
        assert_eq!(hovers.len(), 1);
        let event = hovers.into_iter().next().unwrap();
        assert!(
            matches!(&event, GuiEvent::PopupMenuHover { menu_id: 41, parent: actual, index: Some(actual_index) }
            if *actual == parent && *actual_index == index)
        );
        bridge_tx.send(event).unwrap();
    }
    for region in [&regions[1], &regions[0]] {
        visual.simulate_event(ScrollWheelEvent {
            position: region.bounds.center(),
            delta: ScrollDelta::Lines(point(0.0, -2.0)),
            ..Default::default()
        });
        let events = rx
            .try_iter()
            .filter(|event| {
                matches!(
                    event,
                    GuiEvent::PopupMenuScroll { .. } | GuiEvent::PopupMenuScrollAt { .. }
                )
            })
            .collect::<Vec<_>>();
        assert_eq!(events.len(), 1);
        bridge_tx.send(events.into_iter().next().unwrap()).unwrap();
    }
    let (grids, _receive_grids) = smol::channel::bounded(1);
    let mut io = ChanIo::new_for_test(bridge_rx, grids);
    assert!(matches!(
        io.poll_key_or_mouse(Duration::ZERO).unwrap(),
        Some(InputEvent::PopupMenuHover {
            parent: None,
            index: Some(2),
            ..
        })
    ));
    assert!(matches!(
        io.poll_input(Duration::ZERO).unwrap(),
        Some(InputEvent::PopupMenuHover {
            parent: Some(2),
            index: Some(3),
            ..
        })
    ));
    assert!(
        matches!(io.poll_key_or_mouse(Duration::ZERO).unwrap(), Some(InputEvent::PopupMenuScrollAt { parent: Some(2), delta_lines, .. }) if delta_lines == 2.0)
    );
    assert!(
        matches!(io.poll_input(Duration::ZERO).unwrap(), Some(InputEvent::PopupMenuScroll { delta_lines, .. }) if delta_lines == 2.0)
    );
}
