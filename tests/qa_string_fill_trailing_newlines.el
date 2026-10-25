;;; -*- lexical-binding: nil; -*-
;; string-fill keeps only one trailing newline (#420).
(qa (string-fill "abc\n\n" 80) "abc\n")
(princ (qa--done))
