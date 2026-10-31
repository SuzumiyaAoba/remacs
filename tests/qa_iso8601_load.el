;;; -*- lexical-binding: t -*-
;; Loading the complete source must define the parsers and its final feature.
;; Regressions for #2012/#3015: no stubs for this library or its dependents.
(require 'iso8601)

(qa (featurep 'iso8601) t)
(qa (iso8601--time-in-range-p "12:34:56") t)
(qa (iso8601--time-in-range-p "25:00:00") nil)
(qa (cl-subseq (iso8601-parse-time "12:34:56Z") 0 3) '(56 34 12))
(qa (iso8601-parse-zone "+01:30") 90)
(qa (decoded-time-second (iso8601-parse-duration "PT2.5S")) '(25 . 10))
(qa (and (iso8601-valid-p "2024-02-29T12:34:56Z") t) t)
(qa (iso8601-valid-p "2023-02-29T12:34:56Z") nil)

;; Cookie expiration and other RFC date consumers use the real parse-time
;; fallback after the ISO parser rejects a non-ISO date.
(qa (format-time-string "%Y-%m-%d %H:%M:%S"
                        (date-to-time "Thu, 01 Jan 1970 00:00:00 GMT") t)
    "1970-01-01 00:00:00")

(princ (qa--done))
