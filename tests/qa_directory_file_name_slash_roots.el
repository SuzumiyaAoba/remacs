;;; -*- lexical-binding: nil; -*-
;; directory-file-name keeps at most two root slashes (#460).
(qa (list (directory-file-name "/")
          (directory-file-name "//")
          (directory-file-name "///")
          (directory-file-name "////"))
    '("/" "//" "/" "/"))
(princ (qa--done))
