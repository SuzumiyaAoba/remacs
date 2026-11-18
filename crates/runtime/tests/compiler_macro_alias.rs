use remacs_runtime::lisp::Interp;

#[test]
fn macroexpand_all_follows_long_compiler_macro_alias_chains() {
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(|| {
            let mut interp = Interp::new();
            let result = interp
                .eval_str(
                    "(require 'macroexp)\
                     (defun qa-long-cmacro-handler (form &rest args) 42)\
                     (defmacro qa-long-ordinary-macro (arg)\
                       (list 'quote arg))\
                     (put 'qa-long-cmacro-target 'compiler-macro\
                          #'qa-long-cmacro-handler)\
                     (let ((last 'qa-long-cmacro-target))\
                       (dotimes (n 201)\
                         (let ((alias (intern\
                                       (concat \"qa-long-cmacro-\"\
                                               (number-to-string n)))))\
                           (defalias alias last)\
                           (setq last alias)))\
                       (list (macroexpand-all (list last 'arg))\
                             (macroexpand-all\
                              '(qa-long-ordinary-macro ordinary-arg))))",
                )
                .unwrap();
            assert_eq!(
                interp.prin1_to_string(&result),
                "(42 'ordinary-arg)"
            );
        })
        .unwrap()
        .join()
        .unwrap();
}

#[test]
fn call_interactively_reads_a_spec_before_a_circular_tail() {
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(|| {
            let mut interp = Interp::new();
            let result = interp
                .eval_str(
                    "(defun qa-circular-interactive-spec (value) value)\
                     (let* ((spec (list 'interactive '(list 42)))\
                            (tail (last spec)))\
                       (setcdr tail tail)\
                       (unwind-protect\
                           (progn\
                             (put 'qa-circular-interactive-spec\
                                  'interactive-form spec)\
                             (call-interactively 'qa-circular-interactive-spec))\
                         (setplist 'qa-circular-interactive-spec nil)\
                         (setcdr tail nil)))",
                )
                .unwrap();
            assert_eq!(interp.prin1_to_string(&result), "42");
        })
        .unwrap()
        .join()
        .unwrap();
}

#[test]
fn interactive_form_finds_a_declaration_before_a_circular_lambda_body_tail() {
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(|| {
            let mut interp = Interp::new();
            let result = interp
                .eval_str(
                    "(let* ((spec (list 'interactive '(list 42)))\
                            (form (list 'lambda nil spec))\
                            (tail (last form)))\
                       (setcdr tail tail)\
                       (unwind-protect\
                           (interactive-form form)\
                         (setcdr tail nil)))",
                )
                .unwrap();
            assert_eq!(interp.prin1_to_string(&result), "(interactive (list 42))");
        })
        .unwrap()
        .join()
        .unwrap();
}
