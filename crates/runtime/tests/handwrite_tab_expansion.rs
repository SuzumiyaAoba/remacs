use remacs_runtime::lisp::Interp;

fn handwrite_tab_width_uses_the_source_column() {
    let mut interp = Interp::new();
    let result = interp
        .eval_str(
            r#"(progn
                 (require 'handwrite)
                 (let ((handwrite-numlines 100000)
                       (handwrite-pagenumbering nil))
                   (fset 'y-or-n-p (lambda (&rest _args) nil))
                   (with-temp-buffer
                     (setq-local tab-width 8)
                     (insert "a\tb\n")
                     (handwrite)
                     (with-current-buffer "*handwritten1.ps*"
                       (let* ((output (buffer-string))
                              (start (string-match "xym(a" output))
                              (end (and start (string-match "b" output start))))
                         (- end (+ start 5)))))))"#,
        )
        .unwrap();

    assert_eq!(result.int(), Some(7));
}

#[test]
fn tabs_are_expanded_before_postscript_prefixes_are_inserted() {
    std::thread::Builder::new()
        .name("handwrite-tab-expansion".into())
        .stack_size(8 * 1024 * 1024)
        .spawn(handwrite_tab_width_uses_the_source_column)
        .unwrap()
        .join()
        .expect("handwrite tab test thread panicked");
}
