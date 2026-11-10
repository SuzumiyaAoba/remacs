;;; -*- lexical-binding: t; -*-
;; Rect and its Calc consumer load from the shipped Lisp sources (#1976).

(qa (require 'rect) 'rect)
(qa (featurep 'rect) t)
(qa (require 'calc) 'calc)
(qa (featurep 'calc) t)

(princ (qa--done))
