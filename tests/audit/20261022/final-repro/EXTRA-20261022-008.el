;;; -*- lexical-binding: t; -*-
(load "/Users/suzumiyaaoba/ghq/github.com/SuzumiyaAoba/remacs/tests/audit_helpers.el" nil t)
(audit-check "overlay:fake" '(list (overlayp [overlay 0 0]) (condition-case e (overlay-start [overlay 0 0]) (error (car e))) (condition-case e (overlay-put [overlay 0 0] 'x 1) (error (car e)))))
(princ "AUDIT-DONE\n")
