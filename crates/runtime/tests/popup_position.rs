use remacs_runtime::{
    editor::FrameRef,
    frontend::render_frame_grid_with_input_cached,
    lisp::{Interp, Value},
};
use std::rc::Rc;

fn eval(interp: &mut Interp, source: &str) -> Value {
    interp
        .eval_str(source)
        .unwrap_or_else(|flow| panic!("{source}: {flow:?}"))
}

fn frame(interp: &mut Interp, source: &str) -> FrameRef {
    let Value::Frame(frame) = eval(interp, source) else {
        panic!("expected frame: {source}");
    };
    frame
}

fn text_origin(interp: &mut Interp, frame: &FrameRef, window: &str) -> (i128, i128) {
    let Value::Window(window) = eval(interp, window) else {
        panic!("expected window");
    };
    let id = window.borrow().id;
    let grid = render_frame_grid_with_input_cached(interp, frame, 80, 24, None);
    let input = grid.input.as_ref().unwrap();
    for row in 0..24 {
        for column in 0..80 {
            if input
                .hit_test_position(row, column, 0)
                .is_some_and(|position| position.window_id == Some(id))
            {
                return (column as i128 * 8, row as i128 * 10);
            }
        }
    }
    panic!("window has no rendered input region");
}

fn check(
    interp: &mut Interp,
    frames: &[FrameRef],
    position: &str,
    expected_frame: &FrameRef,
    expected_pixel: (i128, i128),
) {
    let selected = interp.selected_frame.clone().unwrap();
    eval(
        interp,
        &format!("(x-popup-menu {position} qa-position-menu)"),
    );
    for frame in frames {
        let grid = render_frame_grid_with_input_cached(interp, frame, 80, 24, None);
        if !Rc::ptr_eq(frame, expected_frame) {
            assert!(grid.popup_menu_items.is_empty(), "wrong frame: {position}");
            continue;
        }
        assert_eq!(grid.popup_menu_items.len(), 3, "position: {position}");
        let title = &grid.popup_menu_items[0];
        assert!(title.selection.is_none());
        assert!(grid.popup_menu_items[1].selection.is_none());
        let item = &grid.popup_menu_items[2];
        assert_eq!(item.command.as_deref(), Some("ignore"));
        // The title and its separator occupy two additional rows. Keep the
        // whole menu inside the area above the one-row minibuffer.
        let top = (expected_pixel.1.max(0) as usize / 10).min(23 - 5);
        assert_eq!(title.row, top + 1);
        assert_eq!(
            (item.left, item.row),
            (expected_pixel.0.max(0) as usize / 8 + 1, top + 3,),
            "position: {position}"
        );
        let label = grid.rows[item.row]
            .chars()
            .skip(item.left)
            .take(item.width)
            .collect::<String>();
        assert_eq!(label.trim(), "Choose");
    }
    assert!(Rc::ptr_eq(
        interp.selected_frame.as_ref().unwrap(),
        &selected
    ));
}

