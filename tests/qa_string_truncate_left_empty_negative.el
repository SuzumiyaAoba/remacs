;;; -*- lexical-binding: nil; -*-
;; Empty input with negative length follows substring's negative-index error (#394).
(qa-err-data (string-truncate-left "" -1)
             'args-out-of-range '("" -1 nil))
(princ (qa--done))
