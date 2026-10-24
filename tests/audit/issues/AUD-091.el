;;; -*- lexical-binding: t; -*-

;; 'string:glyph-explicit-composition'
(let ((print-circle t) (print-escape-newlines t)
      (print-escape-control-characters t))
  (prin1 (condition-case err
             (list 'ok (eval '(let ((s (copy-sequence "ab"))) (compose-string s 0 2) (mapcar #'length (string-glyph-split s))) t))
           (error (list 'error err))))
  (terpri))
