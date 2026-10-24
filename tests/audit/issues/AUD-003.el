;;; -*- lexical-binding: t; -*-
;; "equal:cycles"
(let ((print-circle t) (print-escape-newlines t)
      (print-escape-control-characters t))
  (prin1 (condition-case err
             (list 'ok (eval '(let ((a (list 1)) (b (list 1))) (setcdr a a) (setcdr b b) (equal a b)) t))
           (error (list 'error err))))
  (terpri))
