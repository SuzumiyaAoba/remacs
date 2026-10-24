;;; -*- lexical-binding: t; -*-

;; 'bytecode:code-in-place'
(let ((print-circle t) (print-escape-newlines t)
      (print-escape-control-characters t))
  (prin1 (condition-case err
             (list 'ok (eval '(let* ((f (make-byte-code 0 (unibyte-string 192 135) (vector 11 22) 1)) (before (funcall f))) (aset (aref f 1) 0 193) (list before (funcall f))) t))
           (error (list 'error err))))
  (terpri))

;; 'bytecode:constants-in-place-control'
(let ((print-circle t) (print-escape-newlines t)
      (print-escape-control-characters t))
  (prin1 (condition-case err
             (list 'ok (eval '(let* ((f (make-byte-code 0 (unibyte-string 192 135) (vector 11 22) 1)) (before (funcall f))) (aset (aref f 2) 0 33) (list before (funcall f))) t))
           (error (list 'error err))))
  (terpri))
