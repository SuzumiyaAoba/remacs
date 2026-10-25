;;; -*- lexical-binding: nil; -*-
;; Replacement preserves the source's multibyte representation and TO's properties (#614).
(qa (multibyte-string-p
     (string-replace "é" "a" (string 233 ?x)))
    t)
(qa (object-intervals
     (string-replace "x" (propertize "Y" 'face 'bold) "axb"))
    '((0 1 nil) (1 2 (face bold)) (2 3 nil)))
(princ (qa--done))
