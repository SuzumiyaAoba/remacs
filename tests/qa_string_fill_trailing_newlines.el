;;; -*- lexical-binding: nil; -*-
;; string-fill preserves trailing blank lines (GNU Emacs 31.1).
(qa (string-fill "abc\n\n" 80) "abc\n\n")
(qa (string-fill "abc\n\n\n" 80) "abc\n\n\n")
(princ (qa--done))
