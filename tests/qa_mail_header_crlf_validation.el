;;; qa_mail_header_crlf_validation.el --- reject line breaks in generated headers -*- lexical-binding: t; -*-

(require 'mail-utils)
(require 'rfc2047)
(require 'mml)

(defun qa-mail-header--signals-error-p (function)
  (condition-case nil
      (progn (funcall function) nil)
    (error t)))

(defun qa-mail-header--mml-description-result (description)
  (with-temp-buffer
    (let ((signaled nil))
      (condition-case nil
          (mml-insert-mime-headers
           (list (cons 'description description)) "text/plain" nil '7bit nil)
        (error (setq signaled t)))
      (list signaled (mail-fetch-field "Bcc") (buffer-string)))))

(defun qa-mail-header--mml-description-safe-p ()
  (with-temp-buffer
    (mml-insert-mime-headers '((description . "safe")) "text/plain" nil '7bit nil)
    (not (null (string-match-p "Content-Description: safe" (buffer-string))))))

(qa
 (list
  (qa-mail-header--signals-error-p
   (lambda ()
     (rfc2047-encode-parameter
      "filename" (concat "safe" (string 10) "Bcc: victim@example.com"))))
  (qa-mail-header--signals-error-p
   (lambda ()
     (rfc2047-encode-parameter
      "filename" (concat "safe" (string 13) "Bcc: victim@example.com"))))
  (qa-mail-header--signals-error-p
   (lambda ()
     (rfc2047-encode-parameter
      (concat "file" (string 10) "name") "safe")))
  (equal (rfc2047-encode-parameter "filename" "safe.txt")
         "filename=safe.txt")
  (qa-mail-header--mml-description-result
   (concat "safe" (string 10) "Bcc: victim@example.com"))
  (qa-mail-header--mml-description-result
   (concat "safe" (string 13) "Bcc: victim@example.com"))
  (qa-mail-header--mml-description-safe-p))
 '(t t t t (t nil "") (t nil "") t))

(princ (qa--done))
