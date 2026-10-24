;;; -*- lexical-binding: t; -*-

;; 'string-to-list:list-alias'
(let ((print-circle t) (print-escape-newlines t)
      (print-escape-control-characters t))
  (prin1 (condition-case err
             (list 'ok (eval '(let* ((source (list 1 2)) (out (string-to-list source))) (let ((shared (eq source out))) (setcar out 9) (list shared source out))) t))
           (error (list 'error err))))
  (terpri))

;; 'string-to-list:improper-list'
(let ((print-circle t) (print-escape-newlines t)
      (print-escape-control-characters t))
  (prin1 (condition-case err
             (list 'ok (eval '(string-to-list '(1 . 2)) t))
           (error (list 'error err))))
  (terpri))
