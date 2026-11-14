use remacs_runtime::lisp::Interp;
use std::path::PathBuf;

#[test]
fn whitespace_options_have_custom_types_and_group_membership() {
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(|| {
            let mut i = Interp::new();
            let source = PathBuf::from(env!("REMACS_SOURCE_ROOT")).join("lisp/whitespace.el");
            assert!(source.is_file(), "missing source file: {}", source.display());
            let expression = format!(
                r#"(progn
                     (require 'custom)
                     (load-file "{}")
                     (and
                      (get 'whitespace 'group-documentation)
                      (get 'whitespace 'custom-group)
                      (not (memq nil
                                 (mapcar #'custom-variable-p
                                         '(whitespace-style whitespace-action
                                           whitespace-line-column
                                           whitespace-display-mappings
                                           whitespace-global-modes
                                           whitespace-global-mode-buffers
                                           whitespace-trailing-regexp
                                           whitespace-space-before-tab-regexp
                                           whitespace-indentation-regexp
                                           whitespace-empty-at-bob-regexp
                                           whitespace-empty-at-eob-regexp
                                           whitespace-space-after-tab-regexp
                                           whitespace-hspace-regexp
                                           whitespace-space-regexp
                                           whitespace-tab-regexp
                                           whitespace-big-indent-regexp))))
                      (eq (car (get 'whitespace-style 'custom-type)) 'set)
                      (eq (car (get 'whitespace-action 'custom-type)) 'choice)
                      (eq (car (get 'whitespace-line-column 'custom-type)) 'choice)
                      (eq (car (get 'whitespace-display-mappings 'custom-type))
                          'repeat)
                      (eq (car (get 'whitespace-global-modes 'custom-type)) 'choice)
                      (eq (car (get 'whitespace-trailing-regexp 'custom-type))
                          'regexp)
                      t))"#,
                source.display()
            );
            let result = i.eval_str(&expression).unwrap();
            assert_eq!(i.prin1_to_string(&result).to_unicode_lossy(), "t");
        })
        .unwrap()
        .join()
        .unwrap();
}
