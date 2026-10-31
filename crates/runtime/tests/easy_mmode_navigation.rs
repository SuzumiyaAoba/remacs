use std::path::PathBuf;

use remacs_runtime::lisp::{Flow, Interp};

fn lisp_string(value: &str) -> String {
    format!(
        "\"{}\"",
        value.replace('\\', "\\\\").replace('"', "\\\"")
    )
}

#[test]
fn navigation_restores_restriction_after_errors_and_renarrows_on_success() {
    let source = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../lisp/easy-mmode.el")
        .canonicalize()
        .unwrap();
    let code = format!(
        r#"(progn
          (load {})
          (defun remacs-test-navigation-narrow-to-line ()
            (narrow-to-region (line-beginning-position)
                              (min (point-max) (1+ (line-end-position)))))
          (defun remacs-test-navigation-error-narrow ()
            (error "Narrowing failed"))
          (easy-mmode-define-navigation remacs-test-navigation "^hit$" "entry"
            nil remacs-test-navigation-narrow-to-line)
          (defun remacs-test-navigation-failure (command start end)
            (with-temp-buffer
              (insert "aa\nhit\nzz\n")
              (narrow-to-region start end)
              (goto-char (1+ start))
              (let ((signal nil))
                (condition-case err (funcall command)
                  (error (setq signal (car err))))
                (unless (eq signal 'user-error)
                  (error "Expected user-error, got %S" signal))
                (list t (list (point-min) (point-max))))))
          (defun remacs-test-navigation-error ()
            (with-temp-buffer
              (insert "aa\nhit\nzz\n")
              (narrow-to-region 1 4)
              (goto-char 2)
              (let ((signal nil))
                (condition-case err
                    (easy-mmode--next "^hit$" "entry" 1 nil
                                      #'remacs-test-navigation-error-narrow)
                  (error (setq signal (car err))))
                (unless (eq signal 'error) (error "Expected a NARROWFUN error"))
                (list t (list (point-min) (point-max))))))
          (defun remacs-test-navigation-success (command start end)
            (with-temp-buffer
              (insert "aa\nhit\nzz\n")
              (narrow-to-region start end)
              (goto-char (1+ start))
              (funcall command)
              (list (point-min) (point-max))))
          (defun remacs-test-navigation-unrestricted-success ()
            (with-temp-buffer
              (insert "aa\nhit\nzz\n")
              (goto-char 2)
              (easy-mmode--next "^hit$" "entry" 1)
              (list (buffer-narrowed-p) (point))))
          (list
           (remacs-test-navigation-failure
            (lambda () (easy-mmode--next "^missing$" "entry" 1 nil
                                         #'remacs-test-navigation-narrow-to-line))
            1 4)
           (remacs-test-navigation-failure
            (lambda () (easy-mmode--prev "^missing$" "entry" 1 nil
                                         #'remacs-test-navigation-narrow-to-line))
            8 11)
           (remacs-test-navigation-error)
           (remacs-test-navigation-success #'remacs-test-navigation-next 1 4)
           (remacs-test-navigation-success #'remacs-test-navigation-prev 8 11)
           (remacs-test-navigation-unrestricted-success)))"#,
        lisp_string(source.to_str().unwrap())
    );

    let mut interp = Interp::new();
    let result = interp.eval_str(&code).unwrap_or_else(|flow| match flow {
        Flow::Signal(signal) => panic!(
            "{} {}",
            interp.prin1_to_string(&signal.symbol()),
            interp.prin1_to_string(&signal.data())
        ),
        other => panic!("{other:?}"),
    });
    assert_eq!(
        interp.prin1_to_string(&result),
        "((t (1 4)) (t (8 11)) (t (1 4)) (4 8) (4 8) (nil 4))"
    );
}
