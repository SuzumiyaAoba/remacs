;;; -*- lexical-binding: nil; -*-
;; file-name-base removes backup versions before extensions (#451).
(qa (file-name-base "archive.tar.gz.~1~") "archive.tar")
(princ (qa--done))
