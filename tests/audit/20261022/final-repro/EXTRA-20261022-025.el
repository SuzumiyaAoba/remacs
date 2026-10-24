;;; -*- lexical-binding: t; -*-
(load "/Users/suzumiyaaoba/ghq/github.com/SuzumiyaAoba/remacs/tests/audit_helpers.el" nil t)
(audit-check "extract:multibyte-ascii" '(with-temp-buffer (insert "abc") (let ((s (delete-and-extract-region 1 3))) (list (multibyte-string-p s) (string-to-list s)))))
(princ "AUDIT-DONE\n")
