;;; -*- lexical-binding: t; -*-

;; 'string:unibyte-reverse'
(let ((print-circle t) (print-escape-newlines t)
      (print-escape-control-characters t))
  (prin1 (condition-case err
             (list 'ok (eval '(let* ((s (unibyte-string 128 255)) (out (reverse s))) (list (multibyte-string-p out) (string-to-list out) (multibyte-string-p s) (string-to-list s))) t))
           (error (list 'error err))))
  (terpri))

;; 'string:unibyte-nreverse'
(let ((print-circle t) (print-escape-newlines t)
      (print-escape-control-characters t))
  (prin1 (condition-case err
             (list 'ok (eval '(let* ((s (unibyte-string 128 255)) (out (nreverse s))) (list (multibyte-string-p out) (string-to-list out) (multibyte-string-p s) (string-to-list s))) t))
           (error (list 'error err))))
  (terpri))
