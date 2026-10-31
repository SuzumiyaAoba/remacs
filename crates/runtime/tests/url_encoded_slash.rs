use remacs_runtime::lisp::{Flow, Interp};

const CASES: &str = r##"
(progn
  (require 'url-expand)
  (dolist (case '((nil "") ("" nil) ("a%2Fb" nil)
                  ("/a%2Fb" "/") ("/a%2fb" "/")
                  ("/x/a%2Fb" "/x/") ("/a%2Fb/c" "/a%2Fb/")
                  ("/a%2F" "/") ("/a%2F/" "/a%2F/")
                  ("/a%252Fb" "/")
                  ("/x/a%2Fb?q=/bad/" "/x/")
                  ("/x/a%2Fb#frag/bad/" "/x/")))
    (unless (and (equal (url-file-directory (car case)) (cadr case))
                 (equal (url-basepath (car case)) (cadr case)))
      (error "Encoded slash changed the directory boundary: %S" case)))
  (dolist (case '(("http://example.com/a%2Fb" "c" "http://example.com/c")
                  ("http://example.com/a%2fb" "c" "http://example.com/c")
                  ("http://example.com/x/a%2Fb" "c" "http://example.com/x/c")
                  ("http://example.com/a%2Fb/c" "d" "http://example.com/a%2Fb/d")
                  ("http://example.com/a%2F" "c" "http://example.com/c")
                  ("http://example.com/a%2F/" "c" "http://example.com/a%2F/c")
                  ("http://example.com/a%252Fb" "c" "http://example.com/c")
                  ("http://example.com/x/a%2Fb?q=/bad/#old" "c?x=%2F#new"
                    "http://example.com/x/c?x=%2F#new")
                  ("https://example.com:8443/x%2Fy/a%2Fb?q=old#old" "../c"
                    "https://example.com:8443/c")
                  ("https://example.com/x/a%2Fb" "c%2Fd" "https://example.com/x/c%2Fd")
                  ("file:///a%2Fb" "c" "file:///c")
                  ("https://example.com/a%2Fb?q=old#old" "?new"
                    "https://example.com/a%2Fb?new")))
    (let* ((original (nth 0 case)) (reference (nth 1 case)) (expected (nth 2 case))
           (base (url-generic-parse-url original)) (before (url-recreate-url base)))
      (unless (and (equal (url-expand-file-name reference original) expected)
                   (equal (url-expand-file-name reference base) expected)
                   (equal (url-recreate-url base) before))
        (error "Relative reference %S merged at an encoded slash in %S" reference original))))
  t)
"##;

fn check(interp: &mut Interp, source: &str) {
    match interp.eval_str(source) {
        Ok(value) => assert!(value.truthy()),
        Err(Flow::Signal(signal)) => panic!(
            "{} {}",
            interp.prin1_to_string(&signal.symbol()),
            interp.prin1_to_string(&signal.data())
        ),
        Err(other) => panic!("{other:?}"),
    }
}

#[test]
fn encoded_slashes_stay_inside_url_path_segments() {
    let mut interp = Interp::new();
    check(&mut interp, CASES);
    check(
        &mut interp,
        &format!(
            "(load {:?} nil t)",
            concat!(env!("REMACS_SOURCE_ROOT"), "/lisp-elc/url-util.elc")
        ),
    );
    check(&mut interp, CASES);
    check(&mut interp, r#"(load "url-util.el" nil t)"#);
    check(&mut interp, CASES);
}
