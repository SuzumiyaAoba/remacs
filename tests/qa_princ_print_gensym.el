;;; -*- lexical-binding: nil; -*-
;; `princ' honors print-gensym without escaping symbol names (#589).
(qa
 (let ((print-gensym t))
   (with-output-to-string
     (princ (make-symbol "alpha"))))
 "#:alpha")
(qa
 (let ((print-gensym nil))
   (with-output-to-string
     (princ (make-symbol "alpha"))))
 "alpha")
(qa
 (let* ((print-gensym t)
        (print-circle t)
        (symbol (make-symbol "alpha")))
   (with-output-to-string
     (princ (list symbol symbol))))
 "(#1=#:alpha #1#)")
(qa
 (let ((print-gensym t))
   (with-output-to-string
     (princ (make-symbol "a b"))))
 "#:a b")
(princ (qa--done))
