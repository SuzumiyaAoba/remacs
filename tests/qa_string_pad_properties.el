;;; -*- lexical-binding: nil; -*-
;; string-pad preserves identity and source text properties (#365).
(let ((s (propertize "abc" 'face 'bold)))
  (qa (eq (string-pad s 3) s) t)
  (qa (get-text-property 0 'face (string-pad s 5)) 'bold))
(princ (qa--done))
