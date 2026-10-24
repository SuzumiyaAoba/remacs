;;; audit_buffer_transfer.el --- copying between buffers -*- lexical-binding: t; -*-
(load (expand-file-name "audit_helpers.el" (file-name-directory load-file-name)) nil t)

(audit-check "buffer-transfer:copy-create"
 '(let ((name (generate-new-buffer-name " *audit-copy-new*")))
    (unwind-protect
        (with-temp-buffer
          (insert "abc")
          (copy-to-buffer name 1 4)
          (with-current-buffer name (buffer-string)))
      (when (get-buffer name) (kill-buffer name)))))

(dolist (operation '(copy-to-buffer append-to-buffer prepend-to-buffer))
  (audit-check (format "buffer-transfer:properties-%s" operation)
   `(let ((target (generate-new-buffer " *audit-transfer*")))
      (unwind-protect
          (with-temp-buffer
            (insert (propertize "abc" 'face 'bold))
            (,operation target 1 4)
            (with-current-buffer target
              (list (buffer-substring-no-properties 1 4)
                    (mapcar (lambda (p) (get-text-property p 'face)) '(1 2 3)))))
        (kill-buffer target))))
  (audit-check (format "buffer-transfer:point-%s" operation)
   `(let ((target (generate-new-buffer " *audit-transfer*")))
      (unwind-protect
          (progn
            (with-current-buffer target (insert "XY") (goto-char 2))
            (with-temp-buffer (insert "abc") (,operation target 1 4))
            (with-current-buffer target (list (buffer-string) (point))))
        (kill-buffer target))))
  (audit-check (format "buffer-transfer:readonly-%s" operation)
   `(let ((target (generate-new-buffer " *audit-transfer*")))
      (unwind-protect
          (progn
            (with-current-buffer target (setq buffer-read-only t))
            (with-temp-buffer
              (insert "abc")
              (list (condition-case err (progn (,operation target 1 4) 'copied)
                      (error (car err)))
                    (with-current-buffer target (buffer-string)))))
        (kill-buffer target))))
  (audit-check (format "buffer-transfer:narrowing-%s" operation)
   `(let ((target (generate-new-buffer " *audit-transfer*")))
      (unwind-protect
          (with-temp-buffer
            (insert "[abc]") (narrow-to-region 2 5)
            (list (condition-case err (progn (,operation target 1 6) 'copied)
                    (error (car err)))
                  (with-current-buffer target (buffer-string))))
        (kill-buffer target)))))

(princ "AUDIT-DONE\n")
