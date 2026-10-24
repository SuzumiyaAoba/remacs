;;; -*- lexical-binding: t; -*-
;; "hash:custom-test"
(let ((print-circle t) (print-escape-newlines t)
      (print-escape-control-characters t))
  (prin1 (condition-case err
             (list 'ok (eval '(progn (define-hash-table-test 'audit-case-fold (lambda (a b) (string= (downcase a) (downcase b))) (lambda (a) (sxhash-equal (downcase a)))) (let ((h (make-hash-table :test 'audit-case-fold))) (puthash "HELLO" 42 h) (list (gethash "hello" h) (hash-table-test h)))) t))
           (error (list 'error err))))
  (terpri))
