;;; -*- lexical-binding: t; -*-

;; 'replace-region:narrowed-source'
(let ((print-circle t) (print-escape-newlines t)
      (print-escape-control-characters t))
  (prin1 (condition-case err
             (list 'ok (eval '(let ((source (generate-new-buffer " *audit-replace-source*"))) (unwind-protect (progn (with-current-buffer source (insert "xxNEWyy") (narrow-to-region 3 6)) (with-temp-buffer (insert "old") (replace-region-contents 1 4 source) (buffer-string))) (kill-buffer source))) t))
           (error (list 'error err))))
  (terpri))
