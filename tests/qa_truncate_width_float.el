;;; -*- lexical-binding: nil; -*-
;; Floating END-COLUMN is compared before truncating (#367).
(qa (truncate-string-to-width "abc" 2.5) "ab")
(princ (qa--done))
