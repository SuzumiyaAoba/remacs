;;; -*- lexical-binding: t; -*-
(defun audit-window-metrics-fixture (fn)
  (save-window-excursion
    (delete-other-windows)
    (with-temp-buffer
      (switch-to-buffer (current-buffer))
      (insert "012345678901234567890123456789\n")
      (goto-char 1)
      (funcall fn (selected-window)))))
;; 'window-metrics:old-buffer-after-set'
(let ((print-circle t) (print-escape-newlines t)
      (print-escape-control-characters t))
  (prin1 (condition-case err
             (list 'ok (eval '(audit-window-metrics-fixture (lambda (window) (let ((other (generate-new-buffer " *audit-window-old-buffer*")) (old (window-old-buffer window)) (previous (window-buffer window))) (unwind-protect (progn (set-window-buffer window other) (list (eq (window-old-buffer window) old) (eq (window-old-buffer window) previous))) (set-window-buffer window previous) (kill-buffer other))))) t))
           (error (list 'error err))))
  (terpri))
