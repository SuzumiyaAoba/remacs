;;; -*- lexical-binding: nil; -*-
;; Padding starts from END-COLUMN if START-COLUMN is past the string (#364).
(qa (truncate-string-to-width "a" 10 5 ?x) "xxxxxxxxxx")
(qa-err-data (truncate-string-to-width "a" 3 0 'invalid-padding)
             'wrong-type-argument '(characterp invalid-padding))
(qa (truncate-string-to-width "a" 1 0 'invalid-padding) "a")
(princ (qa--done))
