;;; -*- lexical-binding: t; -*-
;; A non-recurring event's TZID must determine its local index date (#1951).

(require 'icalendar-parser)

(qa
 (with-temp-buffer
   (insert (concat
            "BEGIN:VCALENDAR\nVERSION:2.0\n"
            "BEGIN:VTIMEZONE\nTZID:Pacific/Kiritimati\n"
            "BEGIN:STANDARD\nDTSTART:20250101T000000\n"
            "TZOFFSETFROM:+1400\nTZOFFSETTO:+1400\n"
            "END:STANDARD\nEND:VTIMEZONE\n"
            "BEGIN:VEVENT\nUID:cross-midnight\n"
            "DTSTART;TZID=Pacific/Kiritimati:20250102T003000\n"
            "SUMMARY:Cross-midnight\nEND:VEVENT\nEND:VCALENDAR\n"))
   (let* ((parsed (icalendar-parse-and-index))
          (index (cadr parsed)))
     (list (length (icalendar-index-get index :date '(1 1 2025)))
           (length (icalendar-index-get index :date '(1 2 2025))))))
 '(1 0))

(princ (qa--done))
