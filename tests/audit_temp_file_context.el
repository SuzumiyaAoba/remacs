;;; -*- lexical-binding: t; -*-
(load (expand-file-name "audit_helpers.el" (file-name-directory load-file-name)) nil t)
(audit-check "temp-context:directory-variable" '(let* ((dir (make-temp-file "remacs-audit-context-" t)) (temporary-file-directory (file-name-as-directory dir)) file)
  (unwind-protect (progn (setq file (make-temp-file "child-")) (equal (file-name-directory file) temporary-file-directory))
    (when file (delete-file file)) (delete-directory dir t))))
(audit-check "temp-context:write-coding" '(let ((coding-system-for-write 'iso-latin-1) file)
  (unwind-protect (progn (setq file (make-temp-file "remacs-audit-text-" nil nil "é"))
    (with-temp-buffer (set-buffer-multibyte nil) (insert-file-contents-literally file) (string-to-list (buffer-string))))
    (when file (delete-file file)))))
(audit-check "temp-context:internal-bytes" '(let (file) (unwind-protect
  (progn (setq file (make-temp-file-internal (expand-file-name "remacs-audit-internal-" temporary-file-directory) nil "" (unibyte-string 128 255)))
    (with-temp-buffer (set-buffer-multibyte nil) (insert-file-contents-literally file) (string-to-list (buffer-string))))
  (when file (delete-file file)))))
(audit-check "temp-context:internal-file-mode" '(let (file) (unwind-protect
  (progn (setq file (make-temp-file-internal (expand-file-name "remacs-audit-internal-" temporary-file-directory) nil "" nil)) (logand (file-modes file) #o777))
  (when file (delete-file file)))))
(audit-check "temp-context:internal-dir-mode" '(let (file) (unwind-protect
  (progn (setq file (make-temp-file-internal (expand-file-name "remacs-audit-internal-" temporary-file-directory) t "" nil)) (logand (file-modes file) #o777))
  (when file (delete-directory file)))))
(princ "AUDIT-DONE\n")
