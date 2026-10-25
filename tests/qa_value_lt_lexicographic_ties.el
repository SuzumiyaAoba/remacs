;;; -*- lexical-binding: nil; -*-
;; value< continues lexicographic comparison when an item compares as tied (#561).
(qa (value< '(1 0) '(1.0 1)) t)
(qa (value< [1 0] [1.0 1]) t)
(qa (value< '(1.0 0) '(1 1)) t)
(qa (value< (list 0.0e+NaN 0) (list 0.0e+NaN 1)) t)
(qa (value< (list (make-hash-table) 0)
            (list (make-hash-table) 1))
    t)
(princ (qa--done))
