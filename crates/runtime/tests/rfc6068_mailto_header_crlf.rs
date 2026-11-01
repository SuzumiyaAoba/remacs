use remacs_runtime::lisp::{Flow, Interp};
use std::path::PathBuf;

const CASES: &str = r##"(progn
  (require 'cl-lib)
  (require 'rfc6068)
  (require 'browse-url)
  (defvar mail-header-separator "--MAIL-HEADER-SEPARATOR--")
  (defvar remacs-rfc6068-compose-calls 0)
  (defvar remacs-rfc6068-compose-args nil)
  (defun remacs-rfc6068-assert (condition message)
    (unless condition (error "%s" message)))
  (defun remacs-rfc6068-compose-capture (&rest args)
    ;; Capture the compose-mail boundary only; never initialize an MUA or send.
    (setq remacs-rfc6068-compose-calls
          (1+ remacs-rfc6068-compose-calls)
          remacs-rfc6068-compose-args (copy-tree args))
    (let ((to (nth 0 args))
          (subject (nth 1 args)))
      (erase-buffer)
      (insert "To: " (or to "") "\n")
      (when subject (insert "Subject: " subject "\n"))
      (insert mail-header-separator "\n")
      (goto-char (point-min)))
    t)
  (defun remacs-rfc6068-rejects-p (url)
    (condition-case nil
        (progn (rfc6068-parse-mailto-url url) nil)
      (error t)))
  (defun remacs-rfc6068-check-parser ()
    (remacs-rfc6068-assert
     (equal (rfc6068-parse-mailto-url
             "mailto:to%40example.org?subject=caf%C3%A9&body=one%0D%0Atwo")
            '(("To" . "to@example.org")
              ("Subject" . "café")
              ("Body" . "one\ntwo")))
     "safe UTF-8 subject or encoded Body line break changed")
    (remacs-rfc6068-assert
     (remacs-rfc6068-rejects-p
      "mailto:to%40example.org?subject=safe%0D%0ABcc%3Aevil%40example.net")
     "decoded CRLF in a non-Body header value was accepted")
    (remacs-rfc6068-assert
     (remacs-rfc6068-rejects-p
      "mailto:to%40example.org?X-Test%0D%0ABcc=evil")
     "decoded CRLF in a header name was accepted")
    (remacs-rfc6068-assert
     (remacs-rfc6068-rejects-p
      "mailto:to%40example.org?cc=copy%0ABcc%3Aevil")
     "decoded LF in a non-Body header value was accepted")
    (remacs-rfc6068-assert
     (remacs-rfc6068-rejects-p
      "mailto:to%0D%0ABcc%3Aevil%40example.org")
     "decoded CRLF in the address field was accepted")
    (dolist (url '("mailto:to%40example.org?subject=safe%0DBcc%3Aevil"
                   "mailto:to%40example.org?X%0DName=value"
                   "mailto:to%40example.org?BoDy%0AName=value"
                   "mailto:to%0DBcc%3Aevil%40example.org"))
      (remacs-rfc6068-assert (remacs-rfc6068-rejects-p url)
                             "a decoded bare CR or header-name LF was accepted"))
    (remacs-rfc6068-assert
     (equal (cdr (assoc "Body" (rfc6068-parse-mailto-url
                                "mailto:to%40example.org?bOdY=one%0D%0Atwo")))
            "one\ntwo")
     "case-insensitive Body field stopped allowing multiline content")
    t)
  (defun remacs-rfc6068-check-compose-boundary ()
    (let ((mail-header-separator "--MAIL-HEADER-SEPARATOR--"))
      ;; An unsafe subject must stop before compose-mail receives any headers.
      (with-temp-buffer
        (insert "untouched")
        (let ((before (buffer-string))
              (remacs-rfc6068-compose-calls 0)
              (remacs-rfc6068-compose-args nil))
          (cl-letf (((symbol-function 'compose-mail)
                     #'remacs-rfc6068-compose-capture)
                    ((symbol-function 'compose-mail-other-window)
                     #'remacs-rfc6068-compose-capture))
            (remacs-rfc6068-assert
             (condition-case nil
                 (progn
                   (browse-url-mail
                    "mailto:to%40example.org?subject=safe%0D%0ABcc%3Aevil%40example.net")
                   nil)
               (error t))
             "browse-url-mail accepted a header-injecting subject")
            (remacs-rfc6068-assert
             (= remacs-rfc6068-compose-calls 0)
             "unsafe mailto reached the compose-mail boundary")
            (remacs-rfc6068-assert
             (equal before (buffer-string))
             "rejecting an unsafe mailto modified the caller buffer"))))
      ;; A valid UTF-8 subject, ordinary Cc, and CRLF Body reach compose safely.
      (with-temp-buffer
        (setq remacs-rfc6068-compose-calls 0
              remacs-rfc6068-compose-args nil)
        (cl-letf (((symbol-function 'compose-mail)
                   #'remacs-rfc6068-compose-capture)
                  ((symbol-function 'compose-mail-other-window)
                   #'remacs-rfc6068-compose-capture))
          (browse-url-mail
           "mailto:to%40example.org?subject=caf%C3%A9&cc=copy%40example.net&body=one%0D%0Atwo%20caf%C3%A9")
          (remacs-rfc6068-assert
           (= remacs-rfc6068-compose-calls 1)
           "valid mailto did not reach compose exactly once")
          (remacs-rfc6068-assert
           (equal (list (nth 0 remacs-rfc6068-compose-args)
                        (nth 1 remacs-rfc6068-compose-args)
                        (nth 2 remacs-rfc6068-compose-args))
                  '("to@example.org" "café" (("Cc" . "copy@example.net"))))
           "compose-mail arguments changed for valid UTF-8/header fields")
          (remacs-rfc6068-assert
           (not (assoc "Body" (nth 2 remacs-rfc6068-compose-args)))
           "Body pseudo-header was forwarded as a real header")
          (let ((body-start
                 (save-excursion
                   (goto-char (point-min))
                   (search-forward
                    (concat "\n" mail-header-separator "\n") nil t))))
            (remacs-rfc6068-assert
             (and body-start
                  (equal (buffer-substring-no-properties body-start (point-max))
                         "one\ntwo café\n"))
             "encoded Body line breaks or UTF-8 were not preserved"))))
      t))
  (remacs-rfc6068-assert (remacs-rfc6068-check-parser)
                         "RFC 6068 parser CRLF regression")
  (remacs-rfc6068-assert (remacs-rfc6068-check-compose-boundary)
                         "mailto compose boundary CRLF regression")
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

fn run_with_libraries() -> String {
    let root = PathBuf::from(env!("REMACS_SOURCE_ROOT"));
    let mut interp = Interp::new();
    for relative in ["lisp/rfc6068.el", "lisp/browse-url.el"] {
        let path = root.join(relative);
        eval(
            &mut interp,
            &format!("(load {} nil t)", lisp_string(path.to_str().unwrap())),
        );
    }
    eval(&mut interp, CASES)
}

#[test]
fn rfc6068_rejects_header_crlf_but_keeps_body_crlf() {
    assert_eq!(run_with_libraries(), "t");
}
