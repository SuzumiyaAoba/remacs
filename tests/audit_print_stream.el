;;; audit_print_stream.el --- output stream liveness and read-only checks -*- lexical-binding: t; -*-
(load (expand-file-name "audit_helpers.el" (file-name-directory load-file-name)) nil t)

(audit-check "print:readonly-buffer"
 '(with-temp-buffer
    (setq buffer-read-only t)
    (let ((result (condition-case err (progn (princ "payload" (current-buffer)) 'printed)
                    (error (car err)))))
      (list result (buffer-string)))))

(audit-check "print:readonly-marker"
 '(with-temp-buffer
    (let ((m (point-marker)))
      (setq buffer-read-only t)
      (let ((result (condition-case err (progn (princ "payload" m) 'printed)
                      (error (car err)))))
        (list result (buffer-string) (marker-position m))))))

(audit-check "print:detached-marker"
 '(let ((m (make-marker)))
    (condition-case err (progn (princ "payload" m) 'printed) (error (car err)))))

(audit-check "print:killed-buffer"
 '(let ((dead (generate-new-buffer " *audit-print-dead*")))
    (kill-buffer dead)
    (with-temp-buffer
      (insert "original")
      (let ((result (condition-case err (progn (princ "payload" dead) 'printed)
                      (error (car err)))))
        (list result (buffer-string))))))

(audit-check "print:live-marker-control"
 '(with-temp-buffer
    (insert "ab") (goto-char 1)
    (let ((m (copy-marker 2)))
      (princ "X" m)
      (list (buffer-string) (point) (marker-position m)))))

(princ "AUDIT-DONE\n")
