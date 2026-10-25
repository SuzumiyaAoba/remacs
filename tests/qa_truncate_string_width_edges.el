;;; -*- lexical-binding: nil; -*-
;; Padding starts from END-COLUMN if START-COLUMN is past the string (#364).
(qa (truncate-string-to-width "a" 10 5 ?x) "xxxxxxxxxx")
(princ (qa--done))
