;;; -*- lexical-binding: nil; -*-
;; file-name-extension strips backup version suffix first (#465).
(qa (file-name-extension "archive.tar.gz.~1~") "gz")
(princ (qa--done))
