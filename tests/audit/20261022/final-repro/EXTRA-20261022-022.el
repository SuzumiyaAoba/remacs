;;; -*- lexical-binding: t; -*-
(load "/Users/suzumiyaaoba/ghq/github.com/SuzumiyaAoba/remacs/tests/audit_helpers.el" nil t)
(audit-check "completion:unibyte-result" '(let* ((s (unibyte-string 255 65)) (out (try-completion "" (list s)))) (list (multibyte-string-p out) (string-to-list out))))
(princ "AUDIT-DONE\n")
