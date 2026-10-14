;; Issue #89: integer exponents must not round through floating point.
(qa (logb most-positive-fixnum) 60)
(qa (logb most-negative-fixnum) 61)
(qa (logb (- most-positive-fixnum)) 60)
(qa (logb 170141183460469231731687303715884105727) 126)
(qa (logb -170141183460469231731687303715884105728) 127)
(qa (logb -170141183460469231731687303715884105727) 126)

;; Check both signs immediately around every power of two representable
;; without overflowing remacs' current i128 integer representation.
(dotimes (offset 126)
  (let* ((exponent (1+ offset))
         (power (ash 1 exponent)))
    (dolist (sign '(1 -1))
      (qa (logb (* sign (1- power))) (1- exponent))
      (qa (logb (* sign power)) exponent)
      (qa (logb (* sign (1+ power))) exponent))))

(qa (logb 0) -1.0e+INF)
(qa (logb 1) 0)
(qa (logb -1) 0)
(qa (logb 0.0) -1.0e+INF)
(qa (logb -0.0) -1.0e+INF)
(qa (logb 7.0) 2)
(qa (logb -8.0) 3)
(qa (logb 0.125) -3)
(qa (logb 1.0e+INF) 1.0e+INF)
(qa (logb -1.0e+INF) 1.0e+INF)
(qa (isnan (logb 0.0e+NaN)) t)
(qa-err-data (logb nil) 'wrong-type-argument '(numberp nil))
(qa-err-data (logb "1") 'wrong-type-argument '(numberp "1"))
(qa-err (logb (make-marker)) 'wrong-type-argument)

(princ (qa--done))
