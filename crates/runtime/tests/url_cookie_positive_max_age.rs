use remacs_runtime::lisp::{Flow, Interp};
use std::path::PathBuf;

fn lisp_string(value: &str) -> String {
    format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
}

#[test]
fn huge_positive_max_age_saturates_and_survives_cleanup() {
    std::thread::Builder::new()
        .name("url-cookie-positive-max-age".into())
        .stack_size(64 * 1024 * 1024)
        .spawn(check_huge_positive_max_age)
        .unwrap()
        .join()
        .expect("positive Max-Age test thread panicked");
}

fn check_huge_positive_max_age() {
    let mut interp = Interp::new();
    let source = PathBuf::from(env!("REMACS_SOURCE_ROOT")).join("lisp/url-cookie.el");
    let load_code = format!(
        "(progn
         (setq noninteractive t)
         (defvar url-setup-done nil)
         (defvar url-cookie-file nil)
         (defvar url-cookie-storage nil)
         (defvar url-cookie-secure-storage nil)
         (defvar url-cookies-changed-since-last-save nil)
         (defvar url-cookie-timer nil)
         (defvar url-cookie-save-interval 0)
         (let ((url-setup-done nil)
               (url-current-object nil)
               (url-cookie-file nil)
               (url-cookie-storage nil)
               (url-cookie-secure-storage nil)
               (url-cookies-changed-since-last-save nil)
               (url-cookie-timer nil)
               (url-cookie-save-interval 0))
           (load {} nil t t)))",
        lisp_string(source.to_str().unwrap())
    );
    interp
        .eval_str(&load_code)
        .unwrap_or_else(|flow| match flow {
            Flow::Signal(signal) => panic!(
                "{} {}",
                interp.prin1_to_string(&signal.symbol()),
                interp.prin1_to_string(&signal.data())
            ),
            other => panic!("{other:?}"),
        });
    interp
        .eval_str(
            r#"(progn
              (setq url-cookie-file nil
                    url-cookie-storage nil
                    url-cookie-secure-storage nil
                    url-cookies-changed-since-last-save nil
                    url-cookie-timer nil
                    url-cookie-save-interval 0
                    url-current-object
                    (url-generic-parse-url "http://example.test/path")
                    url-cookie-confirmation nil
                    url-cookie-trusted-urls nil
                    url-cookie-untrusted-urls nil)
              (fset 'url-parse-args
                    (lambda (header &optional _parse)
                      (let ((start (string-match "Max-Age=" header)))
                        (list (cons "big" "1")
                              (cons "Max-Age" (substring header (+ start 8)))))))
              (fset 'url-host (lambda (&rest _) "example.test"))
              (fset 'url-view-url (lambda (&optional _) "http://example.test/path"))
              (fset 'url-path-and-query (lambda (&rest _) (list "/path" nil))))"#,
        )
        .unwrap();
    interp
        .eval_str(
            "(url-cookie-handle-set-cookie (concat \"big=1; Max-Age=\" (make-string 101 ?9)))",
        )
        .unwrap_or_else(|flow| match flow {
            Flow::Signal(signal) => panic!(
                "{} {}",
                interp.prin1_to_string(&signal.symbol()),
                interp.prin1_to_string(&signal.data())
            ),
            other => panic!("{other:?}"),
        });
    let value = interp
        .eval_str(
            "(let* ((cookie (cadr (car url-cookie-storage)))\
                    (expires (url-cookie-expires cookie))\
                    (expiration (url-cookie--expires-date-to-time expires)))\
               (url-cookie-clean-up)\
               (list (= (string-to-number (substring expires -8 -4)) 9999)\
                     (length url-cookie-storage)\
                     (time-less-p expiration nil)))",
        )
        .unwrap_or_else(|flow| match flow {
            Flow::Signal(signal) => panic!(
                "{} {}",
                interp.prin1_to_string(&signal.symbol()),
                interp.prin1_to_string(&signal.data())
            ),
            other => panic!("{other:?}"),
        });
    assert_eq!(interp.prin1_to_string(&value), "(t 1 nil)");
}
