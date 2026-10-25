;;; -*- lexical-binding: nil; -*-
;; Character substitution compares and emits high-Mule proxies (#381).
(let ((high #x110000))
  (qa (subst-char-in-string high ?X (string high)) "X")
  (qa (subst-char-in-string ?X high "X") (string high)))
(princ (qa--done))
