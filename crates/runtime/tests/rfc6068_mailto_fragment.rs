use remacs_runtime::lisp::{Flow, Interp};
use std::path::PathBuf;

const CASES: &str = r##"(progn
  (require 'cl-lib)
  (require 'rfc6068)
  (require 'browse-url)
  (defvar mail-header-separator "--MAIL-HEADER-SEPARATOR--")
  (defvar remacs-1647-compose-calls 0)
  (defvar remacs-1647-compose-args nil)
  (defun remacs-1647-assert (condition message)
    (unless condition (error "%s" message)))
  (defun remacs-1647-compose-capture (&rest args)
    ;; Capture the public composition boundary; never invoke an MUA or send.
    (setq remacs-1647-compose-calls (1+ remacs-1647-compose-calls)
          remacs-1647-compose-args (copy-tree args))
    (erase-buffer)
    (insert "To: " (or (nth 0 args) "") "\n")
    (when (nth 1 args)
      (insert "Subject: " (nth 1 args) "\n"))
    (insert mail-header-separator "\n")
    (goto-char (point-min))
    t)
  (remacs-1647-assert
   (equal (rfc6068-parse-mailto-url
           "mailto:to%40example.org#section?subject=ignored")
          '(("To" . "to@example.org")))
   "a raw fragment after the address leaked its query into parsing")
  (remacs-1647-assert
   (equal (rfc6068-parse-mailto-url
           "mailto:to%40example.org?subject=kept&body=before%23after#section&from=forged")
          '(("To" . "to@example.org")
            ("Subject" . "kept")
            ("Body" . "before#after")))
   "raw fragment was parsed or encoded %23 stopped being data")
  (remacs-1647-assert
   (equal (rfc6068-parse-mailto-url
           "mailto:alice@example.net?subject=hello#section")
          '(("To" . "alice@example.net") ("Subject" . "hello")))
   "the original raw Subject fragment was not ignored")
  (remacs-1647-assert
   (equal (rfc6068-parse-mailto-url
           "mailto:to%23tag%40example.org?subject=caf%C3%A9%23data#ignored")
          '(("To" . "to#tag@example.org") ("Subject" . "café#data")))
   "encoded # in address/UTF-8 Subject was treated as a fragment")
  (with-temp-buffer
    (let ((mail-header-separator "--MAIL-HEADER-SEPARATOR--"))
      (setq remacs-1647-compose-calls 0
            remacs-1647-compose-args nil)
      (cl-letf (((symbol-function 'compose-mail)
                 #'remacs-1647-compose-capture)
                ((symbol-function 'compose-mail-other-window)
                 #'remacs-1647-compose-capture))
        (browse-url-mail
         "mailto:to%40example.org?subject=kept&body=before%23after#section?subject=evil&from=forged")
        (remacs-1647-assert (= remacs-1647-compose-calls 1)
                            "valid mailto did not reach compose once")
        (remacs-1647-assert
         (equal (list (nth 0 remacs-1647-compose-args)
                      (nth 1 remacs-1647-compose-args)
                      (nth 2 remacs-1647-compose-args))
                '("to@example.org" "kept" nil))
         "query fields after a raw fragment reached compose-mail")
        (let ((body-start
               (save-excursion
                 (goto-char (point-min))
                 (search-forward
                  (concat "\n" mail-header-separator "\n") nil t))))
          (remacs-1647-assert
           (and body-start
                (equal (buffer-substring-no-properties body-start (point-max))
                       "before#after\n"))
           "encoded # body data or fragment cutoff changed at compose boundary")))))
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
fn rfc6068_ignores_raw_mailto_fragments() {
    let root = PathBuf::from(env!("REMACS_SOURCE_ROOT"));
    let mut interp = Interp::new();
    for relative in ["lisp/rfc6068.el", "lisp/browse-url.el"] {
        let path = root.join(relative);
        eval(
            &mut interp,
            &format!("(load {} nil t)", lisp_string(path.to_str().unwrap())),
        );
    }
    assert_eq!(eval(&mut interp, CASES), "t");
}
