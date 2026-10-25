;;; -*- lexical-binding: nil; -*-
;; string-version-lessp follows gnulib hidden-file and shared-extension order (#412).
(qa (list (string-version-lessp ".a" "a")
          (string-version-lessp "a.tar.gz" "a.1.tar.gz"))
    '(t t))
(princ (qa--done))
