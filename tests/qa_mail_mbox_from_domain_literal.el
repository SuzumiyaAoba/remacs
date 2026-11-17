;;; qa_mail_mbox_from_domain_literal.el --- domain-literal address regressions -*- lexical-binding: t; -*-

(require 'mail-utils)

(defun qa-mail-mbox-from-prefix-p (from prefix)
  (with-temp-buffer
    (insert "From: " from "\n\n")
    (let ((line (mail-mbox-from)))
      (and (>= (length line) (length prefix))
           (string= (substring line 0 (length prefix)) prefix)))))

(qa
 (list
  (let ((mail-use-rfc822 nil))
    (mail-strip-quoted-names "User <user@[foo,bar]>"))
  (let ((mail-use-rfc822 nil))
    (mail-strip-quoted-names "User <user@example.com>"))
  (qa-mail-mbox-from-prefix-p "user@[foo,bar]" "From user@[foo,bar] ")
  (qa-mail-mbox-from-prefix-p "user@[foo,bar], other@example.com"
                              "From user@[foo,bar] ")
  (qa-mail-mbox-from-prefix-p "user@example.com, other@example.com"
                              "From user@example.com "))
 '("user@[foo,bar]" "user@example.com" t t t))

(princ (qa--done))
