;;; -*- lexical-binding: nil; -*-
;; byte-to-string accepts only byte values (#425).
(qa-err-data (byte-to-string 256) 'args-out-of-range '(256))
(qa-err-data (byte-to-string -1) 'args-out-of-range '(-1))
(princ (qa--done))
