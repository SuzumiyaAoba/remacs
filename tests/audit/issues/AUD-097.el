;;; -*- lexical-binding: t; -*-

;; 'string-compare:unibyte-order'
(let ((print-circle t) (print-escape-newlines t)
      (print-escape-control-characters t))
  (prin1 (condition-case err
             (list 'ok (eval '(let ((byte (unibyte-string 233)) (unicode (string 256))) (list (string-lessp byte unicode) (string-lessp unicode byte) (compare-strings byte nil nil unicode nil nil))) t))
           (error (list 'error err))))
  (terpri))

;; 'string-compare:unibyte-unicode'
(let ((print-circle t) (print-escape-newlines t)
      (print-escape-control-characters t))
  (prin1 (condition-case err
             (list 'ok (eval '(let ((byte (unibyte-string 233)) (unicode (string 233))) (list (equal byte unicode) (string= byte unicode) (string-lessp byte unicode) (string-lessp unicode byte) (compare-strings byte nil nil unicode nil nil))) t))
           (error (list 'error err))))
  (terpri))
