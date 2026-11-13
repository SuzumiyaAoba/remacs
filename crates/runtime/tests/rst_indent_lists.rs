use remacs_runtime::lisp::{Flow, Interp};
use std::path::PathBuf;

#[test]
fn rst_indent_tracks_definition_and_option_list_descriptions() {
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(|| {
            let mut i = Interp::new();
            let lisp_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../lisp")
                .canonicalize()
                .unwrap();
            i.eval_str(&format!(
                "(setq load-path (cons {:?} load-path))",
                lisp_dir.to_string_lossy()
            ))
            .unwrap();
            let result = i
                .eval_str(
                    r#"(progn
                      (provide 'ansi-color)
                      (provide 'comint)
                      (provide 'compile)
                      (require 'rst)
                      (list
                       (with-temp-buffer
                         (rst-mode)
                         (insert "-a, --all  Do everything.\n\n")
                         (goto-char (point-max))
                         (let ((tabs (rst-compute-tabs (point)))
                               (indent (rst-indent-line)))
                           (list tabs indent (current-indentation))))
                       (with-temp-buffer
                         (rst-mode)
                         (insert "Term\n")
                         (goto-char (point-max))
                         (let ((tabs (rst-compute-tabs (point)))
                               (indent (rst-indent-line)))
                           (list tabs indent (current-indentation))))
                       (with-temp-buffer
                         (rst-mode)
                         (insert "A normal sentence.\n")
                         (goto-char (point-max))
                         (let ((tabs (rst-compute-tabs (point)))
                               (indent (rst-indent-line)))
                           (list tabs indent (current-indentation))))))"#,
                )
                .unwrap_or_else(|flow| match flow {
                    Flow::Signal(signal) => panic!(
                        "rst regression evaluation failed: {} {}",
                        i.prin1_to_string(&signal.symbol()),
                        i.prin1_to_string(&signal.data())
                    ),
                    other => panic!("rst regression evaluation failed: {other:?}"),
                });
            assert_eq!(
                i.prin1_to_string(&result).to_unicode_lossy(),
                "(((11 0) 11 11) ((2 0) 2 2) ((0) 0 0))"
            );
        })
        .unwrap()
        .join()
        .unwrap();
}
