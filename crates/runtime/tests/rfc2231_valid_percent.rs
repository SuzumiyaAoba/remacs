use remacs_runtime::lisp::{Flow, Interp};
use std::path::PathBuf;

const CASES: &str = r##"(progn
  (require 'rfc2231)
  (dolist (value '("" "plain" "%00" "%20%41%aF" "a%25b"))
    (unless (rfc2231--valid-percent-escapes-p value)
      (error "Valid RFC 2231 hex octets rejected: %S" value)))
  (dolist (value '("%" "%0" "%GG" "%1z" "%41%"))
    (when (rfc2231--valid-percent-escapes-p value)
      (error "Invalid RFC 2231 hex octet accepted: %S" value)))
  (unless (equal (rfc2231-decode-encoded-string "utf-8''caf%C3%A9%20menu" t)
                 "café menu")
    (error "Strict RFC 2231 decoding changed"))
  (unless (equal (rfc2231-get-value
                  (rfc2231-parse-string
                   "application/octet-stream; filename*=utf-8''a%20b.txt" t)
                  'filename)
                 "a b.txt")
    (error "Strict header parsing rejected a valid hex octet"))
  (unless (equal (rfc2231-decode-encoded-string "utf-8''bad%QQ") "bad%QQ")
    (error "Liberal malformed-octet handling changed"))
  (unless (condition-case nil
              (progn (rfc2231-decode-encoded-string "utf-8''bad%QQ" t) nil)
            (error t))
    (error "Strict malformed-octet rejection changed"))
  t)"##;

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
fn rfc2231_strict_accepts_valid_percent_octets_and_rejects_malformed_ones() {
    let mut interp = Interp::new();
    check(&mut interp, CASES);
    let source = PathBuf::from(env!("REMACS_SOURCE_ROOT")).join("lisp/rfc2231.el");
    let filename = source
        .to_str()
        .unwrap()
        .replace('\\', "\\\\")
        .replace('"', "\\\"");
    check(&mut interp, &format!("(progn (load \"{filename}\" nil t t) t)"));
    check(&mut interp, CASES);
}
