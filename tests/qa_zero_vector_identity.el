;;; -*- lexical-binding: nil; -*-
;; Empty ordinary vectors share GNU's zero_vector across constructors (#368).
(let ((v []))
  (qa (eq v (vector)) t)
  (qa (eq v (make-vector 0 nil)) t)
  (qa (eq v (copy-sequence v)) t)
  (qa (eq v (reverse v)) t)
  (qa (eq v (vconcat)) t)
  (qa (eq v (substring v)) t))
(princ (qa--done))
