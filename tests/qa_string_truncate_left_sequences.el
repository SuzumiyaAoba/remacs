;;; -*- lexical-binding: nil; -*-
;; string-truncate-left uses sequence length before substring/concat (#396).
(qa (string-truncate-left [65] 2) [65])
(qa (string-truncate-left [97 98 99 100] 3) "...d")
(princ (qa--done))
