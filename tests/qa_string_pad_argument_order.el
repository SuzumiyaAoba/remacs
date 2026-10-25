;;; -*- lexical-binding: nil; -*-
;; string-pad validates LENGTH before STRING (#398).
(qa-err-data (string-pad 42 -1) 'wrong-type-argument '(natnump -1))
(princ (qa--done))
