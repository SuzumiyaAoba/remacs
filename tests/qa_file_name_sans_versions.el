;;; -*- lexical-binding: nil; -*-
;; file-name-sans-versions strips backup suffixes, not extensions (#448).
(qa (file-name-sans-versions "archive.tar.gz~") "archive.tar.gz")
(qa (file-name-sans-versions "archive.tar.gz~" t) "archive.tar.gz~")
(princ (qa--done))
