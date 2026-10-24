;;; -*- lexical-binding: t; -*-

;; 'bytecode:code-slot-control'
(let ((print-circle t) (print-escape-newlines t)
      (print-escape-control-characters t))
  (prin1 (condition-case err
             (list 'ok (eval '(let* ((f (make-byte-code 0 (unibyte-string 192 135) (vector 11 22) 1)) (before (funcall f))) (let ((result (condition-case err (progn (aset f 1 (unibyte-string 193 135)) 'stored) (error (list (car err) (cadr err)))))) (list before result (funcall f)))) t))
           (error (list 'error err))))
  (terpri))

;; 'bytecode:constants-slot-control'
(let ((print-circle t) (print-escape-newlines t)
      (print-escape-control-characters t))
  (prin1 (condition-case err
             (list 'ok (eval '(let* ((f (make-byte-code 0 (unibyte-string 192 135) (vector 11 22) 1)) (before (funcall f))) (let ((result (condition-case err (progn (aset f 2 (vector 33)) 'stored) (error (list (car err) (cadr err)))))) (list before result (funcall f)))) t))
           (error (list 'error err))))
  (terpri))
