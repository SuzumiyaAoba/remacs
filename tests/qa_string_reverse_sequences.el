;;; -*- lexical-binding: nil; -*-
(require 'subr-x)
;; string-reverse follows reverse's supported sequence types (#408).
(qa (string-reverse [97 98]) [98 97])
(qa (string-reverse '(a b)) '(b a))
(let ((v (make-bool-vector 3 nil)))
  (aset v 0 t)
  (qa (string-reverse v) (bool-vector nil nil t)))
(princ (qa--done))
