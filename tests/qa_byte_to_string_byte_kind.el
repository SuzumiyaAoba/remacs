;;; -*- lexical-binding: nil; -*-
;; byte-to-string always returns one unibyte byte (#426).
(let ((s (byte-to-string 233)))
  (qa (multibyte-string-p s) nil)
  (qa (string-bytes s) 1))
(princ (qa--done))
