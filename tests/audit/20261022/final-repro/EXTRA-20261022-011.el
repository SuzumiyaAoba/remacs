;;; -*- lexical-binding: t; -*-
(load "/Users/suzumiyaaoba/ghq/github.com/SuzumiyaAoba/remacs/tests/audit_helpers.el" nil t)
(audit-check "overlay:make-dead-buffer" '(let ((b (generate-new-buffer " *audit-dead-arg*"))) (kill-buffer b) (condition-case e (make-overlay 1 1 b) (error (car e)))))
(princ "AUDIT-DONE\n")
