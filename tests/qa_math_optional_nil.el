;;; -*- lexical-binding: nil; -*-
;; Explicit nil has the same meaning as an omitted BASE/X (#542).
(dolist (function '(log atan))
  (dolist (number '(0 1 -1 2 1024 0.0 -0.0 0.5 -0.5
                   1.0e+INF -1.0e+INF 0.0e+NaN))
    (let ((omitted (funcall function number))
          (explicit (funcall function number nil)))
      (qa (if (isnan omitted) (isnan explicit) (equal explicit omitted)) t)))
  (qa-err-data (funcall function nil nil)
               'wrong-type-argument '(numberp nil))
  (qa-err-data (funcall function 'first 'second)
               'wrong-type-argument '(numberp first))
  (qa-err-data (funcall function 1 'second)
               'wrong-type-argument '(numberp second)))
;; Zero is supplied, and must select the two-argument calculation.
(qa (atan 1 0) (/ float-pi 2))
(qa (atan -1 0) (- (/ float-pi 2)))
(qa (log 8 2) 3.0)
(qa (log 1000 10) 3.0)
(qa (log 2 0) -0.0)
(qa (apply #'log '(2 nil)) (log 2))
(qa (apply #'atan '(1 nil)) (atan 1))
(princ (qa--done))
