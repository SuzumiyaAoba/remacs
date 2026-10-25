;;; -*- lexical-binding: nil; -*-
;; string-version-lessp treats nil as the symbol name "nil" (#413).
(qa (string-version-lessp nil "x") t)
(princ (qa--done))
