use remacs_runtime::{
    editor::{Frame, reflow_frame_windows},
    frontend::render_grid,
    lisp::{Interp, Value},
};

fn eval_integer(interp: &mut Interp, source: &str) -> i128 {
    match interp.eval_str(source).expect(source) {
        Value::Int(value) => value,
        other => panic!("{source} returned {other:?}"),
    }
}

fn tall_image_keeps_pixel_scroll_continuous() {
    let mut interp = Interp::new_with_noninteractive(false);
    let buffer_id = interp.create_buffer("pixel-scroll-tall-image");
    {
        let buffer = interp.buffers.get(buffer_id).unwrap();
        buffer.borrow_mut().insert("X\nbelow image\n");
        buffer.borrow_mut().set_point(2);
    }
    let minibuffer_id = interp.create_buffer(" *Minibuf-0*");
    let frame = Frame::new_tty(buffer_id, minibuffer_id, 80, 25);
    interp.selected_frame = Some(frame.clone());
    interp.frames.push(frame.clone());
    interp.set_current_buffer(buffer_id);
    interp
        .eval_str(
            r#"(progn
              (set-frame-parameter nil 'window-system 'ns)
              (set-frame-parameter nil 'menu-bar-lines 0)
              (set-frame-parameter nil 'tool-bar-lines 0)
              (set-frame-parameter nil 'tab-bar-lines 0)
              (put-text-property
               (point-min) (1+ (point-min)) 'display
               '(image :type svg
                       :data "<svg xmlns='http://www.w3.org/2000/svg' width='40' height='1200'><rect width='40' height='1200' fill='red'/></svg>"))
              (goto-char 3)
              (set-window-point nil (point))
              (require 'pixel-scroll))"#,
        )
        .unwrap();
    reflow_frame_windows(&frame, 80, 25);

    let grid = render_grid(&interp, 80, 25);
    let window_id = frame.borrow().selected.borrow().id;
    let text_area = grid
        .window_text_areas
        .iter()
        .find(|area| area.window_id == window_id)
        .unwrap();
    assert_eq!(grid.row_heights[text_area.top], 1200);
    assert_eq!(eval_integer(&mut interp, "(posn-point (posn-at-x-y 0 100))"), 1);
    interp.eval_str("(set-window-start nil 3 t)").unwrap();

    let metrics = eval_integer(
        &mut interp,
        "(cadr (window-text-pixel-size nil (cons (window-start) -10) (window-start) nil nil nil t))",
    );
    assert_eq!(metrics, 1200);

    interp
        .eval_str("(pixel-scroll-precision-scroll-up-page 10)")
        .unwrap();
    assert_eq!(eval_integer(&mut interp, "(window-start)"), 1);
    assert_eq!(eval_integer(&mut interp, "(window-vscroll nil t)"), 1190);
    assert_eq!(eval_integer(&mut interp, "(posn-point (posn-at-x-y 0 0))"), 1);
    assert_eq!(eval_integer(&mut interp, "(posn-point (posn-at-x-y 0 20))"), 3);
    assert_eq!(eval_integer(&mut interp, "(cdr (posn-x-y (posn-at-point 3)))"), 10);

    interp
        .eval_str("(pixel-scroll-precision-scroll-up-page 10)")
        .unwrap();
    assert_eq!(eval_integer(&mut interp, "(window-vscroll nil t)"), 1180);
}

#[test]
fn pixel_scroll_tracks_tall_image_pixels() {
    std::thread::Builder::new()
        .name("pixel-scroll-tall-image".into())
        .stack_size(64 * 1024 * 1024)
        .spawn(tall_image_keeps_pixel_scroll_continuous)
        .unwrap()
        .join()
        .expect("pixel scroll tall-image test thread panicked");
}
