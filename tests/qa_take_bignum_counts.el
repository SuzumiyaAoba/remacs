;;; -*- lexical-binding: nil; -*-
;; take/ntake accept bignum counts without narrowing to machine sizes (#454).
(let ((items '(a b)))
  (qa (list (take (expt 2 200) items)
            (ntake (expt 2 64) (copy-sequence items))
            (take (- (expt 2 200)) 'x))
      '((a b) (a b) nil)))
(princ (qa--done))
