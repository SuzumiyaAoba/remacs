use remacs_runtime::lisp::{Flow, Interp};

const CASES: &str = r##"
;;; -*- lexical-binding: t; -*-
(progn
  (require 'eww)
  (require 'cl-lib)
  (dolist (case '(("?q=2#part" "https://example.test/base/page?old=1#old"
                    "https://example.test/base/page?q=2#part")
                  ("next" "https://example.test/base/page?old=1#old"
                    "https://example.test/base/next")
                  ("../next" "https://example.test/base/page" "https://example.test/next")
                  ("child" "https://example.test/base/page?x=/bad/" "https://example.test/base/child")
                  ("child" "file:///tmp/base/page?x=/bad/" "file:///tmp/base/child")
                  ("c" "https://example.test/a%2Fb" "https://example.test/c")
                  ("#part" "https://example.test/base/page?old=1#old"
                    "https://example.test/base/page?old=1#part")
                  ("" "https://example.test/base/page?old=1#old"
                    "https://example.test/base/page?old=1")))
    (unless (and (equal (shr-expand-url (nth 0 case) (nth 1 case)) (nth 2 case))
                 (let ((shr-base (shr-parse-base (nth 1 case))))
                   (equal (shr-expand-url (nth 0 case)) (nth 2 case))))
      (error "SHR lost the full base URL when resolving %S" case)))
  (defun remacs-test-eww-submit-query (action method value expected &optional empty)
    (with-temp-buffer
      (let* ((base "https://example.test/base/page?old=1#old")
             (form (list (cons :method method) (cons :action action)))
             (input (list :eww-form form :type "text"
                          :name (unless empty "q") :value value))
             (eww-data (list :url base))
             (shr-base base)
             (url-request-method nil)
             (url-request-data nil)
             (url-request-extra-headers nil)
             (before (copy-sequence action))
             observed)
        (insert "input")
        (put-text-property (point-min) (point-max) 'eww-form input)
        (goto-char (point-min))
        (cl-letf (((symbol-function 'eww-browse-url)
                   (lambda (url &rest _)
                     (setq observed
                           (list url url-request-method url-request-data
                                 url-request-extra-headers)))))
          (eww-submit))
        (unless (and observed (or (null expected) (equal (car observed) expected))
                     (equal action before) (eq (cdr (assq :action form)) action)
                     (eq (plist-get eww-data :url) base)
                     (null url-request-method) (null url-request-data)
                     (null url-request-extra-headers))
          (error "Form action %S did not retain its components/state: %S" action observed))
        (if (equal method "POST")
            (unless (equal (cdr observed)
                           '("POST" "q=hello"
                             (("Content-Type" . "application/x-www-form-urlencoded"))))
              (error "GET query mutation changed POST request state: %S" observed))
          (unless (equal (cdr observed) '(nil nil nil))
            (error "GET form changed request state: %S" observed)))
        (car observed))))
  (dolist (case '(("https://example.test/search?lang=en#results"
                   "https://example.test/search?q=hello#results")
                  ("https://example.test/search#results"
                   "https://example.test/search?q=hello#results")
                  ("https://example.test/search?lang=en" "https://example.test/search?q=hello")
                  ("https://example.test/search" "https://example.test/search?q=hello")
                  ("search?lang=en#results" "https://example.test/base/search?q=hello#results")
                  ("#results" "https://example.test/base/page?q=hello#results")
                  ("?lang=en#results" "https://example.test/base/page?q=hello#results")
                  ("https://example.test/search?#" "https://example.test/search?q=hello#")
                  ("https://user:pw@example.test:8443/a%23b?old=1#res%3Fults"
                   "https://user:pw@example.test:8443/a%23b?q=hello#res%3Fults")))
    (dolist (method '("get" "GET" nil))
      (remacs-test-eww-submit-query (car case) method "hello" (cadr case))))
  (remacs-test-eww-submit-query "https://example.test/search?old=1#results"
                                "GET" "" "https://example.test/search?#results" t)
  (let* ((opened (remacs-test-eww-submit-query
                  "https://example.test/search?old=1#results" "GET" "x#y?z&=" nil))
         (parsed (url-generic-parse-url opened))
         (path-query (url-path-and-query parsed)))
    (unless (and (equal (car path-query) "/search")
                 (equal (url-parse-query-string (cdr path-query)) '(("q" "x#y?z&=")))
                 (equal (url-target parsed) "results"))
      (error "Encoded form data escaped its query component: %S" opened)))
  (remacs-test-eww-submit-query "https://example.test/search?old=1#results"
                                "POST" "hello" "https://example.test/search?old=1#results")
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
fn get_form_data_replaces_the_query_and_preserves_the_fragment() {
    let mut interp = Interp::new();
    check(&mut interp, CASES);
    for library in ["shr", "eww"] {
        let path = format!("{}/lisp-elc/{library}.elc", env!("REMACS_SOURCE_ROOT"));
        check(&mut interp, &format!("(load {path:?} nil t)"));
    }
    check(&mut interp, CASES);

    for (source, name, next) in [
        (
            include_str!("../../../lisp/shr.el"),
            "shr-expand-url",
            "shr-ensure-newline",
        ),
        (
            include_str!("../../../lisp/eww.el"),
            "eww-submit",
            "eww-browse-with-external-browser",
        ),
    ] {
        let start = source.find(&format!("(defun {name} ")).unwrap();
        let end = source[start..]
            .find(&format!("(defun {next} "))
            .unwrap()
            + start;
        check(&mut interp, &source[start..end]);
    }
    check(&mut interp, CASES);
}
