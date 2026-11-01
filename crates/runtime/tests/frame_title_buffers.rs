use remacs_runtime::{
    editor::FrameRef,
    frontend::render_frame_grid_with_input_cached,
    lisp::{Interp, Value},
};

fn check(i: &Interp, a: &FrameRef, b: &FrameRef, titles: [&str; 2]) {
    let selected = i.selected_frame.as_ref().unwrap().clone();
    let current = i.current_buffer;
    let point = i.buffers.get(current).unwrap().borrow().point();
    for (frame, expected) in [a, b].into_iter().zip(titles) {
        let grid = render_frame_grid_with_input_cached(i, frame, 48, 12, None);
        assert_eq!(grid.title, expected);
        assert!(std::rc::Rc::ptr_eq(
            i.selected_frame.as_ref().unwrap(),
            &selected
        ));
        assert_eq!(i.current_buffer, current);
        assert_eq!(i.buffers.get(current).unwrap().borrow().point(), point);
    }
}

#[test]
fn frame_titles_read_their_buffer_bindings_and_restore_the_caller() {
    let mut i = Interp::new();
    i.eval_str(r#"(progn
        (set-frame-parameter nil 'window-system 'ns)
        (set-frame-parameter nil 'menu-bar-lines 0)
        (set-frame-parameter nil 'tool-bar-lines 0)
        (set-frame-parameter nil 'tab-bar-lines 0)
        (set-frame-size nil 48 12)
        (setq-default frame-title-format "Default:%b" mode-line-format nil header-line-format nil tab-line-format nil)
        (defvar qa-title-piece "global")
        (defvaralias 'qa-title-alias 'qa-title-piece)
        (switch-to-buffer (get-buffer-create "*title-A*"))
        (setq-local frame-title-format "Title A" qa-title-piece "alpha")
        (with-current-buffer (get-buffer-create "*title-B*")
          (setq-local frame-title-format "Title B" qa-title-piece "beta"))
        (setq qa-title-default (get-buffer-create "*title-default*")
              qa-title-caller (get-buffer-create "*title-caller*")
              qa-title-frame-a (selected-frame)
              qa-title-frame-b (make-frame '((window-system . ns) (width . 48) (height . 12)
                (menu-bar-lines . 0) (tool-bar-lines . 0) (tab-bar-lines . 0))))
        (set-window-buffer (frame-selected-window qa-title-frame-b) "*title-B*")
        (select-frame qa-title-frame-a))"#).unwrap();
    let a = i.selected_frame.as_ref().unwrap().clone();
    let Value::Frame(b) = i.eval_str("qa-title-frame-b").unwrap() else {
        panic!("frame B")
    };
    check(&i, &a, &b, ["Title A", "Title B"]);
    i.eval_str(
        r#"(progn
        (with-current-buffer "*title-A*" (setq frame-title-format '("A:" qa-title-alias)))
        (with-current-buffer "*title-B*" (setq frame-title-format '("B:" qa-title-alias))))"#,
    )
    .unwrap();
    check(&i, &a, &b, ["A:alpha", "B:beta"]);
    // A local dynamic binding affects its buffer, including through aliases.
    let format = i.intern("frame-title-format");
    let piece = i.intern("qa-title-piece");
    let mark = i.specbind_depth();
    i.specbind(format, Value::string("temporary:%b")).unwrap();
    check(&i, &a, &b, ["temporary:*title-A*", "B:beta"]);
    i.unbind_to(mark).unwrap();
    let mark = i.specbind_depth();
    i.specbind(piece, Value::string("temporary alpha")).unwrap();
    check(&i, &a, &b, ["A:temporary alpha", "B:beta"]);
    i.unbind_to(mark).unwrap();
    check(&i, &a, &b, ["A:alpha", "B:beta"]);
    i.eval_str("(select-frame qa-title-frame-b)").unwrap();
    check(&i, &a, &b, ["A:alpha", "B:beta"]);
    // Bind a default in a buffer with no local value. Other buffers retain
    // their own values; a frame showing that buffer sees the active default.
    i.eval_str("(set-buffer qa-title-default)").unwrap();
    let mark = i.specbind_depth();
    i.specbind(format, Value::string("temporary global:%b"))
        .unwrap();
    check(&i, &a, &b, ["A:alpha", "B:beta"]);
    i.eval_str("(set-window-buffer (frame-selected-window qa-title-frame-a) qa-title-default)")
        .unwrap();
    check(&i, &a, &b, ["temporary global:*title-default*", "B:beta"]);
    i.eval_str("(set-window-buffer (frame-selected-window qa-title-frame-a) \"*title-A*\")")
        .unwrap();
    i.unbind_to(mark).unwrap();
    i.eval_str(
        r#"(progn (set-buffer qa-title-caller) (setq-local frame-title-format "Caller only"))"#,
    )
    .unwrap();
    check(&i, &a, &b, ["A:alpha", "B:beta"]);
    i.eval_str("(set-window-buffer (frame-selected-window qa-title-frame-a) \"*title-B*\")")
        .unwrap();
    check(&i, &a, &b, ["B:beta", "B:beta"]);
    i.eval_str(r#"(with-current-buffer "*title-B*" (kill-local-variable 'frame-title-format))"#)
        .unwrap();
    check(&i, &a, &b, ["Default:*title-B*", "Default:*title-B*"]);
    i.eval_str("(set-window-buffer (frame-selected-window qa-title-frame-a) qa-title-default)")
        .unwrap();
    check(&i, &a, &b, ["Default:*title-default*", "Default:*title-B*"]);
    i.eval_str("(select-frame qa-title-frame-a)").unwrap();
    check(&i, &a, &b, ["Default:*title-default*", "Default:*title-B*"]);
}
