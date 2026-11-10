use remacs_runtime::{
    editor::FrameRef,
    frontend::{CellColor, render_frame_grid_with_input_cached},
    lisp::{Flow, Interp, Value},
};

fn eval(interp: &mut Interp, source: &str) -> Value {
    interp.eval_str(source).unwrap_or_else(|flow| match flow {
        Flow::Signal(signal) => panic!(
            "{} {}",
            interp.prin1_to_string(&signal.symbol()),
            interp.prin1_to_string(&signal.data())
        ),
        other => panic!("{other:?}"),
    })
}

fn mouse_color(interp: &Interp, frame: &FrameRef) -> Option<CellColor> {
    render_frame_grid_with_input_cached(interp, frame, 40, 12, None).mouse_color
}

#[test]
fn native_mouse_pointer_color_tracks_the_frame_parameter() {
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(|| {
            let mut interp = Interp::new();
            eval(
                &mut interp,
                "(modify-frame-parameters nil '((window-system . ns) (width . 40) (height . 12)))",
            );
            let frame = interp.selected_frame.as_ref().unwrap().clone();

            eval(&mut interp, "(set-mouse-color \"red\")");
            assert_eq!(mouse_color(&interp, &frame), Some(CellColor(255, 0, 0)));

            eval(
                &mut interp,
                "(set-frame-parameter nil 'mouse-color \"#123456\")",
            );
            assert_eq!(
                mouse_color(&interp, &frame),
                Some(CellColor(0x12, 0x34, 0x56))
            );

            eval(&mut interp, "(set-frame-parameter nil 'mouse-color nil)");
            assert_eq!(mouse_color(&interp, &frame), None);
        })
        .unwrap()
        .join()
        .unwrap();
}
