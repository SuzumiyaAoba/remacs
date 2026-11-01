use remacs_runtime::lisp::{Flow, Interp};
use std::path::PathBuf;

const CASES: &str = r#"
(progn
  (require 'rfc2231)
  (defun remacs-rfc2231-filename (header &optional strict)
    (rfc2231-get-value (rfc2231-parse-string header strict) 'filename))
  (let* ((bad-prefix
          (condition-case err
              (remacs-rfc2231-filename
               "application/octet-stream; filename*0=plain; filename*1*=utf-8''suffix" t)
            (error (list (car err) (cadr err)))))
         (liberal-prefix
          (remacs-rfc2231-filename
           "application/octet-stream; filename*0=plain; filename*1*=utf-8''suffix"))
         (bad-quoted-prefix
          (condition-case err
              (remacs-rfc2231-filename
               "application/octet-stream; filename*0=plain; filename*1*=\"utf-8''suffix\"" t)
            (error (list (car err) (cadr err)))))
         (liberal-quoted-prefix
          (remacs-rfc2231-filename
           "application/octet-stream; filename*0=plain; filename*1*=\"utf-8''suffix\""))
         (encoded-apostrophe-data
          (remacs-rfc2231-filename
           "application/octet-stream; filename*0=plain; filename*1*=utf-8%27%27suffix" t))
         (out-of-order
          (remacs-rfc2231-filename
           "application/octet-stream; filename*1*=two; filename*0*=utf-8''caf%C3%A9%20" t))
         (mixed-encoding
          (remacs-rfc2231-filename
           "application/octet-stream; filename*1=tail; filename*0*=utf-8''first%20" t))
         (plain-continuation
          (remacs-rfc2231-filename
           "application/octet-stream; filename*1=two; filename*0=one" t))
         (plain-value
          (remacs-rfc2231-filename
           "application/octet-stream; filename=plain.txt" t))
         (bad-percent
          (condition-case err
              (remacs-rfc2231-filename
               "application/octet-stream; filename*=utf-8''bad%QQ" t)
            (error (car err))))
         (missing-number
          (condition-case err
              (remacs-rfc2231-filename
               "application/octet-stream; filename*0*=utf-8''first; filename*2*=third" t)
            (error (car err)))))
    (list bad-prefix liberal-prefix bad-quoted-prefix liberal-quoted-prefix
          encoded-apostrophe-data out-of-order mixed-encoding
          plain-continuation plain-value bad-percent missing-number)))
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
fn rfc2231_strict_rejects_charset_prefixes_after_continuation_segment_zero() {
    let mut interp = Interp::new();
    let expected = r#"((error "RFC 2231 charset prefix outside first segment for filename") "suffix" (error "RFC 2231 charset prefix outside first segment for filename") "plainutf-8''suffix" "plainutf-8''suffix" "café two" "first tail" "onetwo" "plain.txt" error error)"#;
    assert_eq!(eval(&mut interp, CASES), expected);
    load_source(&mut interp);
    assert_eq!(eval(&mut interp, CASES), expected);
}
