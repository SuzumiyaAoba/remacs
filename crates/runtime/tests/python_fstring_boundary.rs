use remacs_runtime::lisp::{Flow, Interp};

fn eval(i: &mut Interp, source: &str) -> remacs_runtime::lisp::Value {
    i.eval_str(source).unwrap_or_else(|flow| match flow {
        Flow::Signal(signal) => panic!(
            "{} {}",
            i.prin1_to_string(&signal.symbol()).to_unicode_lossy(),
            i.prin1_to_string(&signal.data()).to_unicode_lossy()
        ),
        other => panic!("{other:?}"),
    })
}

#[test]
fn python_f_string_predicate_short_circuits_at_point_min() {
    std::thread::Builder::new()
        .stack_size(128 << 20)
        .spawn(|| {
            let mut i = Interp::new();
            let path = concat!(env!("REMACS_SOURCE_ROOT"), "/lisp/python.el");
            let source = format!(
                r#"(progn
                     (with-temp-buffer
                       (insert-file-contents {path:?})
                       (let* ((search "(defun python--f-string-p")
                              (end (search-forward search nil t)))
                         (unless end (error "python--f-string-p not found"))
                         (goto-char (- end (length search)))
                         (eval (read (current-buffer)))))
                     (setq python--f-string-start-regexp "f.")
                     (with-temp-buffer
                       (insert "f\"{{1}}\"\n")
                       (python--f-string-p '(nil nil nil t nil nil nil nil 2))))"#
            );
            let result = eval(&mut i, &source);
            assert_eq!(i.prin1_to_string(&result).to_unicode_lossy(), "t");
        })
        .unwrap()
        .join()
        .unwrap();
}
