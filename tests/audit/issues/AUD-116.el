;;; -*- lexical-binding: t; -*-
(defun audit-window-fixture (fn)
  (let ((saved (current-window-configuration))
        (buffer (generate-new-buffer " *audit-window-buffer*")))
    (unwind-protect
        (progn
          (delete-other-windows)
          (switch-to-buffer buffer)
          (insert (apply #'concat (make-list 40 "0123456789\n")))
          (goto-char 1)
          (set-window-start nil 1 t) (set-window-hscroll nil 0)
          (funcall fn buffer (selected-window)))
      (set-window-dedicated-p nil nil)
      (set-window-configuration saved)
      (when (buffer-live-p buffer) (kill-buffer buffer)))))

(defun audit-window-parameter-fixture (fn)
  (let* ((window (selected-window))
         (old (window-parameter window 'audit-persistent)))
    (unwind-protect (funcall fn window)
      (set-window-parameter window 'audit-persistent old))))
;; 'window-config:scroll-restoration'
(let ((print-circle t) (print-escape-newlines t)
      (print-escape-control-characters t))
  (prin1 (condition-case err
             (list 'ok (eval '(audit-window-fixture (lambda (_buffer window) (set-window-start window 12 t) (set-window-hscroll window 2) (let ((saved (current-window-configuration))) (set-window-start window 23 t) (set-window-hscroll window 9) (set-window-configuration saved) (list (window-start window) (window-hscroll window))))) t))
           (error (list 'error err))))
  (terpri))

;; 'window-config:noncurrent-point'
(let ((print-circle t) (print-escape-newlines t)
      (print-escape-control-characters t))
  (prin1 (condition-case err
             (list 'ok (eval '(audit-window-fixture (lambda (_buffer window) (let ((other (generate-new-buffer " *audit-window-other*"))) (unwind-protect (progn (with-current-buffer other (insert "01234567890123456789")) (let ((second (split-window window nil 'right))) (set-window-buffer second other) (set-window-point second 4) (let ((saved (current-window-configuration))) (set-window-point second 12) (set-window-configuration saved) (window-point second)))) (kill-buffer other))))) t))
           (error (list 'error err))))
  (terpri))

;; 'window-config:current-point'
(let ((print-circle t) (print-escape-newlines t)
      (print-escape-control-characters t))
  (prin1 (condition-case err
             (list 'ok (eval '(audit-window-fixture (lambda (_buffer _window) (goto-char 4) (let ((saved (current-window-configuration))) (goto-char 12) (set-window-configuration saved) (point)))) t))
           (error (list 'error err))))
  (terpri))

;; 'window-config:margin-restoration'
(let ((print-circle t) (print-escape-newlines t)
      (print-escape-control-characters t))
  (prin1 (condition-case err
             (list 'ok (eval '(let* ((window (selected-window)) (old (window-margins window))) (unwind-protect (progn (set-window-margins window 2 3) (let ((before (window-margins window)) (saved (current-window-configuration))) (set-window-margins window 4 5) (let ((changed (window-margins window))) (set-window-configuration saved) (list before changed (window-margins window))))) (set-window-margins window (car old) (cdr old)))) t))
           (error (list 'error err))))
  (terpri))
