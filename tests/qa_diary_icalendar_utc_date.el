;;; -*- lexical-binding: t; -*-
;; UTC recurrence dates are assigned after conversion to local time (#1967).

(require 'diary-icalendar)
(require 'icalendar-parser)

(defvar date)
(defvar entry)

(qa
 (unwind-protect
     (progn
       (set-time-zone-rule "PST8PDT,M3.2.0,M11.1.0")
       (with-temp-buffer
         (insert (concat
                  "BEGIN:VCALENDAR\nVERSION:2.0\n"
                  "BEGIN:VEVENT\nUID:utc-local-date\n"
                  "DTSTART:20250101T010000Z\n"
                  "RRULE:FREQ=DAILY;COUNT=2\n"
                  "SUMMARY:UTC recurrence\nEND:VEVENT\nEND:VCALENDAR\n"))
         (let* ((parsed (icalendar-parse-and-index))
                (event (car (icalendar-ast-node-children-of
                             'icalendar-vevent (car parsed)))))
           (icalendar-with-component event
               ((icalendar-dtstart :value start)
                (icalendar-rrule :value rule))
             (list
              (calendar-dlet ((date '(12 31 2024))
                              (entry "UTC recurrence"))
                (not (null (diary-rrule :rule rule :start start))))
              (calendar-dlet ((date '(1 1 2025))
                              (entry "UTC recurrence"))
                (not (null (diary-rrule :rule rule :start start))))
              (calendar-dlet ((date '(1 2 2025))
                              (entry "UTC recurrence"))
                (not (null (diary-rrule :rule rule :start start)))))))))
   (set-time-zone-rule 'wall))
 '(t t nil))

(princ (qa--done))
