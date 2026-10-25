;;; -*- lexical-binding: nil; -*-
;; frexp keeps normalized mantissas across the float exponent range (#541).
(qa (mapcar #'frexp '(1023.9999999999999
                      1.7976931348623157e+308
                      5.0e-324))
    '((0.9999999999999999 . 10)
      (0.9999999999999999 . 1024)
      (0.5 . -1073)))
(qa (eql (car (frexp -0.0)) -0.0) t)
(qa (cdr (frexp -0.0)) 0)
(qa (frexp 1.0e+INF) '(1.0e+INF . 0))
(qa (frexp -1.0e+INF) '(-1.0e+INF . 0))
(qa (let ((result (frexp 0.0e+NaN)))
      (list (isnan (car result)) (cdr result)))
    '(t 0))
(qa-err-data (frexp "not a number") 'wrong-type-argument '(numberp "not a number"))
(princ (qa--done))
