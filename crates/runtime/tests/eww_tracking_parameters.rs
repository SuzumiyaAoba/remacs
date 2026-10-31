use remacs_runtime::lisp::{Flow, Interp};

const CASES: &str = r##"
;;; -*- lexical-binding: t; -*-
(progn
  (require 'eww)
  (dolist (folding '(nil t))
    (let ((case-fold-search folding)
          (eww-url-transformers '(eww-remove-tracking)))
      (dolist (case '(("https://example.test/docs/utm_guide?lang=en"
                       "https://example.test/docs/utm_guide?lang=en")
                      ("https://example.test/page?utm_source=campaign&lang=fr"
                       "https://example.test/page?lang=fr")
                      ("https://example.test/page?lang=fr&utm_source=campaign"
                       "https://example.test/page?lang=fr")
                      ("https://example.test/page?a=1&utm_source=x&b=2&utm_medium=y#Part"
                       "https://example.test/page?a=1&b=2#Part")
                      ("https://example.test/page?utm_source=x&utm_medium=y#Part"
                       "https://example.test/page#Part")
                      ("https://example.test/page?utm_source=x"
                       "https://example.test/page")
                      ("https://example.test/page?utm_source&flag&lang=fr"
                       "https://example.test/page?flag&lang=fr")
                      ("https://example.test/page?UTM_SOURCE=x&lang=fr"
                       "https://example.test/page?lang=fr")
                      ("https://example.test/page?%75tm_source=x&keep=%2f%2F+%20"
                       "https://example.test/page?keep=%2f%2F+%20")
                      ("https://example.test/page?lang=en&utm_x=x&lang=fr"
                       "https://example.test/page?lang=en&lang=fr")
                      ("https://example.test/page?&&utm_x=a&&keep=b&"
                       "https://example.test/page?&&&keep=b&")
                      ("https://example.test/page?utm_x=a#frag?utm_again=b"
                       "https://example.test/page#frag?utm_again=b")
                      ("relative/path?lang=en&utm_source=x#frag"
                       "relative/path?lang=en#frag")
                      ("HTTP://Example.TEST/a%2Fb?utm_x=1&key=%23x#Part"
                       "HTTP://Example.TEST/a%2Fb?key=%23x#Part")
                      ("https://example.test/page#frag?utm_source=x"
                       "https://example.test/page#frag?utm_source=x")
                      ("https://example.test/utm_path?q=utm_source%3Dx&lang=fr#utm_part"
                       "https://example.test/utm_path?q=utm_source%3Dx&lang=fr#utm_part")
                      ("https://example.test/page?q=x;utm_source=y&lang=fr"
                       "https://example.test/page?q=x;utm_source=y&lang=fr")
                      ("https://example.test/page?notutm_source=x&xutm_=y&utm=z"
                       "https://example.test/page?notutm_source=x&xutm_=y&utm=z")
                      ("https://example.test/page?" "https://example.test/page?")
                      ("https://example.test/page" "https://example.test/page")))
        (let* ((input (propertize (car case) 'remacs-test t))
               (snapshot (copy-sequence input))
               (expected (cadr case))
               (actual (eww-remove-tracking input)))
          (unless (and (equal actual expected)
                       (equal (eww--transform-url input) expected)
                       (equal-including-properties input snapshot)
                       (or (not (equal input expected)) (eq actual input)))
            (error "Tracking filter changed unrelated URL data with folding=%S: %S -> %S"
                   folding input actual))))))
  (dolist (url '("https://example.test/?utm_source=x&keep=y"
                 "https://example.test/?keep=y"))
    (string-match "\\(seed\\)" "seed")
    (let ((before (match-data)))
      (eww-remove-tracking url)
      (unless (equal (match-data) before)
        (error "Tracking removal changed caller match data"))))
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
fn tracking_removal_preserves_unrelated_url_components() {
    let mut interp = Interp::new();
    check(&mut interp, CASES);
    check(
        &mut interp,
        &format!(
            "(load {:?} nil t)",
            concat!(env!("REMACS_SOURCE_ROOT"), "/lisp-elc/eww.elc")
        ),
    );
    check(&mut interp, CASES);

    let source = include_str!("../../../lisp/eww.el");
    let start = source.find("(defun eww-remove-tracking ").unwrap();
    let end = source[start..]
        .find("(defun eww--transform-url ")
        .unwrap()
        + start;
    check(&mut interp, &source[start..end]);
    check(&mut interp, CASES);
}
