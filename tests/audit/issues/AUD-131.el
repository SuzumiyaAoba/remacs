;;; -*- lexical-binding: t; -*-

;; 'macroexpand:atom-unused-environment'
(let ((print-circle t) (print-escape-newlines t)
      (print-escape-control-characters t))
  (prin1 (condition-case err
             (list 'ok (eval '(macroexpand "x" "bad") t))
           (error (list 'error err))))
  (terpri))

;; 'macroexpand:single-atom-unused-environment'
(let ((print-circle t) (print-escape-newlines t)
      (print-escape-control-characters t))
  (prin1 (condition-case err
             (list 'ok (eval '(macroexpand-1 "x" "bad") t))
           (error (list 'error err))))
  (terpri))

;; 'macroexpand:all-atom-unused-environment'
(let ((print-circle t) (print-escape-newlines t)
      (print-escape-control-characters t))
  (prin1 (condition-case err
             (list 'ok (eval '(macroexpand-all "x" "bad") t))
           (error (list 'error err))))
  (terpri))

;; 'macroexpand:bad-environment-used-control'
(let ((print-circle t) (print-escape-newlines t)
      (print-escape-control-characters t))
  (prin1 (condition-case err
             (list 'ok (eval '(condition-case err (macroexpand '(when t 1) 5) (error (car err))) t))
           (error (list 'error err))))
  (terpri))
