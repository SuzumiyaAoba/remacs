;;; -*- lexical-binding: nil; -*-
;; Positioned symbols print through their underlying symbol view (#588).
(qa
 (let* ((print-gensym t)
        (p (position-symbol (make-symbol "a b") 17)))
   (prin1-to-string p))
 "#<symbol #:a\\ b at 17>")
(qa
 (let* ((print-gensym t)
        (print-symbols-bare t)
        (p (position-symbol (make-symbol "a b") 17)))
   (prin1-to-string p))
 "#:a\\ b")
(qa
 (let* ((print-gensym t)
        (print-circle t)
        (symbols-with-pos-enabled nil)
        (p (position-symbol (make-symbol "a b") 17)))
   (prin1-to-string (list p p)))
 "(#<symbol #:a\\ b at 17> #<symbol #:a\\ b at 17>)")
(qa
 (let* ((print-gensym t)
        (print-circle t)
        (symbols-with-pos-enabled t)
        (symbol (make-symbol "a b"))
        (p (position-symbol symbol 17)))
   (prin1-to-string (list p symbol)))
 "(#1=#<symbol #1# at 17> #1#)")
(qa
 (prin1-to-string (record 'symbol-with-pos 'alpha 1))
 "#s(symbol-with-pos alpha 1)")
(princ (qa--done))
