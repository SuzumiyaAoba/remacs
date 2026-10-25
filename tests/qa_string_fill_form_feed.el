;;; -*- lexical-binding: nil; -*-
;; string-fill respects form-feed paragraph separators (#437).
(qa (string-fill "a\n\f\nb" 80) "a\n\f\nb")
(princ (qa--done))
