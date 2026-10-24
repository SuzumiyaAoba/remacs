;;; audit_window_config.el --- configuration restoration and comparison -*- lexical-binding: t; -*-
(load (expand-file-name "audit_helpers.el" (file-name-directory load-file-name)) nil t)

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

(audit-check "window-config:scroll-restoration"
 '(audit-window-fixture
   (lambda (_buffer window)
     (set-window-start window 12 t) (set-window-hscroll window 2)
     (let ((saved (current-window-configuration)))
       (set-window-start window 23 t) (set-window-hscroll window 9)
       (set-window-configuration saved)
       (list (window-start window) (window-hscroll window))))))

(audit-check "window-config:noncurrent-point"
 '(audit-window-fixture
   (lambda (_buffer window)
     (let ((other (generate-new-buffer " *audit-window-other*")))
       (unwind-protect
           (progn
             (with-current-buffer other (insert "01234567890123456789"))
             (let ((second (split-window window nil 'right)))
               (set-window-buffer second other) (set-window-point second 4)
               (let ((saved (current-window-configuration)))
                 (set-window-point second 12) (set-window-configuration saved)
                 (window-point second))))
         (kill-buffer other))))))

(audit-check "window-config:current-point"
 '(audit-window-fixture
   (lambda (_buffer _window)
     (goto-char 4)
     (let ((saved (current-window-configuration)))
       (goto-char 12) (set-window-configuration saved) (point)))))

(audit-check "window-config:horizontal-tree"
 '(audit-window-fixture
   (lambda (_buffer window)
     (split-window window nil 'right)
     (let ((saved (current-window-configuration)))
       (set-window-configuration saved)
       (let ((root (frame-root-window)))
         (list (eq (window-left-child root) window)
               (eq (window-top-child root) window) (length (window-list))))))))

(audit-check "window-config:nested-tree"
 '(audit-window-fixture
   (lambda (_buffer window)
     (let* ((right (split-window window nil 'right))
            (bottom (split-window right nil 'below))
            (saved (current-window-configuration)))
       (set-window-configuration saved)
       (list (eq (window-parent window) (window-parent right))
             (eq (window-parent right) (window-parent bottom)))))))

(audit-check "window-config:compare-ignores-scroll"
 '(audit-window-fixture
   (lambda (_buffer window)
     (let ((before (current-window-configuration)))
       (set-window-start window 12 t) (set-window-hscroll window 7)
       (compare-window-configurations before (current-window-configuration))))))

(audit-check "window-config:forged-record"
 '(let ((fake (record 'window-configuration (selected-frame) nil nil)))
    (list (window-configuration-p fake)
          (condition-case err (framep (window-configuration-frame fake))
            (error (car err))))))

(princ "AUDIT-DONE\n")
