use super::EditorView;
use remacs_runtime::frontend::Grid;
use std::sync::mpsc;

#[test]
fn native_titles_apply_to_minimized_windows_without_a_render_callback() {
    let mut app = gpui::TestAppContext::single();
    let (tx, _rx) = mpsc::channel();
    let first = app.add_window(|_, cx| EditorView::new(tx.clone(), cx));
    let second = app.add_window(|_, cx| EditorView::new(tx, cx));
    let mut visual = gpui::VisualTestContext::from_window(first.into(), &app);
    let mut other = gpui::VisualTestContext::from_window(second.into(), &app);
    second
        .update(&mut app, |view, window, _| {
            let mut grid = Grid::blank(40, 8);
            grid.title = "other frame".into();
            view.set_grid(grid, window);
        })
        .unwrap();
    for (title, minimized) in [
        ("icon:日本語", true),
        ("new icon label", true),
        ("normal title", false),
    ] {
        first
            .update(&mut app, |view, window, _| {
                if minimized {
                    window.minimize_window();
                } else {
                    window.set_visible(true);
                }
                let mut grid = Grid::blank(40, 8);
                grid.title = title.into();
                // Intentionally do not notify or run the renderer: snapshots
                // must update native titles even when a window is minimized.
                view.set_grid(grid, window);
                assert_eq!(window.is_visible(), !minimized);
            })
            .unwrap();
        assert_eq!(visual.window_title().as_deref(), Some(title));
        assert_eq!(other.window_title().as_deref(), Some("other frame"));
    }
}
