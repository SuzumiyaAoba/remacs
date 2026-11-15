use remacs_runtime::lisp::{Interp, Value};

#[test]
fn display_mouse_p_tracks_the_selected_frames_input_capability() {
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(|| {
            let mut interp = Interp::new();
            let tty = interp.eval_str("(display-mouse-p)").unwrap();
            assert!(matches!(tty, Value::Nil));

            interp
                .eval_str("(set-frame-parameter nil 'window-system 'gpui)")
                .unwrap();
            let graphical = interp.eval_str("(display-mouse-p)").unwrap();
            assert!(matches!(graphical, Value::Sym(symbol) if symbol == remacs_runtime::lisp::obarray::sym::T));

            let explicit_frame = interp
                .eval_str("(display-mouse-p (selected-frame))")
                .unwrap();
            assert!(matches!(explicit_frame, Value::Sym(symbol) if symbol == remacs_runtime::lisp::obarray::sym::T));
        })
        .unwrap()
        .join()
        .unwrap();
}
