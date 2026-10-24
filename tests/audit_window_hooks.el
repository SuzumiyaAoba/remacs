;;; audit_window_hooks.el --- scroll notifications -*- lexical-binding: t; -*-
(load (expand-file-name "audit_helpers.el" (file-name-directory load-file-name)) nil t)

(audit-check "window-hooks:explicit-scroll-functions"
 '(save-window-excursion
    (let ((buffer (generate-new-buffer " *audit-scroll-hook*")))
      (unwind-protect
          (progn
            (delete-other-windows) (switch-to-buffer buffer)
            (insert "one\ntwo\n") (goto-char 1)
            (set-window-start nil 1 t)
            (let ((window (selected-window)) hits)
              (let ((window-scroll-functions
                     (list (lambda (w start)
                             (push (list (eq w window) start (eq (current-buffer) buffer)) hits)))))
                (run-window-scroll-functions window))
              (nreverse hits)))
        (kill-buffer buffer)))))

(audit-check "window-hooks:split-notification"
 '(save-window-excursion
    (let ((buffer (generate-new-buffer " *audit-split-hook*")))
      (unwind-protect
          (progn
            (delete-other-windows) (switch-to-buffer buffer)
            (insert "one\ntwo\n") (goto-char 1)
            (let ((window (selected-window)) got)
              (let ((window-scroll-functions (list (lambda (w _start) (setq got w)))))
                (let ((new (split-window window nil 'right)))
                  (list (not (null got)) (eq got new))))))
        (kill-buffer buffer)))))

(princ "AUDIT-DONE\n")
