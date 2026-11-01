use super::{EditorView, GuiEvent};
use crate::logic::ChanIo;
use gpui::{ScrollDelta, ScrollWheelEvent, point, px};
use remacs_runtime::frontend::{Grid, GridPopupMenuItem, InputEvent, KeyIo};
use std::{sync::mpsc, time::Duration};

#[test]
fn popup_wheel_motion_reaches_both_input_paths_without_buffer_wheel_events() {
    let mut app = gpui::TestAppContext::single();
    let (tx, rx) = mpsc::channel();
    let handle = app.add_window(|_, cx| EditorView::new(tx, cx));
    handle
        .update(&mut app, |view, window, cx| {
            window.activate_window();
            view.grid = Grid::blank(32, 6);
            view.grid.rows = vec![" ".repeat(32); 6];
            view.grid.rows[1] = "    Item                        ".into();
            view.grid.popup_menu_items = vec![GridPopupMenuItem {
                parent: None,
                menu_id: 41,
                selection: Some(37),
                row: 1,
                left: 4,
                width: 4,
                byte_range: Some((4, 8)),
                command: None,
            }];
            cx.notify();
        })
        .unwrap();
    app.run_until_parked();
    rx.try_iter().for_each(drop);
    let mut visual = gpui::VisualTestContext::from_window(handle.into(), &app);
    // Popup wheel input must work independently of the underlying text grid,
    // including when the pointer is outside the item or on a caption/border.
    for (position, lines) in [
        (point(px(40.0), px(20.0)), -3.5),
        (point(px(200.0), px(80.0)), 1.25),
    ] {
        visual.simulate_event(ScrollWheelEvent {
            position,
            delta: ScrollDelta::Lines(point(0.0, lines)),
            ..Default::default()
        });
    }
    let bounds = handle
        .update(&mut app, |view, _, _| {
            view.popup_hit_regions.borrow()[0].bounds
        })
        .unwrap();
    visual.simulate_click(bounds.center(), Default::default());
    let events: Vec<_> = rx.try_iter().collect();
    assert!(
        !events
            .iter()
            .any(|event| matches!(event, GuiEvent::Scroll { .. }))
    );
    let (bridge_tx, bridge_rx) = mpsc::channel();
    let mut count = 0;
    for event in events {
        if matches!(
            event,
            GuiEvent::PopupMenuScroll { .. } | GuiEvent::PopupMenuSelection { .. }
        ) {
            if matches!(event, GuiEvent::PopupMenuScroll { .. }) {
                count += 1;
            }
            bridge_tx.send(event).unwrap();
        }
    }
    assert_eq!(count, 2);
    let (grid_tx, _grid_rx) = smol::channel::bounded(1);
    let mut io = ChanIo::new_for_test(bridge_rx, grid_tx);
    for (event, expected) in [
        (io.poll_key_or_mouse(Duration::ZERO).unwrap(), 3.5),
        (io.poll_input(Duration::ZERO).unwrap(), -1.25),
    ] {
        let Some(InputEvent::PopupMenuScroll {
            menu_id,
            delta_lines,
        }) = event
        else {
            panic!("popup wheel event");
        };
        assert_eq!(menu_id, 41);
        assert_eq!(delta_lines, expected);
    }
    assert!(matches!(
        io.poll_key_or_mouse(Duration::ZERO).unwrap(),
        Some(InputEvent::PopupMenuSelection {
            menu_id: 41,
            index: Some(37)
        })
    ));
}
