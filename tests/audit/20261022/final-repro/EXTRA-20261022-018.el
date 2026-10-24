;;; -*- lexical-binding: t; -*-
(load "/Users/suzumiyaaoba/ghq/github.com/SuzumiyaAoba/remacs/tests/audit_helpers.el" nil t)
(audit-check "insert:insert-char-character--1" '(with-temp-buffer (set-buffer-multibyte nil) (list (condition-case e (progn (insert-char -1 1) 'inserted) (error (car e))) (string-to-list (buffer-string)))))
(princ "AUDIT-DONE\n")
