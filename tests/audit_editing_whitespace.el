;;; audit_editing_whitespace.el --- whitespace deletion and accessible bounds -*- lexical-binding: t; -*-
(load (expand-file-name "audit_helpers.el" (file-name-directory load-file-name)) nil t)

(audit-check "delete-blank-lines:nbsp"
 '(with-temp-buffer
    (insert "head\n \nbody\n") (goto-char 6)
    (delete-blank-lines)
    (string-to-list (buffer-string))))

(audit-check "delete-blank-lines:carriage-return"
 '(with-temp-buffer
    (insert "head\n\r\nbody\n") (goto-char 6)
    (delete-blank-lines)
    (string-to-list (buffer-string))))

(audit-check "delete-blank-lines:ascii-control"
 '(with-temp-buffer
    (insert "head\n \t\nbody\n") (goto-char 6)
    (delete-blank-lines)
    (string-to-list (buffer-string))))

(audit-check "delete-blank-lines:narrowing"
 '(with-temp-buffer
    (insert "head\n\n\nbody\n")
    (put-text-property 6 7 'face 'bold)
    (put-text-property 7 8 'face 'italic)
    (narrow-to-region 6 7) (goto-char 6)
    (delete-blank-lines)
    (let ((bounds (list (point-min) (point-max)))
          (visible (string-to-list (buffer-string))))
      (widen)
      (list bounds visible (buffer-substring-no-properties (point-min) (point-max))
            (get-text-property 6 'face)))))

(princ "AUDIT-DONE\n")
