;;; -*- lexical-binding: t; -*-
(defun audit-window-metrics-fixture (fn)
  (save-window-excursion
    (delete-other-windows)
    (with-temp-buffer
      (switch-to-buffer (current-buffer))
      (insert "012345678901234567890123456789\n")
      (goto-char 1)
      (funcall fn (selected-window)))))
;; 'window-metrics:old-point-after-set'
(let ((print-circle t) (print-escape-newlines t)
      (print-escape-control-characters t))
  (prin1 (condition-case err
             (list 'ok (eval '(audit-window-metrics-fixture (lambda (first) (let* ((other (split-window first 20 'right)) (before (window-old-point other))) (set-window-point other 10) (list (window-point other) before (window-old-point other))))) t))
           (error (list 'error err))))
  (terpri))
