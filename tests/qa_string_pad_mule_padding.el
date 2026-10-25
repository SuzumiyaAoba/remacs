;;; -*- lexical-binding: nil; -*-
;; string-pad accepts high-Mule padding characters (#401).
(qa (equal (string-pad "a" 2 #x110000)
           (concat "a" (string #x110000))) t)
(princ (qa--done))
