;;; -*- lexical-binding: nil; -*-
;; Exact frexp/logb decomposition at normal and subnormal boundaries (#485).
(qa (frexp (ldexp 1.0 1023)) '(0.5 . 1024))
(qa (let ((x (- (ldexp 1.0 500) (ldexp 1.0 447))))
      (list (logb x) (frexp x)))
    '(499 (0.9999999999999999 . 500)))
(qa (let ((x (- (ldexp 1.0 -1022) (ldexp 1.0 -1074))))
      (list (logb x) (frexp x)))
    '(-1023 (0.9999999999999998 . -1022)))
(qa (list (logb 1.7976931348623157e+308)
          (frexp 1.7976931348623157e+308))
    '(1023 (0.9999999999999999 . 1024)))
(qa (frexp 5.0e-324) '(0.5 . -1073))
(princ (qa--done))
