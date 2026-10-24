;;; -*- lexical-binding: t; -*-

;; 'shell:initial-shell-environment'
(let ((print-circle t) (print-escape-newlines t)
      (print-escape-control-characters t))
  (prin1 (condition-case err
             (list 'ok (eval '(list (or (getenv "ESHELL") (getenv "SHELL") "/bin/sh") shell-file-name) t))
           (error (list 'error err))))
  (terpri))

;; 'shell:explicit-shell-control'
(let ((print-circle t) (print-escape-newlines t)
      (print-escape-control-characters t))
  (prin1 (condition-case err
             (list 'ok (eval '(let ((shell-file-name "/bin/sh")) (shell-command-to-string "printf audit-shell-ok")) t))
           (error (list 'error err))))
  (terpri))
