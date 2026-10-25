;;; -*- lexical-binding: nil; -*-
;; truncate-string-to-width compares floating START-COLUMN values (#431).
(qa (truncate-string-to-width "abcdef" 4 1.5) "cd")
(princ (qa--done))
