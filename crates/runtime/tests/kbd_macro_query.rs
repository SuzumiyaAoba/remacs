use remacs_runtime::lisp::Interp;

#[test]
fn kbd_macro_query_exit_stops_the_executing_macro() {
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(|| {
            let mut i = Interp::new();
            let result = i
                .eval_str(
                    r#"(progn
                         (require 'cl-lib)
                         (let ((executing-kbd-macro [120])
                               (defining-kbd-macro nil)
                               (query-replace-map (make-sparse-keymap)))
                           (define-key query-replace-map (vector ?q) 'exit)
                           (cl-letf (((symbol-function 'read-event)
                                      (lambda (&rest _) ?q)))
                             (kbd-macro-query nil))
                           executing-kbd-macro))"#,
                )
                .unwrap();
            assert_eq!(i.prin1_to_string(&result).to_unicode_lossy(), "t");
        })
        .unwrap()
        .join()
        .unwrap();
}
