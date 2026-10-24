;;; -*- lexical-binding: t; -*-
(load "/Users/suzumiyaaoba/ghq/github.com/SuzumiyaAoba/remacs/tests/audit_helpers.el" nil t)
(audit-check "relocation:delete-region" '(with-temp-buffer (insert "abcd") (goto-char 2) (let ((before-change-functions (list (lambda (&rest _) (save-excursion (goto-char 1) (insert "P")))))) (let ((result (delete-region 2 3))) (list result (buffer-string) (point))))))
(princ "AUDIT-DONE\n")
