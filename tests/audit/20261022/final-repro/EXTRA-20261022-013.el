;;; -*- lexical-binding: t; -*-
(load "/Users/suzumiyaaoba/ghq/github.com/SuzumiyaAoba/remacs/tests/audit_helpers.el" nil t)
(audit-check "overlay:window-restricted" '(save-window-excursion (with-temp-buffer (insert "abc") (switch-to-buffer (current-buffer)) (let ((o (make-overlay 1 4)) (other (split-window))) (overlay-put o 'face 'bold) (overlay-put o 'window other) (list (get-char-property 2 'face (selected-window)) (get-char-property 2 'face other) (get-char-property 2 'face (current-buffer)))))))
(princ "AUDIT-DONE\n")
