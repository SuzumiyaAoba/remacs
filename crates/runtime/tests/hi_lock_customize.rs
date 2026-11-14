use remacs_runtime::lisp::Interp;
use std::path::PathBuf;

#[test]
fn hi_lock_user_settings_have_custom_types_and_groups() {
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(|| {
            let mut i = Interp::new();
            let source = PathBuf::from(env!("REMACS_SOURCE_ROOT")).join("lisp/hi-lock.el");
            assert!(source.is_file(), "missing source file: {}", source.display());
            let expression = format!(
                r#"(progn
                     (require 'custom)
                     (load-file "{}")
                     (and
                      (custom-variable-p 'hi-lock-file-patterns-range)
                      (eq (get 'hi-lock-file-patterns-range 'custom-type) 'natnum)
                      (custom-variable-p 'hi-lock-highlight-range)
                      (eq (get 'hi-lock-highlight-range 'custom-type) 'natnum)
                      (custom-variable-p 'hi-lock-exclude-modes)
                      (equal (get 'hi-lock-exclude-modes 'custom-type)
                             '(repeat symbol))
                      (custom-variable-p 'hi-lock-file-patterns-policy)
                      (eq (car (get 'hi-lock-file-patterns-policy 'custom-type))
                          'choice)
                      (custom-variable-p 'hi-lock-auto-select-face)
                      (eq (get 'hi-lock-auto-select-face 'custom-type) 'boolean)
                      (get 'hi-lock 'group-documentation)
                      (get 'hi-lock-faces 'group-documentation)
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
