;;; -*- lexical-binding: nil; -*-
;; subst-char-in-string compares full Lisp character values (#611).
(qa (subst-char-in-string 4294967361 ?x "ABC") "ABC")
(qa
 (string-to-list
  (subst-char-in-string #x110000 ?a (string #x110000 ?x)))
 '(97 120))
(princ (qa--done))
