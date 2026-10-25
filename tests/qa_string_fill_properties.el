;;; -*- lexical-binding: nil; -*-
;; string-fill keeps properties on retained characters (#370).
(let ((s (propertize "abc" 'face 'bold)))
  (qa (get-text-property 0 'face (string-fill s 80)) 'bold))
(princ (qa--done))
