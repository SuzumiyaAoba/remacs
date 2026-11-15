use remacs_runtime::lisp::{Interp, Value};

#[test]
fn image_transforms_reports_scaling_only_for_graphical_frames() {
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(|| {
            let mut interp = Interp::new();
            let tty = interp
                .eval_str("(image-transforms-p (selected-frame))")
                .unwrap();
            assert!(matches!(tty, Value::Nil));

            interp
                .eval_str("(set-frame-parameter nil 'window-system 'gpui)")
                .unwrap();
            let graphical = interp.eval_str("(image-transforms-p)").unwrap();
            assert_eq!(interp.prin1_to_string(&graphical), "(scale)");
        })
        .unwrap()
        .join()
        .unwrap();
}
