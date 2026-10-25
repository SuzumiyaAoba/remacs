;;; -*- lexical-binding: nil; -*-
;; START-COLUMN inside a wide character gets leading padding (#429).
(qa (truncate-string-to-width "日a" 4 1 ?x) "xax")
(princ (qa--done))
