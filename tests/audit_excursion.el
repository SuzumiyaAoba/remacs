;;; audit_excursion.el --- excursion lifetime and mark restoration -*- lexical-binding: t; -*-
(load (expand-file-name "audit_helpers.el" (file-name-directory load-file-name)) nil t)

(audit-check "excursion:killed-saved-buffer"
 '(save-current-buffer
    (let ((a (generate-new-buffer " *audit-excursion-a*"))
          (b (generate-new-buffer " *audit-excursion-b*")))
      (unwind-protect
          (progn
            (set-buffer a)
            (save-excursion (kill-buffer a) (set-buffer b))
            (list (eq (current-buffer) b) (buffer-live-p (current-buffer))))
        (when (buffer-live-p a) (kill-buffer a))
        (kill-buffer b)))))

(audit-check "excursion:initially-unset-mark"
 '(with-temp-buffer
    (insert "abc") (goto-char 1)
    (set-marker (mark-marker) nil)
    (save-mark-and-excursion (set-mark 2) (setq mark-active t) (goto-char 3))
    (list (point) (mark t) mark-active)))

(audit-check "excursion:active-mark-control"
 '(with-temp-buffer
    (insert "abc") (goto-char 1) (set-mark 2) (setq mark-active t)
    (save-mark-and-excursion (set-mark 3) (setq mark-active nil) (goto-char 3))
    (list (point) (mark t) mark-active)))

(audit-check "excursion:mark-activation-hooks"
 '(with-temp-buffer
    (insert "abc") (set-mark 2) (setq mark-active t)
    (let ((hits 0))
      (let ((activate-mark-hook (list (lambda () (setq hits (1+ hits))))))
        (save-mark-and-excursion (set-mark 3) (setq mark-active nil)))
      hits)))

(princ "AUDIT-DONE\n")
