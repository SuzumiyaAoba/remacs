;;; -*- lexical-binding: t; -*-
(load "/Users/suzumiyaaoba/ghq/github.com/SuzumiyaAoba/remacs/tests/audit_helpers.el" nil t)
(audit-check "json:utf8-serialize" '(string-to-list (json-serialize (unibyte-string 195 169))))
(princ "AUDIT-DONE\n")
