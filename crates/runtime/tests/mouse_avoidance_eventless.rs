use remacs_runtime::{editor::Frame, lisp::Interp};

fn eventless_gui_position_change_is_seen_by_mouse_avoidance() {
    let mut interp = Interp::new();
    let buffer_id = interp.create_buffer("mouse-avoidance-test");
    let minibuffer_id = interp.create_buffer(" *Minibuf-0*");
    let frame = Frame::new_tty(buffer_id, minibuffer_id, 80, 25);
    let frame_id = frame.borrow().id;
    interp.selected_frame = Some(frame.clone());
    interp.frames.push(frame);
    interp.set_current_buffer(buffer_id);
    interp.eval_str("(require 'avoid)").unwrap();
    interp
        .eval_str(
            r#"(setq mouse-avoidance-last-position (mouse-position)
                      mouse-avoidance-pointer-moved nil
                      last-input-event 'keyboard)"#,
        )
        .unwrap();

    // Simulate the GUI polling the OS cursor after it moved without an input event.
    interp.gui_mouse_frame_position = Some((frame_id, 1, 2));
    interp.eval_str("(mouse-avoidance-ignore-p)").unwrap();

    assert!(
        !interp
            .eval_str("mouse-avoidance-pointer-moved")
            .unwrap()
            .is_nil(),
        "mouse avoidance should retain the user's eventless pointer movement"
    );
}

#[test]
fn mouse_avoidance_observes_eventless_pointer_updates() {
    std::thread::Builder::new()
        .name("mouse-avoidance-eventless-position".into())
        .stack_size(8 * 1024 * 1024)
        .spawn(eventless_gui_position_change_is_seen_by_mouse_avoidance)
        .unwrap()
        .join()
        .expect("mouse avoidance test thread panicked");
}
