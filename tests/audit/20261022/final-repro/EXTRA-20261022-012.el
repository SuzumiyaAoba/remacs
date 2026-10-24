;;; -*- lexical-binding: t; -*-
(load "/Users/suzumiyaaoba/ghq/github.com/SuzumiyaAoba/remacs/tests/audit_helpers.el" nil t)
(audit-check "overlay:category-visible" '(with-temp-buffer (insert "abc") (let ((o (make-overlay 1 4)) (cat (make-symbol "audit-cat"))) (put cat 'face 'bold) (overlay-put o 'category cat) (list (overlay-get o 'face) (get-char-property 2 'face)))))
(princ "AUDIT-DONE\n")
