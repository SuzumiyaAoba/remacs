;;; qa_icalendar_recurrences_window_repeat.el --- keep recurrence caches reusable -*- lexical-binding: t; -*-

(require 'icalendar-parser)
(require 'icalendar-recur)

(qa
 (with-temp-buffer
   (insert "BEGIN:VEVENT\nUID:repeat-window-call\n"
           "DTSTAMP:20250101T000000Z\n"
           "DTSTART;VALUE=DATE:20250101\n"
           "RRULE:FREQ=DAILY\n"
           "END:VEVENT\n")
   (goto-char (point-min))
   (let* ((event (icalendar-parse-component (point-max)))
          (first (icalendar-recur-recurrences-in-window
                  '(1 1 2025) '(1 4 2025) event))
          (second (icalendar-recur-recurrences-in-window
                   '(1 1 2025) '(1 4 2025) event)))
     (list (equal first second) (length first))))
 '(t 3))

(princ (qa--done))
