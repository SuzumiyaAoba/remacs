use remacs_runtime::lisp::Interp;
use std::path::PathBuf;

#[test]
fn whitespace_unload_removes_watchers_and_disables_modes() {
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(|| {
            let mut i = Interp::new();
            let source = PathBuf::from(env!("REMACS_SOURCE_ROOT")).join("lisp/whitespace.el");
            assert!(source.is_file(), "missing source file: {}", source.display());
            let expression = format!(
                r#"(progn
                     (load-file "{}")
                     (let ((first (generate-new-buffer " *qa-ws-unload-1*"))
                           (second (generate-new-buffer " *qa-ws-unload-2*")))
                       (unwind-protect
                           (progn
                             (with-current-buffer first (whitespace-mode 1))
                             (global-whitespace-mode 1)
                             (with-current-buffer second (whitespace-mode 1))
                             (let ((watcher-before
                                    (and (memq 'whitespace--variable-watcher
                                               (get-variable-watchers 'fill-column))
                                         t)))
                               (whitespace-unload-function)
                               (list watcher-before
                                     (memq 'whitespace--variable-watcher
                                           (get-variable-watchers 'fill-column))
                                     global-whitespace-mode
                                     (with-current-buffer first whitespace-mode)
                                     (with-current-buffer second whitespace-mode))))
                         (when (buffer-live-p first) (kill-buffer first))
                         (when (buffer-live-p second) (kill-buffer second)))))"#,
                source.display()
            );
            let result = i.eval_str(&expression).unwrap();
            assert_eq!(
                i.prin1_to_string(&result).to_unicode_lossy(),
                "(t nil nil nil nil)"
            );
        })
        .unwrap()
        .join()
        .unwrap();
}
