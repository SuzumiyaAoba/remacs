use remacs_runtime::lisp::{Flow, Interp};
use std::path::PathBuf;

const CASES: &str = r#"
(progn
  (require 'rfc2231)
  (defun remacs-rfc2231-filename (header &optional strict)
    (rfc2231-get-value (rfc2231-parse-string header strict) 'filename))
  (let* ((plain (remacs-rfc2231-filename
                 "application/octet-stream; filename=plain.txt" t))
         (plain-duplicate (remacs-rfc2231-filename
                           "application/octet-stream; filename=first.txt; filename=second.txt" t))
         (plain-duplicate-reversed (remacs-rfc2231-filename
                                    "application/octet-stream; filename=second.txt; filename=first.txt" t))
         (extended (remacs-rfc2231-filename
                    "application/octet-stream; filename*=utf-8''extended%20name.txt" t))
         (plain-first (remacs-rfc2231-filename
                       "application/octet-stream; filename=plain.txt; filename*=utf-8''extended%20name.txt" t))
         (extended-first (remacs-rfc2231-filename
                          "application/octet-stream; filename*=utf-8''extended%20name.txt; filename=plain.txt" t))
         (fallback-before-continuation
          (remacs-rfc2231-filename
           "application/octet-stream; filename=plain.txt; filename*1*=tail; filename*0*=utf-8''head%20" t))
         (continuation-before-fallback
          (remacs-rfc2231-filename
           "application/octet-stream; filename*0*=utf-8''head%20; filename=plain.txt; filename*1*=tail" t))
         (intermingled-fallback-before-extension
          (let ((parsed
                 (rfc2231-parse-string
                  "application/octet-stream; filename*=utf-8''extended%20name.txt; name*0*=utf-8''label%20; filename=plain.txt; name*1*=tail"
                  t)))
            (list (rfc2231-get-value parsed 'filename)
                  (rfc2231-get-value parsed 'name))))
         (intermingled-extension-before-fallback
          (let ((parsed
                 (rfc2231-parse-string
                  "application/octet-stream; filename=plain.txt; name*0*=utf-8''label%20; filename*=utf-8''extended%20name.txt; name*1*=tail"
                  t)))
            (list (rfc2231-get-value parsed 'filename)
                  (rfc2231-get-value parsed 'name))))
         (other-attribute
          (rfc2231-get-value
           (rfc2231-parse-string
            "application/octet-stream; name=label; filename=plain.txt" t)
           'name))
         (parsed-shape
          (rfc2231-parse-string
           "application/octet-stream; filename=plain.txt; filename*=utf-8''extended%20name.txt" t)))
    (list plain plain-duplicate extended plain-first extended-first
          fallback-before-continuation continuation-before-fallback
          intermingled-fallback-before-extension
          intermingled-extension-before-fallback
          other-attribute parsed-shape plain-duplicate-reversed)))
"#;

fn lisp_string(value: &str) -> String {
    format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
}

fn eval(interp: &mut Interp, source: &str) -> String {
    let value = interp.eval_str(source).unwrap_or_else(|flow| match flow {
        Flow::Signal(signal) => panic!(
            "{} {}",
            interp.prin1_to_string(&signal.symbol()),
            interp.prin1_to_string(&signal.data())
        ),
        other => panic!("{other:?}"),
    });
    format!("{}", interp.prin1_to_string(&value))
}

fn load_source(interp: &mut Interp) {
    let source = PathBuf::from(env!("REMACS_SOURCE_ROOT")).join("lisp/rfc2231.el");
    eval(interp, &format!("(load {} nil t)", lisp_string(source.to_str().unwrap())));
}

#[test]
fn rfc2231_extended_parameter_precedes_plain_fallback_independent_of_order() {
    let mut interp = Interp::new();
    let expected = r#"("plain.txt" "second.txt" "extended name.txt" "extended name.txt" "extended name.txt" "head tail" "head tail" ("extended name.txt" "label tail") ("extended name.txt" "label tail") "label" ("application/octet-stream" (filename . "extended name.txt")) "first.txt")"#;
    assert_eq!(eval(&mut interp, CASES), expected);
    load_source(&mut interp);
    assert_eq!(eval(&mut interp, CASES), expected);
}
