;;; -*- lexical-binding: nil; -*-
;; string-reverse is the obsolete alias of reverse, not a separate subr (#487).
(require 'subr-x)
(qa (eq (symbol-function 'string-reverse) (symbol-function 'reverse)) t)
(qa (string-reverse [1 2]) [2 1])
(princ (qa--done))
