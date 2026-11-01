use remacs_runtime::{
    frontend::render_frame_grid_with_input_cached,
    lisp::{Interp, Value},
};

#[test]
fn frame_titles_expand_memory_warnings_and_preserve_escaped_percent_signs() {
    let mut i = Interp::new();
    i.eval_str(
        r#"(progn
        (set-frame-parameter nil 'window-system 'ns)
        (set-frame-parameter nil 'menu-bar-lines 0)
        (set-frame-parameter nil 'tool-bar-lines 0)
        (set-frame-parameter nil 'tab-bar-lines 0)
        (set-frame-size nil 48 12))"#,
    )
    .unwrap();
    let primary = i.selected_frame.as_ref().unwrap().clone();
    let Value::Frame(second) = i
        .eval_str(r#"(make-frame '((window-system . ns) (width . 48) (height . 12)))"#)
        .unwrap()
    else {
        panic!("second frame")
    };
    let selected = i.selected_frame.as_ref().unwrap().clone();
    let current = i.current_buffer;
    for format in [r#""title:%e|%%e|%e""#, r#"'("title:" "%e" "|%%e|" "%e")"#] {
        i.eval_str(&format!("(setq frame-title-format {format})"))
            .unwrap();
        for (state, warning) in [
            ("nil", ""),
            ("t", "!MEM FULL! "),
            ("0", "!MEM FULL! "),
            ("nil", ""),
        ] {
            // Exercise the existing warning variable without exhausting memory.
            i.eval_str(&format!("(setq memory-full {state})")).unwrap();
            for frame in [&primary, &second] {
                let grid = render_frame_grid_with_input_cached(&i, frame, 48, 12, None);
                assert_eq!(
                    grid.title,
                    format!("title:{warning}|%e|{warning}"),
                    "memory-full={state}, format={format}"
                );
                assert!(std::rc::Rc::ptr_eq(
                    i.selected_frame.as_ref().unwrap(),
                    &selected
                ));
                assert_eq!(i.current_buffer, current);
            }
        }
    }
}
