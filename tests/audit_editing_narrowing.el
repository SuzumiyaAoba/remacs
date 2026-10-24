;;; audit_editing_narrowing.el --- whitespace commands at restriction edges -*- lexical-binding: t; -*-
(load (expand-file-name "audit_helpers.el" (file-name-directory load-file-name)) nil t)

(audit-check "delete-horizontal-space:narrowing"
 '(with-temp-buffer
    (insert "A    B") (narrow-to-region 3 5) (goto-char 4)
    (delete-horizontal-space)
    (widen) (buffer-string)))

(audit-check "just-one-space:narrowing"
 '(with-temp-buffer
    (insert "A    B") (narrow-to-region 3 5) (goto-char 4)
    (just-one-space)
    (widen) (buffer-string)))

(audit-check "delete-indentation:narrowing"
 '(with-temp-buffer
    (insert "head\n  body") (narrow-to-region 8 12) (goto-char 8)
    (delete-indentation)
    (widen) (buffer-string)))

(princ "AUDIT-DONE\n")
