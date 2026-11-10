;;; -*- lexical-binding: t; -*-
;; DTSTART is part of an unbounded RRULE's recurrence set (#1938).

(require 'icalendar-parser)

(qa
 (with-temp-buffer
   (insert (concat
            "BEGIN:VCALENDAR\nVERSION:2.0\n"
            "BEGIN:VEVENT\nUID:recurring-start-date\n"
            "DTSTART;VALUE=DATE:20250101\n"
            "RRULE:FREQ=DAILY\n"
            "END:VEVENT\nEND:VCALENDAR\n"))
   (let* ((parsed (icalendar-parse-and-index))
          (index (cadr parsed)))
     (list (length (icalendar-index-get index :date '(1 1 2025)))
           (length (icalendar-index-get index :date '(1 2 2025)))
           (length (icalendar-index-get index :date '(12 31 2024))))))
 '(1 1 0))

(princ (qa--done))
