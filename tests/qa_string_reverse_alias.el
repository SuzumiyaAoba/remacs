;;; -*- lexical-binding: nil; -*-
;; string-reverse is the obsolete alias of reverse, not a separate subr (#487).
(require 'subr-x)
(qa (symbol-function 'string-reverse) 'reverse)
(qa (string-reverse [1 2]) [2 1])
(princ (qa--done))
