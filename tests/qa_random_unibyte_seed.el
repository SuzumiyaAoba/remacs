;;; -*- lexical-binding: nil; -*-
;; Random string seeds use Emacs storage bytes (#486).
(qa (let (a b)
      (random (unibyte-string 233))
      (setq a (list (random 100) (random 100) (random 100)))
      (random "é")
      (setq b (list (random 100) (random 100) (random 100)))
      (list a b))
    '((5 4 35) (79 41 4)))
(princ (qa--done))
