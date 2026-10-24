;;; audit_print_overrides.el --- per-call printer settings -*- lexical-binding: t; -*-
(load (expand-file-name "audit_helpers.el" (file-name-directory load-file-name)) nil t)

(audit-check "print:override-length"
 '(prin1-to-string '(1 2 3) nil '((length . 1))))

(audit-check "print:override-defaults"
 '(let ((print-length 1)) (prin1-to-string '(1 2 3) nil t)))

(audit-check "print:override-buffer-stream"
 '(with-temp-buffer
    (prin1 '(1 2 3) (current-buffer) '((length . 1)))
    (buffer-string)))

(audit-check "print:override-invalid-setting"
 '(prin1-to-string '(1 2 3) nil '((not-a-print-setting . 1))))

(princ "AUDIT-DONE\n")
