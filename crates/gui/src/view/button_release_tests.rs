use super::{EditorView, GuiEvent};
use gpui::{Modifiers, MouseButton, point, px};
use remacs_runtime::frontend::{Grid, GridPopupMenuItem, GridTabBarItem, GridToolBarItem};
use std::sync::mpsc;

#[derive(Clone, Copy, Debug)]
enum Kind {
    Tool,
    Tab,
    Popup,
}

fn grid(kind: Kind) -> Grid {
    let mut grid = Grid::blank(40, 8);
    grid.rows = vec![" ".repeat(40); 8];
    grid.rows[1] = "    First  Second                       ".into();
    for (index, (left, end)) in [(4, 9), (11, 17)].into_iter().enumerate() {
        match kind {
            Kind::Tool => grid.tool_bar_items.push(GridToolBarItem {
                row: 1,
                left,
                width: end - left,
                byte_range: Some((left, end)),
                command: "same-tool-command".into(),
                command_id: None,
            }),
            Kind::Tab => grid.tab_bar_items.push(GridTabBarItem {
                row: 1,
                left,
                width: end - left,
                byte_range: Some((left, end)),
                command: "tab-bar-select-tab".into(),
                argument: Some(index as i128 + 1),
            }),
            Kind::Popup => grid.popup_menu_items.push(GridPopupMenuItem {
                menu_id: 51,
                parent: None,
                selection: Some(index),
                row: 1,
                left,
                width: end - left,
                byte_range: Some((left, end)),
                command: None,
            }),
        }
    }
    grid
}

fn activations(events: impl IntoIterator<Item = GuiEvent>) -> Vec<String> {
    events
        .into_iter()
        .filter_map(|event| match event {
            GuiEvent::MenuCommand(command) => Some(command),
            GuiEvent::CommandArgs { command, args } => Some(format!("{command} {args:?}")),
            GuiEvent::PopupMenuSelection {
                menu_id,
                index: Some(index),
            } => Some(format!("popup {menu_id} {index}")),
            GuiEvent::MouseDown { .. }
            | GuiEvent::MouseUp { .. }
            | GuiEvent::MouseDragMove { .. } => {
                panic!("button gesture leaked to the buffer")
            }
            _ => None,
        })
        .collect()
}

#[test]
fn buttons_wait_for_release_and_cancel_outside_the_pressed_item() {
    for kind in [Kind::Tool, Kind::Tab, Kind::Popup] {
        let mut app = gpui::TestAppContext::single();
        let (tx, rx) = mpsc::channel();
        let handle = app.add_window(|_, cx| EditorView::new(tx, cx));
        handle
            .update(&mut app, |view, window, cx| {
                window.activate_window();
                view.set_internal_border_width(3);
                view.grid = grid(kind);
                cx.notify();
            })
            .unwrap();
        app.run_until_parked();
        rx.try_iter().for_each(drop);
        let points = handle
            .update(&mut app, |view, _, _| {
                let bounds = match kind {
                    Kind::Tool => view
                        .tool_hit_regions
                        .borrow()
                        .iter()
                        .map(|item| item.bounds)
                        .collect::<Vec<_>>(),
                    Kind::Tab => view
                        .tab_hit_regions
                        .borrow()
                        .iter()
                        .map(|item| item.bounds)
                        .collect::<Vec<_>>(),
                    Kind::Popup => view
                        .popup_hit_regions
                        .borrow()
                        .iter()
                        .map(|item| item.bounds)
                        .collect::<Vec<_>>(),
                };
                [bounds[0].center(), bounds[1].center()]
                    .map(|location| location + point(px(3.0), px(3.0)))
            })
            .unwrap();
        let mut visual = gpui::VisualTestContext::from_window(handle.into(), &app);
        let outside = point(px(2.0), points[0].y + px(50.0));
        // Adjacent buttons may invoke the same command. The pressed item itself
        // must match the release, rather than just its command string.
        for release in [points[1], outside] {
            visual.simulate_mouse_down(points[0], MouseButton::Left, Modifiers::none());
            assert!(
                activations(rx.try_iter()).is_empty(),
                "{kind:?} activated on press"
            );
            visual.simulate_mouse_move(release, Some(MouseButton::Left), Modifiers::none());
            visual.simulate_mouse_up(release, MouseButton::Left, Modifiers::none());
            let events = rx.try_iter().collect::<Vec<_>>();
            if matches!(kind, Kind::Popup) {
                assert_eq!(
                    events
                        .iter()
                        .filter(|event| matches!(
                            event,
                            GuiEvent::PopupMenuSelection {
                                menu_id: 51,
                                index: None
                            }
                        ))
                        .count(),
                    1,
                    "a canceled popup drag must release its waiting caller"
                );
            }
            assert!(
                activations(events).is_empty(),
                "{kind:?} activated after dragging away"
            );
        }
        visual.simulate_mouse_down(points[0], MouseButton::Left, Modifiers::none());
        assert!(activations(rx.try_iter()).is_empty());
        visual.simulate_mouse_move(outside, Some(MouseButton::Left), Modifiers::none());
        visual.simulate_mouse_move(points[0], Some(MouseButton::Left), Modifiers::none());
        visual.simulate_mouse_up(points[0], MouseButton::Left, Modifiers::none());
        let expected = match kind {
            Kind::Tool => "same-tool-command",
            Kind::Tab => "tab-bar-select-tab [1]",
            Kind::Popup => "popup 51 0",
        };
        assert_eq!(activations(rx.try_iter()), [expected], "{kind:?}");
    }
}

