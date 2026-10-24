;;; -*- lexical-binding: t; -*-
;; "hash:weak-key-gc"
(let ((print-circle t) (print-escape-newlines t)
      (print-escape-control-characters t))
  (prin1 (condition-case err
             (list 'ok (eval '(let ((h (make-hash-table :test 'eq :weakness 'key))) (dotimes (n 1000) (puthash (vector n) 42 h)) (garbage-collect) (garbage-collect) (< (hash-table-count h) 10)) t))
           (error (list 'error err))))
  (terpri))
