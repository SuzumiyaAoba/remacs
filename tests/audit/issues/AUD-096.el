;;; -*- lexical-binding: t; -*-

;; 'string-to-vector:list'
(let ((print-circle t) (print-escape-newlines t)
      (print-escape-control-characters t))
  (prin1 (condition-case err
             (list 'ok (eval '(string-to-vector '(1 2)) t))
           (error (list 'error err))))
  (terpri))

;; 'string-to-vector:vector'
(let ((print-circle t) (print-escape-newlines t)
      (print-escape-control-characters t))
  (prin1 (condition-case err
             (list 'ok (eval '(let* ((source (vector 1 2)) (out (string-to-vector source))) (list (eq source out) out)) t))
           (error (list 'error err))))
  (terpri))

;; 'string-to-vector:bool-vector'
(let ((print-circle t) (print-escape-newlines t)
      (print-escape-control-characters t))
  (prin1 (condition-case err
             (list 'ok (eval '(string-to-vector (bool-vector t nil)) t))
           (error (list 'error err))))
  (terpri))
