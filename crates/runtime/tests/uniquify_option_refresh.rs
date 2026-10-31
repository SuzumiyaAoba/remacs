use remacs_runtime::lisp::{Flow, Interp};

#[test]
fn changing_uniquify_style_rebuilds_managed_names() {
    let mut interp = Interp::new();
    let result = interp.eval_str(
        r#"(progn
             (require 'uniquify)
             (let ((old-style (default-value 'uniquify-buffer-name-style))
                   (uniquify-min-dir-content 0)
                   (uniquify-strip-common-suffix nil)
                   (uniquify-after-kill-buffer-flag nil)
                   first second unmanaged)
               (setq first (generate-new-buffer "item.txt")
                     second (generate-new-buffer "item.txt")
                     unmanaged (generate-new-buffer "unmanaged-item"))
               (unwind-protect
                   (progn
                     (with-current-buffer first
                       (setq buffer-file-name "/tmp/a/item.txt"))
                     (with-current-buffer second
                       (setq buffer-file-name "/tmp/b/item.txt"))
                     (uniquify--set-option
                      'uniquify-buffer-name-style 'post-forward-angle-brackets)
                     (uniquify-rationalize-file-buffer-names
                      "item.txt" "/tmp/a" first)
                     (uniquify-rationalize-file-buffer-names
                      "item.txt" "/tmp/b" second)
                     (unless (equal (list (buffer-name first) (buffer-name second))
                                    '("item.txt<a>" "item.txt<b>"))
                       (error "Initial uniquify names: %S"
                              (list (buffer-name first) (buffer-name second))))
                     (uniquify--set-option 'uniquify-buffer-name-style 'forward)
                     (unless (equal (list (buffer-name first) (buffer-name second))
                                    '("a/item.txt" "b/item.txt"))
                       (error "Forward uniquify names: %S"
                              (list (buffer-name first) (buffer-name second))))
                     (uniquify--set-option 'uniquify-buffer-name-style 'reverse)
                     (unless (equal (list (buffer-name first) (buffer-name second))
                                    '("item.txt\\a" "item.txt\\b"))
                       (error "Reverse uniquify names: %S"
                              (list (buffer-name first) (buffer-name second))))
                     (uniquify--set-option 'uniquify-buffer-name-style 'post-forward)
                     (unless (equal (list (buffer-name first) (buffer-name second))
                                    '("item.txt|a" "item.txt|b"))
                       (error "Post-forward uniquify names: %S"
                              (list (buffer-name first) (buffer-name second))))
                     (uniquify--set-option 'uniquify-buffer-name-style nil)
                     (unless (equal (sort (list (buffer-name first) (buffer-name second))
                                          #'string-lessp)
                                    '("item.txt" "item.txt<2>"))
                       (error "Numeric uniquify names: %S"
                              (list (buffer-name first) (buffer-name second))))
                     (unless (condition-case nil
                                 (progn
                                   (uniquify--set-option
                                    'uniquify-buffer-name-style 'invalid-style)
                                   nil)
                               (error t))
                       (error "An invalid uniquify style was accepted"))
                     (unless (equal (buffer-name unmanaged) "unmanaged-item")
                       (error "An unmanaged buffer was renamed: %S"
                              (buffer-name unmanaged))))
                 (dolist (buffer (list first second unmanaged))
                   (when (buffer-live-p buffer)
                     (kill-buffer buffer)))
                 (uniquify--set-option 'uniquify-buffer-name-style old-style)))
             t)"#,
    );
    match result {
        Ok(value) => assert!(value.truthy()),
        Err(Flow::Signal(signal)) => panic!(
            "{} {}",
            interp.prin1_to_string(&signal.symbol()),
            interp.prin1_to_string(&signal.data())
        ),
        Err(other) => panic!("{other:?}"),
    }
}
