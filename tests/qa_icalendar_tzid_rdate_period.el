;;; qa_icalendar_tzid_rdate_period.el --- apply TZID offsets to RDATE PERIOD values -*- lexical-binding: t; -*-

(require 'icalendar-parser)
(require 'icalendar-utils)
(require 'icalendar-recur)

(qa
 (with-temp-buffer
   (insert "BEGIN:VCALENDAR\n"
           "VERSION:2.0\n"
           "BEGIN:VTIMEZONE\n"
           "TZID:Test/Zone\n"
           "BEGIN:STANDARD\n"
           "DTSTART:19700101T000000\n"
           "TZOFFSETFROM:+0100\n"
           "TZOFFSETTO:+0100\n"
           "END:STANDARD\n"
           "END:VTIMEZONE\n"
           "BEGIN:VEVENT\n"
           "DTSTART:20250101T090000\n"
           "RDATE;TZID=Test/Zone;VALUE=PERIOD:"
           "20250102T090000/PT2H,20250103T090000/20250103T110000\n"
           "END:VEVENT\n"
           "END:VCALENDAR\n")
   (goto-char (point-min))
   (let* ((calendar (icalendar-parse-component (point-max)))
          (timezone (car (icalendar-ast-node-children-of
                          'icalendar-vtimezone calendar)))
          (event (car (icalendar-ast-node-children-of
                       'icalendar-vevent calendar)))
          (rdate (icalendar-ast-node-first-child-of 'icalendar-rdate event)))
     (icalendar-recur-tz-set-zones-in (list timezone) event)
     (let ((periods (mapcar #'icalendar-ast-node-value
                            (icalendar-ast-node-value rdate))))
       (list
        (mapcar (lambda (period)
                  (decoded-time-zone (icalendar-period-start period)))
                periods)
        (mapcar (lambda (period)
                  (decoded-time-zone (icalendar-period-end period)))
                periods)))))
 '((3600 3600) (3600 3600)))

(princ (qa--done))
