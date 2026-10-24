;;; -*- lexical-binding: t; -*-

;; 'string:clear-unibyte'
(let ((print-circle t) (print-escape-newlines t)
      (print-escape-control-characters t))
  (prin1 (condition-case err
             (list 'ok (eval '(let ((s (unibyte-string 128 255))) (clear-string s) (list (length s) (string-bytes s) (string-to-list s))) t))
           (error (list 'error err))))
  (terpri))

;; 'string:clear-multibyte-control'
(let ((print-circle t) (print-escape-newlines t)
      (print-escape-control-characters t))
  (prin1 (condition-case err
             (list 'ok (eval '(let ((s (copy-sequence "éあ"))) (clear-string s) (list (length s) (string-bytes s) (multibyte-string-p s) (string-to-list s))) t))
           (error (list 'error err))))
  (terpri))
