use super::{EditorView, GuiEvent};
use gpui::{Modifiers, MouseButton, ScrollDelta, ScrollWheelEvent, point, px};
use remacs_runtime::{
    editor::CHAR_SHIFT,
    frontend::{CellStyle, Grid, GridTabBarItem, MouseArea, render_grid_with_input_cached},
    lisp::Interp,
};
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
                    command_id: None,
                    argument: Some(1),
                },
                GridTabBarItem {
                    row: 0,
                    left: 7,
                    width: 6,
                    byte_range: Some((7, 13)),
                    command: "tab-bar-select-tab".into(),
                    command_id: None,
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

#[test]
fn middle_click_on_tab_closes_that_tab() {
    let mut app = gpui::TestAppContext::single();
    let (tx, rx) = mpsc::channel();
    let handle = app.add_window(|_, cx| EditorView::new(tx, cx));
    handle
        .update(&mut app, |view, window, cx| {
            window.activate_window();
            view.set_internal_border_width(3);
            let mut grid = Grid::blank(32, 5);
            grid.rows = vec![" ".repeat(32); 5];
            grid.rows[0] = "Second tab                    ".into();
            grid.tab_bar_items.push(GridTabBarItem {
                row: 0,
                left: 0,
                width: 7,
                byte_range: Some((0, 7)),
                command: "tab-bar-select-tab".into(),
                command_id: None,
                argument: Some(2),
            });
            view.grid = grid;
            cx.notify();
        })
        .unwrap();
    app.run_until_parked();

    let bounds = handle
        .update(&mut app, |view, _, _| {
            view.tab_hit_regions.borrow()[0].bounds
        })
        .unwrap();
    let position = point(
        px(3.0) + bounds.right() - px(0.25),
        px(3.0) + bounds.size.height / 2.0,
    );
    let mut visual = gpui::VisualTestContext::from_window(handle.into(), &app);
    visual.simulate_mouse_down(position, MouseButton::Middle, Modifiers::none());
    visual.simulate_mouse_up(position, MouseButton::Middle, Modifiers::none());

    let commands: Vec<_> = rx
        .try_iter()
        .filter_map(|event| match event {
            GuiEvent::CommandArgs { command, args } => Some((command, args)),
            _ => None,
        })
        .collect();
    assert_eq!(commands, vec![("tab-bar-close-tab".into(), vec![2])]);
}

#[test]
fn wheel_on_tab_dispatches_tab_navigation_and_shift_moves_tabs() {
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(wheel_on_tab_dispatches_tab_navigation_and_shift_moves_tabs_inner)
        .unwrap()
        .join()
        .unwrap();
}

fn wheel_on_tab_dispatches_tab_navigation_and_shift_moves_tabs_inner() {
    let mut interp = Interp::new();
    interp
        .eval_str(
            "(progn (set-frame-parameter nil 'window-system 'ns) \
             (set-frame-parameter nil 'menu-bar-lines 0) \
             (set-frame-parameter nil 'tool-bar-lines 0) \
             (set-frame-parameter nil 'tab-bar-lines 1))",
        )
        .unwrap();
    let mut grid = render_grid_with_input_cached(&interp, 32, 5, None);
    grid.rows[0] = "Second tab                    ".into();
    grid.tab_bar_items.clear();
    grid.tab_bar_items.push(GridTabBarItem {
        row: 0,
        left: 0,
        width: 7,
        byte_range: Some((0, 7)),
        command: "tab-bar-select-tab".into(),
        command_id: None,
        argument: Some(2),
    });

    let mut app = gpui::TestAppContext::single();
    let (tx, rx) = mpsc::channel();
    let handle = app.add_window(|_, cx| EditorView::new(tx, cx));
    handle
        .update(&mut app, move |view, window, cx| {
            window.activate_window();
            view.set_internal_border_width(3);
            view.grid = grid;
            cx.notify();
        })
        .unwrap();
    app.run_until_parked();

    let bounds = handle
        .update(&mut app, |view, _, _| {
            view.tab_hit_regions.borrow()[0].bounds
        })
        .unwrap();
    let position = point(
        px(3.0) + bounds.right() - px(0.25),
        px(3.0) + bounds.size.height / 2.0,
    );
    let mut visual = gpui::VisualTestContext::from_window(handle.into(), &app);
    for (delta, modifiers) in [
        (point(0.0, -1.0), Modifiers::none()),
        (point(0.0, 1.0), Modifiers::none()),
        (
            point(0.0, -1.0),
            Modifiers {
                shift: true,
                ..Default::default()
            },
        ),
        (
            point(0.0, 1.0),
            Modifiers {
                shift: true,
                ..Default::default()
            },
        ),
    ] {
        visual.simulate_event(ScrollWheelEvent {
            position,
            delta: ScrollDelta::Lines(delta),
            modifiers,
            ..Default::default()
        });
    }

    let events: Vec<_> = rx
        .try_iter()
        .filter_map(|event| match event {
            GuiEvent::Scroll {
                delta,
                horizontal,
                pixel_delta,
                modifiers,
                position,
            } => Some((delta, horizontal, pixel_delta, modifiers, position.area)),
            _ => None,
        })
        .collect();
    assert_eq!(events.len(), 4);
    assert_eq!(
        events.iter().map(|(delta, ..)| *delta).collect::<Vec<_>>(),
        vec![1.0, -1.0, 1.0, -1.0]
    );
    assert_eq!(
        events.iter().map(|(_, _, _, modifiers, _)| *modifiers).collect::<Vec<_>>(),
        vec![0, 0, CHAR_SHIFT, CHAR_SHIFT]
    );
    assert!(events
        .iter()
        .all(|(_, horizontal, pixels, _, area)| !horizontal
            && pixels.is_none()
            && *area == Some(MouseArea::TabBar)));
}
