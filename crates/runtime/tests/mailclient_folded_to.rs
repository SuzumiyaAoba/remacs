use remacs_runtime::lisp::{Flow, Interp};
use std::path::PathBuf;

const CASES: &str = r##"(progn
  (require 'mailclient)
  (defun remacs-2478-assert (condition message)
    (unless condition (error "%s" message)))
  (defun remacs-2478-check-to-field (field expected)
    (with-temp-buffer
      (insert "To:" field "\n\nbody\n")
      (let ((mailclient-delim-static "?"))
        (equal (mailclient-gather-addresses "To") expected))))
  (remacs-2478-assert
   (remacs-2478-check-to-field
    " first@example.org,\n\tsecond@example.org"
    "?To=first%40example.org&To=second%40example.org")
   "LF/tab-folded To field leaked a line break or failed to split recipients")
  (remacs-2478-assert
   (remacs-2478-check-to-field
    " first@example.org,\r\n  second@example.org"
    "?To=first%40example.org&To=second%40example.org")
   "CRLF/space-folded To field leaked a line break or failed to split recipients")
  (remacs-2478-assert
   (remacs-2478-check-to-field
    " \"local\r\n  part\"@example.org,\tsecond@example.org"
    "?To=%22local%20%20part%22%40example.org&To=second%40example.org")
   "unfolding collapsed meaningful quoted local-part whitespace")
  ;; Address unfolding does not modify the independent URL body line rules.
  (remacs-2478-assert
   (equal (mailclient-encode-string-as-url "body\nline") "body%0D%0Aline")
   "address unfolding changed body newline encoding")
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
fn mailclient_unfolds_folded_to_addresses_without_collapsing_wsp() {
    let root = PathBuf::from(env!("REMACS_SOURCE_ROOT"));
    let mut interp = Interp::new();
    let path = root.join("lisp/mailclient.el");
    eval(
        &mut interp,
        &format!("(load {} nil t)", lisp_string(path.to_str().unwrap())),
    );
    assert_eq!(eval(&mut interp, CASES), "t");
}
