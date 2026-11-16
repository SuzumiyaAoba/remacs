use remacs_runtime::lisp::Interp;

#[test]
fn graphical_frame_fringe_width_reports_the_rendered_pixel_total() {
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(|| {
            let mut interp = Interp::new();
            interp
                .eval_str(
                    "(progn \
                     (set-frame-parameter nil 'window-system 'gpui) \
                     (set-frame-parameter nil 'left-fringe nil) \
                     (set-frame-parameter nil 'right-fringe nil))",
                )
                .unwrap();
            assert_eq!(
                interp.eval_str("(frame-fringe-width)").unwrap().int(),
                Some(16)
            );

            interp
                .eval_str(
                    "(set-frame-parameter nil 'left-fringe 3) \
                     (set-frame-parameter nil 'right-fringe 5)",
                )
                .unwrap();
            assert_eq!(
                interp.eval_str("(frame-fringe-width)").unwrap().int(),
                Some(8)
            );
        })
        .unwrap()
        .join()
        .unwrap();
}
