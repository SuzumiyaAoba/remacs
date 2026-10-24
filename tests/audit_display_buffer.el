;;; audit_display_buffer.el --- fallback when selected window cannot split -*- lexical-binding: t; -*-
(load (expand-file-name "audit_helpers.el" (file-name-directory load-file-name)) nil t)

(audit-check "display-buffer:small-selected-window"
 '(save-window-excursion
    (delete-other-windows)
    (let ((buffer (generate-new-buffer " *audit-display*")))
      (unwind-protect
          (progn
            (split-window (selected-window) 4 'below)
            (let ((window (display-buffer buffer)))
              (list (window-live-p window) (eq (window-buffer window) buffer))))
        (kill-buffer buffer)))))

(audit-check "display-buffer:repeated-buffer-menu"
 '(save-window-excursion
    (delete-other-windows)
    (let ((completed 0))
      (unwind-protect
          (condition-case err
              (progn
                (dotimes (_ 4)
                  (list-buffers)
                  (setq completed (1+ completed))
                  (kill-buffer "*Buffer List*"))
                (list completed nil))
            (error (list completed (car err))))
        (when (get-buffer "*Buffer List*") (kill-buffer "*Buffer List*"))))))

(audit-check "display-buffer:explicit-reuse-control"
 '(save-window-excursion
    (delete-other-windows)
    (let ((buffer (generate-new-buffer " *audit-display-control*")))
      (unwind-protect
          (let ((window (display-buffer buffer '((display-buffer-use-some-window)))))
            (list (window-live-p window) (eq (window-buffer window) buffer)))
        (kill-buffer buffer)))))

(princ "AUDIT-DONE\n")
