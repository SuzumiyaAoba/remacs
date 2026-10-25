;;; -*- lexical-binding: nil; -*-
;; logb/frexp do not round the exponent up just below a power of two (#540).
(qa (list (logb 1023.9999999999999)
          (frexp 1023.9999999999999))
    '(9 (0.9999999999999999 . 10)))
(princ (qa--done))
