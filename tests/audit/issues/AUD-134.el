;;; -*- lexical-binding: t; -*-

;; 'keymap:key-parse-repeat'
(let ((print-circle t) (print-escape-newlines t)
      (print-escape-control-characters t))
  (prin1 (condition-case err
             (list 'ok (eval '(list (append (key-parse "3*C-x") nil) (append (key-parse "2*a") nil)) t))
           (error (list 'error err))))
  (terpri))

;; 'keymap:key-parse-repeat-control'
(let ((print-circle t) (print-escape-newlines t)
      (print-escape-control-characters t))
  (prin1 (condition-case err
             (list 'ok (eval '(list (append (key-parse "C-x C-x C-x") nil) (append (key-parse "a a") nil)) t))
           (error (list 'error err))))
  (terpri))
