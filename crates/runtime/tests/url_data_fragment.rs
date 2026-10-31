use remacs_runtime::lisp::{Flow, Interp};

const CASES: &str = r##"
(progn
  (require 'url-expand)
  (let ((base (url-generic-parse-url "data:text/plain,abc#old")))
    (unless (and (equal (url-filename base) "text/plain,abc") (equal (url-target base) "old"))
      (error "The data URL parser did not separate its fragment")))
  (dolist (case '(("data:text/plain,abc" "#frag" "data:text/plain,abc#frag")
                  ("data:text/plain;base64,YWJj" "#frag" "data:text/plain;base64,YWJj#frag")
                  ("data:text/plain,a/b?x=1%23v#old" "#new" "data:text/plain,a/b?x=1%23v#new")
                  ("data:text/plain,abc#old" "#" "data:text/plain,abc#")
                  ("data:text/plain,abc" "#frag ment" "data:text/plain,abc#fragment")
                  ("data:text/plain,abc" "#a%20b" "data:text/plain,abc#a%20b")
                  ("urn:example:animal#old" "#new" "urn:example:animal#new")
                  ("file:///tmp/a#old" "#new" "file:///tmp/a#new")
                  ("https://user:pw@example.com:8443/a?q=1#old" "#new"
                    "https://user:pw@example.com:8443/a?q=1#new")))
    (let* ((original (nth 0 case)) (reference (nth 1 case)) (expected (nth 2 case))
           (base (url-generic-parse-url original))
           (before (url-recreate-url base)))
      (unless (and (equal (url-expand-file-name reference original) expected)
                   (equal (url-expand-file-name reference base) expected)
                   (equal (url-recreate-url base) before))
        (error "Fragment %S lost data or changed its base: %S" reference original))
      (with-temp-buffer
        (setq-local url-current-object base)
        (unless (and (equal (url-expand-file-name reference) expected)
                     (eq url-current-object base) (equal (url-recreate-url base) before))
          (error "Fragment expansion changed the current buffer URL")))))
  (let ((base (url-generic-parse-url "data:text/plain,abc")))
    (cl-letf (((symbol-function 'url-scheme-get-property)
               (lambda (&rest _) (error "Looked up a scheme handler for a fragment"))))
      (unless (equal (url-expand-file-name "#frag" base) "data:text/plain,abc#frag")
        (error "Fragment expansion depended on a transport handler"))))
  (unless (equal (url-expand-file-name "" "data:text/plain,abc#old") "data:text/plain,abc")
    (error "Empty data references still inherited the old fragment"))
  t)
"##;

#[test]
fn fragment_only_references_preserve_opaque_url_payloads() {
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
