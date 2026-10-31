;;; -*- lexical-binding: t -*-
;; #3376: read the whole source, reach its provide, and exercise the parser.
(require 'ietf-drums-date)

(qa (featurep 'ietf-drums-date) t)
(qa (ietf-drums-date--tokenize-string "(note) 01 Jan 1970") '(1 "Jan" 1970))
(qa (ietf-drums-date--tokenize-string "Thu, 01 Jan" t) '("Thu" 44 1 "Jan"))
(qa (ietf-drums-parse-date-string "Thu, 01 Jan 1970 00:00:00 +0000" t t)
    '(0 0 0 1 1 1970 4 -1 0))
(qa (ietf-drums-parse-date-string "Fri, 21 Nov 1997 09:55:06 -0600" t t)
    '(6 55 9 21 11 1997 5 -1 -21600))
(qa (ietf-drums-parse-date-string "01 Jan 1970 00:00 +0130" t t)
    '(0 0 0 1 1 1970 nil -1 5400))
(qa-err (ietf-drums-parse-date-string "Thu, 01 Jan 1970 25:00:00 +0000" t t)
        'date-parse-error)

(princ (qa--done))
