;;; -*- lexical-binding: t; -*-
(load "/Users/suzumiyaaoba/ghq/github.com/SuzumiyaAoba/remacs/tests/audit_helpers.el" nil t)
(audit-check "relocation:kill-delete" '(with-temp-buffer (insert "SAFE") (let* ((outer (current-buffer)) (b (generate-new-buffer " *audit-killed-target*")) (result (with-current-buffer b (insert "abcd") (goto-char 2) (let ((before-change-functions (list (lambda (&rest _) (kill-buffer (current-buffer)) (set-buffer outer))))) (condition-case e (delete-region 2 3) (error (car e))))))) (list result (buffer-string)))))
(princ "AUDIT-DONE\n")
