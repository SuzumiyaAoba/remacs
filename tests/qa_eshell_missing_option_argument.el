;;; qa_eshell_missing_option_argument.el --- signal for a missing required Eshell option value -*- lexical-binding: t; -*-

(require 'esh-opt)

(qa
 (list
  (condition-case nil
      (progn
        (eshell--process-args
         "cmd" '("-o") '((?o nil t output "output")) '(output))
        nil)
    (error t))
  (eshell--process-args
   "cmd" '("-o" "value") '((?o nil t output "output")) '(output))
  (eshell--process-args
   "cmd" '("-ovalue") '((?o nil t output "output")) '(output)))
 '(t ("value") ("value")))

(princ (qa--done))
