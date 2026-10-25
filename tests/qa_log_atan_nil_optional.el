;;; -*- lexical-binding: nil; -*-
;; Explicit nil means the optional base/denominator is omitted (#542).
(qa (log 2 nil) 0.6931471805599453)
(qa (atan 1 nil) 0.7853981633974483)
(qa (log 2) (log 2 nil))
(qa (atan 1) (atan 1 nil))

;; Numeric zero remains supplied, unlike nil.
(qa (< (/ 1.0 (log 2 0)) 0.0) t)
(qa (atan 1 0) 1.5707963267948966)
(qa (atan -0.0 nil) -0.0)
(qa (condition-case e (log 'not-a-number nil) (error (car e)))
    'wrong-type-argument)
(qa (condition-case e (atan 'not-a-number nil) (error (car e)))
    'wrong-type-argument)
(princ (qa--done))
