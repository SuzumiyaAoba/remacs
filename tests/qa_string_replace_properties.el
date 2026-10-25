;;; -*- lexical-binding: nil; -*-
;; string-replace copies TO-STRING's text properties (#373).
(let ((to (propertize "X" 'face 'bold)))
  (qa (get-text-property 0 'face (string-replace "a" to "a")) 'bold))
(princ (qa--done))
