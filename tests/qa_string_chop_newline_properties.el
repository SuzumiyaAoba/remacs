;;; -*- lexical-binding: nil; -*-
;; string-chop-newline preserves identity or substring properties (#369).
(let ((s (propertize "abc" 'face 'bold)))
  (qa (eq (string-chop-newline s) s) t)
  (qa (get-text-property 0 'face (string-chop-newline (concat s "\n")))
      'bold))
(princ (qa--done))
