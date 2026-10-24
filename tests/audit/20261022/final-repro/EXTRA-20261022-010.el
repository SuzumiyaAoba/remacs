;;; -*- lexical-binding: t; -*-
(load "/Users/suzumiyaaoba/ghq/github.com/SuzumiyaAoba/remacs/tests/audit_helpers.el" nil t)
(audit-check "overlay:category-evaporate" '(with-temp-buffer (insert "abc") (let ((cat (make-symbol "audit-evap")) (o (make-overlay 1 3))) (put cat 'evaporate t) (overlay-put o 'category cat) (delete-region 1 3) (list (overlay-start o) (and (overlay-buffer o) t)))))
(princ "AUDIT-DONE\n")
