;;; -*- lexical-binding: t; -*-
(load "/Users/suzumiyaaoba/ghq/github.com/SuzumiyaAoba/remacs/tests/audit_helpers.el" nil t)
(audit-check "json:insert-unibyte" '(with-temp-buffer (set-buffer-multibyte nil) (json-insert "é") (string-to-list (buffer-string))))
(princ "AUDIT-DONE\n")
