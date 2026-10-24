;;; -*- lexical-binding: t; -*-

;; 'string:fillarray-unibyte-ascii'
(let ((print-circle t) (print-escape-newlines t)
      (print-escape-control-characters t))
  (prin1 (condition-case err
             (list 'ok (eval '(let ((s (unibyte-string 128 255))) (list (condition-case err (progn (fillarray s 97) 'filled) (error (car err))) (multibyte-string-p s) (string-to-list s))) t))
           (error (list 'error err))))
  (terpri))

;; 'string:fillarray-unibyte-high'
(let ((print-circle t) (print-escape-newlines t)
      (print-escape-control-characters t))
  (prin1 (condition-case err
             (list 'ok (eval '(let ((s (unibyte-string 128 255))) (list (condition-case err (progn (fillarray s 233) 'filled) (error (car err))) (multibyte-string-p s) (string-to-list s))) t))
           (error (list 'error err))))
  (terpri))

;; 'string:fillarray-multibyte-ascii'
(let ((print-circle t) (print-escape-newlines t)
      (print-escape-control-characters t))
  (prin1 (condition-case err
             (list 'ok (eval '(let ((s (with-temp-buffer (insert "aa") (buffer-string)))) (list (condition-case err (progn (fillarray s 233) 'filled) (error (car err))) (multibyte-string-p s) (string-to-list s))) t))
           (error (list 'error err))))
  (terpri))
