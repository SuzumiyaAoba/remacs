;;; -*- lexical-binding: t; -*-
(load "/Users/suzumiyaaoba/ghq/github.com/SuzumiyaAoba/remacs/tests/audit_helpers.el" nil t)
(audit-check "json:utf8-input" '(string-to-list (json-parse-string (unibyte-string 34 195 169 34))))
(princ "AUDIT-DONE\n")
