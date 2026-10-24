;;; -*- lexical-binding: t; -*-

;; 'replace-region:properties'
(let ((print-circle t) (print-escape-newlines t)
      (print-escape-control-characters t))
  (prin1 (condition-case err
             (list 'ok (eval '(with-temp-buffer (insert "abc") (replace-region-contents 1 4 (propertize "aXc" 'face 'bold)) (list (buffer-substring-no-properties 1 4) (mapcar (lambda (p) (get-text-property p 'face)) '(1 2 3)))) t))
           (error (list 'error err))))
  (terpri))
