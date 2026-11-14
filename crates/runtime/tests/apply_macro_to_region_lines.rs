use remacs_runtime::lisp::Interp;

#[test]
fn apply_macro_to_region_lines_autoloads_and_runs_on_each_line() {
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(|| {
            let mut i = Interp::new();
            let result = i
                .eval_str(
                    r#"(condition-case err
                          (progn
                            (require 'cl-lib)
                            (let ((qa-macro-call-count 0)
                                  (qa-was-autoload
                                   (autoloadp
                                    (symbol-function 'apply-macro-to-region-lines))))
                              (with-temp-buffer
                                (insert "a\nb\n")
                                (let ((last-kbd-macro [120]))
                                  (cl-letf (((symbol-function 'execute-kbd-macro)
                                             (lambda (_macro)
                                               (setq qa-macro-call-count
                                                     (1+ qa-macro-call-count))
                                               (insert "x"))))
                                    (apply-macro-to-region-lines
                                     (point-min) (point-max)))
                                  (list (buffer-string)
                                        qa-macro-call-count
                                        qa-was-autoload
                                        (fboundp 'apply-macro-to-region-lines))))))
                        (error (format "failure: %S" err)))"#,
                )
                .unwrap();
            assert_eq!(
                i.prin1_to_string(&result).to_unicode_lossy(),
                "(\"xa\nxb\n\" 2 t t)"
            );
        })
        .unwrap()
        .join()
        .unwrap();
}
