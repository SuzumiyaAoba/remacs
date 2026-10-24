;;; -*- lexical-binding: t; -*-
(defun audit-window-metrics-fixture (fn)
  (save-window-excursion
    (delete-other-windows)
    (with-temp-buffer
      (switch-to-buffer (current-buffer))
      (insert "012345678901234567890123456789\n")
      (goto-char 1)
      (funcall fn (selected-window)))))
;; 'window-metrics:horizontal-normal-size'
(let ((print-circle t) (print-escape-newlines t)
      (print-escape-control-characters t))
  (prin1 (condition-case err
             (list 'ok (eval '(audit-window-metrics-fixture (lambda (left) (let ((right (split-window left 20 'right))) (list (< 0.0 (window-normal-size left t) 1.0) (< 0.0 (window-normal-size right t) 1.0) (= 1.0 (+ (window-normal-size left t) (window-normal-size right t))) (window-normal-size left) (window-normal-size right))))) t))
           (error (list 'error err))))
  (terpri))

;; 'window-metrics:vertical-normal-size'
(let ((print-circle t) (print-escape-newlines t)
      (print-escape-control-characters t))
  (prin1 (condition-case err
             (list 'ok (eval '(audit-window-metrics-fixture (lambda (top) (let ((bottom (split-window top 10 'below))) (list (< 0.0 (window-normal-size top) 1.0) (< 0.0 (window-normal-size bottom) 1.0) (= 1.0 (+ (window-normal-size top) (window-normal-size bottom))) (window-normal-size top t) (window-normal-size bottom t))))) t))
           (error (list 'error err))))
  (terpri))
