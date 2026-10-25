;;; -*- lexical-binding: nil; -*-
;; string-replace follows concat's byte-kind rule (#391).
(let ((result (string-replace "a" (unibyte-string 98)
                              (unibyte-string 97 233))))
  (qa (list (multibyte-string-p result) (string-bytes result)) '(nil 2)))
(princ (qa--done))
