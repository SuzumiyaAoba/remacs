;;; -*- lexical-binding: t; -*-
(defun audit-window-metrics-fixture (fn)
  (save-window-excursion
    (delete-other-windows)
    (with-temp-buffer
      (switch-to-buffer (current-buffer))
      (insert "012345678901234567890123456789\n")
      (goto-char 1)
      (funcall fn (selected-window)))))
;; 'window-metrics:bump-use-order'
(let ((print-circle t) (print-escape-newlines t)
      (print-escape-control-characters t))
  (prin1 (condition-case err
             (list 'ok (eval '(audit-window-metrics-fixture (lambda (first) (let* ((second (split-window first 20 'right)) (third (split-window second 20 'right))) (select-window first) (select-window second) (select-window third) (let ((before (window-use-time third)) (result (window-bump-use-time first))) (list (integerp result) (= (window-use-time first) before) (> (window-use-time third) before) (> (window-use-time first) (window-use-time second)) (eq (selected-window) third)))))) t))
           (error (list 'error err))))
  (terpri))

;; 'window-metrics:bump-selected-control'
(let ((print-circle t) (print-escape-newlines t)
      (print-escape-control-characters t))
  (prin1 (condition-case err
             (list 'ok (eval '(audit-window-metrics-fixture (lambda (window) (let ((before (window-use-time window))) (list (window-bump-use-time window) (= before (window-use-time window)))))) t))
           (error (list 'error err))))
  (terpri))
