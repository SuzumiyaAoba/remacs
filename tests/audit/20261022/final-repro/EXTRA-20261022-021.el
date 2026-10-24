;;; -*- lexical-binding: t; -*-
(load "/Users/suzumiyaaoba/ghq/github.com/SuzumiyaAoba/remacs/tests/audit_helpers.el" nil t)
(audit-check "completion:try-completion-invalid" '(let ((completion-regexp-list (list "["))) (condition-case e (try-completion "foo" '("foo" "foobar")) (error (car e)))))
(princ "AUDIT-DONE\n")
