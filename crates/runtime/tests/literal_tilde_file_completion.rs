use remacs_runtime::lisp::Interp;

fn literal_tilde_directory_completion() {
    let mut interp = Interp::new();
    let actual = interp
        .eval_str(
            r#"(let* ((root (make-temp-file "remacs-sifn-tilde-" t))
                     (dir (concat (file-name-as-directory root) "~literal"))
                     (default-directory "/"))
                (unwind-protect
                    (progn
                      (make-directory dir)
                      (with-temp-file (expand-file-name "child" dir) (insert "x"))
                      (list (and (member "child" (file-name-all-completions "" dir)) t)
                            (and (member "child"
                                         (all-completions
                                          (concat dir "/")
                                          'completion--file-name-table nil))
                                 t)))
                  (delete-directory root t)))"#,
        )
        .unwrap();
    assert_eq!(interp.prin1_to_string(&actual).to_unicode_lossy(), "(t t)");
}

#[test]
fn absolute_path_with_literal_tilde_component_keeps_completing() {
    std::thread::Builder::new()
        .name("literal-tilde-file-completion".into())
        .stack_size(64 * 1024 * 1024)
        .spawn(literal_tilde_directory_completion)
        .unwrap()
        .join()
        .unwrap();
}
