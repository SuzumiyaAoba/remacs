;;; -*- lexical-binding: nil; -*-
;; string-replace distinguishes unibyte bytes from Unicode scalars (#439).
(let ((s (unibyte-string 233)))
  (qa (eq s (string-replace "é" "X" s)) t))
(qa (string-replace (unibyte-string 233) "X" "é") "é")
(princ (qa--done))
