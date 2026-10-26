;;; -*- lexical-binding: nil; -*-
;; GNU 31.1 verified; regression for #641 (nonterminating position parser).
(dolist (text '("80x24+bad" "80x24+1bad" "80x24+" "80x24+1-"
                "80x24+1+2+3" "80x24+999999999999999999bad"))
  (qa (x-parse-geometry text) nil))
(qa (x-parse-geometry "80x24+1-2")
    '((height . 24) (width . 80) (top . -2) (left . 1)))
(qa (x-parse-geometry "80x24+1")
    '((height . 24) (width . 80) (left . 1)))
(qa (x-parse-geometry "80x24+999999999999999999999-999999999999999999999")
    '((height . 24) (width . 80) (top . -2147483648) (left . 2147483647)))

(princ (qa--done))
