;;; -*- lexical-binding: nil; -*-
;; Sequence replacements use concat's characterp validation (#403).
(qa-err-data (string-replace "a" [not-a-character] "a")
             'wrong-type-argument '(characterp not-a-character))
(princ (qa--done))
