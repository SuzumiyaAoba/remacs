;;; -*- lexical-binding: nil; -*-
;; expand-file-name rejects filenames containing NUL (#450).
(qa (condition-case nil
        (expand-file-name (concat "file" (string 0)))
      (error t))
    t)
(princ (qa--done))
