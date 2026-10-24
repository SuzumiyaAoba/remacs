;;; audit_line_filter.el --- filtering retained lines and count context -*- lexical-binding: t; -*-
(load (expand-file-name "audit_helpers.el" (file-name-directory load-file-name)) nil t)

(audit-check "flush-lines:default-point"
 '(with-temp-buffer
    (insert "a\nb\na\n") (goto-char 3)
    (flush-lines "^a$")
    (buffer-string)))

(audit-check "keep-lines:default-point"
 '(with-temp-buffer
    (insert "a\nb\nc\n") (goto-char 3)
    (keep-lines "^b$")
    (buffer-string)))

(audit-check "flush-lines:retained-metadata"
 '(with-temp-buffer
    (insert (propertize "keep\n" 'face 'bold) "drop\n" (propertize "stay\n" 'face 'italic))
    (goto-char 1)
    (let ((m (copy-marker 3)))
      (flush-lines "^drop$")
      (list (buffer-substring-no-properties (point-min) (point-max))
            (get-text-property 1 'face) (get-text-property 6 'face) (marker-position m)))))

(audit-check "keep-lines:retained-metadata"
 '(with-temp-buffer
    (insert (propertize "keep\n" 'face 'bold) "drop\n" (propertize "stay\n" 'face 'italic))
    (goto-char 1)
    (let ((m (copy-marker 3)))
      (keep-lines "^\\(keep\\|stay\\)$")
      (list (buffer-substring-no-properties (point-min) (point-max))
            (get-text-property 1 'face) (get-text-property 6 'face) (marker-position m)))))

(audit-check "how-many:beginning-context"
 '(with-temp-buffer (insert "ab") (how-many "^b" 2 3)))

(audit-check "how-many:word-context"
 '(with-temp-buffer (insert "ab") (how-many "\\<b" 2 3)))

(audit-check "how-many:end-context"
 '(with-temp-buffer (insert "abX") (how-many "b$" 1 3)))

(audit-check "how-many:buffer-end-context"
 '(with-temp-buffer (insert "abX") (how-many "b\\'" 1 3)))

(princ "AUDIT-DONE\n")
