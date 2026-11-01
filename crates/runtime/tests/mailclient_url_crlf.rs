use remacs_runtime::lisp::{Flow, Interp};
use std::path::PathBuf;

const CASES: &str = r##"(progn
  (require 'mailclient)
  (defun remacs-2477-assert (condition message)
    (unless condition (error "%s" message)))
  (remacs-2477-assert
   (equal (mailclient-encode-string-as-url (concat "a" (string 13 10) "b"))
          "a%0D%0Ab")
   "CRLF was encoded twice instead of one URL newline")
  (remacs-2477-assert
   (equal (mailclient-encode-string-as-url (concat "a" (string 10) "b"))
          "a%0D%0Ab")
   "LF-only input stopped becoming a CRLF URL newline")
  (remacs-2477-assert
   (equal (mailclient-encode-string-as-url (concat "a" (string 13) "b"))
          "a%0db")
   "bare CR encoding changed")
  (remacs-2477-assert
   (equal (mailclient-encode-string-as-url "é% z") "%c3%a9%25%20z")
   "UTF-8, percent, or space escaping changed")
  t)"##;

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

#[test]
fn mailclient_encodes_crlf_as_one_url_newline() {
    let root = PathBuf::from(env!("REMACS_SOURCE_ROOT"));
    let mut interp = Interp::new();
    let path = root.join("lisp/mailclient.el");
    eval(
        &mut interp,
        &format!("(load {} nil t)", lisp_string(path.to_str().unwrap())),
    );
    assert_eq!(eval(&mut interp, CASES), "t");
}
