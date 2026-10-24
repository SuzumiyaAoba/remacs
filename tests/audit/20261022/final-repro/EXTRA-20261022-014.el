;;; -*- lexical-binding: t; -*-
(load "/Users/suzumiyaaoba/ghq/github.com/SuzumiyaAoba/remacs/tests/audit_helpers.el" nil t)
(audit-check "json:cyclic-vector" '(let ((v (vector nil))) (aset v 0 v) (condition-case e (json-serialize v) (error (car e)))))
(princ "AUDIT-DONE\n")
