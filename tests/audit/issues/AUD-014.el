;;; -*- lexical-binding: t; -*-
;; "hash:cycle-lookup"
(let ((print-circle t) (print-escape-newlines t)
      (print-escape-control-characters t))
  (prin1 (condition-case err
             (list 'ok (eval '(let* ((a (list 1)) (b (list 1)) (h (make-hash-table :test 'equal))) (setcdr a a) (setcdr b b) (puthash a 42 h) (gethash b h)) t))
           (error (list 'error err))))
  (terpri))
