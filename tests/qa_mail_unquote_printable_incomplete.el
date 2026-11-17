;;; qa_mail_unquote_printable_incomplete.el --- quoted-printable validation regression -*- lexical-binding: t -*-

(require 'mail-utils)

(defun qa-mail-unquote-printable-region-result (text &optional noerror)
  (with-temp-buffer
    (insert text)
    (list (condition-case error
              (mail-unquote-printable-region
               (point-min) (point-max) nil noerror)
            (error (car error)))
          (buffer-string))))

(defun qa-mail-unquote-printable-string-result (text)
  (condition-case error
      (mail-unquote-printable text)
    (error (car error))))

(qa (qa-mail-unquote-printable-region-result "tail=41") '(t "tailA"))
(qa (qa-mail-unquote-printable-string-result "tail=41") "tailA")

(let ((invalid-results
       (list (qa-mail-unquote-printable-region-result "tail=")
             (qa-mail-unquote-printable-region-result "tail=A")
             (qa-mail-unquote-printable-region-result "==")
             (qa-mail-unquote-printable-region-result "tail=" t)
             (qa-mail-unquote-printable-region-result "tail=A" t)
             (qa-mail-unquote-printable-string-result "tail=")
             (qa-mail-unquote-printable-string-result "tail=A")
             (qa-mail-unquote-printable-string-result "=="))))
  (if qa--remacs
      (qa invalid-results
          '((error "tail=")
            (error "tail=A")
            (error "==")
            (nil "tail=")
            (nil "tail=A")
            error error error))
    ;; GNU 31.1 also accepts incomplete escapes in both public APIs.
    (qa-xf invalid-results
           '((error "tail=")
             (error "tail=A")
             (error "==")
             (nil "tail=")
             (nil "tail=A")
             error error error)
           "Quoted-printable decoders reject incomplete escapes")))

(princ (qa--done))
