use remacs_runtime::lisp::Interp;
use std::path::PathBuf;

#[test]
fn battery_library_parses_and_loads_through_the_source_reader() {
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(|| {
            let mut i = Interp::new();
            let source = PathBuf::from(env!("REMACS_SOURCE_ROOT")).join("lisp/battery.el");
            assert!(source.is_file(), "missing source file: {}", source.display());
            let expression = format!(
                r#"(let ((forms 0) form-start parse-error)
                     (with-temp-buffer
                       (insert-file-contents "{}")
                       (goto-char (point-min))
                       (condition-case err
                           (catch 'battery-source-eof
                             (while t
                               (skip-chars-forward " \t\r\n\f")
                               (while (eq (char-after) ?;)
                                 (forward-line 1)
                                 (skip-chars-forward " \t\r\n\f"))
                               (when (eobp)
                                 (throw 'battery-source-eof nil))
                               (setq form-start (point))
                               (read (current-buffer))
                               (setq forms (1+ forms))))
                         (error
                          (setq parse-error
                                (format "%S at character %d after %d forms"
                                        err (1- (or form-start (point-min))) forms)))))
                     (or parse-error (format "parsed %d forms" forms)))"#,
                source.display()
            );
            let result = i.eval_str(&expression).unwrap();
            let result = i.prin1_to_string(&result).to_unicode_lossy();
            assert!(result.starts_with("\"parsed "), "{result}");

            let load_expression = format!(
                "(condition-case err (progn (load-file \"{}\")\n                                              (fboundp 'battery-upower))\n                                (error (format \"%S\" err)))",
                source.display()
            );
            let result = i.eval_str(&load_expression).unwrap();
            assert_eq!(i.prin1_to_string(&result).to_unicode_lossy(), "t");
        })
        .unwrap()
        .join()
        .unwrap();
}
