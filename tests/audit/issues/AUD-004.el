;;; -*- lexical-binding: t; -*-
;; "equal:deep-different-lists"
(let ((print-circle t) (print-escape-newlines t)
      (print-escape-control-characters t))
  (prin1 (condition-case err
             (list 'ok (eval '(let ((a 1) (b 2)) (dotimes (_ 205) (setq a (list a) b (list b))) (equal a b)) t))
           (error (list 'error err))))
  (terpri))
