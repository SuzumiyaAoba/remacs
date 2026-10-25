;;; -*- lexical-binding: nil; -*-
;; butlast arithmetic preserves float counts until integer-only take/nthcdr (#492).
(qa-err-data (butlast '(a b) 0.5) 'wrong-type-argument '(integerp 1.5))
(qa-err-data (butlast '(a b) 1.5) 'wrong-type-argument '(integerp 0.5))
(qa-err-data (nbutlast '(a b) 0.5) 'wrong-type-argument '(integerp 0.5))
(qa-err-data (nbutlast '(a b) 1.5) 'wrong-type-argument '(integerp -0.5))
(qa (butlast '(a b) -0.5) '(a b))
(qa (nbutlast '(a b) -0.5) '(a b))
(qa (butlast '(a b) 0.0) '(a b))
(qa (nbutlast '(a b) 0.0) '(a b))
(qa-err-data (butlast '(a b) 0.0e+NaN) 'wrong-type-argument
             '(integerp 0.0e+NaN))
(qa (nbutlast '(a b) 0.0e+NaN) nil)
(qa (butlast '(a b) -1.0e+INF) '(a b))
(qa (nbutlast '(a b) -1.0e+INF) '(a b))
(qa-err-data (butlast '(a b) 1.0e+INF) 'wrong-type-argument
             '(integerp -1.0e+INF))
(qa (nbutlast '(a b) 1.0e+INF) nil)
(princ (qa--done))
