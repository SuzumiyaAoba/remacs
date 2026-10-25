;;; -*- lexical-binding: nil; -*-
;; string-fill retains a single trailing newline (#406).
(qa (string-fill "abc\n" 80) "abc\n")
(princ (qa--done))
