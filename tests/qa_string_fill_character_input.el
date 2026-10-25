;;; -*- lexical-binding: nil; -*-
;; string-fill accepts character integers through its GNU insert path (#402).
(qa (string-fill ?a 80) "a")
(princ (qa--done))
