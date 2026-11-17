;;; qa_icalendar_period_order_and_duration.el --- validate iCalendar period endpoints and duration -*- lexical-binding: t; -*-

(require 'icalendar-parser)
(require 'icalendar-utils)

(defun qa--icalendar-period-errors-p (value)
  (condition-case nil
      (progn (icalendar-parse-from-string 'icalendar-period value) nil)
    (error t)))

(qa
 (list
  (qa--icalendar-period-errors-p
   "20250102T090000/20250101T100000")
  (qa--icalendar-period-errors-p
   "20250101T090000/-PT1H")
  (not (qa--icalendar-period-errors-p
        "20250101T090000/20250101T100000"))
  (not (qa--icalendar-period-errors-p
        "20250101T090000/PT1H")))
 '(t t t t))

(princ (qa--done))
