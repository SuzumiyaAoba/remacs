;;; -*- lexical-binding: nil; -*-
;; concat validates original character codes before narrowing to u32 (#550).
(qa-err-data (concat '(4294967361)) 'wrong-type-argument
             '(characterp 4294967361))
(qa-err-data (concat '(-4294967231)) 'wrong-type-argument
             '(characterp -4294967231))
(qa-err-data (concat [4294967361]) 'wrong-type-argument
             '(characterp 4294967361))
(qa-err-data (concat [-4294967231]) 'wrong-type-argument
             '(characterp -4294967231))
(qa (concat '(0 65 1114112 4194303)) (concat (string 0 65 1114112 4194303)))
(princ (qa--done))
