use remacs_runtime::lisp::Interp;

#[test]
fn insert_kbd_macro_with_keys_records_global_bindings() {
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(|| {
            let mut i = Interp::new();
            let result = i
                .eval_str(
                    r#"(progn
                         (fset 'qa-audit-kbd-macro [120])
                         (global-set-key (kbd "C-c m") 'qa-audit-kbd-macro)
                         (let ((with-keys
                                (with-temp-buffer
                                  (insert-kbd-macro 'qa-audit-kbd-macro t)
                                  (buffer-string)))
                               (without-keys
                                (with-temp-buffer
                                  (insert-kbd-macro 'qa-audit-kbd-macro)
                                  (buffer-string))))
                           (list (and (string-match-p "keymap-global-set" with-keys) t)
                                 (and (string-match-p "C-c m" with-keys) t)
                                 (and (string-match-p "keymap-global-set" without-keys) t))))"#,
                )
                .unwrap();
            assert_eq!(
                i.prin1_to_string(&result).to_unicode_lossy(),
                "(t t nil)"
            );
        })
        .unwrap()
        .join()
        .unwrap();
}
