;;; -*- lexical-binding: nil; -*-
;; string-replace follows GNU's character and empty-pattern validation order (#615).
(qa-err-data (string-replace "x" [4294967361] "x")
             'wrong-type-argument '(characterp 4294967361))
(qa-err-data (string-replace "" "a" 7)
             'wrong-length-argument '(0))
;; TO is converted through concat only when a replacement will be made.
(qa (string-replace "x" [4294967361] "abc") "abc")
(princ (qa--done))
