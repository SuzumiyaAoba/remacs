;;; -*- lexical-binding: nil; -*-
;; string-fill inserts unibyte high bytes as byte8 characters (#432).
(qa (string-to-list (string-fill (unibyte-string 233) 80)) '(#x3fffe9))
(princ (qa--done))
