;;; -*- lexical-binding: nil; -*-
;; string-search distinguishes unibyte bytes from Unicode characters (#433).
(qa (string-search "é" (unibyte-string 233)) nil)
(qa (string-search (unibyte-string 233) "é") nil)
(qa (string-search (unibyte-string 233) (unibyte-string 233)) 0)
(princ (qa--done))