#[test]
fn button_release_uses_the_current_item_and_drops_stale_presses() {
    for kind in [Kind::Tool, Kind::Tab, Kind::Popup] {
        let mut app = gpui::TestAppContext::single();
        let (tx, rx) = mpsc::channel();
        let handle = app.add_window(|_, cx| EditorView::new(tx, cx));
        handle
            .update(&mut app, |view, window, cx| {
                window.activate_window();
                view.set_internal_border_width(0);
                view.grid = grid(kind);
                cx.notify();
            })
            .unwrap();
        app.run_until_parked();
        rx.try_iter().for_each(drop);
        let position = handle
            .update(&mut app, |view, _, _| match kind {
                Kind::Tool => view.tool_hit_regions.borrow()[0].bounds.center(),
                Kind::Tab => view.tab_hit_regions.borrow()[0].bounds.center(),
                Kind::Popup => view.popup_hit_regions.borrow()[0].bounds.center(),
            })
            .unwrap();
        let mut visual = gpui::VisualTestContext::from_window(handle.into(), &app);
        for changed in [false, true] {
            visual.simulate_mouse_down(position, MouseButton::Left, Modifiers::none());
            assert!(activations(rx.try_iter()).is_empty());
            handle
                .update(&mut app, |view, _, cx| {
                    let mut next = grid(kind);
                    if changed {
                        match kind {
                            Kind::Tool => next.tool_bar_items[0].command = "replacement".into(),
                            Kind::Tab => next.tab_bar_items[0].argument = Some(99),
                            Kind::Popup => {
                                for item in &mut next.popup_menu_items {
                                    item.menu_id += 1;
                                }
                            }
                        }
                    }
                    view.grid = next;
                    cx.notify();
                })
                .unwrap();
            app.run_until_parked();
            visual.simulate_mouse_up(position, MouseButton::Left, Modifiers::none());
            assert_eq!(
                activations(rx.try_iter()).len(),
                usize::from(!changed),
                "redraw: {kind:?}"
            );
        }
        // A release outside the whole editor must clear the capture too. A
        // later stray release over the original item cannot complete it.
        visual.simulate_mouse_down(position, MouseButton::Left, Modifiers::none());
        assert!(activations(rx.try_iter()).is_empty());
        visual.simulate_mouse_up(
            point(px(-10.0), px(-10.0)),
            MouseButton::Left,
            Modifiers::none(),
        );
        visual.simulate_mouse_up(position, MouseButton::Left, Modifiers::none());
        assert!(
            activations(rx.try_iter()).is_empty(),
            "outside release: {kind:?}"
        );

        visual.simulate_mouse_down(position, MouseButton::Left, Modifiers::none());
        assert!(activations(rx.try_iter()).is_empty());
        visual.deactivate_window();
        handle
            .update(&mut app, |_, window, cx| {
                window.activate_window();
                cx.notify();
            })
            .unwrap();
        app.run_until_parked();
        visual.simulate_mouse_up(position, MouseButton::Left, Modifiers::none());
        assert!(
            activations(rx.try_iter()).is_empty(),
            "focus change: {kind:?}"
        );
    }
}
