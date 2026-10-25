;;; -*- lexical-binding: nil; -*-
;; string-replace rejects empty FROM before checking IN (#436).
(qa-err-data (string-replace "" "x" 42) 'wrong-length-argument '(0))
(princ (qa--done))
