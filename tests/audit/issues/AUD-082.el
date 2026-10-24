;;; -*- lexical-binding: t; -*-

;; 'buffer-transfer:narrowing-append-to-buffer'
(let ((print-circle t) (print-escape-newlines t)
      (print-escape-control-characters t))
  (prin1 (condition-case err
             (list 'ok (eval '(let ((target (generate-new-buffer " *audit-transfer*"))) (unwind-protect (with-temp-buffer (insert "[abc]") (narrow-to-region 2 5) (list (condition-case err (progn (append-to-buffer target 1 6) 'copied) (error (car err))) (with-current-buffer target (buffer-string)))) (kill-buffer target))) t))
           (error (list 'error err))))
  (terpri))
