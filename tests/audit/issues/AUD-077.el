;;; -*- lexical-binding: t; -*-

;; 'replace-region:vector-source'
(let ((print-circle t) (print-escape-newlines t)
      (print-escape-control-characters t))
  (prin1 (condition-case err
             (list 'ok (eval '(let ((source (generate-new-buffer " *audit-replace-source*"))) (unwind-protect (progn (with-current-buffer source (insert "xxNEWyy")) (with-temp-buffer (insert "old") (replace-region-contents 1 4 (vector source 3 6)) (buffer-string))) (kill-buffer source))) t))
           (error (list 'error err))))
  (terpri))
