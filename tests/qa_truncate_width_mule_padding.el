;;; -*- lexical-binding: nil; -*-
;; Width padding preserves Emacs eight-bit and high-Mule characters (#378).
(dolist (code '(#x3fff80 #x110000))
  (qa (equal (truncate-string-to-width "a" 3 0 code)
             (concat "a" (string code code))) t))
(princ (qa--done))
