;;; -*- lexical-binding: nil; -*-
;; subst-char-in-string preserves unibyte results (#392).
(let ((result (subst-char-in-string ?a ?b (unibyte-string 97 233))))
  (qa (list (multibyte-string-p result) (string-bytes result)) '(nil 2)))
(princ (qa--done))
