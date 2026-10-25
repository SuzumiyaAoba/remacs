;;; -*- lexical-binding: nil; -*-
;; concat sequences preserve high-Mule Emacs characters (#380).
(dolist (code '(#x110000 #x3fff7f))
  (qa (string= (concat (vector code)) (string code)) t)
  (qa (string= (concat (list code)) (string code)) t))
(princ (qa--done))
