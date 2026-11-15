;;; -*- lexical-binding: t; -*-
;;; Calendar day-of-year conversion boundary cases (Issues #2278 and #2322).

(require 'calendar)

(qa (calendar-date-from-day-of-year 1 1) '(1 1 1))
(qa (calendar-date-from-day-of-year 1 -1) '(12 31 1))

(qa-err-data (calendar-date-from-day-of-year -1 1.5)
  'wrong-type-argument '(integer-or-marker-p 1.5))
(qa-err-data (calendar-date-from-day-of-year -1 -1.5)
  'wrong-type-argument '(integer-or-marker-p -1.5))

(princ (qa--done))
