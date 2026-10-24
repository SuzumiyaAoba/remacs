;;; -*- lexical-binding: t; -*-

;; 'string-width:tab-width'
(let ((print-circle t) (print-escape-newlines t)
      (print-escape-control-characters t))
  (prin1 (condition-case err
             (list 'ok (eval '(let ((tab-width 4)) (list (char-width 9) (string-width "\11") (truncate-string-to-width "\11X" 4))) t))
           (error (list 'error err))))
  (terpri))
