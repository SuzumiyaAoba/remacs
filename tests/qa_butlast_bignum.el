;;; -*- lexical-binding: nil; -*-
;; butlast never narrows a large positive count to usize (#491).
(qa (butlast '(a b) (expt 2 64)) nil)
(qa (butlast '(a b) (expt 2 200)) nil)
(qa (butlast '(a b) (- (expt 2 200))) '(a b))
(qa-err-data (butlast 42 (expt 2 200)) 'wrong-type-argument '(sequencep 42))
(princ (qa--done))
