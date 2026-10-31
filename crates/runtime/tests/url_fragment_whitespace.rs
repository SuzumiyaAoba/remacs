use remacs_runtime::lisp::{Flow, Interp};

const CASES: &str = r##"
(progn
  (require 'url-expand)
  (dolist (case '(("#frag with space" "https://example.com/a"
                    "https://example.com/a#fragwithspace")
                  ("#a\nb\rc d" "https://example.com/a?q=1#old"
                    "https://example.com/a?q=1#abcd")
                  ("#frag ment" "https://example.com:8443/a?q=1#old"
                    "https://example.com:8443/a?q=1#fragment")
                  ("#a%20b" "https://example.com/a" "https://example.com/a#a%20b")
                  ("#path /query ?value" "https://example.com/a"
                    "https://example.com/a#path/query?value")
                  (" \r\n#frag ment" "https://example.com/a"
                    "https://example.com/a#fragment")
                  ("b c" "https://example.com/dir/a" "https://example.com/dir/bc")
                  ("https://other.example/a b#c d" "https://example.com/a"
                    "https://other.example/ab#cd")))
    (let ((actual (url-expand-file-name (nth 0 case) (nth 1 case))))
      (unless (equal actual (nth 2 case))
        (error "URL %S: expected %S, got %S" (car case) (nth 2 case) actual))))
  (let* ((input (propertize "#frag ment" 'face 'bold))
         (base (url-generic-parse-url "https://example.com/a?q=1#old"))
         (before (url-recreate-url base)))
    (unless (and (equal (url-expand-file-name input base) "https://example.com/a?q=1#fragment")
                 (equal input "#frag ment") (eq (get-text-property 0 'face input) 'bold)
                 (equal (url-recreate-url base) before) (equal (url-target base) "old"))
      (error "Fragment cleanup changed the caller's input or base object"))
    (with-temp-buffer
      (setq-local url-current-object base)
      (unless (equal (url-expand-file-name "#frag ment") "https://example.com/a?q=1#fragment")
        (error "Fragment cleanup ignored the current buffer URL")))
    (unless (equal (url-expand-file-name "# \r\n " base) (url-expand-file-name "#" base))
      (error "Whitespace-only fragments were not normalized consistently")))
  t)
"##;

#[test]
fn url_fragment_references_use_the_existing_whitespace_cleanup() {
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
