;;; -*- lexical-binding: t; -*-
(load "/Users/suzumiyaaoba/ghq/github.com/SuzumiyaAoba/remacs/tests/audit_helpers.el" nil t)
(audit-check "json:float-overflow" '(condition-case e (json-parse-string "1e9999") (error (car e))))
(princ "AUDIT-DONE\n")
