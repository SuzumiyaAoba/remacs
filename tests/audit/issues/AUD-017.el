;;; -*- lexical-binding: t; -*-
;; "hash:size-growth"
(let ((print-circle t) (print-escape-newlines t)
      (print-escape-control-characters t))
  (prin1 (condition-case err
             (list 'ok (eval '(let ((h (make-hash-table :size 1))) (dotimes (n 10) (puthash n n h)) (list (hash-table-count h) (> (hash-table-size h) 1))) t))
           (error (list 'error err))))
  (terpri))
