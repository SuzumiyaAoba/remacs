;;; -*- lexical-binding: nil; -*-
;; directory-file-name preserves one or two root slashes only (#460).
(qa (list (directory-file-name "/")
          (directory-file-name "//")
          (directory-file-name "///"))
    '("/" "//" "/"))
(princ (qa--done))
