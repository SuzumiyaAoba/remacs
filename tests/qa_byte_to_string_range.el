;;; -*- lexical-binding: nil; -*-
;; GNU 31.1 reports out-of-range values with its byte constructor error.
(qa-err-data (byte-to-string 256) 'error '("Invalid byte"))
(qa-err-data (byte-to-string -1) 'error '("Invalid byte"))
(princ (qa--done))
