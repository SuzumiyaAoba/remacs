;;; -*- lexical-binding: nil; -*-
;; Batch Info node lookup remains usable without display fontification (#585).
(require 'info)
(qa
 (progn
   (Info-find-node (expand-file-name "info/elisp.info") "Top")
   (list Info-current-node (> (buffer-size) 0)))
 '("Top" t))
(princ (qa--done))
