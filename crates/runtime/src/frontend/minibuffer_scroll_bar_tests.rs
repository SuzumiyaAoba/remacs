use super::*;

#[test]
fn explicit_bottom_horizontal_scroll_bar_is_rendered_for_minibuffer_only() {
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(explicit_bottom_horizontal_scroll_bar_is_rendered_for_minibuffer_only_inner)
        .expect("spawn minibuffer scroll bar test thread")
        .join()
        .expect("minibuffer scroll bar test thread");
}

fn explicit_bottom_horizontal_scroll_bar_is_rendered_for_minibuffer_only_inner() {
    let mut i = interp_with_frame("");
    i.eval_str(
        r#"(progn
          (set-frame-parameter nil 'window-system 'gpui)
          (set-frame-parameter nil 'horizontal-scroll-bars 'bottom)
          (setq resize-mini-windows 'grow-only max-mini-window-height 3)
          (with-current-buffer (window-buffer (minibuffer-window))
            (setq-local truncate-lines t)
            (erase-buffer)
            (insert (make-string 120 ?x) "\nline-2\nline-3")))"#,
    )
    .unwrap();
    i.minibuf_level = 1;

    let frame = i.selected_frame.as_ref().unwrap().clone();
    let minibuffer = frame.borrow().minibuffer.clone().unwrap();
    let minibuffer_id = minibuffer.borrow().id;
    minibuffer.borrow_mut().height = 3;
    i.eval_str("(set-window-scroll-bars (minibuffer-window) nil nil 12 t t)")
        .unwrap();
    let frame_default = render_grid(&i, 80, 25);
    assert!(
        !frame_default.scroll_bars.iter().any(|bar| {
            bar.window_id == minibuffer_id
                && bar.orientation == crate::frontend::GridScrollBarOrientation::Horizontal
        }),
        "frame-default horizontal type should be ignored in minibuffers: stored={:?}, effective={:?}, bars={:?}",
        minibuffer.borrow().scroll_bars,
        crate::editor::winxtra::window_scroll_bar_settings(&i, &minibuffer),
        frame_default.scroll_bars
    );

    i.eval_str("(set-window-scroll-bars (minibuffer-window) nil nil 12 'bottom t)")
        .unwrap();
    let explicit_bottom = render_grid(&i, 80, 25);
    assert!(
        explicit_bottom.scroll_bars.iter().any(|bar| {
            bar.window_id == minibuffer_id
                && bar.orientation == crate::frontend::GridScrollBarOrientation::Horizontal
        }),
        "explicit bottom should render for minibuffer: height={}, stored={:?}, effective={:?}, bars={:?}",
        minibuffer.borrow().height,
        minibuffer.borrow().scroll_bars,
        crate::editor::winxtra::window_scroll_bar_settings(&i, &minibuffer),
        explicit_bottom.scroll_bars
    );
    let max_offset = explicit_bottom
        .scroll_bars
        .iter()
        .find(|bar| {
            bar.window_id == minibuffer_id
                && bar.orientation == crate::frontend::GridScrollBarOrientation::Horizontal
        })
        .unwrap()
        .max_offset;
    assert!(max_offset > 0);

    let minibuffer_buffer_id = minibuffer.borrow().buffer;
    let position = crate::frontend::MousePosition {
        buffer_id: minibuffer_buffer_id,
        buffer_position: 0,
        utf16_offset: 0,
        x: 0,
        y: 0,
        row: 0,
        column: 0,
        window_id: Some(minibuffer_id),
        area: Some(crate::frontend::MouseArea::HorizontalScrollBar),
        object: None,
        timestamp: 1,
        mode_line_buffer_name: false,
    };
    i.eval_str(
        r#"(progn
          (setq qa-minibuffer-scrollbar-event nil)
          (defun qa-record-minibuffer-scrollbar-event ()
            (interactive)
            (setq qa-minibuffer-scrollbar-event
                  (list (car last-input-event)
                        (nth 1 (cadr last-input-event)))))
          (define-key global-map [horizontal-scroll-bar mouse-1]
            #'qa-record-minibuffer-scrollbar-event))"#,
    )
    .unwrap();
    let (frontend, _) = test_term(80, 25);
    let term = std::rc::Rc::new(std::cell::RefCell::new(frontend));
    let mut keys = Vec::new();
    let mut arg_mode = false;
    let initial_hscroll = minibuffer.borrow().hscroll;
    let mouse = crate::frontend::MouseInput {
        button: crate::frontend::MouseButton::Left,
        click_count: 1,
        modifiers: 0,
        start: position,
        end: position,
    };
    super::super::dispatch_scroll_bar_input(
        &term,
        &mut i,
        minibuffer_id,
        crate::frontend::GridScrollBarOrientation::Horizontal,
        max_offset,
        max_offset,
        Some(mouse),
        &mut keys,
        &mut arg_mode,
    )
    .unwrap();
    assert_eq!(minibuffer.borrow().hscroll, initial_hscroll);
    let event = i.eval_str("qa-minibuffer-scrollbar-event").unwrap();
    assert_eq!(
        i.prin1_to_string(&event),
        "(mouse-1 horizontal-scroll-bar)"
    );

    super::super::dispatch_scroll_bar_input(
        &term,
        &mut i,
        minibuffer_id,
        crate::frontend::GridScrollBarOrientation::Horizontal,
        max_offset,
        max_offset,
        None,
        &mut keys,
        &mut arg_mode,
    )
    .unwrap();
    assert_eq!(minibuffer.borrow().hscroll, max_offset);
}
