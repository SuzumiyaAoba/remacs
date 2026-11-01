use remacs_runtime::lisp::{Flow, Interp};
use std::path::PathBuf;

const CASES: &str = r##"(progn
  (require 'cl-lib)
  (require 'rfc6068)
  (require 'browse-url)
  (defvar mail-header-separator "--MAIL-HEADER-SEPARATOR--")
  (defvar remacs-browse-url-1646-compose-calls 0)
  (defvar remacs-browse-url-1646-compose-args nil)
  (defun remacs-browse-url-1646-assert (condition message)
    (unless condition (error "%s" message)))
  (defun remacs-browse-url-1646-compose-capture (&rest args)
    ;; Capture compose-mail arguments and build a harmless compose buffer.
    ;; No MUA/send function is called by this fixture.
    (setq remacs-browse-url-1646-compose-calls
          (1+ remacs-browse-url-1646-compose-calls)
          remacs-browse-url-1646-compose-args (copy-tree args))
    (erase-buffer)
    (insert "To: " (or (nth 0 args) "") "\n")
    (when (nth 1 args)
      (insert "Subject: " (nth 1 args) "\n"))
    (insert mail-header-separator "\n")
    (goto-char (point-min))
    t)
  (defun remacs-browse-url-1646-rejects-p (url)
    (condition-case nil
        (progn (rfc6068-parse-mailto-url url) nil)
      (error t)))
  (defun remacs-browse-url-1646-normalize-header-names (headers)
    (mapcar (lambda (header)
              (cons (downcase (car header)) (cdr header)))
            headers))
  (defun remacs-browse-url-1646-check-parser-crlf ()
    (remacs-browse-url-1646-assert
     (remacs-browse-url-1646-rejects-p
      "mailto:?subject=safe%0D%0ABcc%3Aevil%40example.net")
     "rfc6068 parser accepted a header-injecting Subject")
    (remacs-browse-url-1646-assert
     (remacs-browse-url-1646-rejects-p
      "mailto:?X-Test%0D%0ABcc=evil")
     "rfc6068 parser accepted a CRLF header name")
    (remacs-browse-url-1646-assert
     (remacs-browse-url-1646-rejects-p
      "mailto:?cc=safe%0ABcc%3Aevil")
     "rfc6068 parser accepted an LF non-Body header value")
    (remacs-browse-url-1646-assert
     (equal (rfc6068-parse-mailto-url
             "mailto:?subject=caf%C3%A9&body=one%0D%0Atwo")
            '(("Subject" . "café") ("Body" . "one\ntwo")))
     "parser changed UTF-8 Subject or multiline Body behavior")
    t)
  (defun remacs-browse-url-1646-check-compose-filter ()
    (let ((mail-header-separator "--MAIL-HEADER-SEPARATOR--"))
      (with-temp-buffer
        (setq remacs-browse-url-1646-compose-calls 0
              remacs-browse-url-1646-compose-args nil)
        (cl-letf (((symbol-function 'compose-mail)
                   #'remacs-browse-url-1646-compose-capture)
                  ((symbol-function 'compose-mail-other-window)
                   #'remacs-browse-url-1646-compose-capture))
          (browse-url-mail
           "mailto:to%40example.org?subject=caf%C3%A9&body=one%0D%0Atwo%20caf%C3%A9&cc=copy%40example.net&bcc=blind%40example.net&in-reply-to=msg%40example.net&references=ref%40example.net&keywords=tag&%46rom=forged%40evil.example&Date=forged&Sender=forged&Reply-To=forged&Apparently-To=forged&Resent-From=forged&Resent-To=forged&Received=forged&Return-Path=forged&MIME-Version=1.0&cOnTeNt-Type=text%2Fhtml&Content-Transfer-Encoding=base64")
          (remacs-browse-url-1646-assert
           (= remacs-browse-url-1646-compose-calls 1)
           "safe mailto fields did not reach compose exactly once")
          (remacs-browse-url-1646-assert
           (equal (list (nth 0 remacs-browse-url-1646-compose-args)
                        (nth 1 remacs-browse-url-1646-compose-args)
                        (remacs-browse-url-1646-normalize-header-names
                         (nth 2 remacs-browse-url-1646-compose-args)))
                  (list "to@example.org" "café"
                        '(("cc" . "copy@example.net")
                          ("bcc" . "blind@example.net")
                          ("in-reply-to" . "msg@example.net")
                          ("references" . "ref@example.net")
                          ("keywords" . "tag"))))
           "compose-mail received unsafe or lost safe header fields")
          (remacs-browse-url-1646-assert
           (not (member "from"
                        (mapcar (lambda (header) (downcase (car header)))
                                (nth 2 remacs-browse-url-1646-compose-args))))
           "forged From header reached compose-mail")
          (let ((body-start
                 (save-excursion
                   (goto-char (point-min))
                   (search-forward
                    (concat "\n" mail-header-separator "\n") nil t))))
            (remacs-browse-url-1646-assert
             (and body-start
                  (equal (buffer-substring-no-properties body-start (point-max))
                         "one\ntwo café\n"))
             "Body CRLF/UTF-8 did not remain in the body section"))))
      ;; Exact issue shape: unsafe fields alone are discarded before compose.
      (with-temp-buffer
        (setq remacs-browse-url-1646-compose-calls 0
              remacs-browse-url-1646-compose-args nil)
        (cl-letf (((symbol-function 'compose-mail)
                   #'remacs-browse-url-1646-compose-capture)
                  ((symbol-function 'compose-mail-other-window)
                   #'remacs-browse-url-1646-compose-capture))
          (browse-url-mail
           "mailto:?from=forged%40evil.example&content-type=text%2Fhtml")
          (remacs-browse-url-1646-assert
           (= remacs-browse-url-1646-compose-calls 1)
           "exact unsafe-header reproduction did not reach compose")
          (remacs-browse-url-1646-assert
           (equal (list (nth 0 remacs-browse-url-1646-compose-args)
                        (nth 1 remacs-browse-url-1646-compose-args)
                        (nth 2 remacs-browse-url-1646-compose-args))
                  '(nil nil nil))
           "unsafe-only mailto leaked headers into compose")))
      ;; The #3026 parser guard remains in force at this composition boundary.
      (with-temp-buffer
        (insert "untouched")
        (let ((before (buffer-string))
              (remacs-browse-url-1646-compose-calls 0)
              (remacs-browse-url-1646-compose-args nil))
          (cl-letf (((symbol-function 'compose-mail)
                     #'remacs-browse-url-1646-compose-capture)
                    ((symbol-function 'compose-mail-other-window)
                     #'remacs-browse-url-1646-compose-capture))
            (remacs-browse-url-1646-assert
             (condition-case nil
                 (progn
                   (browse-url-mail
                    "mailto:?subject=safe%0D%0ABcc%3Aevil%40example.net")
                   nil)
               (error t))
             "browse-url-mail accepted a CRLF Subject")
            (remacs-browse-url-1646-assert
             (= remacs-browse-url-1646-compose-calls 0)
             "CRLF Subject reached compose-mail")
            (remacs-browse-url-1646-assert
             (equal before (buffer-string))
             "rejected mailto changed the caller buffer"))))
      t))
  (remacs-browse-url-1646-assert
   (remacs-browse-url-1646-check-parser-crlf)
   "mailto parser boundary regression")
  (remacs-browse-url-1646-assert
   (remacs-browse-url-1646-check-compose-filter)
   "mailto compose unsafe-header filter regression")
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
fn browse_url_mailto_ignores_rfc6068_unsafe_header_fields() {
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
