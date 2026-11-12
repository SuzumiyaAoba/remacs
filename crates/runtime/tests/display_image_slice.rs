use remacs_runtime::{
    editor::Frame,
    frontend::{GridImageSlicePixels, render_grid},
    lisp::Interp,
};

fn image_slice_display_spec_reaches_the_frontend_grid() {
    let mut interp = Interp::new();
    let buffer_id = interp.create_buffer("image-slice-test");
    {
        let buffer = interp.buffers.get(buffer_id).unwrap();
        buffer.borrow_mut().insert("XYZ\n");
        buffer.borrow_mut().set_point(0);
    }
    let minibuffer_id = interp.create_buffer(" *Minibuf-0*");
    let frame = Frame::new_tty(buffer_id, minibuffer_id, 80, 25);
    interp.selected_frame = Some(frame.clone());
    interp.frames.push(frame);
    interp.set_current_buffer(buffer_id);

    interp
        .eval_str(
            r#"(let* ((start (point-min))
                      (image '(image :type svg
                                     :data "<svg xmlns='http://www.w3.org/2000/svg' width='40' height='20'><rect width='20' height='20' fill='red'/><rect x='20' width='20' height='20' fill='blue'/></svg>"))
                      (left (list '(slice 0 0 20 20) image))
                      (right (list '(slice 20 0 20 20) image))
                      (fraction (list '(slice 0.0 0.0 0.5 1.0) image)))
                 (put-text-property start (1+ start) 'display left)
                 (put-text-property (1+ start) (+ start 2) 'display right)
                 (put-text-property (+ start 2) (+ start 3) 'display fraction))"#,
        )
        .unwrap();

    let grid = render_grid(&interp, 8, 4);
    assert_eq!(grid.images.len(), 3);
    assert_eq!(
        grid.images[0].resolved_slice(),
        Some(GridImageSlicePixels {
            x: 0,
            y: 0,
            width: 20,
            height: 20,
        })
    );
    assert_eq!(
        grid.images[1].resolved_slice(),
        Some(GridImageSlicePixels {
            x: 20,
            y: 0,
            width: 20,
            height: 20,
        })
    );
    assert_eq!(
        grid.images[2].resolved_slice(),
        Some(GridImageSlicePixels {
            x: 0,
            y: 0,
            width: 20,
            height: 20,
        })
    );
    assert!(!grid.rows[0]
        .chars()
        .any(|character| matches!(character, 'X' | 'Y' | 'Z')));
}

#[test]
fn image_slice_display_spec_replaces_its_placeholder() {
    std::thread::Builder::new()
        .name("image-slice-display-property".into())
        .stack_size(8 * 1024 * 1024)
        .spawn(image_slice_display_spec_reaches_the_frontend_grid)
        .unwrap()
        .join()
        .expect("image slice test thread panicked");
}
