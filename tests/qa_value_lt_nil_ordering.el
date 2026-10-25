;;; -*- lexical-binding: nil; -*-
;; value< compares nil as both the symbol "nil" and empty list (#567).
(qa (value< nil 'z) t)
(qa (value< 'z nil) nil)
(qa (value< nil '(1)) t)
(qa (value< '(1) nil) nil)
(qa (value< nil nil) nil)
(qa (value< '(1) '(1 2)) t)
(qa (value< '(1 2) '(1)) nil)
(princ (qa--done))
