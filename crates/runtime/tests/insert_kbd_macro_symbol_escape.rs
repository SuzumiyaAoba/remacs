use remacs_runtime::lisp::Interp;

#[test]
fn insert_kbd_macro_emits_readable_escaped_symbol_names() {
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(|| {
            let mut i = Interp::new();
            let result = i
                .eval_str(
                    r#"(let ((name (intern "weird macro")))
                         (fset name [120])
                         (with-temp-buffer
                           (insert-kbd-macro name)
                           (eval-buffer)
                           (equal (symbol-function name) [120])))"#,
                )
                .unwrap();
            assert_eq!(i.prin1_to_string(&result).to_unicode_lossy(), "t");
        })
        .unwrap()
        .join()
        .unwrap();
}
