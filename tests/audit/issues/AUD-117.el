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
;; 'window-config:horizontal-tree'
(let ((print-circle t) (print-escape-newlines t)
      (print-escape-control-characters t))
  (prin1 (condition-case err
             (list 'ok (eval '(audit-window-fixture (lambda (_buffer window) (split-window window nil 'right) (let ((saved (current-window-configuration))) (set-window-configuration saved) (let ((root (frame-root-window))) (list (eq (window-left-child root) window) (eq (window-top-child root) window) (length (window-list))))))) t))
           (error (list 'error err))))
  (terpri))

;; 'window-config:nested-tree'
(let ((print-circle t) (print-escape-newlines t)
      (print-escape-control-characters t))
  (prin1 (condition-case err
             (list 'ok (eval '(audit-window-fixture (lambda (_buffer window) (let* ((right (split-window window nil 'right)) (bottom (split-window right nil 'below)) (saved (current-window-configuration))) (set-window-configuration saved) (list (eq (window-parent window) (window-parent right)) (eq (window-parent right) (window-parent bottom)))))) t))
           (error (list 'error err))))
  (terpri))
