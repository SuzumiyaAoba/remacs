;;; -*- lexical-binding: t; -*-
(load "/Users/suzumiyaaoba/ghq/github.com/SuzumiyaAoba/remacs/tests/audit_helpers.el" nil t)
(audit-check "completion:try-completion-input-identity" '(let ((input (propertize "foo" 'audit 42))) (try-completion input (lambda (s p a) (list (eq s input) (get-text-property 0 'audit s))))))
(princ "AUDIT-DONE\n")
