;;; -*- lexical-binding: nil; -*-
;; Byte-code primitive dispatch survives uninterning its public name (#596).
(qa
 (let ((fn (byte-compile '(lambda (x y) (+ x y)))))
   (unintern "+" nil)
   (list (funcall fn 1 2) (null (intern-soft "+"))))
 '(3 t))
(princ (qa--done))
