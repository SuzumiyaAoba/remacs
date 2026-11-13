use super::*;

#[test]
fn vertical_scroll_bar_click_and_drag_dispatch_to_its_keymap() {
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(vertical_scroll_bar_click_and_drag_dispatch_to_its_keymap_inner)
        .expect("spawn scrollbar mouse test thread")
        .join()
        .expect("scrollbar mouse test thread");
}

fn vertical_scroll_bar_click_and_drag_dispatch_to_its_keymap_inner() {
    let text = (0..100)
        .map(|line| format!("line-{line:03}\n"))
        .collect::<String>();
    let mut i = interp_with_frame(&text);
    i.eval_str(
        r#"(progn
          (setq qa-scrollbar-events nil)
          (defun qa-scrollbar-record-event ()
            (interactive)
            (push (list (car last-input-event)
                        (nth 1 (cadr last-input-event)))
                  qa-scrollbar-events))
          (define-key global-map [vertical-scroll-bar mouse-1]
            #'qa-scrollbar-record-event)
          (define-key global-map [vertical-scroll-bar drag-mouse-1]
            #'qa-scrollbar-record-event))"#,
    )
    .unwrap();

    let mut click_position = mouse_position(&i, 1);
    click_position.area = Some(crate::frontend::MouseArea::VerticalScrollBar);
    let drag_start = click_position;
    let mut drag_end = click_position;
    drag_end.x += 1;

    let (frontend, _) = test_term(80, 25);
    let term = Rc::new(RefCell::new(frontend));
    let (window_id, window) = {
        let frame = i.selected_frame.as_ref().unwrap().borrow();
        let window = frame.selected.clone();
        (window.borrow().id, window.clone())
    };
    let initial_start = window.borrow().start;
    let mut keys = Vec::new();
    let mut arg_mode = false;
    let mut dispatch_scroll_bar = |mouse| {
        super::super::dispatch_scroll_bar_input(
            &term,
            &mut i,
            window_id,
            GridScrollBarOrientation::Vertical,
            99,
            99,
            Some(mouse),
            &mut keys,
            &mut arg_mode,
        )
        .unwrap();
    };
    dispatch_scroll_bar(MouseInput {
        button: MouseButton::Left,
        click_count: 1,
        modifiers: 0,
        start: click_position,
        end: click_position,
    });
    dispatch_scroll_bar(MouseInput {
        button: MouseButton::Left,
        click_count: 1,
        modifiers: 0,
        start: drag_start,
        end: drag_end,
    });
    drop(dispatch_scroll_bar);

    let events = i.eval_str("qa-scrollbar-events").unwrap();
    assert_eq!(
        i.prin1_to_string(&events),
        "((drag-mouse-1 vertical-scroll-bar) (mouse-1 vertical-scroll-bar))"
    );
    assert_eq!(window.borrow().start, initial_start);
}
