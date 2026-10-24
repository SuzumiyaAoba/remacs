;;; -*- lexical-binding: t; -*-
(load "/Users/suzumiyaaoba/ghq/github.com/SuzumiyaAoba/remacs/tests/audit_helpers.el" nil t)
(audit-check "json:null-nil" '(json-serialize [nil] :null-object nil))
(princ "AUDIT-DONE\n")
