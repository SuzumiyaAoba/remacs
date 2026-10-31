use remacs_runtime::lisp::{Flow, Interp};

const CASES: &str = r##"
(progn
  (require 'url-expand)
  (dolist (case '(("https://example.com/a?q=1#old" "https://example.com/a?q=1")
                  ("https://example.com/a#old" "https://example.com/a")
                  ("https://example.com/a?q=1" "https://example.com/a?q=1")
                  ("https://example.com/a#" "https://example.com/a")
                  ("https://example.com:8443/a?q=1#old" "https://example.com:8443/a?q=1")
                  ("https://user:pw@example.com/a?q=1#old" "https://user:pw@example.com/a?q=1")
                  ("https://example.com/a%23b?q=%23v#old" "https://example.com/a%23b?q=%23v")
                  ("https://example.com/a?#old" "https://example.com/a?")
                  ("https://example.com/a/../b?q=1#old" "https://example.com/a/../b?q=1")))
    (let* ((original (car case)) (expected (cadr case))
           (base (url-generic-parse-url original))
           (before (url-recreate-url base))
           (target (url-target base)))
      (dolist (reference '("" nil " \r\n "))
        (unless (equal (url-expand-file-name reference original) expected)
          (error "Empty reference %S retained the base fragment: %S" reference original))
        (unless (and (equal (url-expand-file-name reference base) expected)
                     (equal (url-recreate-url base) before) (equal (url-target base) target))
          (error "Empty reference changed or failed to copy the parsed base: %S" original))
        (with-temp-buffer
          (setq-local url-current-object base)
          (unless (and (equal (url-expand-file-name reference) expected)
                       (eq url-current-object base)
                       (equal (url-recreate-url base) before))
            (error "Empty reference changed the current buffer URL: %S" original))))))
  (unless (and (equal (url-expand-file-name "?" "https://example.com/a?q=1#old")
                      "https://example.com/a?")
               (equal (url-expand-file-name "#new" "https://example.com/a?q=1#old")
                      "https://example.com/a?q=1#new"))
    (error "Empty-reference handling changed explicit query/fragment references"))
  t)
"##;

#[test]
fn empty_url_references_drop_only_the_inherited_fragment() {
    let mut interp = Interp::new();
    match interp.eval_str(CASES) {
        Ok(value) => assert!(value.truthy()),
        Err(Flow::Signal(signal)) => panic!(
            "{} {}",
            interp.prin1_to_string(&signal.symbol()),
            interp.prin1_to_string(&signal.data())
        ),
        Err(other) => panic!("{other:?}"),
    }
}
