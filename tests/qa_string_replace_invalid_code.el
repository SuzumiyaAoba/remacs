;;; -*- lexical-binding: nil; -*-
;; string-replace rejects codes outside Emacs' character range (#404).
(qa-err-data (string-replace "a" [#x400000] "a")
             'wrong-type-argument '(characterp 4194304))
(princ (qa--done))
