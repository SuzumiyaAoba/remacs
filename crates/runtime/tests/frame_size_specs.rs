use remacs_runtime::lisp::{Interp, Value};

#[test]
fn frame_size_parameters_update_cells_for_ratios_and_text_pixels() {
    let mut interp = Interp::new();
    interp
        .eval_str(
            r#"(progn
              (set-frame-parameter nil 'window-system 'ns)
              (set-frame-parameter nil 'char-width 8)
              (set-frame-parameter nil 'char-height 16)
              (set-frame-parameter nil 'display-monitor-attributes-list
                '(((name . "test") (workarea 0 0 1600 900))))
              (set-frame-parameter nil 'width 0.25))"#,
        )
        .unwrap();
    assert_eq!(interp.eval_str("(frame-width)").unwrap().int(), Some(50));

    let ratio_frame = interp
        .eval_str("(make-frame '((window-system . ns) (width . 0.5) (height . 0.5)))")
        .unwrap();
    let Value::Frame(ratio_frame) = ratio_frame else {
        panic!("make-frame should return a frame");
    };
    assert_eq!(ratio_frame.borrow().width, 100);
    assert_eq!(ratio_frame.borrow().height, 28);

    let pixel_frame = interp
        .eval_str(
            "(make-frame '((window-system . ns) \
                           (width . (text-pixels . 720)) \
                           (height . (text-pixels . 480))))",
        )
        .unwrap();
    let Value::Frame(pixel_frame) = pixel_frame else {
        panic!("make-frame should return a frame");
    };
    assert_eq!(pixel_frame.borrow().width, 90);
    assert_eq!(pixel_frame.borrow().height, 30);
}
