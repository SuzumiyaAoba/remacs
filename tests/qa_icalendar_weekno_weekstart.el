;;; qa_icalendar_weekno_weekstart.el --- honor the iCalendar week start day -*- lexical-binding: t; -*-

(require 'icalendar-utils)

(qa
 (list
  (icalendar-date/time-weekno '(1 5 2025) 0)
  (icalendar-date/time-weekno '(1 5 2025) 1)
  (icalendar-date/time-weekno '(1 5 2025) 6)
  (icalendar-date/time-weekno '(1 1 2021) 1)
  (condition-case nil
      (progn (icalendar-date/time-weekno '(1 1 2025) 7) nil)
    (error t)))
 '(2 1 1 53 t))

(princ (qa--done))
