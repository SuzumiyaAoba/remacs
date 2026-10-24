;;; audit_buffer.el --- buffer editing and undo probes -*- lexical-binding: t; -*-
(load (expand-file-name "audit_helpers.el" (file-name-directory load-file-name)) nil t)

(audit-check "buffer:undo-deleted-properties"
 '(with-temp-buffer
    (buffer-enable-undo)
    (insert (propertize "abc" 'face 'bold))
    (setq buffer-undo-list nil)
    (delete-region 2 3)
    (setq buffer-undo-list (primitive-undo 1 buffer-undo-list))
    (list (buffer-substring-no-properties 1 4)
          (mapcar (lambda (p) (get-text-property p 'face)) '(1 2 3)))))

(audit-check "string:propertize-unibyte"
 '(let* ((s (unibyte-string 128 255))
         (result (propertize s 'face 'bold)))
    (list (multibyte-string-p result) (string-to-list result)
          (aref result 0) (vconcat result) (get-text-property 0 'face result))))

(princ "AUDIT-DONE\n")
