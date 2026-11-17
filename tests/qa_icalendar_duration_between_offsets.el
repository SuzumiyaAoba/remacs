;;; qa_icalendar_duration_between_offsets.el --- account for explicit UTC offsets in duration -*- lexical-binding: t; -*-

(require 'icalendar-utils)

(qa
 (list
  (let* ((start (icalendar-make-date-time
                 :second 0 :minute 0 :hour 12
                 :day 1 :month 1 :year 2025 :zone 0))
         (end (icalendar-make-date-time
               :second 0 :minute 0 :hour 12
               :day 1 :month 1 :year 2025 :zone -3600))
         (duration (icalendar-duration-between start end)))
    (list (icalendar-print-dur-value duration)
          (time-convert (time-subtract (encode-time end) (encode-time start))
                        'integer)))
  (let* ((start (icalendar-make-date-time
                 :second 0 :minute 0 :hour 12
                 :day 9 :month 3 :year 2024))
         (end (icalendar-make-date-time
               :second 0 :minute 0 :hour 12
               :day 10 :month 3 :year 2024)))
    (icalendar-print-dur-value (icalendar-duration-between start end))))
 '(("PT1H" 3600) "P1D"))

(princ (qa--done))
