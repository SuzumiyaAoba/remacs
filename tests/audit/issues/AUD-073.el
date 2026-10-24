;;; -*- lexical-binding: t; -*-

;; 'string:propertize-unibyte'
(let ((print-circle t) (print-escape-newlines t)
      (print-escape-control-characters t))
  (prin1 (condition-case err
             (list 'ok (eval '(let* ((s (unibyte-string 128 255)) (result (propertize s 'face 'bold))) (list (multibyte-string-p result) (string-to-list result) (aref result 0) (vconcat result) (get-text-property 0 'face result))) t))
           (error (list 'error err))))
  (terpri))
