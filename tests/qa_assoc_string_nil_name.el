;;; -*- lexical-binding: nil; -*-
;; assoc-string treats nil as the symbol name "nil" (#531).
(qa (list (assoc-string nil (list (cons nil 'value)))
          (assoc-string "nil" (list (cons nil 'value))))
    '((nil . value) (nil . value)))
;; KEY validation remains lazy until a comparable string/symbol candidate.
(qa (assoc-string 5 '(1 2)) nil)
(qa-err-data (assoc-string 5 (list (cons nil 'value)))
             'wrong-type-argument '(stringp 5))
(princ (qa--done))
