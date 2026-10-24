;;; -*- lexical-binding: t; -*-

;; 'buffer-transfer:properties-append-to-buffer'
(let ((print-circle t) (print-escape-newlines t)
      (print-escape-control-characters t))
  (prin1 (condition-case err
             (list 'ok (eval '(let ((target (generate-new-buffer " *audit-transfer*"))) (unwind-protect (with-temp-buffer (insert (propertize "abc" 'face 'bold)) (append-to-buffer target 1 4) (with-current-buffer target (list (buffer-substring-no-properties 1 4) (mapcar (lambda (p) (get-text-property p 'face)) '(1 2 3))))) (kill-buffer target))) t))
           (error (list 'error err))))
  (terpri))
