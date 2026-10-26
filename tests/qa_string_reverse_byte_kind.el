;;; -*- lexical-binding: nil; -*-
(require 'subr-x)
;; string-reverse preserves unibyte representation for high bytes (#384).
(let* ((s (unibyte-string 233 255))
       (r (string-reverse s)))
  (qa (multibyte-string-p r) nil)
  (qa (string-bytes r) 2)
  (qa (equal r (unibyte-string 255 233)) t))
(princ (qa--done))
