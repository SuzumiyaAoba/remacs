;;; -*- lexical-binding: t; -*-
(load "/Users/suzumiyaaoba/ghq/github.com/SuzumiyaAoba/remacs/tests/audit_helpers.el" nil t)
(audit-check "json:parse-narrow" '(with-temp-buffer (insert "[1,2]") (narrow-to-region 1 4) (goto-char 1) (list (condition-case e (json-parse-buffer) (error (car e))) (point))))
(princ "AUDIT-DONE\n")
