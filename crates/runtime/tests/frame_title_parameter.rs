use remacs_runtime::{
    editor::FrameRef,
    frontend::render_frame_grid_with_input_cached,
    lisp::{Interp, Value},
};

fn check(i: &Interp, frames: [&FrameRef; 2], expected: [&str; 2]) {
    let selected = i.selected_frame.as_ref().unwrap().clone();
    let current = i.current_buffer;
    for (frame, title) in frames.into_iter().zip(expected) {
        let name = frame.borrow().name.clone();
        assert_eq!(
            render_frame_grid_with_input_cached(i, frame, 48, 12, None).title,
            title
        );
        assert!(std::rc::Rc::ptr_eq(&frame.borrow().name, &name));
        assert!(std::rc::Rc::ptr_eq(
            i.selected_frame.as_ref().unwrap(),
            &selected
        ));
        assert_eq!(i.current_buffer, current);
    }
}

#[test]
fn explicit_titles_override_formats_and_clearing_them_restores_formatting() {
    let mut i = Interp::new();
    i.eval_str(r#"(progn
        (set-frame-parameter nil 'window-system 'ns)
        (set-frame-parameter nil 'menu-bar-lines 0)
        (set-frame-parameter nil 'tool-bar-lines 0)
        (set-frame-parameter nil 'tab-bar-lines 0)
        (set-frame-size nil 48 12)
        (setq-default mode-line-format nil header-line-format nil tab-line-format nil)
        (switch-to-buffer (get-buffer-create "*pinned-A*"))
        (setq-local frame-title-format "A:%b")
        (with-current-buffer (get-buffer-create "*pinned-B*") (setq-local frame-title-format "B:%b"))
        (setq qa-pinned-a (selected-frame)
              qa-pinned-b (make-frame '((window-system . ns) (width . 48) (height . 12)
                (menu-bar-lines . 0) (tool-bar-lines . 0) (tab-bar-lines . 0) (title . "Created title"))))
        (set-window-buffer (frame-selected-window qa-pinned-b) "*pinned-B*"))"#).unwrap();
    let a = i.selected_frame.as_ref().unwrap().clone();
    let Value::Frame(b) = i.eval_str("qa-pinned-b").unwrap() else {
        panic!("frame B")
    };
    check(&i, [&a, &b], ["A:*pinned-A*", "Created title"]);
    i.eval_str("(set-frame-parameter qa-pinned-b 'title nil)")
        .unwrap();
    check(&i, [&a, &b], ["A:*pinned-A*", "B:*pinned-B*"]);
    i.eval_str(
        r#"(progn
        (select-frame qa-pinned-b)
        (set-frame-parameter qa-pinned-a 'title "Pinned A")
        (with-current-buffer "*pinned-A*"
          (setq frame-title-format "Changed:%b") (rename-buffer "*pinned-A-new*")))"#,
    )
    .unwrap();
    check(&i, [&a, &b], ["Pinned A", "B:*pinned-B*"]);
    i.eval_str(r#"(modify-frame-parameters qa-pinned-b '((title . "Pinned %b %%e 名")))"#)
        .unwrap();
    check(&i, [&a, &b], ["Pinned A", "Pinned %b %%e 名"]);
    i.eval_str(r#"(set-frame-parameter qa-pinned-a 'name "Friendly name")"#)
        .unwrap();
    check(&i, [&a, &b], ["Pinned A", "Pinned %b %%e 名"]);
    i.eval_str("(set-frame-parameter qa-pinned-a 'name nil)")
        .unwrap();
    i.eval_str(r#"(set-frame-parameter qa-pinned-a 'title "")"#)
        .unwrap();
    check(&i, [&a, &b], ["", "Pinned %b %%e 名"]);
    i.eval_str(
        r#"(progn
        (setq qa-pinned-string (copy-sequence "Pinned %b"))
        (set-frame-parameter qa-pinned-a 'title qa-pinned-string)
        (aset qa-pinned-string 0 ?Z))"#,
    )
    .unwrap();
    check(&i, [&a, &b], ["Zinned %b", "Pinned %b %%e 名"]);
    i.eval_str("(progn (set-frame-parameter qa-pinned-a 'title nil) (set-frame-parameter qa-pinned-b 'title nil))").unwrap();
    check(&i, [&a, &b], ["Changed:*pinned-A-new*", "B:*pinned-B*"]);
    i.eval_str("(select-frame qa-pinned-a)").unwrap();
    check(&i, [&a, &b], ["Changed:*pinned-A-new*", "B:*pinned-B*"]);
}
