;;; -*- lexical-binding: nil; -*-
;; string-pad ignores PADDING when no padding is needed (#399).
(qa (string-pad "long" 1 "not-a-character") "long")
(princ (qa--done))
