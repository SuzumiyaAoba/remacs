;;; -*- lexical-binding: nil; -*-
;; Empty string-fill input does not inspect unused WIDTH (#407).
(qa (string-fill "" 'invalid-width) "")
(princ (qa--done))
