use remacs_runtime::{
    frontend::{PopupMenu, PopupMenuAnchor, PopupMenuEntry, PopupMenuPosition, render_grid},
    lisp::{Interp, Value},
};

#[test]
fn popup_item_ranges_identify_the_composed_unicode_text() {
    let mut i = Interp::new();
    i.eval_str(
        r#"(progn
      (set-frame-parameter nil 'window-system 'ns)
      (set-frame-parameter nil 'menu-bar-lines 0)
      (set-frame-parameter nil 'tool-bar-lines 0)
      (set-frame-parameter nil 'tab-bar-lines 0)
      (erase-buffer) (insert "漢漢漢漢漢漢漢漢漢漢\n漢漢漢漢漢漢漢漢漢漢\n") (goto-char 1)
      (set-face-attribute 'menu nil :width 'expanded))"#,
    )
    .unwrap();
    let frame_id = i.selected_frame.as_ref().unwrap().borrow().id;
    i.gui_popup_menu = Some(
        PopupMenu {
            id: 29,
            position: PopupMenuPosition {
                frame_id,
                anchor: PopupMenuAnchor::Frame,
                from_event: false,
                x: 24,
                y: 0,
            },
            entries: vec![
                PopupMenuEntry {
                    label: "e\u{301}漢WW".into(),
                    command: None,
                    separator: false,
                    depth: 0,
                    value: Some(Value::Int(1)),
                },
                PopupMenuEntry {
                    label: "Second".into(),
                    command: None,
                    separator: false,
                    depth: 1,
                    value: Some(Value::Int(2)),
                },
            ],
        }
        .into(),
    );
    let grid = render_grid(&i, 40, 12);
    assert_eq!(grid.popup_menu_items.len(), 2);
    for (item, expected) in grid.popup_menu_items.iter().zip(["e\u{301}漢WW", "Second"]) {
        let (start, end) = item.byte_range.expect("composed item byte range");
        assert_eq!(grid.rows[item.row][start..end].trim(), expected);
        assert!(end > start);
        assert_eq!(item.menu_id, 29);
    }
}
