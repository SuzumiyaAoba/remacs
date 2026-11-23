use remacs_runtime::lisp::{Flow, Interp};
use std::path::PathBuf;

const CASES: &str = r###"(progn
  (defvar remacs-2970-duplicate-query nil)
  (defvar remacs-2970-unique-query nil)
  (defun url-unhex-string (string &optional _allow-newlines)
    string)
  (defun remacs-2970-check (label condition)
    (unless condition
      (error "url-parse-query-string #2970 failed: %S" label))
    t)
  (remacs-2970-check
   'interleaved-key-order
   (equal (url-parse-query-string "x=1&y=2&x=3")
          '(("x" "1" "3") ("y" "2"))))
  (let* ((remacs-2970-duplicate-query
          (mapconcat (lambda (_) "x=1") (make-list 10000 nil) "&"))
         (result (url-parse-query-string remacs-2970-duplicate-query))
         (entry (car result))
         (values (cdr entry)))
    (remacs-2970-check
     'large-duplicate-values
     (and (= (length result) 1)
          (equal (car entry) "x")
          (= (length values) 10000)
          (equal (car values) "1")
          (equal (car (last values)) "1"))))
  (let* ((remacs-2970-unique-query
          (mapconcat (lambda (n) (format "k%d=v%d" n n))
                     (number-sequence 0 4999) "&"))
         (result (url-parse-query-string remacs-2970-unique-query))
         (last-entry (car (last result))))
    (remacs-2970-check
     'large-distinct-key-order
     (and (= (length result) 5000)
          (equal (caar result) "k0")
          (equal (car last-entry) "k4999")
          (equal (cadr last-entry) "v4999"))))
  t)"###;

#[test]
fn query_string_aggregation_preserves_order_for_large_inputs() {
    let source = PathBuf::from(env!("REMACS_SOURCE_ROOT"))
        .join("lisp/url-util.el")
        .canonicalize()
        .unwrap();
    let source = std::fs::read_to_string(source).unwrap();
    let start = source.find("(defun url-parse-query-string").unwrap();
    let end = start + source[start..].find("\n(defun url-unhex").unwrap();
    let definition = &source[start..end];
    let code = format!("(progn {} {})", definition, CASES);
    let mut interp = Interp::new();
    let result = interp.eval_str(&code).unwrap_or_else(|flow| match flow {
        Flow::Signal(signal) => panic!(
            "{} {}",
            interp.prin1_to_string(&signal.symbol()),
            interp.prin1_to_string(&signal.data())
        ),
        other => panic!("{other:?}"),
    });
    assert!(result.truthy());
}
