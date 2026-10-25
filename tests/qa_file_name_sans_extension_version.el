;;; -*- lexical-binding: nil; -*-
;; file-name-sans-extension removes backup version before extension (#464).
(qa (file-name-sans-extension "archive.tar.gz.~1~") "archive.tar")
(princ (qa--done))
