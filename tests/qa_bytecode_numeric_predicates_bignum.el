;;; -*- lexical-binding: nil; -*-
;; Numeric predicate byte-code instructions recognize bignums (#594).
(qa
 (funcall (byte-compile '(lambda (x) (list (integerp x) (numberp x))))
          2305843009213693952)
 '(t t))
(qa
 (funcall (byte-compile '(lambda (x) (list (integerp x) (numberp x)))
                        )
          1.5)
 '(nil t))
(princ (qa--done))
