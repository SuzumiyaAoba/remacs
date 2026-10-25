;;; -*- lexical-binding: nil; -*-
;; file-relative-name uses compare-strings case folding for Unicode paths (#466).
(when (file-name-case-insensitive-p temporary-file-directory)
  (let ((base (concat temporary-file-directory "Ä/"))
        (file (concat temporary-file-directory "ä/leaf")))
    (qa (file-relative-name file base) "leaf")))
(princ (qa--done))
