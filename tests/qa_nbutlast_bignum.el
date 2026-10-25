;;; -*- lexical-binding: nil; -*-
;; nbutlast compares arbitrary-size counts before narrowing (#490).
(qa (nbutlast '(a b) (expt 2 200)) nil)
(qa (nbutlast '(a b) (- (expt 2 200))) '(a b))
(qa (let ((list (list 'a 'b)))
      (list (nbutlast list (expt 2 200)) list))
    '(nil (a b)))
(qa-err-data (nbutlast 42 (expt 2 200)) 'wrong-type-argument '(sequencep 42))
(princ (qa--done))
