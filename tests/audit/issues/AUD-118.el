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
;; 'window-config:compare-ignores-scroll'
(let ((print-circle t) (print-escape-newlines t)
      (print-escape-control-characters t))
  (prin1 (condition-case err
             (list 'ok (eval '(audit-window-fixture (lambda (_buffer window) (let ((before (current-window-configuration))) (set-window-start window 12 t) (set-window-hscroll window 7) (compare-window-configurations before (current-window-configuration))))) t))
           (error (list 'error err))))
  (terpri))
