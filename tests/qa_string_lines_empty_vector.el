;;; -*- lexical-binding: nil; -*-
;; string-lines includes an empty vector as its one empty sequence field (#414).
(qa (string-lines []) '([]))
(princ (qa--done))
