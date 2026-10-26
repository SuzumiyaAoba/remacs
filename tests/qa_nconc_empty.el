;;; -*- lexical-binding: nil; -*-
;; #645: the empty argument list must not reach args.last().unwrap().
(qa (nconc) nil)
(qa (apply #'nconc nil) nil)
(qa (nconc nil) nil)
(qa (nconc nil nil) nil)
(qa (nconc 'tail) 'tail)
(princ (qa--done))
