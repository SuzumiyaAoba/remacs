;;; -*- lexical-binding: nil; -*-
;; string-chop-newline keeps unibyte representation (#390).
(let* ((s (unibyte-string 233 10))
       (result (string-chop-newline s)))
  (qa (multibyte-string-p result) nil)
  (qa (string-bytes result) 1))
(princ (qa--done))
