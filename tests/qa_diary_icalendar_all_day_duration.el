;;; -*- lexical-binding: t; -*-
;; All-day recurrence duration covers each diary date in the occurrence (#1969).

(require 'diary-icalendar)
(require 'icalendar-parser)

(defvar date)
(defvar entry)

(qa
 (with-temp-buffer
   (insert (concat
            "BEGIN:VCALENDAR\nVERSION:2.0\n"
            "BEGIN:VEVENT\nUID:two-day-all-day\n"
            "DTSTART;VALUE=DATE:20250106\n"
            "DURATION:P2D\n"
            "RRULE:FREQ=WEEKLY;BYDAY=MO;COUNT=2\n"
            "SUMMARY:Conference\nEND:VEVENT\nEND:VCALENDAR\n"))
   (let* ((parsed (icalendar-parse-and-index))
          (event (car (icalendar-ast-node-children-of
                       'icalendar-vevent (car parsed)))))
     (icalendar-with-component event
         ((icalendar-dtstart :value start)
          (icalendar-duration :value duration)
          (icalendar-rrule :value rule))
       (list
        ;; Check the second day of both weekly occurrences.
        (calendar-dlet ((date '(1 7 2025)) (entry "Conference"))
          (not (null (diary-rrule :rule rule :start start :duration duration))))
        (calendar-dlet ((date '(1 14 2025)) (entry "Conference"))
          (not (null (diary-rrule :rule rule :start start :duration duration))))))))
 '(t t))

(princ (qa--done))