#[test]
fn popup_resolves_explicit_event_and_current_pointer_positions() {
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(|| {
            let mut interp = Interp::new();
            eval(
                &mut interp,
                r#"(progn
                   (set-frame-parameter nil 'window-system 'ns)
                   (set-frame-parameter nil 'char-width 8)
                   (set-frame-parameter nil 'char-height 10)
                   (set-frame-parameter nil 'tool-bar-lines 0)
                   (set-frame-parameter nil 'tab-bar-lines 0)
                   (set-frame-parameter nil 'left-fringe 8)
                   (set-frame-parameter nil 'vertical-scroll-bars nil)
                   (set-frame-size nil 80 24)
                   (setq-local header-line-format nil tab-line-format nil)
                   (erase-buffer)
                   (insert "anchor probe\n")
                   (goto-char 1)
                   (setq qa-position-a (selected-frame)
                         qa-position-b
                         (make-frame '((window-system . ns) (width . 80) (height . 24)
                                       (char-width . 8) (char-height . 10)
                                       (tool-bar-lines . 0) (tab-bar-lines . 0)))
                         qa-position-menu (make-sparse-keymap "Popup"))
                   (define-key qa-position-menu [choose] '(menu-item "Choose" ignore))
                   (select-frame qa-position-a))"#,
            );
            let a = frame(&mut interp, "qa-position-a");
            let b = frame(&mut interp, "qa-position-b");
            let frames = [a.clone(), b.clone()];
            // Explicit coordinates must win over a temporary mouse callback.
            interp.gui_mouse_position = Some((320, 150));
            check(
                &mut interp,
                &frames,
                "(list '(8 20) qa-position-a)",
                &a,
                (8, 20),
            );
            check(
                &mut interp,
                &frames,
                "(list '(24 40) qa-position-b)",
                &b,
                (24, 40),
            );

            eval(
                &mut interp,
                "(setq qa-position-right (split-window-right 40)
                       qa-position-bottom (split-window qa-position-right 12 'below))",
            );
            let text = text_origin(&mut interp, &a, "qa-position-bottom");
            // With no decorations, only the default left fringe precedes text.
            let origin = (text.0 - 8, text.1);
            assert!(origin.0 > 0 && origin.1 > 0);
            check(
                &mut interp,
                &frames,
                "(list '(16 20) qa-position-bottom)",
                &a,
                (origin.0 + 16, origin.1 + 20),
            );

            eval(
                &mut interp,
                "(progn (set-frame-parameter nil 'tool-bar-lines 1)
                        (set-frame-parameter nil 'tab-bar-lines 1))",
            );
            check(
                &mut interp,
                &frames,
                "(list '(8 4) (frame-selected-window qa-position-a))",
                &a,
                (8, 24),
            );
            // The bars also reduce the area used to lay out split windows.
            let text_with_bars = text_origin(&mut interp, &a, "qa-position-bottom");
            check(
                &mut interp,
                &frames,
                "(list '(8 4) qa-position-bottom)",
                &a,
                (text_with_bars.0, text_with_bars.1 + 4),
            );
            // Frame coordinates may use the full area above the minibuffer.
            check(
                &mut interp,
                &frames,
                "(list '(8 200) qa-position-a)",
                &a,
                (8, 200),
            );
            check(
                &mut interp,
                &frames,
                "(list '(8 -40) (minibuffer-window qa-position-a))",
                &a,
                (8, 190),
            );
            eval(
                &mut interp,
                "(progn (set-frame-parameter nil 'tool-bar-lines 0)
                        (set-frame-parameter nil 'tab-bar-lines 0))",
            );

            eval(
                &mut interp,
                r#"(progn
                   (setq-local header-line-format "Header" tab-line-format "Tabs")
                   (set-window-margins qa-position-bottom 2 0)
                   (set-window-fringes qa-position-bottom 6 0)
                   (set-window-scroll-bars qa-position-bottom 14 'left))"#,
            );
            let decorated_text = text_origin(&mut interp, &a, "qa-position-bottom");
            check(
                &mut interp,
                &frames,
                "(list 'mouse-3 (list qa-position-bottom 1 '(16 . 20) 0))",
                &a,
                (decorated_text.0 + 16, decorated_text.1 + 20),
            );
            check(
                &mut interp,
                &frames,
                "(list 'mouse-3 (list qa-position-bottom 'mode-line '(16 . 20) 0))",
                &a,
                (origin.0 + 16, origin.1 + 20),
            );

            // Persistent frame-relative positions survive keyboard dispatch.
            interp.gui_mouse_position = None;
            interp.gui_mouse_frame_pixel_position = Some((b.borrow().id, 80, 50));
            for position in ["t", "'(menu-bar)", "'(mouse-1 tab-bar)"] {
                check(&mut interp, &frames, position, &b, (80, 50));
            }
            check(
                &mut interp,
                &frames,
                "'(mouse-1 (not-a-window nil (nil)))",
                &b,
                (80, 50),
            );
            interp.gui_mouse_frame_pixel_position = None;
            interp.gui_mouse_frame_position = Some((b.borrow().id, 9, 6));
            check(&mut interp, &frames, "t", &b, (72, 60));
            eval(&mut interp, "(delete-frame qa-position-b t)");
            check(&mut interp, &[a.clone()], "t", &a, (0, 0));

            for (position, expected) in [
                ("42", "(wrong-type-argument listp 42)"),
                ("'((1 bad) 42)", "(wrong-type-argument windowp 42)"),
                (
                    "(list '(bad 2) (selected-window))",
                    "(wrong-type-argument integerp bad)",
                ),
            ] {
                let value = eval(
                    &mut interp,
                    &format!(
                        "(condition-case err (x-popup-menu {position} qa-position-menu) (error err))"
                    ),
                );
                assert_eq!(interp.prin1_to_string(&value), expected);
            }
        })
        .unwrap()
        .join()
        .unwrap();
}
