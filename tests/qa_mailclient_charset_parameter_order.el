;;; qa_mailclient_charset_parameter_order.el --- parse charset after unknown MIME parameters -*- lexical-binding: t; -*-

(require 'cl-lib)
(require 'mailclient)

(defun qa-mailclient--body-has-czech-a (content-type)
  (let ((mail-aliases nil)
        uri)
    (cl-letf (((symbol-function 'browse-url)
               (lambda (url) (setq uri url))))
      (with-temp-buffer
        (insert "To: a@example.com\n"
                "Content-Type: " content-type "\n"
                "Content-Transfer-Encoding: quoted-printable\n"
                "Subject: probe\n"
                "\n=A1")
        (mailclient-send-it)))
    (not (null (and (stringp uri)
                    (string-match-p "%c4%84" uri))))))

(qa
 (list
  (qa-mailclient--body-has-czech-a
   "text/plain; x-note=ignored; charset=iso-8859-2")
  (qa-mailclient--body-has-czech-a
   "text/plain; x-note=\"foo;bar\"; charset=iso-8859-2")
  (qa-mailclient--body-has-czech-a
   "text/plain; charset=iso-8859-2; format=flowed; delsp=yes"))
 '(t t t))

(princ (qa--done))
