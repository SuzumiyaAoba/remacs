use remacs_runtime::lisp::{Flow, Interp};

#[test]
fn invalid_sort_arguments_keep_type_errors_and_raw_prefixes_still_work() {
    let mut interp = Interp::new();
    let result = interp
        .eval_str(
            r#"(progn
                 (require 'tabulated-list)
                 (require 'cl-lib)
                 (with-temp-buffer
                   (tabulated-list-mode)
                   (setq tabulated-list-format [("Column" 8 t)])
                   (let (called)
                     (cl-letf (((symbol-function 'tabulated-list--sort-by-column-name)
                                (lambda (name) (setq called name))))
                       (list
                        (mapcar
                         (lambda (arg)
                           (condition-case err (tabulated-list-sort arg)
                             (wrong-type-argument err)))
                         '("a" "0" [] foo ("a") (nil) ((0))))
                        (let ((bad (propertize "a" 'face 'bold)))
                          (condition-case err (tabulated-list-sort bad)
                            (wrong-type-argument
                             (and (eq (nth 1 err) 'number-or-marker-p)
                                  (eq (nth 2 err) bad)))))
                        (mapcar
                         (lambda (arg)
                           (setq called nil)
                           (tabulated-list-sort arg)
                           called)
                         '(0 (0) (0 . ignored)))
                        (mapcar
                         (lambda (arg)
                           (condition-case err (tabulated-list-sort arg)
                             (error err)))
                         '(- -1 (-1))))))))"#,
        )
        .unwrap_or_else(|flow| match flow {
            Flow::Signal(signal) => panic!(
                "{} {}",
                interp.prin1_to_string(&signal.symbol()),
                interp.prin1_to_string(&signal.data())
            ),
            other => panic!("{other:?}"),
        });
    let errors = [
        r#"(wrong-type-argument number-or-marker-p "a")"#,
        r#"(wrong-type-argument number-or-marker-p "0")"#,
        "(wrong-type-argument number-or-marker-p [])",
        "(wrong-type-argument number-or-marker-p foo)",
        r#"(wrong-type-argument number-or-marker-p ("a"))"#,
        "(wrong-type-argument number-or-marker-p (nil))",
        "(wrong-type-argument number-or-marker-p ((0)))",
    ]
    .join(" ");
    let original_order_error = r#"(error "Order is already in original order")"#;
    assert_eq!(
        interp.prin1_to_string(&result),
        format!(
            r#"(({errors}) t ("Column" "Column" "Column") ({original_order_error} {original_order_error} {original_order_error}))"#,
        )
    );
}
