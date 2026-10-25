;;; -*- lexical-binding: nil; -*-
;; string-lines nil returns its one retained final field (#419).
(qa (string-lines nil) '(nil))
(princ (qa--done))
