;;; -*- lexical-binding: t; -*-
(load "/Users/suzumiyaaoba/ghq/github.com/SuzumiyaAoba/remacs/tests/audit_helpers.el" nil t)
(audit-check "overlay:dead-buffer-properties" '(let* ((b (generate-new-buffer " *audit-dead*")) (o (make-overlay 1 1 b))) (overlay-put o 'x 1) (kill-buffer b) (list (overlay-buffer o) (overlay-start o) (overlay-get o 'x) (progn (overlay-put o 'y 2) (overlay-get o 'y)))))
(princ "AUDIT-DONE\n")
