;;; -*- lexical-binding: nil; -*-
;; delete returns the original string/vector when no element matches (#388).
(qa (let ((s "aba")) (eq s (delete ?z s))) t)
(qa (let ((v [a b])) (eq v (delete 'z v))) t)
(princ (qa--done))
