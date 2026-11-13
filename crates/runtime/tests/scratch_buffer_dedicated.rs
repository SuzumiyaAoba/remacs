use remacs_runtime::lisp::Interp;

#[test]
fn scratch_buffer_uses_another_window_when_selected_window_is_dedicated() {
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(|| {
            let mut interp = Interp::new();
            let result = interp
                .eval_str(
                    r#"(let* ((window (selected-window))
                             (buffer (get-buffer-create "scratch-dedicated-test")))
                        (set-window-buffer window buffer)
                        (set-window-dedicated-p window t)
                        (scratch-buffer)
                        (list (eq (window-buffer window) buffer)
                              (window-dedicated-p window)
                              (eq (current-buffer) (get-buffer "*scratch*"))
                              (eq (selected-window) window)
                              (eq (window-buffer (selected-window))
                                  (get-buffer "*scratch*"))))"#,
                )
                .unwrap_or_else(|flow| panic!("scratch-buffer failed: {flow:?}"));
            assert_eq!(interp.prin1_to_string(&result), "(t t t nil t)");
        })
        .unwrap()
        .join()
        .unwrap();
}
