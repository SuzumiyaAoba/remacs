;;; -*- lexical-binding: t; -*-
(load "/Users/suzumiyaaoba/ghq/github.com/SuzumiyaAoba/remacs/tests/audit_helpers.el" nil t)
(audit-check "overlay:sorted-priority-cons" '(with-temp-buffer (insert "abc") (let ((a (make-overlay 1 4)) (b (make-overlay 1 4))) (overlay-put a 'priority '(nil . 10)) (overlay-put b 'priority '(nil . 1)) (overlay-put a 'face 'bold) (overlay-put b 'face 'italic) (list (eq a (car (overlays-at 2 t))) (get-char-property 2 'face)))))
(princ "AUDIT-DONE\n")
