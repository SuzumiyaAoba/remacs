;;; -*- lexical-binding: nil; -*-
;; string-truncate-left preserves properties on the retained suffix (#395).
(let ((s (propertize "abcdefghij" 'face 'bold)))
  (qa (get-text-property 0 'face (string-truncate-left s 8)) 'bold))
(princ (qa--done))
