;;; -*- lexical-binding: nil; -*-
;; Byte-code add1/sub1/unary-minus preserve marker arithmetic semantics (#595).
(qa
 (with-temp-buffer
   (list (funcall (byte-compile '(lambda (value) (1+ value))) (point-marker))
         (funcall (byte-compile '(lambda (value) (1- value))) (point-marker))
         (funcall (byte-compile '(lambda (value) (- value))) (point-marker))))
 '(2 0 -1))
(qa
 (let ((m (make-marker)))
   (condition-case err
       (funcall (byte-compile '(lambda (value) (1+ value))) m)
     (error (error-message-string err))))
 "Marker does not point anywhere")
(princ (qa--done))
