;;; -*- lexical-binding: nil; -*-
;; #678: the length-zero loop also skips empty vectors and bool-vectors.
(qa (string-lines []) nil)
(qa (string-lines (make-bool-vector 0 nil)) nil)
(qa (string-lines "") '(""))
(qa (string-lines "" t) nil)
(princ (qa--done))
